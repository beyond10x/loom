//! Action selection from the projected catalogue (Atlas ADR 0073,
//! `docs/contracts/loom-action-selection.md`).
//!
//! A selector is handed the selection context and the catalogue's entries, and nothing else: it
//! cannot add a capability, execute anything or decide authority. It answers with one action id
//! and an optional confidence. Loom refuses an id the catalogue does not list, whatever the
//! confidence (`loom.run.SelectAction`, outcome `not-in-catalogue`), and only an accepted choice
//! becomes the synthesized `loom.run.Selection`.

use crate::model::primitives::Decimal;
use crate::model::run::{
    ActionCatalogue, ActionNotInCatalogue, CatalogueEntry, CatalogueEntryStatus, Selection,
    SelectionData, SelectionId, SelectionStrategy, action_catalogue_state, selection_state,
};

/// What a selector is told besides the candidates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionContext {
    /// The instruction the run is working on.
    pub prompt: String,
}

/// A selector's answer: one action id and how confident it is. Confidence never grants authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// The `action` of one of the candidates the selector was handed.
    pub action: String,
    /// The selector's confidence in `action`, where it has one.
    pub confidence: Option<Decimal>,
}

/// Why a selector picked no action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectorError {
    /// The candidates offer nothing to select. Loom answers it with `NoUsefulAction`.
    NothingAdmissible,
    /// The selector could not answer, for the reason given. Loom answers it with
    /// `Suspended(ExternalAvailability)` carrying the reason.
    Unavailable(String),
}

/// Picks the action Loom proposes next, from the projected catalogue only.
pub trait ActionSelector {
    /// One of `candidates`, chosen in `context`.
    fn select(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError>;

    /// How this selector chooses, recorded on every selection it makes.
    fn strategy(&self) -> SelectionStrategy;
}

/// Why Loom made no selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionRefusal {
    /// The selector picked nothing.
    Selector(SelectorError),
    /// The selector named an action the catalogue does not list.
    NotInCatalogue(ActionNotInCatalogue),
}

/// Deterministic bootstrap selector used only for tests/examples: the first `Admissible` entry.
#[derive(Debug, Default)]
pub struct FirstAdmissibleSelector;

impl ActionSelector for FirstAdmissibleSelector {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        candidates
            .iter()
            .find(|entry| entry.status == CatalogueEntryStatus::Admissible)
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

/// `selector`'s choice from `catalogue`, as the selection `selection_id`: refused when the selector
/// picks nothing or names an action `catalogue` does not list. The selection names the catalogue
/// and carries its case revision.
pub fn select(
    selector: &impl ActionSelector,
    context: &SelectionContext,
    catalogue: &ActionCatalogue<action_catalogue_state::Projected>,
    selection_id: SelectionId,
) -> Result<Selection<selection_state::Selected>, SelectionRefusal> {
    let data = catalogue.data();
    let choice = selector
        .select(context, &data.entries)
        .map_err(SelectionRefusal::Selector)?;
    if !data
        .entries
        .iter()
        .any(|entry| entry.action == choice.action)
    {
        return Err(SelectionRefusal::NotInCatalogue(ActionNotInCatalogue {
            action: choice.action,
        }));
    }
    Ok(Selection::new(SelectionData {
        selection_id,
        catalogue_id: data.catalogue_id.clone(),
        action: choice.action,
        confidence: choice.confidence,
        strategy: selector.strategy(),
        case_revision: data.case_revision,
    }))
}
