//! Adversary pass 1, wave 2026-10-10-w1, `story:fallback-selection-recording`:
//! `RequestRecord::overrule`, the host's `loom.run.OverruleSelection`, at its edges.
//!
//! The story's outcome: "two `loom.run.Selection`s, the fast selection and the stronger selector's
//! selection that replaced it. The fast one references its replacement ... Both belong to the run's
//! turn." `RequestRecord::overrule` adds one check to the generated behaviour: the replacement is
//! held. These cases ask what else a replacement has to be.

use b10x_loom_executor::arguments::{OverruleRefused, RequestRecord};
use b10x_loom_executor::model::behaviour::SelectionStorage;
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::obligations::RevalidateSelectionBehavior;
use b10x_loom_executor::model::run::{
    AnySelection, CatalogueId, OverruleSelection, OverruleSelectionOutcome, RevalidateSelection,
    RevalidateSelectionOutcome, Selection, SelectionData, SelectionId, SelectionNotFound,
    SelectionSnapshot, SelectionState, SelectionStrategy,
};

const REVISION: i64 = 7;

fn uuid(n: u32) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn id(n: u32) -> SelectionId {
    SelectionId(uuid(n))
}

fn selected_in(id_: u32, catalogue: u32, action: &str) -> SelectionSnapshot {
    AnySelection::Selected(Selection::new(SelectionData {
        selection_id: id(id_),
        catalogue_id: CatalogueId(uuid(catalogue)),
        action: action.to_owned(),
        confidence: None,
        strategy: SelectionStrategy::FastTyped,
        case_revision: REVISION,
        replaced_by: None,
    }))
    .snapshot()
}

fn selected(id_: u32, action: &str) -> SelectionSnapshot {
    selected_in(id_, 0xc00, action)
}

fn overrule(selection: u32, replacement: u32) -> OverruleSelection {
    OverruleSelection {
        selection_id: id(selection),
        replacement_id: id(replacement),
    }
}

/// A selection cannot be "the stronger selector's selection that replaced it" for itself. The
/// replacement-is-held check passes because the selection holds itself, so the selection ends
/// `Overruled`, naming itself, and nothing that replaced it exists.
#[test]
fn adversary_w4_a_selection_overruled_by_itself_is_refused() {
    let mut record = RequestRecord::default();
    record.put(selected(0x5e1, "metrics.inspect"));

    let outcome = record.overrule(overrule(0x5e1, 0x5e1));

    let held = record.get(&id(0x5e1)).expect("held");
    assert!(
        outcome.is_err(),
        "a selection overruled by itself was accepted: {outcome:?}; it is now {:?} replaced by {:?}",
        held.state,
        held.data.replaced_by
    );
    assert_eq!(held.state, SelectionState::Selected);
    assert_eq!(held.data.replaced_by, None);
}

/// A replacement that is itself overruled replaced nothing: A overruled by B, then B overruled by
/// A, leaves two selections each naming the other and no selection that was ever chosen.
#[test]
fn adversary_w4_a_replacement_that_is_itself_overruled_is_refused() {
    let mut record = RequestRecord::default();
    record.put(selected(0x5e1, "metrics.inspect"));
    record.put(selected(0x5e2, "logs.search"));

    assert!(matches!(
        record.overrule(overrule(0x5e1, 0x5e2)),
        Ok(OverruleSelectionOutcome::Overruled { .. })
    ));
    let cycle = record.overrule(overrule(0x5e2, 0x5e1));

    let states: Vec<_> = record
        .selections()
        .iter()
        .map(|held| (held.state, held.data.replaced_by.clone()))
        .collect();
    assert!(
        cycle.is_err(),
        "an Overruled selection was accepted as a replacement: {cycle:?}; the record now holds \
         {states:?}"
    );
}

/// "Both belong to the run's turn": a replacement made from another catalogue did not replace the
/// fast selection of this one.
#[test]
fn adversary_w4_a_replacement_from_another_catalogue_is_refused() {
    let mut record = RequestRecord::default();
    record.put(selected_in(0x5e1, 0xc00, "metrics.inspect"));
    record.put(selected_in(0x5e2, 0xc01, "logs.search"));

    let outcome = record.overrule(overrule(0x5e1, 0x5e2));

    assert!(
        outcome.is_err(),
        "a replacement from catalogue c01 overruled a selection of catalogue c00: {outcome:?}"
    );
}

/// An `Admitted` and a `Refused` selection are not overruled: `wrong-state`, carrying the state.
#[test]
fn adversary_w4_an_admitted_or_refused_selection_is_not_overruled() {
    for (name, frontier, expected) in [
        (
            "admitted",
            vec!["metrics.inspect".to_owned()],
            SelectionState::Admitted,
        ),
        ("refused", Vec::new(), SelectionState::Refused),
    ] {
        let mut record = RequestRecord::default();
        record.put(selected(0x5e1, "metrics.inspect"));
        record.put(selected(0x5e2, "logs.search"));
        let revalidated = record
            .revalidate_selection(RevalidateSelection {
                selection_id: id(0x5e1),
                case_revision: REVISION,
                frontier_actions: frontier,
            })
            .expect("a declared outcome");
        assert!(
            !matches!(revalidated, RevalidateSelectionOutcome::WrongState { .. }),
            "{name}: {revalidated:?}"
        );

        let outcome = record.overrule(overrule(0x5e1, 0x5e2));
        assert!(
            matches!(
                outcome,
                Ok(OverruleSelectionOutcome::WrongState { ref error }) if error.state == expected
            ),
            "{name}: {outcome:?}"
        );
        let held = record.get(&id(0x5e1)).expect("held");
        assert_eq!(held.state, expected, "{name}");
        assert_eq!(held.data.replaced_by, None, "{name}");
    }
}

/// A selection the record does not hold, named with a held replacement, is `wrong-state` with no
/// state, and nothing is recorded; with an unknown replacement too, the replacement is named.
#[test]
fn adversary_w4_an_unknown_selection_is_refused_and_nothing_is_recorded() {
    let mut record = RequestRecord::default();
    record.put(selected(0x5e2, "logs.search"));

    assert_eq!(
        record.overrule(overrule(0x5e1, 0x5e2)),
        Ok(OverruleSelectionOutcome::WrongStateUnknownInstance)
    );
    assert_eq!(record.selections(), [selected(0x5e2, "logs.search")]);

    assert_eq!(
        record.overrule(overrule(0x5e1, 0x5e9)),
        Err(OverruleRefused::ReplacementNotFound(SelectionNotFound {
            selection_id: id(0x5e9),
        }))
    );
    assert_eq!(record.selections(), [selected(0x5e2, "logs.search")]);
}
