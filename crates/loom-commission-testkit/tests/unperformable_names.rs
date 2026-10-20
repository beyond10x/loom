//! Acceptance for `story:unperformable-names-actions`: a run that ends `NoPerformableAction` names
//! the actions no effect performs, in frontier order, and the claims of that frontier that do not
//! hold (`ess/commission/domains/responsibility.yaml`, `RunOutcomeNoPerformableAction`,
//! `JudgePerformableActions`). On 0.28.0 the outcome was `NoPerformableAction(Unit)` and said
//! nothing about which actions, so a host could not say what to hand over.
//!
//! The runtime raises it at three places (`runtime`'s module documentation): before the executor
//! runs, on a frontier that lists actions none of which the effect port performs (item 4); when an
//! admitted request's action is not one the port performs (item 8); and when a move the governor
//! bears out leaves a frontier that lists actions none of which the port performs (items 9 and 10).
//! Each names its actions. The frontier relates no action to a claim, so the open claims are the
//! frontier's: its claims whose value is not `True`, in frontier order. Loom names what is missing
//! and decides nothing about who performs it.

use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId,
    Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed,
    ExecutorOutcome, ExecutorOutcomeCaseMoved, ExecutorOutcomeProposedAction, FrontierAction,
    FrontierClaim, ObservationId, PrincipalId, ProposedActionArguments,
    RevalidateActionRequestOutcome, RunId, RunOutcome, RunOutcomeNoPerformableAction, Truth,
    commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};

/// The trusted time the loop's context gives.
const NOW: &str = "2026-10-20T15:00:00Z";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xe1)),
        agent_revision_id: AgentRevisionId(uuid(0xe2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-unperformable".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn action(name: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

fn admissible(name: &str) -> FrontierAction {
    action(name, ActionStatus::Admissible, None)
}

fn claim(name: &str, value: Truth) -> FrontierClaim {
    FrontierClaim {
        claim: name.to_owned(),
        value,
    }
}

/// The claims every frontier here lists: one that holds between two that do not, so the order
/// and the filter both show.
fn claims() -> Vec<FrontierClaim> {
    vec![
        claim("release.published", Truth::Unknown),
        claim("tests.pass", Truth::True),
        claim("pr.opened", Truth::False),
    ]
}

/// The claims of [`claims`] that do not hold, in frontier order.
fn open_claims() -> Vec<String> {
    vec!["release.published".to_owned(), "pr.opened".to_owned()]
}

/// The case open at `revision`, its frontier listing [`claims`] and exactly `actions`.
fn listing(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(claims(), Vec::new(), actions)
}

fn proposal(name: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(Value::Text(format!("{name} arguments"))),
    })
}

/// The outcome naming `actions`, in that order, and [`open_claims`].
fn unperformable(actions: &[&str]) -> RunOutcome {
    RunOutcome::NoPerformableAction(RunOutcomeNoPerformableAction {
        actions: actions.iter().map(|name| (*name).to_owned()).collect(),
        open_claims: open_claims(),
    })
}

/// The governor calls an iteration makes before it runs the executor.
fn iteration(case: &CaseId) -> Vec<GovernorCall> {
    vec![
        GovernorCall::CurrentRevision(case.clone()),
        GovernorCall::Completion(case.clone()),
        GovernorCall::Frontier(case.clone()),
    ]
}

/// New ids from counters, the one trusted time [`NOW`], and a step budget of ten.
#[derive(Debug)]
struct Context {
    requests: u64,
    observations: u64,
}

impl Context {
    fn new() -> Self {
        Self {
            requests: 0x700,
            observations: 0x800,
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
        Some(10)
    }
}

/// A fake effect port that performs exactly the actions it was given, answers every invocation
/// `Performed`, and records each request it was handed.
struct Effects {
    performs: Vec<&'static str>,
    invoked: Mutex<Vec<ActionRequestData>>,
}

impl Effects {
    fn performing(performs: Vec<&'static str>) -> Self {
        Self {
            performs,
            invoked: Mutex::new(Vec::new()),
        }
    }

    fn invoked(&self) -> Vec<ActionRequestData> {
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl EffectPort for Effects {
    fn performs(&self, action: &str) -> bool {
        self.performs.contains(&action)
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let data = request.data().clone();
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(data);
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Text("done".to_owned()),
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// Runs one loop on a fresh store; returns its end.
fn run(
    governor: &FakeGovernor,
    executor: &ScriptedExecutor,
    effects: &Effects,
    case: &CaseId,
) -> LoopEnd {
    let mut issued = 0;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x200 + issued))
    }));
    run_until_blocked(
        governor,
        executor,
        &StaticAuthorityProvider::new(),
        effects,
        &commission(case),
        &mut runs,
        &mut Context::new(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

/// Item 4: a frontier listing two actions the port does not perform ends the run naming both, in
/// frontier order (not sorted: `publish` before `open_pr`), with the frontier's open claims. The
/// executor is never run and nothing is invoked.
#[test]
fn the_outcome_names_each_unperformed_action_and_its_claim() {
    let case = CaseId("case-two-unperformed".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            3,
            vec![
                admissible("publish"),
                action("open_pr", ActionStatus::ApprovalRequired, Some("forge.write")),
            ],
        )],
    );
    let executor = ScriptedExecutor::new([]);
    let effects = Effects::performing(vec!["edit"]);

    let end = run(&governor, &executor, &effects, &case);

    assert_eq!(
        end.outcome,
        unperformable(&["publish", "open_pr"]),
        "both actions, in frontier order, with the frontier's open claims"
    );
    assert_eq!(governor.calls(), iteration(&case), "one frontier");
    assert!(executor.calls().is_empty(), "the executor ran");
    assert!(effects.invoked().is_empty(), "{:?}", effects.invoked());
    assert!(end.requests.is_empty(), "{:?}", end.requests);
    assert_eq!(end.steps, 0);
}

/// Item 4, an action the frontier lists more than once: it is named once, where it is first
/// listed.
#[test]
fn an_action_listed_twice_is_named_once() {
    let case = CaseId("case-listed-twice".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            3,
            vec![
                admissible("publish"),
                admissible("open_pr"),
                action("publish", ActionStatus::Blocked, None),
            ],
        )],
    );
    let executor = ScriptedExecutor::new([]);
    let effects = Effects::performing(vec!["edit"]);

    let end = run(&governor, &executor, &effects, &case);

    assert_eq!(end.outcome, unperformable(&["publish", "open_pr"]));
    assert!(executor.calls().is_empty(), "the executor ran");
}

/// Item 8: the executor proposes an admitted action the port does not perform. The run ends naming
/// that action alone, not the frontier's other actions, with the frontier's open claims; the
/// request is recorded with its revalidation and never handed to the port.
#[test]
fn a_proposed_unperformed_action_is_named() {
    let case = CaseId("case-proposed-unperformed".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(5, vec![admissible("edit"), admissible("publish")])],
    );
    let executor = ScriptedExecutor::new([proposal("publish")]);
    let effects = Effects::performing(vec!["edit"]);

    let end = run(&governor, &executor, &effects, &case);

    assert_eq!(
        end.outcome,
        unperformable(&["publish"]),
        "the proposed action, with the frontier's open claims"
    );
    assert_eq!(executor.calls().len(), 1);
    assert!(effects.invoked().is_empty(), "the port was handed it");
    assert_eq!(end.requests.len(), 1);
    assert_eq!(
        end.requests[0].outcome,
        RevalidateActionRequestOutcome::Admitted
    );
    assert!(end.admitted.is_empty(), "{:?}", end.admitted);
    assert!(end.effects.is_empty());
}

/// Item 9: the executor reports a move the governor bears out, and the frontier at the new
/// revision lists only actions the port does not perform. The run ends naming them, in frontier
/// order, with that frontier's open claims.
#[test]
fn a_moved_case_whose_frontier_performs_nothing_names_its_actions() {
    let case = CaseId("case-moved-unperformed".to_owned());
    let governor = FakeGovernor::new();
    // One answer per governor call: the iteration's three at revision 2, then revision 3 for the
    // load, the completion and the frontier that judge the move.
    let before = || listing(2, vec![admissible("edit")]);
    governor.script(
        case.clone(),
        [
            before(),
            before(),
            before(),
            listing(3, vec![admissible("publish"), admissible("open_pr")]),
        ],
    );
    let executor = ScriptedExecutor::new([ExecutorOutcome::CaseMoved(ExecutorOutcomeCaseMoved {
        expected_case_revision: 2,
    })]);
    let effects = Effects::performing(vec!["edit"]);

    let end = run(&governor, &executor, &effects, &case);

    assert_eq!(end.outcome, unperformable(&["publish", "open_pr"]));
    assert_eq!(executor.calls().len(), 1);
    assert!(effects.invoked().is_empty(), "{:?}", effects.invoked());
}
