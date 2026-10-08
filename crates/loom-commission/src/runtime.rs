//! The runtime loop: [`run_until_blocked`] drives one commission over its ports until its run ends,
//! and hands every admitted action request to the [`EffectPort`] (Atlas ADR 0082).
//!
//! One call is one loop, and one loop starts one Run of the commission (`StartRun`) at the case
//! revision its first iteration loaded. A Run holds the case at that revision, or at a revision its
//! own effect moved the case to (`ess/commission/domains/responsibility.yaml`, `Run.case_revision`).
//! Each iteration makes these calls, in this order:
//!
//! 0. the step budget ([`LoopContext::step_budget`], or [`UNBUDGETED_STEP_LIMIT`] when it sets
//!    none): when the steps taken reach it, the Run is suspended through `SuspendRun` with
//!    `SuspensionReason::Budget({"max_steps": <budget>})` and the run ends suspended with that
//!    reason. A step is an executor call whose iteration did not end the run ([`LoopEnd::steps`]).
//!    When the last step left an approval gate pending (5), the iteration first reads the case and
//!    the frontier (1-3) and checks the gate, so a last step that leaves the gate unchanged ends
//!    `AwaitingApproval`, not suspended for its budget. When the last step's proposal was stale (8),
//!    the iteration first loads the case (1-2), so a move it finds ends the run as item 2 says;
//!    only a case still at the revision held is suspended for its budget;
//! 1. [`Governor::current_revision`]: the case is loaded. After an iteration whose effect the port
//!    reported `Performed`, the loaded revision is the one the loop holds the case at from then on;
//!    after a `Refused` one, as after no effect, a move is somebody else's;
//! 2. [`Governor::completion`]: a case the governor holds complete ends the run completed, carrying
//!    the governor's outcome, and the iteration makes no further call. Otherwise, when the loaded
//!    revision is not the one the loop holds the case at, the runtime has found a move itself (as
//!    after a stale proposal, 8): the frontier is read ([`Governor::frontier`]), and the run ends
//!    `CaseMovedOn` when that frontier is for the loaded revision and, as the executor would be
//!    handed it (6), admits an action, naming the Run's revision and the loaded one
//!    (`story:moved-run-named-outcome`); otherwise with no admissible action. The executor is not
//!    run;
//! 3. [`Governor::frontier`]: the frontier is obtained ([`LoopEnd::last_frontier`]). When it is for
//!    another revision than the one held, the run ends with no admissible action, so every request
//!    carries the held revision;
//! 4. a frontier that lists actions, none of which the effect port performs
//!    ([`EffectPort::performs`]), ends the run with `NoPerformableAction`, and the executor is not
//!    run. A frontier that lists no action at all goes on: the outcome is derived from it;
//! 5. the approval gate, read on the frontier the executor is handed (6): when the previous
//!    iteration was a step on a handed frontier listing actions that need approval, and this
//!    handed frontier lists the same actions, each with the same status and reasons, the run ends
//!    `AwaitingApproval`, carrying the actions that need approval in frontier order, and the
//!    executor is not run. An action an authority verdict denied in this Run is not awaited; a
//!    frontier whose only actions needing approval were denied is no gate;
//! 6. [`AgentExecutor::run`] on that frontier, less every action the effect port does not perform
//!    and that needs no authority (`story:effect-invocation`), action by action: an action with an
//!    entry listed `ApprovalRequired`, or an entry naming a capability, stays with all its entries,
//!    performed or not, so a run still stops at its gate, and a kept action is admitted as the
//!    governor's frontier admits it. The gate (5) and the run outcome (8-10) read this handed
//!    frontier, or after a move (9, 10) the reloaded one as it would be handed, so an action the
//!    executor was never offered neither keeps a run going nor takes it off its gate; the request
//!    (8) and [`LoopEnd::last_frontier`] read the frontier as the governor issued it. The step is
//!    reported to the [`ObservationPort`] as one observation: its id and time come from the
//!    [`LoopContext`], its source is `executor`, its subject is `<case>@<frontier revision>`, and
//!    its payload names the outcome (`outcome`) and carries a proposal's `action` and `arguments`,
//!    a human request's `request`, or a reported move's `expected_case_revision`. It is never
//!    evidence;
//! 7. on `Suspended`, the Run is suspended through `SuspendRun` with the executor's reason and the
//!    run ends suspended, carrying it. The Run is suspended with that reason even when delivering
//!    the step's observation failed; the loop then returns that failure as a [`LoopError`] that
//!    carries the reason;
//! 8. on `ProposedAction`, the proposal becomes an action request bound to the frontier's revision
//!    ([`request`], its id from the context) and is revalidated ([`revalidate`]). Admitted, or
//!    needing authority the provider allows ([`check_authority`]): when the port does not perform
//!    its action ([`EffectPort::performs`]), the run ends with `NoPerformableAction`, the request
//!    recorded with its revalidation in [`LoopEnd::requests`] but not admitted, and the port is
//!    never handed it. Otherwise the request is recorded as admitted and handed to
//!    [`EffectPort::invoke`] once, as an [`AdmittedRequest`]. Its
//!    [`EffectOutcome`] is recorded ([`LoopEnd::effects`]) and delivered as one observation: id and
//!    time from the context, source [`EFFECT_SOURCE`], subject `<case>@<request revision>`, payload
//!    `outcome` (`Performed` or `Refused`), `action_request`, `action`, `arguments`, and `report`
//!    with, when the effect was invoked through a Connector, the one `attempt` a write produced or
//!    the `audit` record of a read, or `reason`. It is never evidence.
//!    The loop never retries an invocation. Stale: the iteration is a step, and the next iteration
//!    loads the case, which has moved: it ends the run completed if the case is complete, else
//!    `CaseMovedOn` or with no admissible action (2). Otherwise, and after an authority answer other than an allow, the run
//!    outcome is derived ([`derive`]) from the frontier the executor was handed, and the case is
//!    not loaded again;
//! 9. on `CaseMoved`, by which the executor reports that the case moved while it worked, naming
//!    the revision of the frontier it was handed (`story:moved-case-outcome`), the governor decides
//!    whether it moved: the case is loaded once more ([`Governor::current_revision`]). At the
//!    revision the loop holds it at, the move is not borne out, and the step is taken as
//!    `NoUsefulAction`: the run goes on as on that outcome, but the case is not loaded again for
//!    it (10). At another, the run ends judged on the case as the governor holds it now, and the
//!    executor is not run again ([`Governor::completion`], then [`Governor::frontier`]): a case the
//!    governor holds complete ends the run completed, with no frontier read; a frontier for another
//!    revision than the one just loaded ends it with no admissible action; one that lists actions,
//!    none of which the effect port performs, with `NoPerformableAction` (4). Otherwise the outcome
//!    is derived from that frontier as the executor would be handed it (6), and where it would let
//!    the run go on, the run ends `CaseMovedOn` (`story:moved-run-named-outcome`), naming the Run's
//!    revision (`bound_case_revision`) and the one just loaded (`current_case_revision`): a Run
//!    stays bound to its revision, and the caller starts a new Run at the current one. The
//!    reported revision is observed (6), never trusted;
//! 10. on `CompletedLocalReasoning` or `NoUsefulAction`, the run outcome is derived from the
//!     frontier the executor was handed. Where that would end the run, the case is first loaded
//!     once more ([`Governor::current_revision`]): an executor can propose nothing without having
//!     read a move that happened while it worked. At the revision the loop holds it at, the run
//!     ends on the derived outcome; at another, it ends as on a `CaseMoved` the governor bears out
//!     (9). A governor that fails on that read fails the loop as on item 9's. Where the derivation
//!     lets the run go on, the case is not loaded here: the next iteration loads it (1). On any
//!     other outcome, the run outcome is derived from the frontier the executor was handed.
//!
//! Without a step budget the loop is bounded where the frontier does not change: two iterations in
//! a row that are idle end the run with no admissible action. An iteration is idle when it admits
//! no request, or admits only a request with the action and arguments of one already admitted in
//! this Run, at whatever revision: an effect that moved the case does not make the same proposal
//! new. A newly admitted request starts the count again. Idle covers `NoUsefulAction`,
//! `CompletedLocalReasoning`, a refused proposal, an authority answer other than an allow and a
//! repeated admission alike, and a `CaseMoved` the governor does not bear out counts as
//! `NoUsefulAction`; a stale proposal is not counted, since the next iteration ends the run, and a
//! `CaseMoved` the governor bears out ends it at once (9). The idle bound ends a run whose
//! derivation would let it go on, so the case is not loaded again before it (10). A loop that
//! keeps making progress is ended by [`UNBUDGETED_STEP_LIMIT`]. With a step budget, the budget
//! alone bounds the loop.
//!
//! A failure that leaves no run outcome is a [`LoopError`]. Once the Run is started, the error
//! names it. When the governor, its observation port or the effect port fails after the Run
//! started, the loop first suspends the Run (`SuspendRun`, `ExternalAvailability`) unless it
//! already suspended it for the executor, so no Run is left `Running` that no return value
//! identifies; when that suspension fails too, the error carries both failures. The error reports
//! the reason the loop suspended the Run for ([`LoopError::suspension_reason`]): the reason is
//! reported, not stored on the Run.
#![forbid(unsafe_code)]

use std::fmt;

use crate::action_request::{request, revalidate};
use crate::model::json::Value;
use crate::model::obligation::UnmetObligation;
use crate::model::primitives::Timestamp;
use crate::model::responsibility::obligations::{StartRunBehavior, SuspendRunBehavior};
use crate::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AuthorityVerdict, CaseId, Commission,
    CompletionDetermination, EffectOutcome, ExecutorOutcome, Frontier, FrontierAction,
    FrontierData, GovernorError, Observation, ObservationData, ObservationId,
    RevalidateActionRequestOutcome, RunId, RunOutcome, RunOutcomeAwaitingApproval,
    RunOutcomeCaseMovedOn, RunOutcomeCompleted, RunOutcomeSuspended, RunStarted, StartRun,
    StartRunOutcome, SuspendRun, SuspendRunOutcome, SuspensionReason, Unit, commission_state,
    frontier_state, observation_state,
};
use crate::outcome::{CapabilityVerdict, Derived, derive};
use crate::ports::authority::{AuthorityCheck, AuthorityProvider, check_authority};
use crate::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use crate::ports::evidence::ObservationPort;
use crate::ports::executor::AgentExecutor;
use crate::ports::governor::Governor;

/// The source of the observation of an executor step.
pub const EXECUTOR_SOURCE: &str = "executor";

/// The source of the observation of an effect.
pub const EFFECT_SOURCE: &str = "effect";

/// What the loop needs that neither the model nor the executor may supply: new ids, trusted time
/// and the step budget.
pub trait LoopContext {
    /// A new id for an action request.
    fn action_request_id(&mut self) -> ActionRequestId;
    /// A new id for an observation.
    fn observation_id(&mut self) -> ObservationId;
    /// The current time, from a trusted clock.
    fn now(&mut self) -> Timestamp;
    /// The most steps the loop takes before it suspends the Run for `Budget`. `None` sets no
    /// budget: the idle bound then ends a loop that makes no progress, and [`UNBUDGETED_STEP_LIMIT`]
    /// ends any other, so the loop always ends.
    fn step_budget(&self) -> Option<usize>;
}

/// The steps a loop with no step budget takes at most before it suspends the Run for `Budget`.
pub const UNBUDGETED_STEP_LIMIT: usize = 64;

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
    /// The requests admitted, in order. Each was handed to the effect port once.
    pub admitted: Vec<ActionRequestData>,
    /// What the effect port answered for each admitted request, in the same order.
    pub effects: Vec<EffectOutcome>,
    /// The executor calls whose iteration did not end the run.
    pub steps: usize,
    /// The frontier the last iteration that got so far read before its executor step; `None` when
    /// no iteration read one.
    pub last_frontier: Option<FrontierData>,
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
    /// The effect port could not answer for an admitted request.
    Effect(EffectError),
}

impl fmt::Display for LoopFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Governor(error) => write!(f, "governor failed: {error:?}"),
            Self::Obligation(unmet) => write!(f, "run command failed: {unmet}"),
            Self::NotSuspended(outcome) => write!(f, "run not suspended: {outcome:?}"),
            Self::Effect(error) => error.fmt(f),
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
    /// The reason the loop suspended the Run for, or tried to: the executor's, `Budget`, or
    /// `ExternalAvailability` after a governor or effect failure. `None` when it did not try.
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
pub fn run_until_blocked<G, E, A, F, R, C>(
    governor: &G,
    executor: &E,
    authority: &A,
    effects: &F,
    commission: &Commission<commission_state::Assigned>,
    runs: &mut R,
    context: &mut C,
) -> Result<LoopEnd, LoopError>
where
    G: Governor + ObservationPort + ?Sized,
    E: AgentExecutor + ?Sized,
    A: AuthorityProvider + ?Sized,
    F: EffectPort + ?Sized,
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
    let ports = Ports {
        governor,
        executor,
        authority,
        effects,
    };
    let result = iterate(&ports, commission, runs, context, &run_started);
    let run_id = run_started.run_id;
    let Stopped { failure, suspended } = match result {
        Ok(end) => return Ok(end),
        Err(stopped) => stopped,
    };
    let unavailable = match (&failure, &suspended) {
        (_, Some(_)) => None,
        (LoopFailure::Governor(error), None) => Some(format!("governor failed: {error:?}")),
        (LoopFailure::Effect(error), None) => Some(error.to_string()),
        (_, None) => None,
    };
    let suspended = match unavailable {
        Some(text) => {
            let reason = SuspensionReason::ExternalAvailability(Value::Text(text));
            let suspension = suspend(runs, &run_id, reason.clone()).err();
            Some((reason, suspension))
        }
        None => suspended,
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

/// The ports one loop calls out through.
struct Ports<'p, G: ?Sized, E: ?Sized, A: ?Sized, F: ?Sized> {
    governor: &'p G,
    executor: &'p E,
    authority: &'p A,
    effects: &'p F,
}

/// What one loop has made so far.
#[derive(Default)]
struct Track {
    requests: Vec<Revalidated>,
    admitted: Vec<ActionRequestData>,
    effects: Vec<EffectOutcome>,
    steps: usize,
    last_frontier: Option<FrontierData>,
}

impl Track {
    fn end(self, run_id: &RunId, outcome: RunOutcome) -> LoopEnd {
        LoopEnd {
            run_id: run_id.clone(),
            outcome,
            requests: self.requests,
            admitted: self.admitted,
            effects: self.effects,
            steps: self.steps,
            last_frontier: self.last_frontier,
        }
    }
}

/// The iterations of the loop of the started Run `run`, whose case was loaded at its revision.
fn iterate<G, E, A, F, R, C>(
    ports: &Ports<'_, G, E, A, F>,
    commission: &Commission<commission_state::Assigned>,
    runs: &mut R,
    context: &mut C,
    run: &RunStarted,
) -> Result<LoopEnd, Stopped>
where
    G: Governor + ObservationPort + ?Sized,
    E: AgentExecutor + ?Sized,
    A: AuthorityProvider + ?Sized,
    F: EffectPort + ?Sized,
    R: SuspendRunBehavior + ?Sized,
    C: LoopContext + ?Sized,
{
    let Ports {
        governor,
        executor,
        authority,
        effects,
    } = *ports;
    let case = &commission.data().case_id;
    let run_id = &run.run_id;
    let budget = context.step_budget();
    let mut track = Track::default();
    // The revision the loop holds the case at: the Run's, or one its own effect moved the case to.
    let mut held = run.case_revision;
    // Whether the previous iteration invoked an effect.
    let mut effected = false;
    // Whether the previous iteration admitted no request.
    let mut idle = false;
    // The actions of the frontier the previous step's executor was handed, when it listed actions
    // awaiting approval.
    let mut gate: Option<Vec<FrontierAction>> = None;
    // The actions an authority verdict denied in this Run.
    let mut denied: Vec<String> = Vec::new();
    let mut first = true;
    // Whether the previous iteration's proposal was stale: the case has moved, and this iteration
    // loads it before any budget stop (0).
    let mut after_stale = false;

    let limit = budget.unwrap_or(UNBUDGETED_STEP_LIMIT);

    loop {
        // With the budget used up, a pending approval gate is still read and checked first, and
        // after a stale proposal the move is judged first.
        let exhausted = track.steps >= limit;
        if exhausted && gate.is_none() && !after_stale {
            return out_of_budget(runs, run_id, limit, track);
        }

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
            return Ok(track.end(run_id, outcome));
        }
        if effected {
            held = loaded;
            effected = false;
        } else if loaded != held {
            // The runtime found the move itself, as after a stale proposal (2).
            let ended = found_moved(
                governor,
                effects,
                case,
                run.case_revision,
                loaded,
                &determination,
            )?;
            return Ok(track.end(run_id, ended));
        }

        let frontier = governor.frontier(case)?;
        if frontier.data().case_revision != held {
            return Ok(track.end(run_id, RunOutcome::NoAdmissibleAction(Unit(true))));
        }
        track.last_frontier = Some(frontier.data().clone());
        if performs_none(&frontier, effects) {
            return Ok(track.end(run_id, RunOutcome::NoPerformableAction(Unit(true))));
        }
        // The frontier the executor is handed; the gate and the outcome read it too.
        let handed = offered(&frontier, effects);
        if let Some(before) = &gate
            && unchanged(before, &handed.data().actions)
        {
            let actions = awaited(&handed, &denied);
            if !actions.is_empty() {
                let outcome = RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval { actions });
                return Ok(track.end(run_id, outcome));
            }
        }
        if exhausted {
            return out_of_budget(runs, run_id, limit, track);
        }

        let outcome = executor.run(commission, &handed);
        let observed = governor.observe(observation(context, &frontier, &outcome));
        if let ExecutorOutcome::Suspended(suspended) = &outcome {
            let reason = suspended.reason.clone();
            let suspension = suspend(runs, run_id, reason.clone()).err();
            return match (observed, suspension) {
                (Ok(()), None) => Ok(track.end(
                    run_id,
                    RunOutcome::Suspended(RunOutcomeSuspended { reason }),
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
        // An outcome that proposes nothing ends the run only once the case is loaded again (10).
        let proposes_nothing = matches!(
            outcome,
            ExecutorOutcome::CompletedLocalReasoning(_) | ExecutorOutcome::NoUsefulAction(_)
        );
        let outcome = match outcome {
            ExecutorOutcome::CaseMoved(_) => {
                match moved_outcome(governor, effects, case, held, run.case_revision, &outcome)? {
                    Some(ended) => return Ok(track.end(run_id, ended)),
                    // The governor holds the case where the Run does: the move is not borne out.
                    None => ExecutorOutcome::NoUsefulAction(Unit(true)),
                }
            }
            other => other,
        };

        let mut progressed = false;
        let mut decided = false;
        let mut stale = false;
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
            track.requests.push(Revalidated {
                request: data.clone(),
                outcome: answer.clone(),
            });
            // A request for the action and arguments of one already admitted in this Run is no
            // progress, at whatever revision: an effect that moves the case does not make the same
            // proposal new.
            let new = !track.admitted.iter().any(|earlier: &ActionRequestData| {
                earlier.action == data.action && earlier.arguments == data.arguments
            });
            let mut admitted = false;
            match answer {
                RevalidateActionRequestOutcome::Admitted => admitted = true,
                // Not counted toward the idle bound: the next iteration's load ends the run.
                RevalidateActionRequestOutcome::Stale { .. } => stale = true,
                RevalidateActionRequestOutcome::NeedsAuthority { error } => {
                    if let AuthorityCheck::Decided(answer) =
                        check_authority(authority, commission, &error.capability)
                    {
                        match &answer {
                            AuthorityVerdict::Allow(Unit(true)) => admitted = true,
                            AuthorityVerdict::Deny(_) => denied.push(error.action.clone()),
                            _ => {}
                        }
                        verdict = Some((error.capability, answer));
                    }
                }
                RevalidateActionRequestOutcome::NotAdmitted { .. } => {}
            }
            if admitted {
                // Never hand the port an action it says it does not perform.
                if !effects.performs(&data.action) {
                    return Ok(track.end(run_id, RunOutcome::NoPerformableAction(Unit(true))));
                }
                track.admitted.push(data.clone());
                progressed = new;
                decided = true;
                let effect = effects
                    .invoke(commission, &AdmittedRequest::new(data.clone()))
                    .map_err(|error| Stopped::from(LoopFailure::Effect(error)))?;
                track.effects.push(effect.clone());
                governor.observe(effect_observation(context, &data, &effect))?;
                // Only an effect that took place may have moved the case; after a refusal, a move
                // is somebody else's.
                effected = matches!(effect, EffectOutcome::Performed(_));
            }
        }

        if !decided && !stale {
            let counted = verdict
                .as_ref()
                .map(|(capability, verdict)| CapabilityVerdict {
                    capability,
                    verdict,
                });
            if let Derived::Ended(derived) = derive(&determination, &handed, &outcome, counted) {
                // The case may have moved while the executor worked without its reading it (10).
                if proposes_nothing
                    && let Some(ended) =
                        moved_outcome(governor, effects, case, held, run.case_revision, &outcome)?
                {
                    return Ok(track.end(run_id, ended));
                }
                return Ok(track.end(run_id, derived));
            }
        }

        if budget.is_none() && !stale && !progressed && idle {
            return Ok(track.end(run_id, RunOutcome::NoAdmissibleAction(Unit(true))));
        }
        after_stale = stale;
        if !stale {
            idle = !progressed;
        }
        track.steps += 1;
        gate = if awaited(&handed, &denied).is_empty() {
            None
        } else {
            Some(handed.data().actions.clone())
        };
    }
}

/// Suspends the Run for `Budget` with `limit` steps, and ends the loop suspended.
fn out_of_budget<R: SuspendRunBehavior + ?Sized>(
    runs: &mut R,
    run_id: &RunId,
    limit: usize,
    track: Track,
) -> Result<LoopEnd, Stopped> {
    let reason = SuspensionReason::Budget(Value::Object(vec![(
        "max_steps".to_owned(),
        Value::Number(limit.to_string()),
    )]));
    if let Err(failure) = suspend(runs, run_id, reason.clone()) {
        return Err(Stopped {
            failure,
            suspended: Some((reason, None)),
        });
    }
    let outcome = RunOutcome::Suspended(RunOutcomeSuspended { reason });
    Ok(track.end(run_id, outcome))
}

/// Whether `frontier` lists actions and the effect port performs none of them (item 4).
fn performs_none<F: EffectPort + ?Sized>(
    frontier: &Frontier<frontier_state::Issued>,
    effects: &F,
) -> bool {
    let actions = &frontier.data().actions;
    !actions.is_empty()
        && !actions
            .iter()
            .any(|listed| effects.performs(&listed.action))
}

/// How a run ends after its executor reported `CaseMoved` (item 9), or proposed nothing on a
/// frontier that would end the run (item 10), or `None` when the governor does not bear a move
/// out: it holds `case` at `held`, the revision the loop holds it at. Otherwise the case is judged
/// by [`moved_judged`] at the revision just loaded.
fn moved_outcome<G, F>(
    governor: &G,
    effects: &F,
    case: &CaseId,
    held: i64,
    bound: i64,
    reported: &ExecutorOutcome,
) -> Result<Option<RunOutcome>, GovernorError>
where
    G: Governor + ?Sized,
    F: EffectPort + ?Sized,
{
    let loaded = governor.current_revision(case)?;
    if loaded == held {
        return Ok(None);
    }
    let determination = governor.completion(case)?;
    moved_judged(
        governor,
        effects,
        case,
        bound,
        loaded,
        &determination,
        reported,
    )
    .map(Some)
}

/// How a run ends whose case the runtime found moved to `loaded` when it loaded it (item 2), the
/// governor not holding it complete (`determination`): `CaseMovedOn` when the frontier current then
/// is for `loaded` and, as the executor would be handed it, admits an action; otherwise with no
/// admissible action. The Run is bound to `bound`, its own revision.
fn found_moved<G, F>(
    governor: &G,
    effects: &F,
    case: &CaseId,
    bound: i64,
    loaded: i64,
    determination: &CompletionDetermination,
) -> Result<RunOutcome, GovernorError>
where
    G: Governor + ?Sized,
    F: EffectPort + ?Sized,
{
    let frontier = governor.frontier(case)?;
    let admits = frontier.data().case_revision == loaded
        && matches!(
            derive(
                determination,
                &offered(&frontier, effects),
                &ExecutorOutcome::NoUsefulAction(Unit(true)),
                None,
            ),
            Derived::Continue
        );
    Ok(if admits {
        case_moved_on(bound, loaded)
    } else {
        RunOutcome::NoAdmissibleAction(Unit(true))
    })
}

/// `CaseMovedOn` from the Run's revision `bound` to `loaded`.
fn case_moved_on(bound: i64, loaded: i64) -> RunOutcome {
    RunOutcome::CaseMovedOn(RunOutcomeCaseMovedOn {
        bound_case_revision: bound,
        current_case_revision: loaded,
    })
}

/// How a run whose case moved to `loaded` ends, judged on the case as the governor holds it now
/// (`determination`, then the frontier current then, as the executor would be handed it). A case
/// the governor holds complete ends the run completed, with no frontier read; a frontier for
/// another revision than `loaded` ends it with no admissible action; one that lists actions, none
/// of which the effect port performs, with `NoPerformableAction`. Otherwise the outcome is derived
/// from that frontier, and where it would let the run go on, the run ends `CaseMovedOn`: the Run is
/// bound to `bound`, its own revision, and the caller starts a new Run at `loaded`.
fn moved_judged<G, F>(
    governor: &G,
    effects: &F,
    case: &CaseId,
    bound: i64,
    loaded: i64,
    determination: &CompletionDetermination,
    reported: &ExecutorOutcome,
) -> Result<RunOutcome, GovernorError>
where
    G: Governor + ?Sized,
    F: EffectPort + ?Sized,
{
    if let CompletionDetermination::Complete(complete) = determination {
        return Ok(RunOutcome::Completed(RunOutcomeCompleted {
            outcome: complete.outcome.clone(),
        }));
    }
    let frontier = governor.frontier(case)?;
    if frontier.data().case_revision != loaded {
        return Ok(RunOutcome::NoAdmissibleAction(Unit(true)));
    }
    if performs_none(&frontier, effects) {
        return Ok(RunOutcome::NoPerformableAction(Unit(true)));
    }
    let handed = offered(&frontier, effects);
    Ok(match derive(determination, &handed, reported, None) {
        Derived::Ended(outcome) => outcome,
        Derived::Continue => case_moved_on(bound, loaded),
    })
}

/// The frontier an executor is handed: `frontier` less every action the effect port does not
/// perform and that needs no authority. An action behind an authority gate stays, performed or not
/// (`decision-blocker:gated-unbound-action-visibility`, B). The filter works by action, not by
/// entry: an action the frontier lists more than once is kept or dropped with every entry it has,
/// and it needs authority when any of its entries is `ApprovalRequired` or names a capability, as
/// the local slice's `repository.merge` does while it is still `Blocked`. So each kept action is
/// admitted exactly as the governor's frontier admits it. Same id, case and revision.
fn offered<F: EffectPort + ?Sized>(
    frontier: &Frontier<frontier_state::Issued>,
    effects: &F,
) -> Frontier<frontier_state::Issued> {
    let listed = &frontier.data().actions;
    let gated = |action: &str| {
        listed.iter().any(|entry| {
            entry.action == action
                && (entry.status == ActionStatus::ApprovalRequired
                    || entry
                        .capability
                        .as_deref()
                        .is_some_and(|capability| !capability.is_empty()))
        })
    };
    let mut data = frontier.data().clone();
    data.actions
        .retain(|entry| effects.performs(&entry.action) || gated(&entry.action));
    Frontier::new(data)
}

/// The actions `frontier` lists as needing approval that no verdict in this Run denied, in its
/// order.
fn awaited(frontier: &Frontier<frontier_state::Issued>, denied: &[String]) -> Vec<String> {
    frontier
        .data()
        .actions
        .iter()
        .filter(|listed| listed.status == ActionStatus::ApprovalRequired)
        .filter(|listed| !denied.contains(&listed.action))
        .map(|listed| listed.action.clone())
        .collect()
}

/// Whether `after` lists the same actions as `before`, each with the same status and reasons.
fn unchanged(before: &[FrontierAction], after: &[FrontierAction]) -> bool {
    before.len() == after.len()
        && before.iter().zip(after).all(|(was, is)| {
            was.action == is.action && was.status == is.status && was.reasons == is.reasons
        })
}

/// The observation of one executor step, as the module documents.
fn observation<C: LoopContext + ?Sized>(
    context: &mut C,
    frontier: &Frontier<frontier_state::Issued>,
    outcome: &ExecutorOutcome,
) -> Observation<observation_state::Reported> {
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
        ExecutorOutcome::CaseMoved(moved) => vec![
            ("outcome".to_owned(), text("CaseMoved")),
            (
                "expected_case_revision".to_owned(),
                Value::Number(moved.expected_case_revision.to_string()),
            ),
        ],
    };
    Observation::new(ObservationData {
        observation_id: context.observation_id(),
        source: EXECUTOR_SOURCE.to_owned(),
        subject: format!("{}@{}", frontier.case_id.0, frontier.case_revision),
        observed_at: context.now(),
        payload: Value::Object(payload),
    })
}

/// The observation of one effect, as the module documents.
fn effect_observation<C: LoopContext + ?Sized>(
    context: &mut C,
    request: &ActionRequestData,
    effect: &EffectOutcome,
) -> Observation<observation_state::Reported> {
    let text = |text: &str| Value::Text(text.to_owned());
    let mut payload = vec![
        (
            "outcome".to_owned(),
            text(match effect {
                EffectOutcome::Performed(_) => "Performed",
                EffectOutcome::Refused(_) => "Refused",
            }),
        ),
        (
            "action_request".to_owned(),
            text(&request.action_request_id.0.0),
        ),
        ("action".to_owned(), text(&request.action)),
        ("arguments".to_owned(), request.arguments.0.clone()),
    ];
    match effect {
        EffectOutcome::Performed(performed) => {
            payload.push(("report".to_owned(), performed.report.clone()));
            if let Some(attempt) = &performed.attempt {
                payload.push(("attempt".to_owned(), text(&attempt.0)));
            }
            if let Some(audit) = &performed.audit {
                payload.push(("audit".to_owned(), text(&audit.0)));
            }
        }
        EffectOutcome::Refused(refused) => {
            payload.push(("reason".to_owned(), text(&refused.reason)));
        }
    }
    Observation::new(ObservationData {
        observation_id: context.observation_id(),
        source: EFFECT_SOURCE.to_owned(),
        subject: format!("{}@{}", request.case_id.0, request.expected_case_revision),
        observed_at: context.now(),
        payload: Value::Object(payload),
    })
}
