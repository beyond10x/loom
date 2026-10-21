//! Acceptance for run-wide and per-action budgets: a run carries a [`RunBudget`] from its
//! [`LoopContext`], every performed effect reports what it spent ([`EffectUsage`]), and the runtime
//! charges that to the run and to the request's action ([`LoopEnd::charges`]). A spending limit
//! suspends the Run for `SuspensionReason::RunBudget` once an effect's usage took the charge past
//! it; a trial limit suspends it before the request that would pass it is handed to the port. The
//! charges reach the run's usage report beside model-port usage ([`RunMeter::charge_effects`]).
//!
//! `story:run-budget-tokens-and-deadline`: a run may also be bounded by its input and output
//! tokens together (`max_tokens`) and by an RFC 3339 deadline read against the trusted time before
//! each step, and the host's [`StopPolicy`] chooses, in `on_breach`, whether an executor step whose
//! reported usage passed a spending limit is discarded (the default) or kept: acted on as with no
//! breach, the Run suspended for the breach before the next step.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::{Generated, RunStorage};
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionBudget, ActionCharge, ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId,
    AuthorityContext, AuthorityVerdict, AuthorityVerdictDeny, BreachStep, BudgetLimit, CaseId,
    ChargedUsage, Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomePending,
    EffectOutcomePerformed, EffectUsage, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ExecutorStep, ExecutorUsage, Frontier, FrontierAction, HoldSuspension, HoldSuspensionOutcome,
    ObservationId, PrincipalId, ProposedActionArguments, RunBudget, RunBudgetBreach, RunId,
    RunOutcome, RunOutcomeSuspended, RunOutcomeWaiting, RunState, StopPolicy, SuspensionKey,
    SuspensionReason, SuspensionState, commission_state, frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::suspension::SuspensionPort;
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, LoopError, LoopFailure, ResumeError, resume_after_grant,
    run_until_blocked,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_commission_testkit::file_suspension::FileSuspensionStore;
use b10x_loom_executor::metering::RunMeter;
use b10x_loom_executor::model::metering::ActionUsage;

const EVALUATE: &str = "evaluate";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x81)),
        agent_revision_id: AgentRevisionId(uuid(0x82)),
        case_id: case.clone(),
        principal: PrincipalId("principal-b".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// The case open at revision 1, its frontier listing `evaluate` admissible, on every call.
fn governor(case: &CaseId) -> FakeGovernor {
    governor_naming(case, None)
}

/// The case open at revision 1, on every call, its frontier listing `evaluate` admissible when no
/// capability is named; otherwise as needing approval for `capability`, beside an admissible
/// `inspect`, so a step whose proposal is denied leaves the run going on.
fn governor_naming(case: &CaseId, capability: Option<&str>) -> FakeGovernor {
    let entry = |action: &str, status, capability: Option<&str>| FrontierAction {
        action: action.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    };
    let actions = match capability {
        None => vec![entry(EVALUATE, ActionStatus::Admissible, None)],
        Some(capability) => vec![
            entry(EVALUATE, ActionStatus::ApprovalRequired, Some(capability)),
            entry("inspect", ActionStatus::Admissible, None),
        ],
    };
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [Answer::at(1).with_items(Vec::new(), Vec::new(), actions)],
    );
    governor
}

/// An executor over an agent session: each call proposes `evaluate` with arguments of its own, so
/// each is progress, and reports `usage` as what the call spent.
struct Session {
    usage: Option<ExecutorUsage>,
    calls: AtomicUsize,
}

impl Session {
    fn reporting(usage: Option<ExecutorUsage>) -> Self {
        Self {
            usage,
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl AgentExecutor for Session {
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
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        ExecutorStep {
            outcome: ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
                action: EVALUATE.to_owned(),
                arguments: ProposedActionArguments(Value::Number(call.to_string())),
            }),
            usage: self.usage.clone(),
            transcript_digest: None,
            summary: None,
        }
    }
}

/// What one executor call reports it spent: `input_tokens` in, `output_tokens` out.
fn call_usage(input_tokens: i64, output_tokens: i64) -> ExecutorUsage {
    ExecutorUsage {
        input_tokens,
        output_tokens,
        duration_ms: 500,
        cost_micro_usd: Some(3),
    }
}

/// The stop policy the context gives when the test sets none: the default.
fn default_policy() -> StopPolicy {
    StopPolicy {
        approval_gate: true,
        idle_bound: None,
        on_breach: BreachStep::DiscardStep,
    }
}

/// The default stop policy, keeping the step that passed a spending limit.
fn keep_step() -> StopPolicy {
    StopPolicy {
        on_breach: BreachStep::KeepStep,
        ..default_policy()
    }
}

/// The trusted time before the first executor call.
const BEFORE: &str = "2026-10-21T09:00:00Z";
/// The deadline the deadline test sets, between [`BEFORE`] and [`AFTER`].
const DEADLINE: &str = "2026-10-21T09:30:00Z";
/// `DEADLINE` in whole seconds since the Unix epoch.
const DEADLINE_SECONDS: i64 = 1_792_575_000;
/// The trusted time once the executor was called.
const AFTER: &str = "2026-10-21T10:00:00Z";
/// `AFTER` in whole seconds since the Unix epoch.
const AFTER_SECONDS: i64 = 1_792_576_800;

/// An executor proposing `evaluate` `n` times, each with its own arguments, so each is progress.
fn evaluations(n: u64) -> ScriptedExecutor {
    ScriptedExecutor::new((1..=n).map(|trial| {
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: EVALUATE.to_owned(),
            arguments: ProposedActionArguments(Value::Number(trial.to_string())),
        })
    }))
}

fn usage(input_tokens: i64, output_tokens: i64) -> EffectUsage {
    EffectUsage {
        input_tokens,
        output_tokens,
        duration_ms: 40,
        cost_micro_usd: Some(7),
    }
}

/// An effect port performing every action, each invocation reporting `usage`, recording what it
/// was handed; or, `pending`, answering every invocation `Pending`, with `ends_run` as given.
struct Effects {
    usage: Option<EffectUsage>,
    pending: bool,
    ends_run: bool,
    invoked: Mutex<VecDeque<ActionRequestData>>,
}

impl Effects {
    fn reporting(usage: Option<EffectUsage>) -> Self {
        Self {
            usage,
            pending: false,
            ends_run: false,
            invoked: Mutex::new(VecDeque::new()),
        }
    }

    fn pending() -> Self {
        Self {
            pending: true,
            ..Self::reporting(None)
        }
    }

    /// Answers every invocation `Pending` with `ends_run`.
    fn pending_ending() -> Self {
        Self {
            ends_run: true,
            ..Self::pending()
        }
    }

    fn invoked(&self) -> usize {
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

impl EffectPort for Effects {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(request.data().clone());
        if self.pending {
            return Ok(EffectOutcome::Pending(EffectOutcomePending {
                report: Value::Text("still evaluating".to_owned()),
                ends_run: self.ends_run,
            }));
        }
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Text("evaluated".to_owned()),
            attempt: None,
            audit: None,
            usage: self.usage.clone(),
        }))
    }
}

/// What the context's trusted clock reads.
enum Clock<'a> {
    /// One reading, always.
    Fixed(&'static str),
    /// [`BEFORE`] until the session was called once, [`AFTER`] from then on.
    PastDeadlineAfterFirstCall(&'a Session),
    /// Each reading in turn, the last one from then on.
    Readings(VecDeque<&'static str>),
}

/// New ids from counters, a trusted clock, no step budget, the run budget it was given and the
/// stop policy it was given.
struct Context<'a> {
    requests: u64,
    observations: u64,
    budget: RunBudget,
    clock: Clock<'a>,
    policy: StopPolicy,
}

impl<'a> Context<'a> {
    fn with(budget: RunBudget) -> Self {
        Self::reading(
            budget,
            Clock::Fixed("2026-10-16T09:00:00Z"),
            default_policy(),
        )
    }

    fn reading(budget: RunBudget, clock: Clock<'a>, policy: StopPolicy) -> Self {
        Self {
            requests: 0,
            observations: 0,
            budget,
            clock,
            policy,
        }
    }
}

impl LoopContext for Context<'_> {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x300 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x400 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        let reading = match &mut self.clock {
            Clock::Fixed(reading) => *reading,
            Clock::PastDeadlineAfterFirstCall(session) if session.calls() == 0 => BEFORE,
            Clock::PastDeadlineAfterFirstCall(_) => AFTER,
            Clock::Readings(readings) if readings.len() > 1 => {
                readings.pop_front().unwrap_or(AFTER)
            }
            Clock::Readings(readings) => readings.front().copied().unwrap_or(AFTER),
        };
        Timestamp(reading.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }

    fn run_budget(&self) -> Option<RunBudget> {
        Some(self.budget.clone())
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

fn action_budget() -> ActionBudget {
    ActionBudget {
        action: EVALUATE.to_owned(),
        max_trials: None,
        max_tokens_in: None,
        max_tokens_out: None,
        max_duration_ms: None,
        max_cost_micro_usd: None,
    }
}

fn run(
    executor: &ScriptedExecutor,
    effects: &Effects,
    budget: RunBudget,
) -> (LoopEnd, Generated<RunStore>) {
    let case = CaseId("case-budget".to_owned());
    let governor = governor(&case);
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x101))));
    let mut context = Context::with(budget);
    let end = run_until_blocked(
        &governor,
        executor,
        &StaticAuthorityProvider::new(),
        effects,
        &commission(&case),
        &mut runs,
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"));
    (end, runs)
}

/// `run_until_blocked` of `session` on [`governor_naming`] under `context`, asking `authority`,
/// with the run store.
fn run_session(
    session: &Session,
    effects: &Effects,
    capability: Option<&str>,
    authority: &StaticAuthorityProvider,
    context: &mut Context<'_>,
) -> (Result<LoopEnd, LoopError>, Generated<RunStore>) {
    let case = CaseId("case-budget-session".to_owned());
    let governor = governor_naming(&case, capability);
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x101))));
    let end = run_until_blocked(
        &governor,
        session,
        authority,
        effects,
        &commission(&case),
        &mut runs,
        context,
    );
    (end, runs)
}

fn suspended_for(breach: RunBudgetBreach) -> RunOutcome {
    RunOutcome::Suspended(RunOutcomeSuspended {
        reason: SuspensionReason::RunBudget(breach),
    })
}

fn charged(trials: i64, input_tokens: i64, output_tokens: i64) -> ChargedUsage {
    ChargedUsage {
        trials,
        input_tokens,
        output_tokens,
        duration_ms: 40 * trials,
        cost_micro_usd: Some(7 * trials),
    }
}

#[test]
fn an_effect_that_reports_usage_past_its_action_budget_stops_the_run() {
    let executor = evaluations(5);
    let effects = Effects::reporting(Some(usage(600, 20)));
    let budget = RunBudget {
        per_action: vec![ActionBudget {
            max_tokens_in: Some(1000),
            ..action_budget()
        }],
        ..unlimited()
    };

    let (end, runs) = run(&executor, &effects, budget);

    assert_eq!(
        end.outcome,
        suspended_for(RunBudgetBreach {
            action: Some(EVALUATE.to_owned()),
            limit: BudgetLimit::MaxTokensIn,
            max: 1000,
            charged: Some(1200),
        }),
        "the second performance breaches the action's input-token budget"
    );
    assert_eq!(effects.invoked(), 2, "no third request is invoked");
    assert_eq!(end.effects.len(), 2, "both performances are recorded");
    assert_eq!(
        RunStorage::get(&runs.ports, &end.run_id).map(|held| held.state),
        Some(RunState::Suspended),
        "the Run is suspended in the run store"
    );
    assert_eq!(end.charges.run, charged(2, 1200, 40), "charged to the run");
    assert_eq!(
        end.charges.actions,
        vec![ActionCharge {
            action: EVALUATE.to_owned(),
            charged: charged(2, 1200, 40),
        }],
        "charged to the action"
    );

    let meter = RunMeter::new(None);
    meter.charge_effects(&end.charges);
    let report = meter.report(&end.run_id).effects;
    assert_eq!(report.input_tokens, 1200, "the usage report's run total");
    assert_eq!(
        report.actions,
        vec![ActionUsage {
            action: EVALUATE.to_owned(),
            trials: 2,
            input_tokens: 1200,
            output_tokens: 40,
            duration_ms: 80,
            cost_micro_usd: Some(14),
        }],
        "the usage report's action line"
    );
}

#[test]
fn a_run_wide_spending_limit_stops_the_run_once_passed() {
    let executor = evaluations(5);
    let effects = Effects::reporting(Some(usage(10, 300)));
    let budget = RunBudget {
        max_tokens_out: Some(500),
        ..unlimited()
    };

    let (end, _) = run(&executor, &effects, budget);

    assert_eq!(
        end.outcome,
        suspended_for(RunBudgetBreach {
            action: None,
            limit: BudgetLimit::MaxTokensOut,
            max: 500,
            charged: Some(600),
        })
    );
    assert_eq!(effects.invoked(), 2);
}

#[test]
fn an_action_at_its_trial_limit_is_not_invoked_again() {
    let executor = evaluations(5);
    let effects = Effects::reporting(None);
    let budget = RunBudget {
        per_action: vec![ActionBudget {
            max_trials: Some(3),
            ..action_budget()
        }],
        ..unlimited()
    };

    let (end, _) = run(&executor, &effects, budget);

    assert_eq!(
        end.outcome,
        suspended_for(RunBudgetBreach {
            action: Some(EVALUATE.to_owned()),
            limit: BudgetLimit::MaxTrials,
            max: 3,
            charged: Some(3),
        })
    );
    assert_eq!(
        effects.invoked(),
        3,
        "the fourth request is never handed over"
    );
    assert_eq!(
        end.charges.run,
        ChargedUsage {
            trials: 3,
            input_tokens: 0,
            output_tokens: 0,
            duration_ms: 0,
            cost_micro_usd: Some(0),
        },
        "an effect that reports no usage charges a trial and nothing else"
    );
}

#[test]
fn a_cost_limit_suspends_once_an_effect_reports_usage_without_a_cost() {
    let executor = evaluations(5);
    let effects = Effects::reporting(Some(EffectUsage {
        cost_micro_usd: None,
        ..usage(1, 1)
    }));
    let budget = RunBudget {
        max_cost_micro_usd: Some(1_000_000),
        ..unlimited()
    };

    let (end, _) = run(&executor, &effects, budget);

    assert_eq!(
        end.outcome,
        suspended_for(RunBudgetBreach {
            action: None,
            limit: BudgetLimit::MaxCostMicroUsd,
            max: 1_000_000,
            charged: None,
        }),
        "an unknown cost under a cost limit fails toward less effect"
    );
    assert_eq!(effects.invoked(), 1);
}

/// `max_tokens` bounds the input and output tokens charged to the run together: a call reporting
/// 60 in and 50 out passes a limit of 100 that neither figure passes alone.
#[test]
fn max_tokens_counts_input_and_output_together() {
    let session = Session::reporting(Some(call_usage(60, 50)));
    let effects = Effects::reporting(None);
    let budget = RunBudget {
        max_tokens: Some(100),
        ..unlimited()
    };

    let (end, runs) = run_session(
        &session,
        &effects,
        None,
        &StaticAuthorityProvider::new(),
        &mut Context::reading(budget, Clock::Fixed(BEFORE), default_policy()),
    );
    let end = end.unwrap_or_else(|error| panic!("the loop failed: {error}"));

    assert_eq!(
        end.outcome,
        suspended_for(RunBudgetBreach {
            action: None,
            limit: BudgetLimit::MaxTokens,
            max: 100,
            charged: Some(110),
        }),
        "the first call's 110 tokens pass the run's total-token limit"
    );
    assert_eq!(session.calls(), 1, "no second executor call");
    assert_eq!(
        effects.invoked(),
        0,
        "under the default DiscardStep, the breaching step's proposal is not acted on"
    );
    assert_eq!(
        RunStorage::get(&runs.ports, &end.run_id).map(|held| held.state),
        Some(RunState::Suspended)
    );
}

/// A deadline is read against the trusted time before each executor call: once the clock reads
/// past it, the run is suspended for `Deadline` before the next step, naming the deadline and the
/// reading in whole seconds since the Unix epoch.
#[test]
fn a_passed_deadline_stops_the_run_before_the_next_step() {
    let session = Session::reporting(None);
    let effects = Effects::reporting(None);
    let budget = RunBudget {
        deadline: Some(Timestamp(DEADLINE.to_owned())),
        ..unlimited()
    };

    let (end, runs) = run_session(
        &session,
        &effects,
        None,
        &StaticAuthorityProvider::new(),
        &mut Context::reading(
            budget,
            Clock::PastDeadlineAfterFirstCall(&session),
            default_policy(),
        ),
    );
    let end = end.unwrap_or_else(|error| panic!("the loop failed: {error}"));

    assert_eq!(
        end.outcome,
        suspended_for(RunBudgetBreach {
            action: None,
            limit: BudgetLimit::Deadline,
            max: DEADLINE_SECONDS,
            charged: Some(AFTER_SECONDS),
        })
    );
    assert_eq!(session.calls(), 1, "the executor is called once");
    assert_eq!(
        effects.invoked(),
        1,
        "the first step, taken before the deadline, is performed"
    );
    assert_eq!(
        RunStorage::get(&runs.ports, &end.run_id).map(|held| held.state),
        Some(RunState::Suspended)
    );
}

/// A deadline that is not RFC 3339 is a loop error before the Run starts: no Run is stored, the
/// executor is never called and nothing is performed.
#[test]
fn a_deadline_that_is_not_rfc_3339_is_refused_before_the_run_starts() {
    let session = Session::reporting(None);
    let effects = Effects::reporting(None);
    let budget = RunBudget {
        deadline: Some(Timestamp("next tuesday".to_owned())),
        ..unlimited()
    };

    let (end, runs) = run_session(
        &session,
        &effects,
        None,
        &StaticAuthorityProvider::new(),
        &mut Context::reading(budget, Clock::Fixed(BEFORE), default_policy()),
    );

    let error = match end {
        Ok(end) => panic!("the loop ran: {:?}", end.outcome),
        Err(error) => error,
    };
    assert_eq!(error.run_id, None, "no Run was started");
    assert!(
        error.to_string().contains("next tuesday"),
        "the error names the deadline: {error}"
    );
    assert!(
        RunStorage::get(&runs.ports, &RunId(uuid(0x101))).is_none(),
        "the run store holds no Run"
    );
    assert_eq!(session.calls(), 0, "the executor is never called");
    assert_eq!(effects.invoked(), 0, "nothing is performed");
}

/// Under `on_breach: KeepStep`, the step whose reported usage passed a spending limit is acted on:
/// its proposal is performed once, and the run then ends suspended for the breach with no further
/// executor call.
#[test]
fn keep_step_acts_on_the_step_that_passed_a_spending_limit() {
    let session = Session::reporting(Some(call_usage(60, 50)));
    let effects = Effects::reporting(None);
    let budget = RunBudget {
        max_tokens: Some(100),
        ..unlimited()
    };

    let (end, runs) = run_session(
        &session,
        &effects,
        None,
        &StaticAuthorityProvider::new(),
        &mut Context::reading(budget, Clock::Fixed(BEFORE), keep_step()),
    );
    let end = end.unwrap_or_else(|error| panic!("the loop failed: {error}"));

    assert_eq!(
        end.outcome,
        suspended_for(RunBudgetBreach {
            action: None,
            limit: BudgetLimit::MaxTokens,
            max: 100,
            charged: Some(110),
        }),
        "the kept step's breach suspends the run before the next step"
    );
    assert_eq!(
        effects.invoked(),
        1,
        "the kept step's proposal is performed once"
    );
    assert_eq!(end.admitted.len(), 1);
    assert_eq!(session.calls(), 1, "no further executor call");
    assert_eq!(
        RunStorage::get(&runs.ports, &end.run_id).map(|held| held.state),
        Some(RunState::Suspended)
    );
}

/// A kept step is acted on as with no breach, and no more: its proposal of an action the provider
/// does not allow is asked about and never handed to the effect port, and the run is then
/// suspended for the breach.
#[test]
fn keep_step_still_refuses_an_unadmitted_proposal() {
    let session = Session::reporting(Some(call_usage(60, 50)));
    let effects = Effects::reporting(None);
    let authority = StaticAuthorityProvider::new().answer(
        EVALUATE,
        AuthorityVerdict::Deny(AuthorityVerdictDeny {
            reason: "no evaluations today".to_owned(),
        }),
    );
    let budget = RunBudget {
        max_tokens: Some(100),
        ..unlimited()
    };

    let (end, _) = run_session(
        &session,
        &effects,
        Some(EVALUATE),
        &authority,
        &mut Context::reading(budget, Clock::Fixed(BEFORE), keep_step()),
    );
    let end = end.unwrap_or_else(|error| panic!("the loop failed: {error}"));

    assert_eq!(
        authority.requests().len(),
        1,
        "the kept step's proposal is asked about"
    );
    assert_eq!(
        effects.invoked(),
        0,
        "a proposal the provider does not allow is never handed to the effect port"
    );
    assert!(end.admitted.is_empty());
    assert_eq!(
        end.outcome,
        suspended_for(RunBudgetBreach {
            action: None,
            limit: BudgetLimit::MaxTokens,
            max: 100,
            charged: Some(110),
        }),
        "the run would go on, so it is suspended for the breach before the next step"
    );
    assert_eq!(session.calls(), 1, "no further executor call");
}

/// A deadline reached while a request is pending is read before the request is invoked again: the
/// run ends `Waiting`, naming the request and the deadline, and the request is not invoked again.
#[test]
fn a_deadline_reached_while_a_request_is_pending_ends_the_run_waiting() {
    let session = Session::reporting(None);
    let effects = Effects::pending();
    let budget = RunBudget {
        deadline: Some(Timestamp(DEADLINE.to_owned())),
        ..unlimited()
    };

    let (end, runs) = run_session(
        &session,
        &effects,
        None,
        &StaticAuthorityProvider::new(),
        &mut Context::reading(
            budget,
            Clock::PastDeadlineAfterFirstCall(&session),
            default_policy(),
        ),
    );
    let end = end.unwrap_or_else(|error| panic!("the loop failed: {error}"));

    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![ActionRequestId(uuid(0x301))],
            reason: SuspensionReason::RunBudget(RunBudgetBreach {
                action: None,
                limit: BudgetLimit::Deadline,
                max: DEADLINE_SECONDS,
                charged: Some(AFTER_SECONDS),
            }),
        })
    );
    assert_eq!(
        effects.invoked(),
        1,
        "the pending request is not invoked again"
    );
    assert_eq!(session.calls(), 1);
    assert_eq!(
        RunStorage::get(&runs.ports, &end.run_id).map(|held| held.state),
        Some(RunState::Suspended)
    );
}

/// A clock reading that is not RFC 3339 counts as past the deadline, with no charge known: the
/// run is suspended for `Deadline` before the first executor call.
#[test]
fn a_reading_that_is_not_rfc_3339_counts_as_past_the_deadline() {
    let session = Session::reporting(None);
    let effects = Effects::reporting(None);
    let budget = RunBudget {
        deadline: Some(Timestamp(DEADLINE.to_owned())),
        ..unlimited()
    };

    let (end, _) = run_session(
        &session,
        &effects,
        None,
        &StaticAuthorityProvider::new(),
        &mut Context::reading(budget, Clock::Fixed("half past nine"), default_policy()),
    );
    let end = end.unwrap_or_else(|error| panic!("the loop failed: {error}"));

    assert_eq!(
        end.outcome,
        suspended_for(RunBudgetBreach {
            action: None,
            limit: BudgetLimit::Deadline,
            max: DEADLINE_SECONDS,
            charged: None,
        })
    );
    assert_eq!(session.calls(), 0, "the executor is never called");
    assert_eq!(effects.invoked(), 0);
}

/// A deadline given with an offset is the instant it names: a reading before that instant takes
/// the step, though the reading's written time of day is past the deadline's.
#[test]
fn a_deadline_with_an_offset_is_the_instant_it_names() {
    let session = Session::reporting(Some(call_usage(1, 1)));
    let effects = Effects::reporting(None);
    let budget = RunBudget {
        // 09:30 UTC, written 08:30; the clock reads 09:00 UTC throughout.
        deadline: Some(Timestamp("2026-10-21T08:30:00-01:00".to_owned())),
        max_tokens: Some(3),
        ..unlimited()
    };

    let (end, _) = run_session(
        &session,
        &effects,
        None,
        &StaticAuthorityProvider::new(),
        &mut Context::reading(budget, Clock::Fixed(BEFORE), default_policy()),
    );
    let end = end.unwrap_or_else(|error| panic!("the loop failed: {error}"));

    assert_eq!(
        end.outcome,
        suspended_for(RunBudgetBreach {
            action: None,
            limit: BudgetLimit::MaxTokens,
            max: 3,
            charged: Some(4),
        }),
        "the deadline never stops the run; the second call's tokens do"
    );
    assert_eq!(session.calls(), 2);
    assert_eq!(effects.invoked(), 1);
}

/// A resume under a deadline that is not RFC 3339 is refused before it takes the Suspension: the
/// Suspension stays `Pending`, and nothing is performed.
#[test]
fn a_resume_under_a_deadline_that_is_not_rfc_3339_takes_nothing() {
    let case = CaseId("case-budget-resume".to_owned());
    let commission = commission(&case);
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("run-and-action-budgets")
        .join("resume-invalid-deadline");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    let suspensions = FileSuspensionStore::open(&dir).expect("the suspension store opens");
    let commission_id = commission.data().commission_id.clone();
    let held = suspensions.hold(HoldSuspension {
        suspension: SuspensionKey {
            commission_id: commission_id.clone(),
        },
        commission_id: commission_id.clone(),
        run_id: RunId(uuid(0x101)),
        run_case_revision: 1,
        action_request_id: ActionRequestId(uuid(0x301)),
        case_id: case.clone(),
        expected_case_revision: 1,
        action: EVALUATE.to_owned(),
        arguments: ProposedActionArguments(Value::Null),
        reason: SuspensionReason::Authority(Value::Null),
    });
    assert!(matches!(held, HoldSuspensionOutcome::Held { .. }));
    let session = Session::reporting(None);
    let effects = Effects::reporting(None);
    let budget = RunBudget {
        deadline: Some(Timestamp("tomorrow at noon".to_owned())),
        ..unlimited()
    };
    let mut runs = RunStore::new(|| RunId(uuid(0x102)));

    let resumed = resume_after_grant(
        &governor(&case),
        &session,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission,
        &mut runs,
        &suspensions,
        &mut Context::reading(budget, Clock::Fixed(BEFORE), default_policy()),
    );

    match resumed {
        Err(ResumeError::Loop(error)) => assert_eq!(
            error.failure,
            LoopFailure::InvalidDeadline("tomorrow at noon".to_owned())
        ),
        other => panic!("the resume was not refused for its deadline: {other:?}"),
    }
    let stored = suspensions
        .get(&commission_id)
        .expect("the store reads")
        .expect("the Suspension is still stored");
    assert_eq!(stored.state, SuspensionState::Pending, "nothing was taken");
    assert_eq!(effects.invoked(), 0, "nothing is performed");
    assert_eq!(session.calls(), 0);
}

/// A suspension store under the test's own directory `name`, holding `commission`'s Suspension of
/// an `evaluate` request at revision 1, the request `0x301` of the Run `0x101`.
fn holding(name: &str, commission: &Commission<commission_state::Assigned>) -> FileSuspensionStore {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("run-and-action-budgets")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    let suspensions = FileSuspensionStore::open(&dir).expect("the suspension store opens");
    let commission_id = commission.data().commission_id.clone();
    let held = suspensions.hold(HoldSuspension {
        suspension: SuspensionKey {
            commission_id: commission_id.clone(),
        },
        commission_id,
        run_id: RunId(uuid(0x101)),
        run_case_revision: 1,
        action_request_id: ActionRequestId(uuid(0x301)),
        case_id: commission.data().case_id.clone(),
        expected_case_revision: 1,
        action: EVALUATE.to_owned(),
        arguments: ProposedActionArguments(Value::Null),
        reason: SuspensionReason::Authority(Value::Null),
    });
    assert!(matches!(held, HoldSuspensionOutcome::Held { .. }));
    suspensions
}

/// A resume past its run budget's deadline performs nothing, as a passed deadline stops a run
/// before an executor call: the resume ends suspended for `Deadline`. Read past it before the
/// take, the resume takes nothing and the Suspension stays `Pending`; read before it then, and
/// past it right before the held request is invoked, the Suspension the resume took is released.
/// Either way the effect port is never invoked and no executor runs.
#[test]
fn a_resume_past_its_deadline_performs_nothing() {
    let passed = suspended_for(RunBudgetBreach {
        action: None,
        limit: BudgetLimit::Deadline,
        max: DEADLINE_SECONDS,
        charged: Some(AFTER_SECONDS),
    });
    let resumed = |name: &str, clock: Clock<'static>| {
        let case = CaseId(format!("case-budget-{name}"));
        let commission = commission(&case);
        let suspensions = holding(name, &commission);
        let session = Session::reporting(None);
        let effects = Effects::reporting(None);
        let budget = RunBudget {
            deadline: Some(Timestamp(DEADLINE.to_owned())),
            ..unlimited()
        };
        let mut runs = RunStore::new(|| RunId(uuid(0x102)));
        let end = resume_after_grant(
            &governor(&case),
            &session,
            &StaticAuthorityProvider::new(),
            &effects,
            &commission,
            &mut runs,
            &suspensions,
            &mut Context::reading(budget, clock, default_policy()),
        );
        let stored = suspensions
            .get(&commission.data().commission_id)
            .expect("the store reads");
        let run = RunStorage::get(&runs, &RunId(uuid(0x101))).map(|held| held.state);
        (
            end,
            stored.map(|held| held.state),
            run,
            effects.invoked(),
            session.calls(),
        )
    };

    let (end, stored, run, invoked, calls) =
        resumed("resume-past-deadline-before-take", Clock::Fixed(AFTER));
    let end = end.unwrap_or_else(|error| panic!("the resume failed: {error:?}"));
    assert_eq!(end.outcome, passed, "ended as a passed deadline ends a run");
    assert_eq!(invoked, 0, "nothing is performed");
    assert_eq!(
        stored,
        Some(SuspensionState::Pending),
        "nothing was taken: the Suspension stays pending"
    );
    assert_eq!(run, Some(RunState::Suspended));
    assert_eq!(calls, 0, "no executor runs");

    let (end, stored, run, invoked, calls) = resumed(
        "resume-past-deadline-after-take",
        Clock::Readings(VecDeque::from([BEFORE, AFTER])),
    );
    let end = end.unwrap_or_else(|error| panic!("the resume failed: {error:?}"));
    assert_eq!(end.outcome, passed, "ended as a passed deadline ends a run");
    assert_eq!(invoked, 0, "nothing is performed");
    assert_eq!(stored, None, "the Suspension the resume took is released");
    assert_eq!(run, Some(RunState::Suspended));
    assert_eq!(calls, 0, "no executor runs");
}

/// Under `on_breach: KeepStep`, a kept step's proposal answered `Pending` with `ends_run` is not
/// invoked again, and the breach the step kept ends the run first: `Waiting`, naming the request,
/// for the breach.
#[test]
fn keep_step_ends_a_run_waiting_on_an_ending_pending_answer_for_its_breach() {
    let session = Session::reporting(Some(call_usage(60, 50)));
    let effects = Effects::pending_ending();
    let budget = RunBudget {
        max_tokens: Some(100),
        ..unlimited()
    };

    let (end, runs) = run_session(
        &session,
        &effects,
        None,
        &StaticAuthorityProvider::new(),
        &mut Context::reading(budget, Clock::Fixed(BEFORE), keep_step()),
    );
    let end = end.unwrap_or_else(|error| panic!("the loop failed: {error}"));

    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![ActionRequestId(uuid(0x301))],
            reason: SuspensionReason::RunBudget(RunBudgetBreach {
                action: None,
                limit: BudgetLimit::MaxTokens,
                max: 100,
                charged: Some(110),
            }),
        })
    );
    assert_eq!(effects.invoked(), 1, "not invoked again");
    assert_eq!(session.calls(), 1);
    assert_eq!(
        RunStorage::get(&runs.ports, &end.run_id).map(|held| held.state),
        Some(RunState::Suspended)
    );
}
