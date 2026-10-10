//! Acceptance for `story:fallback-selection-recording`: a confidence fallback is recorded as two
//! linked `loom.run.Selection`s (`decision-blocker:fallback-selection-record`, option B).
//!
//! Loom runs as Commission's `AgentExecutor` with a `HybridSelector` of two scripted selectors, a
//! `FastTyped` one and a `ReasoningModel` one, and a fake governor it revalidates against. When the
//! fast selection is overruled, the record holds both: the fast one `Overruled`, naming the
//! replacement's `selection_id` in `replaced_by`, and the replacement, which alone reaches argument
//! generation and revalidation. When it is not overruled, the record holds one selection with no
//! replacement. A fast selector that errs or names an action outside the candidates leaves no fast
//! selection. Each selection carries the strategy of the selector that made it.
//!
//! `loom.run.OverruleSelection` on the record refuses a replacement it does not hold with
//! `SelectionNotFound` naming it, and an overruled selection is never given arguments and never
//! revalidated. No model or network call is made.

use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorOutcomeProposedAction, FrontierAction, PrincipalId,
    ProposedActionArguments, commission_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_executor::SelectorError;
use b10x_loom_executor::arguments::{OverruleRefused, RequestRecord};
use b10x_loom_executor::model::behaviour::SelectionStorage;
use b10x_loom_executor::model::primitives::{Decimal, Uuid};
use b10x_loom_executor::model::run::obligations::{
    RequestArgumentsBehavior, RevalidateSelectionBehavior,
};
use b10x_loom_executor::model::run::{
    AnySelection, ArgumentRequestId, CatalogueEntry, CatalogueId, OverruleSelection,
    OverruleSelectionOutcome, RequestArguments, RequestArgumentsOutcome, RevalidateSelection,
    RevalidateSelectionOutcome, Selection, SelectionAdmitted, SelectionData, SelectionId,
    SelectionNotFound, SelectionOverruled, SelectionSnapshot, SelectionState, SelectionStrategy,
};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{
    ActionSelector, ArgumentContext, ArgumentGenerator, HybridSelector, Loom,
};

const CASE: &str = "CASE-FALLBACK-1";
const REVISION: i64 = 7;
const PROMPT: &str = "The checkout latency doubled after the last deploy; find out why.";

/// What the scripted fast selector names when it names a candidate.
const FAST: &str = "metrics.inspect";
/// What the scripted stronger selector names: another candidate.
const STRONGER: &str = "logs.search";
const RELEASE: &str = "release.inspect";
/// The frontier the governor serves.
const FRONTIER: [&str; 3] = [FAST, STRONGER, RELEASE];
/// Listed nowhere in the frontier.
const OUTSIDE: &str = "release.rollback";

/// The threshold this test supplies; the hybrid has no default.
const THRESHOLD: &str = "0.9";

// ---------------------------------------------------------------------------------------------
// Scripted selectors and a counting argument generator.

/// A selector that gives one scripted answer under one strategy and counts how often it was asked.
struct Scripted {
    answer: Result<Choice, SelectorError>,
    strategy: SelectionStrategy,
    asked: Mutex<usize>,
}

impl Scripted {
    fn fast(answer: Result<Choice, SelectorError>) -> Self {
        Self {
            answer,
            strategy: SelectionStrategy::FastTyped,
            asked: Mutex::new(0),
        }
    }

    fn fast_naming(action: &str, confidence: Option<&str>) -> Self {
        Self::fast(Ok(Choice {
            action: action.to_owned(),
            confidence: confidence.map(|text| Decimal(text.to_owned())),
        }))
    }

    fn stronger() -> Self {
        Self {
            answer: Ok(Choice {
                action: STRONGER.to_owned(),
                confidence: None,
            }),
            strategy: SelectionStrategy::ReasoningModel,
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

/// The arguments the generator writes for `action`.
fn arguments_for(action: &str) -> CommissionValue {
    CommissionValue::Object(vec![(
        "for".to_owned(),
        CommissionValue::Text(action.to_owned()),
    )])
}

/// An argument generator that records every entry it is handed.
#[derive(Default)]
struct CountingArguments {
    handed: Mutex<Vec<String>>,
}

impl ArgumentGenerator for &CountingArguments {
    fn generate(
        &self,
        _context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<CommissionValue, String> {
        self.handed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(entry.action.clone());
        Ok(arguments_for(&entry.action))
    }
}

// ---------------------------------------------------------------------------------------------
// Commission's side.

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000f01".to_owned(),
        )),
        agent_revision_id: AgentRevisionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000f02".to_owned(),
        )),
        case_id: case(),
        principal: PrincipalId("principal-sre".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

/// The fake governor, holding the case at [`REVISION`] with the three-action frontier.
fn governor() -> FakeGovernor {
    let governor = FakeGovernor::new();
    let actions = FRONTIER
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
        [Answer::at(REVISION).with_items(Vec::new(), Vec::new(), actions)],
    );
    governor
}

// ---------------------------------------------------------------------------------------------
// One run.

/// What one run produced.
struct Observed {
    outcome: ExecutorOutcome,
    selections: Vec<SelectionSnapshot>,
    argument_requests: Vec<SelectionId>,
    revalidations: Vec<RevalidateSelectionOutcome>,
    handed: Vec<String>,
    stronger_asked: usize,
}

/// One run of Loom on the governor's frontier with a hybrid of `fast` and the scripted stronger
/// selector at [`THRESHOLD`].
fn run(fast: &Scripted) -> Observed {
    let stronger = Scripted::stronger();
    let selector = HybridSelector::new(fast, &stronger, &Decimal(THRESHOLD.to_owned()))
        .expect("the supplied threshold is a decimal in [0, 1]");
    let arguments = CountingArguments::default();
    let governor = governor();
    let loom = Loom::new(selector, &arguments, PROMPT).with_governor(&governor);
    let frontier = governor
        .frontier(&case())
        .expect("the governor serves the case");

    let outcome = loom.run(&commission(), &frontier);

    assert_eq!(fast.asked(), 1, "the fast selector is asked once");
    Observed {
        outcome,
        selections: loom.selections(),
        argument_requests: loom
            .argument_requests()
            .into_iter()
            .map(|request| request.data.selection_id)
            .collect(),
        revalidations: loom.revalidations(),
        handed: arguments
            .handed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone(),
        stronger_asked: stronger.asked(),
    }
}

/// `expected` was proposed with its arguments, and the one selection that made it is the only one
/// the argument request and the revalidation serve: admitted, made by `strategy`, replaced by none.
fn assert_proposed(name: &str, observed: &Observed, expected: &str, strategy: SelectionStrategy) {
    assert_eq!(
        observed.outcome,
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: expected.to_owned(),
            arguments: ProposedActionArguments(arguments_for(expected)),
        }),
        "{name}: the proposal"
    );
    let proposed = observed
        .selections
        .last()
        .unwrap_or_else(|| panic!("{name}: no selection was recorded"));
    assert_eq!(proposed.data.action, expected, "{name}: the selection");
    assert_eq!(proposed.data.strategy, strategy, "{name}: its strategy");
    assert_eq!(proposed.state, SelectionState::Admitted, "{name}: admitted");
    assert_eq!(
        proposed.data.replaced_by, None,
        "{name}: it has no replacement"
    );
    assert_eq!(
        observed.handed,
        [expected],
        "{name}: arguments are generated once, for the proposed action only"
    );
    assert_eq!(
        observed.argument_requests,
        [proposed.data.selection_id.clone()],
        "{name}: one argument request, serving the proposed selection"
    );
    assert_eq!(
        observed.revalidations,
        [RevalidateSelectionOutcome::Admitted {
            selection_admitted: SelectionAdmitted {
                selection_id: proposed.data.selection_id.clone(),
            },
        }],
        "{name}: one revalidation, of the proposed selection"
    );
}

// ---------------------------------------------------------------------------------------------
// The acceptance.

/// A fast choice below the threshold, or with no confidence, is overruled: the record holds the
/// fast selection `Overruled` and naming its replacement, then the stronger selector's, each with
/// its own selector's strategy and both from the run's one catalogue. The fast one never reaches
/// argument generation or revalidation.
#[test]
fn an_overruled_fast_selection_is_recorded_overruled_naming_its_replacement() {
    for (name, confidence) in [
        ("below the threshold", Some("0.42")),
        ("no confidence", None),
    ] {
        let fast = Scripted::fast_naming(FAST, confidence);
        let observed = run(&fast);

        assert_eq!(
            observed.stronger_asked, 1,
            "{name}: the stronger selector is asked"
        );
        assert_eq!(
            observed.selections.len(),
            2,
            "{name}: two selections: {:?}",
            observed.selections
        );
        assert_proposed(name, &observed, STRONGER, SelectionStrategy::ReasoningModel);

        let (overruled, replacement) = (&observed.selections[0], &observed.selections[1]);
        assert_eq!(overruled.data.action, FAST, "{name}: the fast selection");
        assert_eq!(
            overruled.data.strategy,
            SelectionStrategy::FastTyped,
            "{name}: the fast selection carries the fast selector's strategy"
        );
        assert_eq!(
            overruled.data.confidence,
            confidence.map(|text| Decimal(text.to_owned())),
            "{name}: the fast selection carries the fast selector's confidence"
        );
        assert_eq!(
            overruled.state,
            SelectionState::Overruled,
            "{name}: the fast selection is overruled"
        );
        assert_eq!(
            overruled.data.replaced_by,
            Some(replacement.data.selection_id.clone()),
            "{name}: the fast selection names its replacement"
        );
        assert_ne!(
            overruled.data.selection_id, replacement.data.selection_id,
            "{name}: the two selections have ids of their own"
        );
        assert_eq!(
            overruled.data.catalogue_id, replacement.data.catalogue_id,
            "{name}: both selections were made from the run's one catalogue"
        );
        assert_eq!(
            overruled.data.case_revision, REVISION,
            "{name}: the fast selection carries its catalogue's case revision"
        );
        assert!(
            !observed.handed.iter().any(|action| action == FAST),
            "{name}: the fast selection reached argument generation"
        );
        assert!(
            !observed
                .argument_requests
                .contains(&overruled.data.selection_id),
            "{name}: an argument request serves the overruled selection"
        );
    }
}

/// A fast choice at or above the threshold is not overruled: the record holds that one selection,
/// made by the fast selector, with no replacement, and the stronger selector is never asked.
#[test]
fn a_fast_selection_that_was_not_overruled_is_the_one_selection_with_no_replacement() {
    for (name, confidence) in [
        ("above the threshold", "0.96"),
        ("at the threshold", "0.90"),
    ] {
        let fast = Scripted::fast_naming(FAST, Some(confidence));
        let observed = run(&fast);

        assert_eq!(
            observed.stronger_asked, 0,
            "{name}: the stronger selector is not asked"
        );
        assert_eq!(
            observed.selections.len(),
            1,
            "{name}: one selection: {:?}",
            observed.selections
        );
        assert_proposed(name, &observed, FAST, SelectionStrategy::FastTyped);
        assert_eq!(
            observed.selections[0].data.confidence,
            Some(Decimal(confidence.to_owned())),
            "{name}: the selection carries the fast selector's confidence"
        );
    }
}

/// A fast selector that errs, or names an action outside the candidates, leaves no fast selection:
/// `selection::chosen` refuses it before a `Selection` exists. The record holds only the stronger
/// selector's, with no replacement.
#[test]
fn a_fast_selector_that_errs_or_leaves_the_candidates_leaves_no_fast_selection() {
    let cases = [
        (
            "unavailable",
            Scripted::fast(Err(SelectorError::Unavailable("timed out".to_owned()))),
        ),
        (
            "nothing admissible",
            Scripted::fast(Err(SelectorError::NothingAdmissible)),
        ),
        (
            "outside the candidates",
            Scripted::fast_naming(OUTSIDE, Some("0.99")),
        ),
    ];
    for (name, fast) in &cases {
        let observed = run(fast);

        assert_eq!(
            observed.stronger_asked, 1,
            "{name}: the stronger selector is asked"
        );
        assert_eq!(
            observed.selections.len(),
            1,
            "{name}: only the stronger selector's selection: {:?}",
            observed.selections
        );
        assert_proposed(name, &observed, STRONGER, SelectionStrategy::ReasoningModel);
        assert!(
            observed
                .selections
                .iter()
                .all(|held| held.data.action != OUTSIDE && held.state != SelectionState::Overruled),
            "{name}: a fast selection was recorded: {:?}",
            observed.selections
        );
    }
}

// ---------------------------------------------------------------------------------------------
// `loom.run.OverruleSelection` on the record.

fn uuid(n: u32) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn selected(id: u32, action: &str) -> SelectionSnapshot {
    AnySelection::Selected(Selection::new(SelectionData {
        selection_id: SelectionId(uuid(id)),
        catalogue_id: CatalogueId(uuid(0xc00)),
        action: action.to_owned(),
        confidence: None,
        strategy: SelectionStrategy::FastTyped,
        case_revision: REVISION,
        replaced_by: None,
    }))
    .snapshot()
}

/// `OverruleSelection` naming a replacement the record does not hold is refused with
/// `SelectionNotFound` naming the replacement, and the selection stays `Selected`, replaced by
/// none.
#[test]
fn overrule_selection_naming_an_unknown_replacement_is_refused_with_selection_not_found() {
    let mut record = RequestRecord::default();
    record.put(selected(0x5e1, FAST));

    let refused = record.overrule(OverruleSelection {
        selection_id: SelectionId(uuid(0x5e1)),
        replacement_id: SelectionId(uuid(0x5e9)),
    });

    assert_eq!(
        refused,
        Err(OverruleRefused::ReplacementNotFound(SelectionNotFound {
            selection_id: SelectionId(uuid(0x5e9)),
        })),
    );
    assert_eq!(record.selections(), [selected(0x5e1, FAST)]);
}

/// With the replacement held, the selection is overruled and names it; an overruled selection is
/// then never given arguments and never revalidated.
#[test]
fn an_overruled_selection_is_never_given_arguments_nor_revalidated() {
    let mut record = RequestRecord::default();
    record.put(selected(0x5e1, FAST));
    record.put(selected(0x5e2, STRONGER));

    let overruled = record.overrule(OverruleSelection {
        selection_id: SelectionId(uuid(0x5e1)),
        replacement_id: SelectionId(uuid(0x5e2)),
    });
    assert_eq!(
        overruled,
        Ok(OverruleSelectionOutcome::Overruled {
            selection_overruled: SelectionOverruled {
                selection_id: SelectionId(uuid(0x5e1)),
                replacement_id: SelectionId(uuid(0x5e2)),
            },
        })
    );
    let held = record
        .get(&SelectionId(uuid(0x5e1)))
        .expect("the overruled selection is held");
    assert_eq!(held.state, SelectionState::Overruled);
    assert_eq!(held.data.replaced_by, Some(SelectionId(uuid(0x5e2))));

    let requested = record.request_arguments(RequestArguments {
        argument_request_id: ArgumentRequestId(uuid(0xa01)),
        selection_id: SelectionId(uuid(0x5e1)),
    });
    assert!(
        matches!(
            requested,
            Ok(RequestArgumentsOutcome::SelectionNotSelected { .. })
        ),
        "{requested:?}"
    );
    assert!(record.argument_requests().is_empty());

    let revalidated = record.revalidate_selection(RevalidateSelection {
        selection_id: SelectionId(uuid(0x5e1)),
        case_revision: REVISION,
        frontier_actions: FRONTIER.iter().map(|action| (*action).to_owned()).collect(),
    });
    assert!(
        matches!(
            revalidated,
            Ok(RevalidateSelectionOutcome::WrongState { ref error })
                if error.state == SelectionState::Overruled
        ),
        "{revalidated:?}"
    );
    assert_eq!(
        record.get(&SelectionId(uuid(0x5e1))).map(|held| held.state),
        Some(SelectionState::Overruled)
    );

    let again = record.overrule(OverruleSelection {
        selection_id: SelectionId(uuid(0x5e1)),
        replacement_id: SelectionId(uuid(0x5e2)),
    });
    assert!(
        matches!(
            again,
            Ok(OverruleSelectionOutcome::WrongState { ref error })
                if error.state == SelectionState::Overruled
        ),
        "a selection is overruled once: {again:?}"
    );
}
