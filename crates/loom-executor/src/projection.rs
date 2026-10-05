//! The model-visible action catalogue, projected from the current frontier (Atlas ADR 0072).
//!
//! The catalogue lists what the frontier lists and nothing else: each distinct action once, at its
//! first position in the frontier, with the status Commission's admission gives it. `Admissible`
//! is projected `Admissible`, `NeedsAuthority` is projected `ApprovalRequired`, and an action
//! Commission refuses is not projected. Where the frontier lists one action more than once, the
//! least-authority entry decides, whatever their order: the rule Loom's executor already follows
//! (Atlas ADR 0082). Nothing is visible because it was registered at startup. Loom adds no protocol
//! or engineering semantics: why an action is blocked is the frontier's to say.

use std::collections::BTreeSet;

use b10x_loom_commission::admission::admit;
use b10x_loom_commission::model::responsibility::{Admission, Frontier, frontier_state};

use crate::model::run::{
    ActionCatalogue, ActionCatalogueData, CatalogueEntry, CatalogueEntryStatus, CatalogueId,
    TurnId, action_catalogue_state,
};

/// The catalogue `catalogue_id` for turn `turn_id`, projected from `frontier`: it names the
/// frontier by id and carries the frontier's case revision.
pub fn project(
    frontier: &Frontier<frontier_state::Issued>,
    catalogue_id: CatalogueId,
    turn_id: TurnId,
) -> ActionCatalogue<action_catalogue_state::Projected> {
    let data = frontier.data();
    let mut seen = BTreeSet::new();
    let entries = data
        .actions
        .iter()
        .filter(|listed| seen.insert(listed.action.as_str()))
        .filter_map(|listed| {
            entry_status(&admit(frontier, &listed.action)).map(|status| CatalogueEntry {
                action: listed.action.clone(),
                status,
            })
        })
        .collect();
    ActionCatalogue::new(ActionCatalogueData {
        catalogue_id,
        turn_id,
        frontier: data.frontier_id.0.0.clone(),
        case_revision: data.case_revision,
        entries,
    })
}

/// The catalogue status of Commission's admission of an action; `None` for one it refuses.
/// Exhaustive on purpose: an admission Commission adds is decided here, not defaulted.
fn entry_status(admission: &Admission) -> Option<CatalogueEntryStatus> {
    match admission {
        Admission::Admissible(_) => Some(CatalogueEntryStatus::Admissible),
        Admission::NeedsAuthority(_) => Some(CatalogueEntryStatus::ApprovalRequired),
        Admission::Refused(_) => None,
    }
}
