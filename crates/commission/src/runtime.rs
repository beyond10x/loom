//! The local runtime loop: [`run_until_blocked`] drives one commission over its ports until its run
//! ends, and executes no effect.
//!
//! One call is one loop, and one loop starts one Run of the commission (`StartRun`) at the case
//! revision its first iteration loaded. A Run is bound to that revision: a proposal made on another
//! revision is stale (`ess/domains/responsibility.yaml`, `Run.case_revision`), so once the case is
//! at another revision the Run has no admissible action left. Each iteration makes these calls, in
//! this order:
//!
//! 1. [`Governor::current_revision`]: the case is loaded;
//! 2. [`Governor::completion`]: a case the governor holds complete ends the run completed, carrying
//!    the governor's outcome, and the iteration makes no further call. Otherwise, when the loaded
//!    revision is not the Run's, the run ends with no admissible action;
//! 3. [`Governor::frontier`]: the frontier is obtained. When it is for another revision than the
//!    Run's, the run ends with no admissible action, so every request carries the Run's revision;
//! 4. [`AgentExecutor::run`] on that frontier. The step is reported to the [`ObservationPort`] as
//!    one observation: its id and time come from the [`LoopContext`], its source is `executor`, its
//!    subject is `<case>@<frontier revision>`, and its payload names the outcome (`outcome`) and
//!    carries a proposal's `action` and `arguments` or a human request's `request`. It is never
//!    evidence;
//! 5. on `Suspended`, the Run is suspended through `SuspendRun` with the executor's reason and the
//!    run ends suspended, carrying it. The Run is suspended with that reason even when delivering
//!    the step's observation failed; the loop then returns that failure as a [`LoopError`] that
//!    carries the reason;
//! 6. on `ProposedAction`, the proposal becomes an action request bound to the frontier's revision
//!    ([`request`], its id from the context) and is revalidated ([`revalidate`]). Admitted: the
//!    request is recorded as admitted and the loop reads the frontier again. Stale: the iteration
//!    does not count toward the bound below, and the next iteration loads the case, which has
//!    moved: it ends the run completed if the case is complete, else with no admissible action.
//!    Needs authority: the provider is asked for the capability the frontier
//!    names ([`check_authority`]); on an allow the request is recorded as admitted. Otherwise, and
//!    after an authority answer other than an allow, the run outcome is derived ([`derive`]);
//! 7. on any other outcome, the run outcome is derived.
//!
//! Every request is recorded with its revalidation outcome ([`LoopEnd::requests`]); no request is
//! executed. Commission has no effect port yet.
//!
//! The loop is bounded where the frontier does not change. Every frontier the executor is given is
//! of the Run's revision, so two iterations in a row that are idle end the run with no admissible
//! action. An iteration is idle when it admits no request, or admits only a request equal to one
//! already admitted in this Run: the same action and arguments, at the Run's revision. No effect
//! executes, so admitting such a request again changes nothing. A newly admitted request starts the
//! count again. Idle covers `NoUsefulAction`, `CompletedLocalReasoning`, a refused proposal, an
//! authority answer other than an allow and a repeated admission alike; a stale proposal is not
//! counted, since the next iteration ends the run.
//!
//! A failure that leaves no run outcome is a [`LoopError`]. Once the Run is started, the error
//! names it. When the governor or its observation port fails after the Run started, the loop first
//! suspends the Run (`SuspendRun`, `ExternalAvailability`) unless it already suspended it for the
//! executor, so no Run is left `Running` that no return value identifies; when that suspension
//! fails too, the error carries both failures. The error reports the reason the loop suspended the
//! Run for ([`LoopError::suspension_reason`]): the reason is reported, not stored on the Run.
#![forbid(unsafe_code)]

use std::fmt;

use crate::action_request::{request, revalidate};
use crate::model::json::Value;
use crate::model::obligation::UnmetObligation;
use crate::model::primitives::Timestamp;
use crate::model::responsibility::obligations::{StartRunBehavior, SuspendRunBehavior};
use crate::model::responsibility::{
    ActionRequestData, ActionRequestId, AuthorityVerdict, Commission, CompletionDetermination,
    ExecutorOutcome, Frontier, GovernorError, Observation, ObservationData, ObservationId,
    RevalidateActionRequestOutcome, RunId, RunOutcome, RunOutcomeCompleted, RunOutcomeSuspended,
    RunStarted, StartRun, StartRunOutcome, SuspendRun, SuspendRunOutcome, SuspensionReason, Unit,
    commission_state, frontier_state,
};
use crate::outcome::{CapabilityVerdict, Derived, derive};
use crate::ports::authority::{AuthorityCheck, AuthorityProvider, check_authority};
use crate::ports::evidence::ObservationPort;
use crate::ports::executor::AgentExecutor;
use crate::ports::governor::Governor;

/// What the loop needs that neither the model nor the executor may supply: new ids and trusted
/// time.
pub trait LoopContext {
    /// A new id for an action request.
    fn action_request_id(&mut self) -> ActionRequestId;
    /// A new id for an observation.
    fn observation_id(&mut self) -> ObservationId;
    /// The current time, from a trusted clock.
    fn now(&mut self) -> Timestamp;
}

/// One action request the loop made and how its revalidation answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revalidated {
    /// The request, bound to the frontier it was chosen from.
    pub request: ActionRequestData,
    /// Its revalidation outcome.
    pub outcome: RevalidateActionRequestOutcome,
}

/// How a loop ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopEnd {
    /// The Run the loop started.
    pub run_id: RunId,
    /// How the run ended.
    pub outcome: RunOutcome,
    /// Every request the loop made, in order, with its revalidation outcome.
    pub requests: Vec<Revalidated>,
    /// The requests admitted, in order. None of them was executed.
    pub admitted: Vec<ActionRequestData>,
}

/// One failure that stopped a loop, or the suspension after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopFailure {
    /// The governor, or its observation port, could not answer.
    Governor(GovernorError),
    /// A run command refused with an unmet obligation.
    Obligation(UnmetObligation),
    /// `SuspendRun` did not suspend the loop's run.
    NotSuspended(Box<SuspendRunOutcome>),
}

impl fmt::Display for LoopFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Governor(error) => write!(f, "governor failed: {error:?}"),
            Self::Obligation(unmet) => write!(f, "run command failed: {unmet}"),
            Self::NotSuspended(outcome) => write!(f, "run not suspended: {outcome:?}"),
        }
    }
}

impl From<GovernorError> for LoopFailure {
    fn from(error: GovernorError) -> Self {
        Self::Governor(error)
    }
}

impl From<UnmetObligation> for LoopFailure {
    fn from(unmet: UnmetObligation) -> Self {
        Self::Obligation(unmet)
    }
}

/// Why a loop stopped without a run outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopError {
    /// The Run the loop started, or `None` when it failed before starting one.
    pub run_id: Option<RunId>,
    /// What stopped the loop.
    pub failure: LoopFailure,
    /// When the loop could not suspend the Run after a governor failure, why.
    pub suspension: Option<Box<LoopFailure>>,
    /// The reason the loop suspended the Run for, or tried to: the executor's, or
    /// `ExternalAvailability` after a governor failure. `None` when it did not try.
    pub suspension_reason: Option<Box<SuspensionReason>>,
}

impl fmt::Display for LoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.run_id {
            Some(run_id) => write!(f, "run {}: {}", run_id.0.0, self.failure)?,
            None => write!(f, "no run started: {}", self.failure)?,
        }
        if let Some(reason) = &self.suspension_reason {
            write!(f, "; suspended for {reason:?}")?;
        }
        match &self.suspension {
            Some(suspension) => write!(f, "; suspending the run failed too: {suspension}"),
            None => Ok(()),
        }
    }
}

impl std::error::Error for LoopError {}

/// A failure before the Run started.
fn unstarted(failure: impl Into<LoopFailure>) -> LoopError {
    LoopError {
        run_id: None,
        failure: failure.into(),
        suspension: None,
        suspension_reason: None,
    }
}

/// What stopped the iterations without a run outcome, and the suspension the loop already made.
struct Stopped {
    failure: LoopFailure,
    /// The reason the loop suspended the Run for, or tried to, and the suspension's failure.
    suspended: Option<(SuspensionReason, Option<LoopFailure>)>,
}

impl From<LoopFailure> for Stopped {
    fn from(failure: LoopFailure) -> Self {
        Self {
            failure,
            suspended: None,
        }
    }
}

impl From<GovernorError> for Stopped {
    fn from(error: GovernorError) -> Self {
        LoopFailure::from(error).into()
    }
}

/// Suspends the loop's Run for `reason`.
fn suspend<R: SuspendRunBehavior + ?Sized>(
    runs: &mut R,
    run_id: &RunId,
    reason: SuspensionReason,
) -> Result<(), LoopFailure> {
    match runs.suspend_run(SuspendRun {
        run_id: run_id.clone(),
        reason,
    })? {
        SuspendRunOutcome::Suspended { .. } => Ok(()),
        other => Err(LoopFailure::NotSuspended(Box::new(other))),
    }
}

/// Runs `commission` until its run ends, in the order the module documents.
pub fn run_until_blocked<G, E, A, R, C>(
    governor: &G,
    executor: &E,
    authority: &A,
    commission: &Commission<commission_state::Assigned>,
    runs: &mut R,
    context: &mut C,
) -> Result<LoopEnd, LoopError>
where
    G: Governor + ObservationPort + ?Sized,
    E: AgentExecutor + ?Sized,
    A: AuthorityProvider + ?Sized,
    R: StartRunBehavior + SuspendRunBehavior + ?Sized,
    C: LoopContext + ?Sized,
{
    let revision = governor
        .current_revision(&commission.data().case_id)
        .map_err(unstarted)?;
    let StartRunOutcome::Started { run_started } = runs
        .start_run(StartRun {
            commission_id: commission.data().commission_id.clone(),
            case_revision: revision,
        })
        .map_err(unstarted)?;
    let result = iterate(
        governor,
        executor,
        authority,
        commission,
        runs,
        context,
        &run_started,
    );
    let run_id = run_started.run_id;
    let Stopped { failure, suspended } = match result {
        Ok(end) => return Ok(end),
        Err(stopped) => stopped,
    };
    let suspended = match (&failure, suspended) {
        (_, Some(suspended)) => Some(suspended),
        (LoopFailure::Governor(error), None) => {
            let reason = SuspensionReason::ExternalAvailability(Value::Text(format!(
                "governor failed: {error:?}"
            )));
            let suspension = suspend(runs, &run_id, reason.clone()).err();
            Some((reason, suspension))
        }
        (_, None) => None,
    };
    let (suspension_reason, suspension) = match suspended {
        Some((reason, suspension)) => (Some(Box::new(reason)), suspension.map(Box::new)),
        None => (None, None),
    };
    Err(LoopError {
        run_id: Some(run_id),
        failure,
        suspension,
        suspension_reason,
    })
}

/// The iterations of the loop of the started Run `run`, whose case was loaded at its revision.
fn iterate<G, E, A, R, C>(
    governor: &G,
    executor: &E,
    authority: &A,
    commission: &Commission<commission_state::Assigned>,
    runs: &mut R,
    context: &mut C,
    run: &RunStarted,
) -> Result<LoopEnd, Stopped>
where
    G: Governor + ObservationPort + ?Sized,
    E: AgentExecutor + ?Sized,
    A: AuthorityProvider + ?Sized,
    R: SuspendRunBehavior + ?Sized,
    C: LoopContext + ?Sized,
{
    let case = &commission.data().case_id;
    let run_id = &run.run_id;
    let mut requests = Vec::new();
    let mut admitted = Vec::new();
    let end = |outcome, requests, admitted| LoopEnd {
        run_id: run_id.clone(),
        outcome,
        requests,
        admitted,
    };
    let moved = || RunOutcome::NoAdmissibleAction(Unit(true));
    // Whether the previous iteration admitted no request.
    let mut idle = false;
    let mut first = true;

    loop {
        let loaded = if first {
            run.case_revision
        } else {
            governor.current_revision(case)?
        };
        first = false;

        let determination = governor.completion(case)?;
        if let CompletionDetermination::Complete(complete) = &determination {
            let outcome = RunOutcome::Completed(RunOutcomeCompleted {
                outcome: complete.outcome.clone(),
            });
            return Ok(end(outcome, requests, admitted));
        }
        if loaded != run.case_revision {
            return Ok(end(moved(), requests, admitted));
        }

        let frontier = governor.frontier(case)?;
        if frontier.data().case_revision != run.case_revision {
            return Ok(end(moved(), requests, admitted));
        }
        let outcome = executor.run(commission, &frontier);
        let observed = governor.observe(observation(context, &frontier, &outcome));
        if let ExecutorOutcome::Suspended(suspended) = &outcome {
            let reason = suspended.reason.clone();
            let suspension = suspend(runs, run_id, reason.clone()).err();
            return match (observed, suspension) {
                (Ok(()), None) => Ok(end(
                    RunOutcome::Suspended(RunOutcomeSuspended { reason }),
                    requests,
                    admitted,
                )),
                (Ok(()), Some(failure)) => Err(Stopped {
                    failure,
                    suspended: Some((reason, None)),
                }),
                (Err(error), suspension) => Err(Stopped {
                    failure: LoopFailure::Governor(error),
                    suspended: Some((reason, suspension)),
                }),
            };
        }
        observed?;

        let mut progressed = false;
        let mut decided = false;
        let mut verdict: Option<(String, AuthorityVerdict)> = None;
        if let ExecutorOutcome::ProposedAction(proposed) = &outcome {
            let made = request(
                context.action_request_id(),
                run_id.clone(),
                &frontier,
                proposed.clone(),
            );
            let answer = revalidate(governor, &made)?;
            let data = made.into_data();
            requests.push(Revalidated {
                request: data.clone(),
                outcome: answer.clone(),
            });
            // A request equal to one already admitted in this Run is no progress.
            let new = !admitted.iter().any(|earlier: &ActionRequestData| {
                earlier.action == data.action
                    && earlier.arguments == data.arguments
                    && earlier.expected_case_revision == data.expected_case_revision
            });
            match answer {
                RevalidateActionRequestOutcome::Admitted => {
                    admitted.push(data);
                    progressed = new;
                    decided = true;
                }
                // Not counted toward the bound: the next iteration's load ends the run.
                RevalidateActionRequestOutcome::Stale { .. } => continue,
                RevalidateActionRequestOutcome::NeedsAuthority { error } => {
                    if let AuthorityCheck::Decided(answer) =
                        check_authority(authority, commission, &error.capability)
                    {
                        if answer == AuthorityVerdict::Allow(Unit(true)) {
                            admitted.push(data);
                            progressed = new;
                            decided = true;
                        }
                        verdict = Some((error.capability, answer));
                    }
                }
                RevalidateActionRequestOutcome::NotAdmitted { .. } => {}
            }
        }

        if !decided {
            let counted = verdict
                .as_ref()
                .map(|(capability, verdict)| CapabilityVerdict {
                    capability,
                    verdict,
                });
            if let Derived::Ended(outcome) = derive(&determination, &frontier, &outcome, counted) {
                return Ok(end(outcome, requests, admitted));
            }
        }

        if !progressed && idle {
            return Ok(end(
                RunOutcome::NoAdmissibleAction(Unit(true)),
                requests,
                admitted,
            ));
        }
        idle = !progressed;
    }
}

/// The observation of one executor step, as the module documents.
fn observation<C: LoopContext + ?Sized>(
    context: &mut C,
    frontier: &Frontier<frontier_state::Issued>,
    outcome: &ExecutorOutcome,
) -> Observation<crate::model::responsibility::observation_state::Reported> {
    let frontier = frontier.data();
    let text = |text: &str| Value::Text(text.to_owned());
    let payload = match outcome {
        ExecutorOutcome::ProposedAction(proposed) => vec![
            ("outcome".to_owned(), text("ProposedAction")),
            ("action".to_owned(), text(&proposed.action)),
            ("arguments".to_owned(), proposed.arguments.0.clone()),
        ],
        ExecutorOutcome::NeedsHumanJudgment(needs) => vec![
            ("outcome".to_owned(), text("NeedsHumanJudgment")),
            ("request".to_owned(), needs.request.0.clone()),
        ],
        ExecutorOutcome::Suspended(_) => vec![("outcome".to_owned(), text("Suspended"))],
        ExecutorOutcome::NoUsefulAction(_) => vec![("outcome".to_owned(), text("NoUsefulAction"))],
        ExecutorOutcome::CompletedLocalReasoning(_) => {
            vec![("outcome".to_owned(), text("CompletedLocalReasoning"))]
        }
    };
    Observation::new(ObservationData {
        observation_id: context.observation_id(),
        source: "executor".to_owned(),
        subject: format!("{}@{}", frontier.case_id.0, frontier.case_revision),
        observed_at: context.now(),
        payload: Value::Object(payload),
    })
}
