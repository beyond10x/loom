//! Adversary pass 1 on `story:executor-reports-usage`: the run meter against an executor's
//! reported usage, read from `crates/loom-executor/src/metering.rs`'s module doc,
//! `ess/domains/metering.yaml` and the CHANGELOG entry. Nothing calls a model or the network.

use std::sync::atomic::{AtomicU64, Ordering};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid as CommissionUuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, CompletionDetermination, EffectOutcome, EffectOutcomePerformed,
    ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorStep, ExecutorUsage, Frontier,
    FrontierAction, FrontierData, FrontierId, GovernorError, Observation, ObservationId,
    PrincipalId, ProposedActionArguments, RunId, Unit, commission_state, frontier_state,
    observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_executor::{MeteredExecutor, RunMeter};

const CASE: &str = "CASE-ADVERSARY-U3";
const EDIT: &str = "repository.edit";

fn uuid(n: u64) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

/// The case: open at revision 1, its frontier listing the admissible edit, whatever is performed.
struct Case;

impl Governor for Case {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(1)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        Ok(Frontier::new(FrontierData {
            frontier_id: FrontierId(uuid(0xf1)),
            case_id: case.clone(),
            case_revision: 1,
            claims: Vec::new(),
            obligations: Vec::new(),
            actions: vec![FrontierAction {
                action: EDIT.to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            }],
        }))
    }

    fn completion(&self, _case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        Ok(CompletionDetermination::Open(Unit(true)))
    }
}

impl ObservationPort for Case {
    fn observe(
        &self,
        _observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        Ok(())
    }
}

/// Performs every action and reports no usage of its own.
struct Effects;

impl EffectPort for Effects {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: CommissionValue::Null,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// Fresh ids, a fixed trusted time and a step budget.
struct Context {
    ids: u64,
    budget: usize,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids += 1;
        ActionRequestId(uuid(0x300 + self.ids))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids += 1;
        ObservationId(uuid(0x400 + self.ids))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-20T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        Some(self.budget)
    }
}

/// `run_until_blocked` on [`Case`] with `executor`, ending at `budget` steps at the latest.
fn run(executor: &(impl AgentExecutor + ?Sized), budget: usize) -> LoopEnd {
    let ids = AtomicU64::new(0x500);
    let mut runs = Generated::new(RunStore::new(move || {
        RunId(uuid(ids.fetch_add(1, Ordering::SeqCst)))
    }));
    run_until_blocked(
        &Case,
        executor,
        &StaticAuthorityProvider::new(),
        &Effects,
        &commission(),
        &mut runs,
        &mut Context { ids: 0, budget },
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

/// An executor over an agent session, not a model loop: it proposes the edit and reports 1,000
/// input and 50 output tokens over two seconds, at 300 micro-US-dollars, for each call.
struct Session;

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
        ExecutorStep {
            outcome: ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
                action: EDIT.to_owned(),
                arguments: ProposedActionArguments(CommissionValue::Object(Vec::new())),
            }),
            usage: Some(ExecutorUsage {
                input_tokens: 1000,
                output_tokens: 50,
                duration_ms: 2000,
                cost_micro_usd: Some(300),
            }),
        }
    }
}

/// A host's executor over another that implements `run` alone, as every executor written before
/// `run_with_usage` existed does and as `Observed` in `tests/run_metering.rs` wraps a
/// `MeteredExecutor`.
struct Logged<E> {
    inner: E,
}

impl<E: AgentExecutor> AgentExecutor for Logged<E> {
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.inner.run(commission, frontier)
    }
}

#[test]
fn a_wrapper_that_does_not_forward_leaves_the_meter_and_the_budget_agreeing() {
    let meter = RunMeter::new(None);
    let executor = Logged {
        inner: MeteredExecutor::new(Session, &meter),
    };

    let end = run(&executor, 2);
    meter.charge_effects(&end.charges);
    let report = meter.report(&end.run_id);

    // CHANGELOG (Unreleased): "a host whose executor wraps another forwards `run_with_usage`, or
    // the inner executor's usage reaches neither the run nor its meter"; ess/domains/metering.yaml
    // (RunUsageReport): the usage an executor call reported is in its step "though the runtime
    // charges both to the run". A step's reported usage the run was never charged for is spend
    // the report shows and the run budget never bounded.
    assert_eq!(
        (report.input_tokens, report.cost_micro_usd),
        (end.charges.run.input_tokens, end.charges.run.cost_micro_usd),
        "the meter shows the session's usage in its steps only when the run was charged it: \
         report {report:?}, run charge {:?}",
        end.charges.run
    );
}
