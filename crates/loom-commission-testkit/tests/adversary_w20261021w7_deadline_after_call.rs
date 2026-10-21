//! Adversary cases for the deadline read once an executor call was observed
//! (`story:run-budget-tokens-and-deadline`, the fix that reads `RunBudget.deadline` again after
//! the spending limits and lets `on_breach` decide a call that ended at or after it).
//!
//! The ports are those of `adversary_w20261021w7_runtime.rs`, repeated here so each file builds
//! alone; a few of them serve only that file's cases. The cases drive `run_until_blocked`,
//! `run_with_suspension` and `resume_after_grant` with a clock that passes the deadline once the
//! executor was called a given number of times.

#![allow(dead_code)]

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::{Generated, RunStorage};
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext,
    AuthorityVerdict, AuthorityVerdictApprovalRequired, BreachStep, BudgetLimit, CaseId,
    Commission, CommissionData, CommissionId, CompletionDetermination,
    CompletionDeterminationComplete, EffectOutcome, EffectOutcomePending, EffectOutcomePerformed,
    EffectOutcomeRefused, EffectUsage, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ExecutorOutcomeProposedReads, ExecutorOutcomeSuspended, ExecutorStep, ExecutorUsage, Frontier,
    FrontierAction, FrontierData, FrontierId, GovernorError, HoldSuspension, HoldSuspensionOutcome,
    Observation, ObservationId, PrincipalId, ProposedActionArguments, RunBudget, RunBudgetBreach,
    RunId, RunOutcome, RunOutcomeCompleted, RunOutcomeEffectRefused, RunOutcomeNeedsAuthority,
    RunOutcomeSuspended, RunOutcomeWaiting, RunState, StopPolicy, SuspensionKey, SuspensionReason,
    SuspensionState, Unit, commission_state, frontier_state, observation_state,
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
    LoopContext, LoopEnd, LoopError, LoopFailure, resume_after_grant, run_until_blocked,
    run_with_suspension,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;

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

// --- The deadline read after the executor call ---------------------------------------------------

/// The deadline every case below sets.
const DEADLINE: &str = "2026-10-21T09:30:00Z";
const DEADLINE_SECONDS: i64 = 1_792_575_000;
/// What the clock reads once the deadline passed.
const AFTER: &str = "2026-10-21T09:45:00Z";
const AFTER_SECONDS: i64 = 1_792_575_900;
/// A reading that is no RFC 3339 date-time.
const GARBLED: &str = "quarter to ten";

/// A context whose clock reads `before` until the executor was called `n` times and `after` from
/// then on, counting every reading.
struct Timed {
    inner: Context,
    calls: Arc<AtomicUsize>,
    n: usize,
    before: String,
    after: String,
    reads: usize,
}

impl Timed {
    fn after_calls(inner: Context, executor: &Steps<'_>, n: usize, after: &str) -> Self {
        Self {
            inner,
            calls: Arc::clone(&executor.calls),
            n,
            before: NOW.to_owned(),
            after: after.to_owned(),
            reads: 0,
        }
    }

    /// Past the deadline once the executor was called once.
    fn past_first_call(
        budget: Option<RunBudget>,
        policy: StopPolicy,
        executor: &Steps<'_>,
    ) -> Self {
        Self::after_calls(Context::new(budget, policy), executor, 1, AFTER)
    }
}

impl LoopContext for Timed {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.inner.action_request_id()
    }

    fn observation_id(&mut self) -> ObservationId {
        self.inner.observation_id()
    }

    fn now(&mut self) -> Timestamp {
        self.reads += 1;
        Timestamp(if self.calls.load(Ordering::SeqCst) >= self.n {
            self.after.clone()
        } else {
            self.before.clone()
        })
    }

    fn step_budget(&self) -> Option<usize> {
        self.inner.step_budget()
    }

    fn run_budget(&self) -> Option<RunBudget> {
        self.inner.run_budget()
    }

    fn stop_policy(&self) -> StopPolicy {
        self.inner.stop_policy()
    }
}

/// `case`, which the governor holds complete once the executor was called once.
struct CompletesDuringCall<'a> {
    case: &'a Case,
    calls: Arc<AtomicUsize>,
}

impl Governor for CompletesDuringCall<'_> {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.case.current_revision(case)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        self.case.frontier(case)
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.case.completion(case)?;
        if self.calls.load(Ordering::SeqCst) >= 1 {
            Ok(CompletionDetermination::Complete(
                CompletionDeterminationComplete {
                    outcome: "done".to_owned(),
                },
            ))
        } else {
            Ok(CompletionDetermination::Open(Unit(true)))
        }
    }
}

impl ObservationPort for CompletesDuringCall<'_> {
    fn observe(
        &self,
        observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        self.case.observe(observation)
    }
}

/// `run_until_blocked` on `case` under any context.
fn run_timed<G, A, C>(
    governor: &G,
    case: &CaseId,
    executor: &Steps<'_>,
    authority: &A,
    effects: &Effects,
    context: &mut C,
) -> (Result<LoopEnd, LoopError>, Generated<RunStore>)
where
    G: Governor + ObservationPort,
    A: AuthorityProvider,
    C: LoopContext,
{
    let mut store = Generated::new(runs());
    let end = run_until_blocked(
        governor,
        executor,
        authority,
        effects,
        &commission(case),
        &mut store,
        context,
    );
    (end, store)
}

fn past_deadline() -> RunOutcome {
    deadline_passed(DEADLINE_SECONDS, Some(AFTER_SECONDS))
}

fn deadline_breach() -> SuspensionReason {
    breach(BudgetLimit::Deadline, DEADLINE_SECONDS, Some(AFTER_SECONDS))
}

fn ending(report: &'static str) -> Reply {
    Reply::Pending {
        ends_run: true,
        report,
    }
}

fn going_on(report: &'static str) -> Reply {
    Reply::Pending {
        ends_run: false,
        report,
    }
}

// Steps that end the run.

/// Under `KeepStep`, a kept proposal the port refuses as final ends the run `EffectRefused`, the
/// step's own outcome, though its call ended past the deadline; it is invoked once.
#[test]
fn adversary_a_kept_step_refused_as_final_past_the_deadline_ends_effect_refused() {
    let case = Case::new("case-after-call-refused", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(2, None);
    let effects = Effects::default().answering(EVALUATE, [Reply::Refused { ends_run: true }]);
    let mut context = Timed::past_first_call(deadline(DEADLINE), keep_step(), &executor);

    let (end, _) = run_timed(
        &case,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );
    let end = ended(end);

    assert_eq!(
        end.outcome,
        RunOutcome::EffectRefused(RunOutcomeEffectRefused {
            action_request_id: request_id(1),
            reason: "nothing changed".to_owned(),
        })
    );
    assert_eq!(effects.invoked_of(EVALUATE), 1);
    assert_eq!(executor.calls(), 1);
}

/// Under `DiscardStep`, a proposal from a call past the deadline that the port would refuse as
/// final is never handed to it: the run is suspended for the deadline.
#[test]
fn adversary_a_discarded_step_is_not_invoked_even_to_be_refused() {
    let case = Case::new(
        "case-after-call-refused-discarded",
        vec![admissible(EVALUATE)],
    );
    let executor = Steps::evaluations(2, None);
    let effects = Effects::default().answering(EVALUATE, [Reply::Refused { ends_run: true }]);
    let mut context = Timed::past_first_call(deadline(DEADLINE), default_policy(), &executor);

    let (end, store) = run_timed(
        &case,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );
    let end = ended(end);

    assert_eq!(end.outcome, past_deadline());
    assert!(effects.invoked().is_empty());
    assert!(end.admitted.is_empty());
    assert_eq!(run_state(&store, &end), Some(RunState::Suspended));
}

/// Under `DiscardStep`, an executor's own suspension from a call past the deadline is not the
/// Run's: the Run is suspended for the deadline.
#[test]
fn adversary_an_executor_suspension_past_the_deadline_is_suspended_for_the_deadline() {
    let case = Case::new("case-after-call-suspended", vec![admissible(EVALUATE)]);
    let executor = Steps::new([(
        ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
            reason: SuspensionReason::Evidence(vec!["the executor waits".to_owned()]),
        }),
        None,
    )]);
    let effects = Effects::default();
    let mut context = Timed::past_first_call(deadline(DEADLINE), default_policy(), &executor);

    let (end, store) = run_timed(
        &case,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );
    let end = ended(end);

    assert_eq!(end.outcome, past_deadline());
    assert_eq!(run_state(&store, &end), Some(RunState::Suspended));
    assert!(effects.invoked().is_empty());
}

/// Under `DiscardStep`, a case somebody completed while a call ran past the deadline is reported
/// suspended for the deadline: the step is discarded before the case is loaded again, as the step
/// budget stops a step that performed nothing.
#[test]
fn adversary_a_case_completed_during_a_call_past_the_deadline_is_suspended() {
    let case = Case::new("case-after-call-completed", vec![admissible(EVALUATE)]);
    let executor = Steps::new([(ExecutorOutcome::CompletedLocalReasoning(Unit(true)), None)]);
    let governor = CompletesDuringCall {
        case: &case,
        calls: Arc::clone(&executor.calls),
    };
    let effects = Effects::default();
    let mut context = Timed::past_first_call(deadline(DEADLINE), default_policy(), &executor);

    let (end, _) = run_timed(
        &governor,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );

    assert_eq!(ended(end).outcome, past_deadline());
}

/// Under `KeepStep` the same step is acted on "as with no breach". With no deadline, that run ends
/// completed once the case is loaded again; kept past the deadline it is suspended instead.
#[test]
fn adversary_a_kept_step_on_a_case_completed_during_the_call() {
    let case = Case::new("case-after-call-completed-kept", vec![admissible(EVALUATE)]);
    let executor = Steps::new([(ExecutorOutcome::CompletedLocalReasoning(Unit(true)), None)]);
    let governor = CompletesDuringCall {
        case: &case,
        calls: Arc::clone(&executor.calls),
    };
    let effects = Effects::default();

    let mut unbounded = Timed::past_first_call(None, keep_step(), &executor);
    let (without, _) = run_timed(
        &governor,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut unbounded,
    );
    assert_eq!(
        ended(without).outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "done".to_owned(),
        })
    );

    let case = Case::new("case-after-call-completed-kept", vec![admissible(EVALUATE)]);
    let executor = Steps::new([(ExecutorOutcome::CompletedLocalReasoning(Unit(true)), None)]);
    let governor = CompletesDuringCall {
        case: &case,
        calls: Arc::clone(&executor.calls),
    };
    let mut bounded = Timed::past_first_call(deadline(DEADLINE), keep_step(), &executor);
    let (with, _) = run_timed(
        &governor,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut bounded,
    );
    assert_eq!(ended(with).outcome, past_deadline());
}

// The fan-out path.

/// Under `DiscardStep`, a fan-out from a call past the deadline invokes none of its reads.
#[test]
fn adversary_a_fan_out_from_a_call_past_the_deadline_reads_nothing() {
    let case = Case::new(
        "case-after-call-fan-out",
        vec![admissible("a.read"), admissible("b.read")],
    );
    let executor = Steps::new([(reads(&["a.read", "b.read"]), None)]);
    let effects = Effects::reading();
    let mut context = Timed::past_first_call(deadline(DEADLINE), default_policy(), &executor);

    let (end, _) = run_timed(
        &case,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );
    let end = ended(end);

    assert_eq!(end.outcome, past_deadline());
    assert!(effects.invoked().is_empty());
    assert!(end.admitted.is_empty());
}

/// Under `KeepStep`, a kept fan-out is read once each, and the run is then suspended for the
/// deadline with no further executor call.
#[test]
fn adversary_a_kept_fan_out_past_the_deadline_reads_once_and_stops() {
    let case = Case::new(
        "case-after-call-fan-out-kept",
        vec![admissible("a.read"), admissible("b.read")],
    );
    let executor = Steps::new([
        (reads(&["a.read", "b.read"]), None),
        (reads(&["a.read", "b.read"]), None),
    ]);
    let effects = Effects::reading();
    let mut context = Timed::past_first_call(deadline(DEADLINE), keep_step(), &executor);

    let (end, _) = run_timed(
        &case,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );
    let end = ended(end);

    assert_eq!(end.outcome, past_deadline());
    assert_eq!(effects.invoked_of("a.read"), 1);
    assert_eq!(effects.invoked_of("b.read"), 1);
    assert_eq!(executor.calls(), 1);
}

/// Under `KeepStep`, a kept fan-out whose read ends the run pending waits on it for the deadline,
/// the breach the kept step passed, and invokes nothing again.
#[test]
fn adversary_a_kept_fan_out_whose_read_ends_the_run_waits_for_the_deadline() {
    let case = Case::new(
        "case-after-call-fan-out-ending",
        vec![admissible("a.read"), admissible("b.read")],
    );
    let executor = Steps::new([(reads(&["a.read", "b.read"]), None)]);
    let effects = Effects::reading().answering("b.read", [ending("b waits")]);
    let mut context = Timed::past_first_call(deadline(DEADLINE), keep_step(), &executor);

    let (end, store) = run_timed(
        &case,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );
    let end = ended(end);

    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![request_id(2)],
            reason: deadline_breach(),
        })
    );
    assert_eq!(effects.invoked().len(), 2);
    assert_eq!(run_state(&store, &end), Some(RunState::Suspended));
}

// A pending request.

/// Under `KeepStep`, a kept proposal answered pending (not ending the run) is not invoked again:
/// the run ends `Waiting` on it for the deadline.
#[test]
fn adversary_a_kept_pending_request_past_the_deadline_is_not_invoked_again() {
    let case = Case::new("case-after-call-pending-kept", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(2, None);
    let effects = Effects::default().answering(EVALUATE, [going_on("evaluating")]);
    let mut context = Timed::past_first_call(deadline(DEADLINE), keep_step(), &executor);

    let (end, _) = run_timed(
        &case,
        &case.id,
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
            reason: deadline_breach(),
        })
    );
    assert_eq!(effects.invoked_of(EVALUATE), 1);
    assert_eq!(executor.calls(), 1);
}

/// Under `KeepStep`, a kept proposal whose pending answer ends the run waits on it for the
/// deadline, the kept breach, not for the port's time.
#[test]
fn adversary_a_kept_pending_answer_that_ends_the_run_waits_for_the_deadline() {
    let case = Case::new("case-after-call-pending-ending", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(2, None);
    let effects = Effects::default().answering(EVALUATE, [ending("port waits")]);
    let mut context = Timed::past_first_call(deadline(DEADLINE), keep_step(), &executor);

    let (end, _) = run_timed(
        &case,
        &case.id,
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
            reason: deadline_breach(),
        })
    );
    assert_eq!(effects.invoked_of(EVALUATE), 1);
}

/// A pending request invoked again until it is performed, then a later call that ends past the
/// deadline: the request is invoked twice and nothing the later call proposed is performed.
#[test]
fn adversary_a_call_after_a_reinvoked_request_is_discarded_past_the_deadline() {
    let case = Case::new("case-after-call-reinvoked", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(3, None);
    let effects = Effects::default().answering(
        EVALUATE,
        [
            going_on("evaluating"),
            Reply::Performed(None),
            Reply::Performed(None),
        ],
    );
    let mut context = Timed::after_calls(
        Context::new(deadline(DEADLINE), default_policy()),
        &executor,
        2,
        AFTER,
    );

    let (end, _) = run_timed(
        &case,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );
    let end = ended(end);

    assert_eq!(end.outcome, past_deadline());
    assert_eq!(executor.calls(), 2);
    let invoked = effects.invoked();
    assert_eq!(invoked.len(), 2);
    assert!(
        invoked
            .iter()
            .all(|data| data.action_request_id == request_id(1))
    );
}

// Ordering against a spending limit.

/// Under `KeepStep`, a call past both a spending limit and the deadline is kept for the spending
/// limit, which is checked first: its proposal is performed once and the run is suspended for
/// `MaxTokens`.
#[test]
fn adversary_a_kept_call_past_a_spending_limit_and_the_deadline_names_the_spending_limit() {
    let case = Case::new("case-after-call-both-kept", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(2, Some(tokens(60, 50)));
    let effects = Effects::default();
    let budget = Some(RunBudget {
        max_tokens: Some(100),
        deadline: Some(Timestamp(DEADLINE.to_owned())),
        ..unlimited()
    });
    let mut context = Timed::past_first_call(budget, keep_step(), &executor);

    let (end, _) = run_timed(
        &case,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );
    let end = ended(end);

    assert_eq!(
        end.outcome,
        suspended(breach(BudgetLimit::MaxTokens, 100, Some(110)))
    );
    assert_eq!(effects.invoked_of(EVALUATE), 1);
    assert_eq!(executor.calls(), 1);
}

/// Under `KeepStep`, a step kept for the deadline whose effect then passes `max_tokens` performs
/// nothing more and calls the executor no further; the run is suspended.
#[test]
fn adversary_a_step_kept_for_the_deadline_whose_effect_passes_max_tokens_stops() {
    let case = Case::new(
        "case-after-call-kept-then-spent",
        vec![admissible(EVALUATE)],
    );
    let executor = Steps::evaluations(2, Some(tokens(10, 10)));
    let effects = Effects::default().answering(
        EVALUATE,
        [Reply::Performed(Some(EffectUsage {
            input_tokens: 100,
            output_tokens: 100,
            duration_ms: 1,
            cost_micro_usd: None,
        }))],
    );
    let budget = Some(RunBudget {
        max_tokens: Some(100),
        deadline: Some(Timestamp(DEADLINE.to_owned())),
        ..unlimited()
    });
    let mut context = Timed::past_first_call(budget, keep_step(), &executor);

    let (end, store) = run_timed(
        &case,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );
    let end = ended(end);

    // The deadline the step was kept for is the reason given; the effect's spending limit is not
    // read while a breach is kept.
    assert_eq!(end.outcome, past_deadline());
    assert_eq!(effects.invoked_of(EVALUATE), 1);
    assert_eq!(executor.calls(), 1);
    assert_eq!(run_state(&store, &end), Some(RunState::Suspended));
}

// A clock reading that is not RFC 3339 after the call.

/// Under `DiscardStep`, a reading after the call that is no RFC 3339 date-time counts as past the
/// deadline: nothing is performed and the breach charges nothing known.
#[test]
fn adversary_a_garbled_reading_after_the_call_discards_its_step() {
    let case = Case::new("case-after-call-garbled", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(2, None);
    let effects = Effects::default();
    let mut context = Timed::after_calls(
        Context::new(deadline(DEADLINE), default_policy()),
        &executor,
        1,
        GARBLED,
    );

    let (end, _) = run_timed(
        &case,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );
    let end = ended(end);

    assert_eq!(end.outcome, deadline_passed(DEADLINE_SECONDS, None));
    assert!(effects.invoked().is_empty());
}

/// Under `KeepStep`, the same reading keeps the step: it is performed once and the run is then
/// suspended for the deadline, charging nothing known.
#[test]
fn adversary_a_garbled_reading_after_a_kept_call_performs_once() {
    let case = Case::new("case-after-call-garbled-kept", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(2, None);
    let effects = Effects::default();
    let mut context = Timed::after_calls(
        Context::new(deadline(DEADLINE), keep_step()),
        &executor,
        1,
        GARBLED,
    );

    let (end, _) = run_timed(
        &case,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );
    let end = ended(end);

    assert_eq!(end.outcome, deadline_passed(DEADLINE_SECONDS, None));
    assert_eq!(effects.invoked_of(EVALUATE), 1);
    assert_eq!(executor.calls(), 1);
}

// No extra clock read without a deadline.

/// With no deadline the clock is read only for observations: one reading per observation the
/// governor received, though the clock reads no RFC 3339 date-time at all.
#[test]
fn adversary_without_a_deadline_the_clock_is_read_only_for_observations() {
    let case = Case::new("case-after-call-no-deadline", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(3, None);
    let effects = Effects::default();
    let mut context = Timed::after_calls(
        Context::new(None, default_policy()).steps(3),
        &executor,
        0,
        GARBLED,
    );

    let (end, _) = run_timed(
        &case,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );
    let end = ended(end);

    assert!(
        matches!(end.outcome, RunOutcome::Suspended(_)),
        "{:?}",
        end.outcome
    );
    assert_eq!(effects.invoked_of(EVALUATE), 3);
    assert_eq!(executor.calls(), 3);
    assert_eq!(context.reads, case.observed.load(Ordering::SeqCst));
}

/// With a deadline never reached, each executor step reads the clock twice besides its
/// observations, once before the call and once after it.
#[test]
fn adversary_with_a_deadline_each_call_reads_the_clock_before_and_after() {
    let case = Case::new("case-after-call-deadline-reads", vec![admissible(EVALUATE)]);
    let executor = Steps::evaluations(3, None);
    let effects = Effects::default();
    let mut context = Timed::after_calls(
        Context::new(deadline(DEADLINE), default_policy()).steps(3),
        &executor,
        usize::MAX,
        AFTER,
    );

    let (end, _) = run_timed(
        &case,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );
    let end = ended(end);

    assert!(
        matches!(end.outcome, RunOutcome::Suspended(_)),
        "{:?}",
        end.outcome
    );
    assert_eq!(executor.calls(), 3);
    assert_eq!(
        context.reads,
        case.observed.load(Ordering::SeqCst) + 2 * executor.calls()
    );
}

// resume_after_grant.

/// A resume performs its held request, then a call past the deadline: by default nothing that
/// call proposed is performed, the Suspension is released and the Run is suspended for the
/// deadline.
#[test]
fn adversary_a_resumed_runs_call_past_the_deadline_is_discarded() {
    let case = Case::new("case-after-call-resume", vec![admissible(EVALUATE)]);
    let held_commission = commission(&case.id);
    let suspensions = holding(&held_commission, EVALUATE);
    let executor = Steps::evaluations(2, None);
    let effects = Effects::default();
    let mut context = Timed::past_first_call(deadline(DEADLINE), default_policy(), &executor);
    let mut store = runs();

    let end = resume_after_grant(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &held_commission,
        &mut store,
        &suspensions,
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the resume failed: {error:?}"));

    assert_eq!(end.outcome, past_deadline());
    let invoked = effects.invoked();
    assert_eq!(invoked.len(), 1);
    assert_eq!(invoked[0].action_request_id, ActionRequestId(uuid(0x301)));
    assert_eq!(executor.calls(), 1);
    assert_eq!(stored_state(&suspensions, &held_commission), None);
}

/// Under `KeepStep`, the resumed Run's call past the deadline is kept: its proposal is performed
/// once after the held request, and the Run is then suspended for the deadline.
#[test]
fn adversary_a_resumed_runs_kept_call_past_the_deadline_performs_once() {
    let case = Case::new("case-after-call-resume-kept", vec![admissible(EVALUATE)]);
    let held_commission = commission(&case.id);
    let suspensions = holding(&held_commission, EVALUATE);
    let executor = Steps::evaluations(2, None);
    let effects = Effects::default();
    let mut context = Timed::past_first_call(deadline(DEADLINE), keep_step(), &executor);
    let mut store = runs();

    let end = resume_after_grant(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &held_commission,
        &mut store,
        &suspensions,
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the resume failed: {error:?}"));

    assert_eq!(end.outcome, past_deadline());
    assert_eq!(effects.invoked().len(), 2);
    assert_eq!(executor.calls(), 1);
    assert_eq!(stored_state(&suspensions, &held_commission), None);
}

/// Under `KeepStep` with a suspension port, a kept proposal past the deadline that the provider
/// withholds holds its Suspension and invokes nothing; a resume after the deadline then takes
/// nothing and performs nothing, the Suspension still `Pending`.
#[test]
fn adversary_a_withheld_kept_step_past_the_deadline_is_never_performed_on_resume() {
    let case = Case::new(
        "case-after-call-withheld-kept",
        vec![needs_approval(MERGE, CAPABILITY)],
    );
    let held_commission = commission(&case.id);
    let suspensions = SuspensionStore::new();
    let executor = Steps::new([(
        ExecutorOutcome::ProposedAction(proposed(MERGE, Value::Null)),
        None,
    )]);
    let effects = Effects::default();
    let authority = Sequenced::new([withheld()]);
    let mut context = Timed::past_first_call(deadline(DEADLINE), keep_step(), &executor);

    let end = run_with_suspension(
        &case,
        &executor,
        &authority,
        &effects,
        &held_commission,
        &mut runs(),
        &suspensions,
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    assert_eq!(end.outcome, needs_authority());
    assert!(effects.invoked().is_empty());
    assert_eq!(
        stored_state(&suspensions, &held_commission),
        Some(SuspensionState::Pending)
    );

    let resumed = resume_after_grant(
        &case,
        &executor,
        &Sequenced::new([allow()]),
        &effects,
        &held_commission,
        &mut runs(),
        &suspensions,
        &mut Context::new(deadline(DEADLINE), keep_step()).reading(AFTER),
    )
    .unwrap_or_else(|error| panic!("the resume failed: {error:?}"));

    assert_eq!(resumed.outcome, past_deadline());
    assert!(effects.invoked().is_empty());
    assert_eq!(
        stored_state(&suspensions, &held_commission),
        Some(SuspensionState::Pending)
    );
}

// A step the governor did not observe.

/// `case`, whose governor fails every observation.
struct ObservesNothing<'a> {
    case: &'a Case,
}

impl Governor for ObservesNothing<'_> {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.case.current_revision(case)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        self.case.frontier(case)
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.case.completion(case)
    }
}

impl ObservationPort for ObservesNothing<'_> {
    fn observe(
        &self,
        _observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        Err(GovernorError::GovernorUnavailable)
    }
}

/// A call past the deadline whose step the governor fails to observe fails the loop for the
/// governor, as with no deadline: the deadline is decided only once the step was observed, and
/// nothing the call proposed is performed.
#[test]
fn adversary_an_unobserved_call_past_the_deadline_fails_for_the_governor() {
    let case = Case::new("case-after-call-unobserved", vec![admissible(EVALUATE)]);
    let governor = ObservesNothing { case: &case };
    let executor = Steps::evaluations(2, None);
    let effects = Effects::default();
    let mut context = Timed::past_first_call(deadline(DEADLINE), default_policy(), &executor);

    let (end, _) = run_timed(
        &governor,
        &case.id,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &mut context,
    );

    let error = end.expect_err("an unobserved step fails the loop");
    assert!(
        matches!(
            error.failure,
            LoopFailure::Governor(GovernorError::GovernorUnavailable)
        ),
        "{:?}",
        error.failure
    );
    assert!(effects.invoked().is_empty());
    assert_eq!(executor.calls(), 1);
}
