//! Adversary pass over `story:breach-choice-per-limit` (wave 2026-10-22-w1).
//!
//! `StopPolicy.breach_by_limit` chooses, per run limit, whether an executor step that breached it
//! is kept or discarded. These cases hold the choice against the count limits it must not touch
//! (`MaxSteps`, `MaxTrials`), against the order it lists limits in, against an executor outcome
//! that ends the run by itself, against an action's own limit, which an effect breaches and which
//! no step choice reaches, and against two spending limits of which only the second discards.

use std::sync::atomic::{AtomicUsize, Ordering};

use b10x_loom_commission::model::behaviour::{Generated, RunStorage};
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionBudget, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, BreachStep,
    BudgetLimit, CaseId, Commission, CommissionData, CommissionId, EffectOutcome,
    EffectOutcomePerformed, EffectUsage, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ExecutorOutcomeSuspended, ExecutorStep, ExecutorUsage, Frontier, FrontierAction,
    LimitBreachStep, ObservationId, PrincipalId, ProposedActionArguments, RunBudget,
    RunBudgetBreach, RunId, RunOutcome, RunOutcomeSuspended, RunState, StopPolicy,
    SuspensionReason, commission_state, frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, LoopError, LoopFailure, run_until_blocked,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

const EVALUATE: &str = "evaluate";

/// The trusted time before the executor was called, and after.
const BEFORE: &str = "2026-10-22T09:00:00Z";
const AFTER: &str = "2026-10-22T10:00:00Z";
const DEADLINE: &str = "2026-10-22T09:30:00Z";
const DEADLINE_SECONDS: i64 = 1_792_661_400;
const AFTER_SECONDS: i64 = 1_792_663_200;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xa1)),
        agent_revision_id: AgentRevisionId(uuid(0xa2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adversary-b".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// The case open at revision 1, its frontier listing `evaluate` admissible on every call.
fn admitting_evaluate(case: &CaseId) -> FakeGovernor {
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [Answer::at(1).with_items(
            Vec::new(),
            Vec::new(),
            vec![FrontierAction {
                action: EVALUATE.to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            }],
        )],
    );
    governor
}

/// What each executor call answers.
#[derive(Clone, Copy)]
enum Answering {
    /// `evaluate`, with the call's number as its arguments.
    Proposes,
    /// Its own suspension, for an external availability.
    Suspends,
}

/// An executor over an agent session that reports `input_tokens` in and 10 out for each call.
struct Session {
    answering: Answering,
    input_tokens: i64,
    calls: AtomicUsize,
}

impl Session {
    fn spending(input_tokens: i64) -> Self {
        Self {
            answering: Answering::Proposes,
            input_tokens,
            calls: AtomicUsize::new(0),
        }
    }

    fn suspending(input_tokens: i64) -> Self {
        Self {
            answering: Answering::Suspends,
            ..Self::spending(input_tokens)
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

fn unavailable() -> SuspensionReason {
    SuspensionReason::ExternalAvailability(Value::Text("the session's sandbox is gone".to_owned()))
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
        let outcome = match self.answering {
            Answering::Proposes => ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
                action: EVALUATE.to_owned(),
                arguments: ProposedActionArguments(Value::Number(call.to_string())),
            }),
            Answering::Suspends => ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
                reason: unavailable(),
            }),
        };
        ExecutorStep {
            outcome,
            usage: Some(ExecutorUsage {
                input_tokens: self.input_tokens,
                output_tokens: 10,
                duration_ms: 500,
                cost_micro_usd: Some(3),
            }),
            transcript_digest: None,
            summary: None,
        }
    }
}

/// An effect port that performs every action, reporting `usage` as what each effect spent.
struct Effects {
    usage: Option<EffectUsage>,
    invoked: AtomicUsize,
}

impl Effects {
    fn spending(usage: Option<EffectUsage>) -> Self {
        Self {
            usage,
            invoked: AtomicUsize::new(0),
        }
    }

    fn invoked(&self) -> usize {
        self.invoked.load(Ordering::SeqCst)
    }
}

impl EffectPort for Effects {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.invoked.fetch_add(1, Ordering::SeqCst);
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Text("evaluated".to_owned()),
            attempt: None,
            audit: None,
            usage: self.usage.clone(),
        }))
    }
}

/// New ids from counters, the run budget and stop policy given, no step budget, and a trusted
/// clock reading [`BEFORE`], or [`AFTER`] once `past_deadline_after` was called.
struct Context<'a> {
    requests: u64,
    observations: u64,
    budget: RunBudget,
    policy: StopPolicy,
    past_deadline_after: Option<&'a Session>,
}

impl<'a> Context<'a> {
    fn new(budget: RunBudget, policy: StopPolicy) -> Self {
        Self {
            requests: 0,
            observations: 0,
            budget,
            policy,
            past_deadline_after: None,
        }
    }

    fn past_deadline_after(self, session: &'a Session) -> Self {
        Self {
            past_deadline_after: Some(session),
            ..self
        }
    }
}

impl LoopContext for Context<'_> {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x500 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x600 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        let past = self
            .past_deadline_after
            .is_some_and(|session| session.calls() > 0);
        Timestamp(if past { AFTER } else { BEFORE }.to_owned())
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

fn no_limits() -> RunBudget {
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

fn per_limit(on_breach: BreachStep, listed: &[(BudgetLimit, BreachStep)]) -> StopPolicy {
    StopPolicy {
        approval_gate: true,
        idle_bound: None,
        on_breach,
        breach_by_limit: listed
            .iter()
            .map(|&(limit, step)| LimitBreachStep { limit, step })
            .collect(),
    }
}

fn run(
    name: &str,
    session: &Session,
    effects: &Effects,
    context: &mut Context<'_>,
) -> (Result<LoopEnd, LoopError>, Generated<RunStore>) {
    let case = CaseId(format!("case-adversary-b-{name}"));
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x111))));
    let end = run_until_blocked(
        &admitting_evaluate(&case),
        session,
        &StaticAuthorityProvider::new(),
        effects,
        &commission(&case),
        &mut runs,
        context,
    );
    (end, runs)
}

fn ended(end: Result<LoopEnd, LoopError>) -> LoopEnd {
    end.unwrap_or_else(|error| panic!("the loop failed: {error}"))
}

fn breach(action: Option<&str>, limit: BudgetLimit, max: i64, charged: i64) -> RunOutcome {
    RunOutcome::Suspended(RunOutcomeSuspended {
        reason: SuspensionReason::RunBudget(RunBudgetBreach {
            action: action.map(str::to_owned),
            limit,
            max,
            charged: Some(charged),
        }),
    })
}

/// Every way of choosing for the count limits: not listing them, listing them kept under a
/// discarding default, and listing them discarded under a keeping default.
fn count_limit_policies() -> Vec<(&'static str, StopPolicy)> {
    let counts = |step| {
        [
            (BudgetLimit::MaxSteps, step),
            (BudgetLimit::MaxTrials, step),
        ]
    };
    vec![
        ("default", per_limit(BreachStep::DiscardStep, &[])),
        ("keeping", per_limit(BreachStep::KeepStep, &[])),
        (
            "counts-kept",
            per_limit(BreachStep::DiscardStep, &counts(BreachStep::KeepStep)),
        ),
        (
            "counts-discarded",
            per_limit(BreachStep::KeepStep, &counts(BreachStep::DiscardStep)),
        ),
    ]
}

/// The design: "MaxSteps and MaxTrials ... listing them is allowed and changes nothing". A run
/// budget of one step ends the run after that step, however the policy chooses for the count
/// limits.
#[test]
fn adversary_listing_max_steps_changes_nothing() {
    for (name, policy) in count_limit_policies() {
        let session = Session::spending(10);
        let effects = Effects::spending(None);
        let budget = RunBudget {
            max_steps: Some(1),
            ..no_limits()
        };
        let mut context = Context::new(budget, policy);

        let (end, _) = run(
            &format!("max-steps-{name}"),
            &session,
            &effects,
            &mut context,
        );

        assert_eq!(
            (ended(end).outcome, session.calls(), effects.invoked()),
            (breach(None, BudgetLimit::MaxSteps, 1, 1), 1, 1),
            "under the {name} policy"
        );
    }
}

/// The same for `MaxTrials`: an action allowed one trial is performed once, and the next proposal
/// of it ends the run before it is invoked, however the policy chooses for the count limits.
#[test]
fn adversary_listing_max_trials_changes_nothing() {
    for (name, policy) in count_limit_policies() {
        let session = Session::spending(10);
        let effects = Effects::spending(None);
        let budget = RunBudget {
            per_action: vec![ActionBudget {
                action: EVALUATE.to_owned(),
                max_trials: Some(1),
                max_tokens_in: None,
                max_tokens_out: None,
                max_duration_ms: None,
                max_cost_micro_usd: None,
            }],
            ..no_limits()
        };
        let mut context = Context::new(budget, policy);

        let (end, _) = run(
            &format!("max-trials-{name}"),
            &session,
            &effects,
            &mut context,
        );

        assert_eq!(
            (ended(end).outcome, session.calls(), effects.invoked()),
            (breach(Some(EVALUATE), BudgetLimit::MaxTrials, 1, 1), 2, 1),
            "under the {name} policy"
        );
    }
}

/// When every breached limit keeps the step, the Run is suspended for the first breached limit in
/// the order the runtime checks them (spending limits, then the deadline), not in the order the
/// policy lists them.
#[test]
fn adversary_the_kept_reason_follows_the_check_order_not_the_listed_order() {
    let session = Session::spending(60);
    let effects = Effects::spending(None);
    let budget = RunBudget {
        max_tokens_in: Some(50),
        deadline: Some(Timestamp(DEADLINE.to_owned())),
        ..no_limits()
    };
    let policy = per_limit(
        BreachStep::DiscardStep,
        &[
            (BudgetLimit::Deadline, BreachStep::KeepStep),
            (BudgetLimit::MaxTokensIn, BreachStep::KeepStep),
        ],
    );
    let mut context = Context::new(budget, policy).past_deadline_after(&session);

    let (end, runs) = run("kept-order", &session, &effects, &mut context);
    let end = ended(end);

    assert_eq!(end.outcome, breach(None, BudgetLimit::MaxTokensIn, 50, 60));
    assert_eq!(effects.invoked(), 1, "the kept step is performed");
    assert_eq!(session.calls(), 1);
    assert_eq!(
        RunStorage::get(&runs.ports, &end.run_id).map(|held| held.state),
        Some(RunState::Suspended)
    );
    assert_ne!(
        end.outcome,
        breach(None, BudgetLimit::Deadline, DEADLINE_SECONDS, AFTER_SECONDS),
        "the deadline is the second breach"
    );
}

/// Two spending limits breached by one call, the first kept and the second discarded: the step is
/// discarded and the Run suspended for the second, the first that discards.
#[test]
fn adversary_a_later_spending_limit_that_discards_is_the_reason() {
    let session = Session::spending(60);
    let effects = Effects::spending(None);
    let budget = RunBudget {
        max_tokens_in: Some(50),
        max_tokens: Some(50),
        ..no_limits()
    };
    let policy = per_limit(
        BreachStep::DiscardStep,
        &[(BudgetLimit::MaxTokensIn, BreachStep::KeepStep)],
    );
    let mut context = Context::new(budget, policy);

    let (end, _) = run("later-discards", &session, &effects, &mut context);

    assert_eq!(
        ended(end).outcome,
        breach(None, BudgetLimit::MaxTokens, 50, 70)
    );
    assert_eq!(effects.invoked(), 0, "the discarded step proposes nothing");
}

/// A kept step whose executor suspended itself ends the run for the executor's own reason: the
/// outcome ends the run before the kept breach would.
#[test]
fn adversary_a_kept_step_that_suspends_itself_ends_for_the_executors_reason() {
    let session = Session::suspending(60);
    let effects = Effects::spending(None);
    let budget = RunBudget {
        max_tokens_in: Some(50),
        ..no_limits()
    };
    let policy = per_limit(
        BreachStep::DiscardStep,
        &[(BudgetLimit::MaxTokensIn, BreachStep::KeepStep)],
    );
    let mut context = Context::new(budget, policy);

    let (end, runs) = run("kept-suspends", &session, &effects, &mut context);
    let end = ended(end);

    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended {
            reason: unavailable()
        })
    );
    assert_eq!(effects.invoked(), 0);
    assert_eq!(
        RunStorage::get(&runs.ports, &end.run_id).map(|held| held.state),
        Some(RunState::Suspended)
    );
}

/// An action's own spending limit, breached by its effect, ends the run once the effect is
/// observed whatever the policy chooses for that kind of limit: the choice is an executor step's.
#[test]
fn adversary_an_effects_breach_of_its_actions_limit_ignores_the_step_choice() {
    let session = Session::spending(1);
    let effects = Effects::spending(Some(EffectUsage {
        input_tokens: 10,
        output_tokens: 0,
        duration_ms: 1,
        cost_micro_usd: Some(1),
    }));
    let budget = RunBudget {
        per_action: vec![ActionBudget {
            action: EVALUATE.to_owned(),
            max_trials: None,
            max_tokens_in: Some(5),
            max_tokens_out: None,
            max_duration_ms: None,
            max_cost_micro_usd: None,
        }],
        ..no_limits()
    };
    let policy = per_limit(
        BreachStep::KeepStep,
        &[(BudgetLimit::MaxTokensIn, BreachStep::KeepStep)],
    );
    let mut context = Context::new(budget, policy);

    let (end, _) = run("action-limit", &session, &effects, &mut context);

    assert_eq!(
        ended(end).outcome,
        breach(Some(EVALUATE), BudgetLimit::MaxTokensIn, 5, 10)
    );
    assert_eq!((session.calls(), effects.invoked()), (1, 1));
}

/// A limit listed twice with the same choice is still listed twice: refused before the Run starts,
/// naming the limit.
#[test]
fn adversary_a_limit_listed_twice_with_the_same_choice_is_refused() {
    let session = Session::spending(10);
    let effects = Effects::spending(None);
    let policy = per_limit(
        BreachStep::DiscardStep,
        &[
            (BudgetLimit::MaxCostMicroUsd, BreachStep::KeepStep),
            (BudgetLimit::MaxCostMicroUsd, BreachStep::KeepStep),
        ],
    );
    let mut context = Context::new(no_limits(), policy);

    let (end, runs) = run("twice-same", &session, &effects, &mut context);

    let error = match end {
        Ok(end) => panic!("the loop ran: {:?}", end.outcome),
        Err(error) => error,
    };
    assert!(
        matches!(
            error.failure,
            LoopFailure::BreachLimitListedTwice(BudgetLimit::MaxCostMicroUsd)
        ),
        "{error}"
    );
    assert_eq!(error.run_id, None);
    assert!(RunStorage::get(&runs.ports, &RunId(uuid(0x111))).is_none());
    assert_eq!((session.calls(), effects.invoked()), (0, 0));
}
