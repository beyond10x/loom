//! The local effect adapter: Commission's [`EffectPort`] over the slice's [`LocalExecutor`]
//! (`story:runtime-merge`, Atlas ADR 0082).
//!
//! Commission's runtime hands [`LocalEffects`] each action request it admitted; the adapter
//! performs it in the workspace with [`LocalExecutor`], lets the [`TestResultVerifier`] submit
//! evidence for a test run, prints the step, and records it in the briefing. It answers:
//!
//! - `Performed`, carrying the executor's report as text, for a performed action;
//! - `Refused`, carrying the reason, for an action the executor refuses (a path outside the
//!   workspace, an ignored path, arguments the action does not take, an action it never performs)
//!   and for a `repository.inspect` that cannot read a path it names. The model is told why;
//! - an [`EffectError`] for every other failure (the workspace, git, the governor, the verifier or
//!   the output), keeping the typed failure for the slice's caller.
//!
//! It performs `repository.inspect`, `repository.edit` and `tests.run`, and only when the case's
//! protocol is `software-change@1`: for any other protocol it performs nothing, so Commission's
//! runtime ends the run with `NoPerformableAction` before the agent is asked. There is no sandbox
//! (see [`crate::executor`]).

use std::cell::{Cell, RefCell};
use std::io::Write;

use b10x_commission::model::json;
use b10x_commission::model::responsibility::{
    ActionRequestData, CaseId, Commission, EffectOutcome, EffectOutcomePerformed,
    EffectOutcomeRefused, ExecutorOutcomeProposedAction, commission_state,
};
use b10x_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use governor::{CanonGovernor, CaseStore};

use crate::executor::{EDIT, ExecuteError, INSPECT, LocalExecutor, Report, TESTS_RUN};
use crate::run::{LOCAL_PROTOCOL, SliceError, json_text, printable};
use crate::selector::Briefing;
use crate::verifier::{TestResultVerifier, VerifyError};

/// The slice's output, shared by the agent step and the effect adapter: where lines go, the
/// number of the last step printed, and the first failure neither could return through its port.
pub(crate) struct Console<'o> {
    out: RefCell<&'o mut dyn Write>,
    steps: Cell<usize>,
    failure: RefCell<Option<SliceError>>,
}

impl<'o> Console<'o> {
    pub(crate) fn new(out: &'o mut dyn Write) -> Self {
        Self {
            out: RefCell::new(out),
            steps: Cell::new(0),
            failure: RefCell::new(None),
        }
    }

    /// Runs `write` on the output.
    pub(crate) fn write<T>(
        &self,
        write: impl FnOnce(&mut dyn Write) -> Result<T, SliceError>,
    ) -> Result<T, SliceError> {
        let mut out = self.out.borrow_mut();
        write(&mut **out)
    }

    /// The number of the next step.
    pub(crate) fn next_step(&self) -> usize {
        let step = self.steps.get() + 1;
        self.steps.set(step);
        step
    }

    /// Keeps `failure` for the caller, unless an earlier one is kept.
    pub(crate) fn fail(&self, failure: SliceError) {
        self.failure.borrow_mut().get_or_insert(failure);
    }

    /// The failure kept, if any.
    pub(crate) fn take_failure(&self) -> Option<SliceError> {
        self.failure.borrow_mut().take()
    }
}

/// The local effect adapter (see the module documentation).
pub struct LocalEffects<'a, 'o, S> {
    executor: LocalExecutor<'a, S>,
    verifier: TestResultVerifier<'a, S>,
    governor: &'a CanonGovernor<S>,
    case: CaseId,
    briefing: Briefing,
    console: &'a Console<'o>,
    local: bool,
}

impl<'a, 'o, S: CaseStore> LocalEffects<'a, 'o, S> {
    /// An adapter for `case`, opened on `protocol`, performing through `executor`.
    pub(crate) fn new(
        executor: LocalExecutor<'a, S>,
        verifier: TestResultVerifier<'a, S>,
        governor: &'a CanonGovernor<S>,
        case: CaseId,
        protocol: &str,
        briefing: Briefing,
        console: &'a Console<'o>,
    ) -> Self {
        Self {
            executor,
            verifier,
            governor,
            case,
            briefing,
            console,
            local: protocol == LOCAL_PROTOCOL,
        }
    }

    /// Performs `request`, printing the step, or refuses it.
    fn perform(&self, request: &ActionRequestData) -> Result<EffectOutcome, SliceError> {
        let proposal = ExecutorOutcomeProposedAction {
            action: request.action.clone(),
            arguments: request.arguments.clone(),
        };
        let action = proposal.action.as_str();
        let step = self.console.next_step();
        self.console.write(|out| {
            writeln!(
                out,
                "step {step}: {} {}",
                printable(action),
                printable(&json_text(&proposal.arguments.0))
            )?;
            Ok(())
        })?;
        let report = match self.executor.execute(&proposal) {
            Ok(report) => report,
            Err(
                refused @ (ExecuteError::NotExecuted { .. }
                | ExecuteError::OutsideWorkspace { .. }
                | ExecuteError::Ignored { .. }
                | ExecuteError::InvalidArguments { .. }),
            ) => return self.refuse(action, &refused.to_string()),
            // An inspect that cannot read a path (most often one that does not exist) changed
            // nothing.
            Err(refused @ ExecuteError::Workspace { .. }) if action == INSPECT => {
                return self.refuse(action, &refused.to_string());
            }
            Err(error) => return Err(SliceError::Execute(error)),
        };
        self.console.write(|out| print_effect(out, &report))?;
        let evidence = match self.verifier.verify(&report) {
            Ok(Some(evidence)) => {
                // What the governor holds, not what the report says.
                let held = self.governor.evidence(&self.case)?;
                let record = held.iter().find(|record| record.evidence_id == evidence);
                let kind = record.map_or("unknown", |record| record.kind.as_str());
                let result = match record.and_then(|record| record.facts.member("result")) {
                    Some(json::Value::Text(result)) => result.as_str(),
                    _ => "unknown",
                };
                format!("{} {}", printable(kind), printable(result))
            }
            Ok(None) => "none".to_owned(),
            Err(VerifyError::NoRevision) => format!("none ({})", VerifyError::NoRevision),
            Err(error) => return Err(SliceError::Verify(error)),
        };
        self.console.write(|out| {
            writeln!(out, "  evidence: {evidence}")?;
            Ok(())
        })?;
        self.briefing.record(&proposal, &report);
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: json::Value::Text(report.to_string()),
        }))
    }

    /// Prints a step that was not performed, for `reason`, and tells the model so.
    fn refuse(&self, action: &str, reason: &str) -> Result<EffectOutcome, SliceError> {
        self.console
            .write(|out| refuse(out, &self.briefing, action, reason))?;
        Ok(EffectOutcome::Refused(EffectOutcomeRefused {
            reason: reason.to_owned(),
        }))
    }
}

impl<S: CaseStore> EffectPort for LocalEffects<'_, '_, S> {
    fn performs(&self, action: &str) -> bool {
        self.local && [INSPECT, EDIT, TESTS_RUN].contains(&action)
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

/// The report's first line as the effect, every further line indented and marked with `|`, so no
/// line of a report can pass for a line of the run's own output.
fn print_effect(out: &mut dyn Write, report: &Report) -> Result<(), SliceError> {
    let text = report.to_string();
    let mut lines = text.lines();
    writeln!(
        out,
        "  effect: {}",
        printable(lines.next().unwrap_or_default())
    )?;
    for line in lines {
        writeln!(out, "    | {}", printable(line))?;
    }
    Ok(())
}

/// Prints a step that was not performed, for `reason`, and tells the model so.
pub(crate) fn refuse(
    out: &mut dyn Write,
    briefing: &Briefing,
    action: &str,
    reason: &str,
) -> Result<(), SliceError> {
    writeln!(out, "  effect: refused: {}", printable(reason))?;
    writeln!(out, "  evidence: none")?;
    briefing.record_refusal(action, reason);
    Ok(())
}
