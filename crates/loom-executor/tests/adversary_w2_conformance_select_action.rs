//! Adversary pass 1 on `story:loom-ess-conformance`: `selection::select_action`, held to
//! `ess/domains/run.yaml` where the synthesized suite does not reach it.
//!
//! `select_action` is called by nothing in this workspace but the conformance target
//! (`crates/loom-conformance/src/lib.rs`), so the synthesized suite is the only check it has. That
//! suite's `revision-mismatch` scenario claims a case revision *below* the catalogue's and asserts
//! only the error's `case_revision`; no scenario reads the stored selection after a refusal, none
//! publishes `confidence`, and none has an unlisted action at a mismatched revision. Each case below
//! drives one of those from the specification:
//!
//! * `revision-mismatch` holds for `case_revision != input.case_revision`, so a revision *above*
//!   the catalogue's is refused too, and the error names the catalogue and both revisions;
//! * a refusal stores nothing (the `selected` outcome alone `creates: loom.run.Selection`);
//! * `when_related` outcomes are selected in declaration order, so an unlisted action is
//!   `not-in-catalogue` even at a mismatched revision;
//! * `selected` sets `confidence: input.confidence` and `strategy: input.strategy`.

use b10x_loom_executor::arguments::RequestRecord;
use b10x_loom_executor::model::primitives::{Decimal, Uuid};
use b10x_loom_executor::model::run::obligations::ProjectCatalogueBehavior;
use b10x_loom_executor::model::run::{
    ActionNotInCatalogue, ActionSelected, CatalogueEntry, CatalogueEntryStatus, CatalogueId,
    CatalogueRevisionMismatch, ProjectCatalogue, ProjectCatalogueOutcome, SelectAction,
    SelectActionOutcome, SelectionData, SelectionId, SelectionState, SelectionStrategy, TurnId,
};
use b10x_loom_executor::selection::select_action;
use b10x_loom_executor::session::TurnRecord;

const CATALOGUE_REVISION: i64 = 7;
const LISTED: &str = "repository.read";

fn uuid(n: u32) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn catalogue_id() -> CatalogueId {
    CatalogueId(uuid(0xc00))
}

/// A record holding one catalogue, projected at [`CATALOGUE_REVISION`] and listing [`LISTED`].
fn projected() -> TurnRecord {
    let mut turns = TurnRecord::default();
    let outcome = turns
        .project_catalogue(ProjectCatalogue {
            catalogue_id: catalogue_id(),
            turn_id: TurnId(uuid(0x7e1)),
            frontier: "frontier".to_owned(),
            case_revision: CATALOGUE_REVISION,
            entries: vec![CatalogueEntry {
                action: LISTED.to_owned(),
                status: CatalogueEntryStatus::Admissible,
            }],
        })
        .expect("the executor answers ProjectCatalogue");
    assert!(
        matches!(outcome, ProjectCatalogueOutcome::Projected { .. }),
        "{outcome:?}"
    );
    turns
}

fn input(action: &str, case_revision: i64) -> SelectAction {
    SelectAction {
        selection_id: SelectionId(uuid(0x5e1)),
        catalogue_id: catalogue_id(),
        action: action.to_owned(),
        confidence: Some(Decimal("0.25".to_owned())),
        strategy: SelectionStrategy::FastTyped,
        case_revision,
    }
}

/// `revision-mismatch` for a case revision on either side of the catalogue's, naming the
/// catalogue, the claimed revision and the catalogue's own; nothing is stored on the refusal.
#[test]
fn adversary_w2_a_revision_on_either_side_of_the_catalogues_is_refused_and_nothing_stored() {
    let mut failures = Vec::new();
    for claimed in [CATALOGUE_REVISION - 1, CATALOGUE_REVISION + 1] {
        let turns = projected();
        let mut requests = RequestRecord::default();

        let outcome = select_action(&turns, &mut requests, input(LISTED, claimed));

        let expected = SelectActionOutcome::RevisionMismatch {
            error: CatalogueRevisionMismatch {
                catalogue_id: catalogue_id(),
                case_revision: claimed,
                catalogue_revision: CATALOGUE_REVISION,
            },
        };
        if outcome != expected {
            failures.push(format!(
                "claimed revision {claimed} on a catalogue at {CATALOGUE_REVISION}: expected \
                 {expected:?}, got {outcome:?}"
            ));
        }
        if !requests.selections().is_empty() {
            failures.push(format!(
                "claimed revision {claimed}: the refusal stored {:?}",
                requests.selections()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// `not-in-catalogue` is declared before `revision-mismatch`: an action the catalogue does not
/// list is refused as such whatever revision the input claims, and nothing is stored.
#[test]
fn adversary_w2_an_unlisted_action_at_another_revision_is_not_in_catalogue() {
    let turns = projected();
    let mut requests = RequestRecord::default();

    let outcome = select_action(
        &turns,
        &mut requests,
        input("repository.merge", CATALOGUE_REVISION + 1),
    );

    assert_eq!(
        outcome,
        SelectActionOutcome::NotInCatalogue {
            error: ActionNotInCatalogue {
                action: "repository.merge".to_owned(),
            },
        }
    );
    assert!(
        requests.selections().is_empty(),
        "{:?}",
        requests.selections()
    );
}

/// `selected` stores the selection `Selected`, with every field the outcome `sets:` from the
/// input: the confidence and the strategy included, which no view publishes.
#[test]
fn adversary_w2_a_selection_keeps_the_confidence_and_strategy_it_was_made_with() {
    let turns = projected();
    let mut requests = RequestRecord::default();

    let outcome = select_action(&turns, &mut requests, input(LISTED, CATALOGUE_REVISION));

    assert_eq!(
        outcome,
        SelectActionOutcome::Selected {
            action_selected: ActionSelected {
                selection_id: SelectionId(uuid(0x5e1)),
                catalogue_id: catalogue_id(),
                action: LISTED.to_owned(),
            },
        }
    );
    let stored: Vec<(SelectionState, SelectionData)> = requests
        .selections()
        .iter()
        .map(|held| (held.state, held.data.clone()))
        .collect();
    assert_eq!(
        stored,
        vec![(
            SelectionState::Selected,
            SelectionData {
                selection_id: SelectionId(uuid(0x5e1)),
                catalogue_id: catalogue_id(),
                action: LISTED.to_owned(),
                confidence: Some(Decimal("0.25".to_owned())),
                strategy: SelectionStrategy::FastTyped,
                case_revision: CATALOGUE_REVISION,
                replaced_by: None,
            },
        )]
    );
}
