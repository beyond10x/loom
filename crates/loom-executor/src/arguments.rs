//! Argument generation for the selected action only (Atlas ADR 0073, step 1;
//! `docs/contracts/loom-action-selection.md`).
//!
//! A generator is handed the argument context and the one catalogue entry the selection names,
//! never the rest of the catalogue, and returns a JSON value. That value becomes the proposed
//! action's `ProposedActionArguments`. Before arguments are generated, the request for them is
//! recorded against the selection it serves (`loom.run.RequestArguments`, outcome `requested`),
//! in the [`RequestRecord`].
//!
//! Validating the arguments against the action's schema (ADR 0073, step 2) is not done here:
//! where an action schema comes from is still undecided (`decision-blocker:action-argument-schema`).

use b10x_loom_commission::model::json::Value;

use crate::model::behaviour::SelectionStorage;
use crate::model::obligation::UnmetObligation;
use crate::model::run::obligations::RequestArgumentsBehavior;
use crate::model::run::{
    AnyArgumentRequest, ArgumentRequest, ArgumentRequestData, ArgumentRequestSnapshot,
    ArgumentsRequested, CatalogueEntry, RequestArguments, RequestArgumentsOutcome,
    RevalidateSelectionOutcome, SelectionId, SelectionNotFound, SelectionNotSelected,
    SelectionSnapshot, SelectionState,
};

/// What a generator is told besides the selected entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentContext {
    /// The instruction the run is working on.
    pub prompt: String,
}

/// Generates the arguments of one selected action.
pub trait ArgumentGenerator {
    /// The arguments for `entry`, the catalogue entry the selection names, in `context`. An `Err`
    /// is a generator that could not answer, for the reason given; Loom answers it with
    /// `Suspended(ExternalAvailability)` carrying the reason.
    fn generate(&self, context: &ArgumentContext, entry: &CatalogueEntry) -> Result<Value, String>;
}

/// Bootstrap argument generator: always the empty object.
#[derive(Debug, Default)]
pub struct EmptyObjectArguments;

impl ArgumentGenerator for EmptyObjectArguments {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        Ok(Value::Object(Vec::new()))
    }
}

/// The selections Loom made, the argument requests that serve them and the outcome of every
/// revalidation of them ([`crate::revalidation`]), in the order they were recorded. A selection or
/// argument request stored under an identity already held replaces it.
#[derive(Debug, Default)]
pub struct RequestRecord {
    selections: Vec<SelectionSnapshot>,
    argument_requests: Vec<ArgumentRequestSnapshot>,
    revalidations: Vec<RevalidateSelectionOutcome>,
}

impl RequestRecord {
    /// Every recorded selection.
    #[must_use]
    pub fn selections(&self) -> &[SelectionSnapshot] {
        &self.selections
    }

    /// Every recorded argument request.
    #[must_use]
    pub fn argument_requests(&self) -> &[ArgumentRequestSnapshot] {
        &self.argument_requests
    }

    /// The outcome of every revalidation, in the order they were made.
    #[must_use]
    pub fn revalidations(&self) -> &[RevalidateSelectionOutcome] {
        &self.revalidations
    }

    /// Records the outcome of one revalidation.
    pub(crate) fn record_revalidation(&mut self, outcome: RevalidateSelectionOutcome) {
        self.revalidations.push(outcome);
    }
}

impl SelectionStorage for RequestRecord {
    fn get(&self, identity: &SelectionId) -> Option<SelectionSnapshot> {
        self.selections
            .iter()
            .find(|held| &held.data.selection_id == identity)
            .cloned()
    }

    fn put(&mut self, snapshot: SelectionSnapshot) {
        match self
            .selections
            .iter_mut()
            .find(|held| held.data.selection_id == snapshot.data.selection_id)
        {
            Some(held) => *held = snapshot,
            None => self.selections.push(snapshot),
        }
    }

    fn delete(&mut self, identity: &SelectionId) {
        self.selections
            .retain(|held| &held.data.selection_id != identity);
    }

    fn list(&self) -> Vec<SelectionSnapshot> {
        self.selections.clone()
    }
}

/// `loom.run.RequestArguments`: refused for a selection the record does not hold
/// (`selection-unknown`) or one no longer `Selected` (`selection-not-selected`); otherwise the
/// `loom.run.ArgumentRequest` is recorded against the selection (`requested`).
impl RequestArgumentsBehavior for RequestRecord {
    fn request_arguments(
        &mut self,
        input: RequestArguments,
    ) -> Result<RequestArgumentsOutcome, UnmetObligation> {
        let Some(selection) = SelectionStorage::get(self, &input.selection_id) else {
            return Ok(RequestArgumentsOutcome::SelectionUnknown {
                error: SelectionNotFound {
                    selection_id: input.selection_id,
                },
            });
        };
        if selection.state != SelectionState::Selected {
            return Ok(RequestArgumentsOutcome::SelectionNotSelected {
                error: SelectionNotSelected {
                    selection_id: input.selection_id,
                },
            });
        }
        let snapshot = AnyArgumentRequest::Requested(ArgumentRequest::new(ArgumentRequestData {
            argument_request_id: input.argument_request_id.clone(),
            selection_id: input.selection_id.clone(),
        }))
        .snapshot();
        match self
            .argument_requests
            .iter_mut()
            .find(|held| held.data.argument_request_id == snapshot.data.argument_request_id)
        {
            Some(held) => *held = snapshot,
            None => self.argument_requests.push(snapshot),
        }
        Ok(RequestArgumentsOutcome::Requested {
            arguments_requested: ArgumentsRequested {
                argument_request_id: input.argument_request_id,
                selection_id: input.selection_id,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::primitives::Uuid;
    use crate::model::run::{
        AnySelection, ArgumentRequestId, ArgumentRequestState, CatalogueId, Selection,
        SelectionData, SelectionStrategy,
    };

    fn uuid(n: u32) -> Uuid {
        Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
    }

    fn selected(id: u32) -> Selection<crate::model::run::selection_state::Selected> {
        Selection::new(SelectionData {
            selection_id: SelectionId(uuid(id)),
            catalogue_id: CatalogueId(uuid(0xc00)),
            action: "tests.run".to_owned(),
            confidence: None,
            strategy: SelectionStrategy::Rule,
            case_revision: 1,
        })
    }

    fn request(request: u32, selection: u32) -> RequestArguments {
        RequestArguments {
            argument_request_id: ArgumentRequestId(uuid(request)),
            selection_id: SelectionId(uuid(selection)),
        }
    }

    /// The selection and the argument request get different ids here, as `Loom::run` gives them, so
    /// a request that names its own id, or any id but its selection's, is caught.
    #[test]
    fn a_request_names_the_selection_it_serves() {
        let mut record = RequestRecord::default();
        record.put(AnySelection::Selected(selected(0x5e1)).snapshot());
        record.put(AnySelection::Selected(selected(0x5e2)).snapshot());

        let outcome = record.request_arguments(request(0xa01, 0x5e2));

        assert_eq!(
            outcome,
            Ok(RequestArgumentsOutcome::Requested {
                arguments_requested: ArgumentsRequested {
                    argument_request_id: ArgumentRequestId(uuid(0xa01)),
                    selection_id: SelectionId(uuid(0x5e2)),
                },
            })
        );
        let requests = record.argument_requests();
        assert_eq!(requests.len(), 1, "{requests:?}");
        assert_eq!(requests[0].state, ArgumentRequestState::Requested);
        assert_eq!(
            requests[0].data.argument_request_id,
            ArgumentRequestId(uuid(0xa01))
        );
        assert_eq!(requests[0].data.selection_id, SelectionId(uuid(0x5e2)));
    }

    /// `selection-unknown`: nothing is recorded for a selection the record does not hold.
    #[test]
    fn a_request_for_an_unknown_selection_is_refused() {
        let mut record = RequestRecord::default();
        record.put(AnySelection::Selected(selected(0x5e1)).snapshot());

        let outcome = record.request_arguments(request(0xa01, 0x5e9));

        assert_eq!(
            outcome,
            Ok(RequestArgumentsOutcome::SelectionUnknown {
                error: SelectionNotFound {
                    selection_id: SelectionId(uuid(0x5e9)),
                },
            })
        );
        assert!(record.argument_requests().is_empty());
    }

    /// `selection-not-selected`: nothing is recorded for a selection no longer `Selected`.
    #[test]
    fn a_request_for_a_refused_selection_is_refused() {
        let mut record = RequestRecord::default();
        record.put(AnySelection::Refused(selected(0x5e1).refuse()).snapshot());

        let outcome = record.request_arguments(request(0xa01, 0x5e1));

        assert_eq!(
            outcome,
            Ok(RequestArgumentsOutcome::SelectionNotSelected {
                error: SelectionNotSelected {
                    selection_id: SelectionId(uuid(0x5e1)),
                },
            })
        );
        assert!(record.argument_requests().is_empty());
    }
}
