//! Acceptance for `story:reasoning-model-selector`, held against the selector whose strategy is
//! `ReasoningModel` since `story:metering-on-llm-model`: the model selector over an llm `Model`
//! (`b10x_loom_executor::model_selector::ModelSelector`). With a scripted model and a
//! three-candidate set, it returns the candidate the script names, and Loom returns a selection
//! error rather than a selection when the script names an action outside the set (Atlas ADR 0073
//! § Decision; `docs/contracts/loom-action-selection.md`, safety rules 1 and 2).
//!
//! The model is reached through llm's `Model` only. The script answers each turn by calling the
//! one tool the turn is held to, with the action it was given, so what it names is the script's
//! and the tool it calls is the selector's. No model or network call is made.

use std::collections::VecDeque;
use std::sync::Mutex;

use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome, TurnRequest,
};
use serde_json::{Value, json};

use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{
    ActionCatalogue, ActionCatalogueData, ActionNotInCatalogue, CatalogueEntry,
    CatalogueEntryStatus, CatalogueId, SelectionId, SelectionStrategy, TurnId,
    action_catalogue_state,
};
use b10x_loom_executor::model_selector::{Briefing, ModelSelector};
use b10x_loom_executor::selection::{SelectionContext, SelectionRefusal, select};
use b10x_loom_executor::{ActionSelector, SelectorError};

const PROMPT: &str = "the build on main is red; find out why";

/// The model the scripted binding serves, which every request names.
const MODEL: &str = "reasoning-model";

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

/// An llm `Model` that replays its script and keeps every request it was sent.
struct Scripted {
    provenance: Provenance,
    capabilities: Capabilities,
    answers: Mutex<VecDeque<Answer>>,
    requests: Mutex<Vec<TurnRequest>>,
}

impl Scripted {
    fn new(answers: Vec<Answer>) -> Self {
        let id = |value: &str| Id::new(value).expect("a fixture id");
        Self {
            provenance: Provenance {
                protocol: Protocol::Responses,
                provider: id("scripted"),
                account: id("fixture"),
                endpoint: id("in-process"),
                model: id(MODEL),
                binding_revision: id("scripted-1"),
            },
            capabilities: Capabilities {
                tools: true,
                tool_choice: true,
                ..Capabilities::text(128_000, 8_192)
            },
            answers: Mutex::new(answers.into()),
            requests: Mutex::default(),
        }
    }

    fn requests(&self) -> Vec<TurnRequest> {
        self.requests.lock().expect("lock").clone()
    }

    fn answer(&self, request: &TurnRequest) -> Result<TurnOutcome, Error> {
        let observation = TurnObservation {
            final_usage: true,
            ..TurnObservation::new(self.provenance.clone())
        };
        match self.answers.lock().expect("lock").pop_front() {
            Some(Answer::Names(action)) => {
                let ToolChoice::Named(name) = &request.tool_choice else {
                    return Err(Error::protocol("the turn is held to no tool"));
                };
                Ok(TurnOutcome {
                    stop_reason: StopReason::ToolCalls,
                    items: vec![Item::ToolCall(ToolCall {
                        call_id: CallId::new("call-1").expect("valid call id"),
                        name: name.clone(),
                        arguments: json!({ "action": action }),
                    })],
                    observation,
                })
            }
            Some(Answer::Prose) => Ok(TurnOutcome {
                stop_reason: StopReason::EndTurn,
                items: vec![Item::assistant("I would run the tests.")],
                observation,
            }),
            None => Err(Error::protocol("the script has no further turn")),
        }
    }
}

impl Model for Scripted {
    fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        _sink: &'a mut dyn StreamSink,
        _cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        self.requests.lock().expect("lock").push(request.clone());
        let answered = self.answer(request);
        Box::pin(async move { answered })
    }
}

/// What the selector tells the model: the run's instruction.
struct Brief;

impl Briefing for Brief {
    fn text(&self) -> String {
        format!("Instruction: {PROMPT}\n")
    }

    fn selection_instructions(&self) -> String {
        "Choose the one candidate action that best advances the instruction.".to_owned()
    }

    fn argument_instructions(&self) -> String {
        "Write the selected action's arguments.".to_owned()
    }

    fn arguments_schema(&self, _action: &str) -> Value {
        json!({"type": "object"})
    }
}

fn selector(model: &Scripted) -> ModelSelector<'_, Brief> {
    ModelSelector::new(model, Brief)
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
    let ToolChoice::Named(name) = &request.tool_choice else {
        panic!(
            "the selection turn is held to one tool: {:?}",
            request.tool_choice
        );
    };
    let tool = request
        .tools
        .iter()
        .find(|tool| tool.name == *name)
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
    let model = Scripted::new(vec![Answer::Names(json!(TESTS_RUN))]);
    let selector = selector(&model);
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
    let requests = model.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert!(matches!(request.tool_choice, ToolChoice::Named(_)));
    assert_eq!(request.tools.len(), 1);
    assert_eq!(request.model, MODEL);
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
    let model = Scripted::new(vec![Answer::Names(json!(OUTSIDE))]);
    let selector = selector(&model);

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
    assert_eq!(offered(&model.requests()[0]), [INSPECT, TESTS_RUN, MERGE]);
}

/// An answer that names no action — prose, a non-string `action`, a failed turn — is the selector
/// failing to answer, never a guess at one.
#[test]
fn an_answer_naming_no_action_is_a_selector_error() {
    for answers in [
        vec![Answer::Prose],
        vec![Answer::Names(json!(3))],
        Vec::new(),
    ] {
        let model = Scripted::new(answers);
        let refusal = select(&selector(&model), &context(), &catalogue(), selection_id())
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
    let model = Scripted::new(vec![Answer::Names(json!(TESTS_RUN))]);
    assert_eq!(
        selector(&model).select(&context(), &[]),
        Err(SelectorError::NothingAdmissible)
    );
    assert_eq!(model.requests().len(), 0);
}
