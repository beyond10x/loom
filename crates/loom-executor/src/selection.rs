//! Action selection from the projected catalogue (Atlas ADR 0073,
//! `docs/contracts/loom-action-selection.md`).
//!
//! A selector is handed the selection context and the catalogue's entries, and nothing else: it
//! cannot add a capability, execute anything or decide authority. It answers with one action id
//! and an optional confidence. Loom refuses an id the catalogue does not list, whatever the
//! confidence (`loom.run.SelectAction`, outcome `not-in-catalogue`), and only an accepted choice
//! becomes the synthesized `loom.run.Selection`.
//!
//! The selector whose strategy is `ReasoningModel` is [`crate::model_selector::ModelSelector`]: it
//! asks an llm `Model` to choose among exactly the candidates it was given, and Loom's membership
//! rule ([`select`]) refuses any other answer, so an action outside them is a selection error and
//! never a selection. [`HybridSelector`] is the selector whose strategy is `Hybrid`: it returns a
//! fast selector's choice only at or above a confidence threshold the host supplies, and otherwise
//! the stronger selector's. Its [`ActionSelector::resolve`] also names the fast pick it overruled,
//! so Loom records a fallback as two selections, each with its own selector's strategy
//! ([`resolve`]).
//!
//! [`DeterministicSelector`] is the selector whose strategy is `Deterministic`: it calls no model,
//! keeps only admissible candidates that are useful, takes reads before writes and then orders by
//! action id. What it knows beyond a candidate's id and status comes from two host ports,
//! [`ActionFactsPort`] and [`StepHistory`], never from a model or the candidate list.
//!
//! [`select_action`] is `loom.run.SelectAction` as a command over stored catalogues and
//! selections: the one behaviour ESS leaves to Loom (`generated/rust/loom/PLAN.md`), answered by
//! the same membership rule as [`select`].

use std::time::Instant;

use b10x_loom_commission::model::responsibility::CaseId;

use crate::model::behaviour::{ActionCatalogueStorage, SelectionStorage};
use crate::model::json::{Value, decimal_at};
use crate::model::primitives::Decimal;
use crate::model::run::{
    ActionCatalogue, ActionFacts, ActionNotInCatalogue, ActionSelected, AnyActionCatalogue,
    AnySelection, CatalogueEntry, CatalogueEntryStatus, CatalogueNotFound,
    CatalogueRevisionMismatch, SelectAction, SelectActionOutcome, Selection, SelectionData,
    SelectionExists, SelectionId, SelectionStrategy, action_catalogue_state, selection_state,
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
    /// selector that made its choice and, on a fallback, the fast pick, and a selector that knows
    /// what its pick spent reports those tokens.
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

    /// [`ActionSelector::resolve`] for a run of `case`: what Loom asks when it runs a commission,
    /// naming the case of the frontier it was handed. By default the case is not read; a selector
    /// whose answer depends on the case ([`DeterministicSelector`]) refuses a case it does not
    /// answer for, and [`HybridSelector`] hands the case on to both its selectors.
    fn resolve_for_case(
        &self,
        case: &CaseId,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Resolution, SelectorError> {
        let _ = case;
        self.resolve(context, candidates)
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

/// The facts of a case's actions, as the host answers them for the [`DeterministicSelector`]: the
/// case revision they were read at, and per action its effect class, the evidence kinds it may
/// produce and its open evidence (`loom.run.ActionFacts`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseFacts {
    /// The case revision the facts were read at: the one [`StepHistory::performed_at`] is asked
    /// about.
    pub case_revision: i64,
    /// The facts of each action the host knows of. An action listed twice is read at its first
    /// entry.
    pub actions: Vec<ActionFacts>,
}

/// The host port that answers a case's action facts for the [`DeterministicSelector`]: never a
/// model, and never the candidate list. The governor answers it from the protocol and the case's
/// current decision (`loom_governor::CanonGovernor::action_facts`).
pub trait ActionFactsPort {
    /// The facts of `case`'s actions at its current revision; `Err` with the reason when the host
    /// cannot answer.
    fn action_facts(&self, case: &CaseId) -> Result<CaseFacts, String>;
}

impl<P: ActionFactsPort + ?Sized> ActionFactsPort for &P {
    fn action_facts(&self, case: &CaseId) -> Result<CaseFacts, String> {
        (**self).action_facts(case)
    }
}

/// The host port that answers what the run already did, for the [`DeterministicSelector`]: the
/// history half of usefulness, which is the runtime's and not Canon's.
pub trait StepHistory {
    /// Whether `action` was performed at the case revision `case_revision`. An action that may
    /// produce no evidence is useful until this is true for the revision the facts were read at.
    fn performed_at(&self, action: &str, case_revision: i64) -> Result<bool, String>;

    /// Whether `action` is due again whatever its evidence and its history, as for a retry or a
    /// pending copy: while it is, an admissible `action` is useful.
    fn due(&self, action: &str) -> Result<bool, String>;
}

impl<H: StepHistory + ?Sized> StepHistory for &H {
    fn performed_at(&self, action: &str, case_revision: i64) -> Result<bool, String> {
        (**self).performed_at(action, case_revision)
    }

    fn due(&self, action: &str) -> Result<bool, String> {
        (**self).due(action)
    }
}

/// The effect class that sorts an action after every other: `write`.
const WRITE: &str = "write";

/// The selector whose strategy is `Deterministic`: it calls no model, and the same candidates,
/// facts and history give the same choice.
///
/// From the candidates it keeps only the `Admissible` ones whose action is useful, orders them
/// with every action whose effect class is not `write` (an undeclared one included) before every
/// `write` action and then by action id, and takes the first. An action is useful while
/// [`StepHistory::due`] is true; otherwise, when it may produce evidence, while its open evidence
/// is not empty, and when it may produce none, until [`StepHistory::performed_at`] is true for
/// the case revision the facts were read at. A candidate the facts do not name is not useful, and
/// an action the facts name that is not a candidate is never returned. With no useful candidate it
/// answers [`SelectorError::NothingAdmissible`]; a port that cannot answer makes it
/// [`SelectorError::Unavailable`] with the port's reason. Its pick has no confidence and reports no
/// tokens.
///
/// It answers for the one case it was built for. A run of a commission for another case
/// ([`ActionSelector::resolve_for_case`], which Loom calls with the case of the frontier it was
/// handed) is [`SelectorError::Unavailable`] naming both cases, and no port is asked.
/// [`ActionSelector::select`] and [`ActionSelector::resolve`] name no case and answer for the
/// selector's own.
pub struct DeterministicSelector<F, H> {
    case: CaseId,
    facts: F,
    history: H,
}

impl<F: ActionFactsPort, H: StepHistory> DeterministicSelector<F, H> {
    /// A selector for `case`, reading its action facts from `facts` and its step history from
    /// `history`.
    pub fn new(case: CaseId, facts: F, history: H) -> Self {
        Self {
            case,
            facts,
            history,
        }
    }

    /// Whether `action`, whose facts are `facts`, is useful at `case_revision`.
    fn useful(
        &self,
        action: &str,
        facts: &ActionFacts,
        case_revision: i64,
    ) -> Result<bool, SelectorError> {
        if self
            .history
            .due(action)
            .map_err(SelectorError::Unavailable)?
        {
            return Ok(true);
        }
        if facts.may_produce.is_empty() {
            return self
                .history
                .performed_at(action, case_revision)
                .map(|performed| !performed)
                .map_err(SelectorError::Unavailable);
        }
        Ok(!facts.open_evidence.is_empty())
    }
}

impl<F: ActionFactsPort, H: StepHistory> ActionSelector for DeterministicSelector<F, H> {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        let facts = self
            .facts
            .action_facts(&self.case)
            .map_err(SelectorError::Unavailable)?;
        // (whether it writes, action id): `false` orders before `true`, then ids by code point.
        let mut useful: Vec<(bool, &str)> = Vec::new();
        for entry in candidates
            .iter()
            .filter(|entry| entry.status == CatalogueEntryStatus::Admissible)
        {
            let Some(known) = facts
                .actions
                .iter()
                .find(|known| known.action == entry.action)
            else {
                continue;
            };
            if self.useful(&entry.action, known, facts.case_revision)? {
                useful.push((known.effect.as_deref() == Some(WRITE), &entry.action));
            }
        }
        useful
            .into_iter()
            .min()
            .map(|(_, action)| Choice {
                action: action.to_owned(),
                confidence: None,
            })
            .ok_or(SelectorError::NothingAdmissible)
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::Deterministic
    }

    /// [`ActionSelector::resolve`] when `case` is the case this selector was built for; for any
    /// other case [`SelectorError::Unavailable`] naming both, and no port is asked, so a run is
    /// never given a pick made from another case's facts.
    fn resolve_for_case(
        &self,
        case: &CaseId,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Resolution, SelectorError> {
        if *case != self.case {
            return Err(SelectorError::Unavailable(format!(
                "the deterministic selector answers for case `{}`, and the run is for case `{}`",
                self.case.0, case.0
            )));
        }
        self.resolve(context, candidates)
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
        self.fall_back(candidates, self.fast.resolve(context, candidates), || {
            self.stronger.resolve(context, candidates)
        })
    }

    /// [`ActionSelector::resolve`] as [`HybridSelector`] resolves, with `case` handed on to both
    /// selectors' own [`ActionSelector::resolve_for_case`].
    fn resolve_for_case(
        &self,
        case: &CaseId,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Resolution, SelectorError> {
        self.fall_back(
            candidates,
            self.fast.resolve_for_case(case, context, candidates),
            || self.stronger.resolve_for_case(case, context, candidates),
        )
    }
}

impl<F: ActionSelector, S: ActionSelector> HybridSelector<F, S> {
    /// `fast` when it is accepted from `candidates`; otherwise the resolution `stronger` gives,
    /// with the fast pick as the one it overruled when the fast selector answered at all.
    fn fall_back(
        &self,
        candidates: &[CatalogueEntry],
        fast: Result<Resolution, SelectorError>,
        stronger: impl FnOnce() -> Result<Resolution, SelectorError>,
    ) -> Result<Resolution, SelectorError> {
        if let Ok(resolution) = &fast
            && self.accepts(&resolution.chosen.choice, candidates)
        {
            return fast;
        }
        let stronger = stronger()?;
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
    ids: (SelectionId, SelectionId),
) -> Result<Resolved, SelectionRefusal> {
    let resolution = selector.resolve(context, &catalogue.data().entries);
    resolved(catalogue, resolution, ids)
}

/// [`resolve`] for a run of `case` ([`ActionSelector::resolve_for_case`]): what Loom asks when it
/// runs a commission, naming the case of the frontier it was handed. Refused as [`resolve`]
/// refuses, and when the selector does not answer for `case`.
pub fn resolve_for_case(
    selector: &impl ActionSelector,
    case: &CaseId,
    context: &SelectionContext,
    catalogue: &ActionCatalogue<action_catalogue_state::Projected>,
    ids: (SelectionId, SelectionId),
) -> Result<Resolved, SelectionRefusal> {
    let resolution = selector.resolve_for_case(case, context, &catalogue.data().entries);
    resolved(catalogue, resolution, ids)
}

/// The selections `resolution`, a selector's answer from `catalogue`, makes, as [`resolve`]
/// describes them.
fn resolved(
    catalogue: &ActionCatalogue<action_catalogue_state::Projected>,
    resolution: Result<Resolution, SelectorError>,
    (selection_id, overruled_id): (SelectionId, SelectionId),
) -> Result<Resolved, SelectionRefusal> {
    let resolution = resolution.map_err(SelectionRefusal::Selector)?;
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
/// The outcomes, in the order `ess/domains/run.yaml` declares them: `selection-exists` for a
/// selection id `selections` already holds, in any state, so a refused or admitted selection is
/// never selected again; `catalogue-unknown` for a catalogue `catalogues` does not hold;
/// `not-in-catalogue`, by the rule [`select`] applies to every selector, for an action the
/// catalogue does not list, whatever the confidence; `revision-mismatch` when the case revision
/// `input` claims is not the catalogue's; otherwise `selected`, and the selection is stored
/// `Selected`. Nothing is stored on a refusal.
pub fn select_action(
    catalogues: &impl ActionCatalogueStorage,
    selections: &mut impl SelectionStorage,
    input: SelectAction,
) -> SelectActionOutcome {
    if selections.get(&input.selection_id).is_some() {
        return SelectActionOutcome::SelectionExists {
            error: SelectionExists {
                selection_id: input.selection_id,
            },
        };
    }
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
