//! Adversary cases for the wave's runtime surfaces: the stop policy's approval gate and idle bound
//! (`story:stop-policy-gate-and-idle-bound`), the run budget's `max_tokens`, its RFC 3339 deadline
//! and `on_breach` (`story:run-budget-tokens-and-deadline`), and a pending answer that ends the run
//! (`story:pending-answer-ends-run`).
//!
//! Each case drives `run_until_blocked`, `run_with_suspension` or `resume_after_grant` through
//! ports written here: a governor whose case can move while the executor works, an executor that
//! reports scripted usage, an effect port answering per action, and a context whose clock reads
//! a fixed instant or one past the executor's first call.

use std::collections::{BTreeMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::{Generated, RunStorage};
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext,
    AuthorityVerdict, AuthorityVerdictApprovalRequired, BreachStep, BudgetLimit, CaseId,
    Commission, CommissionData, CommissionId, CompletionDetermination, EffectOutcome,
    EffectOutcomePending, EffectOutcomePerformed, EffectOutcomeRefused, EffectUsage,
    ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorOutcomeProposedReads, ExecutorStep,
    ExecutorUsage, Frontier, FrontierAction, FrontierData, FrontierId, GovernorError,
    HoldSuspension, HoldSuspensionOutcome, Observation, ObservationId, PrincipalId,
    ProposedActionArguments, RunBudget, RunBudgetBreach, RunId, RunOutcome, RunOutcomeIdleBound,
    RunOutcomeNeedsAuthority, RunOutcomeSuspended, RunOutcomeWaiting, RunState, StopPolicy,
    SuspensionKey, SuspensionReason, SuspensionState, Unit, commission_state, frontier_state,
    observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use b10x_loom_commission::ports::effect::{
    AdmittedRequest, EffectError, EffectPort, invoke_concurrently,
};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::ports::suspension::{SuspensionPort, SuspensionStore};
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, LoopError, LoopFailure, ResumeError, ResumeRefusal, resume_after_grant,
    run_until_blocked, run_with_suspension,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::file_suspension::FileSuspensionStore;

/// The trusted time the clock reads unless a case says otherwise.
const NOW: &str = "2026-10-21T09:00:00Z";
/// `NOW` in whole seconds since the Unix epoch.
const NOW_SECONDS: i64 = 1_792_573_200;

const EVALUATE: &str = "evaluate";
const MERGE: &str = "repository.merge";
const CAPABILITY: &str = "repository.merge";
const APPROVE: &str = "approve the merge";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn request_id(n: u64) -> ActionRequestId {
    ActionRequestId(uuid(0x500 + n))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x71)),
        agent_revision_id: AgentRevisionId(uuid(0x72)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adversary".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn admissible(name: &str) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status: ActionStatus::Admissible,
        capability: None,
        reasons: Vec::new(),
    }
}

fn needs_approval(name: &str, capability: &str) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status: ActionStatus::ApprovalRequired,
        capability: Some(capability.to_owned()),
        reasons: Vec::new(),
    }
}

/// A governor holding one open case at a revision that the executor may move, listing `actions`
/// on every frontier, recording each observation.
struct Case {
    id: CaseId,
    revision: AtomicI64,
    actions: Vec<FrontierAction>,
    issued: AtomicU64,
    observed: AtomicUsize,
    /// The frontier after whose issue somebody else moves the case; 0 is none.
    move_after_frontier: u64,
}

impl Case {
    fn new(name: &str, actions: Vec<FrontierAction>) -> Self {
        Self {
            id: CaseId(name.to_owned()),
            revision: AtomicI64::new(1),
            actions,
            issued: AtomicU64::new(0),
            observed: AtomicUsize::new(0),
            move_after_frontier: 0,
        }
    }

    /// Somebody else moves the case right after the governor issued its `n`th frontier.
    fn moving_after_frontier(mut self, n: u64) -> Self {
        self.move_after_frontier = n;
        self
    }

    fn known(&self, case: &CaseId) -> Result<(), GovernorError> {
        if *case == self.id {
            Ok(())
        } else {
            Err(GovernorError::UnknownCase)
        }
    }

    /// Somebody else moves the case to its next revision.
    fn move_on(&self) {
        self.revision.fetch_add(1, Ordering::SeqCst);
    }
}

impl Governor for Case {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.known(case)?;
        Ok(self.revision.load(Ordering::SeqCst))
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        self.known(case)?;
        let issued = self.issued.fetch_add(1, Ordering::SeqCst) + 1;
        let frontier = Frontier::new(FrontierData {
            frontier_id: FrontierId(uuid(0x900 + issued)),
            case_id: case.clone(),
            case_revision: self.revision.load(Ordering::SeqCst),
            claims: Vec::new(),
            obligations: Vec::new(),
            actions: self.actions.clone(),
        });
        if issued == self.move_after_frontier {
            self.move_on();
        }
        Ok(frontier)
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.known(case)?;
        Ok(CompletionDetermination::Open(Unit(true)))
    }
}

impl ObservationPort for Case {
    fn observe(
        &self,
        _observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        self.observed.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// An executor answering each call with the next scripted outcome and usage, then proposing
/// nothing and reporting nothing; it moves `moves` on each call when given one.
struct Steps<'a> {
    script: Mutex<VecDeque<(ExecutorOutcome, Option<ExecutorUsage>)>>,
    calls: Arc<AtomicUsize>,
    moves: Option<&'a Case>,
}

impl<'a> Steps<'a> {
    fn new(script: impl IntoIterator<Item = (ExecutorOutcome, Option<ExecutorUsage>)>) -> Self {
        Self {
            script: Mutex::new(script.into_iter().collect()),
            calls: Arc::new(AtomicUsize::new(0)),
            moves: None,
        }
    }

    /// Proposes `evaluate` `n` times, each with its own arguments, each call reporting `usage`.
    fn evaluations(n: u64, usage: Option<ExecutorUsage>) -> Self {
        Self::new((1..=n).map(|call| (evaluate(call), usage.clone())))
    }

    fn moving(mut self, case: &'a Case) -> Self {
        self.moves = Some(case);
        self
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl AgentExecutor for Steps<'_> {
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.run_with_usage(commission, frontier).outcome
    }

    fn run_with_usage(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorStep {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(case) = self.moves {
            case.move_on();
        }
        let (outcome, usage) = self
            .script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or((ExecutorOutcome::NoUsefulAction(Unit(true)), None));
        ExecutorStep {
            outcome,
            usage,
            transcript_digest: None,
            summary: None,
        }
    }
}

fn proposed(action: &str, arguments: Value) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: ProposedActionArguments(arguments),
    }
}

fn evaluate(call: u64) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(proposed(EVALUATE, Value::Number(call.to_string())))
}

fn reads(actions: &[&str]) -> ExecutorOutcome {
    ExecutorOutcome::ProposedReads(ExecutorOutcomeProposedReads {
        reads: actions
            .iter()
            .map(|action| proposed(action, Value::Null))
            .collect(),
    })
}

fn idle() -> (ExecutorOutcome, Option<ExecutorUsage>) {
    (ExecutorOutcome::NoUsefulAction(Unit(true)), None)
}

fn tokens(input_tokens: i64, output_tokens: i64) -> ExecutorUsage {
    ExecutorUsage {
        input_tokens,
        output_tokens,
        duration_ms: 1,
        cost_micro_usd: Some(1),
    }
}

/// What the effect port answers an invocation of an action.
#[derive(Debug, Clone)]
enum Reply {
    Performed(Option<EffectUsage>),
    Pending {
        ends_run: bool,
        report: &'static str,
    },
    Refused {
        ends_run: bool,
    },
}

/// An effect port answering each action from its own script (the last answer repeating; an action
/// with none is performed), reading every action when `reads` is set, and recording what it was
/// handed.
#[derive(Default)]
struct Effects {
    reads: bool,
    replies: Mutex<BTreeMap<String, VecDeque<Reply>>>,
    invoked: Mutex<Vec<ActionRequestData>>,
    invocations: Arc<AtomicUsize>,
}

impl Effects {
    fn reading() -> Self {
        Self {
            reads: true,
            ..Self::default()
        }
    }

    fn answering(self, action: &str, replies: impl IntoIterator<Item = Reply>) -> Self {
        self.replies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(action.to_owned(), replies.into_iter().collect());
        self
    }

    fn invoked(&self) -> Vec<ActionRequestData> {
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn invoked_of(&self, action: &str) -> usize {
        self.invoked()
            .iter()
            .filter(|data| data.action == action)
            .count()
    }

    fn reply(&self, action: &str) -> Reply {
        let mut replies = self.replies.lock().unwrap_or_else(PoisonError::into_inner);
        match replies.get_mut(action) {
            Some(script) if script.len() > 1 => {
                script.pop_front().unwrap_or(Reply::Performed(None))
            }
            Some(script) => script.front().cloned().unwrap_or(Reply::Performed(None)),
            None => Reply::Performed(None),
        }
    }
}

impl EffectPort for Effects {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn reads(&self, _action: &str) -> bool {
        self.reads
    }

    fn invoke_reads(
        &self,
        commission: &Commission<commission_state::Assigned>,
        requests: &[AdmittedRequest],
    ) -> Option<Vec<Result<EffectOutcome, EffectError>>> {
        Some(invoke_concurrently(self, commission, requests))
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let data = request.data().clone();
        let reply = self.reply(&data.action);
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(data);
        self.invocations.fetch_add(1, Ordering::SeqCst);
        Ok(match reply {
            Reply::Performed(usage) => EffectOutcome::Performed(EffectOutcomePerformed {
                report: Value::Text("done".to_owned()),
                attempt: None,
                audit: None,
                usage,
            }),
            Reply::Pending { ends_run, report } => EffectOutcome::Pending(EffectOutcomePending {
                report: Value::Text(report.to_owned()),
                ends_run,
            }),
            Reply::Refused { ends_run } => EffectOutcome::Refused(EffectOutcomeRefused {
                reason: "nothing changed".to_owned(),
                ends_run,
            }),
        })
    }
}

/// An authority provider answering each question with the next verdict, the last repeating.
struct Sequenced {
    verdicts: Mutex<VecDeque<AuthorityVerdict>>,
    asked: AtomicUsize,
}

impl Sequenced {
    fn new(verdicts: impl IntoIterator<Item = AuthorityVerdict>) -> Self {
        Self {
            verdicts: Mutex::new(verdicts.into_iter().collect()),
            asked: AtomicUsize::new(0),
        }
    }
}

impl AuthorityProvider for Sequenced {
    fn decide(
        &self,
        _commission: &CommissionData,
        _request: &ActionRequestData,
        _capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        let mut verdicts = self.verdicts.lock().unwrap_or_else(PoisonError::into_inner);
        let verdict = if verdicts.len() > 1 {
            verdicts.pop_front()
        } else {
            verdicts.front().cloned()
        };
        verdict.ok_or_else(|| AuthorityProviderError::new("no verdict"))
    }
}

/// An authority provider that panics whenever it is asked.
struct Panicking;

impl AuthorityProvider for Panicking {
    fn decide(
        &self,
        _commission: &CommissionData,
        _request: &ActionRequestData,
        _capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        panic!("the provider panics");
    }
}

fn allow() -> AuthorityVerdict {
    AuthorityVerdict::Allow(Unit(true))
}

fn withheld() -> AuthorityVerdict {
    AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
        request: APPROVE.to_owned(),
    })
}

fn policy(approval_gate: bool, idle_bound: Option<i64>, on_breach: BreachStep) -> StopPolicy {
    StopPolicy {
        approval_gate,
        idle_bound,
        on_breach,
    }
}

fn gate_off() -> StopPolicy {
    policy(false, Some(1), BreachStep::DiscardStep)
}

fn default_policy() -> StopPolicy {
    policy(true, None, BreachStep::DiscardStep)
}

fn keep_step() -> StopPolicy {
    policy(true, None, BreachStep::KeepStep)
}

/// What the context's trusted clock reads.
enum Clock {
    /// One reading, always.
    Fixed(String),
    /// `before` until `calls` counted one call, `after` from then on.
    AfterFirstCall {
        calls: Arc<AtomicUsize>,
        before: String,
        after: String,
    },
}

/// New ids from counters, a clock, a step budget, a run budget and a stop policy.
struct Context {
    requests: u64,
    observations: u64,
    steps: Option<usize>,
    budget: Option<RunBudget>,
    policy: StopPolicy,
    clock: Clock,
}

impl Context {
    fn new(budget: Option<RunBudget>, policy: StopPolicy) -> Self {
        Self {
            requests: 0,
            observations: 0,
            steps: None,
            budget,
            policy,
            clock: Clock::Fixed(NOW.to_owned()),
        }
    }

    fn reading(mut self, reading: &str) -> Self {
        self.clock = Clock::Fixed(reading.to_owned());
        self
    }

    /// `before` until the effect port was handed a request once, `after` from then on.
    fn past_first_invoke(mut self, effects: &Effects, before: &str, after: &str) -> Self {
        self.clock = Clock::AfterFirstCall {
            calls: Arc::clone(&effects.invocations),
            before: before.to_owned(),
            after: after.to_owned(),
        };
        self
    }

    fn steps(mut self, steps: usize) -> Self {
        self.steps = Some(steps);
        self
    }
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        request_id(self.requests)
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x600 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(match &self.clock {
            Clock::Fixed(reading) => reading.clone(),
            Clock::AfterFirstCall {
                calls,
                before,
                after,
            } => {
                if calls.load(Ordering::SeqCst) == 0 {
                    before.clone()
                } else {
                    after.clone()
                }
            }
        })
    }

    fn step_budget(&self) -> Option<usize> {
        self.steps
    }

    fn run_budget(&self) -> Option<RunBudget> {
        self.budget.clone()
    }

    fn stop_policy(&self) -> StopPolicy {
        self.policy.clone()
    }
}

fn unlimited() -> RunBudget {
    RunBudget {
        max_steps: None,
        max_tokens_in: None,
        max_tokens_out: None,
        max_duration_ms: None,
        max_cost_micro_usd: None,
        max_tokens: None,
        deadline: None,
        per_action: Vec::new(),
    }
}

fn max_tokens(max: i64) -> Option<RunBudget> {
    Some(RunBudget {
        max_tokens: Some(max),
        ..unlimited()
    })
}

fn deadline(at: &str) -> Option<RunBudget> {
    Some(RunBudget {
        deadline: Some(Timestamp(at.to_owned())),
        ..unlimited()
    })
}

fn runs() -> RunStore {
    let mut issued = 0;
    RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    })
}

/// `run_until_blocked` on `case`, returning the loop's answer and the run store.
fn run<A: AuthorityProvider>(
    case: &Case,
    executor: &Steps<'_>,
    authority: &A,
    effects: &Effects,
    context: &mut Context,
) -> (Result<LoopEnd, LoopError>, Generated<RunStore>) {
    let mut store = Generated::new(runs());
    let end = run_until_blocked(
        case,
        executor,
        authority,
        effects,
        &commission(&case.id),
        &mut store,
        context,
    );
    (end, store)
}

fn ended(end: Result<LoopEnd, LoopError>) -> LoopEnd {
    end.unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

fn run_state(store: &Generated<RunStore>, end: &LoopEnd) -> Option<RunState> {
    RunStorage::get(&store.ports, &end.run_id).map(|held| held.state)
}

fn breach(limit: BudgetLimit, max: i64, charged: Option<i64>) -> SuspensionReason {
    SuspensionReason::RunBudget(RunBudgetBreach {
        action: None,
        limit,
        max,
        charged,
    })
}

fn suspended(reason: SuspensionReason) -> RunOutcome {
    RunOutcome::Suspended(RunOutcomeSuspended { reason })
}

fn deadline_passed(max: i64, charged: Option<i64>) -> RunOutcome {
    suspended(breach(BudgetLimit::Deadline, max, charged))
}

fn needs_authority() -> RunOutcome {
    RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
        request: APPROVE.to_owned(),
    })
}

/// A suspension store holding `commission`'s Suspension of an `action` request at revision 1, the
/// request `0x301` of the Run `0x101`.
fn holding(commission: &Commission<commission_state::Assigned>, action: &str) -> SuspensionStore {
    let suspensions = SuspensionStore::new();
    let commission_id = commission.data().commission_id.clone();
    let held = SuspensionPort::hold(
        &suspensions,
        HoldSuspension {
            suspension: SuspensionKey {
                commission_id: commission_id.clone(),
            },
            commission_id,
            run_id: RunId(uuid(0x101)),
            run_case_revision: 1,
            action_request_id: ActionRequestId(uuid(0x301)),
            case_id: commission.data().case_id.clone(),
            expected_case_revision: 1,
            action: action.to_owned(),
            arguments: ProposedActionArguments(Value::Null),
            reason: SuspensionReason::Authority(Value::Null),
        },
    );
    assert!(matches!(held, HoldSuspensionOutcome::Held { .. }));
    suspensions
}

fn stored_state(
    suspensions: &SuspensionStore,
    commission: &Commission<commission_state::Assigned>,
) -> Option<SuspensionState> {
    SuspensionPort::get(suspensions, &commission.data().commission_id)
        .unwrap_or_else(|error| panic!("the store reads: {error:?}"))
        .map(|held| held.state)
}

// --- 1. The approval gate off -----------------------------------------------------------------

/// With the gate off, a fan-out naming a read the frontier lists as needing approval invokes none
/// of its reads, though the provider would allow it: a read of a fan-out is never performed on an
/// authority, and the provider is not even asked.
#[test]
fn adversary_gate_off_a_fan_out_naming_an_approval_required_read_invokes_nothing() {
    let case = Case::new(
        "case-gate-off-fan-out",
        vec![
            admissible("source.read"),
            needs_approval("secret.read", "secret.read"),
        ],
    );
    let executor = Steps::new([(reads(&["source.read", "secret.read"]), None)]);
    let authority = StaticAuthorityProvider::new().answer("secret.read", allow());
    let effects = Effects::reading();

    let (end, _) = run(
        &case,
        &executor,
        &authority,
        &effects,
        &mut Context::new(None, gate_off()),
    );
    let end = ended(end);

    assert!(
        effects.invoked().is_empty(),
        "no read of the fan-out is invoked: {:?}, ended {:?}",
        effects.invoked(),
        end.outcome
    );
    assert!(authority.requests().is_empty(), "no authority is asked");
    assert!(end.admitted.is_empty());
}

/// With the gate off, a provider that fails to decide grants nothing: the proposal needing approval
/// is never handed to the effect port.
#[test]
fn adversary_gate_off_a_provider_that_fails_performs_nothing() {
    let case = Case::new(
        "case-gate-off-failing",
        vec![needs_approval(MERGE, CAPABILITY)],
    );
    let executor = Steps::new([(
        ExecutorOutcome::ProposedAction(proposed(MERGE, Value::Null)),
        None,
    )]);
    let authority = StaticAuthorityProvider::new().fail(CAPABILITY, "the provider is down");
    let effects = Effects::default();

    let (end, _) = run(
        &case,
        &executor,
        &authority,
        &effects,
        &mut Context::new(None, gate_off()),
    );
    let end = ended(end);

    assert_eq!(authority.requests().len(), 1, "the provider is asked");
    assert!(effects.invoked().is_empty(), "ended {:?}", end.outcome);
    assert!(end.admitted.is_empty());
}

/// With the gate off, a provider that panics while it decides performs nothing before the panic
/// leaves the loop.
#[test]
fn adversary_gate_off_a_provider_that_panics_performs_nothing() {
    let case = Case::new(
        "case-gate-off-panicking",
        vec![needs_approval(MERGE, CAPABILITY)],
    );
    let executor = Steps::new([(
        ExecutorOutcome::ProposedAction(proposed(MERGE, Value::Null)),
        None,
    )]);
    let effects = Effects::default();

    let unwound = catch_unwind(AssertUnwindSafe(|| {
        run(
            &case,
            &executor,
            &Panicking,
            &effects,
            &mut Context::new(None, gate_off()),
        )
        .0
        .is_ok()
    }));

    assert!(unwound.is_err(), "the provider's panic leaves the loop");
    assert!(effects.invoked().is_empty(), "nothing was invoked first");
}

/// With the gate off, a request the provider allowed and the port answered `Pending` is asked
/// about again before it is invoked again; withheld then, it is not invoked again, the run ends
/// needing authority, and with a suspension port the Suspension holds it, `Pending`.
#[test]
fn adversary_gate_off_a_pending_request_withheld_when_asked_again_is_not_invoked_again() {
    let case = Case::new(
        "case-gate-off-pending",
        vec![needs_approval(MERGE, CAPABILITY)],
    );
    let executor = Steps::new([(
        ExecutorOutcome::ProposedAction(proposed(MERGE, Value::Null)),
        None,
    )]);
    let authority = Sequenced::new([allow(), withheld()]);
    let effects = Effects::default().answering(
        MERGE,
        [Reply::Pending {
            ends_run: false,
            report: "merging",
        }],
    );
    let suspensions = SuspensionStore::new();
    let held_commission = commission(&case.id);

    let end = run_with_suspension(
        &case,
        &executor,
        &authority,
        &effects,
        &held_commission,
        &mut runs(),
        &suspensions,
        &mut Context::new(None, gate_off()),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    assert_eq!(end.outcome, needs_authority());
    assert_eq!(
        effects.invoked_of(MERGE),
        1,
        "not invoked again once withheld"
    );
    assert_eq!(authority.asked.load(Ordering::SeqCst), 2, "asked again");
    assert_eq!(executor.calls(), 1);
    assert_eq!(
        stored_state(&suspensions, &held_commission),
        Some(SuspensionState::Pending)
    );
}

/// With the gate off, a resume whose request the provider withholds, or for which it panics, takes
/// nothing and performs nothing: the Suspension stays `Pending`.
#[test]
fn adversary_gate_off_a_resume_the_provider_does_not_allow_performs_nothing() {
    let case = Case::new(
        "case-gate-off-resume",
        vec![needs_approval(MERGE, CAPABILITY)],
    );
    let held_commission = commission(&case.id);
    let suspensions = holding(&held_commission, MERGE);
    let executor = Steps::new([]);
    let effects = Effects::default();
    let authority = StaticAuthorityProvider::new().answer(CAPABILITY, withheld());

    let resumed = resume_after_grant(
        &case,
        &executor,
        &authority,
        &effects,
        &held_commission,
        &mut runs(),
        &suspensions,
        &mut Context::new(None, gate_off()),
    );

    assert!(
        matches!(
            resumed,
            Err(ResumeError::Refused(ResumeRefusal::NotGranted(_)))
        ),
        "{resumed:?}"
    );
    assert!(effects.invoked().is_empty());
    assert_eq!(executor.calls(), 0);
    assert_eq!(
        stored_state(&suspensions, &held_commission),
        Some(SuspensionState::Pending)
    );

    let unwound = catch_unwind(AssertUnwindSafe(|| {
        resume_after_grant(
            &case,
            &executor,
            &Panicking,
            &effects,
            &held_commission,
            &mut runs(),
            &suspensions,
            &mut Context::new(None, gate_off()),
        )
        .is_ok()
    }));
    assert!(unwound.is_err(), "the provider's panic leaves the resume");
    assert!(effects.invoked().is_empty());
    assert_eq!(
        stored_state(&suspensions, &held_commission),
        Some(SuspensionState::Pending),
        "nothing was taken before the provider decided"
    );
}

/// With the gate off and a step kept past a spending limit, a proposal the provider withholds is
/// never handed to the effect port: the run ends needing authority.
#[test]
fn adversary_gate_off_a_kept_step_withheld_needs_authority_and_performs_nothing() {
    let case = Case::new(
        "case-gate-off-kept",
        vec![needs_approval(MERGE, CAPABILITY)],
    );
    let executor = Steps::new([(
        ExecutorOutcome::ProposedAction(proposed(MERGE, Value::Null)),
        Some(tokens(60, 50)),
    )]);
    let authority = StaticAuthorityProvider::new().answer(CAPABILITY, withheld());
    let effects = Effects::default();

    let (end, _) = run(
        &case,
        &executor,
        &authority,
        &effects,
        &mut Context::new(max_tokens(100), policy(false, None, BreachStep::KeepStep)),
    );
    let end = ended(end);

    assert_eq!(end.outcome, needs_authority());
    assert!(effects.invoked().is_empty());
    assert_eq!(executor.calls(), 1);
}

// --- 2. The deadline ----------------------------------------------------------------------------

/// A deadline equal to the clock's reading is reached: the executor is never called.
#[test]
fn adversary_a_deadline_equal_to_the_reading_is_reached() {
    let case = Case::new("case-deadline-equal", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(1, None);
    let effects = Effects::default();

    let (end, store) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut Context::new(deadline(NOW), default_policy()),
    );
    let end = ended(end);

    assert_eq!(end.outcome, deadline_passed(NOW_SECONDS, Some(NOW_SECONDS)));
    assert_eq!(executor.calls(), 0);
    assert!(effects.invoked().is_empty());
    assert_eq!(run_state(&store, &end), Some(RunState::Suspended));
}

/// A deadline one second before the reading is reached; one second after it is not, and the run
/// goes on to its idle bound.
#[test]
fn adversary_a_deadline_one_second_either_side_of_the_reading() {
    let case = Case::new("case-deadline-before", vec![admissible(EVALUATE)]);
    let executor = Steps::new([idle()]);
    let (end, _) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &Effects::default(),
        &mut Context::new(deadline("2026-10-21T08:59:59Z"), default_policy()),
    );
    assert_eq!(
        ended(end).outcome,
        deadline_passed(NOW_SECONDS - 1, Some(NOW_SECONDS))
    );
    assert_eq!(executor.calls(), 0);

    let case = Case::new("case-deadline-after", vec![admissible(EVALUATE)]);
    let executor = Steps::new([idle(), idle()]);
    let (end, _) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &Effects::default(),
        &mut Context::new(deadline("2026-10-21T09:00:01Z"), default_policy()),
    );
    assert_eq!(
        ended(end).outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 2 })
    );
    assert_eq!(executor.calls(), 2);
}

/// A deadline half a second past the reading's whole second is not reached at that second, and is
/// reached at the reading that equals it or passes it. The breach names whole seconds.
#[test]
fn adversary_a_sub_second_deadline_compares_its_fraction() {
    let cases = [
        ("2026-10-21T09:00:00.5Z", NOW, false),
        ("2026-10-21T09:00:00.5Z", "2026-10-21T09:00:00.5Z", true),
        (
            "2026-10-21T09:00:00.5Z",
            "2026-10-21T09:00:00.500000001Z",
            true,
        ),
        ("2026-10-21T09:00:00.000000001Z", NOW, false),
        (
            "2026-10-21T09:00:00.000000001Z",
            "2026-10-21T09:00:00.25Z",
            true,
        ),
    ];
    for (at, reading, reached) in cases {
        let case = Case::new("case-deadline-fraction", vec![admissible(EVALUATE)]);
        let executor = Steps::new([idle(), idle()]);
        let (end, _) = run(
            &case,
            &executor,
            &StaticAuthorityProvider::new(),
            &Effects::default(),
            &mut Context::new(deadline(at), default_policy()).reading(reading),
        );
        let end = ended(end);
        if reached {
            assert_eq!(
                end.outcome,
                deadline_passed(NOW_SECONDS, Some(NOW_SECONDS)),
                "deadline {at}, reading {reading}"
            );
            assert_eq!(executor.calls(), 0, "deadline {at}, reading {reading}");
        } else {
            assert_eq!(
                end.outcome,
                RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 2 }),
                "deadline {at}, reading {reading}"
            );
            assert_eq!(executor.calls(), 2, "deadline {at}, reading {reading}");
        }
    }
}

/// A deadline that names no RFC 3339 instant is refused before the Run starts, whatever the
/// near miss: no Run, no executor call, the failure naming the text.
#[test]
fn adversary_a_deadline_that_names_no_instant_is_refused_before_the_run_starts() {
    for text in [
        "2100-02-29T00:00:00Z",
        "1900-02-29T00:00:00Z",
        "2026-02-30T00:00:00Z",
        "2026-10-21T24:00:00Z",
        "2026-10-21T09:30:00+24:00",
        "2026-10-21T09:30:00-24:00",
        "2026-10-21T09:30:00+14",
        "2026-10-21T09:30:00.Z",
        "2026-10-21T09:30:00",
        " 2026-10-21T09:30:00Z",
        "2026-10-21T09:30:00Z\n",
        "2026-10-21T09:3\u{0660}:00Z",
        "10000-01-01T00:00:00Z",
        "-0001-01-01T00:00:00Z",
        "2026-10-21T09:30:00\u{00a0}Z",
        "",
    ] {
        let case = Case::new("case-deadline-refused", vec![admissible(EVALUATE)]);
        let executor = Steps::evaluations(1, None);
        let effects = Effects::default();
        let (end, store) = run(
            &case,
            &executor,
            &StaticAuthorityProvider::new(),
            &effects,
            &mut Context::new(deadline(text), default_policy()),
        );
        match end {
            Ok(end) => panic!("{text:?} was taken as a deadline: {:?}", end.outcome),
            Err(error) => {
                assert_eq!(
                    error.failure,
                    LoopFailure::InvalidDeadline(text.to_owned()),
                    "{text:?}"
                );
                assert_eq!(error.run_id, None, "{text:?}");
            }
        }
        assert!(RunStorage::get(&store.ports, &RunId(uuid(0x101))).is_none());
        assert_eq!(executor.calls(), 0, "{text:?}");
        assert!(effects.invoked().is_empty(), "{text:?}");
    }
}

/// Every RFC 3339 spelling of an instant before the reading is a deadline already reached, named in
/// whole seconds; one after it is not reached.
#[test]
fn adversary_every_rfc_3339_spelling_names_its_instant() {
    let before = NOW_SECONDS - 1;
    for (text, seconds) in [
        ("2026-10-21t08:59:59z", before),
        ("2026-10-21T22:59:59+14:00", before),
        ("2026-10-20T20:59:59-12:00", before),
        ("2026-10-21T08:59:59-00:00", before),
        ("2026-10-21T08:59:59.999999999999Z", before),
        ("2024-02-29T00:00:00Z", 1_709_164_800),
        ("2000-02-29T00:00:00Z", 951_782_400),
        ("2016-12-31T23:59:60Z", 1_483_228_800),
        ("1969-12-31T23:59:59Z", -1),
        ("0000-01-01T00:00:00Z", -62_167_219_200),
    ] {
        let case = Case::new("case-deadline-spelling", vec![admissible(EVALUATE)]);
        let executor = Steps::new([idle()]);
        let (end, _) = run(
            &case,
            &executor,
            &StaticAuthorityProvider::new(),
            &Effects::default(),
            &mut Context::new(deadline(text), default_policy()),
        );
        assert_eq!(
            ended(end).outcome,
            deadline_passed(seconds, Some(NOW_SECONDS)),
            "{text}"
        );
        assert_eq!(executor.calls(), 0, "{text}");
    }

    for text in [
        "9999-12-31T23:59:59Z",
        "2026-10-21T23:00:01+14:00",
        "2026-10-20T21:00:01-12:00",
    ] {
        let case = Case::new("case-deadline-later", vec![admissible(EVALUATE)]);
        let executor = Steps::new([idle(), idle()]);
        let (end, _) = run(
            &case,
            &executor,
            &StaticAuthorityProvider::new(),
            &Effects::default(),
            &mut Context::new(deadline(text), default_policy()),
        );
        assert_eq!(
            ended(end).outcome,
            RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 2 }),
            "{text}"
        );
        assert_eq!(executor.calls(), 2, "{text}");
    }
}

/// A reading written with an offset is the instant it names: one equal to the deadline reaches it.
#[test]
fn adversary_a_reading_with_an_offset_equal_to_the_deadline_reaches_it() {
    let case = Case::new("case-deadline-offset-reading", vec![admissible(EVALUATE)]);
    let executor = Steps::new([idle()]);
    let (end, _) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &Effects::default(),
        &mut Context::new(deadline(NOW), default_policy()).reading("2026-10-21T11:00:00+02:00"),
    );
    assert_eq!(
        ended(end).outcome,
        deadline_passed(NOW_SECONDS, Some(NOW_SECONDS))
    );
    assert_eq!(executor.calls(), 0);
}

/// A pending request is not invoked again once the clock reads exactly the deadline: the run ends
/// `Waiting` on it, for the deadline.
#[test]
fn adversary_a_pending_request_is_not_invoked_again_at_the_deadline() {
    let case = Case::new("case-deadline-pending-equal", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(1, None);
    let effects = Effects::default().answering(
        EVALUATE,
        [Reply::Pending {
            ends_run: false,
            report: "evaluating",
        }],
    );
    let mut context = Context::new(deadline("2026-10-21T09:30:00Z"), default_policy())
        .past_first_invoke(&effects, NOW, "2026-10-21T09:30:00Z");

    let (end, store) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );
    let end = ended(end);

    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![request_id(1)],
            reason: breach(BudgetLimit::Deadline, 1_792_575_000, Some(1_792_575_000)),
        })
    );
    assert_eq!(effects.invoked_of(EVALUATE), 1);
    assert_eq!(run_state(&store, &end), Some(RunState::Suspended));
}

/// A resume whose clock reads exactly its deadline takes nothing and performs nothing.
#[test]
fn adversary_a_resume_at_its_deadline_takes_nothing() {
    let case = Case::new("case-deadline-resume-equal", vec![admissible(EVALUATE)]);
    let held_commission = commission(&case.id);
    let suspensions = holding(&held_commission, EVALUATE);
    let executor = Steps::new([]);
    let effects = Effects::default();

    let end = resume_after_grant(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &held_commission,
        &mut runs(),
        &suspensions,
        &mut Context::new(deadline(NOW), default_policy()),
    )
    .unwrap_or_else(|error| panic!("the resume failed: {error:?}"));

    assert_eq!(end.outcome, deadline_passed(NOW_SECONDS, Some(NOW_SECONDS)));
    assert!(effects.invoked().is_empty());
    assert_eq!(
        stored_state(&suspensions, &held_commission),
        Some(SuspensionState::Pending)
    );
}

// --- 3. max_tokens ------------------------------------------------------------------------------

/// `max_tokens` adds an effect's reported tokens to the executor's: 50 from the call and 55 from
/// the effect pass a limit of 100 once the effect was recorded.
#[test]
fn adversary_max_tokens_adds_effect_tokens_to_executor_tokens() {
    let case = Case::new("case-tokens-sum", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(3, Some(tokens(30, 20)));
    let effects = Effects::default().answering(
        EVALUATE,
        [Reply::Performed(Some(EffectUsage {
            input_tokens: 25,
            output_tokens: 30,
            duration_ms: 1,
            cost_micro_usd: Some(1),
        }))],
    );

    let (end, _) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut Context::new(max_tokens(100), default_policy()),
    );
    let end = ended(end);

    assert_eq!(
        end.outcome,
        suspended(breach(BudgetLimit::MaxTokens, 100, Some(105)))
    );
    assert_eq!(executor.calls(), 1);
    assert_eq!(effects.invoked_of(EVALUATE), 1);
}

/// A charge equal to `max_tokens` does not pass it; the next token does.
#[test]
fn adversary_a_charge_equal_to_max_tokens_is_no_breach() {
    let case = Case::new("case-tokens-equal", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(3, Some(tokens(60, 40)));
    let effects = Effects::default();

    let (end, _) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut Context::new(max_tokens(100), default_policy()),
    );
    let end = ended(end);

    assert_eq!(
        end.outcome,
        suspended(breach(BudgetLimit::MaxTokens, 100, Some(200)))
    );
    assert_eq!(
        executor.calls(),
        2,
        "the first call's 100 tokens pass nothing"
    );
    assert_eq!(
        effects.invoked_of(EVALUATE),
        1,
        "the second step is discarded"
    );
}

/// A `max_tokens` of zero admits a call reporting no tokens and stops at the first token.
#[test]
fn adversary_a_max_tokens_of_zero() {
    let case = Case::new("case-tokens-zero-free", vec![admissible(EVALUATE)]);
    let executor = Steps::new([
        (
            ExecutorOutcome::NoUsefulAction(Unit(true)),
            Some(tokens(0, 0)),
        ),
        (
            ExecutorOutcome::NoUsefulAction(Unit(true)),
            Some(tokens(0, 0)),
        ),
    ]);
    let (end, _) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &Effects::default(),
        &mut Context::new(max_tokens(0), default_policy()),
    );
    assert_eq!(
        ended(end).outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 2 })
    );
    assert_eq!(executor.calls(), 2);

    let case = Case::new("case-tokens-zero-one", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(2, Some(tokens(0, 1)));
    let effects = Effects::default();
    let (end, _) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut Context::new(max_tokens(0), default_policy()),
    );
    assert_eq!(
        ended(end).outcome,
        suspended(breach(BudgetLimit::MaxTokens, 0, Some(1)))
    );
    assert_eq!(executor.calls(), 1);
    assert!(effects.invoked().is_empty());
}

/// A negative token figure charges nothing, so it cannot offset another: -100 in and 150 out
/// charge 150 against `max_tokens`.
#[test]
fn adversary_a_negative_token_figure_does_not_offset_the_other() {
    let case = Case::new("case-tokens-negative", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(2, Some(tokens(-100, 150)));
    let effects = Effects::default();

    let (end, _) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut Context::new(max_tokens(120), default_policy()),
    );

    assert_eq!(
        ended(end).outcome,
        suspended(breach(BudgetLimit::MaxTokens, 120, Some(150)))
    );
    assert_eq!(executor.calls(), 1);
    assert!(effects.invoked().is_empty());
}

/// Input and output near `i64::MAX` saturate instead of wrapping: their sum passes a limit just
/// below the maximum.
#[test]
fn adversary_max_tokens_saturates_near_the_maximum() {
    let case = Case::new("case-tokens-overflow", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(2, Some(tokens(i64::MAX, i64::MAX)));
    let effects = Effects::default();

    let (end, _) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut Context::new(max_tokens(i64::MAX - 1), default_policy()),
    );

    assert_eq!(
        ended(end).outcome,
        suspended(breach(BudgetLimit::MaxTokens, i64::MAX - 1, Some(i64::MAX)))
    );
    assert_eq!(executor.calls(), 1);
    assert!(effects.invoked().is_empty());
}

// --- 4. on_breach: KeepStep ---------------------------------------------------------------------

/// A kept step proposing an action the frontier does not list is not performed, and the run is
/// suspended for the breach with no further executor call.
#[test]
fn adversary_a_kept_step_outside_the_frontier_is_not_performed() {
    let case = Case::new("case-kept-unlisted", vec![admissible(EVALUATE)]);
    let executor = Steps::new([(
        ExecutorOutcome::ProposedAction(proposed("deploy", Value::Null)),
        Some(tokens(60, 50)),
    )]);
    let effects = Effects::default();

    let (end, store) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut Context::new(max_tokens(100), keep_step()),
    );
    let end = ended(end);

    assert_eq!(
        end.outcome,
        suspended(breach(BudgetLimit::MaxTokens, 100, Some(110)))
    );
    assert!(effects.invoked().is_empty());
    assert_eq!(executor.calls(), 1);
    assert_eq!(run_state(&store, &end), Some(RunState::Suspended));
}

/// A kept step whose case somebody else moved while the executor worked is stale and not
/// performed, and no further executor call is made.
#[test]
fn adversary_a_kept_stale_step_is_not_performed() {
    let case = Case::new("case-kept-stale", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(3, Some(tokens(60, 50))).moving(&case);
    let effects = Effects::default();

    let (end, _) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut Context::new(max_tokens(100), keep_step()),
    );
    let end = ended(end);

    assert!(effects.invoked().is_empty(), "ended {:?}", end.outcome);
    assert_eq!(executor.calls(), 1, "ended {:?}", end.outcome);
}

/// A kept fan-out's reads are performed, each once, and the run is then suspended for the breach;
/// discarded, none is.
#[test]
fn adversary_a_kept_fan_out_is_performed_and_a_discarded_one_is_not() {
    for (on_breach, invoked) in [(BreachStep::KeepStep, 2), (BreachStep::DiscardStep, 0)] {
        let case = Case::new(
            "case-kept-fan-out",
            vec![admissible("a.read"), admissible("b.read")],
        );
        let executor = Steps::new([(reads(&["a.read", "b.read"]), Some(tokens(60, 50)))]);
        let effects = Effects::reading();

        let (end, _) = run(
            &case,
            &executor,
            &StaticAuthorityProvider::new(),
            &effects,
            &mut Context::new(max_tokens(100), policy(true, None, on_breach)),
        );

        assert_eq!(
            ended(end).outcome,
            suspended(breach(BudgetLimit::MaxTokens, 100, Some(110))),
            "{on_breach:?}"
        );
        assert_eq!(effects.invoked().len(), invoked, "{on_breach:?}");
        assert_eq!(executor.calls(), 1, "{on_breach:?}");
    }
}

// --- 5. The idle bound --------------------------------------------------------------------------

/// An idle bound too large to reach leaves a budgeted run to its step budget; with no idle bound
/// and a step budget of one, the budget alone ends it.
#[test]
fn adversary_an_unreachable_idle_bound_leaves_the_step_budget() {
    for (bound, steps) in [(Some(i64::MAX), 3), (None, 1)] {
        let case = Case::new("case-idle-large", vec![admissible(EVALUATE)]);
        let executor = Steps::new([idle(), idle(), idle(), idle()]);
        let (end, _) = run(
            &case,
            &executor,
            &StaticAuthorityProvider::new(),
            &Effects::default(),
            &mut Context::new(None, policy(true, bound, BreachStep::DiscardStep)).steps(steps),
        );
        let end = ended(end);
        assert!(
            matches!(
                end.outcome,
                RunOutcome::Suspended(RunOutcomeSuspended {
                    reason: SuspensionReason::Budget(_)
                })
            ),
            "{bound:?}: {:?}",
            end.outcome
        );
        assert_eq!(executor.calls(), steps, "{bound:?}");
    }
}

// --- 6. A pending answer that ends the run ------------------------------------------------------

/// A fan-out whose reads answer pending without and with `ends_run`, beside a performed one, ends
/// `Waiting` on every pending read in proposal order, for the first ending read's report; each
/// read is invoked once.
#[test]
fn adversary_a_mixed_fan_out_waits_on_every_pending_read_in_order() {
    let case = Case::new(
        "case-pending-mixed",
        vec![
            admissible("a.read"),
            admissible("b.read"),
            admissible("c.read"),
            admissible("d.read"),
        ],
    );
    let executor = Steps::new([(reads(&["a.read", "b.read", "c.read", "d.read"]), None)]);
    let effects = Effects::reading()
        .answering(
            "a.read",
            [Reply::Pending {
                ends_run: false,
                report: "a waits",
            }],
        )
        .answering(
            "b.read",
            [Reply::Pending {
                ends_run: true,
                report: "b waits",
            }],
        )
        .answering(
            "d.read",
            [Reply::Pending {
                ends_run: true,
                report: "d waits",
            }],
        );

    let (end, store) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut Context::new(None, default_policy()),
    );
    let end = ended(end);

    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![request_id(1), request_id(2), request_id(4)],
            reason: SuspensionReason::Time(Value::Text("b waits".to_owned())),
        })
    );
    for read in ["a.read", "b.read", "c.read", "d.read"] {
        assert_eq!(effects.invoked_of(read), 1, "{read}");
    }
    assert_eq!(executor.calls(), 1);
    assert_eq!(run_state(&store, &end), Some(RunState::Suspended));
}

/// A fan-out whose performed read passes a spending limit beside a read answered pending with
/// `ends_run` ends `Waiting` for the limit, which is checked first.
#[test]
fn adversary_a_spending_limit_beside_an_ending_pending_read_is_the_reason() {
    let case = Case::new(
        "case-pending-breach",
        vec![admissible("a.read"), admissible("b.read")],
    );
    let executor = Steps::new([(reads(&["a.read", "b.read"]), None)]);
    let effects = Effects::reading()
        .answering(
            "a.read",
            [Reply::Performed(Some(EffectUsage {
                input_tokens: 80,
                output_tokens: 40,
                duration_ms: 1,
                cost_micro_usd: Some(1),
            }))],
        )
        .answering(
            "b.read",
            [Reply::Pending {
                ends_run: true,
                report: "b waits",
            }],
        );

    let (end, _) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut Context::new(max_tokens(100), default_policy()),
    );

    assert_eq!(
        ended(end).outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![request_id(2)],
            reason: breach(BudgetLimit::MaxTokens, 100, Some(120)),
        })
    );
    assert_eq!(effects.invoked().len(), 2);
}

/// A fan-out with a read refused as final beside one answered pending with `ends_run` ends the run
/// once, invoking each read once and no executor again.
#[test]
fn adversary_a_final_refusal_beside_an_ending_pending_read_invokes_nothing_twice() {
    let case = Case::new(
        "case-pending-refused",
        vec![admissible("a.read"), admissible("b.read")],
    );
    let executor = Steps::new([(reads(&["a.read", "b.read"]), None), idle(), idle()]);
    let effects = Effects::reading()
        .answering("a.read", [Reply::Refused { ends_run: true }])
        .answering(
            "b.read",
            [Reply::Pending {
                ends_run: true,
                report: "b waits",
            }],
        );

    let (end, _) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut Context::new(None, default_policy()),
    );
    let end = ended(end);

    assert!(
        matches!(
            end.outcome,
            RunOutcome::EffectRefused(_) | RunOutcome::Waiting(_)
        ),
        "{:?}",
        end.outcome
    );
    assert_eq!(effects.invoked_of("a.read"), 1);
    assert_eq!(effects.invoked_of("b.read"), 1);
    assert_eq!(executor.calls(), 1);
}

/// A request answered pending without `ends_run`, then with it, is invoked twice and the run ends
/// `Waiting` on it for the second answer's report, with one executor call.
#[test]
fn adversary_a_reinvoked_request_answered_ending_ends_the_run() {
    let case = Case::new("case-pending-again", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(3, None);
    let effects = Effects::default().answering(
        EVALUATE,
        [
            Reply::Pending {
                ends_run: false,
                report: "first",
            },
            Reply::Pending {
                ends_run: true,
                report: "now waits",
            },
        ],
    );

    let (end, store) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut Context::new(None, default_policy()),
    );
    let end = ended(end);

    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![request_id(1)],
            reason: SuspensionReason::Time(Value::Text("now waits".to_owned())),
        })
    );
    assert_eq!(effects.invoked_of(EVALUATE), 2);
    assert_eq!(executor.calls(), 1);
    assert_eq!(run_state(&store, &end), Some(RunState::Suspended));
}

/// A resumed request answered pending with `ends_run` ends the resume `Waiting` on it, its
/// Suspension released, invoked once, with no executor call.
#[test]
fn adversary_a_resumed_request_answered_ending_releases_its_suspension() {
    let case = Case::new("case-pending-resumed", vec![admissible(EVALUATE)]);
    let held_commission = commission(&case.id);
    let suspensions = holding(&held_commission, EVALUATE);
    let executor = Steps::new([]);
    let effects = Effects::default().answering(
        EVALUATE,
        [Reply::Pending {
            ends_run: true,
            report: "resumed waits",
        }],
    );
    let mut store = runs();

    let end = resume_after_grant(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &held_commission,
        &mut store,
        &suspensions,
        &mut Context::new(None, default_policy()),
    )
    .unwrap_or_else(|error| panic!("the resume failed: {error:?}"));

    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![ActionRequestId(uuid(0x301))],
            reason: SuspensionReason::Time(Value::Text("resumed waits".to_owned())),
        })
    );
    assert_eq!(effects.invoked_of(EVALUATE), 1);
    assert_eq!(executor.calls(), 0);
    assert_eq!(stored_state(&suspensions, &held_commission), None);
    assert_eq!(
        RunStorage::get(&store, &RunId(uuid(0x101))).map(|held| held.state),
        Some(RunState::Suspended)
    );
}

/// A read answered pending with `ends_run` beside a read found stale is kept while the move is
/// judged; the move the stale read showed ends the run, and nothing is invoked again or asked of
/// the executor. The run's outcome is the move's, so the pending read is named nowhere in it.
#[test]
fn adversary_an_ending_pending_read_beside_a_stale_read_gives_way_to_the_move() {
    // Frontier 1 is the one handed to the executor, frontier 2 the one `b.read` is revalidated
    // against; right after it somebody else moves the case, so `a.read` is found stale.
    let case = Case::new(
        "case-pending-stale",
        vec![admissible("a.read"), admissible("b.read")],
    )
    .moving_after_frontier(2);
    let executor = Steps::new([(reads(&["b.read", "a.read"]), None), idle(), idle()]);
    let effects = Effects::reading().answering(
        "b.read",
        [Reply::Pending {
            ends_run: true,
            report: "b waits",
        }],
    );

    let (end, _) = run(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut Context::new(None, default_policy()),
    );
    let end = ended(end);

    assert_eq!(effects.invoked_of("b.read"), 1);
    assert_eq!(
        effects.invoked_of("a.read"),
        0,
        "the stale read is not invoked"
    );
    assert_eq!(executor.calls(), 1);
    assert!(
        matches!(end.outcome, RunOutcome::CaseMovedOn(_)),
        "{:?}",
        end.outcome
    );
}

/// The two limits the wave adds survive the governor's file suspension store: a Suspension held
/// for a deadline breach before the epoch with no reading, and for a token breach, reads back as
/// it was held.
#[test]
fn adversary_the_new_limits_survive_the_file_suspension_store() {
    for (name, reason) in [
        (
            "deadline",
            breach(BudgetLimit::Deadline, -62_167_219_200, None),
        ),
        (
            "max-tokens",
            breach(BudgetLimit::MaxTokens, i64::MAX - 1, Some(i64::MAX)),
        ),
    ] {
        let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("adversary-w20261021w7-runtime")
            .join(name);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("the old store is removed");
        }
        let held_commission = commission(&CaseId(format!("case-{name}")));
        let commission_id = held_commission.data().commission_id.clone();
        {
            let store = FileSuspensionStore::open(&dir).expect("the store opens");
            let held = SuspensionPort::hold(
                &store,
                HoldSuspension {
                    suspension: SuspensionKey {
                        commission_id: commission_id.clone(),
                    },
                    commission_id: commission_id.clone(),
                    run_id: RunId(uuid(0x101)),
                    run_case_revision: 1,
                    action_request_id: ActionRequestId(uuid(0x301)),
                    case_id: held_commission.data().case_id.clone(),
                    expected_case_revision: 1,
                    action: EVALUATE.to_owned(),
                    arguments: ProposedActionArguments(Value::Null),
                    reason: reason.clone(),
                },
            );
            assert!(matches!(held, HoldSuspensionOutcome::Held { .. }), "{name}");
        }
        let store = FileSuspensionStore::open(&dir).expect("the store opens again");
        let read = SuspensionPort::get(&store, &commission_id)
            .unwrap_or_else(|error| panic!("{name}: the store reads: {error:?}"))
            .unwrap_or_else(|| panic!("{name}: the Suspension is held"));
        assert_eq!(read.data.reason, reason, "{name}");
        assert_eq!(read.state, SuspensionState::Pending, "{name}");
    }
}
