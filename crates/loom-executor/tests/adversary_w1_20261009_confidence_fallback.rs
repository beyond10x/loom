//! Adversary pass 1 on `story:confidence-fallback`: the hybrid selector and [`Confidence`] driven
//! against the specification's own published decimal pattern and the story's acceptance.
//!
//! The acceptance says a confidence "that is not a decimal counts as missing". What a decimal is
//! comes from ESS, not from this unit: the wire reader ESS generates for this repository's model
//! (`generated/rust/loom/src/json.rs`, `decimal_at`) holds every `Decimal` to "the published
//! pattern: an optional `-`, digits without a leading zero, then an optional `.` and digits". So
//! `00.95`, `01` and `00` are not decimals, and a confidence written that way is missing: the fast
//! choice falls back, and the seam does not record it.
//!
//! Both selectors are scripted: no model or network call is made.

use std::cell::Cell;

use b10x_loom_executor::model::json::{Value, decimal_at};
use b10x_loom_executor::model::primitives::{Decimal, Uuid};
use b10x_loom_executor::model::run::{
    ActionCatalogue, ActionCatalogueData, CatalogueEntry, CatalogueEntryStatus, CatalogueId,
    SelectionId, SelectionStrategy, TurnId, action_catalogue_state,
};
use b10x_loom_executor::selection::{Choice, SelectionContext, SelectionRefusal, select};
use b10x_loom_executor::{ActionSelector, Confidence, HybridSelector, SelectorError};

const FAST: &str = "tests.run";
const STRONGER: &str = "repository.inspect";
const MERGE: &str = "repository.merge";
const OUTSIDE: &str = "release.publish";

fn decimal(text: &str) -> Decimal {
    Decimal(text.to_owned())
}

fn entry(action: &str, status: CatalogueEntryStatus) -> CatalogueEntry {
    CatalogueEntry {
        action: action.to_owned(),
        status,
    }
}

fn candidates() -> Vec<CatalogueEntry> {
    vec![
        entry(STRONGER, CatalogueEntryStatus::Admissible),
        entry(FAST, CatalogueEntryStatus::Admissible),
        entry(MERGE, CatalogueEntryStatus::ApprovalRequired),
    ]
}

fn catalogue() -> ActionCatalogue<action_catalogue_state::Projected> {
    ActionCatalogue::new(ActionCatalogueData {
        catalogue_id: CatalogueId(Uuid("00000000-0000-4000-8000-00000000a0c1".to_owned())),
        turn_id: TurnId(Uuid("00000000-0000-4000-8000-00000000a0a1".to_owned())),
        frontier: "frontier of CHG-1842".to_owned(),
        case_revision: 3,
        entries: candidates(),
    })
}

fn context() -> SelectionContext {
    SelectionContext {
        prompt: "the build on main is red; find out why".to_owned(),
    }
}

fn selection_id() -> SelectionId {
    SelectionId(Uuid("00000000-0000-4000-8000-00000000a0e1".to_owned()))
}

struct Scripted {
    answer: Result<Choice, SelectorError>,
    strategy: SelectionStrategy,
    asked: Cell<usize>,
}

impl Scripted {
    fn names(action: &str, confidence: Option<&str>, strategy: SelectionStrategy) -> Self {
        Self {
            answer: Ok(Choice {
                action: action.to_owned(),
                confidence: confidence.map(decimal),
            }),
            strategy,
            asked: Cell::new(0),
        }
    }

    fn fast(confidence: &str) -> Self {
        Self::names(FAST, Some(confidence), SelectionStrategy::FastTyped)
    }

    fn stronger() -> Self {
        Self::names(STRONGER, None, SelectionStrategy::ReasoningModel)
    }
}

impl ActionSelector for &Scripted {
    fn select(
        &self,
        _context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        self.asked.set(self.asked.get() + 1);
        self.answer.clone()
    }

    fn strategy(&self) -> SelectionStrategy {
        self.strategy
    }
}

fn chosen(confidence: &str, threshold: &str) -> String {
    let fast = Scripted::fast(confidence);
    let stronger = Scripted::stronger();
    let selector = HybridSelector::new(&fast, &stronger, &decimal(threshold))
        .expect("the threshold is a decimal in [0, 1]");
    selector
        .select(&context(), &candidates())
        .expect("the hybrid selector answers")
        .action
}

/// Whether ESS's published pattern reads `text` as a `Decimal`.
fn is_spec_decimal(text: &str) -> bool {
    decimal_at(&Value::Text(text.to_owned()), "confidence", "a decimal").is_ok()
}

// ---- Red: the unit's notion of a decimal is wider than the specification's. ----

#[test]
fn adversary_w1_every_confidence_is_a_decimal_in_the_published_pattern() {
    let mut wider = Vec::new();
    for text in [
        "0", "-0", "0.5", "0.95", "1", "1.0", "1.000", "00", "01", "00.95", "000.5", "01.0",
    ] {
        if Confidence::parse(&decimal(text)).is_some() && !is_spec_decimal(text) {
            wider.push(text);
        }
    }
    assert!(
        wider.is_empty(),
        "Confidence::parse accepts renderings ESS's published Decimal pattern refuses: {wider:?}"
    );
}

#[test]
fn adversary_w1_a_leading_zero_confidence_is_not_a_decimal_and_falls_back() {
    for (confidence, threshold) in [("00.95", "0.9"), ("01", "0.9"), ("00", "0")] {
        assert_eq!(
            chosen(confidence, threshold),
            STRONGER,
            "confidence {confidence:?} is not a decimal in the published pattern, so it counts as \
             missing under threshold {threshold}"
        );
    }
}

#[test]
fn adversary_w1_a_leading_zero_threshold_is_refused() {
    let fast = Scripted::fast("1");
    let stronger = Scripted::stronger();
    for threshold in ["00.5", "01", "00"] {
        assert!(
            HybridSelector::new(&fast, &stronger, &decimal(threshold)).is_err(),
            "threshold {threshold:?} is not a decimal in the published pattern"
        );
    }
}

#[test]
fn adversary_w1_the_seam_does_not_record_a_leading_zero_confidence() {
    for confidence in ["00.95", "01"] {
        let fast = Scripted::fast(confidence);
        let selection = select(&&fast, &context(), &catalogue(), selection_id())
            .expect("a candidate is selected")
            .into_data();
        assert_eq!(
            selection.confidence, None,
            "confidence {confidence:?} is not a decimal in the published pattern"
        );
    }
}

// ---- Probes that hold today; they close gaps the unit's suite leaves open. ----

/// No case of the unit's has a trailing `.`: dropping the `contains('.') && fraction.is_empty()`
/// clause of `Confidence::parse` would keep its suite green.
#[test]
fn adversary_w1_a_trailing_point_is_not_a_decimal() {
    for confidence in ["1.", "0.", "-0."] {
        assert!(!is_spec_decimal(confidence), "{confidence:?}");
        assert_eq!(chosen(confidence, "0"), STRONGER, "{confidence:?}");
    }
    assert_eq!(chosen(".9", "0"), STRONGER);
}

/// The unit documents `-0` as in range and the published pattern agrees; no case of the unit's
/// says so, so refusing every negative would keep its suite green.
#[test]
fn adversary_w1_negative_zero_is_zero() {
    for confidence in ["-0", "-0.0", "-0.000"] {
        assert!(is_spec_decimal(confidence));
        assert_eq!(chosen(confidence, "0"), FAST, "{confidence:?}");
        assert_eq!(chosen(confidence, "0.01"), STRONGER, "{confidence:?}");
    }
    assert!(
        HybridSelector::new(&Scripted::fast("0"), &Scripted::stronger(), &decimal("-0")).is_ok()
    );
}

#[test]
fn adversary_w1_thresholds_of_zero_and_one_are_the_boundaries() {
    assert_eq!(chosen("0", "0"), FAST);
    assert_eq!(chosen("1", "1"), FAST);
    assert_eq!(chosen("1.0", "1.000"), FAST);
    let just_below_one = format!("0.{}", "9".repeat(10_000));
    assert_eq!(chosen(&just_below_one, "1"), STRONGER);
    assert_eq!(chosen(&just_below_one, "0.9"), FAST);
    let just_above_zero = format!("0.{}1", "0".repeat(10_000));
    assert_eq!(chosen(&just_above_zero, "0"), FAST);
    assert_eq!(chosen("0", &just_above_zero), STRONGER);
}

#[test]
fn adversary_w1_signs_exponents_whitespace_and_non_ascii_digits_are_missing() {
    for confidence in [
        "+0.9", "+1", "0.9e0", "1e0", "1E0", "\t0.9", "0.9\n", "0 .9", "--0", "-", "٠.٩", "０.９",
        "0.９", "1_0", "0.9\u{0}",
    ] {
        assert_eq!(chosen(confidence, "0"), STRONGER, "{confidence:?}");
    }
}

#[test]
fn adversary_w1_an_outside_action_at_full_confidence_is_never_the_selection() {
    // The fast selector names an action outside the candidates at confidence 1, and the stronger
    // one also names one: `select` still refuses, whichever selector the hybrid took it from.
    let fast = Scripted::names(OUTSIDE, Some("1"), SelectionStrategy::FastTyped);
    let stronger = Scripted::names("repository.delete", None, SelectionStrategy::ReasoningModel);
    let selector = HybridSelector::new(&fast, &stronger, &decimal("0")).expect("valid");
    let Err(refusal) = select(&selector, &context(), &catalogue(), selection_id()) else {
        panic!("an action outside the catalogue is never a selection");
    };
    match refusal {
        SelectionRefusal::NotInCatalogue(error) => assert_eq!(error.action, "repository.delete"),
        other => panic!("expected not-in-catalogue, got {other:?}"),
    }
    assert_eq!(stronger.asked.get(), 1);
}

#[test]
fn adversary_w1_a_fallback_after_a_confident_outside_choice_is_the_strongers() {
    let fast = Scripted::names(OUTSIDE, Some("1"), SelectionStrategy::FastTyped);
    let stronger = Scripted::stronger();
    let selector = HybridSelector::new(&fast, &stronger, &decimal("0.9")).expect("valid");
    let selection = select(&selector, &context(), &catalogue(), selection_id())
        .expect("the stronger selector's candidate is selected")
        .into_data();
    assert_eq!(selection.action, STRONGER);
    assert_eq!(selection.confidence, None);
    assert_eq!(selection.strategy, SelectionStrategy::Hybrid);
}
