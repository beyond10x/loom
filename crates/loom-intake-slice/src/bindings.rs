//! Commands an operator binds to an installed protocol's actions (`intake.bindings`,
//! `ess/intake/domains/bindings.yaml`), and the effect port that performs an admitted action
//! through its command.
//!
//! A run configuration file names one protocol and, for each of its actions, one argv. [`decode`]
//! reads it, refusing any member it does not declare; [`check`] holds it against the run's catalog
//! before any model call. A run of that protocol performs an admitted action by starting its
//! program directly, never through a shell, with the action's arguments as JSON on standard input,
//! a cleared environment that keeps only `PATH`, and the configuration's timeout
//! ([`DEFAULT_TIMEOUT_SECONDS`] when absent), past which the program is killed and reaped. Exit
//! status 0 is `Performed`, with the program's standard output, one JSON object, as the report;
//! any other end is `Refused`. The report's `evidence` member (a list of
//! `intake.bindings.CommandEvidence`) states evidence about the intent, each of a kind the action
//! declares it may produce; it is submitted under this module's producer, citing one observation
//! of the command's run. A binding confers no authority: the governor admits the action first,
//! and an action that needs a capability still needs its grant.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use b10x_loom_commission::model::responsibility::{
    ActionRequestData, CaseId, Commission, EffectOutcome, EffectOutcomePerformed,
    EffectOutcomeRefused, EvidenceData, EvidenceId, ExecutorOutcomeProposedAction, Observation,
    ObservationData, ObservationId, commission_state,
};
use b10x_loom_commission::model::{json, primitives::Timestamp};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_loom_commission::ports::governor::Governor;
pub use intake_model::bindings::{ActionBinding, ActionBindings, CommandEvidence};
use loom_governor::{CanonGovernor, EvaluationTime, FallibleCaseStore};
use loom_protocols::{ProtocolCatalog, ProtocolEntry, SourceKind};
use serde_json::Value;

use crate::case::INTENT;
use crate::effect::{Console, refuse};
use crate::executor::fresh_uuid;
use crate::run::{SliceError, json_text, printable};
use crate::selector::Briefing;

/// The seconds a command may run when the configuration names no `timeout_seconds`.
pub const DEFAULT_TIMEOUT_SECONDS: i64 = 60;
/// The most seconds a configuration may give a command.
pub const MAX_TIMEOUT_SECONDS: i64 = 3600;
/// The most bytes of a command's standard output read as its report.
pub const MAX_REPORT_BYTES: usize = 1024 * 1024;
/// The only environment variable a command starts with, from the CLI's own environment.
pub const INHERITED: &[&str] = &["PATH"];
const SOURCE: &str = "loom/action-command";
const PRODUCER: &str = "loom/action-command-verifier";
const POLL: Duration = Duration::from_millis(10);
/// The most bytes of a refused command's standard error its refusal names.
const STDERR_TAIL: usize = 512;

/// Decodes a run configuration file's text: a JSON object of exactly `protocol`, `bindings` and
/// an optional `timeout_seconds` and `unconfined_bindings`, each binding exactly `action` and
/// `argv`.
///
/// # Errors
/// What is not as declared, named.
pub fn decode(text: &str) -> Result<ActionBindings, String> {
    let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let object = exactly(
        &value,
        &["protocol", "bindings"],
        &["timeout_seconds", "unconfined_bindings"],
        "the file",
    )?;
    let protocol = object["protocol"]
        .as_str()
        .ok_or("protocol is not a string")?
        .to_owned();
    let timeout_seconds = match object.get("timeout_seconds") {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_i64().ok_or("timeout_seconds is not an integer")?),
    };
    let unconfined_bindings = match object.get("unconfined_bindings") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_bool()
                .ok_or("unconfined_bindings is not a boolean")?,
        ),
    };
    let bindings = object["bindings"]
        .as_array()
        .ok_or("bindings is not a list")?
        .iter()
        .map(|binding| {
            let binding = exactly(binding, &["action", "argv"], &[], "a binding")?;
            let action = binding["action"]
                .as_str()
                .ok_or("a binding's action is not a string")?
                .to_owned();
            let argv = binding["argv"]
                .as_array()
                .ok_or_else(|| format!("{action}: argv is not a list"))?
                .iter()
                .map(|word| {
                    word.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("{action}: an argv word is not a string"))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(ActionBinding { action, argv })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(ActionBindings {
        protocol,
        timeout_seconds,
        unconfined_bindings,
        bindings,
    })
}

fn exactly<'v>(
    value: &'v Value,
    required: &[&str],
    optional: &[&str],
    what: &str,
) -> Result<&'v serde_json::Map<String, Value>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{what} is not a JSON object"))?;
    if let Some(unknown) = object
        .keys()
        .find(|key| !required.contains(&key.as_str()) && !optional.contains(&key.as_str()))
    {
        return Err(format!("{what} has an undeclared member {unknown}"));
    }
    if let Some(missing) = required.iter().find(|key| !object.contains_key(**key)) {
        return Err(format!("{what} has no member {missing}"));
    }
    Ok(object)
}

/// Holds `bindings` against `catalog`: the protocol is an installed one the catalog holds (never a
/// bundled one, whose actions Loom binds itself), declares the `intent` artifact alone and no case
/// input; every binding names an action the protocol declares, once, with a program; the timeout
/// is from 1 to [`MAX_TIMEOUT_SECONDS`].
///
/// # Errors
/// The first that is not so, named.
pub fn check(bindings: &ActionBindings, catalog: &ProtocolCatalog) -> Result<(), String> {
    let protocol = &bindings.protocol;
    let entry = catalog
        .get(protocol)
        .ok_or_else(|| format!("the catalog holds no protocol {protocol}"))?;
    if matches!(
        entry.definition.source.kind,
        SourceKind::Engineering | SourceKind::Loom
    ) {
        return Err(format!(
            "{protocol} is bundled; only an installed protocol's actions are bound to commands"
        ));
    }
    let artifacts: Vec<String> = entry.artifacts().into_iter().map(|(id, _)| id).collect();
    if artifacts != [INTENT] {
        return Err(format!(
            "{protocol} declares the artifacts {}; a bound protocol declares the intent artifact alone",
            artifacts.join(", ")
        ));
    }
    if let Some(input) = entry.inputs().first() {
        return Err(format!(
            "{protocol} declares the case input {input}; a bound protocol declares none"
        ));
    }
    if let Some(seconds) = bindings.timeout_seconds
        && !(1..=MAX_TIMEOUT_SECONDS).contains(&seconds)
    {
        return Err(format!(
            "timeout_seconds {seconds} is not from 1 to {MAX_TIMEOUT_SECONDS}"
        ));
    }
    let declared = entry.actions();
    let mut seen = BTreeSet::new();
    for binding in &bindings.bindings {
        let action = &binding.action;
        if !declared.contains(action) {
            return Err(format!("{protocol} declares no action {action}"));
        }
        if !seen.insert(action) {
            return Err(format!("{action} is bound twice"));
        }
        if binding.argv.first().is_none_or(String::is_empty) {
            return Err(format!("{action} is bound to no program"));
        }
    }
    Ok(())
}

/// The first action `entry` declares that `bindings` binds no command to.
pub fn unbound(bindings: &ActionBindings, entry: &ProtocolEntry) -> Option<String> {
    entry
        .actions()
        .into_iter()
        .find(|action| !bindings.bindings.iter().any(|b| &b.action == action))
}

/// The evidence kinds each action of `entry` declares it may produce.
pub(crate) fn productions(entry: &ProtocolEntry) -> BTreeMap<String, Vec<String>> {
    entry
        .model
        .actions
        .iter()
        .map(|(id, action)| {
            (
                id.to_string(),
                action
                    .may_produce
                    .iter()
                    .map(|production| production.evidence.to_string())
                    .collect(),
            )
        })
        .collect()
}

/// How a command ended.
enum Ended {
    Exited(ExitStatus, Vec<u8>, Vec<u8>),
    TimedOut,
    NotStarted(std::io::Error),
}

/// The effect port that performs an admitted action through the command bound to it.
pub(crate) struct CommandEffects<'a, 'o, S, T> {
    governor: &'a CanonGovernor<S, T>,
    case: CaseId,
    intent_revision: &'a str,
    bindings: &'a ActionBindings,
    productions: BTreeMap<String, Vec<String>>,
    briefing: Briefing,
    console: &'a Console<'o>,
}

impl<'a, 'o, S: FallibleCaseStore, T: EvaluationTime> CommandEffects<'a, 'o, S, T> {
    pub(crate) fn new(
        governor: &'a CanonGovernor<S, T>,
        case: CaseId,
        intent_revision: &'a str,
        bindings: &'a ActionBindings,
        productions: BTreeMap<String, Vec<String>>,
        briefing: Briefing,
        console: &'a Console<'o>,
    ) -> Self {
        Self {
            governor,
            case,
            intent_revision,
            bindings,
            productions,
            briefing,
            console,
        }
    }

    fn binding(&self, action: &str) -> Option<&ActionBinding> {
        self.bindings.bindings.iter().find(|b| b.action == action)
    }

    fn perform(&self, request: &ActionRequestData) -> Result<EffectOutcome, SliceError> {
        let step = self.console.next_step();
        let arguments = json_text(&request.arguments.0);
        self.console.write(|out| {
            writeln!(
                out,
                "step {step}: {} {}",
                printable(&request.action),
                printable(&arguments)
            )?;
            Ok(())
        })?;
        let refused = |reason: String| -> Result<EffectOutcome, SliceError> {
            self.console
                .write(|out| refuse(out, &self.briefing, &request.action, &reason))?;
            Ok(EffectOutcome::Refused(EffectOutcomeRefused { reason }))
        };
        let Some(binding) = self.binding(&request.action) else {
            return refused(format!("no command is bound to {}", request.action));
        };
        if request.case_id != self.case {
            return refused("the request is not this run's case".into());
        }
        if self
            .governor
            .revisions(&self.case)?
            .get(INTENT)
            .map(String::as_str)
            != Some(self.intent_revision)
        {
            return Err(SliceError::Context(
                "the intent's revision changed before the command ran".into(),
            ));
        }
        let program = &binding.argv[0];
        let (status, stdout, stderr) = match self.run(binding, &arguments) {
            Ended::Exited(status, stdout, stderr) => (status, stdout, stderr),
            Ended::TimedOut => {
                return refused(format!(
                    "{program} was killed after {} s",
                    self.timeout().as_secs()
                ));
            }
            Ended::NotStarted(error) => {
                return refused(format!("{program} did not start: {error}"));
            }
        };
        if !status.success() {
            let stderr = String::from_utf8_lossy(&stderr);
            let tail: String = stderr
                .trim()
                .chars()
                .rev()
                .take(STDERR_TAIL)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            return refused(format!("{program} ended {status}: {tail}"));
        }
        let (report, evidence) = self.report(&request.action, &stdout)?;
        let observed_at =
            crate::clock::instant(&crate::clock::SystemClock).map_err(SliceError::Context)?;
        let revision = self.governor.current_revision(&self.case)?;
        let observation = ObservationId(fresh_uuid("command-observation", &self.case.0));
        self.governor.observe(Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: SOURCE.into(),
            subject: INTENT.into(),
            observed_at: observed_at.clone(),
            payload: report.clone(),
        }))?;
        for item in &evidence {
            self.submit(item, revision, &observation, &observed_at)?;
        }
        let text = json_text(&report);
        self.console.write(|out| {
            writeln!(out, "  effect: {}", printable(&text))?;
            for item in &evidence {
                writeln!(
                    out,
                    "  evidence: {} {}",
                    printable(&item.kind),
                    printable(&item.result)
                )?;
            }
            Ok(())
        })?;
        if let Ok(value) = serde_json::from_str(&text) {
            self.briefing.record_handoff(
                &ExecutorOutcomeProposedAction {
                    action: request.action.clone(),
                    arguments: request.arguments.clone(),
                },
                value,
            );
        }
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }

    fn timeout(&self) -> Duration {
        let seconds = self
            .bindings
            .timeout_seconds
            .unwrap_or(DEFAULT_TIMEOUT_SECONDS);
        Duration::from_secs(u64::try_from(seconds).unwrap_or(1))
    }

    /// Runs `binding`'s program with `arguments` on standard input, to its end or for the
    /// timeout, whichever is first; one still running is killed and reaped.
    fn run(&self, binding: &ActionBinding, arguments: &str) -> Ended {
        let deadline = Instant::now() + self.timeout();
        let mut command = Command::new(&binding.argv[0]);
        command
            .args(&binding.argv[1..])
            .env_clear()
            .envs(std::env::vars_os().filter(|(name, _)| INHERITED.iter().any(|kept| name == kept)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => return Ended::NotStarted(error),
        };
        if let Some(mut pipe) = child.stdin.take() {
            let document = arguments.to_owned();
            // Dropping the pipe at the end of the write closes standard input.
            thread::spawn(move || pipe.write_all(document.as_bytes()));
        }
        let stdout = drain(child.stdout.take());
        let stderr = drain(child.stderr.take());
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => thread::sleep(POLL),
                Ok(None) | Err(_) => {
                    stop(&mut child);
                    return Ended::TimedOut;
                }
            }
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        let Ok(stdout) = stdout.recv_timeout(remaining) else {
            return Ended::TimedOut;
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        let Ok(stderr) = stderr.recv_timeout(remaining) else {
            return Ended::TimedOut;
        };
        Ended::Exited(status, stdout, stderr)
    }

    /// The report `stdout` holds, and the evidence it states, each of a kind `action` declares it
    /// may produce.
    fn report(
        &self,
        action: &str,
        stdout: &[u8],
    ) -> Result<(json::Value, Vec<CommandEvidence>), SliceError> {
        let failed = |why: String| SliceError::Context(format!("{action}'s command report {why}"));
        if stdout.len() > MAX_REPORT_BYTES {
            return Err(failed(format!("is longer than {MAX_REPORT_BYTES} bytes")));
        }
        let text = std::str::from_utf8(stdout).map_err(|_| failed("is not UTF-8".into()))?;
        let report = json::parse(text.trim()).map_err(|e| failed(format!("is not JSON: {e:?}")))?;
        let json::Value::Object(members) = &report else {
            return Err(failed("is not a JSON object".into()));
        };
        let declared = self.productions.get(action).cloned().unwrap_or_default();
        let mut evidence = Vec::new();
        if let Some((_, stated)) = members.iter().find(|(name, _)| name == "evidence") {
            let json::Value::Array(items) = stated else {
                return Err(failed("states evidence that is not a list".into()));
            };
            for item in items {
                let json::Value::Object(fields) = item else {
                    return Err(failed("states evidence that is not an object".into()));
                };
                let text_of = |name: &str| {
                    fields.iter().find_map(|(key, value)| match value {
                        json::Value::Text(text) if key == name => Some(text.clone()),
                        _ => None,
                    })
                };
                let (Some(kind), Some(result), 2) =
                    (text_of("kind"), text_of("result"), fields.len())
                else {
                    return Err(failed(
                        "states evidence that is not exactly a kind and a result".into(),
                    ));
                };
                if !declared.contains(&kind) {
                    return Err(failed(format!(
                        "states evidence of the kind {kind}, which {action} does not declare"
                    )));
                }
                evidence.push(CommandEvidence { kind, result });
            }
        }
        Ok((report, evidence))
    }

    fn submit(
        &self,
        item: &CommandEvidence,
        revision: i64,
        observation: &ObservationId,
        observed_at: &Timestamp,
    ) -> Result<(), SliceError> {
        let id = EvidenceId(fresh_uuid("command-evidence", &self.case.0));
        let facts = serde_json::json!({"format": "canon-evidence/1",
            "id": format!("command-{}", id.0.0), "kind": item.kind, "result": item.result,
            "subject": INTENT, "subject_revision": self.intent_revision,
            "observed_at": observed_at.0});
        submit_evidence(
            self.governor,
            PRODUCER,
            EvidenceData {
                evidence_id: id,
                case_id: self.case.clone(),
                kind: item.kind.clone(),
                subject_revision: revision,
                producer: PRODUCER.into(),
                observation_ids: vec![observation.clone()],
                facts: json::parse(&facts.to_string())
                    .map_err(|e| SliceError::Context(format!("command evidence: {e:?}")))?,
                provenance: json::Value::Object(vec![(
                    "source".into(),
                    json::Value::Text(SOURCE.into()),
                )]),
            },
        )
        .map_err(|e| SliceError::Context(e.to_string()))
    }
}

impl<S: FallibleCaseStore, T: EvaluationTime> EffectPort for CommandEffects<'_, '_, S, T> {
    fn performs(&self, action: &str) -> bool {
        self.binding(action).is_some()
    }
    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.perform(request.data()).map_err(|failure| {
            let error = EffectError::new(failure.to_string());
            self.console.fail(failure);
            error
        })
    }
}

/// Reads `pipe` to its end on its own thread, keeping at most one byte past
/// [`MAX_REPORT_BYTES`] and discarding the rest, so a long output never blocks the program; the
/// bytes arrive on the receiver.
fn drain(pipe: Option<impl Read + Send + 'static>) -> mpsc::Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let limit = u64::try_from(MAX_REPORT_BYTES + 1).unwrap_or(u64::MAX);
            let _ = (&mut pipe).take(limit).read_to_end(&mut bytes);
            let _ = std::io::copy(&mut pipe, &mut std::io::sink());
        }
        let _ = sender.send(bytes);
    });
    receiver
}

/// Kills `child` and reaps it, so no process and no zombie is left.
fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_with_an_undeclared_member_is_refused_naming_it() {
        let error = decode(r#"{"protocol":"a@1","bindings":[],"shell":"sh"}"#).unwrap_err();
        assert!(error.contains("shell"), "{error}");
        let error =
            decode(r#"{"protocol":"a@1","bindings":[{"action":"x","argv":["p"],"env":{}}]}"#)
                .unwrap_err();
        assert!(error.contains("env"), "{error}");
    }

    #[test]
    fn unconfined_bindings_decodes_as_a_boolean_and_nothing_else() {
        let bindings =
            decode(r#"{"protocol":"a@1","unconfined_bindings":true,"bindings":[]}"#).unwrap();
        assert_eq!(bindings.unconfined_bindings, Some(true));
        let error =
            decode(r#"{"protocol":"a@1","unconfined_bindings":"yes","bindings":[]}"#).unwrap_err();
        assert!(error.contains("unconfined_bindings"), "{error}");
    }

    #[test]
    fn a_file_decodes_into_the_declared_bindings() {
        let bindings = decode(
            r#"{"protocol":"a@1","timeout_seconds":5,"bindings":[{"action":"x","argv":["p","q"]}]}"#,
        )
        .unwrap();
        assert_eq!(
            bindings,
            ActionBindings {
                protocol: "a@1".into(),
                timeout_seconds: Some(5),
                unconfined_bindings: None,
                bindings: vec![ActionBinding {
                    action: "x".into(),
                    argv: vec!["p".into(), "q".into()],
                }],
            }
        );
    }
}
