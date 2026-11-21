//! The program effect port: performs an admitted action bound to a `program` executor, for any
//! registered protocol, and turns its result into evidence by the binding's table alone.
//!
//! The port is built per case ([`ProgramEffects::new`]) from the governor holding the case, the
//! checked bindings document, the case's workspace root, the store directory and the host's runner,
//! which the host may not give. The runner is admitted by
//! [`b10x_loom_intake_slice::bindings::admit_bound_program_runner`] and checked with
//! [`TestRunner::preflight_child`] on the workspace root before anything starts; a refusal of
//! either is the constructor's. Every program, the `arguments_from` argv included, starts through
//! that runner's [`TestRunner::run_child`] alone, as the child
//! [`b10x_loom_intake_slice::bindings::bound_program_child`] builds: the argv, the arguments as
//! JSON on standard input and never in an argv, `PATH` alone, the binding's timeout, the workspace
//! root as its start directory.
//!
//! A step, in order: the case held under the document's protocol, else `CaseOfAnotherProtocol`
//! and nothing starts; the action's run count (the observations this port kept for the action in
//! the case, [`PROGRAM_SOURCE`]) below its `budget.max_runs`, else `RunsSpent` and nothing starts; the
//! action's effect class and the placed artifacts' digests, read before the program starts; the
//! arguments (the request's, else the
//! binding's literal `arguments`, else the object the `arguments_from` argv prints, else `{}`; a
//! request has none when its arguments are the empty object), an `arguments_from` that does not
//! exit 0 or prints anything but one JSON object within [`MAX_REPORT_BYTES`] refused
//! `ArgumentsUnreadable` before the bound program starts; the run; the blob of its standard output
//! followed by its standard error, kept as `blobs/<sha256>` in the store directory through a
//! temporary file and a rename; one observation of the run (`loom.bindings.ProgramRun`), kept for
//! every call of `run_child` whatever it answered and whether the blob was kept or not, before a
//! runner refusal or a blob failure ends the step; then, for
//! each `evidence` entry, the value its `from` names (the exit code, `timeout` for a run killed at
//! its timeout, or the JSON pointer into the report, one JSON object within [`MAX_REPORT_BYTES`])
//! looked up in its `results`, falling to `*`. A value neither lists is `ResultNotInTable`, a
//! report that cannot be read `ReportUnreadable`; either records nothing. Otherwise one record per
//! entry is submitted, of the entry's kind about its subject, with the entry's producer, the
//! observation's instant, and the revisions the case holds for the subject and the `upstream`
//! artifacts once the step is written: nothing the program printed names a record's result,
//! revision, producer or instant.
//!
//! Revisions (`story:bound-artifact-revisions`): before anything starts the port digests every
//! artifact the document places at a path ([`crate::placements`]); one that cannot be digested
//! refuses the step `ArtifactUnreadable` and starts nothing. Once the run's observation is kept it
//! digests them again; one that cannot be digested then refuses the step `ArtifactUnreadable`, no
//! revision moved and nothing recorded. Otherwise each placed artifact takes the digest as its
//! revision, the moves written with the step's records in one store write
//! ([`CanonGovernor::record_step`]) at every end after the run: a runner refusal, a blob failure,
//! `ReportUnreadable` and `ResultNotInTable` write the moves alone. An action whose effect class is
//! not `write` (`read` or none) whose program changed a placed artifact (its digest after the run
//! differs from before) records nothing: the step is `Discarded` (`ChangedByNonWriter`) and its
//! moves are still written. The report of a performed step carries `step`
//! (`loom.bindings.BoundStep`): the outcome, the placements and the moves.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, CaseId, Commission, EffectOutcome, EffectOutcomePerformed,
    EffectOutcomeRefused, EvidenceData, EvidenceId, GovernorError, Observation, ObservationData,
    ObservationId, commission_state,
};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::{
    AttributedEvidence, EvidencePort, ObservationPort, submit_evidence,
};
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_intake_slice::bindings::{
    DEFAULT_TIMEOUT_SECONDS, MAX_REPORT_BYTES, admit_bound_program_runner, bound_program_child,
};
use b10x_loom_intake_slice::clock::{SystemClock, instant};
use b10x_loom_intake_slice::confinement::{ConfinementError, TestExecution, TestRunner};
use loom::bindings::{
    ArtifactRevision, BindingsDocument, BoundAction, BoundStep, DiscardReason, EvidenceEntry,
    Executor, ProgramRefusal, ProgramRefusalReason, ProgramRun, RecordedEvidence, ResultSource,
    StepOutcome,
};
use loom_governor::{
    CanonGovernor, EvaluationTime, FallibleCaseStore, NoEvaluationTime, StepWritten,
};

use crate::revision::{describe, placements, revisions_of};
use sha2::{Digest as _, Sha256};

/// The `source` of every observation the port keeps: one per run of a bound program. The run count
/// of an action in a case is the number of these naming it and the case.
pub const PROGRAM_SOURCE: &str = "loom/bound-program";
/// The directory of the store directory that holds the blobs, each named by its SHA-256.
pub const BLOBS: &str = "blobs";
/// The key of a result table a run killed at its timeout reads.
pub const TIMEOUT_KEY: &str = "timeout";
/// The key of a result table any value it does not list reads.
pub const ANY_KEY: &str = "*";

/// The program effect port of one case (see the module documentation).
pub struct ProgramEffects<'a, S, T = NoEvaluationTime> {
    governor: &'a CanonGovernor<S, T>,
    case: CaseId,
    document: &'a BindingsDocument,
    workspace: PathBuf,
    store: PathBuf,
    runner: Arc<dyn TestRunner>,
}

impl<S, T> std::fmt::Debug for ProgramEffects<'_, S, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgramEffects")
            .field("case", &self.case)
            .field("protocol", &self.document.protocol)
            .field("workspace", &self.workspace)
            .field("store", &self.store)
            .field("runner", &self.runner.label())
            .finish_non_exhaustive()
    }
}

impl<'a, S: FallibleCaseStore, T: EvaluationTime> ProgramEffects<'a, S, T> {
    /// The port of `case`, held by `governor`, for the checked `document`: its programs start in
    /// `workspace` through `runner`, and their blobs are kept under `store`. Nothing starts here.
    ///
    /// # Errors
    /// `runner` refused by the admission of a host runner for bound programs (no runner is
    /// `NoRunner`; a `local` runner opened for another reason than the `--unconfined-bindings`
    /// flag's is `CommandUnconfined`), or by its own [`TestRunner::preflight_child`] on
    /// `workspace`, passed on unchanged.
    pub fn new(
        governor: &'a CanonGovernor<S, T>,
        case: CaseId,
        document: &'a BindingsDocument,
        workspace: &Path,
        store: &Path,
        runner: Option<Arc<dyn TestRunner>>,
    ) -> Result<Self, ConfinementError> {
        let runner = admit_bound_program_runner(runner.as_ref())?;
        runner.preflight_child(workspace)?;
        Ok(Self {
            governor,
            case,
            document,
            workspace: workspace.to_owned(),
            store: store.to_owned(),
            runner,
        })
    }

    /// The binding of `action` when it is bound to a `program`.
    fn binding(&self, action: &str) -> Option<&BoundAction> {
        self.document
            .actions
            .iter()
            .find(|bound| bound.action == action && bound.executor.kind == Executor::Program)
    }

    /// The runs of `action` in this case: the observations this port kept naming both.
    fn runs(&self, action: &str) -> Result<i64, EffectError> {
        let observations = self
            .governor
            .try_observations()
            .map_err(|error| EffectError::new(format!("the observations: {error:?}")))?;
        let count = observations
            .iter()
            .filter(|observation| observation.source == PROGRAM_SOURCE)
            .filter(|observation| {
                text_member(&observation.payload, "case") == Some(self.case.0.as_str())
                    && text_member(&observation.payload, "action") == Some(action)
            })
            .count();
        Ok(i64::try_from(count).unwrap_or(i64::MAX))
    }

    fn perform(&self, request: &ActionRequestData) -> Result<EffectOutcome, EffectError> {
        let action = request.action.as_str();
        let Some(binding) = self.binding(action) else {
            return Ok(refused(format!("no program is bound to {action}")));
        };
        if request.case_id != self.case {
            return Ok(refused(format!(
                "the request is for case {}, and this port runs case {}",
                request.case_id.0, self.case.0
            )));
        }
        // The document's kinds are the protocol's: a case held under another protocol takes none.
        let held = self.governor.protocol(&self.case).map_err(governor)?;
        if held != self.document.protocol {
            return Ok(refusal(
                ProgramRefusalReason::CaseOfAnotherProtocol,
                action,
                format!(
                    "case {} is held under {held}, and the bindings document binds {}",
                    self.case.0, self.document.protocol
                ),
            ));
        }
        let runs = self.runs(action)?;
        if runs >= binding.budget.max_runs {
            return Ok(refusal(
                ProgramRefusalReason::RunsSpent,
                action,
                format!(
                    "{action} has run {runs} times in case {}, its budget.max_runs is {}",
                    self.case.0, binding.budget.max_runs
                ),
            ));
        }
        let Some(argv) = binding.executor.argv.as_deref() else {
            return Ok(refused(format!("{action} is bound to no argv")));
        };
        let program = argv.first().map_or("the program", String::as_str);
        let seconds = binding.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS);
        let timeout = Duration::from_secs(u64::try_from(seconds).unwrap_or(1));
        // Taken before anything starts, so a timeout the clock cannot hold starts nothing.
        if Instant::now().checked_add(timeout).is_none() {
            return Ok(refused(format!(
                "{program} was not started: timeout_seconds {seconds} is past what this host's clock can hold"
            )));
        }
        let writes = self.writes(action)?;
        // Read before anything starts, so a placed path that cannot be digested starts nothing,
        // and what the program changed is told from what the workspace held already.
        let before = match placements(&self.workspace, self.document) {
            Ok(before) => before,
            Err(unreadable) => {
                return Ok(refusal(
                    ProgramRefusalReason::ArtifactUnreadable,
                    action,
                    format!("nothing was started: {}", describe(&unreadable)),
                ));
            }
        };
        let arguments = match self.arguments(binding, request, timeout) {
            Ok(arguments) => arguments,
            Err(outcome) => return Ok(outcome),
        };
        let child = match bound_program_child(
            argv,
            &arguments,
            timeout,
            &self.workspace,
            self.runner.local_reason(),
        ) {
            Ok(child) => child,
            Err(why) => return Ok(refused(format!("{program} was not started: {why}"))),
        };
        // Read before the program starts, so the run's observation has an instant even when the
        // clock cannot be read after it.
        let started_at = instant(&SystemClock).map_err(EffectError::new)?;
        // The one start of a bound program: through the admitted runner, never here.
        let answer = self.runner.run_child(&child);
        // Every call above is a run: its observation is kept whatever the runner answered and
        // whether the blob can be kept or not, so it counts against `budget.max_runs`.
        let mut run = ProgramRun {
            case: self.case.0.clone(),
            action: action.to_owned(),
            exit: None,
            blob_sha256: None,
            timed_out: false,
            runner_refusal: None,
        };
        let mut blob_failure = None;
        match &answer {
            Ok(execution) => {
                let output = &execution.child_output;
                let mut blob = output.stdout.clone();
                blob.extend_from_slice(&output.stderr);
                let digest = sha256(&blob);
                run.exit = execution.exit_code.map(i64::from);
                run.timed_out = execution.timed_out;
                match keep_blob(&self.store, &digest, &blob) {
                    Ok(()) => run.blob_sha256 = Some(digest),
                    Err(error) => blob_failure = Some(format!("the blob {digest}: {error}")),
                }
            }
            Err(refusal) => run.runner_refusal = Some(refusal.to_string()),
        }
        let observed_at = instant(&SystemClock).unwrap_or(started_at);
        let observation = ObservationId(fresh_uuid("program-observation", &self.case.0));
        self.governor
            .observe(Observation::new(ObservationData {
                observation_id: observation.clone(),
                source: PROGRAM_SOURCE.to_owned(),
                subject: action.to_owned(),
                observed_at: observed_at.clone(),
                payload: commission_value(&run_json(&run))?,
            }))
            .map_err(governor)?;
        // What the workspace holds once the program ended: the revisions this step writes,
        // whatever else it records, and never a value the program, its report or the request names.
        let after = match placements(&self.workspace, self.document) {
            Ok(after) => after,
            Err(unreadable) => {
                return Ok(refusal(
                    ProgramRefusalReason::ArtifactUnreadable,
                    action,
                    format!(
                        "no revision moved and nothing was recorded: {}",
                        describe(&unreadable)
                    ),
                ));
            }
        };
        let targets = revisions_of(&after);
        let changed = before
            .iter()
            .zip(&after)
            .any(|(before, after)| before.revision != after.revision);
        let execution = match answer {
            Ok(execution) => execution,
            Err(refusal) => {
                self.write(&targets, Vec::new())?;
                return Ok(refused(format!(
                    "{program} was refused by its runner: {refusal}"
                )));
            }
        };
        if let Some(failure) = blob_failure {
            self.write(&targets, Vec::new())?;
            return Err(EffectError::new(failure));
        }
        if changed && !writes {
            // A program of an action that does not write changed what it was to observe: its
            // records are dropped, and the artifact still takes what is there.
            let written = self.write(&targets, Vec::new())?;
            return performed(
                &run,
                &[],
                &BoundStep {
                    action: action.to_owned(),
                    outcome: StepOutcome::Discarded,
                    discarded: Some(DiscardReason::ChangedByNonWriter),
                    placements: after,
                    moves: written.moves,
                },
            );
        }
        // The revisions the case holds once this step's moves are written.
        let mut revisions = self.governor.revisions(&self.case).map_err(governor)?;
        let moving = targets
            .iter()
            .filter(|(artifact, revision)| revisions.get(*artifact) != Some(*revision))
            .count();
        let case_revision = self
            .governor
            .current_revision(&self.case)
            .map_err(governor)?
            + i64::try_from(moving).unwrap_or(i64::MAX);
        revisions.extend(targets.clone());
        let mut report = None;
        let mut recorded = Vec::new();
        for entry in &binding.evidence {
            let value = match value_of(entry, &execution, &mut report) {
                Ok(value) => value,
                Err(detail) => {
                    self.write(&targets, Vec::new())?;
                    return Ok(refusal(
                        ProgramRefusalReason::ReportUnreadable,
                        action,
                        detail,
                    ));
                }
            };
            let Some(result) = look_up(entry, value.as_deref()) else {
                self.write(&targets, Vec::new())?;
                return Ok(refusal(
                    ProgramRefusalReason::ResultNotInTable,
                    action,
                    format!(
                        "the value {:?} of the {} entry is not in its results, which list no {ANY_KEY}",
                        value.as_deref().unwrap_or("(none)"),
                        entry.kind
                    ),
                ));
            };
            let revision_of = |artifact: &str| {
                revisions.get(artifact).cloned().ok_or_else(|| {
                    EffectError::new(format!(
                        "case {} holds no revision for {artifact}",
                        self.case.0
                    ))
                })
            };
            recorded.push(RecordedEvidence {
                kind: entry.kind.clone(),
                subject: entry.subject.clone(),
                producer: entry.producer.clone(),
                result,
                subject_revision: revision_of(&entry.subject)?,
                observed_at: loom::primitives::Timestamp(observed_at.0.clone()),
                upstream_revisions: entry
                    .upstream
                    .iter()
                    .map(|artifact| {
                        Ok(ArtifactRevision {
                            artifact: artifact.clone(),
                            revision: revision_of(artifact)?,
                        })
                    })
                    .collect::<Result<_, EffectError>>()?,
            });
        }
        let attributed = recorded
            .iter()
            .map(|record| self.attribute(record, case_revision, &observation))
            .collect::<Result<Vec<_>, _>>()?;
        // The moves and the records in one store write: a crash leaves neither or both.
        let written = self.write(&targets, attributed)?;
        performed(
            &run,
            &recorded,
            &BoundStep {
                action: action.to_owned(),
                outcome: StepOutcome::Recorded,
                discarded: None,
                placements: after,
                moves: written.moves,
            },
        )
    }

    /// Whether the protocol declares `action`'s effect class `write`.
    fn writes(&self, action: &str) -> Result<bool, EffectError> {
        let facts = self.governor.action_facts(&self.case).map_err(governor)?;
        Ok(facts
            .actions
            .iter()
            .find(|facts| facts.action == action)
            .and_then(|facts| facts.effect.as_deref())
            == Some(WRITE_EFFECT))
    }

    /// Writes this step to the case in one store write: the revision of every placed artifact
    /// (`targets`) and the `records`.
    fn write(
        &self,
        targets: &BTreeMap<String, String>,
        records: Vec<AttributedEvidence>,
    ) -> Result<StepWritten, EffectError> {
        self.governor
            .record_step(&self.case, targets, records)
            .map_err(|error| {
                EffectError::new(format!(
                    "the step of case {} was not written, nothing of it was kept: {error}",
                    self.case.0
                ))
            })
    }

    /// The arguments the bound program reads on standard input, as JSON text; `Err` is the step's
    /// refusal, with no bound program started.
    fn arguments(
        &self,
        binding: &BoundAction,
        request: &ActionRequestData,
        timeout: Duration,
    ) -> Result<String, EffectOutcome> {
        let requested = &request.arguments.0;
        let none =
            matches!(requested, commission_json::Value::Object(members) if members.is_empty());
        if !none {
            return Ok(commission_text(requested));
        }
        if let Some(literal) = &binding.arguments {
            return Ok(loom_text(literal));
        }
        let Some(argv) = binding.arguments_from.as_deref() else {
            return Ok("{}".to_owned());
        };
        let action = binding.action.as_str();
        let program = argv
            .first()
            .map_or("the arguments_from program", String::as_str);
        let unreadable = |detail: String| {
            Err(refusal(
                ProgramRefusalReason::ArgumentsUnreadable,
                action,
                detail,
            ))
        };
        let child = match bound_program_child(
            argv,
            &commission_text(requested),
            timeout,
            &self.workspace,
            self.runner.local_reason(),
        ) {
            Ok(child) => child,
            Err(why) => return Err(refused(format!("{program} was not started: {why}"))),
        };
        let execution = match self.runner.run_child(&child) {
            Ok(execution) => execution,
            Err(refusal) => {
                return unreadable(format!("{program} was refused by its runner: {refusal}"));
            }
        };
        if execution.timed_out {
            return unreadable(format!("{program} was killed at its timeout"));
        }
        if execution.exit_code != Some(0) {
            return unreadable(format!(
                "{program} ended exit status {}",
                execution
                    .exit_code
                    .map_or_else(|| "unknown".to_owned(), |code| code.to_string())
            ));
        }
        match one_object(
            &execution.child_output.stdout,
            execution.child_output.stdout_truncated,
        ) {
            Ok(object) => Ok(object.to_string()),
            Err(why) => unreadable(format!("{program}'s output {why}")),
        }
    }

    /// `record` as evidence of the case at `case_revision`, attributed to its producer by
    /// Commission's `submit_evidence`, held for the step's one write.
    fn attribute(
        &self,
        record: &RecordedEvidence,
        case_revision: i64,
        observation: &ObservationId,
    ) -> Result<AttributedEvidence, EffectError> {
        let id = EvidenceId(fresh_uuid("program-evidence", &self.case.0));
        let mut facts = serde_json::json!({
            "format": "canon-evidence/1",
            "id": format!("program-{}", id.0.0),
            "kind": record.kind,
            "subject": record.subject,
            "subject_revision": record.subject_revision,
            "observed_at": record.observed_at.0,
        });
        if let Some(result) = &record.result {
            facts["result"] = serde_json::Value::String(result.clone());
        }
        if !record.upstream_revisions.is_empty() {
            facts["upstream_revisions"] = record
                .upstream_revisions
                .iter()
                .map(|upstream| {
                    (
                        upstream.artifact.clone(),
                        serde_json::Value::String(upstream.revision.clone()),
                    )
                })
                .collect::<serde_json::Map<_, _>>()
                .into();
        }
        let held = Held::default();
        submit_evidence(
            &held,
            &record.producer,
            EvidenceData {
                evidence_id: id,
                case_id: self.case.clone(),
                kind: record.kind.clone(),
                subject_revision: case_revision,
                producer: record.producer.clone(),
                observation_ids: vec![observation.clone()],
                facts: commission_value(&facts)?,
                provenance: commission_value(&serde_json::json!({"source": PROGRAM_SOURCE}))?,
            },
        )
        .map_err(|error| EffectError::new(format!("the {} record: {error}", record.kind)))?;
        held.0
            .into_inner()
            .pop()
            .ok_or_else(|| EffectError::new(format!("the {} record was not held", record.kind)))
    }
}

/// The evidence port a record passes through to be attributed by Commission's `submit_evidence`:
/// it holds the record for the step's one write and decides nothing.
#[derive(Default)]
struct Held(RefCell<Vec<AttributedEvidence>>);

impl EvidencePort for Held {
    fn receive(&self, evidence: AttributedEvidence) -> Result<(), GovernorError> {
        self.0.borrow_mut().push(evidence);
        Ok(())
    }
}

/// The effect class the protocol declares for an action that writes.
const WRITE_EFFECT: &str = "write";

/// The step performed: the run, the records submitted and the step as written.
fn performed(
    run: &ProgramRun,
    recorded: &[RecordedEvidence],
    step: &BoundStep,
) -> Result<EffectOutcome, EffectError> {
    Ok(EffectOutcome::Performed(EffectOutcomePerformed {
        report: commission_value(&serde_json::json!({
            "run": run_json(run),
            "evidence": recorded.iter().map(record_json).collect::<Vec<_>>(),
            "step": step_json(step),
        }))?,
        attempt: None,
        audit: None,
        usage: None,
    }))
}

fn step_json(step: &BoundStep) -> serde_json::Value {
    serde_json::json!({
        "action": step.action,
        "outcome": format!("{:?}", step.outcome),
        "discarded": step.discarded.map(|reason| format!("{reason:?}")),
        "placements": step.placements.iter().map(|placement| serde_json::json!({
            "artifact": placement.artifact,
            "path": placement.path,
            "revision": placement.revision,
        })).collect::<Vec<_>>(),
        "moves": step.moves.iter().map(|moved| serde_json::json!({
            "artifact": moved.artifact,
            "from": moved.from,
            "to": moved.to,
        })).collect::<Vec<_>>(),
    })
}

impl<S: FallibleCaseStore, T: EvaluationTime> EffectPort for ProgramEffects<'_, S, T> {
    fn performs(&self, action: &str) -> bool {
        self.binding(action).is_some()
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.perform(request.data())
    }
}

/// The value `entry` reads from `execution` (`None`: a status that could not be read), the report
/// parsed once into `report`; `Err` says why the report cannot be read.
fn value_of(
    entry: &EvidenceEntry,
    execution: &TestExecution,
    report: &mut Option<Result<serde_json::Value, String>>,
) -> Result<Option<String>, String> {
    if execution.timed_out {
        return Ok(Some(TIMEOUT_KEY.to_owned()));
    }
    match entry.from {
        ResultSource::Exit => Ok(execution.exit_code.map(|code| code.to_string())),
        ResultSource::Effect => Ok(None),
        ResultSource::Report => {
            let parsed = report.get_or_insert_with(|| {
                one_object(
                    &execution.child_output.stdout,
                    execution.child_output.stdout_truncated,
                )
                .map_err(|why| format!("the report {why}"))
            });
            let object = parsed.as_ref().map_err(Clone::clone)?;
            let pointer = entry.pointer.as_deref().unwrap_or_default();
            let value = object
                .pointer(pointer)
                .ok_or_else(|| format!("the report has no value at {pointer}"))?;
            Ok(Some(match value {
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            }))
        }
    }
}

/// The result `entry`'s table gives `value`, falling to its `*` row: `Some(None)` for a row that
/// names no result, `None` when neither row is there.
fn look_up(entry: &EvidenceEntry, value: Option<&str>) -> Option<Option<String>> {
    let row = |key: &str| entry.results.iter().find(|rule| rule.value == key);
    value
        .and_then(row)
        .or_else(|| row(ANY_KEY))
        .map(|rule| rule.result.clone())
}

/// `bytes` read as one JSON object of at most [`MAX_REPORT_BYTES`], nested at most 64 deep and
/// naming no member of an object twice; `truncated` says the runner kept only part of them. `Err`
/// says why not, as a predicate.
fn one_object(bytes: &[u8], truncated: bool) -> Result<serde_json::Value, String> {
    if truncated || bytes.len() > MAX_REPORT_BYTES {
        return Err(format!("is longer than {MAX_REPORT_BYTES} bytes"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "is not UTF-8".to_owned())?;
    // Commission's reader keeps every member as written and bounds the nesting, so a member named
    // twice, which a pointer could read either way, is found.
    let read =
        commission_json::parse(text).map_err(|error| format!("is not one JSON value: {error}"))?;
    if !matches!(read, commission_json::Value::Object(_)) {
        return Err("is not a JSON object".to_owned());
    }
    if let Some(name) = named_twice(&read) {
        return Err(format!("names the member {name:?} twice in one object"));
    }
    serde_json::from_str(text).map_err(|error| format!("is not one JSON value: {error}"))
}

/// The first member name an object of `value` holds twice.
fn named_twice(value: &commission_json::Value) -> Option<String> {
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            commission_json::Value::Array(items) => pending.extend(items),
            commission_json::Value::Object(members) => {
                let mut seen = std::collections::BTreeSet::new();
                for (name, member) in members {
                    if !seen.insert(name.as_str()) {
                        return Some(name.clone());
                    }
                    pending.push(member);
                }
            }
            _ => {}
        }
    }
    None
}

/// Keeps `bytes` as `<store>/blobs/<digest>`, written to a temporary file in that directory and
/// renamed. A regular file already there that holds the same bytes is kept; anything else under
/// that name, a link included, is replaced by the rename and never written through. A `blobs`
/// that is not a directory, a link included, is refused, so nothing is written outside the store
/// directory.
fn keep_blob(store: &Path, digest: &str, bytes: &[u8]) -> std::io::Result<()> {
    let blobs = store.join(BLOBS);
    match std::fs::symlink_metadata(&blobs) {
        Ok(found) if found.file_type().is_dir() => {}
        Ok(_) => {
            return Err(std::io::Error::other(format!(
                "{} is not a directory; a link is not followed",
                blobs.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(store)?;
            match std::fs::create_dir(&blobs) {
                Ok(()) => {}
                // Made meanwhile by another port over the same store: a directory still.
                Err(error)
                    if error.kind() == std::io::ErrorKind::AlreadyExists
                        && std::fs::symlink_metadata(&blobs)?.file_type().is_dir() => {}
                Err(error) => return Err(error),
            }
        }
        Err(error) => return Err(error),
    }
    let path = blobs.join(digest);
    if let Ok(found) = std::fs::symlink_metadata(&path)
        && found.file_type().is_file()
        && std::fs::read(&path)? == bytes
    {
        return Ok(());
    }
    let mut file = tempfile::Builder::new()
        .prefix(".partial-")
        .tempfile_in(&blobs)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(&path).map_err(|error| error.error)?;
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// A refusal of the step that is none of [`ProgramRefusalReason`]'s.
fn refused(reason: String) -> EffectOutcome {
    EffectOutcome::Refused(EffectOutcomeRefused {
        reason,
        ends_run: false,
    })
}

/// The step refused `reason`: `<reason>: action <action>: <detail>`.
fn refusal(reason: ProgramRefusalReason, action: &str, detail: impl Into<String>) -> EffectOutcome {
    let refusal = ProgramRefusal {
        reason,
        action: action.to_owned(),
        detail: detail.into(),
    };
    refused(format!(
        "{:?}: action {}: {}",
        refusal.reason, refusal.action, refusal.detail
    ))
}

fn governor(error: impl std::fmt::Debug) -> EffectError {
    EffectError::new(format!("the governor: {error:?}"))
}

fn run_json(run: &ProgramRun) -> serde_json::Value {
    serde_json::json!({
        "case": run.case,
        "action": run.action,
        "exit": run.exit,
        "blob_sha256": run.blob_sha256,
        "timed_out": run.timed_out,
        "runner_refusal": run.runner_refusal,
    })
}

fn record_json(record: &RecordedEvidence) -> serde_json::Value {
    serde_json::json!({
        "kind": record.kind,
        "subject": record.subject,
        "producer": record.producer,
        "result": record.result,
        "subject_revision": record.subject_revision,
        "observed_at": record.observed_at.0,
        "upstream_revisions": record.upstream_revisions.iter().map(|upstream| serde_json::json!({
            "artifact": upstream.artifact,
            "revision": upstream.revision,
        })).collect::<Vec<_>>(),
    })
}

/// The text member `name` of the object `value`.
fn text_member<'v>(value: &'v commission_json::Value, name: &str) -> Option<&'v str> {
    match value.member(name) {
        Some(commission_json::Value::Text(text)) => Some(text),
        _ => None,
    }
}

fn commission_value(value: &serde_json::Value) -> Result<commission_json::Value, EffectError> {
    commission_json::parse(&value.to_string())
        .map_err(|error| EffectError::new(format!("a JSON value the port built: {error:?}")))
}

/// Writes a generated JSON value as JSON text, each number in the spelling it arrived in.
macro_rules! json_writer {
    ($name:ident, $json:path) => {
        fn $name(value: &$json) -> String {
            use $json as Value;
            fn push(out: &mut String, value: &Value) {
                match value {
                    Value::Null => out.push_str("null"),
                    Value::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
                    Value::Number(spelling) => out.push_str(spelling),
                    Value::Text(text) => push_text(out, text),
                    Value::Array(items) => {
                        out.push('[');
                        for (index, item) in items.iter().enumerate() {
                            if index > 0 {
                                out.push(',');
                            }
                            push(out, item);
                        }
                        out.push(']');
                    }
                    Value::Object(members) => {
                        out.push('{');
                        for (index, (name, member)) in members.iter().enumerate() {
                            if index > 0 {
                                out.push(',');
                            }
                            push_text(out, name);
                            out.push(':');
                            push(out, member);
                        }
                        out.push('}');
                    }
                }
            }
            let mut out = String::new();
            push(&mut out, value);
            out
        }
    };
}

fn push_text(out: &mut String, text: &str) {
    out.push_str(&serde_json::Value::String(text.to_owned()).to_string());
}

json_writer!(commission_text, commission_json::Value);
json_writer!(loom_text, loom::json::Value);

/// A fresh id (version 8, from a digest of the process, the instant and a counter) for a record of
/// `kind` in `case`.
fn fresh_uuid(kind: &str, case: &str) -> Uuid {
    static ISSUED: AtomicU64 = AtomicU64::new(0);
    let issued = ISSUED.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let digest = Sha256::digest(
        format!("{kind}\n{case}\n{}\n{nanos}\n{issued}", std::process::id()).as_bytes(),
    );
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = bytes.iter().fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    });
    Uuid(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_keeps_its_spelling_and_a_text_is_escaped() {
        let value = commission_json::parse(r#"{"a":10.50,"b":"x\"y","c":[null,true]}"#).unwrap();
        assert_eq!(
            commission_text(&value),
            r#"{"a":10.50,"b":"x\"y","c":[null,true]}"#
        );
    }

    #[test]
    fn a_fresh_uuid_is_version_8_shaped_and_fresh() {
        let id = fresh_uuid("k", "case-1").0;
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "8");
        assert_ne!(id, fresh_uuid("k", "case-1").0);
    }
}
