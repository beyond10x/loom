//! Acceptance for `story:executor-reports-usage`: an executor that is not a model loop (an agent
//! session, a remote executor) reports what each call spent ([`ExecutorUsage`], through
//! [`AgentExecutor::run_with_usage`]), and the runtime charges it to the run's charges and budget
//! as it charges a performed effect's usage ([`LoopEnd::charges`], `ExecutorStep` in
//! `ess/commission/domains/responsibility.yaml`). A charge past a spending limit suspends the Run
//! for `SuspensionReason::RunBudget` once the step was observed, before its outcome is acted on.

use std::sync::atomic::{AtomicUsize, Ordering};

use b10x_loom_commission::model::behaviour::{Generated, RunStorage};
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionCharge, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, BudgetLimit,
    CaseId, ChargedUsage, Commission, CommissionData, CommissionId, EffectOutcome,
    EffectOutcomePerformed, ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorStep,
    ExecutorUsage, Frontier, FrontierAction, ObservationId, PrincipalId, ProposedActionArguments,
    RunBudget, RunBudgetBreach, RunId, RunOutcome, RunOutcomeSuspended, RunState, SuspensionReason,
    commission_state, frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::runtime::{EXECUTOR_SOURCE, LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

const EVALUATE: &str = "evaluate";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x91)),
        agent_revision_id: AgentRevisionId(uuid(0x92)),
        case_id: case.clone(),
        principal: PrincipalId("principal-c".to_owned()),
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

/// What each call of [`Session`] reports it spent, with the cost given.
fn spent(cost_micro_usd: Option<i64>) -> ExecutorUsage {
    ExecutorUsage {
        input_tokens: 1000,
        output_tokens: 50,
        duration_ms: 2000,
        cost_micro_usd,
    }
}

/// An executor over an agent session, not a model loop: each call proposes `evaluate` with
/// arguments of its own, so each is progress, and reports `usage` as what the call spent.
struct Session {
    usage: ExecutorUsage,
    calls: AtomicUsize,
}

impl Session {
    fn reporting(usage: ExecutorUsage) -> Self {
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
            usage: Some(self.usage.clone()),
            transcript_digest: None,
            summary: None,
        }
    }
}

/// Performs every action and reports no usage of its own, counting what it was handed.
#[derive(Default)]
struct Effects {
    invoked: AtomicUsize,
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
            usage: None,
        }))
    }
}

/// New ids from counters, one trusted time, a step budget of three and the run budget given.
struct Context {
    requests: u64,
    observations: u64,
    budget: RunBudget,
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
        Timestamp("2026-10-20T09:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        Some(3)
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

/// `run_until_blocked` of `executor` on [`governor`] under `budget`, with the governor it observed
/// through and the run store.
fn run(
    executor: &Session,
    effects: &Effects,
    budget: RunBudget,
) -> (LoopEnd, FakeGovernor, Generated<RunStore>) {
    let case = CaseId("case-executor-usage".to_owned());
    let governor = governor(&case);
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x101))));
    let mut context = Context {
        requests: 0,
        observations: 0,
        budget,
    };
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
    (end, governor, runs)
}

#[test]
fn an_executor_usage_is_charged_to_the_run() {
    let executor = Session::reporting(spent(Some(300)));
    let effects = Effects::default();

    let (end, governor, _) = run(&executor, &effects, unlimited());

    assert_eq!(
        executor.calls(),
        3,
        "three executor calls: {:?}",
        end.outcome
    );
    assert_eq!(
        end.charges.run,
        ChargedUsage {
            trials: 3,
            input_tokens: 3000,
            output_tokens: 150,
            duration_ms: 6000,
            cost_micro_usd: Some(900),
        },
        "each call's usage is charged to the run, beside the trials its requests took"
    );
    assert_eq!(
        end.charges.actions,
        vec![ActionCharge {
            action: EVALUATE.to_owned(),
            charged: ChargedUsage {
                trials: 3,
                input_tokens: 0,
                output_tokens: 0,
                duration_ms: 0,
                cost_micro_usd: Some(0),
            },
        }],
        "an executor call is no trial and is charged to no action"
    );
    let reported: Vec<Value> = governor
        .observations()
        .into_iter()
        .filter(|observation| observation.source == EXECUTOR_SOURCE)
        .filter_map(|observation| match observation.payload {
            Value::Object(fields) => fields
                .into_iter()
                .find(|(name, _)| name == "usage")
                .map(|(_, usage)| usage),
            _ => None,
        })
        .collect();
    let figure = |figure: i64| Value::Number(figure.to_string());
    assert_eq!(
        reported,
        vec![
            Value::Object(vec![
                ("input_tokens".to_owned(), figure(1000)),
                ("output_tokens".to_owned(), figure(50)),
                ("duration_ms".to_owned(), figure(2000)),
                ("cost_micro_usd".to_owned(), figure(300)),
            ]);
            3
        ],
        "each step's observation carries the usage its call reported"
    );
}

#[test]
fn a_run_budget_breach_by_executor_usage_suspends_the_run() {
    let executor = Session::reporting(spent(Some(300)));
    let effects = Effects::default();
    let budget = RunBudget {
        max_tokens_in: Some(2500),
        ..unlimited()
    };

    let (end, _, runs) = run(&executor, &effects, budget);

    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended {
            reason: SuspensionReason::RunBudget(RunBudgetBreach {
                action: None,
                limit: BudgetLimit::MaxTokensIn,
                max: 2500,
                charged: Some(3000),
            }),
        }),
        "the third call's usage takes the run past its input-token budget"
    );
    assert_eq!(
        RunStorage::get(&runs.ports, &end.run_id).map(|held| held.state),
        Some(RunState::Suspended),
        "the Run is suspended in the run store, as an effect's breach suspends it"
    );
    assert_eq!(executor.calls(), 3);
    assert_eq!(
        effects.invoked.load(Ordering::SeqCst),
        2,
        "the breaching step's proposal is not acted on: no third request is invoked"
    );
    assert_eq!(end.admitted.len(), 2);
    assert_eq!(
        end.requests.len(),
        2,
        "no request is made from the breaching step"
    );
    assert_eq!(end.charges.run.input_tokens, 3000);
}

#[test]
fn an_unknown_cost_stays_unknown() {
    let executor = Session::reporting(spent(None));
    let effects = Effects::default();

    let (end, _, _) = run(&executor, &effects, unlimited());

    assert_eq!(executor.calls(), 3);
    assert_eq!(
        end.charges.run.input_tokens, 3000,
        "the tokens are still charged"
    );
    assert_eq!(
        end.charges.run.cost_micro_usd, None,
        "a usage with no cost leaves the run's cost absent, never zero"
    );
}
