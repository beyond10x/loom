//! Revalidation of a selection against the current case revision and frontier before it is
//! proposed (`loom.run.RevalidateSelection`; Atlas ADR 0072, ADR 0073 step 3).
//!
//! The behaviour is the one ESS generates, over the [`RequestRecord`]: a selection made at another
//! case revision than the input's is refused `stale-revision`; one whose action the input's
//! `frontier_actions` do not list is refused `not-in-frontier`; any other is `admitted`. Each
//! refusal moves the selection to `Refused`, an admission to `Admitted`, and every outcome is
//! recorded.
//!
//! `not-in-frontier` is an `external:` outcome: ess 0.57.0 does not synthesize it as a membership
//! guard over `frontier_actions`. The context here answers it from the command input it is handed
//! and the selection the record holds, and refuses to answer any other external branch.
//!
//! The revision and the action ids are the caller's to read from the governor, never from the
//! model: [`crate::Loom::with_governor`].

use crate::arguments::RequestRecord;
use crate::model::behaviour::{
    ExternalCommand, Generated, SelectionStorage, TryContext, unmet_context,
};
use crate::model::obligation::UnmetObligation;
use crate::model::run::obligations::RevalidateSelectionBehavior;
use crate::model::run::{
    RevalidateSelection, RevalidateSelectionOutcome, SelectionId, SelectionSnapshot,
};

/// The record, as the storage and context ports of the generated `RevalidateSelection`.
struct Revalidating<'r> {
    record: &'r mut RequestRecord,
}

impl SelectionStorage for Revalidating<'_> {
    fn get(&self, identity: &SelectionId) -> Option<SelectionSnapshot> {
        SelectionStorage::get(self.record, identity)
    }

    fn put(&mut self, snapshot: SelectionSnapshot) {
        SelectionStorage::put(self.record, snapshot);
    }

    fn delete(&mut self, identity: &SelectionId) {
        SelectionStorage::delete(self.record, identity);
    }

    fn list(&self) -> Vec<SelectionSnapshot> {
        SelectionStorage::list(self.record)
    }
}

impl TryContext for Revalidating<'_> {
    /// `not-in-frontier` is taken when the input's `frontier_actions` do not list the action of the
    /// selection it names, or when the record holds no such selection. Any other external branch
    /// is unanswered, so the command is refused rather than decided by a default.
    fn try_external(
        &mut self,
        command: ExternalCommand<'_>,
        outcome: &'static str,
    ) -> Result<bool, UnmetObligation> {
        match (command, outcome) {
            (ExternalCommand::LoomRunRevalidateSelection(input), "not-in-frontier") => {
                Ok(SelectionStorage::get(self.record, &input.selection_id)
                    .is_none_or(|held| !input.frontier_actions.contains(&held.data.action)))
            }
            _ => Err(unmet_context("loom.run.RevalidateSelection")),
        }
    }
}

/// `loom.run.RevalidateSelection`, generated, over this record; the outcome is recorded
/// ([`RequestRecord::revalidations`]).
impl RevalidateSelectionBehavior for RequestRecord {
    fn revalidate_selection(
        &mut self,
        input: RevalidateSelection,
    ) -> Result<RevalidateSelectionOutcome, UnmetObligation> {
        let outcome = Generated::new(Revalidating { record: self }).revalidate_selection(input)?;
        self.record_revalidation(outcome.clone());
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::primitives::Uuid;
    use crate::model::run::{
        AnySelection, CatalogueId, Selection, SelectionData, SelectionState, SelectionStrategy,
        selection_state,
    };

    fn uuid(n: u32) -> Uuid {
        Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
    }

    fn selected(id: u32) -> Selection<selection_state::Selected> {
        Selection::new(SelectionData {
            selection_id: SelectionId(uuid(id)),
            catalogue_id: CatalogueId(uuid(0xc00)),
            action: "tests.run".to_owned(),
            confidence: None,
            strategy: SelectionStrategy::Rule,
            case_revision: 3,
            replaced_by: None,
        })
    }

    /// A selection revalidated once is no longer `Selected`: a second revalidation is refused as
    /// a wrong state, and is recorded like the first.
    #[test]
    fn a_selection_is_revalidated_once() {
        let mut record = RequestRecord::default();
        record.put(AnySelection::Selected(selected(0x5e1)).snapshot());
        let input = RevalidateSelection {
            selection_id: SelectionId(uuid(0x5e1)),
            case_revision: 3,
            frontier_actions: vec!["tests.run".to_owned()],
        };

        let first = record.revalidate_selection(input.clone());
        let second = record.revalidate_selection(input);

        assert!(
            matches!(first, Ok(RevalidateSelectionOutcome::Admitted { .. })),
            "{first:?}"
        );
        assert!(
            matches!(
                second,
                Ok(RevalidateSelectionOutcome::WrongState { ref error })
                    if error.state == SelectionState::Admitted
            ),
            "{second:?}"
        );
        assert_eq!(record.revalidations().len(), 2);
        assert_eq!(record.selections()[0].state, SelectionState::Admitted);
    }

    /// A selection the record does not hold is never admitted.
    #[test]
    fn an_unknown_selection_is_not_admitted() {
        let mut record = RequestRecord::default();

        let outcome = record.revalidate_selection(RevalidateSelection {
            selection_id: SelectionId(uuid(0x5e9)),
            case_revision: 3,
            frontier_actions: vec!["tests.run".to_owned()],
        });

        assert!(
            !matches!(outcome, Ok(RevalidateSelectionOutcome::Admitted { .. })),
            "{outcome:?}"
        );
    }
}
