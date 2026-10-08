//! Acceptance for `story:moved-case-outcome` (`decision-blocker:run-stale-outcome`, option C): an
//! executor reports that the case moved under it (`ExecutorOutcome::CaseMoved`, naming the
//! revision of the frontier it was handed), and `run_until_blocked` loads the case once more and
//! judges the run on the frontier current then, through the same handed-frontier filter the loop
//! uses (`crates/loom-commission/src/runtime.rs`, module docs, item 9), instead of on the frontier
//! the case left.
//!
//! Acceptance for `story:moved-run-named-outcome` (`decision-blocker:moved-run-admissible-frontier`,
//! option A): where that frontier still admits an action, the run ends `CaseMovedOn`, naming the
//! revision the Run is bound to and the one the runtime loaded, whether the executor reported the
//! move or the runtime found it on a stale proposal.
//!
//! Each case scripts the fake governor per call: the run's load, then the completion and the
//! frontier of the first iteration, all at revision 7; every later call answers revision 8. The
//! scripted executor returns `CaseMoved` (or, on the stale-proposal path, a proposal) once, and a
//! second call would panic (its script is used up), so a case that passes ran the executor exactly
//! once.

use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeCaseMoved, ExecutorOutcomeProposedAction, FrontierAction, FrontierObligation,
    ObservationId, PrincipalId, ProposedActionArguments, RevalidateActionRequestOutcome, RunId,
    RunOutcome, RunOutcomeCaseMovedOn, RunOutcomeCompleted, RunOutcomeNeedsExternalEvidence,
    RunOutcomeSuspended, SuspensionReason, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{EXECUTOR_SOURCE, LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};

const CASE: &str = "CASE-M";
/// The revision the executor was handed.
const LEFT: i64 = 7;
/// The revision the case moved to while the executor worked.
const CURRENT: i64 = 8;

/// An action the effect port performs, behind the authority gate of the capability of its name.
const MERGE: &str = "repository.merge";
/// An action the effect port performs and that needs no authority.
const TEST: &str = "tests.run";
/// An action the effect port does not perform and that needs no authority: never handed over.
const PUBLISH: &str = "docs.publish";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x81)),
        agent_revision_id: AgentRevisionId(uuid(0x90)),
        case_id: case(),
        principal: PrincipalId("principal-m".to_owned()),
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

fn gated(name: &str) -> FrontierAction {
    action(name, ActionStatus::ApprovalRequired, Some(name))
}

fn open(obligation: &str) -> FrontierObligation {
    FrontierObligation {
        obligation: obligation.to_owned(),
        open: true,
    }
}

/// The case open at `revision`, its frontier holding exactly `obligations` and `actions`.
fn at(revision: i64, obligations: Vec<FrontierObligation>, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), obligations, actions)
}

/// The revision the case left: `tests-pass` open, and only the gated merge listed, so the frontier
/// alone admits nothing. Judged on it, a run asks for evidence for `tests-pass`.
fn left() -> Answer {
    at(LEFT, vec![open("tests-pass")], vec![gated(MERGE)])
}

/// A governor at [`left`] for the run's load and the first iteration's completion and frontier,
/// then at `current` for every later call.
fn moved_to(current: Answer) -> FakeGovernor {
    let governor = FakeGovernor::new();
    governor.script(case(), [left(), left(), left(), current]);
    governor
}

/// The executor's report: the case moved while it worked on the frontier of [`LEFT`].
fn case_moved() -> ExecutorOutcome {
    ExecutorOutcome::CaseMoved(ExecutorOutcomeCaseMoved {
        expected_case_revision: LEFT,
    })
}

/// Performs [`MERGE`] and [`TEST`], and records every request it is handed.
#[derive(Debug, Default)]
struct Effects {
    invoked: Mutex<Vec<String>>,
}

impl Effects {
    fn invoked(&self) -> Vec<String> {
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl EffectPort for Effects {
    fn performs(&self, action: &str) -> bool {
        action == MERGE || action == TEST
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.data().action.clone());
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Null,
            attempt: None,
            audit: None,
        }))
    }
}

/// New ids from counters and one trusted time; no step budget.
#[derive(Debug, Default)]
struct Context {
    ids: u64,
    budget: Option<usize>,
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
        Timestamp("2026-10-07T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.budget
    }
}

/// One loop over `governor`, whose executor reports [`case_moved`] once.
fn run(governor: &FakeGovernor, effects: &Effects) -> (LoopEnd, ScriptedExecutor) {
    run_on(governor, effects, case_moved())
}

/// One loop over `governor`, whose executor returns `outcome` once.
fn run_on(
    governor: &FakeGovernor,
    effects: &Effects,
    outcome: ExecutorOutcome,
) -> (LoopEnd, ScriptedExecutor) {
    run_budgeted(governor, effects, outcome, None)
}

/// One loop over `governor` with step budget `budget`, whose executor returns `outcome` once.
fn run_budgeted(
    governor: &FakeGovernor,
    effects: &Effects,
    outcome: ExecutorOutcome,
    budget: Option<usize>,
) -> (LoopEnd, ScriptedExecutor) {
    let executor = ScriptedExecutor::new([outcome]);
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x500))));
    let end = run_until_blocked(
        governor,
        &executor,
        &StaticAuthorityProvider::new(),
        effects,
        &commission(),
        &mut runs,
        &mut Context { ids: 0, budget },
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"));
    (end, executor)
}

/// The governor calls of one iteration up to the executor, and of the one reload after it.
fn load() -> Vec<GovernorCall> {
    vec![
        GovernorCall::CurrentRevision(case()),
        GovernorCall::Completion(case()),
        GovernorCall::Frontier(case()),
    ]
}

/// Story acceptance 2: the case moved past `tests-pass` to a revision whose frontier holds
/// `review-approved` open, lists the gated merge, and lists `docs.publish`, which the effect port
/// does not perform and which needs no authority. The runtime loads the case once more and judges
/// the run on that frontier as the executor would be handed it, without `docs.publish`: it ends
/// asking for evidence for `review-approved`, not for the superseded `tests-pass`.
#[test]
fn a_reported_move_is_judged_once_on_the_current_handed_frontier() {
    let governor = moved_to(at(
        CURRENT,
        vec![open("review-approved")],
        vec![gated(MERGE), admissible(PUBLISH)],
    ));
    let effects = Effects::default();
    let (end, executor) = run(&governor, &effects);

    assert_eq!(
        end.outcome,
        RunOutcome::NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence {
            requirements: vec!["review-approved".to_owned()],
        }),
        "{end:#?}"
    );
    assert_eq!(
        governor.calls(),
        [load(), load()].concat(),
        "one iteration, then exactly one reload of the case and its frontier"
    );
    assert_eq!(executor.calls().len(), 1);
    assert_eq!(executor.frontiers()[0].case_revision, LEFT);
    assert!(end.requests.is_empty(), "{:?}", end.requests);
    assert!(end.admitted.is_empty(), "{:?}", end.admitted);
    assert!(effects.invoked().is_empty(), "{:?}", effects.invoked());
    assert_eq!(end.steps, 0, "the executor call ended the run");
    assert_eq!(
        end.last_frontier
            .as_ref()
            .map(|frontier| frontier.case_revision),
        Some(LEFT),
        "the last frontier read before an executor step"
    );

    let observations = governor.observations();
    assert_eq!(observations.len(), 1, "{observations:?}");
    let step = &observations[0];
    assert_eq!(step.source, EXECUTOR_SOURCE);
    assert_eq!(step.subject, format!("{CASE}@{LEFT}"));
    assert_eq!(
        step.payload.member("outcome"),
        Some(&Value::Text("CaseMoved".to_owned()))
    );
    assert_eq!(
        step.payload.member("expected_case_revision"),
        Some(&Value::Number(LEFT.to_string()))
    );
}

/// The case was completed while the executor worked: the reload finds it complete and the run ends
/// completed with the governor's outcome, reading no frontier after the determination.
#[test]
fn a_case_completed_under_the_executor_ends_completed() {
    let governor = moved_to(at(CURRENT, Vec::new(), Vec::new()).complete("merged"));
    let (end, executor) = run(&governor, &Effects::default());

    assert_eq!(
        end.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "merged".to_owned(),
        }),
        "{end:#?}"
    );
    assert_eq!(
        governor.calls(),
        [
            load(),
            vec![
                GovernorCall::CurrentRevision(case()),
                GovernorCall::Completion(case()),
            ],
        ]
        .concat()
    );
    assert_eq!(executor.calls().len(), 1);
    assert_eq!(end.steps, 0);
}

/// `CaseMovedOn` from the Run's revision [`LEFT`] to [`CURRENT`].
fn moved_on() -> RunOutcome {
    RunOutcome::CaseMovedOn(RunOutcomeCaseMovedOn {
        bound_case_revision: LEFT,
        current_case_revision: CURRENT,
    })
}

/// The executor's proposal of the gated merge the frontier of [`LEFT`] lists.
fn proposes_merge() -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: MERGE.to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    })
}

/// story:moved-run-named-outcome, acceptance 1 and 2, the executor-reported move: the current
/// frontier admits an action the port performs. The Run is bound to the revision it left, so it
/// does not go on at the new one and the executor is not run again: the run ends `CaseMovedOn`,
/// naming the Run's revision and the one the runtime loaded.
#[test]
fn a_reported_move_whose_current_frontier_admits_an_action_ends_case_moved_on() {
    let governor = moved_to(at(
        CURRENT,
        vec![open("tests-pass")],
        vec![admissible(TEST)],
    ));
    let effects = Effects::default();
    let (end, executor) = run(&governor, &effects);

    assert_eq!(end.outcome, moved_on(), "{end:#?}");
    assert_eq!(governor.calls(), [load(), load()].concat());
    assert_eq!(executor.calls().len(), 1);
    assert!(effects.invoked().is_empty(), "{:?}", effects.invoked());
    assert_eq!(end.steps, 0, "the executor call ended the run");
}

/// story:moved-run-named-outcome, acceptance 1, the stale-proposal path: the executor proposes the
/// gated merge on the frontier of [`LEFT`], and revalidation finds the case at [`CURRENT`], so the
/// request is stale. The next iteration loads the case, finds it moved and judges the run on the
/// frontier current then, as the executor would be handed it: it admits an action the port
/// performs, so the run ends `CaseMovedOn` and the executor is not run again.
#[test]
fn a_stale_proposal_whose_current_frontier_admits_an_action_ends_case_moved_on() {
    let governor = moved_to(at(
        CURRENT,
        vec![open("tests-pass")],
        vec![admissible(TEST), admissible(PUBLISH)],
    ));
    let effects = Effects::default();
    let (end, executor) = run_on(&governor, &effects, proposes_merge());

    assert_eq!(end.outcome, moved_on(), "{end:#?}");
    assert_eq!(
        governor.calls(),
        [load(), vec![GovernorCall::CurrentRevision(case())], load()].concat(),
        "one iteration, its stale revalidation, then one load of the moved case and its frontier"
    );
    assert_eq!(executor.calls().len(), 1);
    assert_eq!(end.requests.len(), 1, "{:?}", end.requests);
    assert!(
        matches!(
            end.requests[0].outcome,
            RevalidateActionRequestOutcome::Stale { .. }
        ),
        "{:?}",
        end.requests
    );
    assert!(end.admitted.is_empty(), "{:?}", end.admitted);
    assert!(effects.invoked().is_empty(), "{:?}", effects.invoked());
}

/// The stale-proposal path, on a current frontier that admits an action only before the handed
/// filter: `docs.publish`, which the port does not perform and which needs no authority, is
/// dropped, and the gated merge is not admitted without authority. As handed, the frontier admits
/// nothing, so the run does not end `CaseMovedOn`: it ends with no admissible action, as the
/// stale-proposal path ends on such a frontier (`crates/loom-executor/tests/adversary_w1_runtime_stale.rs`).
#[test]
fn a_stale_proposal_whose_handed_current_frontier_admits_nothing_ends_with_no_admissible_action() {
    let governor = moved_to(at(
        CURRENT,
        vec![open("review-approved")],
        vec![gated(MERGE), admissible(PUBLISH)],
    ));
    let effects = Effects::default();
    let (end, executor) = run_on(&governor, &effects, proposes_merge());

    assert_eq!(
        end.outcome,
        RunOutcome::NoAdmissibleAction(Unit(true)),
        "{end:#?}"
    );
    assert_eq!(
        governor.calls(),
        [load(), vec![GovernorCall::CurrentRevision(case())], load()].concat()
    );
    assert_eq!(executor.calls().len(), 1);
    assert!(effects.invoked().is_empty(), "{:?}", effects.invoked());
}

/// The current frontier lists actions, none of which the effect port performs: the run ends as the
/// loop ends on such a frontier, with no performable action.
#[test]
fn a_moved_case_whose_current_frontier_lists_nothing_performable_ends_with_no_performable_action() {
    let governor = moved_to(at(
        CURRENT,
        vec![open("review-approved")],
        vec![admissible(PUBLISH)],
    ));
    let (end, executor) = run(&governor, &Effects::default());

    assert_eq!(
        end.outcome,
        RunOutcome::NoPerformableAction(Unit(true)),
        "{end:#?}"
    );
    assert_eq!(governor.calls(), [load(), load()].concat());
    assert_eq!(executor.calls().len(), 1);
}

/// A stale proposal is the run's one budgeted step, but the case did not move: the frontier the
/// proposal was revalidated against was issued for [`CURRENT`] while the case stayed at [`LEFT`].
/// The next iteration loads the case first, finds it where the Run holds it, reads the frontier
/// (at [`LEFT`] again), and only then is the Run suspended for its budget.
#[test]
fn a_stale_proposal_on_the_last_budgeted_step_with_no_move_is_suspended_for_its_budget() {
    let governor = FakeGovernor::new();
    let held = at(LEFT, vec![open("tests-pass")], vec![admissible(TEST)]);
    let current = at(CURRENT, vec![open("tests-pass")], vec![admissible(TEST)]);
    governor.script(
        case(),
        [
            held.clone(),
            held.clone(),
            held.clone(),
            held.clone(),
            current,
            held.clone(),
            held.clone(),
            held,
        ],
    );
    let effects = Effects::default();
    let proposal = ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: TEST.to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    });
    let (end, executor) = run_budgeted(&governor, &effects, proposal, Some(1));

    assert!(
        matches!(
            end.requests.first().map(|made| &made.outcome),
            Some(RevalidateActionRequestOutcome::Stale { .. })
        ),
        "{:?}",
        end.requests
    );
    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended {
            reason: SuspensionReason::Budget(Value::Object(vec![(
                "max_steps".to_owned(),
                Value::Number("1".to_owned()),
            )])),
        }),
        "{end:#?}"
    );
    assert_eq!(
        governor.calls(),
        [
            load(),
            vec![
                GovernorCall::CurrentRevision(case()),
                GovernorCall::Frontier(case()),
            ],
            load(),
        ]
        .concat()
    );
    assert_eq!(executor.calls().len(), 1);
    assert!(effects.invoked().is_empty(), "{:?}", effects.invoked());
}
