//! The model-visible action catalogue, projected from the current frontier (Atlas ADR 0072).
//!
//! The catalogue lists what the frontier lists and nothing else: each distinct action once, at its
//! first position in the frontier, with the status Commission's admission gives it. `Admissible`
//! is projected `Admissible`, `NeedsAuthority` is projected `ApprovalRequired`, and an action
//! Commission refuses is not projected. Where the frontier lists one action more than once, the
//! least-authority entry decides, whatever their order: the rule Loom's executor already follows
//! (Atlas ADR 0082). Nothing is visible because it was registered at startup. Loom adds no protocol
//! or engineering semantics: why an action is blocked is the frontier's to say.
//!
//! [`CatalogueRecord`] keeps projected catalogues for `loom.run.ProjectCatalogue`, the generated
//! behaviour over it: a catalogue identity is projected once.

use std::collections::BTreeSet;

use b10x_loom_commission::admission::admit;
use b10x_loom_commission::model::responsibility::{Admission, Frontier, frontier_state};

use crate::model::behaviour::{ActionCatalogueStorage, Generated};
use crate::model::obligation::UnmetObligation;
use crate::model::run::obligations::ProjectCatalogueBehavior;
use crate::model::run::{
    ActionCatalogue, ActionCatalogueData, ActionCatalogueSnapshot, CatalogueEntry,
    CatalogueEntryStatus, CatalogueId, ProjectCatalogue, ProjectCatalogueOutcome, TurnId,
    action_catalogue_state,
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

/// Projected catalogues, in the order projected. A catalogue projected under an identity already
/// held is refused `catalogue-exists` by `loom.run.ProjectCatalogue`; one stored through
/// [`ActionCatalogueStorage`] under an identity already held replaces it.
#[derive(Debug, Default)]
pub struct CatalogueRecord {
    catalogues: Vec<ActionCatalogueSnapshot>,
}

impl CatalogueRecord {
    /// Every catalogue projected here, in the order projected.
    #[must_use]
    pub fn catalogues(&self) -> &[ActionCatalogueSnapshot] {
        &self.catalogues
    }
}

impl ActionCatalogueStorage for CatalogueRecord {
    fn get(&self, identity: &CatalogueId) -> Option<ActionCatalogueSnapshot> {
        self.catalogues
            .iter()
            .find(|held| &held.data.catalogue_id == identity)
            .cloned()
    }

    fn put(&mut self, snapshot: ActionCatalogueSnapshot) {
        match self
            .catalogues
            .iter_mut()
            .find(|held| held.data.catalogue_id == snapshot.data.catalogue_id)
        {
            Some(held) => *held = snapshot,
            None => self.catalogues.push(snapshot),
        }
    }

    fn delete(&mut self, identity: &CatalogueId) {
        self.catalogues
            .retain(|held| &held.data.catalogue_id != identity);
    }

    fn list(&self) -> Vec<ActionCatalogueSnapshot> {
        self.catalogues.clone()
    }
}

/// `loom.run.ProjectCatalogue`, generated, over this record: a catalogue identity is projected
/// once.
impl ProjectCatalogueBehavior for CatalogueRecord {
    fn project_catalogue(
        &mut self,
        input: ProjectCatalogue,
    ) -> Result<ProjectCatalogueOutcome, UnmetObligation> {
        let mut generated = Generated::new(std::mem::take(self));
        let answer = generated.project_catalogue(input);
        *self = generated.ports;
        answer
    }
}
