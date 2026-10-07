//! Action selection from the projected catalogue (Atlas ADR 0073,
//! `docs/contracts/loom-action-selection.md`).
//!
//! A selector is handed the selection context and the catalogue's entries, and nothing else: it
//! cannot add a capability, execute anything or decide authority. It answers with one action id
//! and an optional confidence. Loom refuses an id the catalogue does not list, whatever the
//! confidence (`loom.run.SelectAction`, outcome `not-in-catalogue`), and only an accepted choice
//! becomes the synthesized `loom.run.Selection`.
//!
//! [`select_action`] is `loom.run.SelectAction` as a command over stored catalogues and
//! selections: the one behaviour ESS leaves to Loom (`generated/rust/loom/PLAN.md`), answered by
//! the same membership rule as [`select`].

use crate::model::behaviour::{ActionCatalogueStorage, SelectionStorage};
use crate::model::primitives::Decimal;
use crate::model::run::{
    ActionCatalogue, ActionNotInCatalogue, ActionSelected, AnyActionCatalogue, AnySelection,
    CatalogueEntry, CatalogueEntryStatus, CatalogueNotFound, CatalogueRevisionMismatch,
    SelectAction, SelectActionOutcome, Selection, SelectionData, SelectionId, SelectionStrategy,
    action_catalogue_state, selection_state,
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
    let choice = selector
        .select(context, &catalogue.data().entries)
        .map_err(SelectionRefusal::Selector)?;
    chosen(catalogue, choice, selector.strategy(), selection_id)
        .map_err(SelectionRefusal::NotInCatalogue)
}

/// `choice` from `catalogue`, as the selection `selection_id` made by `strategy`: refused, naming
/// the action, when `catalogue` does not list it, whatever the confidence. The selection names the
/// catalogue and carries its case revision.
fn chosen(
    catalogue: &ActionCatalogue<action_catalogue_state::Projected>,
    choice: Choice,
    strategy: SelectionStrategy,
    selection_id: SelectionId,
) -> Result<Selection<selection_state::Selected>, ActionNotInCatalogue> {
    let data = catalogue.data();
    if !data
        .entries
        .iter()
        .any(|entry| entry.action == choice.action)
    {
        return Err(ActionNotInCatalogue {
            action: choice.action,
        });
    }
    Ok(Selection::new(SelectionData {
        selection_id,
        catalogue_id: data.catalogue_id.clone(),
        action: choice.action,
        confidence: choice.confidence,
        strategy,
        case_revision: data.case_revision,
    }))
}

/// `loom.run.SelectAction`: the action `input` names, selected from the catalogue `catalogues`
/// holds under `input.catalogue_id`, and stored in `selections`.
///
/// The outcomes, in the order `ess/domains/run.yaml` declares them: `catalogue-unknown` for a
/// catalogue `catalogues` does not hold; `not-in-catalogue`, by the rule [`select`] applies to
/// every selector, for an action the catalogue does not list, whatever the confidence;
/// `revision-mismatch` when the case revision `input` claims is not the catalogue's; otherwise
/// `selected`, and the selection is stored `Selected`, replacing one held under its identity.
/// Nothing is stored on a refusal.
pub fn select_action(
    catalogues: &impl ActionCatalogueStorage,
    selections: &mut impl SelectionStorage,
    input: SelectAction,
) -> SelectActionOutcome {
    let Some(held) = catalogues.get(&input.catalogue_id) else {
        return SelectActionOutcome::CatalogueUnknown {
            error: CatalogueNotFound {
                catalogue_id: input.catalogue_id,
            },
        };
    };
    let AnyActionCatalogue::Projected(catalogue) = held.refine();
    let choice = Choice {
        action: input.action,
        confidence: input.confidence,
    };
    let selection = match chosen(&catalogue, choice, input.strategy, input.selection_id) {
        Ok(selection) => selection,
        Err(error) => return SelectActionOutcome::NotInCatalogue { error },
    };
    let catalogue_revision = catalogue.data().case_revision;
    if input.case_revision != catalogue_revision {
        return SelectActionOutcome::RevisionMismatch {
            error: CatalogueRevisionMismatch {
                catalogue_id: input.catalogue_id,
                case_revision: input.case_revision,
                catalogue_revision,
            },
        };
    }
    let data = selection.data();
    let action_selected = ActionSelected {
        selection_id: data.selection_id.clone(),
        catalogue_id: data.catalogue_id.clone(),
        action: data.action.clone(),
    };
    selections.put(AnySelection::Selected(selection).snapshot());
    SelectActionOutcome::Selected { action_selected }
}
