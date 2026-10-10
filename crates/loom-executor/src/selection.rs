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
//! [`HybridSelector`] is the selector whose strategy is `Hybrid`: it returns a fast selector's
//! choice only at or above a confidence threshold the host supplies, and otherwise the stronger
//! selector's. Its [`ActionSelector::resolve`] also names the fast pick it overruled, so Loom
//! records a fallback as two selections, each with its own selector's strategy ([`resolve`]).
//!
//! [`select_action`] is `loom.run.SelectAction` as a command over stored catalogues and
//! selections: the one behaviour ESS leaves to Loom (`generated/rust/loom/PLAN.md`), answered by
//! the same membership rule as [`select`].

use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use serde_json::json;

use crate::harness::wire::{
    Approval, Envelope, Item, ModelPort, Sampling, ToolChoice, ToolName, ToolSpec, TurnRequest,
    Usage, VecSink,
};
use crate::model::behaviour::{ActionCatalogueStorage, SelectionStorage};
use crate::model::json::{Value, decimal_at};
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

/// One selector's pick: its choice, the strategy of the selector that made it, and what making it
/// cost, which Loom records as the selection's telemetry (`loom.run.SelectionRecord`). Telemetry
/// is never evidence (Atlas ADR 0074) and decides nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    /// What the selector chose.
    pub choice: Choice,
    /// How the selector that made the choice chooses.
    pub strategy: SelectionStrategy,
    /// How long the selector took to make the pick, in milliseconds.
    pub latency_ms: i64,
    /// The input tokens the selector reported spending on the pick; 0 when it reported none.
    pub input_tokens: i64,
    /// The output tokens the selector reported spending on the pick; 0 when it reported none.
    pub output_tokens: i64,
}

/// The milliseconds since `started`, saturating.
pub(crate) fn elapsed_ms(started: Instant) -> i64 {
    i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX)
}

/// A token count as the run model's `Integer`, saturating.
pub(crate) fn tokens(count: u64) -> i64 {
    i64::try_from(count).unwrap_or(i64::MAX)
}

/// What a selector's answer resolved to: the pick it chose and, when a stronger selector overruled
/// a fast one to make it, the fast pick it overruled (`story:fallback-selection-recording`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// The pick the selector answers with.
    pub chosen: Pick,
    /// The fast pick a confidence fallback overruled, when there was one. It may name an action
    /// outside the candidates; Loom's membership rule refuses it before it becomes a selection.
    pub overruled: Option<Pick>,
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

    /// [`ActionSelector::select`]'s choice as a pick made with this selector's strategy, and the
    /// pick it overruled, if any. By default nothing is overruled, the pick's latency is the time
    /// `select` took and it reports no tokens; [`HybridSelector`] answers with the pick of the
    /// selector that made its choice and, on a fallback, the fast pick, and
    /// [`ReasoningModelSelector`] reports the tokens of its turn.
    fn resolve(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Resolution, SelectorError> {
        let started = Instant::now();
        let choice = self.select(context, candidates)?;
        Ok(Resolution {
            chosen: Pick {
                choice,
                strategy: self.strategy(),
                latency_ms: elapsed_ms(started),
                input_tokens: 0,
                output_tokens: 0,
            },
            overruled: None,
        })
    }
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
        self.ask(context, candidates).map(|(choice, _)| choice)
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::ReasoningModel
    }

    /// The pick [`ActionSelector::select`] makes, with the time the selection turn took and the
    /// input and output tokens the provider reported for it; 0 for a count it did not report.
    fn resolve(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Resolution, SelectorError> {
        let started = Instant::now();
        let (choice, usage) = self.ask(context, candidates)?;
        let latency_ms = elapsed_ms(started);
        let (input_tokens, output_tokens) = usage.map_or((0, 0), |usage| {
            (tokens(usage.input_tokens), tokens(usage.output_tokens))
        });
        Ok(Resolution {
            chosen: Pick {
                choice,
                strategy: self.strategy(),
                latency_ms,
                input_tokens,
                output_tokens,
            },
            overruled: None,
        })
    }
}

impl<P: ModelPort> ReasoningModelSelector<P> {
    /// One selection turn: the action the model named, and the usage the provider reported for the
    /// turn, when it reported one.
    fn ask(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<(Choice, Option<Usage>), SelectorError> {
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
        Ok((
            Choice {
                action: action.to_owned(),
                confidence: None,
            },
            outcome.usage.clone(),
        ))
    }
}

/// A confidence compared as the number it writes: a decimal in [0, 1].
///
/// [`Decimal`] orders by its rendering, so `0.9` sorts below `0.90`; a `Confidence` is the value
/// itself, so the two are equal. A decimal is what the specification's published `Decimal` pattern
/// accepts ([`decimal_at`]): an optional `-`, digits without a leading zero, then an optional `.`
/// and digits. Only `-0` and its spellings are in range among the signed; anything else, including
/// a leading zero or an exponent, is not a decimal here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Confidence {
    /// Whether the value is exactly 1. Ordered first, so 1 is above every fraction.
    one: bool,
    /// The fractional digits of a value below 1, without trailing zeros: with them gone, the
    /// digit strings order as the fractions they write.
    fraction: String,
}

impl Confidence {
    /// The value `decimal` writes, when it is a decimal in [0, 1]; `None` otherwise.
    pub fn parse(decimal: &Decimal) -> Option<Self> {
        let text = decimal.0.as_str();
        // The one definition of a decimal: the pattern the generated decoders apply.
        decimal_at(&Value::Text(text.to_owned()), "confidence", "a decimal").ok()?;
        let (negative, unsigned) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
        let fraction = fraction.trim_end_matches('0');
        let confidence = match whole {
            "0" => Self {
                one: false,
                fraction: fraction.to_owned(),
            },
            "1" if fraction.is_empty() => Self {
                one: true,
                fraction: String::new(),
            },
            _ => return None,
        };
        if negative && confidence != Self::zero() {
            return None;
        }
        Some(confidence)
    }

    fn zero() -> Self {
        Self {
            one: false,
            fraction: String::new(),
        }
    }
}

/// The threshold given to [`HybridSelector::new`] is not a decimal in [0, 1].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the confidence threshold `{}` is not a decimal in [0, 1]", .0.0)]
pub struct InvalidThreshold(pub Decimal);

/// The selector whose strategy is `Hybrid`: a fast selector, and a stronger one it falls back to
/// (Atlas ADR 0073 § Fallback).
///
/// The fast choice is returned only when it names one of the candidates with a confidence at or
/// above the threshold. Otherwise the stronger selector is asked and its answer, choice or error,
/// is the hybrid's: when the fast confidence is below the threshold, missing, or not a decimal in
/// [0, 1] (which counts as missing), and when the fast selector errs or names an action outside the
/// candidates, which is never returned.
///
/// The threshold has no default: it is calibrated per protocol and supplied by the embedding host
/// for the run's protocol. Confidence and threshold compare as numbers ([`Confidence`]). A
/// confidence decides only which selector's choice is proposed; the choice still passes Loom's
/// membership rule ([`select`]) and revalidation like every other.
pub struct HybridSelector<F, S> {
    fast: F,
    stronger: S,
    threshold: Confidence,
}

impl<F: ActionSelector, S: ActionSelector> HybridSelector<F, S> {
    /// A hybrid of `fast` and `stronger` that accepts a fast choice at or above `threshold`;
    /// refused when `threshold` is not a decimal in [0, 1].
    pub fn new(fast: F, stronger: S, threshold: &Decimal) -> Result<Self, InvalidThreshold> {
        let threshold =
            Confidence::parse(threshold).ok_or_else(|| InvalidThreshold(threshold.clone()))?;
        Ok(Self {
            fast,
            stronger,
            threshold,
        })
    }

    /// Whether `choice`, the fast selector's, is the hybrid's answer from `candidates`.
    fn accepts(&self, choice: &Choice, candidates: &[CatalogueEntry]) -> bool {
        candidates.iter().any(|entry| entry.action == choice.action)
            && choice
                .confidence
                .as_ref()
                .and_then(Confidence::parse)
                .is_some_and(|confidence| confidence >= self.threshold)
    }
}

impl<F: ActionSelector, S: ActionSelector> ActionSelector for HybridSelector<F, S> {
    fn select(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        match self.fast.select(context, candidates) {
            Ok(choice) if self.accepts(&choice, candidates) => Ok(choice),
            _ => self.stronger.select(context, candidates),
        }
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::Hybrid
    }

    /// The fast selector's resolution when it is accepted, as [`ActionSelector::select`] decides;
    /// otherwise the stronger selector's pick, with the fast pick as the one it overruled when the
    /// fast selector answered at all. Each pick carries its own selector's strategy, never
    /// `Hybrid`. Only one overruled pick is kept: a fallback inside the stronger selector is not.
    fn resolve(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Resolution, SelectorError> {
        let fast = self.fast.resolve(context, candidates);
        if let Ok(resolution) = &fast
            && self.accepts(&resolution.chosen.choice, candidates)
        {
            return fast;
        }
        let stronger = self.stronger.resolve(context, candidates)?;
        Ok(Resolution {
            chosen: stronger.chosen,
            overruled: fast.ok().map(|resolution| resolution.chosen),
        })
    }
}

/// `selector`'s choice from `catalogue`, as the selection `selection_id`: refused when the selector
/// picks nothing or names an action `catalogue` does not list. The selection names the catalogue
/// and carries its case revision, and the selector's confidence only when it is a decimal in
/// [0, 1] ([`Confidence`]); any other is recorded as none.
pub fn select(
    selector: &impl ActionSelector,
    context: &SelectionContext,
    catalogue: &ActionCatalogue<action_catalogue_state::Projected>,
    selection_id: SelectionId,
) -> Result<Selection<selection_state::Selected>, SelectionRefusal> {
    let choice = selector
        .select(context, &catalogue.data().entries)
        .map_err(SelectionRefusal::Selector)?;
    chosen(
        catalogue,
        in_range(choice),
        selector.strategy(),
        selection_id,
    )
    .map_err(SelectionRefusal::NotInCatalogue)
}

/// The selection a selector's resolution made, and the fast selection it overruled to make it.
pub struct Resolved {
    /// The selection made from the pick the selector chose, under its selector's strategy.
    pub selection: Selection<selection_state::Selected>,
    /// The selection made from the fast pick a confidence fallback overruled, under the fast
    /// selector's strategy: none when nothing was overruled, and none when the fast pick named an
    /// action the catalogue does not list, which never becomes a selection.
    pub overruled: Option<Selection<selection_state::Selected>>,
    /// The pick `selection` was made from: its latency and tokens are the selection's telemetry.
    pub pick: Pick,
    /// The pick `overruled` was made from, exactly when there is an `overruled` selection.
    pub overruled_pick: Option<Pick>,
}

/// `selector`'s resolution from `catalogue` ([`ActionSelector::resolve`]): the chosen pick as the
/// selection `selection_id`, and an overruled fast pick as the selection `overruled_id`. Each
/// carries the strategy of the selector that made it, and its confidence by the rule [`select`]
/// applies. Refused as [`select`] refuses: when the selector picks nothing or its chosen pick names
/// an action `catalogue` does not list. The overruled selection is still `Selected`: the caller
/// records it overruled by the chosen one (`loom.run.OverruleSelection`).
pub fn resolve(
    selector: &impl ActionSelector,
    context: &SelectionContext,
    catalogue: &ActionCatalogue<action_catalogue_state::Projected>,
    (selection_id, overruled_id): (SelectionId, SelectionId),
) -> Result<Resolved, SelectionRefusal> {
    let resolution = selector
        .resolve(context, &catalogue.data().entries)
        .map_err(SelectionRefusal::Selector)?;
    let selection = chosen(
        catalogue,
        in_range(resolution.chosen.choice.clone()),
        resolution.chosen.strategy,
        selection_id,
    )
    .map_err(SelectionRefusal::NotInCatalogue)?;
    let (overruled, overruled_pick) = resolution
        .overruled
        .and_then(|pick| {
            chosen(
                catalogue,
                in_range(pick.choice.clone()),
                pick.strategy,
                overruled_id,
            )
            .ok()
            .map(|selection| (selection, pick))
        })
        .unzip();
    Ok(Resolved {
        selection,
        overruled,
        pick: resolution.chosen,
        overruled_pick,
    })
}

/// `choice` with its confidence only when that is a decimal in [0, 1] ([`Confidence`]).
fn in_range(mut choice: Choice) -> Choice {
    choice.confidence = choice
        .confidence
        .filter(|confidence| Confidence::parse(confidence).is_some());
    choice
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
        replaced_by: None,
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
