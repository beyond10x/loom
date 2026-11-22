//! A writer's `handoff_when` check in a tree run (story:handoff-when-a-program-decides).
//!
//! When the deterministic selector picks an action whose program binding carries `handoff_when`,
//! the check runs first ([`b10x_loom_bindings::run_handoff_check`]): [`Checked`] wraps the step's
//! executor, runs the check once the executor proposed the action and before the loop performs
//! it, and writes the case's `handoff checked` line before the step or the stop. Exit 0 takes no
//! step: the run stops `AwaitingPerson` with the action's `Handoff` decision. Any other exit
//! hands the proposal on, and the writer runs in that step. A check that gave no exit status
//! (a launch error, a timeout, a signal) takes no step and refuses the run `HandoffCheckFailed`,
//! naming the action. At a stuck stop a binding with `handoff_when` offers its handoff only when
//! its check, run the same way, exits 0. The line is the record of the check's answer: no reader
//! of the log runs the check again.

use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

use b10x_loom_bindings::{HandoffCheck, handoff_check_of, run_handoff_check};
use b10x_loom_commission::model::responsibility::{
    CaseId, Commission, ExecutorOutcome, ExecutorStep, Frontier, Unit, commission_state,
    frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_intake_slice::confinement::TestRunner;
use loom::bindings::BindingsDocument;
use loom::runlog::{HandoffChecked, RunLogEvent, RunLogLine};
use loom::tree::TreeRefusalReason;
use loom_governor::CanonGovernor;

use crate::Refused;
use crate::log::Logs;

/// The `handoff_when` checks of one case: what each needs to run, and the log its lines go to.
pub(crate) struct HandoffChecks<'a, S> {
    pub(crate) governor: &'a CanonGovernor<S>,
    pub(crate) logs: &'a Logs,
    pub(crate) case: &'a CaseId,
    pub(crate) bindings: &'a BindingsDocument,
    pub(crate) workspace: &'a Path,
    pub(crate) store: &'a Path,
    pub(crate) runner: &'a Arc<dyn TestRunner>,
}

impl<S: loom_governor::FallibleCaseStore> HandoffChecks<'_, S> {
    /// Whether `action`'s binding carries a `handoff_when` check.
    pub(crate) fn has_check(&self, action: &str) -> bool {
        handoff_check_of(self.bindings, action).is_some()
    }

    /// Runs the check of `action` once and writes its `handoff checked` line: the check's answer
    /// and the line, `None` when the binding carries no check.
    ///
    /// # Errors
    /// The line could not be written (`LogRefused`, `StoreUnreadable`).
    pub(crate) fn check(
        &self,
        action: &str,
    ) -> Result<Option<(HandoffCheck, RunLogLine)>, Refused> {
        let Some(binding) = handoff_check_of(self.bindings, action) else {
            return Ok(None);
        };
        let Some(check) = run_handoff_check(
            self.governor,
            self.case,
            self.bindings,
            binding,
            self.workspace,
            self.store,
            self.runner,
        ) else {
            return Ok(None);
        };
        let line = self.logs.append(
            &self.case.0,
            RunLogEvent::HandoffChecked(HandoffChecked {
                action: action.to_owned(),
                exit: check.exit,
                refusal: check.refusal.clone(),
                blob_digest: check.blob_digest.clone(),
            }),
        )?;
        Ok(Some((check, line)))
    }

    /// The refusal of a run whose check of `action` gave no exit status.
    pub(crate) fn failed(&self, action: &str, check: &HandoffCheck) -> Refused {
        Refused::new(
            TreeRefusalReason::HandoffCheckFailed,
            check
                .refusal
                .clone()
                .unwrap_or_else(|| "the check gave no answer".to_owned()),
        )
        .case(self.case.0.clone())
        .action(action.to_owned())
    }
}

/// Why a [`Checked`] step took no step.
pub(crate) enum CheckHeld {
    /// The check of the action exited 0: its handoff is offered.
    HandedOff(String),
    /// The check gave no exit status, or its line could not be written: the run is refused.
    Refused(Refused),
}

/// The executor of one step with the `handoff_when` check of the action it proposes run before
/// the loop performs it (see the module documentation).
pub(crate) struct Checked<'a, E: ?Sized, S> {
    inner: &'a E,
    checks: &'a HandoffChecks<'a, S>,
    lines: RefCell<Vec<RunLogLine>>,
    held: RefCell<Option<CheckHeld>>,
}

impl<'a, E: AgentExecutor + ?Sized, S: loom_governor::FallibleCaseStore> Checked<'a, E, S> {
    pub(crate) fn new(inner: &'a E, checks: &'a HandoffChecks<'a, S>) -> Self {
        Self {
            inner,
            checks,
            lines: RefCell::new(Vec::new()),
            held: RefCell::new(None),
        }
    }

    /// The lines the step's checks wrote, in order, and why the step took no step, if a check
    /// held it.
    pub(crate) fn finish(self) -> (Vec<RunLogLine>, Option<CheckHeld>) {
        (self.lines.into_inner(), self.held.into_inner())
    }
}

impl<E: AgentExecutor + ?Sized, S: loom_governor::FallibleCaseStore> AgentExecutor
    for Checked<'_, E, S>
{
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.run_with_usage(commission, frontier).outcome
    }

    fn run_with_usage(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorStep {
        let step = self.inner.run_with_usage(commission, frontier);
        let action = match &step.outcome {
            ExecutorOutcome::ProposedAction(proposed)
                if self.checks.has_check(&proposed.action) =>
            {
                proposed.action.clone()
            }
            _ => return step,
        };
        let held = |held: CheckHeld| {
            *self.held.borrow_mut() = Some(held);
            ExecutorStep {
                outcome: ExecutorOutcome::NoUsefulAction(Unit(true)),
                ..step.clone()
            }
        };
        // One check per step: a step held once proposes nothing again.
        if self.held.borrow().is_some() {
            return ExecutorStep {
                outcome: ExecutorOutcome::NoUsefulAction(Unit(true)),
                ..step
            };
        }
        match self.checks.check(&action) {
            Ok(None) => step,
            Ok(Some((check, line))) => {
                self.lines.borrow_mut().push(line);
                if check.refusal.is_some() {
                    held(CheckHeld::Refused(self.checks.failed(&action, &check)))
                } else if check.hands_off() {
                    held(CheckHeld::HandedOff(action))
                } else {
                    step
                }
            }
            Err(refused) => held(CheckHeld::Refused(refused.action(action))),
        }
    }
}
