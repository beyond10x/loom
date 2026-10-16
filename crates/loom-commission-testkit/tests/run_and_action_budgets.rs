//! Acceptance for run-wide and per-action budgets: a run carries a [`RunBudget`] from its
//! [`LoopContext`], every performed effect reports what it spent ([`EffectUsage`]), and the runtime
//! charges that to the run and to the request's action ([`LoopEnd::charges`]). A spending limit
//! suspends the Run for `SuspensionReason::RunBudget` once an effect's usage took the charge past
//! it; a trial limit suspends it before the request that would pass it is handed to the port. The
//! charges reach the run's usage report beside model-port usage ([`RunMeter::charge_effects`]).

use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::{Generated, RunStorage};
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionBudget, ActionCharge, ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId,
    AuthorityContext, BudgetLimit, CaseId, ChargedUsage, Commission, CommissionData, CommissionId,
    EffectOutcome, EffectOutcomePerformed, EffectUsage, ExecutorOutcome,
    ExecutorOutcomeProposedAction, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RunBudget, RunBudgetBreach, RunId, RunOutcome, RunOutcomeSuspended,
    RunState, SuspensionReason, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
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
/// was handed.
struct Effects {
    usage: Option<EffectUsage>,
    invoked: Mutex<VecDeque<ActionRequestData>>,
}

impl Effects {
    fn reporting(usage: Option<EffectUsage>) -> Self {
        Self {
            usage,
            invoked: Mutex::new(VecDeque::new()),
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
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Text("evaluated".to_owned()),
            attempt: None,
            audit: None,
            usage: self.usage.clone(),
        }))
    }
}

/// New ids from counters, one trusted time, no step budget and the run budget it was given.
struct Context {
    requests: u64,
    observations: u64,
    budget: RunBudget,
}

impl Context {
    fn with(budget: RunBudget) -> Self {
        Self {
            requests: 0,
            observations: 0,
            budget,
        }
    }
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x300 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x400 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-16T09:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }

    fn run_budget(&self) -> Option<RunBudget> {
        Some(self.budget.clone())
    }
}

fn unlimited() -> RunBudget {
    RunBudget {
        max_steps: None,
        max_tokens_in: None,
        max_tokens_out: None,
        max_duration_ms: None,
        max_cost_micro_usd: None,
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
