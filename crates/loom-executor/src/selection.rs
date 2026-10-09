//! Action selection from the projected catalogue (Atlas ADR 0073,
//! `docs/contracts/loom-action-selection.md`).
//!
//! A selector is handed the selection context and the catalogue's entries, and nothing else: it
//! cannot add a capability, execute anything or decide authority. It answers with one action id
//! and an optional confidence. Loom refuses an id the catalogue does not list, whatever the
//! confidence (`loom.run.SelectAction`, outcome `not-in-catalogue`), and only an accepted choice
//! becomes the synthesized `loom.run.Selection`.
//!
//! [`ReasoningModelSelector`] is the selector whose strategy is `ReasoningModel`: it asks a model
//! through the provider-neutral [`ModelPort`] to choose among exactly the candidates it was given.
//!
//! [`select_action`] is `loom.run.SelectAction` as a command over stored catalogues and
//! selections: the one behaviour ESS leaves to Loom (`generated/rust/loom/PLAN.md`), answered by
//! the same membership rule as [`select`].

use std::sync::{Mutex, PoisonError};

use serde_json::json;

use crate::harness::wire::{
    Approval, Envelope, Item, ModelPort, Sampling, ToolChoice, ToolName, ToolSpec, TurnRequest,
    VecSink,
};
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

/// The one tool a reasoning-model selection turn publishes and holds the model to.
const SELECT_TOOL: &str = "select_action";

/// What the reasoning model is told a selection turn is for.
const SELECT_INSTRUCTIONS: &str = "Choose the one action, of the candidates listed, that best \
advances the instruction. Answer only by calling `select_action` with that action's id. You \
choose; you do not supply arguments, run anything or grant authority.";

/// The selector whose strategy is `ReasoningModel`: the stronger path fast selection falls back to
/// (Atlas ADR 0073 § Fallback).
///
/// Each selection is one turn of the model behind `P`, the provider-neutral [`ModelPort`]. The turn
/// publishes one tool, `select_action`, and is held to it ([`ToolChoice::Named`]); the tool's only
/// argument is `action`, a fixed choice of exactly the candidate ids it was handed. The model
/// chooses; it is told no arguments are wanted, and nothing it answers executes or decides
/// authority.
///
/// The `action` the model names is returned as it is, without confidence. Whether it is a
/// candidate is decided by Loom's membership rule ([`select`]), the same for every selector, so an
/// action outside the set is refused as `not-in-catalogue` and never becomes a selection. An answer
/// that names no action (a wire failure, prose, another tool, more than one call, an `action` that
/// is not a string) is [`SelectorError::Unavailable`], never a guess.
///
/// [`ActionSelector::select`] takes `&self` and [`ModelPort::turn`] `&mut self`, so the port is held
/// behind a mutex: one selection turn at a time.
pub struct ReasoningModelSelector<P> {
    model: Mutex<P>,
    model_name: String,
}

impl<P: ModelPort> ReasoningModelSelector<P> {
    /// A selector that asks `model_name` through `model`.
    pub fn new(model: P, model_name: impl Into<String>) -> Self {
        Self {
            model: Mutex::new(model),
            model_name: model_name.into(),
        }
    }

    /// The turn that asks the model to choose one of `candidates` in `context`.
    fn request(&self, context: &SelectionContext, candidates: &[CatalogueEntry]) -> TurnRequest {
        let mut ids: Vec<&str> = Vec::with_capacity(candidates.len());
        for entry in candidates {
            if !ids.contains(&entry.action.as_str()) {
                ids.push(&entry.action);
            }
        }
        let listed = candidates
            .iter()
            .map(|entry| {
                let status = match entry.status {
                    CatalogueEntryStatus::Admissible => "admissible",
                    CatalogueEntryStatus::ApprovalRequired => "admissible once authorized",
                };
                format!("- `{}` ({status})", entry.action)
            })
            .collect::<Vec<_>>()
            .join("\n");
        let name = ToolName::new(SELECT_TOOL).expect("the selection tool's name is a valid id");
        TurnRequest {
            model: self.model_name.clone(),
            instructions: SELECT_INSTRUCTIONS.to_owned(),
            items: vec![
                Item::user(context.prompt.clone()),
                Item::user(format!("Candidates:\n{listed}")),
            ],
            tools: vec![ToolSpec {
                name: name.clone(),
                description: "Selects the next action: one of the candidate ids, and no other."
                    .to_owned(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ids },
                    },
                    "required": ["action"],
                    "additionalProperties": false,
                }),
                approval: Approval::NotRequired,
                envelope: Envelope::default(),
            }],
            max_output_tokens: None,
            sampling: Sampling::default(),
            tool_choice: ToolChoice::Named(name),
        }
    }
}

impl<P: ModelPort> ActionSelector for ReasoningModelSelector<P> {
    fn select(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        if candidates.is_empty() {
            return Err(SelectorError::NothingAdmissible);
        }
        let request = self.request(context, candidates);
        let outcome = self
            .model
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .turn(&request, &mut VecSink::new())
            .map_err(|error| {
                SelectorError::Unavailable(format!("the reasoning model did not answer: {error}"))
            })?;
        let mut calls = outcome.tool_calls();
        let (Some(call), None) = (calls.next(), calls.next()) else {
            return Err(SelectorError::Unavailable(
                "the reasoning model did not answer with exactly one selection call".to_owned(),
            ));
        };
        if call.name.as_str() != SELECT_TOOL {
            return Err(SelectorError::Unavailable(format!(
                "the reasoning model called `{}`, not `{SELECT_TOOL}`",
                call.name
            )));
        }
        let action = call
            .arguments
            .get("action")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                SelectorError::Unavailable(
                    "the reasoning model's selection call names no action".to_owned(),
                )
            })?;
        Ok(Choice {
            action: action.to_owned(),
            confidence: None,
        })
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::ReasoningModel
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
