//! Adversary pass 1, wave 2026-10-08-w1, `story:moved-run-named-outcome`.
//!
//! The story's acceptance: a run whose case moved to a revision whose frontier, as the executor
//! would be handed it, still admits an action ends `CaseMovedOn`, with `bound_case_revision` the
//! Run's revision and `current_case_revision` the revision the runtime loaded. The spec names the
//! bound revision "the run's case_revision" (`ess/commission/domains/responsibility.yaml`,
//! `RunOutcomeCaseMovedOn`), the revision the Run started against, which an effect of the Run's own
//! does not change (`Run.case_revision`).
//!
//! The unit's cases all end with the loop holding the case at the Run's own revision, so none
//! tells `run.case_revision` from the revision the loop holds the case at after an effect of its
//! own. The cases here first let the Run's own effect move the case (5 to 6), then let somebody
//! else move it (6 to 7), on each path that ends `CaseMovedOn`: the stale proposal, the
//! executor-reported move, and an executor that proposes nothing (runtime item 10). They also pin
//! the guard that a found move's frontier is for the loaded revision, and the stale path under a
//! step budget.
//!
//! The fake governor answers one scripted answer per call, whichever port method is called, and
//! repeats its last answer once the script runs out.

use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeCaseMoved, ExecutorOutcomeProposedAction, FrontierAction, FrontierObligation,
    ObservationId, PrincipalId, ProposedActionArguments, RevalidateActionRequestOutcome, RunId,
    RunOutcome, RunOutcomeCaseMovedOn, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

const CASE: &str = "CASE-ADV-W1";
/// The revision the Run starts against.
const START: i64 = 5;
/// The revision the Run's own effect moves the case to.
const OWN: i64 = 6;
/// The revision somebody else moves the case to.
const FOREIGN: i64 = 7;

/// Actions the effect port performs and that need no authority.
const EDIT: &str = "code.edit";
const TEST: &str = "tests.run";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xa1)),
        agent_revision_id: AgentRevisionId(uuid(0xa2)),
        case_id: case(),
        principal: PrincipalId("principal-adv".to_owned()),
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

fn propose(action: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    })
}

/// Performs every action, and records each request it is handed.
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
            .push(request.data().action.clone());
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Null,
            attempt: None,
        }))
    }
}

/// New ids from counters and one trusted time; the step budget it was given.
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
        Timestamp("2026-10-08T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.budget
    }
}

fn run_with(
    governor: &FakeGovernor,
    executor: &ScriptedExecutor,
    effects: &Effects,
    budget: Option<usize>,
) -> LoopEnd {
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x500))));
    run_until_blocked(
        governor,
        executor,
        &StaticAuthorityProvider::new(),
        effects,
        &commission(),
        &mut runs,
        &mut Context { ids: 0, budget },
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"))
}

/// `CaseMovedOn` bound to the Run's starting revision, current at [`FOREIGN`].
fn moved_on_from_start() -> RunOutcome {
    RunOutcome::CaseMovedOn(RunOutcomeCaseMovedOn {
        bound_case_revision: START,
        current_case_revision: FOREIGN,
    })
}

/// The governor answers of the Run's load, the first iteration, the revalidation and admission of
/// `edit` at [`START`] (5 calls), then the next iteration at [`OWN`], the revision the Run's own
/// effect moved the case to (3 calls).
fn own_effect_then(rest: impl IntoIterator<Item = Answer>) -> FakeGovernor {
    let start = at(START, Vec::new(), vec![admissible(EDIT)]);
    let own = at(OWN, Vec::new(), vec![admissible(TEST)]);
    let governor = FakeGovernor::new();
    governor.script(
        case(),
        std::iter::repeat_n(start, 5)
            .chain(std::iter::repeat_n(own, 3))
            .chain(rest),
    );
    governor
}

/// The revision the case moved to by somebody else: its frontier admits `tests.run`.
fn foreign() -> Answer {
    at(FOREIGN, vec![open("tests-pass")], vec![admissible(TEST)])
}

/// Stale-proposal path after an effect of the Run's own: `edit` at 5 is admitted and performed and
/// moves the case to 6, where the loop goes on; `tests.run` proposed at 6 is found stale at 7. The
/// next iteration finds the move and the frontier of 7 admits `tests.run`, so the run ends
/// `CaseMovedOn` bound to the Run's revision, 5, not to the 6 the loop held after its own effect.
#[test]
fn adversary_a_stale_proposal_after_an_own_effect_is_bound_to_the_runs_start_revision() {
    // The stale revalidation (one call), then the found move: load, completion, frontier.
    let governor = own_effect_then(std::iter::repeat_n(foreign(), 4));
    let executor = ScriptedExecutor::new([propose(EDIT), propose(TEST)]);
    let effects = Effects::default();

    let end = run_with(&governor, &executor, &effects, None);

    assert_eq!(effects.invoked(), vec![EDIT.to_owned()], "{end:#?}");
    assert_eq!(end.requests.len(), 2, "{:?}", end.requests);
    assert!(
        matches!(
            end.requests[1].outcome,
            RevalidateActionRequestOutcome::Stale { .. }
        ),
        "{:?}",
        end.requests
    );
    assert_eq!(executor.calls().len(), 2);
    assert_eq!(end.outcome, moved_on_from_start(), "{end:#?}");
}

/// Executor-reported path after an effect of the Run's own: the executor reports `CaseMoved` on the
/// frontier of 6, the revision the Run's own effect moved the case to; the case is at 7, whose
/// frontier admits `tests.run`. The run ends `CaseMovedOn` bound to the Run's 5.
#[test]
fn adversary_a_reported_move_after_an_own_effect_is_bound_to_the_runs_start_revision() {
    let governor = own_effect_then(std::iter::repeat_n(foreign(), 3));
    let executor = ScriptedExecutor::new([
        propose(EDIT),
        ExecutorOutcome::CaseMoved(ExecutorOutcomeCaseMoved {
            expected_case_revision: OWN,
        }),
    ]);
    let effects = Effects::default();

    let end = run_with(&governor, &executor, &effects, None);

    assert_eq!(effects.invoked(), vec![EDIT.to_owned()], "{end:#?}");
    assert_eq!(executor.calls().len(), 2);
    assert_eq!(end.outcome, moved_on_from_start(), "{end:#?}");
}

/// Item 10 after an effect of the Run's own: at 6 the frontier lists nothing, the executor
/// proposes nothing, and the outcome derived from that frontier would end the run. The reload
/// finds the case at 7, whose frontier admits `tests.run`: the run ends `CaseMovedOn` bound to the
/// Run's 5. (No unit case exercises `CaseMovedOn` on this path at all.)
#[test]
fn adversary_proposing_nothing_on_a_moved_case_ends_case_moved_on_bound_to_the_runs_start() {
    let start = at(START, Vec::new(), vec![admissible(EDIT)]);
    let empty_own = at(OWN, Vec::new(), Vec::new());
    let governor = FakeGovernor::new();
    governor.script(
        case(),
        std::iter::repeat_n(start, 5)
            .chain(std::iter::repeat_n(empty_own, 3))
            .chain(std::iter::repeat_n(foreign(), 3)),
    );
    let executor =
        ScriptedExecutor::new([propose(EDIT), ExecutorOutcome::NoUsefulAction(Unit(true))]);
    let effects = Effects::default();

    let end = run_with(&governor, &executor, &effects, None);

    assert_eq!(effects.invoked(), vec![EDIT.to_owned()], "{end:#?}");
    assert_eq!(executor.calls().len(), 2);
    assert_eq!(end.outcome, moved_on_from_start(), "{end:#?}");
}

/// The guard on the found-move path: the frontier read after the runtime found the case at 8 is
/// for 9, another revision than the one loaded. Even though that frontier admits `tests.run`, the
/// run does not name a current revision it never read a frontier for: it ends with no admissible
/// action, as `moved_judged` does for the reported move.
#[test]
fn adversary_a_found_move_whose_frontier_is_for_another_revision_admits_nothing() {
    let start = at(START, Vec::new(), vec![admissible(TEST)]);
    let governor = FakeGovernor::new();
    governor.script(
        case(),
        [
            // The Run's load, the first iteration's completion and frontier.
            start.clone(),
            start.clone(),
            start,
            // The revalidation finds the case at 8: stale.
            at(8, Vec::new(), vec![admissible(TEST)]),
            // The next iteration: load and completion at 8, then a frontier for 9.
            at(8, Vec::new(), vec![admissible(TEST)]),
            at(8, Vec::new(), vec![admissible(TEST)]),
            at(9, Vec::new(), vec![admissible(TEST)]),
        ],
    );
    let executor = ScriptedExecutor::new([propose(TEST)]);
    let effects = Effects::default();

    let end = run_with(&governor, &executor, &effects, None);

    assert!(effects.invoked().is_empty(), "{:?}", effects.invoked());
    assert_eq!(
        end.outcome,
        RunOutcome::NoAdmissibleAction(Unit(true)),
        "{end:#?}"
    );
}

/// The stale-proposal path under a step budget of one: the stale proposal is the run's one step,
/// and the case has moved to a revision whose frontier admits an action. Acceptance 1 says the run
/// ends `CaseMovedOn`; the budget check at the top of the next iteration runs before the load that
/// would find the move, so the Run is suspended for its budget instead, and a caller that resumes
/// it resumes a Run bound to a revision the case has left.
#[test]
fn adversary_a_stale_proposal_on_the_last_budgeted_step_ends_case_moved_on() {
    let start = at(START, Vec::new(), vec![admissible(TEST)]);
    let moved = at(OWN, Vec::new(), vec![admissible(TEST)]);
    let governor = FakeGovernor::new();
    governor.script(
        case(),
        [start.clone(), start.clone(), start, moved.clone(), moved],
    );
    let executor = ScriptedExecutor::new([propose(TEST)]);
    let effects = Effects::default();

    let end = run_with(&governor, &executor, &effects, Some(1));

    assert!(effects.invoked().is_empty(), "{:?}", effects.invoked());
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
        RunOutcome::CaseMovedOn(RunOutcomeCaseMovedOn {
            bound_case_revision: START,
            current_case_revision: OWN,
        }),
        "{end:#?}"
    );
}
