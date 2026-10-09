//! Acceptance for `story:confidence-fallback`: with scripted fast and stronger selectors, run once
//! for each of two different supplied thresholds, the hybrid selector returns the fast choice only
//! when its confidence is at or above that threshold, and the stronger choice when the fast
//! confidence is below it, missing, or the fast selector errs or names an action outside the
//! candidates (Atlas ADR 0073 § Fallback; `docs/integrations/laya-fast-selection.md` § Confidence
//! policy).
//!
//! A confidence outside [0, 1], or one that is not a decimal, counts as missing; a confidence equal
//! to the threshold but written differently (`0.9` and `0.90`) counts as at the threshold. Both
//! selectors are scripted: no model or network call is made.

use std::cell::Cell;

use b10x_loom_executor::model::primitives::{Decimal, Uuid};
use b10x_loom_executor::model::run::{
    ActionCatalogue, ActionCatalogueData, CatalogueEntry, CatalogueEntryStatus, CatalogueId,
    SelectionId, SelectionStrategy, TurnId, action_catalogue_state,
};
use b10x_loom_executor::selection::{Choice, SelectionContext, select};
use b10x_loom_executor::{ActionSelector, HybridSelector, InvalidThreshold, SelectorError};

const INSPECT: &str = "repository.inspect";

/// What the scripted fast selector names when it names a candidate.
const FAST: &str = "tests.run";

/// What the scripted stronger selector names: a different candidate, so the two answers differ.
const STRONGER: &str = "repository.inspect";

const MERGE: &str = "repository.merge";

/// Listed nowhere in the candidate set.
const OUTSIDE: &str = "release.publish";

/// The two thresholds the acceptance runs under, as a host would supply them per protocol.
const THRESHOLDS: [&str; 2] = ["0.90", "0.6"];

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
        entry(INSPECT, CatalogueEntryStatus::Admissible),
        entry(FAST, CatalogueEntryStatus::Admissible),
        entry(MERGE, CatalogueEntryStatus::ApprovalRequired),
    ]
}

fn catalogue() -> ActionCatalogue<action_catalogue_state::Projected> {
    ActionCatalogue::new(ActionCatalogueData {
        catalogue_id: CatalogueId(Uuid("00000000-0000-4000-8000-0000000009c1".to_owned())),
        turn_id: TurnId(Uuid("00000000-0000-4000-8000-0000000009a1".to_owned())),
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
    SelectionId(Uuid("00000000-0000-4000-8000-0000000009e1".to_owned()))
}

/// A selector that gives one scripted answer and counts how often it was asked.
struct Scripted {
    answer: Result<Choice, SelectorError>,
    strategy: SelectionStrategy,
    asked: Cell<usize>,
}

impl Scripted {
    fn fast(answer: Result<Choice, SelectorError>) -> Self {
        Self {
            answer,
            strategy: SelectionStrategy::FastTyped,
            asked: Cell::new(0),
        }
    }

    fn names(action: &str, confidence: Option<&str>) -> Self {
        Self::fast(Ok(Choice {
            action: action.to_owned(),
            confidence: confidence.map(decimal),
        }))
    }

    fn stronger() -> Self {
        Self {
            answer: Ok(Choice {
                action: STRONGER.to_owned(),
                confidence: None,
            }),
            strategy: SelectionStrategy::ReasoningModel,
            asked: Cell::new(0),
        }
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

/// What the hybrid selector answered for `fast` under `threshold`, and whether it asked the
/// stronger selector.
fn hybrid(fast: &Scripted, threshold: &str) -> (Result<Choice, SelectorError>, bool) {
    let stronger = Scripted::stronger();
    let selector = HybridSelector::new(fast, &stronger, &decimal(threshold))
        .expect("the threshold is a decimal in [0, 1]");
    assert_eq!(selector.strategy(), SelectionStrategy::Hybrid);
    let answer = selector.select(&context(), &candidates());
    assert_eq!(fast.asked.get(), 1, "the fast selector is asked once");
    (answer, stronger.asked.get() > 0)
}

/// The action the hybrid selector chose for `fast` under `threshold`.
fn chosen(fast: &Scripted, threshold: &str) -> String {
    let (answer, _) = hybrid(fast, threshold);
    answer.expect("the hybrid selector answers").action
}

#[test]
fn the_fast_choice_is_returned_only_at_or_above_the_supplied_threshold() {
    // (confidence, chosen under "0.90", chosen under "0.6")
    let table: [(&str, &str, &str); 8] = [
        ("0.95", FAST, FAST),
        ("1", FAST, FAST),
        ("1.000", FAST, FAST),
        // At the threshold, written differently from it.
        ("0.9", FAST, FAST),
        ("0.900", FAST, FAST),
        ("0.60", STRONGER, FAST),
        // Below the first threshold, above the second.
        ("0.75", STRONGER, FAST),
        // Below both.
        ("0.059", STRONGER, STRONGER),
    ];
    for (confidence, under_first, under_second) in table {
        for (threshold, expected) in THRESHOLDS.iter().zip([under_first, under_second]) {
            let fast = Scripted::names(FAST, Some(confidence));
            let (answer, asked_stronger) = hybrid(&fast, threshold);
            let choice = answer.expect("the hybrid selector answers");
            assert_eq!(
                choice.action, expected,
                "confidence {confidence} under threshold {threshold}"
            );
            assert_eq!(
                asked_stronger,
                expected == STRONGER,
                "the stronger selector is asked only on a fallback: confidence {confidence} \
                 under threshold {threshold}"
            );
            if expected == FAST {
                assert_eq!(choice.confidence, Some(decimal(confidence)));
            }
        }
    }
}

#[test]
fn equal_values_written_differently_are_at_the_threshold() {
    // String order puts "0.9" below "0.90" and "00.95" below "0.9"; numbers do not.
    for (confidence, threshold) in [
        ("0.9", "0.90"),
        ("0.90", "0.9"),
        ("0.6", "0.600"),
        ("1", "1.0"),
        ("0", "0.00"),
        ("00.95", "0.9"),
    ] {
        let fast = Scripted::names(FAST, Some(confidence));
        assert_eq!(
            chosen(&fast, threshold),
            FAST,
            "confidence {confidence} is at threshold {threshold}"
        );
    }
    // Just below, in more digits than the threshold has.
    let fast = Scripted::names(FAST, Some("0.8999999999999999999999"));
    assert_eq!(chosen(&fast, "0.9"), STRONGER);
}

#[test]
fn a_missing_confidence_falls_back_under_every_threshold() {
    for threshold in THRESHOLDS.iter().chain(&["0"]) {
        let fast = Scripted::names(FAST, None);
        assert_eq!(chosen(&fast, threshold), STRONGER, "threshold {threshold}");
    }
}

#[test]
fn a_confidence_outside_zero_to_one_or_not_a_decimal_counts_as_missing() {
    for confidence in [
        "1.5", "1.0001", "2", "-0.1", "-1", "abc", "", ".", "0.9.1", "NaN", "inf", "9e-1", "0,95",
        " 0.95", "0.95 ", "+0.95", "0x1",
    ] {
        // A threshold of 0 admits every confidence there is, so only a missing one falls back.
        for threshold in THRESHOLDS.iter().chain(&["0"]) {
            let fast = Scripted::names(FAST, Some(confidence));
            assert_eq!(
                chosen(&fast, threshold),
                STRONGER,
                "confidence {confidence:?} under threshold {threshold}"
            );
        }
    }
}

#[test]
fn a_fast_error_falls_back_under_every_threshold() {
    for error in [
        SelectorError::Unavailable("the fast model timed out".to_owned()),
        SelectorError::NothingAdmissible,
    ] {
        for threshold in THRESHOLDS {
            let fast = Scripted::fast(Err(error.clone()));
            assert_eq!(
                chosen(&fast, threshold),
                STRONGER,
                "{error:?} under {threshold}"
            );
        }
    }
}

#[test]
fn a_fast_action_outside_the_candidates_falls_back_and_is_never_returned() {
    for threshold in THRESHOLDS.iter().chain(&["0"]) {
        let fast = Scripted::names(OUTSIDE, Some("1"));
        let (answer, asked_stronger) = hybrid(&fast, threshold);
        let choice = answer.expect("the stronger selector answers");
        assert_eq!(choice.action, STRONGER, "threshold {threshold}");
        assert!(asked_stronger);
    }
}

#[test]
fn the_stronger_selectors_error_is_the_hybrid_answer_on_a_fallback() {
    let fast = Scripted::names(FAST, Some("0.1"));
    let stronger = Scripted::fast(Err(SelectorError::Unavailable("down".to_owned())));
    let selector = HybridSelector::new(&fast, &stronger, &decimal("0.9")).expect("valid");
    assert_eq!(
        selector.select(&context(), &candidates()),
        Err(SelectorError::Unavailable("down".to_owned()))
    );
}

#[test]
fn a_threshold_that_is_not_a_decimal_in_zero_to_one_is_refused() {
    let fast = Scripted::names(FAST, Some("1"));
    let stronger = Scripted::stronger();
    for threshold in ["1.5", "-0.1", "abc", "", "NaN", "9e-1"] {
        assert_eq!(
            HybridSelector::new(&fast, &stronger, &decimal(threshold)).err(),
            Some(InvalidThreshold(decimal(threshold))),
            "threshold {threshold:?}"
        );
    }
    for threshold in ["0", "1", "0.5", "1.00"] {
        assert!(
            HybridSelector::new(&fast, &stronger, &decimal(threshold)).is_ok(),
            "threshold {threshold}"
        );
    }
}

#[test]
fn a_confident_hybrid_choice_still_passes_loom_membership_and_records_hybrid() {
    let fast = Scripted::names(FAST, Some("0.97"));
    let stronger = Scripted::stronger();
    let selector = HybridSelector::new(&fast, &stronger, &decimal("0.9")).expect("valid");
    let selection = select(&selector, &context(), &catalogue(), selection_id())
        .expect("a candidate is selected")
        .into_data();
    assert_eq!(selection.action, FAST);
    assert_eq!(selection.confidence, Some(decimal("0.97")));
    assert_eq!(selection.strategy, SelectionStrategy::Hybrid);
}

#[test]
fn a_selectors_confidence_is_validated_at_the_seam() {
    // A selector's confidence outside [0, 1], or not a decimal, is not recorded on the selection.
    for confidence in ["1.5", "-0.2", "abc", "9e-1"] {
        let fast = Scripted::names(FAST, Some(confidence));
        let selection = select(&&fast, &context(), &catalogue(), selection_id())
            .expect("a candidate is selected")
            .into_data();
        assert_eq!(selection.action, FAST);
        assert_eq!(selection.confidence, None, "confidence {confidence:?}");
    }
    let fast = Scripted::names(FAST, Some("0.42"));
    let selection = select(&&fast, &context(), &catalogue(), selection_id())
        .expect("a candidate is selected")
        .into_data();
    assert_eq!(selection.confidence, Some(decimal("0.42")));
}
