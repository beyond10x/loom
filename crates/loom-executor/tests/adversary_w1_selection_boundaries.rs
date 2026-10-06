//! Adversary pass 1, wave 2026-10-06-w1, `story:selection-revalidation`: boundaries of
//! `Loom::with_governor` revalidation that `selection_revalidation.rs` does not exercise.
//!
//! Every run is handed a frontier at revision 7 for `CASE-1`; what varies is what the governor
//! answers when Loom revalidates, the action the selector picks, and how the governor is held.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, CompletionDetermination, ExecutorOutcome, Frontier, FrontierAction, FrontierData,
    FrontierId, GovernorError, PrincipalId, Unit, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};
use b10x_loom_executor::model::run::{
    CatalogueEntry, RevalidateSelectionOutcome, SelectionId, SelectionStale, SelectionState,
    SelectionStrategy,
};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{
    ActionSelector, ArgumentContext, ArgumentGenerator, EmptyObjectArguments, Loom, SelectorError,
};

const CASE: &str = "CASE-1";
const READ: &str = "repository.read";
const EDIT: &str = "repository.edit";
const DEPLOY: &str = "repository.deploy";
const WRITE: &str = "repository.write";
const REVISION: i64 = 7;

/// Picks `action` whenever it is a candidate.
struct Picks(&'static str);

impl ActionSelector for Picks {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        candidates
            .iter()
            .find(|entry| entry.action == self.0)
            .map(|entry| Choice {
                action: entry.action.clone(),
                confidence: None,
            })
            .ok_or(SelectorError::NothingAdmissible)
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::Rule
    }
}

fn uuid(n: u64) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission_for(case: &str) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: CaseId(case.to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn commission() -> Commission<commission_state::Assigned> {
    commission_for(CASE)
}

fn listed(action: &str, status: ActionStatus) -> FrontierAction {
    FrontierAction {
        action: action.to_owned(),
        status,
        capability: None,
        reasons: Vec::new(),
    }
}

fn gated(action: &str, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: action.to_owned(),
        status: ActionStatus::ApprovalRequired,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

fn frontier_for(
    case: &str,
    id: u64,
    revision: i64,
    actions: Vec<FrontierAction>,
) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(id)),
        case_id: CaseId(case.to_owned()),
        case_revision: revision,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions,
    })
}

/// The frontier a run is handed: `CASE-1` at `revision`, listing `actions`.
fn handed(revision: i64, actions: Vec<FrontierAction>) -> Frontier<frontier_state::Issued> {
    frontier_for(CASE, 0xf0, revision, actions)
}

fn both_admissible() -> Vec<FrontierAction> {
    vec![
        listed(READ, ActionStatus::Admissible),
        listed(EDIT, ActionStatus::Admissible),
    ]
}

fn scripted(answer: Answer) -> FakeGovernor {
    let governor = FakeGovernor::new();
    governor.script(CaseId(CASE.to_owned()), [answer]);
    governor
}

fn current(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

fn proposed(outcome: &ExecutorOutcome, action: &str) -> bool {
    matches!(outcome, ExecutorOutcome::ProposedAction(proposal) if proposal.action == action)
}

fn not_in_frontier(revalidations: &[RevalidateSelectionOutcome], action: &str) -> bool {
    matches!(
        revalidations,
        [RevalidateSelectionOutcome::NotInFrontier { selection_not_in_frontier }]
            if selection_not_in_frontier.action == action
    )
}

/// One run, selecting `pick` from `given`, with a fake governor answering `answer` once.
fn one_run(
    pick: &'static str,
    given: Frontier<frontier_state::Issued>,
    answer: Answer,
) -> (
    ExecutorOutcome,
    Vec<RevalidateSelectionOutcome>,
    Vec<GovernorCall>,
) {
    let governor = scripted(answer);
    let loom = Loom::new(Picks(pick), EmptyObjectArguments, "edit").with_governor(&governor);
    let outcome = loom.run(&commission(), &given);
    (outcome, loom.revalidations(), governor.calls())
}

/// A current frontier at an OLDER revision than the catalogue the selection was made on is not
/// "the same case": the selection is refused stale, naming both revisions.
#[test]
fn a_current_frontier_older_than_the_catalogue_is_stale() {
    let (outcome, revalidations, calls) = one_run(
        EDIT,
        handed(REVISION, both_admissible()),
        current(REVISION - 1, both_admissible()),
    );
    assert_eq!(outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
    assert!(
        matches!(
            revalidations.as_slice(),
            [RevalidateSelectionOutcome::StaleRevision {
                selection_stale: SelectionStale {
                    catalogue_revision: 7,
                    case_revision: 6,
                    ..
                }
            }]
        ),
        "{revalidations:?}"
    );
    assert_eq!(calls, [GovernorCall::Frontier(CaseId(CASE.to_owned()))]);
}

/// A current frontier listing nothing lists not the selected action.
#[test]
fn an_empty_current_frontier_refuses_the_selection() {
    let (outcome, revalidations, _) = one_run(
        EDIT,
        handed(REVISION, both_admissible()),
        current(REVISION, Vec::new()),
    );
    assert_eq!(outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
    assert!(not_in_frontier(&revalidations, EDIT), "{revalidations:?}");
}

/// A current frontier that lists the action twice, once blocked, does not admit it, whatever the
/// order of the two entries.
#[test]
fn a_duplicate_blocked_entry_takes_the_action_out_of_the_frontier() {
    for actions in [
        vec![
            listed(EDIT, ActionStatus::Admissible),
            listed(EDIT, ActionStatus::Blocked),
        ],
        vec![
            listed(EDIT, ActionStatus::Blocked),
            listed(EDIT, ActionStatus::Admissible),
        ],
    ] {
        let (outcome, revalidations, _) = one_run(
            EDIT,
            handed(REVISION, both_admissible()),
            current(REVISION, actions.clone()),
        );
        assert_eq!(
            outcome,
            ExecutorOutcome::NoUsefulAction(Unit(true)),
            "{actions:?}"
        );
        assert!(not_in_frontier(&revalidations, EDIT), "{revalidations:?}");
    }
}

/// An action id that differs from the selected one only in case or surrounding whitespace is
/// another action.
#[test]
fn an_action_id_differing_by_case_or_whitespace_is_not_listed() {
    for lookalike in [
        "Repository.Edit",
        "repository.edit ",
        " repository.edit",
        "repository.edit\n",
    ] {
        let (outcome, revalidations, _) = one_run(
            EDIT,
            handed(REVISION, both_admissible()),
            current(
                REVISION,
                vec![
                    listed(READ, ActionStatus::Admissible),
                    listed(lookalike, ActionStatus::Admissible),
                ],
            ),
        );
        assert_eq!(
            outcome,
            ExecutorOutcome::NoUsefulAction(Unit(true)),
            "{lookalike:?}"
        );
        assert!(
            not_in_frontier(&revalidations, EDIT),
            "{lookalike:?}: {revalidations:?}"
        );
    }
}

/// An approval-gated selection is revalidated like any other: proposed while the current frontier
/// still lists it under one capability, refused once that frontier's entry asks for no capability
/// or for conflicting ones.
#[test]
fn an_approval_gated_selection_follows_the_current_frontier() {
    let given = || {
        handed(
            REVISION,
            vec![
                listed(READ, ActionStatus::Admissible),
                gated(EDIT, Some(WRITE)),
            ],
        )
    };

    let (outcome, revalidations, _) = one_run(
        EDIT,
        given(),
        current(REVISION, vec![gated(EDIT, Some(WRITE))]),
    );
    assert!(proposed(&outcome, EDIT), "same gate: {outcome:?}");
    assert!(
        matches!(
            revalidations.as_slice(),
            [RevalidateSelectionOutcome::Admitted { .. }]
        ),
        "{revalidations:?}"
    );

    for actions in [
        vec![gated(EDIT, None)],
        vec![gated(EDIT, Some("   "))],
        vec![
            gated(EDIT, Some(WRITE)),
            gated(EDIT, Some("repository.admin")),
        ],
    ] {
        let (outcome, revalidations, _) =
            one_run(EDIT, given(), current(REVISION, actions.clone()));
        assert_eq!(
            outcome,
            ExecutorOutcome::NoUsefulAction(Unit(true)),
            "{actions:?}"
        );
        assert!(
            not_in_frontier(&revalidations, EDIT),
            "{actions:?}: {revalidations:?}"
        );
    }
}

/// What the frontier handed to `run` lists never reaches revalidation: an action only it lists is
/// not in the governor's frontier, and a revision only it claims is stale.
#[test]
fn the_handed_frontier_does_not_leak_into_revalidation() {
    let (outcome, revalidations, _) = one_run(
        DEPLOY,
        handed(
            REVISION,
            vec![
                listed(READ, ActionStatus::Admissible),
                listed(DEPLOY, ActionStatus::Admissible),
            ],
        ),
        current(REVISION, both_admissible()),
    );
    assert_eq!(outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
    assert!(not_in_frontier(&revalidations, DEPLOY), "{revalidations:?}");

    let (outcome, revalidations, _) = one_run(
        EDIT,
        handed(REVISION + 5, both_admissible()),
        current(REVISION, both_admissible()),
    );
    assert_eq!(outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
    assert!(
        matches!(
            revalidations.as_slice(),
            [RevalidateSelectionOutcome::StaleRevision {
                selection_stale: SelectionStale {
                    catalogue_revision: 12,
                    case_revision: 7,
                    ..
                }
            }]
        ),
        "{revalidations:?}"
    );
}

/// A governor whose case moves while the arguments are generated: the first frontier it issues
/// after `moved` is set is one revision on.
struct MovesDuringGeneration {
    moved: AtomicBool,
}

impl Governor for MovesDuringGeneration {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(self.revision())
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        Ok(frontier_for(
            &case.0,
            0xf1,
            self.revision(),
            both_admissible(),
        ))
    }

    fn completion(&self, _case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        Ok(CompletionDetermination::Open(Unit(true)))
    }
}

impl MovesDuringGeneration {
    fn revision(&self) -> i64 {
        if self.moved.load(Ordering::SeqCst) {
            REVISION + 1
        } else {
            REVISION
        }
    }
}

/// An argument generator that moves the case it is generating for, as a slow model call during
/// which somebody else moves the case would.
struct MovingArguments<'g>(&'g MovesDuringGeneration);

impl ArgumentGenerator for MovingArguments<'_> {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        self.0.moved.store(true, Ordering::SeqCst);
        Ok(Value::Object(Vec::new()))
    }
}

/// Revalidation is the last step before the proposal, after argument generation: a case that
/// moves while the arguments are generated is caught. No existing case pins this order; with
/// revalidation moved before `generate`, every case in `selection_revalidation.rs` stays green.
#[test]
fn a_case_moving_during_argument_generation_is_refused_stale() {
    let governor = MovesDuringGeneration {
        moved: AtomicBool::new(false),
    };
    let loom = Loom::new(Picks(EDIT), MovingArguments(&governor), "edit").with_governor(&governor);

    let outcome = loom.run(&commission(), &handed(REVISION, both_admissible()));

    assert_eq!(outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
    assert!(
        matches!(
            loom.revalidations().as_slice(),
            [RevalidateSelectionOutcome::StaleRevision {
                selection_stale: SelectionStale {
                    catalogue_revision: 7,
                    case_revision: 8,
                    ..
                }
            }]
        ),
        "{:?}",
        loom.revalidations()
    );
}

/// A governor that panics on its first frontier call and answers afterwards.
struct PanicsOnce {
    frontier_calls: AtomicUsize,
}

impl Governor for PanicsOnce {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(REVISION)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        if self.frontier_calls.fetch_add(1, Ordering::SeqCst) == 0 {
            panic!("adversary: governor panics while issuing the frontier");
        }
        Ok(frontier_for(&case.0, 0xf2, REVISION, both_admissible()))
    }

    fn completion(&self, _case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        Ok(CompletionDetermination::Open(Unit(true)))
    }
}

/// A governor that panics proposes nothing: the run unwinds, the selection stays unadmitted and
/// no revalidation is recorded; the same Loom then revalidates and proposes normally.
#[test]
fn a_panicking_governor_proposes_nothing_and_leaves_the_loom_usable() {
    let governor = PanicsOnce {
        frontier_calls: AtomicUsize::new(0),
    };
    let loom = Loom::new(Picks(EDIT), EmptyObjectArguments, "edit").with_governor(&governor);

    let first = catch_unwind(AssertUnwindSafe(|| {
        loom.run(&commission(), &handed(REVISION, both_admissible()))
    }));
    assert!(
        first.is_err(),
        "the governor's panic did not unwind: {first:?}"
    );
    assert!(
        loom.revalidations().is_empty(),
        "{:?}",
        loom.revalidations()
    );
    assert!(
        loom.selections()
            .iter()
            .all(|held| held.state != SelectionState::Admitted),
        "{:?}",
        loom.selections()
    );

    let second = loom.run(&commission(), &handed(REVISION, both_admissible()));
    assert!(proposed(&second, EDIT), "{second:?}");
    assert!(
        matches!(
            loom.revalidations().as_slice(),
            [RevalidateSelectionOutcome::Admitted { .. }]
        ),
        "{:?}",
        loom.revalidations()
    );
}

/// A governor whose revision moves between two runs of one Loom: each run reads it once, the
/// first is proposed and the second refused stale.
#[test]
fn a_revision_moving_between_runs_is_read_per_run() {
    let governor = FakeGovernor::new();
    governor.script(
        CaseId(CASE.to_owned()),
        [
            current(REVISION, both_admissible()),
            current(REVISION + 1, both_admissible()),
        ],
    );
    let loom = Loom::new(Picks(EDIT), EmptyObjectArguments, "edit").with_governor(&governor);

    let first = loom.run(&commission(), &handed(REVISION, both_admissible()));
    let second = loom.run(&commission(), &handed(REVISION, both_admissible()));

    assert!(proposed(&first, EDIT), "{first:?}");
    assert_eq!(second, ExecutorOutcome::NoUsefulAction(Unit(true)));
    assert_eq!(governor.calls().len(), 2, "{:?}", governor.calls());
}

/// The governor holders the `Loom` docs name: `Arc`, `Rc` and a boxed trait object.
#[test]
fn every_documented_governor_holder_revalidates() {
    let arc = Arc::new(scripted(current(
        REVISION,
        vec![listed(READ, ActionStatus::Admissible)],
    )));
    let loom = Loom::new(Picks(EDIT), EmptyObjectArguments, "edit").with_governor(Arc::clone(&arc));
    assert_eq!(
        loom.run(&commission(), &handed(REVISION, both_admissible())),
        ExecutorOutcome::NoUsefulAction(Unit(true))
    );
    assert_eq!(arc.calls().len(), 1);

    let rc = Rc::new(scripted(current(
        REVISION,
        vec![listed(READ, ActionStatus::Admissible)],
    )));
    let loom = Loom::new(Picks(EDIT), EmptyObjectArguments, "edit").with_governor(Rc::clone(&rc));
    assert_eq!(
        loom.run(&commission(), &handed(REVISION, both_admissible())),
        ExecutorOutcome::NoUsefulAction(Unit(true))
    );
    assert_eq!(rc.calls().len(), 1);

    let boxed: Box<dyn Governor> = Box::new(scripted(current(
        REVISION,
        vec![listed(READ, ActionStatus::Admissible)],
    )));
    let loom = Loom::new(Picks(EDIT), EmptyObjectArguments, "edit").with_governor(boxed);
    assert_eq!(
        loom.run(&commission(), &handed(REVISION, both_admissible())),
        ExecutorOutcome::NoUsefulAction(Unit(true))
    );
    assert!(
        not_in_frontier(&loom.revalidations(), EDIT),
        "{:?}",
        loom.revalidations()
    );
}

/// A governor answering per case: `CASE-1` is current at revision 7, `CASE-STALE` has moved on.
struct PerCase;

const STALE_CASE: &str = "CASE-STALE";

impl Governor for PerCase {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        Ok(if case.0 == STALE_CASE {
            REVISION + 1
        } else {
            REVISION
        })
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let revision = self.current_revision(case)?;
        Ok(frontier_for(&case.0, 0xf3, revision, both_admissible()))
    }

    fn completion(&self, _case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        Ok(CompletionDetermination::Open(Unit(true)))
    }
}

/// Concurrent runs on one Loom, half on a current case and half on a moved one, share the record:
/// each run's outcome follows its own case, and so does every recorded revalidation.
#[test]
fn concurrent_runs_on_one_loom_keep_their_revalidations_apart() {
    const THREADS: u64 = 8;
    const RUNS: u64 = 25;
    let current_frontier = || frontier_for(CASE, 0xa1, REVISION, both_admissible());
    let stale_frontier = || frontier_for(STALE_CASE, 0xa2, REVISION, both_admissible());
    let loom = Loom::new(Picks(EDIT), EmptyObjectArguments, "edit").with_governor(&PerCase);

    let wrong: Vec<String> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..THREADS)
            .map(|thread| {
                let loom = &loom;
                scope.spawn(move || {
                    let mut wrong = Vec::new();
                    for run in 0..RUNS {
                        let stale = (thread + run) % 2 == 0;
                        let outcome = if stale {
                            loom.run(&commission_for(STALE_CASE), &stale_frontier())
                        } else {
                            loom.run(&commission(), &current_frontier())
                        };
                        let right = if stale {
                            outcome == ExecutorOutcome::NoUsefulAction(Unit(true))
                        } else {
                            proposed(&outcome, EDIT)
                        };
                        if !right {
                            wrong.push(format!(
                                "thread {thread} run {run} stale={stale}: {outcome:?}"
                            ));
                        }
                    }
                    wrong
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("a run panicked"))
            .collect()
    });
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));

    let selections = loom.selections();
    let revalidations = loom.revalidations();
    let total = (THREADS * RUNS) as usize;
    assert_eq!(selections.len(), total);
    assert_eq!(revalidations.len(), total);
    let catalogue_of = |id: &SelectionId| {
        selections
            .iter()
            .find(|held| &held.data.selection_id == id)
            .map(|held| held.data.catalogue_id.0.0.clone())
    };
    for outcome in &revalidations {
        let (id, expected) = match outcome {
            RevalidateSelectionOutcome::Admitted { selection_admitted } => {
                (&selection_admitted.selection_id, uuid(0xa1).0)
            }
            RevalidateSelectionOutcome::StaleRevision { selection_stale } => {
                (&selection_stale.selection_id, uuid(0xa2).0)
            }
            other => panic!("unexpected revalidation {other:?}"),
        };
        assert_eq!(catalogue_of(id), Some(expected), "{outcome:?}");
    }
    let admitted = selections
        .iter()
        .filter(|held| held.state == SelectionState::Admitted)
        .count();
    let refused = selections
        .iter()
        .filter(|held| held.state == SelectionState::Refused)
        .count();
    assert_eq!((admitted, refused), (total / 2, total / 2));
}
