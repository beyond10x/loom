//! The program effect port: performs an admitted action bound to a `program` executor, for any
//! registered protocol, and turns its result into evidence by the binding's table alone.
//!
//! The port is built per case ([`ProgramEffects::new`]) from the governor holding the case, the
//! checked bindings document, the case's workspace root, the store directory and the host's runner,
//! which the host may not give. The runner is admitted by
//! [`b10x_loom_intake_slice::bindings::admit_bound_program_runner`] and checked with
//! [`TestRunner::preflight_child`] on the workspace root before anything starts; a refusal of
//! either is the constructor's. Every program, the `arguments_from` argv included, starts through
//! that runner's [`TestRunner::run_child`] alone, as the child [`crate::bound_case_child`] builds:
//! the argv, the arguments as JSON on standard input and never in an argv, an environment of
//! `PATH`, `LOOM_CASE` (the id of the case the port was built for), `LOOM_TIMEOUT_SECONDS` (the
//! whole seconds of the run's timeout) and, for a case a parent opened, `LOOM_CASE_NAME` and
//! `LOOM_PARENT_CASE_NAME` as the governor stored them, alone, the binding's timeout, the
//! workspace root as its start directory. A case id longer than
//! [`crate::MAX_CASE_VARIABLE_BYTES`] refuses the step and nothing starts. A document whose
//! `program_root` is `bindings` runs only once [`crate::resolve_programs`] resolved it: a program
//! it still names beside its bindings file refuses the step and nothing starts.
//!
//! Exit classes (`story:bound-program-contract`): a run whose exit the binding's `exits.error`
//! lists records nothing and refuses the step `ProgramError`, ending the run, its text ending with
//! the run's stderr tail (`story:program-error-carries-its-reason`: the walk of
//! [`b10x_loom_commission::stderr`], within its 4096 bytes), which the run's observation keeps as
//! `reason` and no other run's does; one whose exit
//! `exits.not_called` lists records nothing and the step is performed. Neither is counted against
//! `budget.max_runs`; a run killed at its timeout is in neither class. Each step starts the bound
//! program once, so repeated not-called runs are bounded by the loop that drives the port (its
//! step budget and idle bound), never by the port.
//!
//! A signalled run (`story:bound-program-signal-exit-is-refused`): Substrate reports a child a
//! signal ended as exit 128 plus the signal. A run whose exit is in
//! [`crate::SIGNAL_EXITS`] (129 to 192) and that was not killed at its timeout records nothing,
//! writes the moves alone and refuses the step `ProgramSignalled`, ending the run, naming the
//! exit, the signal and the blob digest. It comes before the exit classes and the tables: such a
//! run has no class, so it counts against `budget.max_runs`, and no `exits` list or table row is
//! read for it.
//!
//! A spent allowance (`story:outcome-when-reads-the-run-count`): an entry read `from: runs` takes
//! its value from the run count, not the run. On the run that brings the count to
//! `budget.max_runs` its value is [`RUNS_SPENT`] and its record is written in the step's one write
//! after the run's other records; on any other run it records nothing. A run that records nothing
//! (cancelled, or in an exit class) writes none either, so a cancelled last run spends the
//! allowance and leaves no such record; [`settle_spent`] writes it once the allowance is spent for
//! good (`story:spent-program-has-its-spent-record`), and the port still refuses the next offer.
//!
//! A fresh allowance after a rejection (`story:rework-allowance-after-a-rejection`): under
//! `budget.runs_per: rejection` the runs counted against `budget.max_runs` are those since the
//! latest rejection, and `budget.rework_limit` bounds the rejections; the step past either bound
//! is `RunsSpent`, its detail naming the bound. A `from: runs` entry then records `spent` once,
//! on the first run made at a rework count equal to `rework_limit`. The rule is
//! `Allowance`'s, one count for the refusal and the entry.
//!
//! A step, in order: the case held under the document's protocol, else `CaseOfAnotherProtocol`
//! and nothing starts; the action's run count (the observations this port kept for the action in
//! the case, [`PROGRAM_SOURCE`]) below its `budget.max_runs`, else `RunsSpent` and nothing starts; the
//! action's effect class and the placed artifacts' digests, read before the program starts; the
//! arguments (the request's, else the
//! binding's literal `arguments`, else the object the `arguments_from` argv prints, else `{}`; a
//! request has none when its arguments are the empty object), an `arguments_from` that does not
//! exit 0 or prints anything but one JSON object within `MAX_REPORT_BYTES` refused
//! `ArgumentsUnreadable` before the bound program starts; the run; the blob of its standard output
//! followed by its standard error, kept as `blobs/<sha256>` in the store directory through a
//! temporary file and a rename; one observation of the run (`loom.bindings.ProgramRun`), kept for
//! every call of `run_child` whatever it answered and whether the blob was kept or not, before a
//! runner refusal or a blob failure ends the step; then, for
//! each `evidence` entry, the value its `from` names (the exit code, `timeout` for a run killed at
//! its timeout, or the JSON pointer into the report, one JSON object within `MAX_REPORT_BYTES`)
//! looked up in its `results`, falling to `*`, but for a run killed at its timeout only the
//! `timeout` row matches (`story:wildcard-result-row-does-not-match-timeout`; a `from: runs` entry
//! is not affected). A value no row matches is `ResultNotInTable`, a
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
//!
//! Cancel (`story:cancel-a-running-bound-program`): a port built [`ProgramEffects::with_cancel`]
//! reads its handle after the run count, refusing the step `Cancelled` with nothing started, and
//! gives it to every program the step starts; a program its cancel ended refuses the step
//! `Cancelled`, ending the run, with its run observed (`cancelled: true`) and nothing recorded.
//!
//! One start path (`story:person-action-runs-a-program`): a step's refusals before a start and
//! its child are [`prepare`]'s, the run and its observation [`start`]'s; the port calls both, and
//! a person's approval (`b10x-loom-tree`) calls them with the principal that replaces each argv
//! entry that is exactly [`crate::PRINCIPAL_PLACEHOLDER`], `{}` on standard input and no retry
//! feedback ([`ChildInputs::empty`]).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
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
use b10x_loom_commission::stderr::stderr_tail;
use b10x_loom_intake_slice::bindings::{DEFAULT_TIMEOUT_SECONDS, admit_bound_program_runner};
use b10x_loom_intake_slice::clock::{SystemClock, instant};
use b10x_loom_intake_slice::confinement::{
    Cancel, ChildCommand, ConfinementError, ConfinementRefusal, TestExecution, TestRunner,
};
use loom::bindings::{
    ArtifactPlacement, ArtifactRevision, BindingsDocument, BoundAction, BoundStep, DiscardReason,
    EvidenceEntry, Executor, ExitClass, ExitClasses, ProgramRefusal, ProgramRefusalReason,
    ProgramRoot, ProgramRun, RecordedEvidence, ResultSource, RunsScope, StepOutcome,
};
use loom_governor::{
    CanonGovernor, EvaluationTime, FallibleCaseStore, NoEvaluationTime, StepWritten,
};

use crate::revision::{describe, placements, revisions_of};
use sha2::{Digest as _, Sha256};

use crate::arguments::{NotPrinted, case_names, one_object, run_arguments_from};
use crate::handoff::SIGNAL_EXITS;
use crate::resolve::beside;
use crate::retry::{PERSON_SOURCE, lists, only_text};
use crate::{CaseNames, bound_case_child};

/// The `source` of every observation the port keeps: one per run of a bound program. The run count
/// of an action in a case is the number of these naming it and the case and holding no `class`.
pub const PROGRAM_SOURCE: &str = "loom/bound-program";
/// The directory of the store directory that holds the blobs, each named by its SHA-256.
pub const BLOBS: &str = "blobs";
/// The key of a result table a run killed at its timeout reads.
pub const TIMEOUT_KEY: &str = "timeout";
/// The key of a result table any value it does not list reads.
pub const ANY_KEY: &str = "*";
/// The one value a `from: runs` entry reads, and the one key its table lists: the run that brings
/// the action's run count to its `budget.max_runs` (`story:outcome-when-reads-the-run-count`).
pub const RUNS_SPENT: &str = "spent";

/// The program effect port of one case (see the module documentation).
pub struct ProgramEffects<'a, S, T = NoEvaluationTime> {
    governor: &'a CanonGovernor<S, T>,
    case: CaseId,
    document: &'a BindingsDocument,
    workspace: PathBuf,
    store: PathBuf,
    runner: Arc<dyn TestRunner>,
    /// The handle every program a step starts holds ([`ProgramEffects::with_cancel`]).
    cancel: Option<Cancel>,
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
            cancel: None,
        })
    }

    /// This port with `cancel`: every program a step starts, the `arguments_from` program
    /// included, holds it ([`b10x_loom_intake_slice::confinement::ChildCommand::with_cancel`]).
    /// Read after the run count, a set handle refuses the step `Cancelled`, ending the run, with
    /// nothing started and no observation kept. A handle set in the window
    /// after the port read it and before the runner starts the program starts nothing either,
    /// but the runner refuses that run `Cancelled`, so it is kept and counted as the cancelled
    /// run below. Set while a program runs, the runner kills it, under `linux` with every
    /// process it started and under `local` with its process group only
    /// (a process that left the group survives), and the step is refused `Cancelled`, ending the
    /// run: the run's observation is kept (`cancelled: true`, `runner_refusal` naming
    /// `Cancelled`) and counts against `budget.max_runs`, the placed artifacts' moves are written
    /// alone, and no evidence record, result table or blob is written. A handle set after the
    /// program ended changes nothing.
    #[must_use]
    pub fn with_cancel(self, cancel: Cancel) -> Self {
        Self {
            cancel: Some(cancel),
            ..self
        }
    }

    /// The binding of `action` when it is bound to a `program`.
    fn binding(&self, action: &str) -> Option<&BoundAction> {
        self.document
            .actions
            .iter()
            .find(|bound| bound.action == action && bound.executor.kind == Executor::Program)
    }

    /// Where this port's programs start.
    fn place(&self) -> RunPlace<'_, S, T> {
        RunPlace {
            governor: self.governor,
            case: &self.case,
            document: self.document,
            workspace: &self.workspace,
            store: &self.store,
            runner: self.runner.as_ref(),
            cancel: self.cancel.as_ref(),
        }
    }

    /// The child's inputs of `binding`'s step for `request`: the retry feedback, kept under the
    /// store, then the arguments ([`ProgramEffects::arguments`]).
    fn inputs(
        &self,
        binding: &BoundAction,
        request: &ActionRequestData,
        timeout: Duration,
        names: &CaseNames,
    ) -> Result<ChildInputs, NotPrepared> {
        // Before the arguments: the records that send the action back, kept as one document, or
        // an error with nothing started and nothing observed.
        let feedback = crate::retry::feedback(self.governor, &self.case, binding, &self.store)
            .map_err(NotPrepared::Failed)?;
        match self.arguments(binding, request, timeout, names) {
            Ok(arguments) => Ok(ChildInputs {
                arguments,
                feedback,
                ended: None,
            }),
            Err(NoArguments::Refused(outcome)) => Err(NotPrepared::Refused(outcome)),
            // The `arguments_from` program its cancel ended: a run like the bound program's,
            // kept and counted, and the bound program never starts.
            Err(NoArguments::Cancelled(refusal)) => Ok(ChildInputs {
                arguments: String::new(),
                feedback: None,
                ended: Some((
                    refusal,
                    binding
                        .arguments_from
                        .as_deref()
                        .and_then(<[String]>::first)
                        .map_or("the arguments_from program", String::as_str)
                        .to_owned(),
                )),
            }),
        }
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
        let place = self.place();
        let prepared = match prepare(&place, binding, None, |timeout, names| {
            self.inputs(binding, request, timeout, names)
        }) {
            Ok(prepared) => prepared,
            Err(NotPrepared::Refused(outcome)) => return Ok(outcome),
            Err(NotPrepared::Failed(error)) => return Err(error),
        };
        let started = start(&place, prepared)?;
        let changed = started.changed();
        let Started {
            run,
            answer,
            observation,
            observed_at,
            blob_failure,
            after,
            writes,
            program,
            ran,
            ..
        } = started;
        let program = program.as_str();
        // What the workspace holds once the program ended: the revisions this step writes,
        // whatever else it records, and never a value the program, its report or the request names.
        let after = match after {
            Ok(after) => after,
            Err(unreadable) => {
                return Ok(refusal(
                    ProgramRefusalReason::ArtifactUnreadable,
                    action,
                    format!("no revision moved and nothing was recorded: {unreadable}"),
                ));
            }
        };
        let targets = revisions_of(&after);
        let execution = match answer {
            Ok(execution) => execution,
            Err(refusal) if run.cancelled => {
                // The moves alone: a cancelled run reads no table and records nothing.
                self.write(&targets, Vec::new())?;
                return Ok(cancelled_step(
                    action,
                    format!("{ran} was ended by its cancel: {}", refusal.detail),
                ));
            }
            Err(refusal) => {
                self.write(&targets, Vec::new())?;
                return Ok(refused(format!(
                    "{ran} was refused by its runner: {refusal}"
                )));
            }
        };
        if let Some(failure) = blob_failure {
            self.write(&targets, Vec::new())?;
            return Err(EffectError::new(failure));
        }
        // A run a signal ended, as Substrate reports it, before the exit classes and the tables:
        // it gave no answer, records nothing, counts (it holds no class) and ends the run.
        if signalled(&execution) {
            self.write(&targets, Vec::new())?;
            return Ok(program_signalled(action, program, &run));
        }
        // A classed exit never reaches the tables: it records nothing, its run is not counted,
        // and a program error ends the run. The revisions still take what the workspace holds.
        match run.class {
            Some(ExitClass::Error) => {
                self.write(&targets, Vec::new())?;
                return Ok(program_error(
                    action,
                    program,
                    &run,
                    execution.child_output.stderr_truncated,
                ));
            }
            Some(ExitClass::NotCalled) => {
                let written = self.write(&targets, Vec::new())?;
                let discarded = (changed && !writes).then_some(DiscardReason::ChangedByNonWriter);
                return performed(
                    &run,
                    &[],
                    &BoundStep {
                        action: action.to_owned(),
                        outcome: if discarded.is_some() {
                            StepOutcome::Discarded
                        } else {
                            StepOutcome::Recorded
                        },
                        discarded,
                        placements: after,
                        moves: written.moves,
                    },
                );
            }
            None => {}
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
        // This run is counted now (its observation is kept and it has no class): it spent the
        // allowance when the count reached `budget.max_runs`, or, under `runs_per: rejection`,
        // when it is the first run at a rework count equal to `rework_limit`.
        let counted = |entry: &&EvidenceEntry| entry.from == ResultSource::Runs;
        let spent = binding.evidence.iter().any(|entry| counted(&entry))
            && allowance(self.governor, &self.case, binding)?.spent_by_the_last_run(binding);
        let mut report = None;
        let mut recorded = Vec::new();
        // The `from: runs` entries after the others, so their records follow the run's other
        // records in the step's one write.
        let entries = binding
            .evidence
            .iter()
            .filter(|entry| !counted(entry))
            .chain(binding.evidence.iter().filter(counted));
        for entry in entries {
            let value = if entry.from == ResultSource::Runs {
                if !spent {
                    continue;
                }
                Some(RUNS_SPENT.to_owned())
            } else {
                match value_of(entry, &execution, &mut report) {
                    Ok(value) => value,
                    Err(detail) => {
                        self.write(&targets, Vec::new())?;
                        return Ok(refusal(
                            ProgramRefusalReason::ReportUnreadable,
                            action,
                            detail,
                        ));
                    }
                }
            };
            // The run's condition, not the string's: a run killed at its timeout matches only a
            // `timeout` row. A `from: runs` entry reads the run count, never the run.
            let timed_out = execution.timed_out && entry.from != ResultSource::Runs;
            let Some(result) = look_up(entry, value.as_deref(), timed_out) else {
                self.write(&targets, Vec::new())?;
                let detail = if timed_out {
                    format!(
                        "the run was killed at its timeout_seconds ({} s), and the results of the {} entry name no {TIMEOUT_KEY}; {ANY_KEY} does not match a timeout",
                        binding.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS),
                        entry.kind
                    )
                } else {
                    format!(
                        "the value {:?} of the {} entry is not in its results, which list no {ANY_KEY}",
                        value.as_deref().unwrap_or("(none)"),
                        entry.kind
                    )
                };
                return Ok(refusal(
                    ProgramRefusalReason::ResultNotInTable,
                    action,
                    detail,
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
    /// refusal, or the `arguments_from` program's cancel, with no bound program started.
    fn arguments(
        &self,
        binding: &BoundAction,
        request: &ActionRequestData,
        timeout: Duration,
        names: &CaseNames,
    ) -> Result<String, NoArguments> {
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
        // A helper program keeps the hour, whatever the bound program may name.
        let timeout = timeout.min(Duration::from_secs(
            crate::MAX_TIMEOUT_SECONDS.unsigned_abs(),
        ));
        match run_arguments_from(
            argv,
            &commission_text(requested),
            timeout,
            &self.workspace,
            self.document.workspace,
            self.runner.as_ref(),
            &self.case,
            names,
            self.cancel.as_ref(),
        ) {
            Ok(printed) => Ok(printed.object.to_string()),
            Err(NotPrinted::NotStarted(why)) => Err(NoArguments::Refused(refused(why))),
            Err(NotPrinted::Unreadable(why)) => Err(NoArguments::Refused(refusal(
                ProgramRefusalReason::ArgumentsUnreadable,
                action,
                why,
            ))),
            Err(NotPrinted::Cancelled(cancelled)) => Err(NoArguments::Cancelled(cancelled)),
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
        attribute(&self.case, record, case_revision, observation)
    }
}

/// `record` as evidence of `case` at `case_revision`, citing `observation`, attributed to its
/// producer by Commission's `submit_evidence`, held for one store write.
fn attribute(
    case: &CaseId,
    record: &RecordedEvidence,
    case_revision: i64,
    observation: &ObservationId,
) -> Result<AttributedEvidence, EffectError> {
    let id = EvidenceId(fresh_uuid("program-evidence", &case.0));
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
            case_id: case.clone(),
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

/// Where the bound programs of one case start, which [`prepare`] and [`start`] read: the governor
/// holding the case, the case, its checked bindings document, its workspace root, the store
/// directory that keeps the blobs, the runner (admitted by
/// [`b10x_loom_intake_slice::bindings::admit_bound_program_runner`] and checked with
/// [`TestRunner::preflight_child`] on the workspace before) and the cancel handle every program
/// holds, if any.
pub struct RunPlace<'a, S, T = NoEvaluationTime> {
    /// The governor holding the case.
    pub governor: &'a CanonGovernor<S, T>,
    /// The case the programs run for.
    pub case: &'a CaseId,
    /// The case's checked bindings document.
    pub document: &'a BindingsDocument,
    /// The workspace root, every program's start directory.
    pub workspace: &'a Path,
    /// The store directory, which keeps the blobs.
    pub store: &'a Path,
    /// The admitted runner every program starts through.
    pub runner: &'a dyn TestRunner,
    /// The handle every program holds ([`ProgramEffects::with_cancel`]).
    pub cancel: Option<&'a Cancel>,
}

impl<S, T> std::fmt::Debug for RunPlace<'_, S, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunPlace")
            .field("case", &self.case)
            .field("protocol", &self.document.protocol)
            .field("workspace", &self.workspace)
            .field("store", &self.store)
            .field("runner", &self.runner.label())
            .finish_non_exhaustive()
    }
}

/// What a bound program's child reads besides its argv, which the caller of [`prepare`] answers:
/// the arguments, as JSON text on standard input, and the retry feedback.
pub struct ChildInputs {
    arguments: String,
    feedback: Option<crate::retry::Feedback>,
    /// The `arguments_from` program its cancel ended, and its name: no child is built, and that
    /// run is the step's run.
    ended: Option<(ConfinementError, String)>,
}

impl ChildInputs {
    /// `{}` on standard input and no retry feedback: what a person's program reads.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            arguments: "{}".to_owned(),
            feedback: None,
            ended: None,
        }
    }
}

impl std::fmt::Debug for ChildInputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChildInputs")
            .field("arguments", &self.arguments)
            .field("feedback", &self.feedback.is_some())
            .field("ended", &self.ended)
            .finish()
    }
}

/// Why [`prepare`] starts nothing; [`prepare`] wrote nothing.
#[derive(Debug)]
pub enum NotPrepared {
    /// The step refused: a [`ProgramRefusal`]'s text (`RunsSpent`, `ArtifactUnreadable`,
    /// `Cancelled`, `ArgumentsUnreadable`) or another (`<program> was not started: …`).
    Refused(EffectOutcome),
    /// A read of the governor, the clock or the store failed.
    Failed(EffectError),
}

/// A bound program ready to start once through [`start`]: its child, built after every refusal
/// that comes before a start.
#[derive(Debug)]
pub struct Prepared {
    action: String,
    program: String,
    ran: String,
    child: Result<ChildCommand, ConfinementError>,
    before: Vec<ArtifactPlacement>,
    writes: bool,
    retry_of: Vec<String>,
    started_at: Timestamp,
    principal: Option<String>,
    exits: Option<ExitClasses>,
}

/// The one run [`start`] made: the run as its observation holds it and what the step reads next.
#[derive(Debug)]
pub struct Started {
    /// The run, as observed.
    pub run: ProgramRun,
    /// What the runner answered.
    pub answer: Result<TestExecution, ConfinementError>,
    /// The id of the run's observation.
    pub observation: ObservationId,
    /// The instant of the run's observation.
    pub observed_at: Timestamp,
    /// Why the blob of the run could not be kept, if it could not.
    pub blob_failure: Option<String>,
    /// The placements digested before the start.
    pub before: Vec<ArtifactPlacement>,
    /// The placements digested after the run, or why one could not be digested.
    pub after: Result<Vec<ArtifactPlacement>, String>,
    /// Whether the protocol declares the action's effect class `write`.
    pub writes: bool,
    /// The bound program, `argv[0]`.
    pub program: String,
    /// The program that ran: the bound program, or the `arguments_from` program its cancel ended.
    pub ran: String,
}

impl Started {
    /// Whether a placed artifact's digest after the run differs from before it; `false` when one
    /// could not be digested after it.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.after.as_ref().is_ok_and(|after| {
            self.before
                .iter()
                .zip(after)
                .any(|(before, after)| before.revision != after.revision)
        })
    }

    /// The run as its observation and the report of a performed step name it.
    #[must_use]
    pub fn run_json(&self) -> serde_json::Value {
        run_json(&self.run)
    }
}

/// Every refusal of `binding`'s step that comes before a start, then the child, in this order
/// (`story:person-action-runs-a-program`): under `runs_per: rejection` the rework count within
/// `budget.rework_limit`, then the run count below `budget.max_runs` (`RunsSpent`, naming the
/// bound; see `Allowance`), the cancel handle unset (`Cancelled`), an argv, the program resolved under `program_root:
/// bindings`, a timeout this host's clock holds, the placed artifacts digested
/// (`ArtifactUnreadable`), the case names, the child's inputs from `inputs` (given the timeout and
/// the names), each argv entry that is exactly [`crate::PRINCIPAL_PLACEHOLDER`] replaced by
/// `principal` as one entry, and the child built ([`bound_case_child`]) with the retry feedback
/// put on it. It starts nothing and writes nothing; what `inputs` does is the caller's.
///
/// # Errors
/// [`NotPrepared`]: the step's refusal, or a read that failed.
pub fn prepare<S: FallibleCaseStore, T: EvaluationTime>(
    place: &RunPlace<'_, S, T>,
    binding: &BoundAction,
    principal: Option<&str>,
    inputs: impl FnOnce(Duration, &CaseNames) -> Result<ChildInputs, NotPrepared>,
) -> Result<Prepared, NotPrepared> {
    let action = binding.action.as_str();
    let allowance = allowance(place.governor, place.case, binding).map_err(NotPrepared::Failed)?;
    let limit = rework_limit(binding);
    if allowance.rework() > limit
        && let Some((evidence, producer)) = &allowance.latest
    {
        return Err(NotPrepared::Refused(refusal(
            ProgramRefusalReason::RunsSpent,
            action,
            format!(
                "{action} was rejected {} times in case {}, the latest {evidence} by {producer}; its budget.rework_limit is {limit}",
                allowance.rework(),
                place.case.0,
            ),
        )));
    }
    if allowance.window >= binding.budget.max_runs {
        let since = allowance
            .opened_by
            .as_ref()
            .map_or_else(String::new, |(evidence, producer)| {
                format!(" since the rejection {evidence} by {producer}")
            });
        return Err(NotPrepared::Refused(refusal(
            ProgramRefusalReason::RunsSpent,
            action,
            format!(
                "{action} has run {} times in case {}{since}, its budget.max_runs is {}",
                allowance.window, place.case.0, binding.budget.max_runs
            ),
        )));
    }
    // After the run count and before any program starts: a cancel set now starts nothing and
    // keeps no observation.
    if place.cancel.is_some_and(Cancel::is_cancelled) {
        return Err(NotPrepared::Refused(cancelled_step(
            action,
            "the step's cancel was set before it started; nothing was started",
        )));
    }
    let Some(argv) = binding.executor.argv.as_deref() else {
        return Err(NotPrepared::Refused(refused(format!(
            "{action} is bound to no argv"
        ))));
    };
    let program = argv.first().map_or("the program", String::as_str);
    let not_started =
        |why: &str| NotPrepared::Refused(refused(format!("{program} was not started: {why}")));
    if let Some(member) = unresolved(place.document, binding) {
        return Err(not_started(&format!(
            "{member} names a file beside the bindings file, and the document was not resolved with resolve_programs"
        )));
    }
    let seconds = binding.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS);
    let timeout = Duration::from_secs(u64::try_from(seconds).unwrap_or(1));
    // Taken before anything starts, so a timeout the clock cannot hold starts nothing.
    if Instant::now().checked_add(timeout).is_none() {
        return Err(not_started(&format!(
            "timeout_seconds {seconds} is past what this host's clock can hold"
        )));
    }
    let writes = writes(place.governor, place.case, action).map_err(NotPrepared::Failed)?;
    // Read before anything starts, so a placed path that cannot be digested starts nothing,
    // and what the program changed is told from what the workspace held already.
    let before = placements(place.workspace, place.document).map_err(|unreadable| {
        NotPrepared::Refused(refusal(
            ProgramRefusalReason::ArtifactUnreadable,
            action,
            format!("nothing was started: {}", describe(&unreadable)),
        ))
    })?;
    // Read before anything starts and before anything is written: a name the governor holds
    // that is not a child name, or a parent that cannot be read, starts and writes nothing.
    let names = case_names(place.governor, place.case)
        .map_err(NotPrepared::Failed)?
        .map_err(|why| not_started(&why))?;
    // Read before any program starts, so the run's observation has an instant even when the
    // clock cannot be read after it.
    let started_at =
        instant(&SystemClock).map_err(|why| NotPrepared::Failed(EffectError::new(why)))?;
    let inputs = inputs(timeout, &names)?;
    let (child, ran, retry_of) = if let Some((refusal, ran)) = inputs.ended {
        // The `arguments_from` program its cancel ended: that run is the step's, and the bound
        // program never starts.
        (Err(refusal), ran, Vec::new())
    } else {
        let argv = with_principal(argv, principal).map_err(|why| not_started(&why))?;
        let child = bound_case_child(
            &argv,
            &inputs.arguments,
            timeout,
            place.workspace,
            place.runner.local_reason(),
            place.case,
            &names,
        )
        .map_err(|why| not_started(&why))?;
        let child = crate::child::in_document_workspace(child, place.document.workspace);
        let child =
            crate::retry::given(child, inputs.feedback.as_ref(), place.runner.local_reason());
        let child = match place.cancel {
            Some(cancel) => child.with_cancel(cancel.clone()),
            None => child,
        };
        let retry_of = inputs
            .feedback
            .map(|given| given.retry_of)
            .unwrap_or_default();
        (Ok(child), program.to_owned(), retry_of)
    };
    Ok(Prepared {
        action: action.to_owned(),
        program: program.to_owned(),
        ran,
        child,
        before,
        writes,
        retry_of,
        started_at,
        principal: principal.map(str::to_owned),
        exits: binding.exits.clone(),
    })
}

/// `argv` with each entry that is exactly [`crate::PRINCIPAL_PLACEHOLDER`] replaced by
/// `principal`, as one entry; nothing else is replaced. The decoder keeps the placeholder out of
/// `argv[0]` and out of every argv but a person's.
///
/// # Errors
/// The argv names the placeholder and no principal is given.
fn with_principal(argv: &[String], principal: Option<&str>) -> Result<Vec<String>, String> {
    argv.iter()
        .map(|entry| {
            if entry != crate::PRINCIPAL_PLACEHOLDER {
                return Ok(entry.clone());
            }
            principal.map(str::to_owned).ok_or_else(|| {
                format!(
                    "the argv names {} and no principal approved the run",
                    crate::PRINCIPAL_PLACEHOLDER
                )
            })
        })
        .collect()
}

/// Runs `prepared`'s child once through `place`'s runner, keeps its blob (its standard output
/// followed by its standard error) under the store directory, keeps one [`ProgramRun`]
/// observation whatever the runner answered and whether the blob was kept or not, with the
/// principal that approved the run when one did, then digests the placed artifacts again. It
/// writes no revision and no record: what the step writes next is the caller's.
///
/// # Errors
/// The run's observation could not be kept.
pub fn start<S: FallibleCaseStore, T: EvaluationTime>(
    place: &RunPlace<'_, S, T>,
    prepared: Prepared,
) -> Result<Started, EffectError> {
    let Prepared {
        action,
        program,
        ran,
        child,
        before,
        writes,
        retry_of,
        started_at,
        principal,
        exits,
    } = prepared;
    // The one start of a bound program: through the admitted runner, never here.
    let answer = match child {
        Ok(child) => place.runner.run_child(&child),
        Err(refusal) => Err(refusal),
    };
    // Every call above is a run: its observation is kept whatever the runner answered and
    // whether the blob can be kept or not, so it counts against `budget.max_runs`.
    let mut run = ProgramRun {
        case: place.case.0.clone(),
        action: action.clone(),
        exit: None,
        blob_sha256: None,
        timed_out: false,
        runner_refusal: None,
        retry_of,
        class: None,
        cancelled: false,
        principal,
        reason: None,
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
            run.class = class_of(exits.as_ref(), execution);
            // A program error's run keeps why the program said it failed: its stderr tail, walked
            // as every stderr Loom shows is. No other run keeps one.
            if run.class == Some(ExitClass::Error) {
                run.reason = error_reason(&output.stderr, output.stderr_truncated);
            }
            match keep_blob(place.store, &digest, &blob) {
                Ok(()) => run.blob_sha256 = Some(digest),
                Err(error) => blob_failure = Some(format!("the blob {digest}: {error}")),
            }
        }
        Err(refusal) => {
            run.runner_refusal = Some(refusal.to_string());
            run.cancelled = refusal.reason == ConfinementRefusal::Cancelled;
        }
    }
    let observed_at = instant(&SystemClock).unwrap_or(started_at);
    let observation = ObservationId(fresh_uuid("program-observation", &place.case.0));
    place
        .governor
        .observe(Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: PROGRAM_SOURCE.to_owned(),
            subject: action,
            observed_at: observed_at.clone(),
            payload: commission_value(&run_json(&run))?,
        }))
        .map_err(governor)?;
    let after =
        placements(place.workspace, place.document).map_err(|unreadable| describe(&unreadable));
    Ok(Started {
        run,
        answer,
        observation,
        observed_at,
        blob_failure,
        before,
        after,
        writes,
        program,
        ran,
    })
}

/// The rework limit of a binding under `runs_per: rejection` that names none.
const DEFAULT_REWORK_LIMIT: i64 = 2;

/// The `budget.rework_limit` of `binding`, [`DEFAULT_REWORK_LIMIT`] when it names none.
fn rework_limit(binding: &BoundAction) -> i64 {
    binding.budget.rework_limit.unwrap_or(DEFAULT_REWORK_LIMIT)
}

/// A rejection as a refusal names it: its evidence id and its producer.
type Rejection = (String, String);

/// Where the run allowance of an action stands in a case: the one count `prepare` refuses by and
/// the `from: runs` entry reads (`story:rework-allowance-after-a-rejection`).
///
/// The runs of an action in a case are the observations kept under [`PROGRAM_SOURCE`] naming
/// both, but those of a run whose exit the binding classes (`exits`), which are not counted.
/// Under `runs_per: case` (or none) the window is all of them and there is no rejection. Under
/// `rejection`, a rejection is a held record of the case whose `kind` member is its kind and whose
/// kind and result `budget.rework_on` lists, else `retry_when` ([`lists`]), at any subject
/// revision and by any producer, but a record citing a [`PROGRAM_SOURCE`] or [`PERSON_SOURCE`]
/// observation naming the case and the action (the action's own step). It is placed at the latest
/// position, in the order the governor received its observations, of an observation it cites that
/// names the case. The window is the runs after the latest placed rejection, all of them with
/// none. The rework count is the number of intervals between the runs (before the first, between
/// two, after the last) that hold a rejection. A placed one lies in the interval its position
/// gives. One placed nowhere lies at or after the interval of the latest observation cited by it
/// or by a record the case received before it, and at or before the interval after the last run:
/// it counts with a placed rejection in that span, and those that meet none count once, at the
/// latest of their first intervals. So they count the fewest intervals they can lie in, never more
/// than they do.
struct Allowance {
    /// The runs counted against `budget.max_runs` now.
    window: i64,
    /// The runs of the whole case.
    runs: usize,
    /// The intervals that hold a rejection, each named by the number of runs before it.
    intervals: BTreeSet<usize>,
    /// The latest placed rejection: the one the window starts at.
    opened_by: Option<Rejection>,
    /// The latest rejection the case received, placed or not.
    latest: Option<Rejection>,
    /// The interval the rejections placed nowhere count in on their own, when they meet no placed
    /// one; it is one of `intervals`.
    alone: Option<usize>,
    /// The observation of the latest counted run and its instant.
    last_run: Option<(ObservationId, Timestamp)>,
}

impl Allowance {
    /// The rework count.
    fn rework(&self) -> i64 {
        i64::try_from(self.intervals.len()).unwrap_or(i64::MAX)
    }

    /// Whether `binding`'s allowance is spent for good (`story:spent-program-has-its-spent-record`):
    /// under `runs_per: case`, when the window is full; under `rejection`, when the rework count
    /// is above `rework_limit`, or the count without the interval the unplaced rejections count
    /// on their own is at least `rework_limit` and the window is full. A later rejection never
    /// lowers the count, and a full window at a count an unplaced rejection carried to the limit
    /// opens again on the next placed one, so that is not spent for good.
    fn spent_for_good(&self, binding: &BoundAction) -> bool {
        let full = self.window >= binding.budget.max_runs;
        if binding.budget.runs_per != Some(RunsScope::Rejection) {
            return full;
        }
        let limit = rework_limit(binding);
        let placed = self.rework() - i64::from(self.alone.is_some());
        self.rework() > limit || (placed >= limit && full)
    }

    /// Whether the latest run spent `binding`'s allowance, read once its observation is kept:
    /// under `runs_per: case`, when the count reached `budget.max_runs`; under `rejection`, when
    /// the interval just before it holds the rejection that brought the rework count to
    /// `rework_limit`, so only the first run made at that count spends it and a full window alone
    /// never does.
    fn spent_by_the_last_run(&self, binding: &BoundAction) -> bool {
        if binding.budget.runs_per != Some(RunsScope::Rejection) {
            return self.window >= binding.budget.max_runs;
        }
        let Some(before) = self.runs.checked_sub(1) else {
            return false;
        };
        let reached = self.intervals.range(..=before).count();
        self.intervals.contains(&before)
            && i64::try_from(reached).unwrap_or(i64::MAX) == rework_limit(binding)
    }
}

/// A `spent` record [`settle_spent`] wrote: the id the case holds it under, and the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpentRecord {
    /// The record's evidence id.
    pub evidence_id: EvidenceId,
    /// The record as written.
    pub record: RecordedEvidence,
}

/// Writes the record each `from: runs` entry of `binding` owes in `case`, all in one
/// [`CanonGovernor::record_step`] with no move, and answers them; writes nothing and answers none
/// when nothing is owed (`story:spent-program-has-its-spent-record`). It never refuses.
///
/// An entry owes its record when the allowance is spent for good (`Allowance`, the count
/// [`prepare`] refuses by: under `runs_per: case` a full window; under `rejection` a rework count
/// above `rework_limit`, or a full window with the count at least `rework_limit` without the
/// interval unplaced rejections count on their own) and the case holds no record of the entry's
/// kind, subject and producer whose result is its table's for [`RUNS_SPENT`], citing a
/// [`PROGRAM_SOURCE`] observation of this case and action. The record is the one the run that
/// spent the allowance would have written: at the instant of the action's last counted run's
/// observation, citing it, about the revisions the case holds now. A case with no counted run of
/// the action owes nothing. Once written, nothing is owed, so a second call writes nothing.
/// `tree run` calls it before each evaluation; a host that drives [`ProgramEffects`] without it
/// calls it itself, since the port still refuses the next offer `RunsSpent`.
///
/// # Errors
/// A read of the governor or the store write failed; nothing was written.
pub fn settle_spent<S: FallibleCaseStore, T: EvaluationTime>(
    governor: &CanonGovernor<S, T>,
    case: &CaseId,
    binding: &BoundAction,
) -> Result<Vec<SpentRecord>, EffectError> {
    let failed = |error: GovernorError| EffectError::new(format!("the governor: {error:?}"));
    let entries: Vec<&EvidenceEntry> = binding
        .evidence
        .iter()
        .filter(|entry| entry.from == ResultSource::Runs)
        .collect();
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    let allowance = allowance(governor, case, binding)?;
    let Some((run, observed_at)) = allowance
        .last_run
        .clone()
        .filter(|_| allowance.spent_for_good(binding))
    else {
        return Ok(Vec::new());
    };
    let action = binding.action.as_str();
    let observations = governor
        .try_observations()
        .map_err(|error| EffectError::new(format!("the observations: {error:?}")))?;
    // The action's own runs in the case, counted or not: what its `spent` record cites.
    let own: BTreeSet<&str> = observations
        .iter()
        .filter(|observation| {
            observation.source == PROGRAM_SOURCE
                && text_member(&observation.payload, "case") == Some(case.0.as_str())
                && text_member(&observation.payload, "action") == Some(action)
        })
        .map(|observation| observation.observation_id.0.0.as_str())
        .collect();
    let held = governor.evidence(case).map_err(failed)?;
    let owed: Vec<(&EvidenceEntry, Option<String>)> = entries
        .into_iter()
        .filter_map(|entry| {
            let result = look_up(entry, Some(RUNS_SPENT), false)?;
            let present = held.iter().any(|record| {
                record.kind == entry.kind
                    && record.producer == entry.producer
                    && only_text(&record.facts, "subject") == Some(entry.subject.as_str())
                    && only_text(&record.facts, "result") == result.as_deref()
                    && record
                        .observation_ids
                        .iter()
                        .any(|cited| own.contains(cited.0.0.as_str()))
            });
            (!present).then_some((entry, result))
        })
        .collect();
    if owed.is_empty() {
        return Ok(Vec::new());
    }
    let revisions = governor.revisions(case).map_err(failed)?;
    let case_revision = governor.current_revision(case).map_err(failed)?;
    let revision_of = |artifact: &str| {
        revisions.get(artifact).cloned().ok_or_else(|| {
            EffectError::new(format!("case {} holds no revision for {artifact}", case.0))
        })
    };
    let mut written = Vec::new();
    let mut attributed = Vec::new();
    for (entry, result) in owed {
        let record = RecordedEvidence {
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
        };
        let evidence = attribute(case, &record, case_revision, &run)?;
        written.push(SpentRecord {
            evidence_id: evidence.evidence().data().evidence_id.clone(),
            record,
        });
        attributed.push(evidence);
    }
    governor
        .record_step(case, &BTreeMap::new(), attributed)
        .map_err(|error| {
            EffectError::new(format!(
                "the spent record of {action} in case {} was not written: {error}",
                case.0
            ))
        })?;
    Ok(written)
}

/// The [`Allowance`] of `binding` in `case`.
fn allowance<S: FallibleCaseStore, T: EvaluationTime>(
    governor: &CanonGovernor<S, T>,
    case: &CaseId,
    binding: &BoundAction,
) -> Result<Allowance, EffectError> {
    let action = binding.action.as_str();
    let observations = governor
        .try_observations()
        .map_err(|error| EffectError::new(format!("the observations: {error:?}")))?;
    let of_case =
        |payload: &commission_json::Value| text_member(payload, "case") == Some(case.0.as_str());
    let own = |source: &str, payload: &commission_json::Value| {
        source == PROGRAM_SOURCE
            && of_case(payload)
            && text_member(payload, "action") == Some(action)
    };
    // The position of each counted run in the order received.
    let positions: Vec<usize> = observations
        .iter()
        .enumerate()
        .filter(|(_, observation)| {
            own(&observation.source, &observation.payload)
                && matches!(
                    observation.payload.member("class"),
                    None | Some(commission_json::Value::Null)
                )
        })
        .map(|(position, _)| position)
        .collect();
    let runs = positions.len();
    let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    let mut allowance = Allowance {
        window: count(runs),
        runs,
        intervals: BTreeSet::new(),
        opened_by: None,
        latest: None,
        alone: None,
        last_run: positions.last().map(|&position| {
            let run = &observations[position];
            (run.observation_id.clone(), run.observed_at.clone())
        }),
    };
    if binding.budget.runs_per != Some(RunsScope::Rejection) {
        return Ok(allowance);
    }
    let listed = binding
        .budget
        .rework_on
        .as_deref()
        .unwrap_or(&binding.retry_when);
    let id = |position: usize| observations[position].observation_id.0.0.as_str();
    // Each observation by its position, and each naming the case; each of the action's own steps
    // (runs, counted or not, and person approvals).
    let received: BTreeMap<&str, usize> = (0..observations.len())
        .map(|position| (id(position), position))
        .collect();
    let placed: BTreeMap<&str, usize> = observations
        .iter()
        .enumerate()
        .filter(|(_, observation)| of_case(&observation.payload))
        .map(|(position, _)| (id(position), position))
        .collect();
    let own_steps: BTreeSet<&str> = observations
        .iter()
        .enumerate()
        .filter(|(_, observation)| {
            own(&observation.source, &observation.payload)
                || (observation.source == PERSON_SOURCE
                    && of_case(&observation.payload)
                    && text_member(&observation.payload, "action") == Some(action))
        })
        .map(|(position, _)| id(position))
        .collect();
    let held = governor
        .evidence(case)
        .map_err(|error| EffectError::new(format!("the evidence of case {}: {error:?}", case.0)))?;
    // The first interval a record placed nowhere can lie in: the latest one an observation cited
    // by it or by a record received before it lies in. One such first interval per rejection.
    let mut earliest = 0;
    let mut unplaced: Vec<usize> = Vec::new();
    let mut opened: Option<(usize, Rejection)> = None;
    for record in &held {
        let cited = || {
            record
                .observation_ids
                .iter()
                .map(|cited| cited.0.0.as_str())
        };
        if let Some(at) = cited().filter_map(|cited| received.get(cited)).max() {
            earliest = earliest.max(positions.partition_point(|position| position <= at));
        }
        if record.case_id != *case {
            continue;
        }
        if cited().any(|cited| own_steps.contains(cited)) {
            continue;
        }
        let (Some(kind), Some(result)) = (
            only_text(&record.facts, "kind"),
            only_text(&record.facts, "result"),
        ) else {
            continue;
        };
        if kind != record.kind || !lists(listed, kind, result) {
            continue;
        }
        let rejection = (record.evidence_id.0.0.clone(), record.producer.clone());
        let at = cited().filter_map(|cited| placed.get(cited).copied()).max();
        match at {
            Some(at) => {
                allowance
                    .intervals
                    .insert(positions.partition_point(|position| *position < at));
                if opened.as_ref().is_none_or(|(before, _)| at >= *before) {
                    opened = Some((at, rejection.clone()));
                }
            }
            None => unplaced.push(earliest),
        }
        allowance.latest = Some(rejection);
    }
    // A record placed nowhere lies from its first interval to the one after the last run. Those
    // whose span holds a placed rejection count with it; the rest can all lie in the latest of
    // their first intervals, and count once there.
    let alone = unplaced
        .into_iter()
        .filter(|first| allowance.intervals.range(first..).next().is_none())
        .max();
    allowance.intervals.extend(alone);
    allowance.alone = alone;
    if let Some((at, rejection)) = opened {
        allowance.window = count(positions.iter().filter(|position| **position > at).count());
        allowance.opened_by = Some(rejection);
    }
    Ok(allowance)
}

/// The member of `binding` that names a program beside the bindings file when `document`'s
/// `program_root` is `bindings`: a document [`crate::resolve_programs`] did not resolve,
/// whose program would otherwise be read as a workspace path.
fn unresolved(document: &BindingsDocument, binding: &BoundAction) -> Option<&'static str> {
    if document.program_root != Some(ProgramRoot::Bindings) {
        return None;
    }
    let names = |argv: Option<&Vec<String>>| {
        argv.and_then(|argv| argv.first())
            .is_some_and(|program| beside(program))
    };
    if names(binding.executor.argv.as_ref()) {
        Some("executor.argv[0]")
    } else if names(binding.arguments_from.as_ref()) {
        Some("arguments_from[0]")
    } else {
        None
    }
}

/// Whether the protocol of `case` declares `action`'s effect class `write`.
fn writes<S: FallibleCaseStore, T: EvaluationTime>(
    governor: &CanonGovernor<S, T>,
    case: &CaseId,
    action: &str,
) -> Result<bool, EffectError> {
    let facts = governor
        .action_facts(case)
        .map_err(|error| EffectError::new(format!("the governor: {error:?}")))?;
    Ok(facts
        .actions
        .iter()
        .find(|facts| facts.action == action)
        .and_then(|facts| facts.effect.as_deref())
        == Some(WRITE_EFFECT))
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
        // The step reads a `from: runs` entry from the run count, never from the run.
        ResultSource::Effect | ResultSource::Runs => Ok(None),
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

/// The result `entry`'s table gives `value`, falling to its `*` row unless the run was killed at
/// its timeout (`timed_out`): `Some(None)` for a row that names no result, `None` when no row
/// matches.
fn look_up(entry: &EvidenceEntry, value: Option<&str>, timed_out: bool) -> Option<Option<String>> {
    let row = |key: &str| entry.results.iter().find(|rule| rule.value == key);
    let found = value.and_then(row);
    if timed_out {
        return found.map(|rule| rule.result.clone());
    }
    found
        .or_else(|| row(ANY_KEY))
        .map(|rule| rule.result.clone())
}

/// Keeps `bytes` as `<store>/blobs/<digest>`, written to a temporary file in that directory and
/// renamed. A regular file already there that holds the same bytes is kept; anything else under
/// that name, a link included, is replaced by the rename and never written through. A `blobs`
/// that is not a directory, a link included, is refused, so nothing is written outside the store
/// directory.
pub(crate) fn keep_blob(store: &Path, digest: &str, bytes: &[u8]) -> std::io::Result<()> {
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
    loom_governor::durable::sync_file(file.as_file())?;
    file.persist(&path).map_err(|error| error.error)?;
    Ok(())
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// Why a step has no arguments for its bound program.
enum NoArguments {
    /// The step's refusal.
    Refused(EffectOutcome),
    /// The `arguments_from` program's runner refused it `Cancelled`.
    Cancelled(ConfinementError),
}

/// The step refused `Cancelled`, ending the run: `Cancelled: action <action>: <detail>`.
pub(crate) fn cancelled_step(action: &str, detail: impl Into<String>) -> EffectOutcome {
    let refusal = ProgramRefusal {
        reason: ProgramRefusalReason::Cancelled,
        action: action.to_owned(),
        detail: detail.into(),
    };
    EffectOutcome::Refused(EffectOutcomeRefused {
        reason: format!(
            "{:?}: action {}: {}",
            refusal.reason, refusal.action, refusal.detail
        ),
        ends_run: true,
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

/// The class a binding's `exits` gives the exit of `execution`: none for a run killed at its
/// timeout, whose status could not be read, a signal ended ([`signalled`]), or whose exit neither
/// list holds.
fn class_of(exits: Option<&ExitClasses>, execution: &TestExecution) -> Option<ExitClass> {
    if execution.timed_out || signalled(execution) {
        return None;
    }
    let code = i64::from(execution.exit_code?);
    let exits = exits?;
    if exits.error.contains(&code) {
        Some(ExitClass::Error)
    } else if exits.not_called.contains(&code) {
        Some(ExitClass::NotCalled)
    } else {
        None
    }
}

/// What a `ProgramError` text puts between the blob digest and the run's stderr tail.
const STDERR_SAID: &str = "; the last lines of its stderr:\n";

/// What a `ProgramError` text puts between the blob digest and the reason of a run whose stderr
/// the runner cut ([`STDERR_CUT`]).
const STDERR_KEPT_SAID: &str = "; the last kept lines of its stderr:\n";

/// What the reason of a run whose standard error the runner cut starts with: the runner keeps the
/// start of a stream up to its bound, so the lines after it are the last ones it kept, not the
/// program's last lines.
const STDERR_CUT: &str = "[it wrote more on its standard error than the runner keeps: the runner kept the start, and what follows are the last kept lines, not its last lines]";

/// The reason a program error's run keeps: its stderr tail, walked and bounded
/// ([`stderr_tail`]), none when it wrote nothing. When the runner cut the stream (`truncated`),
/// the reason says so ([`STDERR_CUT`]) and the tail is read from what it kept without the line
/// its bound cut, so no cut line is ever shown as the program's own.
fn error_reason(stderr: &[u8], truncated: bool) -> Option<String> {
    if !truncated {
        return Some(stderr_tail(&String::from_utf8_lossy(stderr))).filter(|tail| !tail.is_empty());
    }
    let whole = stderr
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(&stderr[..0], |end| &stderr[..=end]);
    let tail = stderr_tail(&String::from_utf8_lossy(whole));
    Some(if tail.is_empty() {
        STDERR_CUT.to_owned()
    } else {
        format!("{STDERR_CUT}\n{tail}")
    })
}

/// The step refused `ProgramError`, ending the run: `run`'s exit is one the binding's
/// `exits.error` lists. The text names the exit code and the blob digest and, when the program
/// wrote on its standard error, ends with the run's `reason` ([`error_reason`]): after
/// [`STDERR_SAID`], or after [`STDERR_KEPT_SAID`] when the runner cut its stderr
/// (`stderr_truncated`); never what it printed on its standard output.
fn program_error(
    action: &str,
    program: &str,
    run: &ProgramRun,
    stderr_truncated: bool,
) -> EffectOutcome {
    let mut detail = format!(
        "{program} ended exit status {}, which exits.error lists; nothing was recorded, and its output is the blob {}",
        run.exit
            .map_or_else(|| "unknown".to_owned(), |code| code.to_string()),
        run.blob_sha256.as_deref().unwrap_or("(none)")
    );
    if let Some(reason) = &run.reason {
        detail.push_str(if stderr_truncated {
            STDERR_KEPT_SAID
        } else {
            STDERR_SAID
        });
        detail.push_str(reason);
    }
    let refusal = ProgramRefusal {
        reason: ProgramRefusalReason::ProgramError,
        action: action.to_owned(),
        detail,
    };
    EffectOutcome::Refused(EffectOutcomeRefused {
        reason: format!(
            "{:?}: action {}: {}",
            refusal.reason, refusal.action, refusal.detail
        ),
        ends_run: true,
    })
}

/// Whether a signal ended the run of `execution`, as Substrate reports it: an exit in
/// [`SIGNAL_EXITS`] (128 plus the signal) of a run not killed at its timeout. A run killed at its
/// timeout reads `timeout`; under `local` a signal leaves no exit status.
fn signalled(execution: &TestExecution) -> bool {
    !execution.timed_out
        && execution
            .exit_code
            .is_some_and(|code| SIGNAL_EXITS.contains(&i64::from(code)))
}

/// The step refused `ProgramSignalled`, ending the run: a signal ended `run`. The text names the
/// program, the exit status, the signal and the blob digest, never what the program printed.
fn program_signalled(action: &str, program: &str, run: &ProgramRun) -> EffectOutcome {
    let exit = run.exit.unwrap_or_default();
    let refusal = ProgramRefusal {
        reason: ProgramRefusalReason::ProgramSignalled,
        action: action.to_owned(),
        detail: format!(
            "{program} was ended by a signal: exit {exit}, the status Substrate gives a child signal {} ended; nothing was recorded, and its output is the blob {}",
            exit - 128,
            run.blob_sha256.as_deref().unwrap_or("(none)")
        ),
    };
    EffectOutcome::Refused(EffectOutcomeRefused {
        reason: format!(
            "{:?}: action {}: {}",
            refusal.reason, refusal.action, refusal.detail
        ),
        ends_run: true,
    })
}

fn governor(error: impl std::fmt::Debug) -> EffectError {
    EffectError::new(format!("the governor: {error:?}"))
}

fn run_json(run: &ProgramRun) -> serde_json::Value {
    let mut json = serde_json::json!({
        "case": run.case,
        "action": run.action,
        "exit": run.exit,
        "blob_sha256": run.blob_sha256,
        "timed_out": run.timed_out,
        "runner_refusal": run.runner_refusal,
        "retry_of": run.retry_of,
        "class": run.class.map(|class| match class {
            ExitClass::Error => "error",
            ExitClass::NotCalled => "not_called",
        }),
        "cancelled": run.cancelled,
    });
    // Only a person's run names a principal: a program's run keeps the payload it always had.
    if let Some(principal) = &run.principal {
        json["principal"] = serde_json::Value::String(principal.clone());
    }
    // Only a program error's run names a reason: every other run keeps the payload it always had.
    if let Some(reason) = &run.reason {
        json["reason"] = serde_json::Value::String(reason.clone());
    }
    json
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
        pub(crate) fn $name(value: &$json) -> String {
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

    /// story:tree-run-tests-fit-the-budget: a kept blob is synced once, through the door of
    /// `loom_governor::durable`, counted on this thread alone; the same bytes kept again write
    /// nothing and sync nothing.
    #[test]
    fn a_kept_blob_is_synced() {
        use loom_governor::durable::{counts, counts_on_this_thread};
        let store = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/kept-blob-sync")
            .join(format!(
                "{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
        let _ = std::fs::remove_dir_all(&store);
        std::fs::create_dir_all(&store).unwrap();
        let bytes = b"standard output\n";
        let keep = || {
            let before = counts_on_this_thread();
            keep_blob(&store, &sha256(bytes), bytes).unwrap();
            let after = counts_on_this_thread();
            (after.synced - before.synced, after.skipped - before.skipped)
        };
        assert_eq!(keep(), (1, 0), "a new blob");
        assert_eq!(keep(), (0, 0), "the same blob again");
        assert_eq!(counts().skipped, 0);
        let _ = std::fs::remove_dir_all(&store);
    }

    #[test]
    fn a_fresh_uuid_is_version_8_shaped_and_fresh() {
        let id = fresh_uuid("k", "case-1").0;
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "8");
        assert_ne!(id, fresh_uuid("k", "case-1").0);
    }
}
