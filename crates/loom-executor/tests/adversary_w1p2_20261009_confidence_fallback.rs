//! Adversary pass 2 on `story:confidence-fallback`: the corrected [`Confidence`] driven against an
//! independent reference at the exact boundary, `-0` paths, and the hybrid selector seen through
//! `selection::select` when either or both selectors err.
//!
//! The reference here does not share the unit's algorithm: a rendering is a decimal when ESS's
//! generated `decimal_at` accepts it (the specification's own pattern), and its value is compared by
//! padding both fractions to a common length, not by trimming trailing zeros.
//!
//! Both selectors are scripted: no model or network call is made.

use std::cell::RefCell;
use std::cmp::Ordering;

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

fn candidates() -> Vec<CatalogueEntry> {
    vec![
        CatalogueEntry {
            action: STRONGER.to_owned(),
            status: CatalogueEntryStatus::Admissible,
        },
        CatalogueEntry {
            action: FAST.to_owned(),
            status: CatalogueEntryStatus::Admissible,
        },
        CatalogueEntry {
            action: MERGE.to_owned(),
            status: CatalogueEntryStatus::ApprovalRequired,
        },
    ]
}

fn catalogue() -> ActionCatalogue<action_catalogue_state::Projected> {
    ActionCatalogue::new(ActionCatalogueData {
        catalogue_id: CatalogueId(Uuid("00000000-0000-4000-8000-00000000b0c1".to_owned())),
        turn_id: TurnId(Uuid("00000000-0000-4000-8000-00000000b0a1".to_owned())),
        frontier: "frontier of CHG-1842".to_owned(),
        case_revision: 7,
        entries: candidates(),
    })
}

fn context() -> SelectionContext {
    SelectionContext {
        prompt: "the build on main is red; find out why".to_owned(),
    }
}

fn selection_id() -> SelectionId {
    SelectionId(Uuid("00000000-0000-4000-8000-00000000b0e1".to_owned()))
}

/// A scripted selector that records what it was handed.
struct Recording {
    answer: Result<Choice, SelectorError>,
    strategy: SelectionStrategy,
    handed: RefCell<Vec<(SelectionContext, Vec<CatalogueEntry>)>>,
}

impl Recording {
    fn answers(answer: Result<Choice, SelectorError>, strategy: SelectionStrategy) -> Self {
        Self {
            answer,
            strategy,
            handed: RefCell::new(Vec::new()),
        }
    }

    fn names(action: &str, confidence: Option<&str>, strategy: SelectionStrategy) -> Self {
        Self::answers(
            Ok(Choice {
                action: action.to_owned(),
                confidence: confidence.map(decimal),
            }),
            strategy,
        )
    }

    fn fast(confidence: Option<&str>) -> Self {
        Self::names(FAST, confidence, SelectionStrategy::FastTyped)
    }

    fn stronger(confidence: Option<&str>) -> Self {
        Self::names(STRONGER, confidence, SelectionStrategy::ReasoningModel)
    }

    fn asked(&self) -> usize {
        self.handed.borrow().len()
    }
}

impl ActionSelector for &Recording {
    fn select(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        self.handed
            .borrow_mut()
            .push((context.clone(), candidates.to_vec()));
        self.answer.clone()
    }

    fn strategy(&self) -> SelectionStrategy {
        self.strategy
    }
}

fn chosen(confidence: &str, threshold: &str) -> String {
    let fast = Recording::fast(Some(confidence));
    let stronger = Recording::stronger(None);
    HybridSelector::new(&fast, &stronger, &decimal(threshold))
        .expect("the threshold is a decimal in [0, 1]")
        .select(&context(), &candidates())
        .expect("the hybrid selector answers")
        .action
}

// ---- The reference: the specification's pattern, and the value by padded comparison. ----

fn is_spec_decimal(text: &str) -> bool {
    decimal_at(&Value::Text(text.to_owned()), "confidence", "a decimal").is_ok()
}

/// The value `text` writes as (negative, whole, fraction padded to `width` digits), when it is a
/// decimal in the specification's pattern.
fn reference(text: &str, width: usize) -> Option<(bool, u128, String)> {
    if !is_spec_decimal(text) {
        return None;
    }
    let (negative, unsigned) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let whole: u128 = whole.parse().ok()?;
    let mut padded = fraction.to_owned();
    while padded.len() < width {
        padded.push('0');
    }
    Some((negative, whole, padded))
}

/// Whether the value is zero.
fn is_zero(whole: u128, fraction: &str) -> bool {
    whole == 0 && fraction.bytes().all(|b| b == b'0')
}

/// Whether `text` is a decimal in [0, 1] by the reference.
fn reference_in_range(text: &str, width: usize) -> bool {
    match reference(text, width) {
        None => false,
        Some((negative, whole, fraction)) => {
            if is_zero(whole, &fraction) {
                true
            } else if negative {
                false
            } else {
                whole == 0 || (whole == 1 && fraction.bytes().all(|b| b == b'0'))
            }
        }
    }
}

/// The order of two in-range values by the reference.
fn reference_cmp(left: &str, right: &str, width: usize) -> Ordering {
    let (_, lw, lf) = reference(left, width).expect("in range");
    let (_, rw, rf) = reference(right, width).expect("in range");
    lw.cmp(&rw).then_with(|| lf.cmp(&rf))
}

/// A fixed-seed xorshift: the property runs the same inputs on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A rendering near the interesting edges: signs, leading zeros, trailing zeros, long fractions,
/// and the digits around 0, 0.9 and 1.
fn rendering(rng: &mut Rng) -> String {
    let mut text = String::new();
    if rng.below(5) == 0 {
        text.push('-');
    }
    let wholes = ["0", "1", "00", "01", "2", "10", "", "0", "0", "0", "1"];
    text.push_str(wholes[rng.below(wholes.len() as u64) as usize]);
    if rng.below(4) != 0 {
        text.push('.');
        let digits = rng.below(24);
        for _ in 0..digits {
            let pool = b"0000999995123";
            text.push(pool[rng.below(pool.len() as u64) as usize] as char);
        }
        for _ in 0..rng.below(4) {
            text.push('0');
        }
    }
    text
}

const WIDTH: usize = 64;

// ---- Properties. ----

#[test]
fn adversary_w1p2_parse_is_some_exactly_for_spec_decimals_in_zero_to_one() {
    let mut rng = Rng(0x5eed_0a7e_2026_1009);
    let mut wrong = Vec::new();
    let (mut accepted, mut refused_decimal, mut not_decimal) = (0, 0, 0);
    for _ in 0..20_000 {
        let text = rendering(&mut rng);
        let parsed = Confidence::parse(&decimal(&text)).is_some();
        if parsed != reference_in_range(&text, WIDTH) {
            wrong.push(text.clone());
        }
        match (parsed, is_spec_decimal(&text)) {
            (true, _) => accepted += 1,
            (false, true) => refused_decimal += 1,
            (false, false) => not_decimal += 1,
        }
    }
    // The generator reaches all three classes, so the property is not vacuous.
    assert!(
        accepted > 1_000 && refused_decimal > 1_000 && not_decimal > 1_000,
        "{accepted} accepted, {refused_decimal} out-of-range decimals, {not_decimal} not decimals"
    );
    wrong.sort();
    wrong.dedup();
    assert!(
        wrong.is_empty(),
        "parse disagrees with the reference: {wrong:?}"
    );
}

#[test]
fn adversary_w1p2_confidence_orders_as_the_reference_and_the_hybrid_follows_it() {
    let mut rng = Rng(0x0dd_ba11_2026_1009);
    let mut in_range = Vec::new();
    while in_range.len() < 300 {
        let text = rendering(&mut rng);
        if reference_in_range(&text, WIDTH) {
            in_range.push(text);
        }
    }
    for left in &in_range {
        let lc = Confidence::parse(&decimal(left)).expect("in range");
        for right in &in_range {
            let rc = Confidence::parse(&decimal(right)).expect("in range");
            assert_eq!(
                lc.cmp(&rc),
                reference_cmp(left, right, WIDTH),
                "{left:?} against {right:?}"
            );
        }
    }
    // The hybrid's decision is the reference's: fast exactly when confidence >= threshold.
    let (mut fast_count, mut stronger_count, mut equal_count) = (0, 0, 0);
    for pair in in_range.chunks(2).take(150) {
        let (confidence, threshold) = (&pair[0], &pair[1]);
        let order = reference_cmp(confidence, threshold, WIDTH);
        if order == Ordering::Equal {
            equal_count += 1;
        }
        let expected = if order == Ordering::Less {
            stronger_count += 1;
            STRONGER
        } else {
            fast_count += 1;
            FAST
        };
        assert_eq!(
            chosen(confidence, threshold),
            expected,
            "confidence {confidence:?} under threshold {threshold:?}"
        );
    }
    assert!(
        fast_count > 20 && stronger_count > 20 && equal_count > 0,
        "{fast_count} fast, {stronger_count} stronger, {equal_count} equal"
    );
}

// ---- The exact boundary with long fractions. ----

#[test]
fn adversary_w1p2_exact_boundary_with_long_fractions() {
    let long = 5_000;
    let threshold = format!("0.{}", "7".repeat(long));
    // Equal, then one more trailing zero, then a different sign of zero padding: all at.
    assert_eq!(chosen(&threshold, &threshold), FAST);
    assert_eq!(chosen(&format!("{threshold}0000"), &threshold), FAST);
    assert_eq!(chosen(&threshold, &format!("{threshold}000")), FAST);
    // One unit in the last place below and above.
    let below = format!("0.{}6", "7".repeat(long - 1));
    let above = format!("0.{}8", "7".repeat(long - 1));
    assert_eq!(chosen(&below, &threshold), STRONGER);
    assert_eq!(chosen(&above, &threshold), FAST);
    // A longer value that is a prefix-extension: above, however small the tail.
    let tail = format!("{threshold}{}1", "0".repeat(long));
    assert_eq!(chosen(&tail, &threshold), FAST);
    assert_eq!(chosen(&threshold, &tail), STRONGER);
    // The threshold 1 written with a long run of zeros.
    let one = format!("1.{}", "0".repeat(long));
    assert_eq!(chosen("1", &one), FAST);
    assert_eq!(chosen(&format!("0.{}", "9".repeat(long)), &one), STRONGER);
    assert!(
        Confidence::parse(&decimal(&format!("1.{}1", "0".repeat(long)))).is_none(),
        "just above one is out of range"
    );
}

// ---- `-0` paths. ----

#[test]
fn adversary_w1p2_negative_zero_threshold_and_confidence() {
    for zero in ["-0", "-0.0", "-0.000000", "0", "0.0"] {
        // As a threshold, every spelling of zero admits every in-range confidence.
        assert_eq!(chosen("0", zero), FAST, "threshold {zero:?}");
        assert_eq!(chosen("-0", zero), FAST, "threshold {zero:?}");
        assert_eq!(chosen("0.0001", zero), FAST, "threshold {zero:?}");
        // And it never equals a value above zero.
        assert_eq!(chosen(zero, "0.0001"), STRONGER, "confidence {zero:?}");
    }
    for negative in ["-0.0001", "-0.1", "-1", "-1.0", "-00", "-01", "-0.", "-.0"] {
        assert!(
            Confidence::parse(&decimal(negative)).is_none(),
            "{negative:?} is not in range"
        );
        assert_eq!(chosen(negative, "0"), STRONGER, "{negative:?}");
    }
}

#[test]
fn adversary_w1p2_negative_zero_is_recorded_as_written_and_out_of_range_never() {
    let fast = Recording::fast(Some("-0.0"));
    let stronger = Recording::stronger(None);
    let selector = HybridSelector::new(&fast, &stronger, &decimal("-0")).expect("valid");
    let selection = select(&selector, &context(), &catalogue(), selection_id())
        .expect("a candidate is selected")
        .into_data();
    assert_eq!(selection.action, FAST);
    assert_eq!(selection.confidence, Some(decimal("-0.0")));
    assert_eq!(selection.strategy, SelectionStrategy::Hybrid);
    assert_eq!(stronger.asked(), 0);
}

// ---- The hybrid seen through `select`. ----

#[test]
fn adversary_w1p2_both_selectors_answer_the_same_context_and_candidates() {
    let fast = Recording::fast(Some("0.1"));
    let stronger = Recording::stronger(None);
    let selector = HybridSelector::new(&fast, &stronger, &decimal("0.9")).expect("valid");
    select(&selector, &context(), &catalogue(), selection_id()).expect("selected");
    let expected = vec![(context(), candidates())];
    assert_eq!(*fast.handed.borrow(), expected, "the fast selector's view");
    assert_eq!(
        *stronger.handed.borrow(),
        expected,
        "the stronger selector is handed the same context and candidates, not fewer"
    );
}

#[test]
fn adversary_w1p2_both_selectors_erring_refuses_with_the_strongers_error() {
    for fast_error in [
        SelectorError::Unavailable("fast down".to_owned()),
        SelectorError::NothingAdmissible,
    ] {
        for stronger_error in [
            SelectorError::Unavailable("stronger down".to_owned()),
            SelectorError::NothingAdmissible,
        ] {
            let fast = Recording::answers(Err(fast_error.clone()), SelectionStrategy::FastTyped);
            let stronger = Recording::answers(
                Err(stronger_error.clone()),
                SelectionStrategy::ReasoningModel,
            );
            let selector = HybridSelector::new(&fast, &stronger, &decimal("0.6")).expect("valid");
            assert_eq!(
                selector.select(&context(), &candidates()),
                Err(stronger_error.clone())
            );
            match select(&selector, &context(), &catalogue(), selection_id()) {
                Err(SelectionRefusal::Selector(error)) => assert_eq!(
                    error, stronger_error,
                    "fast {fast_error:?}, stronger {stronger_error:?}"
                ),
                Err(other) => panic!("expected the stronger selector's error, got {other:?}"),
                Ok(selection) => panic!("expected a refusal, got {:?}", selection.into_data()),
            }
        }
    }
}

#[test]
fn adversary_w1p2_a_stronger_error_after_a_confident_outside_fast_choice_is_the_refusal() {
    let fast = Recording::names(OUTSIDE, Some("1"), SelectionStrategy::FastTyped);
    let stronger = Recording::answers(
        Err(SelectorError::Unavailable("stronger down".to_owned())),
        SelectionStrategy::ReasoningModel,
    );
    let selector = HybridSelector::new(&fast, &stronger, &decimal("0")).expect("valid");
    match select(&selector, &context(), &catalogue(), selection_id()) {
        Err(SelectionRefusal::Selector(SelectorError::Unavailable(reason))) => {
            assert_eq!(reason, "stronger down");
        }
        Err(other) => panic!("expected the stronger selector's error, got {other:?}"),
        Ok(selection) => panic!(
            "the outside fast choice is never returned; got {:?}",
            selection.into_data()
        ),
    }
}

#[test]
fn adversary_w1p2_a_fallback_records_the_strongers_confidence_never_the_fast_one() {
    for (stronger_confidence, recorded) in [
        (Some("0.3"), Some("0.3")),
        (Some("0.30"), Some("0.30")),
        (Some("1.5"), None),
        (Some("01"), None),
        (None, None),
    ] {
        let fast = Recording::fast(Some("0.5"));
        let stronger = Recording::stronger(stronger_confidence);
        let selector = HybridSelector::new(&fast, &stronger, &decimal("0.9")).expect("valid");
        let selection = select(&selector, &context(), &catalogue(), selection_id())
            .expect("selected")
            .into_data();
        assert_eq!(selection.action, STRONGER);
        assert_eq!(
            selection.confidence,
            recorded.map(decimal),
            "stronger confidence {stronger_confidence:?}"
        );
        assert_eq!(selection.strategy, SelectionStrategy::Hybrid);
        assert_eq!(selection.case_revision, 7);
    }
}

#[test]
fn adversary_w1p2_a_hybrid_nested_as_the_stronger_selector_applies_both_thresholds() {
    // fast (0.7) -> inner hybrid of middle (0.5) and last; outer threshold 0.9, inner 0.4.
    let fast = Recording::names(FAST, Some("0.7"), SelectionStrategy::FastTyped);
    let middle = Recording::names(MERGE, Some("0.5"), SelectionStrategy::FastTyped);
    let last = Recording::stronger(None);
    let inner = HybridSelector::new(&middle, &last, &decimal("0.4")).expect("valid");
    let outer = HybridSelector::new(&fast, inner, &decimal("0.9")).expect("valid");
    let selection = select(&outer, &context(), &catalogue(), selection_id())
        .expect("selected")
        .into_data();
    assert_eq!(selection.action, MERGE);
    assert_eq!(selection.confidence, Some(decimal("0.5")));
    assert_eq!(selection.strategy, SelectionStrategy::Hybrid);
    assert_eq!((fast.asked(), middle.asked(), last.asked()), (1, 1, 0));
}
