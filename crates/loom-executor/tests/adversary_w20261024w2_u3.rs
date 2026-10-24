//! Adversary cases for `story:refused-selection-stays-refused` (wave 2026-10-24-w2, unit 3).
//!
//! The claim under attack: a selection id the record already holds, in any state, is never selected
//! again, so a refused selection is never `Selected` or `Admitted` again, whatever path reuses its
//! id; and the guard does not refuse a legitimate flow (a fresh id after a refusal, a second run on
//! the same frontier, a second confidence fallback).
//!
//! The story's own test (`adversary2_run_identity.rs`) drives `SpecPorts`, a test double of
//! `SelectAction`, and the conformance suite sends `SelectAction` twice only for a row still
//! `Selected`. These cases drive the shipped `selection::select_action` over the shipped
//! `RequestRecord` with a row in each of the four states, so a guard that looked only at
//! `Selected` rows would fail here. No model or network call is made.

use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorOutcomeCaseMoved, ExecutorOutcomeProposedAction,
    FrontierAction, PrincipalId, ProposedActionArguments, commission_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_executor::arguments::RequestRecord;
use b10x_loom_executor::model::behaviour::{ActionCatalogueStorage, SelectionStorage};
use b10x_loom_executor::model::primitives::{Decimal, Uuid};
use b10x_loom_executor::model::run::obligations::RevalidateSelectionBehavior;
use b10x_loom_executor::model::run::{
    ActionCatalogue, ActionCatalogueData, AnyActionCatalogue, CatalogueEntry, CatalogueEntryStatus,
    CatalogueId, OverruleSelection, OverruleSelectionOutcome, RevalidateSelection,
    RevalidateSelectionOutcome, SelectAction, SelectActionOutcome, SelectionExists, SelectionId,
    SelectionSnapshot, SelectionState, SelectionStrategy, TurnId,
};
use b10x_loom_executor::projection::CatalogueRecord;
use b10x_loom_executor::selection::{Choice, SelectionContext, select_action};
use b10x_loom_executor::{
    ActionSelector, ArgumentContext, ArgumentGenerator, HybridSelector, Loom, SelectorError,
};

const ACTION: &str = "metrics.inspect";
const OTHER: &str = "logs.search";
const REVISION: i64 = 3;

fn uuid(n: u32) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn catalogue_id() -> CatalogueId {
    CatalogueId(uuid(0xc00))
}

/// One projected catalogue listing [`ACTION`] and [`OTHER`] at [`REVISION`].
fn catalogues() -> CatalogueRecord {
    let mut record = CatalogueRecord::default();
    record.put(
        AnyActionCatalogue::Projected(ActionCatalogue::new(ActionCatalogueData {
            catalogue_id: catalogue_id(),
            turn_id: TurnId(uuid(0x700)),
            frontier: "frontier-adversary".to_owned(),
            case_revision: REVISION,
            entries: [ACTION, OTHER]
                .into_iter()
                .map(|action| CatalogueEntry {
                    action: action.to_owned(),
                    status: CatalogueEntryStatus::Admissible,
                })
                .collect(),
        }))
        .snapshot(),
    );
    record
}

fn select(id: &SelectionId, action: &str) -> SelectAction {
    SelectAction {
        selection_id: id.clone(),
        catalogue_id: catalogue_id(),
        action: action.to_owned(),
        confidence: None,
        strategy: SelectionStrategy::Rule,
        case_revision: REVISION,
    }
}

fn revalidate(id: &SelectionId, case_revision: i64) -> RevalidateSelection {
    RevalidateSelection {
        selection_id: id.clone(),
        case_revision,
        frontier_actions: vec![ACTION.to_owned(), OTHER.to_owned()],
    }
}

fn held(record: &RequestRecord, id: &SelectionId) -> SelectionSnapshot {
    SelectionStorage::get(record, id).unwrap_or_else(|| panic!("no selection {}", id.0.0))
}

/// A record holding `id` (selected for [`ACTION`]) moved to `state` by the shipped commands.
fn holding(id: &SelectionId, state: SelectionState) -> RequestRecord {
    let catalogues = catalogues();
    let mut record = RequestRecord::default();
    let first = select_action(&catalogues, &mut record, select(id, ACTION));
    assert!(
        matches!(first, SelectActionOutcome::Selected { .. }),
        "{first:?}"
    );
    match state {
        SelectionState::Selected => {}
        SelectionState::Refused => {
            let stale = record.revalidate_selection(revalidate(id, REVISION + 1));
            assert!(
                matches!(stale, Ok(RevalidateSelectionOutcome::StaleRevision { .. })),
                "{stale:?}"
            );
        }
        SelectionState::Admitted => {
            let admitted = record.revalidate_selection(revalidate(id, REVISION));
            assert!(
                matches!(admitted, Ok(RevalidateSelectionOutcome::Admitted { .. })),
                "{admitted:?}"
            );
        }
        SelectionState::Overruled => {
            let replacement = SelectionId(uuid(0x5e9));
            let made = select_action(&catalogues, &mut record, select(&replacement, OTHER));
            assert!(
                matches!(made, SelectActionOutcome::Selected { .. }),
                "{made:?}"
            );
            let overruled = record.overrule(OverruleSelection {
                selection_id: id.clone(),
                replacement_id: replacement,
            });
            assert!(
                matches!(overruled, Ok(OverruleSelectionOutcome::Overruled { .. })),
                "{overruled:?}"
            );
        }
    }
    assert_eq!(held(&record, id).state, state);
    record
}

/// The shipped `select_action` refuses an id the record holds in each of the four states, stores
/// nothing (the row stays exactly as it was, though the second call names another action), and a
/// later revalidation of a row that is not `Selected` leaves it where it rests.
#[test]
fn select_action_refuses_a_held_id_in_every_state_and_leaves_the_row_as_it_was() {
    let catalogues = catalogues();
    for state in [
        SelectionState::Selected,
        SelectionState::Refused,
        SelectionState::Admitted,
        SelectionState::Overruled,
    ] {
        let id = SelectionId(uuid(0x5e1));
        let mut record = holding(&id, state);
        let before = held(&record, &id);
        let rows = record.selections().len();

        let again = select_action(&catalogues, &mut record, select(&id, OTHER));

        assert_eq!(
            again,
            SelectActionOutcome::SelectionExists {
                error: SelectionExists {
                    selection_id: id.clone()
                }
            },
            "{state:?}: a held id is selected again"
        );
        assert_eq!(held(&record, &id), before, "{state:?}: the row changed");
        assert_eq!(
            record.selections().len(),
            rows,
            "{state:?}: a row was added"
        );

        if state != SelectionState::Selected {
            for revision in [REVISION, REVISION + 1] {
                let later = record.revalidate_selection(revalidate(&id, revision));
                assert!(
                    matches!(later, Ok(RevalidateSelectionOutcome::WrongState { .. })),
                    "{state:?}: a later revalidation at {revision} answered {later:?}"
                );
                assert_eq!(held(&record, &id).state, state, "{state:?}: it moved");
            }
        }
    }
}

/// `selection-exists` is checked before the catalogue guards, as the synthesis plan's precedence
/// says for a command with `when_related:`: a reused id naming an unknown catalogue, an action the
/// catalogue does not list or another revision is still `selection-exists`, and the refused row
/// stays `Refused`.
#[test]
fn selection_exists_precedes_every_catalogue_guard() {
    let catalogues = catalogues();
    let id = SelectionId(uuid(0x5e2));
    let mut record = holding(&id, SelectionState::Refused);
    let unknown_catalogue = SelectAction {
        catalogue_id: CatalogueId(uuid(0xbad)),
        ..select(&id, ACTION)
    };
    let not_listed = select(&id, "release.rollback");
    let other_revision = SelectAction {
        case_revision: REVISION + 9,
        ..select(&id, ACTION)
    };
    for input in [unknown_catalogue, not_listed, other_revision] {
        let answer = select_action(&catalogues, &mut record, input.clone());
        assert!(
            matches!(answer, SelectActionOutcome::SelectionExists { .. }),
            "{input:?} answered {answer:?}"
        );
        assert_eq!(held(&record, &id).state, SelectionState::Refused);
    }
}

/// The guard keys on the id alone: after a refusal a fresh id for the same action, on the same
/// catalogue, is selected and admitted, and the refused row stays `Refused`.
#[test]
fn a_fresh_id_after_a_refusal_is_selected_and_admitted() {
    let catalogues = catalogues();
    let refused = SelectionId(uuid(0x5e3));
    let mut record = holding(&refused, SelectionState::Refused);
    let fresh = SelectionId(uuid(0x5e4));

    let selected = select_action(&catalogues, &mut record, select(&fresh, ACTION));
    let admitted = record.revalidate_selection(revalidate(&fresh, REVISION));

    assert!(
        matches!(selected, SelectActionOutcome::Selected { .. }),
        "{selected:?}"
    );
    assert!(
        matches!(admitted, Ok(RevalidateSelectionOutcome::Admitted { .. })),
        "{admitted:?}"
    );
    assert_eq!(held(&record, &refused).state, SelectionState::Refused);
    assert_eq!(held(&record, &fresh).state, SelectionState::Admitted);
}

/// An id spelled differently (upper-case hex) is another identity to the record: whatever it
/// answers, the refused row under the canonical spelling is never moved, and no revalidation of
/// either spelling admits the refused row.
#[test]
fn another_spelling_of_a_refused_id_never_moves_the_refused_row() {
    let catalogues = catalogues();
    let refused = SelectionId(uuid(0xabc));
    let mut record = holding(&refused, SelectionState::Refused);
    let shouted = SelectionId(Uuid(refused.0.0.to_uppercase()));
    assert_ne!(shouted, refused);

    let _ = select_action(&catalogues, &mut record, select(&shouted, ACTION));
    let _ = record.revalidate_selection(revalidate(&shouted, REVISION));
    let _ = record.revalidate_selection(revalidate(&refused, REVISION));

    assert_eq!(held(&record, &refused).state, SelectionState::Refused);
}

// ---------------------------------------------------------------------------------------------
// The run pipeline, end to end through `AgentExecutor::run`.

const CASE: &str = "CASE-ADV-U3";
const PROMPT: &str = "Find out why checkout latency doubled.";

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000a31".to_owned(),
        )),
        agent_revision_id: AgentRevisionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000a32".to_owned(),
        )),
        case_id: case(),
        principal: PrincipalId("principal-adversary".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

fn answer(revision: i64) -> Answer {
    Answer::at(revision).with_items(
        Vec::new(),
        Vec::new(),
        [ACTION, OTHER]
            .into_iter()
            .map(|action| FrontierAction {
                action: action.to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            })
            .collect(),
    )
}

/// A selector naming one action under one strategy, with one confidence.
struct Naming(&'static str, SelectionStrategy, Option<&'static str>);

impl ActionSelector for Naming {
    fn select(
        &self,
        _context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        Ok(Choice {
            action: self.0.to_owned(),
            confidence: self.2.map(|text| Decimal(text.to_owned())),
        })
    }

    fn strategy(&self) -> SelectionStrategy {
        self.1
    }
}

/// Arguments naming the action, counting each generation.
#[derive(Default)]
struct Counting(Mutex<usize>);

impl ArgumentGenerator for &Counting {
    fn generate(
        &self,
        _context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<CommissionValue, String> {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        Ok(CommissionValue::Text(entry.action.clone()))
    }
}

fn proposed(action: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: ProposedActionArguments(CommissionValue::Text(action.to_owned())),
    })
}

/// A run whose selection revalidation refused (the case moved) is followed by a second run on the
/// very frontier it was handed: the second run gets an id of its own and is proposed, the first
/// selection stays `Refused`, and each run generated arguments once.
#[test]
fn a_second_run_on_the_same_frontier_after_a_refusal_is_proposed() {
    let governor = FakeGovernor::new();
    // The handed frontier, the first run's revalidation (moved), the second run's (back at 7).
    governor.script(case(), [answer(7), answer(8), answer(7)]);
    let arguments = Counting::default();
    let loom = Loom::new(
        Naming(ACTION, SelectionStrategy::Rule, None),
        &arguments,
        PROMPT,
    )
    .with_governor(&governor);
    let frontier = governor
        .frontier(&case())
        .expect("the governor serves the case");

    let first = loom.run(&commission(), &frontier);
    let second = loom.run(&commission(), &frontier);

    assert_eq!(
        first,
        ExecutorOutcome::CaseMoved(ExecutorOutcomeCaseMoved {
            expected_case_revision: 7
        })
    );
    assert_eq!(second, proposed(ACTION));
    let states: Vec<_> = loom.selections().iter().map(|held| held.state).collect();
    assert_eq!(states, [SelectionState::Refused, SelectionState::Admitted]);
    let selections = loom.selections();
    assert_ne!(
        selections[0].data.selection_id,
        selections[1].data.selection_id
    );
    assert_eq!(
        *arguments.0.lock().unwrap_or_else(PoisonError::into_inner),
        2
    );
}

/// Two runs that each fall back from a low-confidence fast pick are both proposed: the second
/// run's overruled id and selection id are fresh, so the guard on the overruled id refuses
/// nothing, and the record holds two overruled and two admitted selections, none twice.
#[test]
fn a_second_confidence_fallback_is_not_refused_by_the_overruled_id_guard() {
    let governor = FakeGovernor::new();
    governor.script(case(), [answer(7)]);
    let selector = HybridSelector::new(
        Naming(ACTION, SelectionStrategy::FastTyped, Some("0.1")),
        Naming(OTHER, SelectionStrategy::ReasoningModel, None),
        &Decimal("0.9".to_owned()),
    )
    .expect("a threshold in [0, 1]");
    let arguments = Counting::default();
    let loom = Loom::new(selector, &arguments, PROMPT).with_governor(&governor);
    let frontier = governor
        .frontier(&case())
        .expect("the governor serves the case");

    let first = loom.run(&commission(), &frontier);
    let second = loom.run(&commission(), &frontier);

    assert_eq!(first, proposed(OTHER));
    assert_eq!(second, proposed(OTHER));
    let selections = loom.selections();
    let states: Vec<_> = selections.iter().map(|held| held.state).collect();
    assert_eq!(
        states,
        [
            SelectionState::Overruled,
            SelectionState::Admitted,
            SelectionState::Overruled,
            SelectionState::Admitted,
        ]
    );
    let mut ids: Vec<_> = selections
        .iter()
        .map(|held| held.data.selection_id.clone())
        .collect();
    ids.sort_by(|a, b| a.0.0.cmp(&b.0.0));
    ids.dedup();
    assert_eq!(ids.len(), 4, "an id was given twice");
}

/// Two Looms sharing a fixed namespace (`with_instance`, documented for tests only) give their
/// first runs the same selection id. Each record is its own: the first Loom's refused selection
/// stays `Refused` while the second Loom admits its own selection under the same id. A host that
/// fixed one namespace for a Loom built to recover a run would see the id reused across records;
/// the guard is per record and does not reach across them.
#[test]
fn a_fixed_namespace_reuses_ids_across_looms_and_each_record_keeps_its_own_state() {
    let governor = FakeGovernor::new();
    governor.script(case(), [answer(7), answer(8), answer(7)]);
    let arguments = Counting::default();
    let build = || {
        Loom::new(
            Naming(ACTION, SelectionStrategy::Rule, None),
            &arguments,
            PROMPT,
        )
        .with_instance("adversary-fixed")
        .with_governor(&governor)
    };
    let frontier = governor
        .frontier(&case())
        .expect("the governor serves the case");
    let (earlier, recovered) = (build(), build());

    let refused = earlier.run(&commission(), &frontier);
    let again = recovered.run(&commission(), &frontier);

    assert!(
        matches!(refused, ExecutorOutcome::CaseMoved(_)),
        "{refused:?}"
    );
    assert_eq!(again, proposed(ACTION));
    let (earlier, recovered) = (earlier.selections(), recovered.selections());
    assert_eq!(
        earlier[0].data.selection_id, recovered[0].data.selection_id,
        "a fixed namespace gives the same id"
    );
    assert_eq!(earlier[0].state, SelectionState::Refused);
    assert_eq!(recovered[0].state, SelectionState::Admitted);
}
