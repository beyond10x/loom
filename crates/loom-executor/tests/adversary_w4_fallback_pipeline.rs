//! Adversary pass 1, wave 2026-10-10-w1, `story:fallback-selection-recording`: the fallback
//! through `Loom::run` and `selection::resolve`, at the edges the unit's own test leaves out.
//!
//! A stronger selector that errs or leaves the catalogue; a fast confidence outside [0, 1]; the
//! fast and stronger selectors naming the same action; nested hybrids; the chosen selection's id
//! with and without a fallback; and the strategy `Loom::select` returns for a hybrid.

use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, FrontierAction, PrincipalId, commission_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_executor::model::primitives::{Decimal, Uuid};
use b10x_loom_executor::model::run::{
    CatalogueEntry, CatalogueId, SelectionId, SelectionSnapshot, SelectionState, SelectionStrategy,
    TurnId,
};
use b10x_loom_executor::projection::project;
use b10x_loom_executor::selection::{self, Choice, SelectionContext};
use b10x_loom_executor::{
    ActionSelector, ArgumentContext, ArgumentGenerator, HybridSelector, Loom, SelectorError,
};

const CASE: &str = "CASE-ADV-W4-FALLBACK";
const REVISION: i64 = 3;
const PROMPT: &str = "why is checkout slow";
const FAST: &str = "metrics.inspect";
const STRONGER: &str = "logs.search";
const THIRD: &str = "release.inspect";
const FRONTIER: [&str; 3] = [FAST, STRONGER, THIRD];
const OUTSIDE: &str = "release.rollback";
const THRESHOLD: &str = "0.9";

struct Scripted {
    answer: Result<Choice, SelectorError>,
    strategy: SelectionStrategy,
    asked: Mutex<usize>,
}

impl Scripted {
    fn new(strategy: SelectionStrategy, action: &str, confidence: Option<&str>) -> Self {
        Self {
            answer: Ok(Choice {
                action: action.to_owned(),
                confidence: confidence.map(|text| Decimal(text.to_owned())),
            }),
            strategy,
            asked: Mutex::new(0),
        }
    }

    fn failing(strategy: SelectionStrategy, error: SelectorError) -> Self {
        Self {
            answer: Err(error),
            strategy,
            asked: Mutex::new(0),
        }
    }

    fn asked(&self) -> usize {
        *self.asked.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl ActionSelector for &Scripted {
    fn select(
        &self,
        _context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        *self.asked.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        self.answer.clone()
    }

    fn strategy(&self) -> SelectionStrategy {
        self.strategy
    }
}

#[derive(Default)]
struct Handed(Mutex<Vec<String>>);

impl ArgumentGenerator for &Handed {
    fn generate(
        &self,
        _context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<CommissionValue, String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(entry.action.clone());
        Ok(CommissionValue::Object(Vec::new()))
    }
}

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(CommissionUuid(
            "00000000-0000-4000-8000-00000000a401".to_owned(),
        )),
        agent_revision_id: AgentRevisionId(CommissionUuid(
            "00000000-0000-4000-8000-00000000a402".to_owned(),
        )),
        case_id: case(),
        principal: PrincipalId("principal-adv".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

fn governor(runs: usize) -> FakeGovernor {
    let governor = FakeGovernor::new();
    let actions: Vec<FrontierAction> = FRONTIER
        .iter()
        .map(|action| FrontierAction {
            action: (*action).to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        })
        .collect();
    governor.script(
        case(),
        (0..runs * 2 + 2)
            .map(|_| Answer::at(REVISION).with_items(Vec::new(), Vec::new(), actions.clone())),
    );
    governor
}

fn threshold() -> Decimal {
    Decimal(THRESHOLD.to_owned())
}

struct Observed {
    outcome: ExecutorOutcome,
    selections: Vec<SelectionSnapshot>,
    handed: Vec<String>,
    revalidated: usize,
}

fn run_with(selector: impl ActionSelector, instance: Option<&str>) -> Observed {
    let handed = Handed::default();
    let governor = governor(1);
    let mut loom = Loom::new(selector, &handed, PROMPT);
    if let Some(instance) = instance {
        loom = loom.with_instance(instance);
    }
    let loom = loom.with_governor(&governor);
    let frontier = governor.frontier(&case()).expect("served");
    let outcome = loom.run(&commission(), &frontier);
    Observed {
        outcome,
        selections: loom.selections(),
        handed: handed
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone(),
        revalidated: loom.revalidations().len(),
    }
}

fn summary(selections: &[SelectionSnapshot]) -> Vec<(String, SelectionStrategy, SelectionState)> {
    selections
        .iter()
        .map(|held| (held.data.action.clone(), held.data.strategy, held.state))
        .collect()
}

/// A stronger selector that errs after the fast one answered below the threshold: no selection at
/// all, the fast one included, and the run is an outage.
#[test]
fn adversary_w4_a_stronger_selector_that_errs_leaves_no_fast_selection() {
    let fast = Scripted::new(SelectionStrategy::FastTyped, FAST, Some("0.1"));
    let stronger = Scripted::failing(
        SelectionStrategy::ReasoningModel,
        SelectorError::Unavailable("model down".to_owned()),
    );
    let observed = run_with(
        HybridSelector::new(&fast, &stronger, &threshold()).expect("threshold"),
        None,
    );
    assert!(
        matches!(observed.outcome, ExecutorOutcome::Suspended(_)),
        "{:?}",
        observed.outcome
    );
    assert_eq!(
        summary(&observed.selections),
        [],
        "no selection is recorded"
    );
    assert!(observed.handed.is_empty());
    assert_eq!(observed.revalidated, 0);
}

/// A stronger selector naming an action outside the catalogue: no selection, the fast one
/// included, and nothing is proposed.
#[test]
fn adversary_w4_a_stronger_selector_outside_the_catalogue_leaves_no_selection() {
    let fast = Scripted::new(SelectionStrategy::FastTyped, FAST, Some("0.1"));
    let stronger = Scripted::new(SelectionStrategy::ReasoningModel, OUTSIDE, None);
    let observed = run_with(
        HybridSelector::new(&fast, &stronger, &threshold()).expect("threshold"),
        None,
    );
    assert_eq!(summary(&observed.selections), []);
    assert!(observed.handed.is_empty());
    assert_eq!(observed.revalidated, 0);
    assert!(
        !matches!(observed.outcome, ExecutorOutcome::ProposedAction(_)),
        "{:?}",
        observed.outcome
    );
}

/// Fast and stronger name the same action: two selections still, the fast one `Overruled`, and
/// arguments are generated once.
#[test]
fn adversary_w4_the_same_action_from_both_selectors_is_recorded_twice_and_proposed_once() {
    let fast = Scripted::new(SelectionStrategy::FastTyped, FAST, Some("0.5"));
    let stronger = Scripted::new(SelectionStrategy::ReasoningModel, FAST, None);
    let observed = run_with(
        HybridSelector::new(&fast, &stronger, &threshold()).expect("threshold"),
        None,
    );
    assert_eq!(
        summary(&observed.selections),
        [
            (
                FAST.to_owned(),
                SelectionStrategy::FastTyped,
                SelectionState::Overruled
            ),
            (
                FAST.to_owned(),
                SelectionStrategy::ReasoningModel,
                SelectionState::Admitted
            ),
        ]
    );
    assert_eq!(
        observed.selections[0].data.replaced_by.as_ref(),
        Some(&observed.selections[1].data.selection_id)
    );
    assert_eq!(observed.handed, [FAST]);
    assert_eq!(observed.revalidated, 1);
}

/// A fast confidence outside [0, 1] counts as missing: the fallback is taken, and the overruled
/// selection records no confidence.
#[test]
fn adversary_w4_an_out_of_range_fast_confidence_is_overruled_and_recorded_as_none() {
    for confidence in ["1.5", "-0.5", "abc"] {
        let fast = Scripted::new(SelectionStrategy::FastTyped, FAST, Some(confidence));
        let stronger = Scripted::new(SelectionStrategy::ReasoningModel, STRONGER, None);
        let observed = run_with(
            HybridSelector::new(&fast, &stronger, &threshold()).expect("threshold"),
            None,
        );
        assert_eq!(stronger.asked(), 1, "{confidence}");
        assert_eq!(observed.selections.len(), 2, "{confidence}");
        assert_eq!(observed.selections[0].state, SelectionState::Overruled);
        assert_eq!(observed.selections[0].data.confidence, None, "{confidence}");
    }
}

/// The chosen selection keeps the id it had before the story: the same Loom instance mints the
/// same chosen id for the run whether or not a fallback happened.
#[test]
fn adversary_w4_the_chosen_selection_keeps_its_id_on_a_fallback() {
    let accepted = Scripted::new(SelectionStrategy::FastTyped, FAST, Some("0.95"));
    let low = Scripted::new(SelectionStrategy::FastTyped, FAST, Some("0.1"));
    let stronger = Scripted::new(SelectionStrategy::ReasoningModel, STRONGER, None);
    let plain = run_with(
        HybridSelector::new(&accepted, &stronger, &threshold()).expect("threshold"),
        Some("adversary-w4-instance"),
    );
    let fallback = run_with(
        HybridSelector::new(&low, &stronger, &threshold()).expect("threshold"),
        Some("adversary-w4-instance"),
    );
    assert_eq!(plain.selections.len(), 1);
    assert_eq!(fallback.selections.len(), 2);
    assert_eq!(
        fallback.selections[1].data.selection_id, plain.selections[0].data.selection_id,
        "the chosen selection's id changed with the fallback"
    );
    assert_ne!(
        fallback.selections[0].data.selection_id,
        plain.selections[0].data.selection_id
    );
}

/// Two fallback runs on one Loom: four selections, each overruled one naming its own run's
/// replacement, all ids distinct.
#[test]
fn adversary_w4_two_fallback_runs_link_each_fast_selection_to_its_own_replacement() {
    let fast = Scripted::new(SelectionStrategy::FastTyped, FAST, Some("0.1"));
    let stronger = Scripted::new(SelectionStrategy::ReasoningModel, STRONGER, None);
    let handed = Handed::default();
    let governor = governor(2);
    let loom = Loom::new(
        HybridSelector::new(&fast, &stronger, &threshold()).expect("threshold"),
        &handed,
        PROMPT,
    )
    .with_governor(&governor);
    for _ in 0..2 {
        let frontier = governor.frontier(&case()).expect("served");
        let outcome = loom.run(&commission(), &frontier);
        assert!(
            matches!(outcome, ExecutorOutcome::ProposedAction(_)),
            "{outcome:?}"
        );
    }
    let selections = loom.selections();
    assert_eq!(selections.len(), 4, "{:?}", summary(&selections));
    let ids: std::collections::BTreeSet<String> = selections
        .iter()
        .map(|held| held.data.selection_id.0.0.clone())
        .collect();
    assert_eq!(ids.len(), 4);
    for pair in selections.chunks(2) {
        assert_eq!(pair[0].state, SelectionState::Overruled);
        assert_eq!(
            pair[0].data.replaced_by.as_ref(),
            Some(&pair[1].data.selection_id)
        );
        assert_eq!(pair[1].state, SelectionState::Admitted);
    }
}

/// A hybrid as the fast selector of a hybrid: the inner one falls back to a confident stronger
/// pick, which the outer accepts. The inner fast selection is recorded `Overruled`, naming the
/// inner stronger's, and the outer stronger is never asked; no selection carries `Hybrid`.
#[test]
fn adversary_w4_a_nested_hybrid_records_the_inner_fallback() {
    let inner_fast = Scripted::new(SelectionStrategy::FastTyped, FAST, Some("0.1"));
    let inner_stronger = Scripted::new(SelectionStrategy::Rule, THIRD, Some("0.95"));
    let outer_stronger = Scripted::new(SelectionStrategy::ReasoningModel, STRONGER, None);
    let inner = HybridSelector::new(&inner_fast, &inner_stronger, &threshold()).expect("t");
    let observed = run_with(
        HybridSelector::new(inner, &outer_stronger, &threshold()).expect("threshold"),
        None,
    );
    assert_eq!(outer_stronger.asked(), 0);
    assert_eq!(
        summary(&observed.selections),
        [
            (
                FAST.to_owned(),
                SelectionStrategy::FastTyped,
                SelectionState::Overruled
            ),
            (
                THIRD.to_owned(),
                SelectionStrategy::Rule,
                SelectionState::Admitted
            ),
        ]
    );
    assert_eq!(observed.handed, [THIRD]);
}

/// `selection::resolve` asks each selector once, and records the chosen pick under the stronger
/// selector's strategy; `selection::select` with the same hybrid answers the same action.
#[test]
fn adversary_w4_resolve_and_select_agree_on_the_action_for_a_hybrid() {
    let catalogue = project(
        &governor(1).frontier(&case()).expect("served"),
        CatalogueId(Uuid("00000000-0000-4000-8000-000000000c00".to_owned())),
        TurnId(Uuid("00000000-0000-4000-8000-000000000700".to_owned())),
    );
    let context = SelectionContext {
        prompt: PROMPT.to_owned(),
    };
    let fast = Scripted::new(SelectionStrategy::FastTyped, FAST, Some("0.1"));
    let stronger = Scripted::new(SelectionStrategy::ReasoningModel, STRONGER, None);
    let hybrid = HybridSelector::new(&fast, &stronger, &threshold()).expect("threshold");
    let resolved = selection::resolve(
        &hybrid,
        &context,
        &catalogue,
        (
            SelectionId(Uuid("00000000-0000-4000-8000-000000005e01".to_owned())),
            SelectionId(Uuid("00000000-0000-4000-8000-000000005e02".to_owned())),
        ),
    )
    .unwrap_or_else(|_| panic!("resolved"));
    assert_eq!((fast.asked(), stronger.asked()), (1, 1));
    assert_eq!(resolved.selection.data().action, STRONGER);
    assert_eq!(
        resolved.selection.data().strategy,
        SelectionStrategy::ReasoningModel
    );
    let overruled = resolved.overruled.expect("the fast pick is a selection");
    assert_eq!(overruled.data().action, FAST);
    assert_eq!(overruled.data().strategy, SelectionStrategy::FastTyped);
    assert_eq!(overruled.data().replaced_by, None);

    let selected = selection::select(
        &hybrid,
        &context,
        &catalogue,
        SelectionId(Uuid("00000000-0000-4000-8000-000000005e03".to_owned())),
    )
    .unwrap_or_else(|_| panic!("selected"));
    assert_eq!(selected.data().action, STRONGER);
}

/// The story: "Each recorded selection carries the strategy of the selector that made it". The
/// public `Loom::select` still goes through `selection::select`, so a hybrid that fell back hands
/// back a selection made by the reasoning model under the strategy `Hybrid`.
#[test]
fn adversary_w4_loom_select_carries_the_strategy_of_the_selector_that_made_the_pick() {
    let catalogue = project(
        &governor(1).frontier(&case()).expect("served"),
        CatalogueId(Uuid("00000000-0000-4000-8000-000000000c00".to_owned())),
        TurnId(Uuid("00000000-0000-4000-8000-000000000700".to_owned())),
    );
    let fast = Scripted::new(SelectionStrategy::FastTyped, FAST, Some("0.1"));
    let stronger = Scripted::new(SelectionStrategy::ReasoningModel, STRONGER, None);
    let handed = Handed::default();
    let loom = Loom::new(
        HybridSelector::new(&fast, &stronger, &threshold()).expect("threshold"),
        &handed,
        PROMPT,
    );
    let selection = loom
        .select(
            &catalogue,
            SelectionId(Uuid("00000000-0000-4000-8000-000000005e04".to_owned())),
        )
        .unwrap_or_else(|_| panic!("selected"));
    assert_eq!(selection.data().action, STRONGER);
    assert_eq!(
        selection.data().strategy,
        SelectionStrategy::ReasoningModel,
        "Loom::select returned the reasoning model's pick under {:?}",
        selection.data().strategy
    );
}
