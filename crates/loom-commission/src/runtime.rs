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
//!    `AwaitingApproval`, not suspended for its budget. When the last step's effect was
//!    `Performed` (8) and left no gate pending, as a step performed under a grant never does (5),
//!    the iteration first loads the case (1-2), so a case that step completed ends the run
//!    completed; a case still open is suspended for its budget. When the last step's
//!    proposal was stale (8), the iteration first loads the case (1-2), so a move it finds ends the
//!    run as item 2 says; only a case still at the revision held is suspended for its budget;
//! 1. [`Governor::current_revision`]: the case is loaded. After an iteration whose effect the port
//!    reported `Performed`, the loaded revision is the one the loop holds the case at from then on;
//!    after a `Refused` one, as after no effect or a fan-out with a stale read (8a), a move is
//!    somebody else's;
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
//!    frontier whose only actions needing approval were denied is no gate. A step performed under
//!    a grant passes the gate: when the previous step's request needed authority (8), the provider
//!    allowed it and the effect port reported it `Performed`, the gate is not read and the
//!    executor runs. Every other step leaves the gate to be read: one that performed an action
//!    needing no authority, one that proposed nothing, and one whose request was refused, stale,
//!    denied or not performed. The step budget (0) and the idle bound still end a run that only
//!    repeats granted steps; at the budget, the case a granted step leaves is loaded first (0), so
//!    a case it completed ends the run completed;
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
//!    a fan-out's `reads` (each an `action` and its `arguments`, in the order proposed), a human
//!    request's `request`, or a reported move's `expected_case_revision`. It is never evidence;
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
//!
//!    8a. on `ProposedReads` (`architecture-decision-record:read-fan-out`), the fan-out is admitted
//!    only when it lists at least one and at most [`MAX_FAN_OUT`] reads, no two with the same
//!    action and arguments (each read is invoked once), and each names an action the effect port
//!    performs and reads ([`EffectPort::reads`]) that the handed frontier lists, every entry of it
//!    `Admissible` and naming no capability. Otherwise the whole proposal is
//!    refused: no request is made, no authority is asked, nothing is invoked, and the run outcome
//!    is derived as for a refused proposal. An admitted fan-out makes one request per read, in the
//!    order proposed, each with its own id from the context and bound to the frontier's revision,
//!    and revalidates every one before any is invoked. A read whose revalidation does not answer
//!    `Admitted` (a stale one, or one that needs authority by then) is recorded with its answer
//!    and not invoked; the others go on. The admitted reads are recorded as admitted and handed
//!    together to [`EffectPort::invoke_reads`], which invokes each once, concurrently, and joins
//!    them; a port that answers `None` fails the loop as an effect failure. Their effects are
//!    recorded and observed as in 8, in the order proposed, not the order they finished. A
//!    `Refused` read cancels nothing; an `Err` from any read fails the loop once every read has
//!    answered and each that answered was recorded and observed, and nothing is retried. When a
//!    read was stale, somebody else moved the case: once the admitted reads were recorded, the
//!    iteration is as a stale proposal, so the next one loads the case and judges the move (2), and
//!    no read of the fan-out makes the loaded revision the Run's (1). When no read was admitted and
//!    none was stale, the iteration is as a refused proposal. A fan-out is one step, for the step
//!    budget and the idle bound alike; it is progress when one of its admitted reads is new, and it
//!    never passes the approval gate (5);
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
//! no request, or admits only requests with the action and arguments of ones already admitted in
//! this Run, at whatever revision (a refused fan-out admits none): an effect that moved the case does not make the same proposal
//! new. A newly admitted request starts the count again. Idle covers `NoUsefulAction`,
//! `CompletedLocalReasoning`, a refused proposal, an authority answer other than an allow and a
//! repeated admission alike, and a `CaseMoved` the governor does not bear out counts as
//! `NoUsefulAction`; a stale proposal is not counted, since the next iteration ends the run, and a
//! `CaseMoved` the governor bears out ends it at once (9). The idle bound ends a run whose
//! derivation would let it go on, so the case is not loaded again before it (10). A loop that
//! keeps making progress is ended by [`UNBUDGETED_STEP_LIMIT`]. With a step budget, the budget
//! alone bounds the loop.
//!
//! # Run and action budgets
//!
//! A [`LoopContext::run_budget`] bounds the run and each action it lists
//! (`ess/commission/domains/responsibility.yaml`, `RunBudget`). Every effect answer the loop
//! records (8, 8a, and a resumed request) is charged to the run and to its request's action
//! ([`LoopEnd::charges`]): one trial, and the usage a `Performed` effect reported (input and output
//! tokens, wall time, cost; a negative figure charges nothing, and a usage with no cost leaves the
//! cost unknown). A reported usage is delivered in the effect's observation as `usage`. A breach
//! suspends the Run through `SuspendRun` with `SuspensionReason::RunBudget`, naming the action
//! (none for the run's own limits), the limit, its value and the charge, and the run ends
//! suspended with that reason:
//!
//! * `max_steps` acts as the step budget (0), the lower of the two holding; when it is the one
//!   reached, the reason is `RunBudget`, else `Budget`;
//! * an action's `max_trials` is checked before a request is handed to the effect port: a request
//!   that would pass it is recorded with its revalidation and not admitted, and a fan-out that
//!   would take any action past it invokes none of its reads. A resumed request is not checked;
//! * the spending limits (`max_tokens_in`, `max_tokens_out`, `max_duration_ms`,
//!   `max_cost_micro_usd`), the action's in the order the budget lists them and then the run's,
//!   are checked once an effect, or a whole fan-out, was recorded and observed: a charge above a
//!   limit, or an unknown cost under a cost limit, ends the run there, before the case is loaded
//!   again, so no further request is made.
//!
//! A failure that leaves no run outcome is a [`LoopError`]. Once the Run is started, the error
//! names it. When the governor, its observation port or the effect port fails after the Run
//! started, the loop first suspends the Run (`SuspendRun`, `ExternalAvailability`) unless it
//! already suspended it for the executor, so no Run is left `Running` that no return value
//! identifies; when that suspension fails too, the error carries both failures. The error reports
//! the reason the loop suspended the Run for ([`LoopError::suspension_reason`]): the reason is
//! reported, not stored on the Run. A run store that answers `storage-failed` is reported as
//! [`LoopFailure::RunStorage`]: from `StartRun`, the loop ends with no Run named before any
//! executor, authority or effect call; from `SuspendRun`, as the suspension's failure.
//!
//! # Suspending on an approval, and resuming after a restart
//!
//! [`run_with_suspension`] is the same loop given a suspension port ([`SuspensionPort`],
//! `story:approval-suspend-resume-slice`). It differs in two places. Before anything else it reads
//! the port: a commission that holds a Suspension, `Pending` or `Resuming`, starts no Run
//! ([`LoopFailure::CommissionSuspended`], naming its state and request; a port that cannot be read
//! is [`LoopFailure::SuspensionStore`]), and nothing is performed. And when a step's request needs
//! authority and the provider answers `ApprovalRequired`, so the run ends `NeedsAuthority` (8), the
//! Run is first suspended through `SuspendRun` with `SuspensionReason::Authority`, whose payload
//! names the `capability` and the provider's `request`, and the commission's Suspension — the Run,
//! the pending request and that reason — is held `Pending` through the port (`HoldSuspension`).
//! The caller sees the same [`LoopEnd`] as without a port. A `SuspendRun` that fails ends the loop
//! as on any other suspension, and no Suspension is held; a Suspension the port cannot hold ends it
//! with [`LoopFailure::RunStorage`] (`storage-failed`) or [`LoopFailure::SuspensionNotHeld`], the
//! Run suspended, its reason in [`LoopError::suspension_reason`]. A hold the host settled
//! (`settled`: it already did what an approval of the request would lead to,
//! `story:handoff-holds-no-suspension`) holds nothing and is no failure: the Run is put back
//! `Running` through `ResumeRun`, as a loop without a port leaves it, so no Run stays `Suspended`
//! with no Suspension behind it and a resume finds nothing to resume
//! ([`ResumeRefusal::NothingSuspended`]); the caller sees the same [`LoopEnd`]. A `ResumeRun` that
//! fails then ends the loop with [`LoopFailure::RunStorage`] or [`LoopFailure::NotResumed`], the
//! Run suspended. [`run_until_blocked`] passes no port, reads none and never suspends a Run on an
//! approval, as in 0.16.0.
//!
//! [`resume_after_grant`] continues the same Run from the stored Suspension alone, in a process
//! that need not be the one that suspended it. In this order: the Suspension is read
//! ([`ResumeRefusal::NothingSuspended`] when none is stored; [`ResumeRefusal::InDoubt`], naming its
//! request, when it is `Resuming`); it must be the commission's, on its case; the case is loaded,
//! and a revision other than the request's ends it with [`ResumeRefusal::CaseMoved`]; the request
//! is revalidated as the loop revalidates one (8), and a request that still needs authority is
//! resumed only on the provider's allow ([`ResumeRefusal::NotGranted`]); the effect port must
//! perform its action. Then the Run is put back into a run store that does not hold it,
//! `Suspended` at the case revision it started at, and resumed through `ResumeRun`. The Suspension
//! is then taken through the port for the request read and revalidated (`TakeSuspension`,
//! `Pending` → `Resuming`): of two resumes one takes it, and the other is refused,
//! [`ResumeRefusal::NothingSuspended`] when the first already released it,
//! [`ResumeRefusal::InDoubt`] when it is still `Resuming`, and [`ResumeRefusal::OtherRequest`],
//! naming both requests, when the commission holds a Suspension for another request by then (the
//! first resumed it in full and the commission was suspended again), which is kept as it is; a take
//! the port cannot record ends the resume with [`LoopFailure::RunStorage`]. On any answer but
//! `taken` the Run is suspended again in the run store with its stored reason, no Suspension is
//! held, and nothing is performed. The request, under its own id and at its own revision, is handed
//! to [`EffectPort::invoke`] once, recorded and observed as the loop does (8), and the Suspension is
//! released for that request (`ReleaseSuspension`), removed; a release the port refuses ends the
//! loop with [`LoopFailure::SuspensionNotReleased`], naming the performed request. That is the resumed Run's
//! first step ([`LoopEnd::steps`]); the loop holds no frontier an executor was handed for it, so
//! the next iteration reads no approval gate (5), as after a step performed under a grant. The loop
//! goes on from that iteration, holding the case at the revision the request was made at, or at the
//! one a `Performed` effect moved it to (1). A refusal, and any failure before the take, performs
//! nothing and leaves the Suspension `Pending`. A process that stops after the take and before the
//! release — a crash, an effect port or a governor that fails — leaves it `Resuming`: the request
//! may have been performed, so it is never performed again from the Suspension, which stays stored
//! as its durable trace; every later resume is refused [`ResumeRefusal::InDoubt`], and settling the
//! request is the host's.
#![forbid(unsafe_code)]

use std::fmt;

use crate::action_request::{request, revalidate};
use crate::model::behaviour::RunStorage;
use crate::model::json::Value;
use crate::model::obligation::UnmetObligation;
use crate::model::primitives::Timestamp;
use crate::model::responsibility::obligations::{
    ResumeRunBehavior, StartRunBehavior, SuspendRunBehavior,
};
use crate::model::responsibility::{
    ActionCharge, BudgetLimit, ChargedUsage, RunBudget, RunBudgetBreach, RunCharges,
};
use crate::model::responsibility::{
    ActionRequest, ActionRequestData, ActionRequestId, ActionStatus, AnyRun, AuthorityVerdict,
    CaseId, Commission, CompletionDetermination, EffectOutcome, ExecutorOutcome,
    ExecutorOutcomeProposedAction, Frontier, FrontierAction, FrontierData, GovernorError,
    HoldSuspension, HoldSuspensionOutcome, Observation, ObservationData, ObservationId,
    ReleaseSuspension, ReleaseSuspensionOutcome, ResumeRun, ResumeRunOutcome,
    RevalidateActionRequestOutcome, Run, RunData, RunId, RunOutcome, RunOutcomeAwaitingApproval,
    RunOutcomeCaseMovedOn, RunOutcomeCompleted, RunOutcomeSuspended, RunStarted, RunStorageFailed,
    StartRun, StartRunOutcome, SuspendRun, SuspendRunOutcome, SuspensionKey, SuspensionReason,
    SuspensionState, SuspensionStateConflict, TakeSuspension, TakeSuspensionOutcome, Unit,
    commission_state, frontier_state, observation_state,
};
use crate::outcome::{CapabilityVerdict, Derived, derive};
use crate::ports::authority::{AuthorityCheck, AuthorityProvider, check_authority};
use crate::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use crate::ports::evidence::ObservationPort;
use crate::ports::executor::AgentExecutor;
use crate::ports::governor::Governor;
use crate::ports::suspension::{SuspensionPort, SuspensionStoreError};

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
    /// The run's budget and its actions' budgets, as the module documents
    /// (`ess/commission/domains/responsibility.yaml`, `RunBudget`). `None`, the default, sets
    /// none: only the step budget bounds the run.
    fn run_budget(&self) -> Option<RunBudget> {
        None
    }
}

/// The steps a loop with no step budget takes at most before it suspends the Run for `Budget`.
pub const UNBUDGETED_STEP_LIMIT: usize = 64;

/// The most reads one `ExecutorOutcome::ProposedReads` may list; a longer fan-out is refused whole
/// (`architecture-decision-record:read-fan-out`).
pub const MAX_FAN_OUT: usize = 16;

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
    /// What the run, and each action a request was handed to the effect port for, was charged:
    /// the requests handed over and the usage their `Performed` effects reported.
    pub charges: RunCharges,
}

/// One failure that stopped a loop, or the suspension after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopFailure {
    /// The governor, or its observation port, could not answer.
    Governor(GovernorError),
    /// A run command refused with an unmet obligation.
    Obligation(UnmetObligation),
    /// `SuspendRun` did not suspend the loop's run, for a reason other than storage.
    NotSuspended(Box<SuspendRunOutcome>),
    /// The effect port could not answer for an admitted request.
    Effect(EffectError),
    /// The run store could not hold the new run (`StartRun`) or record its suspension
    /// (`SuspendRun`): their `storage-failed` outcome. Also the suspension port's `storage-failed`
    /// when it cannot hold a Suspension (`HoldSuspension`) or record a resume's take
    /// (`TakeSuspension`).
    RunStorage(RunStorageFailed),
    /// [`run_with_suspension`] started no Run: the commission holds a Suspension, in this state and
    /// for this request. Nothing was performed.
    CommissionSuspended(SuspensionStateConflict),
    /// The suspension port could not be read before a Run started. Nothing was performed.
    SuspensionStore(SuspensionStoreError),
    /// `HoldSuspension` did not store the Run's Suspension, for a reason other than storage or a
    /// hold the host settled (`settled`, which is no failure).
    SuspensionNotHeld(Box<HoldSuspensionOutcome>),
    /// `ResumeRun` did not put back `Running` the Run the loop suspended for an approval whose
    /// hold the host settled, for a reason other than storage. The Run stays `Suspended`, with no
    /// Suspension behind it.
    NotResumed(Box<ResumeRunOutcome>),
    /// `ReleaseSuspension` did not remove the Suspension a resume took, after the request it names
    /// was performed. [`UnreleasedSuspension::outcome`] says what the port left stored.
    SuspensionNotReleased(Box<UnreleasedSuspension>),
}

/// A release of the Suspension a resume took that did not remove it, after its request was
/// performed: the effect port answered for it and the governor observed the answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreleasedSuspension {
    /// The request that was performed, the one the resume took the Suspension for.
    pub action_request_id: ActionRequestId,
    /// What `ReleaseSuspension` answered, and so what is stored for the commission:
    ///
    /// * `storage-failed`: the port changed nothing, and the Suspension stays `Resuming` for this
    ///   request, in doubt: every later resume is refused [`ResumeRefusal::InDoubt`], and settling
    ///   it is the host's;
    /// * `not-found`: no Suspension is stored for the commission (something other than this resume
    ///   removed it), so nothing durable records that the request was performed;
    /// * `other-request`: a Suspension for another request is stored, as it was; nothing stored
    ///   names this request;
    /// * `not-taken`: a `Pending` Suspension for this request is stored, as it was, and a later
    ///   resume may take it and perform the request again.
    pub outcome: ReleaseSuspensionOutcome,
}

impl fmt::Display for LoopFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Governor(error) => write!(f, "governor failed: {error:?}"),
            Self::Obligation(unmet) => write!(f, "run command failed: {unmet}"),
            Self::NotSuspended(outcome) => write!(f, "run not suspended: {outcome:?}"),
            Self::Effect(error) => error.fmt(f),
            Self::RunStorage(failed) => write!(f, "run store failed: {}", failed.reason),
            Self::CommissionSuspended(held) => write!(
                f,
                "the commission holds a {:?} Suspension for request {}",
                held.state, held.action_request_id.0.0
            ),
            Self::SuspensionStore(error) => error.fmt(f),
            Self::SuspensionNotHeld(outcome) => write!(f, "suspension not held: {outcome:?}"),
            Self::NotResumed(outcome) => {
                write!(f, "run not resumed after a settled hold: {outcome:?}")
            }
            Self::SuspensionNotReleased(unreleased) => write!(
                f,
                "suspension not released after request {} was performed: {:?}",
                unreleased.action_request_id.0.0, unreleased.outcome
            ),
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

/// Why [`resume_after_grant`] resumed nothing. Nothing was performed, and the Suspension, when one
/// was stored, is stored as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeRefusal {
    /// The suspension port holds no Suspension for the commission, or another resume took and
    /// released it first (`TakeSuspension` answered `not-found`).
    NothingSuspended,
    /// The commission's Suspension is `Resuming`: a resume took it and did not release it, so its
    /// request may have been performed or not. It is never performed again from the Suspension,
    /// which stays stored; settling the request is the host's.
    InDoubt {
        /// The request whose performance is in doubt.
        action_request_id: ActionRequestId,
    },
    /// The commission's Suspension is held for another request than the one this resume read and
    /// revalidated (`TakeSuspension` answered `other-request`): it was resumed and replaced since.
    /// That Suspension is kept as it is, and neither request is performed.
    OtherRequest {
        /// The request this resume read from the Suspension and revalidated.
        action_request_id: ActionRequestId,
        /// The request the Suspension now stored is held for.
        held_action_request_id: ActionRequestId,
    },
    /// The stored Suspension, or the run the run store holds under its run id, names another
    /// commission, case or run revision than the one given.
    OtherCommission,
    /// The case is no longer at the revision the request was suspended at.
    CaseMoved {
        /// The revision the request was suspended at.
        expected_case_revision: i64,
        /// The case's current revision.
        current_case_revision: i64,
    },
    /// The current frontier refuses the request's action: its revalidation answered this.
    NotAdmitted(RevalidateActionRequestOutcome),
    /// The request still needs authority, and the provider did not allow it.
    NotGranted(AuthorityCheck),
    /// The effect port does not perform the request's action.
    NotPerformed,
    /// The run store does not hold the Run suspended: `ResumeRun` answered this.
    RunNotSuspended(ResumeRunOutcome),
}

/// Why [`resume_after_grant`] returned no run outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeError {
    /// Nothing was resumed; why.
    Refused(ResumeRefusal),
    /// The suspension port could not be read. Nothing was resumed.
    Store(SuspensionStoreError),
    /// The governor could not answer before the Run resumed. Nothing was resumed.
    Governor(GovernorError),
    /// `ResumeRun` failed, the Suspension could not be taken, or the resumed loop stopped without
    /// a run outcome.
    Loop(LoopError),
}

impl fmt::Display for ResumeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refusal) => write!(f, "resume refused: {refusal:?}"),
            Self::Store(error) => error.fmt(f),
            Self::Governor(error) => write!(f, "governor failed: {error:?}"),
            Self::Loop(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ResumeError {}

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
    /// The reason the loop suspended the Run for, or tried to, and the suspension's failure. The
    /// reason is boxed, as [`LoopError`] holds it, so a stop stays small.
    suspended: Option<(Box<SuspensionReason>, Option<LoopFailure>)>,
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
        SuspendRunOutcome::StorageFailed { error } => Err(LoopFailure::RunStorage(error)),
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
    let ports = Ports {
        governor,
        executor,
        authority,
        effects,
        suspensions: None,
    };
    start(&ports, commission, runs, context)
}

/// Runs `commission` until its run ends, as [`run_until_blocked`] does, suspending the Run into
/// `suspensions` when it ends on an approval the provider withholds, as the module documents.
// The ports of `run_until_blocked`, and the suspension port.
#[allow(clippy::too_many_arguments)]
pub fn run_with_suspension<G, E, A, F, R, C>(
    governor: &G,
    executor: &E,
    authority: &A,
    effects: &F,
    commission: &Commission<commission_state::Assigned>,
    runs: &mut R,
    suspensions: &dyn SuspensionPort,
    context: &mut C,
) -> Result<LoopEnd, LoopError>
where
    G: Governor + ObservationPort + ?Sized,
    E: AgentExecutor + ?Sized,
    A: AuthorityProvider + ?Sized,
    F: EffectPort + ?Sized,
    R: StartRunBehavior + SuspendRunBehavior + ResumeRunBehavior + ?Sized,
    C: LoopContext + ?Sized,
{
    // A commission holding a Suspension, `Pending` or `Resuming`, starts no new Run: its request
    // is resumed, or settled by the host, from that Suspension alone.
    match suspensions.get(&commission.data().commission_id) {
        Err(error) => return Err(unstarted(LoopFailure::SuspensionStore(error))),
        Ok(Some(held)) => {
            return Err(unstarted(LoopFailure::CommissionSuspended(
                SuspensionStateConflict {
                    state: held.state,
                    action_request_id: held.data.action_request_id,
                },
            )));
        }
        Ok(None) => {}
    }
    let ports = Ports {
        governor,
        executor,
        authority,
        effects,
        suspensions: Some(Suspensions::new(suspensions)),
    };
    start(&ports, commission, runs, context)
}

/// Starts a Run of `commission` and drives it until it ends.
fn start<G, E, A, F, R, C>(
    ports: &Ports<'_, G, E, A, F, R>,
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
    let revision = ports
        .governor
        .current_revision(&commission.data().case_id)
        .map_err(unstarted)?;
    let run_started = match runs
        .start_run(StartRun {
            commission_id: commission.data().commission_id.clone(),
            case_revision: revision,
        })
        .map_err(unstarted)?
    {
        StartRunOutcome::Started { run_started } => run_started,
        StartRunOutcome::StorageFailed { error } => {
            return Err(unstarted(LoopFailure::RunStorage(error)));
        }
    };
    let result = iterate(
        ports,
        commission,
        runs,
        context,
        &run_started,
        run_started.case_revision,
        None,
    );
    finish(runs, run_started.run_id, result)
}

/// The loop's answer for the Run `run_id`: its end, or the error naming it, after suspending the
/// Run for `ExternalAvailability` when a governor or effect failure stopped it and nothing
/// suspended it yet.
fn finish<R: SuspendRunBehavior + ?Sized>(
    runs: &mut R,
    run_id: RunId,
    result: Result<LoopEnd, Stopped>,
) -> Result<LoopEnd, LoopError> {
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
            Some((Box::new(reason), suspension))
        }
        None => suspended,
    };
    let (suspension_reason, suspension) = match suspended {
        Some((reason, suspension)) => (Some(reason), suspension.map(Box::new)),
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
struct Ports<'p, G: ?Sized, E: ?Sized, A: ?Sized, F: ?Sized, R: ?Sized> {
    governor: &'p G,
    executor: &'p E,
    authority: &'p A,
    effects: &'p F,
    /// Where a Run ended on a withheld approval is suspended to; `None` suspends none.
    suspensions: Option<Suspensions<'p, R>>,
}

/// The suspension port of a loop given one, and how the loop puts back `Running` a Run it
/// suspended for an approval when the host settled the hold ([`unsuspend`]).
struct Suspensions<'p, R: ?Sized> {
    port: &'p dyn SuspensionPort,
    unsuspend: fn(&mut R, &RunId) -> Result<(), LoopFailure>,
}

impl<R: ?Sized> Clone for Suspensions<'_, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<R: ?Sized> Copy for Suspensions<'_, R> {}

impl<'p, R: ResumeRunBehavior + ?Sized> Suspensions<'p, R> {
    fn new(port: &'p dyn SuspensionPort) -> Self {
        Self {
            port,
            unsuspend: unsuspend::<R>,
        }
    }
}

/// Puts the Run `run_id`, which the loop suspended for an approval whose hold the host settled,
/// back `Running` (`ResumeRun`): no Suspension stands behind it, so it is left as a loop without a
/// suspension port leaves a Run that ends on an approval.
fn unsuspend<R: ResumeRunBehavior + ?Sized>(
    runs: &mut R,
    run_id: &RunId,
) -> Result<(), LoopFailure> {
    match runs.resume_run(ResumeRun {
        run_id: run_id.clone(),
    })? {
        ResumeRunOutcome::Resumed { .. } => Ok(()),
        ResumeRunOutcome::StorageFailed { error } => Err(LoopFailure::RunStorage(error)),
        other => Err(LoopFailure::NotResumed(Box::new(other))),
    }
}

/// A suspended request [`resume_after_grant`] revalidated and admitted, which the resumed loop
/// hands to the effect port before its first iteration.
struct Resumed {
    request: ActionRequestData,
    answer: RevalidateActionRequestOutcome,
    /// The Suspension the resume took, released once the effect port answered for the request.
    taken: SuspensionKey,
}

/// What one loop has made so far.
struct Track {
    requests: Vec<Revalidated>,
    admitted: Vec<ActionRequestData>,
    effects: Vec<EffectOutcome>,
    steps: usize,
    last_frontier: Option<FrontierData>,
    charges: RunCharges,
}

impl Default for Track {
    fn default() -> Self {
        Self {
            requests: Vec::new(),
            admitted: Vec::new(),
            effects: Vec::new(),
            steps: 0,
            last_frontier: None,
            charges: RunCharges {
                run: uncharged(),
                actions: Vec::new(),
            },
        }
    }
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
            charges: self.charges,
        }
    }

    /// Records `effect`, which the port answered for `request`, and charges it ([`charge`]).
    fn record(&mut self, request: &ActionRequestData, effect: &EffectOutcome) {
        self.effects.push(effect.clone());
        charge(&mut self.charges, &request.action, effect);
    }
}

/// The iterations of the loop of the Run `run`, holding its case at `held`: the Run's own revision
/// for a Run just started, whose case was loaded at it; for a resumed Run, the revision its
/// suspended request was made at, which [`resume_after_grant`] loaded. A resumed Run first hands
/// its request to the effect port, as the module documents.
fn iterate<G, E, A, F, R, C>(
    ports: &Ports<'_, G, E, A, F, R>,
    commission: &Commission<commission_state::Assigned>,
    runs: &mut R,
    context: &mut C,
    run: &RunStarted,
    held: i64,
    resumed: Option<Resumed>,
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
        suspensions,
    } = *ports;
    let case = &commission.data().case_id;
    let run_id = &run.run_id;
    let run_budget = context.run_budget();
    // The run budget's step limit bounds the loop as the step budget does; the lower one holds.
    let run_steps = run_budget
        .as_ref()
        .and_then(|limits| limits.max_steps)
        .map(|max| usize::try_from(max).unwrap_or(0));
    let budget = match (context.step_budget(), run_steps) {
        (Some(steps), Some(run_steps)) => Some(steps.min(run_steps)),
        (steps, run_steps) => steps.or(run_steps),
    };
    let mut track = Track::default();
    // The revision the loop holds the case at: the Run's, or one its own effect moved the case to.
    let mut held = held;
    // Whether the previous iteration invoked an effect.
    let mut effected = false;
    // Whether the previous iteration admitted no request.
    let mut idle = false;
    // The actions of the frontier the previous step's executor was handed, when it listed actions
    // awaiting approval and the step was not performed under a grant.
    let mut gate: Option<Vec<FrontierAction>> = None;
    // The actions an authority verdict denied in this Run.
    let mut denied: Vec<String> = Vec::new();
    let mut first = true;
    // Whether the previous iteration's proposal was stale: the case has moved, and this iteration
    // loads it before any budget stop (0).
    let mut after_stale = false;

    let limit = budget.unwrap_or(UNBUDGETED_STEP_LIMIT);
    // Why the Run is suspended when its steps reach `limit`: the run budget's step limit when it
    // is the one reached, else the step budget.
    let exhausted_for = if run_steps == Some(limit) {
        SuspensionReason::RunBudget(RunBudgetBreach {
            action: None,
            limit: BudgetLimit::MaxSteps,
            max: i64::try_from(limit).unwrap_or(i64::MAX),
            charged: Some(i64::try_from(limit).unwrap_or(i64::MAX)),
        })
    } else {
        step_budget_reason(limit)
    };

    // A resumed Run's suspended request is performed once, as an admitted request is (8). The loop
    // holds no frontier an executor was handed for that step, so no gate is pending after it (5).
    // Once the effect port answered and the governor observed its answer, the request is no
    // longer in doubt and the Suspension the resume took is released. An effect port or a governor
    // that fails first leaves it `Resuming`.
    if let Some(Resumed {
        request: data,
        answer,
        taken,
    }) = resumed
    {
        first = false;
        track.requests.push(Revalidated {
            request: data.clone(),
            outcome: answer,
        });
        track.admitted.push(data.clone());
        let effect = effects
            .invoke(commission, &AdmittedRequest::new(data.clone()))
            .map_err(|error| Stopped::from(LoopFailure::Effect(error)))?;
        track.record(&data, &effect);
        governor.observe(effect_observation(context, &data, &effect))?;
        if let Some(Suspensions { port, .. }) = suspensions {
            match port.release(ReleaseSuspension {
                suspension: taken,
                action_request_id: data.action_request_id.clone(),
            }) {
                ReleaseSuspensionOutcome::Released { .. } => {}
                outcome => {
                    return Err(LoopFailure::SuspensionNotReleased(Box::new(
                        UnreleasedSuspension {
                            action_request_id: data.action_request_id,
                            outcome,
                        },
                    ))
                    .into());
                }
            }
        }
        effected = matches!(effect, EffectOutcome::Performed(_));
        track.steps += 1;
        if let Some(breach) = run_budget
            .as_ref()
            .and_then(|limits| spent_past(limits, &track))
        {
            return suspend_ended(runs, run_id, SuspensionReason::RunBudget(breach), track);
        }
    }

    loop {
        // With the budget used up, a pending approval gate is still read and checked first, after
        // a stale proposal the move is judged first, and after a performed effect the case is
        // loaded first, so a case the last step completed ends the run completed.
        let exhausted = track.steps >= limit;
        if exhausted && gate.is_none() && !after_stale && !effected {
            return suspend_ended(runs, run_id, exhausted_for, track);
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
        // After a performed effect that left no gate pending, a case still open is suspended for
        // its budget (0).
        if exhausted && gate.is_none() && !after_stale {
            return suspend_ended(runs, run_id, exhausted_for, track);
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
            return suspend_ended(runs, run_id, exhausted_for, track);
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
                    suspended: Some((Box::new(reason), None)),
                }),
                (Err(error), suspension) => Err(Stopped {
                    failure: LoopFailure::Governor(error),
                    suspended: Some((Box::new(reason), suspension)),
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
        // Whether this step's request needed authority, the provider allowed it and the effect
        // port performed it: such a step passes the approval gate (5).
        let mut granted = false;
        // This step's request, when its revalidation answered that it needs authority.
        let mut pending: Option<ActionRequestData> = None;
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
                    pending = Some(data.clone());
                    if let AuthorityCheck::Decided(answer) =
                        check_authority(authority, commission, &data, &error.capability)
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
                // An action at its trial limit is not handed to the port again.
                if let Some(breach) = run_budget
                    .as_ref()
                    .and_then(|limits| trials_past(limits, &track, &[data.action.as_str()]))
                {
                    return suspend_ended(runs, run_id, SuspensionReason::RunBudget(breach), track);
                }
                track.admitted.push(data.clone());
                progressed = new;
                decided = true;
                let effect = effects
                    .invoke(commission, &AdmittedRequest::new(data.clone()))
                    .map_err(|error| Stopped::from(LoopFailure::Effect(error)))?;
                track.record(&data, &effect);
                governor.observe(effect_observation(context, &data, &effect))?;
                // Usage past a spending limit stops the run once the effect is recorded and
                // observed.
                if let Some(breach) = run_budget
                    .as_ref()
                    .and_then(|limits| spent_past(limits, &track))
                {
                    return suspend_ended(runs, run_id, SuspensionReason::RunBudget(breach), track);
                }
                // Only an effect that took place may have moved the case; after a refusal, a move
                // is somebody else's.
                effected = matches!(effect, EffectOutcome::Performed(_));
                granted = effected && matches!(verdict, Some((_, AuthorityVerdict::Allow(_))));
            }
        }
        // A fan-out of reads (8a): refused whole unless every read is admissible on the handed
        // frontier and one the port reads; otherwise each read is revalidated, and the admitted
        // ones are invoked together. It is one step.
        if let ExecutorOutcome::ProposedReads(proposed) = &outcome
            && fans_out(&proposed.reads, &handed, effects)
        {
            let fanned = fan_out(
                governor,
                effects,
                commission,
                context,
                run_id,
                &frontier,
                &proposed.reads,
                run_budget.as_ref(),
                &mut track,
            )?;
            if let Some(breach) = fanned.over_budget.or_else(|| {
                run_budget
                    .as_ref()
                    .and_then(|limits| spent_past(limits, &track))
            }) {
                return suspend_ended(runs, run_id, SuspensionReason::RunBudget(breach), track);
            }
            if fanned.admitted {
                decided = true;
                progressed = fanned.progressed;
                // A stale read shows that somebody else moved the case: no read of this fan-out
                // makes the revision the next iteration loads the Run's own (1, 8a).
                effected = fanned.performed && !fanned.stale;
            }
            stale = fanned.stale;
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
                // With a suspension port, a run that ends on an approval the provider withholds is
                // suspended, and its Suspension stored, before the caller hears of it; a hold the
                // host settled stores none and puts the Run back `Running`.
                if let (
                    Some(suspending),
                    RunOutcome::NeedsAuthority(_),
                    Some(request),
                    Some((capability, AuthorityVerdict::ApprovalRequired(required))),
                ) = (suspensions, &derived, &pending, &verdict)
                {
                    let reason = SuspensionReason::Authority(Value::Object(vec![
                        ("capability".to_owned(), Value::Text(capability.clone())),
                        ("request".to_owned(), Value::Text(required.request.clone())),
                    ]));
                    suspend_for_approval(runs, suspending, run, request, reason)?;
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
        gate = if granted || awaited(&handed, &denied).is_empty() {
            None
        } else {
            Some(handed.data().actions.clone())
        };
    }
}

/// The reason a Run is suspended for at a step budget of `limit` steps.
fn step_budget_reason(limit: usize) -> SuspensionReason {
    SuspensionReason::Budget(Value::Object(vec![(
        "max_steps".to_owned(),
        Value::Number(limit.to_string()),
    )]))
}

/// Suspends the Run for `reason`, a budget it reached, and ends the loop suspended.
fn suspend_ended<R: SuspendRunBehavior + ?Sized>(
    runs: &mut R,
    run_id: &RunId,
    reason: SuspensionReason,
    track: Track,
) -> Result<LoopEnd, Stopped> {
    if let Err(failure) = suspend(runs, run_id, reason.clone()) {
        return Err(Stopped {
            failure,
            suspended: Some((Box::new(reason), None)),
        });
    }
    let outcome = RunOutcome::Suspended(RunOutcomeSuspended { reason });
    Ok(track.end(run_id, outcome))
}

/// Suspends the Run `run` for `reason` and holds the commission's Suspension, holding the Run and
/// `request`, through the port (`HoldSuspension`). The Suspension is not held when the Run could
/// not be suspended. A hold the host settled (`settled`) stores nothing: the Run is put back
/// `Running` ([`unsuspend`]) and the run ends as it would without a port.
fn suspend_for_approval<R: SuspendRunBehavior + ?Sized>(
    runs: &mut R,
    suspending: Suspensions<'_, R>,
    run: &RunStarted,
    request: &ActionRequestData,
    reason: SuspensionReason,
) -> Result<(), Stopped> {
    if let Err(failure) = suspend(runs, &run.run_id, reason.clone()) {
        return Err(Stopped {
            failure,
            suspended: Some((Box::new(reason), None)),
        });
    }
    let failure = match suspending.port.hold(HoldSuspension {
        suspension: SuspensionKey {
            commission_id: run.commission_id.clone(),
        },
        commission_id: run.commission_id.clone(),
        run_id: run.run_id.clone(),
        run_case_revision: run.case_revision,
        action_request_id: request.action_request_id.clone(),
        case_id: request.case_id.clone(),
        expected_case_revision: request.expected_case_revision,
        action: request.action.clone(),
        arguments: request.arguments.clone(),
        reason: reason.clone(),
    }) {
        HoldSuspensionOutcome::Held { .. } => return Ok(()),
        // Nothing is held, so nothing may leave the Run suspended: a resume finds no Suspension.
        HoldSuspensionOutcome::Settled { .. } => match (suspending.unsuspend)(runs, &run.run_id) {
            Ok(()) => return Ok(()),
            Err(failure) => failure,
        },
        HoldSuspensionOutcome::StorageFailed { error } => {
            LoopFailure::RunStorage(RunStorageFailed {
                reason: error.reason,
            })
        }
        other => LoopFailure::SuspensionNotHeld(Box::new(other)),
    };
    Err(Stopped {
        failure,
        suspended: Some((Box::new(reason), None)),
    })
}

/// Continues the Run `commission`'s stored Suspension holds, after the grant, as the module
/// documents: the Suspension is taken, the suspended request is performed once, the Suspension is
/// released, and the loop goes on until the Run ends.
///
/// `runs` need not hold the Run: a store of a new process gets it back, `Suspended`, from the
/// Suspension.
///
/// # Errors
///
/// [`ResumeError::Refused`] when there is nothing to resume, the request is in doubt or may not be
/// performed now, or another resume took the Suspension first or the commission holds one for
/// another request by the time it is taken; [`ResumeError::Store`] or
/// [`ResumeError::Governor`] when a port fails before the Run resumes: nothing is performed, and
/// the Suspension stays stored as it was. [`ResumeError::Loop`] when `ResumeRun` fails, the
/// Suspension cannot be taken (nothing is performed), or the resumed loop stops without a run
/// outcome.
// The ports of `run_until_blocked`, and the suspension port.
#[allow(clippy::too_many_arguments)]
pub fn resume_after_grant<G, E, A, F, R, C>(
    governor: &G,
    executor: &E,
    authority: &A,
    effects: &F,
    commission: &Commission<commission_state::Assigned>,
    runs: &mut R,
    suspensions: &dyn SuspensionPort,
    context: &mut C,
) -> Result<LoopEnd, ResumeError>
where
    G: Governor + ObservationPort + ?Sized,
    E: AgentExecutor + ?Sized,
    A: AuthorityProvider + ?Sized,
    F: EffectPort + ?Sized,
    R: RunStorage + SuspendRunBehavior + ResumeRunBehavior + ?Sized,
    C: LoopContext + ?Sized,
{
    let refused = |refusal| Err(ResumeError::Refused(refusal));
    let held = commission.data();
    let Some(stored) = suspensions
        .get(&held.commission_id)
        .map_err(ResumeError::Store)?
    else {
        return refused(ResumeRefusal::NothingSuspended);
    };
    if stored.state == SuspensionState::Resuming {
        return refused(ResumeRefusal::InDoubt {
            action_request_id: stored.data.action_request_id,
        });
    }
    let stored = stored.data;
    if stored.commission_id != held.commission_id
        || stored.suspension.commission_id != held.commission_id
        || stored.case_id != held.case_id
    {
        return refused(ResumeRefusal::OtherCommission);
    }
    let current = governor
        .current_revision(&held.case_id)
        .map_err(ResumeError::Governor)?;
    if current != stored.expected_case_revision {
        return refused(ResumeRefusal::CaseMoved {
            expected_case_revision: stored.expected_case_revision,
            current_case_revision: current,
        });
    }
    let request = ActionRequestData {
        action_request_id: stored.action_request_id.clone(),
        run_id: stored.run_id.clone(),
        case_id: stored.case_id.clone(),
        expected_case_revision: stored.expected_case_revision,
        action: stored.action.clone(),
        arguments: stored.arguments.clone(),
    };
    let answer = revalidate(governor, &ActionRequest::new(request.clone()))
        .map_err(ResumeError::Governor)?;
    match &answer {
        RevalidateActionRequestOutcome::Admitted => {}
        RevalidateActionRequestOutcome::Stale { error } => {
            return refused(ResumeRefusal::CaseMoved {
                expected_case_revision: error.expected_case_revision,
                current_case_revision: error.current_case_revision,
            });
        }
        RevalidateActionRequestOutcome::NotAdmitted { .. } => {
            return refused(ResumeRefusal::NotAdmitted(answer));
        }
        RevalidateActionRequestOutcome::NeedsAuthority { error } => {
            let check = check_authority(authority, commission, &request, &error.capability);
            if !check.allows() {
                return refused(ResumeRefusal::NotGranted(check));
            }
        }
    }
    if !effects.performs(&request.action) {
        return refused(ResumeRefusal::NotPerformed);
    }

    let run_id = stored.run_id.clone();
    match RunStorage::get(runs, &run_id) {
        None => {
            let run = Run::new(RunData {
                run_id: run_id.clone(),
                commission_id: stored.commission_id.clone(),
                case_revision: stored.run_case_revision,
            });
            RunStorage::put(runs, AnyRun::Suspended(run.suspend()).snapshot());
        }
        Some(held_run)
            if held_run.data.commission_id != stored.commission_id
                || held_run.data.case_revision != stored.run_case_revision =>
        {
            return refused(ResumeRefusal::OtherCommission);
        }
        Some(_) => {}
    }
    let failed = |failure: LoopFailure| LoopError {
        run_id: Some(run_id.clone()),
        failure,
        suspension: None,
        suspension_reason: None,
    };
    match runs
        .resume_run(ResumeRun {
            run_id: run_id.clone(),
        })
        .map_err(|unmet| ResumeError::Loop(failed(LoopFailure::Obligation(unmet))))?
    {
        ResumeRunOutcome::Resumed { .. } => {}
        ResumeRunOutcome::StorageFailed { error } => {
            return Err(ResumeError::Loop(failed(LoopFailure::RunStorage(error))));
        }
        other => return refused(ResumeRefusal::RunNotSuspended(other)),
    }
    // Taken before anything is performed, for the request read and revalidated above: of two
    // resumes one takes it, a Suspension a resume took and never released is never taken again,
    // and one held since for another request is not taken. Whatever the take answers but `taken`,
    // the Run is suspended again with its stored reason and nothing is performed.
    let key = stored.suspension.clone();
    let failure = match suspensions.take(TakeSuspension {
        suspension: key.clone(),
        action_request_id: stored.action_request_id.clone(),
    }) {
        TakeSuspensionOutcome::Taken { .. } => None,
        TakeSuspensionOutcome::OtherRequest { error } => Some(Err(ResumeRefusal::OtherRequest {
            action_request_id: error.action_request_id,
            held_action_request_id: error.held_action_request_id,
        })),
        TakeSuspensionOutcome::NotFound { .. } => Some(Err(ResumeRefusal::NothingSuspended)),
        TakeSuspensionOutcome::WrongState { error } => Some(Err(ResumeRefusal::InDoubt {
            action_request_id: error.action_request_id,
        })),
        TakeSuspensionOutcome::StorageFailed { error } => {
            Some(Ok(LoopFailure::RunStorage(RunStorageFailed {
                reason: error.reason,
            })))
        }
    };
    if let Some(failure) = failure {
        let suspension = suspend(runs, &run_id, stored.reason.clone()).err();
        return Err(match failure {
            Err(refusal) => ResumeError::Refused(refusal),
            Ok(failure) => ResumeError::Loop(LoopError {
                run_id: Some(run_id),
                failure,
                suspension: suspension.map(Box::new),
                suspension_reason: Some(Box::new(stored.reason)),
            }),
        });
    }

    let ports = Ports {
        governor,
        executor,
        authority,
        effects,
        suspensions: Some(Suspensions::new(suspensions)),
    };
    let run = RunStarted {
        run_id: run_id.clone(),
        commission_id: stored.commission_id,
        case_revision: stored.run_case_revision,
    };
    let result = iterate(
        &ports,
        commission,
        runs,
        context,
        &run,
        stored.expected_case_revision,
        Some(Resumed {
            request,
            answer,
            taken: key,
        }),
    );
    finish(runs, run_id, result).map_err(ResumeError::Loop)
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

/// Whether a fan-out of `reads` is admitted (item 8a): it lists at least one and at most
/// [`MAX_FAN_OUT`] reads, no two with the same action and arguments, and each names an action the
/// effect port performs and reads ([`EffectPort::reads`]) that the `handed` frontier lists, every
/// entry of it `Admissible` and naming no capability.
fn fans_out<F: EffectPort + ?Sized>(
    reads: &[ExecutorOutcomeProposedAction],
    handed: &Frontier<frontier_state::Issued>,
    effects: &F,
) -> bool {
    let admissible = |action: &str| {
        let mut listed = handed
            .data()
            .actions
            .iter()
            .filter(|entry| entry.action == action)
            .peekable();
        listed.peek().is_some()
            && listed.all(|entry| {
                entry.status == ActionStatus::Admissible
                    && entry.capability.as_deref().is_none_or(str::is_empty)
            })
    };
    // Two reads with the same action and arguments would invoke one read twice in one step.
    let distinct = reads.iter().enumerate().all(|(at, read)| {
        reads[..at]
            .iter()
            .all(|earlier| earlier.action != read.action || earlier.arguments != read.arguments)
    });
    (1..=MAX_FAN_OUT).contains(&reads.len())
        && distinct
        && reads.iter().all(|read| {
            effects.performs(&read.action)
                && effects.reads(&read.action)
                && admissible(&read.action)
        })
}

/// What one admitted fan-out did.
struct FannedOut {
    /// Whether any read was admitted, and so invoked.
    admitted: bool,
    /// Whether any admitted read has the action and arguments of no request admitted before it.
    progressed: bool,
    /// Whether any effect was `Performed`.
    performed: bool,
    /// Whether some revalidation answered stale: the case moved under somebody else.
    stale: bool,
    /// The trial limit the fan-out would have passed, so none of its reads was invoked.
    over_budget: Option<RunBudgetBreach>,
}

/// Performs the fan-out `reads`, which [`fans_out`] admitted, as item 8a documents: each read
/// becomes its own request at `frontier`'s revision and is revalidated, all before any is invoked;
/// the admitted ones are handed to [`EffectPort::invoke_reads`] together, and their effects are
/// recorded and observed in the order proposed. An `Err` from a read fails the loop once every
/// read has answered and each that answered was recorded and observed; the first `Err` in proposal
/// order is the failure. Nothing is retried, and no authority is asked.
// The ports, the loop's state and the proposal.
#[allow(clippy::too_many_arguments)]
fn fan_out<G, F, C>(
    governor: &G,
    effects: &F,
    commission: &Commission<commission_state::Assigned>,
    context: &mut C,
    run_id: &RunId,
    frontier: &Frontier<frontier_state::Issued>,
    reads: &[ExecutorOutcomeProposedAction],
    budget: Option<&RunBudget>,
    track: &mut Track,
) -> Result<FannedOut, Stopped>
where
    G: Governor + ObservationPort + ?Sized,
    F: EffectPort + ?Sized,
    C: LoopContext + ?Sized,
{
    let mut admitted: Vec<ActionRequestData> = Vec::new();
    let mut stale = false;
    for read in reads {
        let made = request(
            context.action_request_id(),
            run_id.clone(),
            frontier,
            read.clone(),
        );
        let answer = revalidate(governor, &made)?;
        let data = made.into_data();
        track.requests.push(Revalidated {
            request: data.clone(),
            outcome: answer.clone(),
        });
        match answer {
            RevalidateActionRequestOutcome::Admitted => admitted.push(data),
            RevalidateActionRequestOutcome::Stale { .. } => stale = true,
            // A read that needs authority is never performed in a fan-out.
            RevalidateActionRequestOutcome::NeedsAuthority { .. }
            | RevalidateActionRequestOutcome::NotAdmitted { .. } => {}
        }
    }
    if admitted.is_empty() {
        return Ok(FannedOut {
            admitted: false,
            progressed: false,
            performed: false,
            stale,
            over_budget: None,
        });
    }
    // A fan-out that would take an action past its trial limit invokes none of its reads.
    let actions: Vec<&str> = admitted.iter().map(|data| data.action.as_str()).collect();
    if let Some(breach) = budget.and_then(|limits| trials_past(limits, track, &actions)) {
        return Ok(FannedOut {
            admitted: false,
            progressed: false,
            performed: false,
            stale,
            over_budget: Some(breach),
        });
    }
    let progressed = admitted.iter().any(|data| {
        !track
            .admitted
            .iter()
            .any(|earlier| earlier.action == data.action && earlier.arguments == data.arguments)
    });
    track.admitted.extend(admitted.iter().cloned());
    let requests: Vec<AdmittedRequest> =
        admitted.iter().cloned().map(AdmittedRequest::new).collect();
    let answers = match effects.invoke_reads(commission, &requests) {
        Some(answers) if answers.len() == requests.len() => answers,
        Some(answers) => {
            return Err(LoopFailure::Effect(EffectError::new(format!(
                "the effect port answered {} of {} reads",
                answers.len(),
                requests.len()
            )))
            .into());
        }
        None => {
            return Err(LoopFailure::Effect(EffectError::new(
                "the effect port reads the actions proposed but invokes no fan-out",
            ))
            .into());
        }
    };
    let mut performed = false;
    let mut failed: Option<EffectError> = None;
    for (data, answer) in admitted.iter().zip(answers) {
        match answer {
            Ok(effect) => {
                performed |= matches!(effect, EffectOutcome::Performed(_));
                track.record(data, &effect);
                governor.observe(effect_observation(context, data, &effect))?;
            }
            Err(error) => {
                failed.get_or_insert(error);
            }
        }
    }
    if let Some(error) = failed {
        return Err(LoopFailure::Effect(error).into());
    }
    Ok(FannedOut {
        admitted: true,
        progressed,
        performed,
        stale,
        over_budget: None,
    })
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
        ExecutorOutcome::ProposedReads(proposed) => vec![
            ("outcome".to_owned(), text("ProposedReads")),
            (
                "reads".to_owned(),
                Value::Array(
                    proposed
                        .reads
                        .iter()
                        .map(|read| {
                            Value::Object(vec![
                                ("action".to_owned(), text(&read.action)),
                                ("arguments".to_owned(), read.arguments.0.clone()),
                            ])
                        })
                        .collect(),
                ),
            ),
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
            if let Some(usage) = &performed.usage {
                let figure = |figure: i64| Value::Number(figure.to_string());
                let mut spent = vec![
                    ("input_tokens".to_owned(), figure(usage.input_tokens)),
                    ("output_tokens".to_owned(), figure(usage.output_tokens)),
                    ("duration_ms".to_owned(), figure(usage.duration_ms)),
                ];
                if let Some(cost) = usage.cost_micro_usd {
                    spent.push(("cost_micro_usd".to_owned(), figure(cost)));
                }
                payload.push(("usage".to_owned(), Value::Object(spent)));
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

/// Nothing charged: no trial, no usage, and a known cost of zero.
fn uncharged() -> ChargedUsage {
    ChargedUsage {
        trials: 0,
        input_tokens: 0,
        output_tokens: 0,
        duration_ms: 0,
        cost_micro_usd: Some(0),
    }
}

/// Charges `effect`, the port's answer for a request for `action`, to the run and to `action`:
/// one trial, and the usage a `Performed` effect reported. A negative figure charges nothing, and
/// a usage that reports no cost leaves the cost unknown from then on.
fn charge(charges: &mut RunCharges, action: &str, effect: &EffectOutcome) {
    let usage = match effect {
        EffectOutcome::Performed(performed) => performed.usage.as_ref(),
        EffectOutcome::Refused(_) => None,
    };
    let index = if let Some(index) = charges.actions.iter().position(|c| c.action == action) {
        index
    } else {
        charges.actions.push(ActionCharge {
            action: action.to_owned(),
            charged: uncharged(),
        });
        charges.actions.len() - 1
    };
    for charged in [&mut charges.run, &mut charges.actions[index].charged] {
        charged.trials = charged.trials.saturating_add(1);
        if let Some(usage) = usage {
            charged.input_tokens = charged
                .input_tokens
                .saturating_add(usage.input_tokens.max(0));
            charged.output_tokens = charged
                .output_tokens
                .saturating_add(usage.output_tokens.max(0));
            charged.duration_ms = charged.duration_ms.saturating_add(usage.duration_ms.max(0));
            charged.cost_micro_usd = match (charged.cost_micro_usd, usage.cost_micro_usd) {
                (Some(sum), Some(cost)) => Some(sum.saturating_add(cost.max(0))),
                _ => None,
            };
        }
    }
}

/// The first spending limit of `limits` that `charged` passed, for `action` (`None` for the run's):
/// a charge above its limit, or an unknown cost under a cost limit.
fn spending(
    action: Option<&str>,
    limits: [(BudgetLimit, Option<i64>, Option<i64>); 4],
) -> Option<RunBudgetBreach> {
    limits.into_iter().find_map(|(limit, max, charged)| {
        let max = max?;
        match charged {
            Some(charged) if charged <= max => None,
            charged => Some(RunBudgetBreach {
                action: action.map(str::to_owned),
                limit,
                max,
                charged,
            }),
        }
    })
}

/// The spending limits of `charged` under the four limits given.
fn spent(
    tokens_in: Option<i64>,
    tokens_out: Option<i64>,
    duration_ms: Option<i64>,
    cost: Option<i64>,
    charged: &ChargedUsage,
) -> [(BudgetLimit, Option<i64>, Option<i64>); 4] {
    [
        (
            BudgetLimit::MaxTokensIn,
            tokens_in,
            Some(charged.input_tokens),
        ),
        (
            BudgetLimit::MaxTokensOut,
            tokens_out,
            Some(charged.output_tokens),
        ),
        (
            BudgetLimit::MaxDurationMs,
            duration_ms,
            Some(charged.duration_ms),
        ),
        (BudgetLimit::MaxCostMicroUsd, cost, charged.cost_micro_usd),
    ]
}

/// The first spending limit of `budget` the charges of `track` passed: an action's, in the order
/// the budget lists them, then the run's.
fn spent_past(budget: &RunBudget, track: &Track) -> Option<RunBudgetBreach> {
    let charges = &track.charges;
    budget
        .per_action
        .iter()
        .find_map(|limits| {
            let charged = &charges
                .actions
                .iter()
                .find(|charge| charge.action == limits.action)?
                .charged;
            spending(
                Some(&limits.action),
                spent(
                    limits.max_tokens_in,
                    limits.max_tokens_out,
                    limits.max_duration_ms,
                    limits.max_cost_micro_usd,
                    charged,
                ),
            )
        })
        .or_else(|| {
            spending(
                None,
                spent(
                    budget.max_tokens_in,
                    budget.max_tokens_out,
                    budget.max_duration_ms,
                    budget.max_cost_micro_usd,
                    &charges.run,
                ),
            )
        })
}

/// The trial limit of `budget` that handing requests for `actions` to the effect port would pass,
/// naming the trials already charged to its action.
fn trials_past(budget: &RunBudget, track: &Track, actions: &[&str]) -> Option<RunBudgetBreach> {
    budget.per_action.iter().find_map(|limits| {
        let max = limits.max_trials?;
        let more = actions
            .iter()
            .filter(|action| **action == limits.action)
            .count();
        if more == 0 {
            return None;
        }
        let charged = track
            .charges
            .actions
            .iter()
            .find(|charge| charge.action == limits.action)
            .map_or(0, |charge| charge.charged.trials);
        (charged.saturating_add(i64::try_from(more).unwrap_or(i64::MAX)) > max).then(|| {
            RunBudgetBreach {
                action: Some(limits.action.clone()),
                limit: BudgetLimit::MaxTrials,
                max,
                charged: Some(charged),
            }
        })
    })
}
