//! Adversary case for `story:idle-bound-outcome` (wave 2026-10-20-w2, U2): the idle bound's
//! outcome, `RunOutcome::IdleBound`, against a completion at the second idle iteration.
//!
//! The runtime decides a case the governor holds complete ahead of every other outcome: rule 1 of
//! `outcome::derive` ends such a run completed "whatever the executor returned", and item 0 of the
//! runtime's module documentation loads the case after a step whose effect was `Performed` before
//! the step budget suspends the run, "so a case that step completed ends the run completed". The
//! idle bound is a bound of the same kind as the step budget ("Without a step budget the loop is
//! bounded where the frontier does not change"), and the specification says it ends a run "where it
//! would otherwise go on" (`ess/commission/domains/responsibility.yaml`, `RunOutcome`).

use std::iter::repeat_n;
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId,
    Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed,
    ExecutorOutcome, ExecutorOutcomeProposedAction, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, RunOutcome, RunOutcomeCompleted, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

const NOW: &str = "2026-10-20T12:00:00Z";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xb1)),
        agent_revision_id: AgentRevisionId(uuid(0xb2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adversary-u2".to_owned()),
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

fn listing(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

fn proposal(name: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(Value::Text(format!("{name} arguments"))),
    })
}

fn idle() -> ExecutorOutcome {
    ExecutorOutcome::NoUsefulAction(Unit(true))
}

fn completed(outcome: &str) -> RunOutcome {
    RunOutcome::Completed(RunOutcomeCompleted {
        outcome: outcome.to_owned(),
    })
}

/// Request ids from `0x301`, observation ids from `0x401`, one trusted time, and `budget` as the
/// step budget.
#[derive(Debug)]
struct Context {
    requests: u64,
    observations: u64,
    budget: Option<usize>,
}

impl Context {
    fn new(budget: Option<usize>) -> Self {
        Self {
            requests: 0x300,
            observations: 0x400,
            budget,
        }
    }
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(NOW.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.budget
    }
}

/// A port that performs every action, answers every invocation `Performed`, and records each
/// request it was handed.
#[derive(Default)]
struct Effects {
    invoked: Mutex<Vec<ActionRequestData>>,
}

impl Effects {
    fn invoked(&self) -> Vec<String> {
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|data| data.action.clone())
            .collect()
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
        let data = request.data().clone();
        let report = Value::Text(format!("done {}", data.action));
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(data);
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

fn run(
    governor: &FakeGovernor,
    executor: &ScriptedExecutor,
    effects: &Effects,
    case: &CaseId,
    budget: Option<usize>,
) -> LoopEnd {
    let mut issued = 0;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));
    run_until_blocked(
        governor,
        executor,
        &StaticAuthorityProvider::new(),
        effects,
        &commission(case),
        &mut runs,
        &mut Context::new(budget),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

/// The governor's answers for the completion case: the case open at revision 4, its frontier
/// listing `check`, for the 13 calls of a new `check` (load, completion, frontier, revalidation's
/// load and frontier), an idle step (load, completion, frontier) and the repeated `check` (five
/// again); from then on the repeated `check` has completed the case, at revision 5.
fn completed_by_the_repeat() -> impl Iterator<Item = Answer> {
    repeat_n(listing(4, vec![admissible("check")]), 13)
        .chain([listing(5, Vec::new()).complete("checks-pass")])
}

/// A run with no step budget whose executor admits `check`, makes one idle step, and admits the
/// same `check` again: a repeated admission, the second idle iteration in a row. The port performs
/// the repeat, and that performance completes the case.
///
/// With a step budget the run never reaches, the same steps end `Completed`: the loop loads the
/// case after a performed effect (item 0) and the governor holds it complete (item 2). Without one,
/// the idle bound ends the run `IdleBound` at once, without loading the case its own effect just
/// completed, so a host is told the run made no progress for two iterations about a case the run
/// completed. On 18cb4e6 the same guard ended it `NoAdmissibleAction`.
#[test]
fn adversary_a_repeated_admission_that_completes_the_case_ends_the_run_completed() {
    let case = CaseId("case-adv-u2-repeat-completes-budgeted".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), completed_by_the_repeat());
    let executor = ScriptedExecutor::new([proposal("check"), idle(), proposal("check")]);
    let effects = Effects::default();
    let budgeted = run(&governor, &executor, &effects, &case, Some(10));
    assert_eq!(
        effects.invoked(),
        ["check", "check"],
        "precondition: both requests are performed"
    );
    assert_eq!(
        budgeted.outcome,
        completed("checks-pass"),
        "precondition: under a step budget it never reaches, the run ends completed"
    );

    let case = CaseId("case-adv-u2-repeat-completes".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), completed_by_the_repeat());
    let executor = ScriptedExecutor::new([proposal("check"), idle(), proposal("check")]);
    let effects = Effects::default();
    let end = run(&governor, &executor, &effects, &case, None);
    assert_eq!(
        effects.invoked(),
        ["check", "check"],
        "precondition: both requests are performed"
    );
    assert_eq!(
        end.outcome,
        completed("checks-pass"),
        "the run's own performed effect completed the case: a case the governor holds complete \
         ends the run completed (derive rule 1, runtime items 0 and 2), not on the idle bound, \
         which ends a run only where it would otherwise go on"
    );
}
