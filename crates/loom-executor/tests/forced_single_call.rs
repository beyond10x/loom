//! Acceptance for `story:forced-call-asks-for-one-call`: every forced request of the model
//! selector and argument generator (`select_action`, `action_arguments`) asks the model for one
//! call (`parallel_tool_calls: Some(false)`, which llm's Messages codec sends as
//! `disable_parallel_tool_use: true`), and an answer that still calls the tool more than once is
//! taken only when every call carries the same arguments. Calls that differ stay unusable: which
//! one the model meant is not stated.
//!
//! The model is a script: each turn answers with the calls it was given, under the tool the turn
//! is held to. No model or network call is made.

use std::collections::VecDeque;
use std::sync::Mutex;

use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome, TurnRequest,
};
use serde_json::{Value, json};

use b10x_loom_executor::model_selector::{
    ARGUMENTS_TOOL, AskError, Briefing, SELECT_TOOL, arguments_once, request, select_once,
};

const MODEL: &str = "forced-model";

/// An llm `Model` that answers each turn with the calls its script holds for it, and keeps every
/// request it was sent.
struct Scripted {
    provenance: Provenance,
    capabilities: Capabilities,
    answers: Mutex<VecDeque<Vec<Value>>>,
    requests: Mutex<Vec<TurnRequest>>,
}

impl Scripted {
    fn new(answers: Vec<Vec<Value>>) -> Self {
        let id = |value: &str| Id::new(value).expect("a fixture id");
        Self {
            provenance: Provenance {
                protocol: Protocol::Messages,
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
        let answered = match (
            &request.tool_choice,
            self.answers.lock().expect("lock").pop_front(),
        ) {
            (ToolChoice::Named(name), Some(calls)) => Ok(TurnOutcome {
                stop_reason: StopReason::ToolCalls,
                items: calls
                    .into_iter()
                    .enumerate()
                    .map(|(n, arguments)| {
                        Item::ToolCall(ToolCall {
                            call_id: CallId::new(format!("call-{n}")).expect("valid call id"),
                            name: name.clone(),
                            arguments,
                        })
                    })
                    .collect(),
                observation: TurnObservation {
                    final_usage: true,
                    ..TurnObservation::new(self.provenance.clone())
                },
            }),
            (_, None) => Err(Error::protocol("the script has no further turn")),
            (choice, Some(_)) => Err(Error::protocol(format!("not held to a tool: {choice:?}"))),
        };
        Box::pin(async move { answered })
    }
}

struct Brief;

impl Briefing for Brief {
    fn text(&self) -> String {
        "Item: the build is red.\n".to_owned()
    }

    fn selection_instructions(&self) -> String {
        "Choose one candidate.".to_owned()
    }

    fn argument_instructions(&self) -> String {
        "Write the arguments.".to_owned()
    }

    fn arguments_schema(&self, _action: &str) -> Value {
        json!({"type": "object"})
    }
}

fn select_schema() -> Value {
    json!({
        "type": "object",
        "properties": {"action": {"type": "string", "enum": ["source.read", "reply.propose"]}},
        "required": ["action"]
    })
}

/// The request every forced call is built from asks for one call: its parallel control is
/// `Some(false)`, alongside the named tool choice.
#[test]
fn a_forced_request_asks_for_one_call() {
    let model = Scripted::new(Vec::new());
    let built = request(
        &model,
        "Choose.",
        "text".to_owned(),
        SELECT_TOOL,
        "Choose one.",
        select_schema(),
    )
    .expect("a valid tool name");
    assert!(matches!(built.tool_choice, ToolChoice::Named(_)));
    assert_eq!(
        built.controls.parallel_tool_calls,
        Some(false),
        "story:forced-call-asks-for-one-call: the forced request must ask for one call"
    );
}

/// Both requests a step sends, the selection and the selected action's arguments, reach the
/// model asking for one call.
#[test]
fn the_selection_and_the_arguments_requests_each_ask_for_one_call() {
    let model = Scripted::new(vec![
        vec![json!({"action": "source.read"})],
        vec![json!({"source": "logs", "kind": "get", "input": {"path": "/x"}})],
    ]);
    let selected = select_once(&Brief, &model, "\nCandidates", select_schema())
        .expect("one call is a selection");
    assert_eq!(selected, json!({"action": "source.read"}));
    let arguments = arguments_once(&Brief, &model, "source.read").expect("one call is arguments");
    assert_eq!(arguments["source"], "logs");

    let requests = model.requests();
    assert_eq!(requests.len(), 2);
    for (sent, tool) in requests.iter().zip([SELECT_TOOL, ARGUMENTS_TOOL]) {
        let ToolChoice::Named(name) = &sent.tool_choice else {
            panic!("the request is held to {tool}: {:?}", sent.tool_choice);
        };
        assert_eq!(name.as_str(), tool);
        assert_eq!(
            sent.controls.parallel_tool_calls,
            Some(false),
            "the {tool} request asks for one call"
        );
    }
}

/// A model that calls the tool twice with the same arguments named one choice: it is taken.
#[test]
fn calls_that_carry_the_same_arguments_are_one_answer() {
    let call = json!({"source": "logs", "kind": "get", "input": {"path": "/x"}});
    let model = Scripted::new(vec![vec![call.clone(), call.clone(), call.clone()]]);
    assert_eq!(arguments_once(&Brief, &model, "source.read"), Ok(call));
}

/// Calls whose arguments differ name more than one choice: the answer is unusable, and neither
/// is taken.
#[test]
fn calls_that_differ_are_unusable_and_none_is_taken() {
    let model = Scripted::new(vec![vec![
        json!({"action": "source.read"}),
        json!({"action": "reply.propose"}),
    ]]);
    let refused = select_once(&Brief, &model, "\nCandidates", select_schema())
        .expect_err("two different choices are no selection");
    assert_eq!(
        refused,
        AskError::Unusable(format!(
            "the model called `{SELECT_TOOL}` more than once, with different arguments"
        ))
    );

    let model = Scripted::new(vec![vec![
        json!({"source": "logs", "kind": "get", "input": {"path": "/a"}}),
        json!({"source": "logs", "kind": "get", "input": {"path": "/b"}}),
    ]]);
    assert!(matches!(
        arguments_once(&Brief, &model, "source.read"),
        Err(AskError::Unusable(why)) if why.contains("more than once")
    ));
}
