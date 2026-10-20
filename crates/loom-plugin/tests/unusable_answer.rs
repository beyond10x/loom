//! A model that answers a turn's request without a usable call ends the turn `stopped`, asking
//! nothing more, so the host records the item, saves the poll's cursors and handles the next one;
//! only a request the model could not answer is `PluginError::Unavailable`
//! (`story:chooser-executor-for-plugin-turns`, fix round 1). The refusal itself is held by
//! `adversary_w20261020w4_u3.rs`; these cases hold a text-only answer, an argument answer llm
//! refuses, and the request that never got an answer.

mod support;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use b10x_loom_plugin::project::project;
use b10x_loom_plugin::turn::turn;
use b10x_loom_plugin::{Intent, PluginError, RecordOutcome, TurnModel};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, ErrorCode, Item as ModelItem, Model,
    Provenance, StopReason, StreamSink, ToolCall, ToolName, TurnObservation, TurnOutcome,
    TurnRequest,
};
use serde_json::json;
use support::{Fixture, Scripted, Seen, chat, classification, item};

/// One answer of a [`Answers`] model.
enum Reply {
    /// A call of `tool` with these arguments.
    Call(&'static str, serde_json::Value),
    /// Text and no call, the turn ended.
    Text(&'static str),
    /// The request failed with this error, and no answer.
    Fails(Error),
}

/// A turn model that answers each request with the next of its replies, keeping every request.
struct Answers {
    binding: Scripted,
    replies: Mutex<VecDeque<Reply>>,
    seen: Seen,
}

impl Answers {
    fn new(replies: Vec<Reply>) -> (Self, Seen) {
        let seen = Seen::default();
        (
            Self {
                binding: Scripted::new(Vec::new()).0,
                replies: Mutex::new(replies.into()),
                seen: Arc::clone(&seen),
            },
            seen,
        )
    }
}

impl Model for Answers {
    fn provenance(&self) -> &Provenance {
        self.binding.provenance()
    }

    fn capabilities(&self) -> &Capabilities {
        self.binding.capabilities()
    }

    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        _sink: &'a mut dyn StreamSink,
        _cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        self.seen.lock().unwrap().push(request.clone());
        let mut observation = TurnObservation::new(self.provenance().clone());
        observation.final_usage = true;
        let reply = self.replies.lock().unwrap().pop_front();
        let answer = match reply {
            Some(Reply::Call(tool, arguments)) => Ok(TurnOutcome {
                stop_reason: StopReason::ToolCalls,
                items: vec![ModelItem::ToolCall(ToolCall {
                    call_id: CallId::new("call_1").expect("call id"),
                    name: ToolName::new(tool).expect("tool name"),
                    arguments,
                })],
                observation,
            }),
            Some(Reply::Text(text)) => Ok(TurnOutcome {
                stop_reason: StopReason::EndTurn,
                items: vec![ModelItem::AssistantText {
                    text: text.to_owned(),
                }],
                observation,
            }),
            Some(Reply::Fails(error)) => Err(error),
            None => Err(Error::invalid("no further reply")),
        };
        Box::pin(async move { answer })
    }
}

/// The turn on `model`, over the source `chat`, with the projection of an `ask`.
fn run(name: &str, model: Answers) -> Result<b10x_loom_plugin::TurnResult, PluginError> {
    let fixture = Fixture::new("unusable_answer", name);
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let classified = classification(Intent::Ask, &["chat"], "0.9");
    let projection = project(&config, &classified);
    let ended = turn(
        &host,
        TurnModel {
            model: Arc::new(model),
        },
        &item("item-1"),
        &classified,
        &projection,
    );
    assert!(
        fixture.runs("invoke").is_empty(),
        "nothing was read: {ended:?}"
    );
    ended
}

/// A selection answered with text and no call, the turn ended (`EndTurn`), is about the item:
/// the turn ends `stopped`, naming why, and the model is asked nothing more.
#[test]
fn a_text_only_answer_ends_the_turn_stopped() {
    let (model, seen) = Answers::new(vec![
        Reply::Text("I would rather not choose."),
        Reply::Call("select_action", json!({"action": "reply.decline"})),
        Reply::Call("action_arguments", json!({"reason": "unanswered"})),
    ]);

    let ended = run("text_only", model).expect("an unusable answer is no failure of the host");

    assert_eq!(seen.lock().unwrap().len(), 1, "asked once: {ended:?}");
    assert_eq!(ended.outcome, RecordOutcome::Stopped, "{ended:?}");
    assert_eq!(ended.proposal, None, "{ended:?}");
    let detail = ended.detail.unwrap_or_default();
    assert!(
        detail.contains("select_action"),
        "the detail names the request the model answered without its call: {detail}"
    );
}

/// An argument answer llm refuses for the request (a call of a tool it did not publish) is about
/// the item too: the turn ends `stopped`, nothing is performed, and nothing more is asked.
#[test]
fn an_argument_answer_llm_refuses_ends_the_turn_stopped() {
    let (model, seen) = Answers::new(vec![
        Reply::Call("select_action", json!({"action": "source.read"})),
        Reply::Call("reply_send", json!({"text": "Done."})),
        Reply::Call("select_action", json!({"action": "reply.decline"})),
    ]);

    let ended = run("argument_refused", model).expect("an unusable answer is no failure");

    assert_eq!(seen.lock().unwrap().len(), 2, "asked twice: {ended:?}");
    assert_eq!(ended.outcome, RecordOutcome::Stopped, "{ended:?}");
    assert!(ended.reads.is_empty(), "{ended:?}");
}

/// A request the model failed, with no answer, is no answer about the item: the turn is
/// `PluginError::Unavailable`, counted against no item.
#[test]
fn a_request_the_model_did_not_answer_is_unavailable() {
    let (model, seen) = Answers::new(vec![Reply::Fails(Error::new(
        ErrorCode::Unauthorized,
        "the credential was refused",
    ))]);

    let ended = run("not_answered", model);

    assert_eq!(seen.lock().unwrap().len(), 1, "asked once: {ended:?}");
    assert!(
        matches!(ended, Err(PluginError::Unavailable(_))),
        "a model that could not answer counts against no item: {ended:?}"
    );
}
