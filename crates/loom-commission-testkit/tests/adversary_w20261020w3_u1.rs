//! Adversary pass 1 on `story:unperformable-names-actions` (wave 2026-10-20-w3, U1).
//!
//! `NoPerformableAction` names the actions no effect performs and the open claims of the frontier
//! the outcome was judged on. These cases drive the raising paths on inputs the unit's own cases
//! leave alone: a frontier whose claims differ from the frontier the run left (items 9 and 10), an
//! admitted request on a later iteration whose frontier carries other claims than the first one
//! (item 8), an admitted request that needed authority the provider allowed (item 8), and a
//! frontier whose claims all hold.

use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext,
    AuthorityVerdict, CaseId, Commission, CommissionData, CommissionId, EffectOutcome,
    EffectOutcomePerformed, ExecutorOutcome, ExecutorOutcomeCaseMoved,
    ExecutorOutcomeProposedAction, FrontierAction, FrontierClaim, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, RunOutcome, RunOutcomeNoPerformableAction, Truth, Unit,
    commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xa1)),
        agent_revision_id: AgentRevisionId(uuid(0xa2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adversary-u1".to_owned()),
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

/// The claims of the frontier a run starts on: two open, one holding.
fn left_claims() -> Vec<FrontierClaim> {
    vec![
        claim("left.unknown", Truth::Unknown),
        claim("shared.claim", Truth::False),
        claim("left.true", Truth::True),
    ]
}

/// The claims of the frontier after a move: `shared.claim` now holds, two others are open.
fn moved_claims() -> Vec<FrontierClaim> {
    vec![
        claim("shared.claim", Truth::True),
        claim("moved.false", Truth::False),
        claim("moved.unknown", Truth::Unknown),
    ]
}

/// The open claims of [`moved_claims`], in frontier order.
fn moved_open() -> Vec<String> {
    vec!["moved.false".to_owned(), "moved.unknown".to_owned()]
}

/// The open claims of [`left_claims`], in frontier order.
fn left_open() -> Vec<String> {
    vec!["left.unknown".to_owned(), "shared.claim".to_owned()]
}

fn at(revision: i64, claims: Vec<FrontierClaim>, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(claims, Vec::new(), actions)
}

fn proposal(name: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(Value::Text(format!("{name} arguments"))),
    })
}

fn unperformable(actions: &[&str], open_claims: Vec<String>) -> RunOutcome {
    RunOutcome::NoPerformableAction(RunOutcomeNoPerformableAction {
        actions: actions.iter().map(|name| (*name).to_owned()).collect(),
        open_claims,
    })
}

#[derive(Debug)]
struct Context {
    ids: u64,
    budget: Option<usize>,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids += 1;
        ActionRequestId(uuid(0x900 + self.ids))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids += 1;
        ObservationId(uuid(0xa00 + self.ids))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-20T16:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.budget
    }
}

/// Performs exactly `performs`, answers every invocation `Performed` and records each request.
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
    fn performs(&self, action: &str) -> bool {
        self.performs.contains(&action)
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.data().clone());
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Text("done".to_owned()),
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

fn run_with(
    governor: &FakeGovernor,
    executor: &ScriptedExecutor,
    authority: &StaticAuthorityProvider,
    effects: &Effects,
    case: &CaseId,
    budget: Option<usize>,
) -> LoopEnd {
    let mut issued = 0;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0xb00 + issued))
    }));
    run_until_blocked(
        governor,
        executor,
        authority,
        effects,
        &commission(case),
        &mut runs,
        &mut Context { ids: 0, budget },
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

/// Item 9: the executor reports a move the governor bears out, and the frontier at the new
/// revision lists only actions the port does not perform, under other claims than the frontier
/// the run left. The open claims are the new frontier's, not the left one's.
#[test]
fn adversary_a_borne_out_move_names_the_open_claims_of_the_new_frontier() {
    let case = CaseId("case-adv-moved-claims".to_owned());
    let governor = FakeGovernor::new();
    let left = || at(2, left_claims(), vec![admissible("edit")]);
    governor.script(
        case.clone(),
        [
            left(),
            left(),
            left(),
            at(
                3,
                moved_claims(),
                vec![admissible("publish"), admissible("open_pr")],
            ),
        ],
    );
    let executor = ScriptedExecutor::new([ExecutorOutcome::CaseMoved(ExecutorOutcomeCaseMoved {
        expected_case_revision: 2,
    })]);
    let effects = Effects::performing(vec!["edit"]);

    let end = run_with(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Some(10),
    );

    assert_eq!(
        end.outcome,
        unperformable(&["publish", "open_pr"], moved_open())
    );
    assert!(effects.invoked().is_empty(), "{:?}", effects.invoked());
}

/// Item 10: the executor proposes nothing on a frontier that admits nothing, so the derived
/// outcome would end the run; the case is loaded once more and has moved to a frontier that lists
/// only actions the port does not perform. The run ends naming them with that frontier's claims.
#[test]
fn adversary_a_move_found_after_proposing_nothing_names_the_new_frontier() {
    let case = CaseId("case-adv-idle-moved".to_owned());
    let governor = FakeGovernor::new();
    let left = || {
        at(
            2,
            left_claims(),
            vec![action("edit", ActionStatus::Blocked, None)],
        )
    };
    governor.script(
        case.clone(),
        [
            left(),
            left(),
            left(),
            at(3, moved_claims(), vec![admissible("publish")]),
        ],
    );
    let executor = ScriptedExecutor::new([ExecutorOutcome::NoUsefulAction(Unit(true))]);
    let effects = Effects::performing(vec!["edit"]);

    let end = run_with(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Some(10),
    );

    assert_eq!(end.outcome, unperformable(&["publish"], moved_open()));
    assert_eq!(executor.calls().len(), 1);
}

/// Item 8 on the second iteration: the first step performs `edit`, which moves the case to
/// revision 6 under other claims; the executor then proposes `publish`, admitted and not
/// performed. The run names `publish` with the open claims of the revision-6 frontier the request
/// was made on, not those of the first frontier.
#[test]
fn adversary_a_later_unperformed_proposal_names_the_claims_of_its_own_frontier() {
    let case = CaseId("case-adv-later-proposal".to_owned());
    let governor = FakeGovernor::new();
    let first = || {
        at(
            5,
            left_claims(),
            vec![admissible("edit"), admissible("publish")],
        )
    };
    // The start, the completion and the frontier, then the revalidation's load and frontier, all
    // at 5; every later call at 6.
    governor.script(
        case.clone(),
        [
            first(),
            first(),
            first(),
            first(),
            first(),
            at(
                6,
                moved_claims(),
                vec![admissible("edit"), admissible("publish")],
            ),
        ],
    );
    let executor = ScriptedExecutor::new([proposal("edit"), proposal("publish")]);
    let effects = Effects::performing(vec!["edit"]);

    let end = run_with(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Some(10),
    );

    assert_eq!(end.outcome, unperformable(&["publish"], moved_open()));
    assert_eq!(effects.invoked(), ["edit"]);
    assert_eq!(executor.calls().len(), 2);
}

/// Item 8 through authority: `deploy` needs approval, the provider allows it, the case is still
/// at the request's revision, and the port does not perform it. The run names `deploy` alone, not
/// the performable `edit`, with the frontier's open claims; nothing is invoked.
#[test]
fn adversary_an_allowed_unperformed_request_is_named_with_the_open_claims() {
    let case = CaseId("case-adv-allowed".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [at(
            4,
            left_claims(),
            vec![
                admissible("edit"),
                action("deploy", ActionStatus::ApprovalRequired, Some("deploy")),
            ],
        )],
    );
    let executor = ScriptedExecutor::new([proposal("deploy")]);
    let authority =
        StaticAuthorityProvider::new().answer("deploy", AuthorityVerdict::Allow(Unit(true)));
    let effects = Effects::performing(vec!["edit"]);

    let end = run_with(&governor, &executor, &authority, &effects, &case, Some(10));

    assert_eq!(end.outcome, unperformable(&["deploy"], left_open()));
    assert!(effects.invoked().is_empty(), "{:?}", effects.invoked());
    assert!(end.admitted.is_empty(), "{:?}", end.admitted);
}

/// Item 4 on a frontier whose claims all hold: the actions are named and no claim is open.
#[test]
fn adversary_a_frontier_whose_claims_all_hold_names_no_open_claim() {
    let case = CaseId("case-adv-all-true".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [at(
            1,
            vec![claim("a", Truth::True), claim("b", Truth::True)],
            vec![admissible("publish")],
        )],
    );
    let executor = ScriptedExecutor::new([]);
    let effects = Effects::performing(vec!["edit"]);

    let end = run_with(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        None,
    );

    assert_eq!(end.outcome, unperformable(&["publish"], Vec::new()));
}

/// Without a step budget, an idle iteration followed by a frontier listing only actions the port
/// does not perform ends `NoPerformableAction`, naming them, before the idle bound is reached.
#[test]
fn adversary_an_idle_iteration_then_nothing_performable_names_the_actions() {
    let case = CaseId("case-adv-idle-then-unperformable".to_owned());
    let governor = FakeGovernor::new();
    let first = || at(7, left_claims(), vec![admissible("edit")]);
    governor.script(
        case.clone(),
        [
            first(),
            first(),
            first(),
            at(7, moved_claims(), vec![admissible("publish")]),
        ],
    );
    let executor = ScriptedExecutor::new([ExecutorOutcome::NoUsefulAction(Unit(true))]);
    let effects = Effects::performing(vec!["edit"]);

    let end = run_with(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        None,
    );

    assert_eq!(end.outcome, unperformable(&["publish"], moved_open()));
    assert_eq!(executor.calls().len(), 1);
}

/// Item 2, the move the runtime finds after a stale proposal, onto the same frontier
/// [`adversary_a_borne_out_move_names_the_open_claims_of_the_new_frontier`] moves to by a reported
/// move: it lists only actions the port does not perform. The reported move ends naming them; the
/// found one should too, or a host learns the names only on one of two paths to one state.
#[test]
fn adversary_a_found_move_onto_nothing_performable_names_the_actions() {
    let case = CaseId("case-adv-found-move".to_owned());
    let governor = FakeGovernor::new();
    let left = || at(2, left_claims(), vec![admissible("edit")]);
    governor.script(
        case.clone(),
        [
            left(),
            left(),
            left(),
            at(
                3,
                moved_claims(),
                vec![admissible("publish"), admissible("open_pr")],
            ),
        ],
    );
    let executor = ScriptedExecutor::new([proposal("edit")]);
    let effects = Effects::performing(vec!["edit"]);

    let end = run_with(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Some(10),
    );

    assert!(effects.invoked().is_empty(), "{:?}", effects.invoked());
    assert_eq!(
        end.outcome,
        unperformable(&["publish", "open_pr"], moved_open())
    );
}
