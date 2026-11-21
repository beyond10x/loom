//! Commands an operator binds to an installed protocol's actions (`intake.bindings`,
//! `ess/intake/domains/bindings.yaml`), and the effect port that performs an admitted action
//! through its command.
//!
//! A run configuration file names one protocol and, for each of its actions, one argv. [`decode`]
//! reads it, refusing any member it does not declare; [`check`] holds it against the run's catalog
//! before any model call. A run of that protocol performs an admitted action by handing its
//! command to the runner the host injected ([`TestRunner::run_child`]); this module starts no
//! process of its own. The command is a [`ChildCommand`]: its program, never a shell, with the
//! action's arguments as JSON on standard input, an environment of `PATH` alone (the host's
//! `PATH`, [`INHERITED`]), and the configuration's timeout ([`DEFAULT_TIMEOUT_SECONDS`] when
//! absent), past which the runner kills the program and the processes it started; a timeout this
//! host's clock cannot hold is refused, naming it, before the program starts.
//!
//! Which runner, decided before any model call ([`CommandRunner::select`]): the operator's opt-in
//! selects Substrate's `local` adapter, unconfined and declared insecure, with a reason naming the
//! opt-in. The run's `--unconfined-bindings` flag is a host runner opened on `local` with
//! [`UNCONFINED_FLAG_REASON`], and wins; the file's `unconfined_bindings: true` opens one with
//! [`UNCONFINED_FILE_REASON`], which is the only reason a file gives. Without an opt-in the
//! host's own runner runs them, refused [`ConfinementRefusal::CommandUnconfined`] when it
//! launches through `local` for another reason; no runner, or one that starts no child, is
//! refused [`ConfinementRefusal::NoRunner`]. A runner that is not `local` gets the program file
//! mounted read-only at its own path when it lies outside the system view Substrate binds. The
//! commands run in the host's workspace, or in a private directory made for the run.
//!
//! Exit status 0 is `Performed`, with the program's standard output, one JSON object, as the
//! report; any other end is `Refused`, naming why. The report's `evidence` member (a list of
//! `intake.bindings.CommandEvidence`) states evidence about the intent, each of a kind the action
//! declares it may produce; it is submitted under this module's producer, citing one observation
//! of the command's run. A binding confers no authority: the governor admits the action first,
//! and an action that needs a capability still needs its grant.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
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
use crate::confinement::{
    ChildCommand, ConfinementAdapter, ConfinementError, ConfinementRefusal, EngineConfinement,
    EngineRunner, ExecEnvironment, TestExecution, TestRunner, escape_reason,
};
use crate::effect::{Console, refuse};
use crate::executor::{applied_label, fresh_uuid};
use crate::run::{SliceError, json_text, printable};
use crate::selector::Briefing;

/// The seconds a command may run when the configuration names no `timeout_seconds`.
pub const DEFAULT_TIMEOUT_SECONDS: i64 = 60;
/// The most seconds a configuration may give a command.
pub const MAX_TIMEOUT_SECONDS: i64 = 3600;
/// The most bytes of a command's standard output read as its report.
pub const MAX_REPORT_BYTES: usize = 1024 * 1024;
/// The only environment variable a command starts with, at the value the host process has.
pub const INHERITED: &[&str] = &["PATH"];
/// The reason a `local` runner carries when the run's `--unconfined-bindings` flag opted in: a
/// host that opts in as the flag does opens its `local` runner with it
/// ([`CommandRunner::select`]).
pub const UNCONFINED_FLAG_REASON: &str =
    "--unconfined-bindings: the operator runs the bound commands unconfined, as the operator";
/// The reason the `local` runner carries when the file's `unconfined_bindings: true` opted in;
/// the only reason a run configuration file gives.
pub const UNCONFINED_FILE_REASON: &str = "unconfined_bindings: true in the run configuration file: the operator runs the bound commands unconfined, as the operator";
/// The directories Substrate's `linux` view binds read-only from the host: a program under one is
/// already there, and is never mounted again.
const SYSTEM_VIEW: [&str; 4] = ["/usr", "/bin", "/lib", "/lib64"];
const SOURCE: &str = "loom/action-command";
const PRODUCER: &str = "loom/action-command-verifier";
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

/// The runner a run's bound commands start through, and the workspace they start in.
pub(crate) struct CommandRunner {
    runner: Arc<dyn TestRunner>,
    workspace: PathBuf,
    /// The private directory holding `workspace` when the host named none, removed with it.
    _private: Option<tempfile::TempDir>,
}

impl fmt::Debug for CommandRunner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandRunner")
            .field("runner", &self.runner.label())
            .field("workspace", &self.workspace)
            .finish_non_exhaustive()
    }
}

impl CommandRunner {
    /// The runner `bindings`' commands start through, given the host's runner `host` and
    /// workspace `workspace`, checked before any model call and with no process started:
    ///
    /// 1. a host runner on `local` with [`UNCONFINED_FLAG_REASON`]: the flag opted in, and wins;
    /// 2. the file's `unconfined_bindings: true`: Substrate's `local` adapter, opened here with
    ///    [`UNCONFINED_FILE_REASON`];
    /// 3. the host's runner, refused [`ConfinementRefusal::CommandUnconfined`] when it launches
    ///    through `local` for another reason (no opt-in selected it);
    /// 4. no runner: refused [`ConfinementRefusal::NoRunner`].
    ///
    /// The workspace is `workspace`, or a private directory made here and removed with the
    /// runner. The runner then checks it can start a child there
    /// ([`TestRunner::preflight_child`]): a runner that starts no child is refused `NoRunner`,
    /// and one whose adapter is not served is refused as its preflight refuses.
    ///
    /// # Errors
    /// The refusal, as above.
    pub(crate) fn select(
        bindings: &ActionBindings,
        host: Option<&Arc<dyn TestRunner>>,
        workspace: Option<&Path>,
    ) -> Result<Self, ConfinementError> {
        let runner = select_runner(bindings, host)?;
        let (workspace, private) = match workspace {
            Some(workspace) => (workspace.to_owned(), None),
            None => {
                let private = tempfile::Builder::new()
                    .prefix("loom-bound-")
                    .tempdir()
                    .map_err(|error| refusal(ConfinementRefusal::ScopeInvalid, error))?;
                // A name Substrate admits for a workspace.
                let workspace = private.path().join("workspace");
                std::fs::create_dir(&workspace)
                    .map_err(|error| refusal(ConfinementRefusal::ScopeInvalid, error))?;
                (workspace, Some(private))
            }
        };
        runner.preflight_child(&workspace)?;
        Ok(Self {
            runner,
            workspace,
            _private: private,
        })
    }

    /// The child `binding`'s program is started as: its argv, `arguments` on standard input,
    /// `PATH` alone, `timeout`; and, under a runner that is not `local`, its program file mounted
    /// read-only at its own path when that is absolute and outside [`SYSTEM_VIEW`].
    ///
    /// # Errors
    /// The host's `PATH` is not UTF-8, which no child environment carries.
    fn child(
        &self,
        binding: &ActionBinding,
        arguments: &str,
        timeout: Duration,
    ) -> Result<ChildCommand, String> {
        let mut set = BTreeMap::new();
        for name in INHERITED {
            if let Some(value) = std::env::var_os(name) {
                let value = value
                    .into_string()
                    .map_err(|_| format!("the host's {name} is not UTF-8"))?;
                set.insert((*name).to_owned(), value);
            }
        }
        let program = &binding.argv[0];
        let mut child = ChildCommand::new(
            program.clone(),
            binding.argv[1..].iter().cloned(),
            &self.workspace,
        )
        .with_environment(ExecEnvironment {
            allow: Vec::new(),
            set,
        })
        .with_stdin(arguments.as_bytes().to_vec())
        .with_timeout(timeout);
        let path = Path::new(program);
        if self.runner.local_reason().is_none()
            && path.is_absolute()
            && !SYSTEM_VIEW.iter().any(|system| path.starts_with(system))
        {
            child = child.at_host_path(path);
        }
        Ok(child)
    }

    fn run(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        self.runner.run_child(child)
    }
}

/// [`CommandRunner::select`]'s choice of runner.
fn select_runner(
    bindings: &ActionBindings,
    host: Option<&Arc<dyn TestRunner>>,
) -> Result<Arc<dyn TestRunner>, ConfinementError> {
    if let Some(host) = host
        && host.local_reason() == Some(UNCONFINED_FLAG_REASON)
    {
        return Ok(Arc::clone(host));
    }
    if bindings.unconfined_bindings == Some(true) {
        let runner = EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(UNCONFINED_FILE_REASON.to_owned()),
            cgroup_root: None,
        })?;
        return Ok(Arc::new(runner));
    }
    match host {
        None => Err(refusal(
            ConfinementRefusal::NoRunner,
            "the request binds commands and names no runner to start them; a bound command starts only through the host's runner, or under the local adapter with --unconfined-bindings or unconfined_bindings: true",
        )),
        Some(host) => match host.local_reason() {
            Some(reason) => Err(refusal(
                ConfinementRefusal::CommandUnconfined,
                format!(
                    "the host's runner launches through the local adapter for another reason ({}); bound commands run unconfined only with --unconfined-bindings or unconfined_bindings: true",
                    escape_reason(reason)
                ),
            )),
            None => Ok(Arc::clone(host)),
        },
    }
}

fn refusal(reason: ConfinementRefusal, detail: impl fmt::Display) -> ConfinementError {
    ConfinementError {
        reason,
        detail: detail.to_string(),
    }
}

/// The effect port that performs an admitted action through the command bound to it.
pub(crate) struct CommandEffects<'a, 'o, S, T> {
    governor: &'a CanonGovernor<S, T>,
    case: CaseId,
    intent_revision: &'a str,
    bindings: &'a ActionBindings,
    productions: BTreeMap<String, Vec<String>>,
    commands: &'a CommandRunner,
    briefing: Briefing,
    console: &'a Console<'o>,
}

impl<'a, 'o, S: FallibleCaseStore, T: EvaluationTime> CommandEffects<'a, 'o, S, T> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        governor: &'a CanonGovernor<S, T>,
        case: CaseId,
        intent_revision: &'a str,
        bindings: &'a ActionBindings,
        productions: BTreeMap<String, Vec<String>>,
        commands: &'a CommandRunner,
        briefing: Briefing,
        console: &'a Console<'o>,
    ) -> Self {
        Self {
            governor,
            case,
            intent_revision,
            bindings,
            productions,
            commands,
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
            Ok(EffectOutcome::Refused(EffectOutcomeRefused {
                reason,
                ends_run: false,
            }))
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
        // The deadline is taken before the program starts, so a timeout the clock cannot hold
        // starts nothing; the runner takes its own from the same timeout.
        if let Err(why) = self.deadline() {
            return refused(format!("{program} was not started: {why}"));
        }
        let child = match self.commands.child(binding, &arguments, self.timeout()) {
            Ok(child) => child,
            Err(why) => return refused(format!("{program} was not started: {why}")),
        };
        // The one start of a bound command: through the runner, never here.
        let execution = match self.commands.run(&child) {
            Ok(execution) => execution,
            Err(refusal) => {
                return refused(format!("{program} was refused by its runner: {refusal}"));
            }
        };
        self.console.write(|out| {
            writeln!(
                out,
                "  confinement: {}",
                printable(&applied_label(&execution.applied))
            )?;
            Ok(())
        })?;
        if execution.timed_out {
            return refused(format!(
                "{program} was killed after {} s",
                self.timeout().as_secs()
            ));
        }
        let output = &execution.child_output;
        let Some(code) = execution.exit_code else {
            return refused(format!("{program}'s status could not be read"));
        };
        if code != 0 {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let tail: String = stderr
                .trim()
                .chars()
                .rev()
                .take(STDERR_TAIL)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            return refused(format!("{program} ended exit status: {code}: {tail}"));
        }
        let (report, evidence) =
            self.report(&request.action, &output.stdout, output.stdout_truncated)?;
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

    /// The instant the timeout ends, from now, added with checked arithmetic.
    ///
    /// # Errors
    /// The timeout is past what this host's clock can hold, naming it.
    fn deadline(&self) -> Result<Instant, String> {
        Instant::now().checked_add(self.timeout()).ok_or_else(|| {
            format!(
                "timeout_seconds {} is past what this host's clock can hold",
                self.bindings
                    .timeout_seconds
                    .unwrap_or(DEFAULT_TIMEOUT_SECONDS)
            )
        })
    }

    /// The report `stdout` holds, and the evidence it states, each of a kind `action` declares it
    /// may produce; `truncated` says the runner kept only the start of the output.
    fn report(
        &self,
        action: &str,
        stdout: &[u8],
        truncated: bool,
    ) -> Result<(json::Value, Vec<CommandEvidence>), SliceError> {
        let failed = |why: String| SliceError::Context(format!("{action}'s command report {why}"));
        if truncated || stdout.len() > MAX_REPORT_BYTES {
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
