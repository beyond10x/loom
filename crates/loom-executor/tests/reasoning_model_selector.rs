//! Acceptance for `story:reasoning-model-selector`: with a scripted model client and a
//! three-candidate set, the reasoning-model selector returns the candidate the script names, and
//! returns a selection error rather than a selection when the script names an action outside the
//! set (Atlas ADR 0073 § Decision; `docs/contracts/loom-action-selection.md`, safety rules 1 and 2).
//!
//! The model is reached through the provider-neutral `ModelPort` only. The script answers each turn
//! by calling the one tool the turn is held to, with the action it was given, so what it names is
//! the script's and the tool it calls is the selector's. No model or network call is made.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use b10x_loom_executor::harness::wire::{
    CallId, Item, ModelPort, StopReason, StreamSink, ToolCall, ToolChoice, TurnOutcome,
    TurnRequest, WireError, WireId,
};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{
    ActionCatalogue, ActionCatalogueData, ActionNotInCatalogue, CatalogueEntry,
    CatalogueEntryStatus, CatalogueId, SelectionId, SelectionStrategy, TurnId,
    action_catalogue_state,
};
use b10x_loom_executor::selection::{SelectionContext, SelectionRefusal, select};
use b10x_loom_executor::{ActionSelector, ReasoningModelSelector, SelectorError};

const PROMPT: &str = "the build on main is red; find out why";

const INSPECT: &str = "repository.inspect";

/// The candidate the script names: neither the first entry nor the first admissible one, so a
/// selector that ignored the model could not pick it by position.
const TESTS_RUN: &str = "tests.run";

const MERGE: &str = "repository.merge";

/// Listed nowhere in the candidate set.
const OUTSIDE: &str = "release.publish";

const REVISION: i64 = 7;

/// One scripted answer: the value the forced call carries as `action`, or prose with no call.
enum Answer {
    Names(Value),
    Prose,
}

/// A `ModelPort` that replays its script and keeps every request it was sent.
struct Scripted {
    wire: WireId,
    answers: VecDeque<Answer>,
    requests: Arc<Mutex<Vec<TurnRequest>>>,
}

impl Scripted {
    fn new(answers: Vec<Answer>) -> (Self, Arc<Mutex<Vec<TurnRequest>>>) {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let model = Self {
            wire: WireId::new("scripted").expect("valid wire id"),
            answers: answers.into(),
            requests: requests.clone(),
        };
        (model, requests)
    }
}

impl ModelPort for Scripted {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.requests.lock().expect("lock").push(request.clone());
        match self.answers.pop_front() {
            Some(Answer::Names(action)) => {
                let name = request
                    .tool_choice
                    .named()
                    .cloned()
                    .ok_or_else(|| WireError::protocol("the turn is held to no tool"))?;
                Ok(TurnOutcome {
                    stop_reason: StopReason::ToolCalls,
                    items: vec![Item::ToolCall(ToolCall {
                        call_id: CallId::new("call-1").expect("valid call id"),
                        name,
                        arguments: json!({ "action": action }),
                    })],
                    usage: None,
                })
            }
            Some(Answer::Prose) => Ok(TurnOutcome {
                stop_reason: StopReason::EndTurn,
                items: vec![Item::assistant("I would run the tests.")],
                usage: None,
            }),
            None => Err(WireError::protocol("the script has no further turn")),
        }
    }
}

fn entry(action: &str, status: CatalogueEntryStatus) -> CatalogueEntry {
    CatalogueEntry {
        action: action.to_owned(),
        status,
    }
}

/// The three-candidate set, as a projected catalogue.
fn catalogue() -> ActionCatalogue<action_catalogue_state::Projected> {
    ActionCatalogue::new(ActionCatalogueData {
        catalogue_id: CatalogueId(Uuid("00000000-0000-4000-8000-0000000007c1".to_owned())),
        turn_id: TurnId(Uuid("00000000-0000-4000-8000-0000000007a1".to_owned())),
        frontier: "frontier of CHG-1842".to_owned(),
        case_revision: REVISION,
        entries: vec![
            entry(INSPECT, CatalogueEntryStatus::Admissible),
            entry(TESTS_RUN, CatalogueEntryStatus::Admissible),
            entry(MERGE, CatalogueEntryStatus::ApprovalRequired),
        ],
    })
}

fn context() -> SelectionContext {
    SelectionContext {
        prompt: PROMPT.to_owned(),
    }
}

fn selection_id() -> SelectionId {
    SelectionId(Uuid("00000000-0000-4000-8000-0000000007e1".to_owned()))
}

/// The ids the request held the model to: the `enum` of the forced tool's `action` property.
fn offered(request: &TurnRequest) -> Vec<String> {
    let name = request
        .tool_choice
        .named()
        .expect("the selection turn is held to one tool");
    let tool = request
        .tool(name)
        .expect("the turn publishes the tool it is held to");
    tool.input_schema["properties"]["action"]["enum"]
        .as_array()
        .expect("the action is a fixed choice")
        .iter()
        .map(|id| id.as_str().expect("an id is a string").to_owned())
        .collect()
}

#[test]
fn the_reasoning_model_selector_returns_the_candidate_the_script_names() {
    let (model, requests) = Scripted::new(vec![Answer::Names(json!(TESTS_RUN))]);
    let selector = ReasoningModelSelector::new(model, "reasoning-model");
    assert_eq!(selector.strategy(), SelectionStrategy::ReasoningModel);

    let catalogue = catalogue();
    let selection = select(&selector, &context(), &catalogue, selection_id())
        .expect("a candidate the script names is selected")
        .into_data();

    assert_eq!(selection.action, TESTS_RUN);
    assert_eq!(selection.strategy, SelectionStrategy::ReasoningModel);
    assert_eq!(selection.catalogue_id, catalogue.data().catalogue_id);
    assert_eq!(selection.case_revision, REVISION);
    assert_eq!(selection.selection_id, selection_id());
    assert_eq!(selection.confidence, None);

    // One turn, held to one tool whose only argument is a fixed choice of exactly the three
    // candidate ids, in the order they were given; the instruction is the run's.
    let requests = requests.lock().expect("lock");
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert!(matches!(request.tool_choice, ToolChoice::Named(_)));
    assert_eq!(request.tools.len(), 1);
    assert_eq!(request.model, "reasoning-model");
    assert_eq!(offered(request), [INSPECT, TESTS_RUN, MERGE]);
    assert!(
        request
            .items
            .iter()
            .any(|item| matches!(item, Item::UserText { text } if text.contains(PROMPT))),
        "the selection turn carries the run's instruction"
    );
}

#[test]
fn an_action_outside_the_candidate_set_is_a_selection_error_and_never_a_selection() {
    let (model, requests) = Scripted::new(vec![Answer::Names(json!(OUTSIDE))]);
    let selector = ReasoningModelSelector::new(model, "reasoning-model");

    let refusal = select(&selector, &context(), &catalogue(), selection_id())
        .map(|selection| selection.into_data())
        .expect_err("an action outside the candidate set is never selected");

    assert_eq!(
        refusal,
        SelectionRefusal::NotInCatalogue(ActionNotInCatalogue {
            action: OUTSIDE.to_owned(),
        })
    );
    // The model was offered the three candidates and not the action it named.
    let requests = requests.lock().expect("lock");
    assert_eq!(offered(&requests[0]), [INSPECT, TESTS_RUN, MERGE]);
}

/// An answer that names no action — prose, a non-string `action`, a wire failure — is the selector
/// failing to answer, never a guess at one.
#[test]
fn an_answer_naming_no_action_is_a_selector_error() {
    for answers in [
        vec![Answer::Prose],
        vec![Answer::Names(json!(3))],
        Vec::new(),
    ] {
        let (model, _requests) = Scripted::new(answers);
        let selector = ReasoningModelSelector::new(model, "reasoning-model");
        let refusal = select(&selector, &context(), &catalogue(), selection_id())
            .map(|selection| selection.into_data())
            .expect_err("no action was named");
        assert!(
            matches!(
                refusal,
                SelectionRefusal::Selector(SelectorError::Unavailable(_))
            ),
            "{refusal:?}"
        );
    }
}

/// With no candidates there is nothing to choose, and the model is not asked.
#[test]
fn an_empty_candidate_set_asks_no_model() {
    let (model, requests) = Scripted::new(vec![Answer::Names(json!(TESTS_RUN))]);
    let selector = ReasoningModelSelector::new(model, "reasoning-model");
    assert_eq!(
        selector.select(&context(), &[]),
        Err(SelectorError::NothingAdmissible)
    );
    assert_eq!(requests.lock().expect("lock").len(), 0);
}
