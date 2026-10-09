//! What the slack-handler's tests share: the plugin host's test support (the fake `connectors` of
//! `loom-connectors` in a fixture directory, the scripted model port), the Slack and `docs`
//! answers under `tests/fixtures/`, a classifier that answers by the item's text, and a fixed
//! clock. No model, network or credential is reached. Every id is synthetic.
#![allow(dead_code)]

#[path = "../../../loom-plugin/tests/support/mod.rs"]
pub mod host;

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use b10x_loom_plugin::datasource::{
    AdapterAlias, ConnectionId, DataSource, OperationId, SourceName,
};
use b10x_loom_plugin::{Decimal, Objective, PluginConfig, PluginError};
use b10x_loom_plugin_slack::{ChannelObjectives, SlackConfig, SlackHandler, TurnModels};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item as ModelItem, Model, Protocol,
    Provenance, StopReason, StreamSink, ToolCall, ToolName, TurnObservation, TurnOutcome,
    TurnRequest,
};
use serde_json::{Value, json};

pub use host::{Fixture, Scripted};

pub const BOT: &str = "U0BOT";
pub const CHANNEL: &str = "C0FIXTURE1";
pub const LIST: &str = "conversations.list";
pub const HISTORY: &str = "conversations.history";
pub const REPLIES: &str = "conversations.replies";
pub const SEARCH: &str = "docs.search";
pub const PROTOCOL: &str = "software-change@1";

/// The fixed now of every library test: 2023-11-16, a day after the fixture messages.
pub const NOW_SECONDS: i64 = 1_700_100_000;

/// This crate's fixtures.
pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// A fixture directory whose fake answers the three Slack reads and the `docs` search from
/// `tests/fixtures/`.
pub fn fixture(test: &str) -> Fixture {
    let fixture = Fixture::new("slack-handler", test);
    for (verb, operation) in [
        ("describe", LIST),
        ("describe", HISTORY),
        ("describe", REPLIES),
        ("describe", SEARCH),
        ("invoke", LIST),
        ("invoke", HISTORY),
        ("invoke", REPLIES),
        ("invoke", SEARCH),
    ] {
        fs::copy(
            fixtures().join(format!("{verb}-{operation}.json")),
            fixture
                .connectors_state()
                .join(format!("{verb}.{operation}.json")),
        )
        .unwrap();
    }
    fixture
}

/// `operation` of the fake answers `result`, Slack's own response body.
pub fn answer(fixture: &Fixture, operation: &str, adapter: &str, result: Value) {
    let document = json!({
        "ok": true,
        "result": {"adapter": adapter, "operation": operation, "revision": "rev-1", "result": result}
    });
    fs::write(
        fixture
            .connectors_state()
            .join(format!("invoke.{operation}.json")),
        document.to_string(),
    )
    .unwrap();
}

/// The channel list answers `channels`, one page.
pub fn channels(fixture: &Fixture, channels: Value) {
    answer(
        fixture,
        LIST,
        "slack",
        json!({"ok": true, "channels": channels, "response_metadata": {"next_cursor": ""}}),
    );
}

/// A member channel with `members` members.
pub fn member(id: &str, members: i64) -> Value {
    json!({"id": id, "name": id.to_lowercase(), "is_channel": true, "is_member": true,
           "is_archived": false, "num_members": members})
}

/// Every history answers `messages`, given oldest first and answered newest first, as Slack does.
pub fn history(fixture: &Fixture, mut messages: Vec<Value>) {
    messages.reverse();
    answer(
        fixture,
        HISTORY,
        "slack",
        json!({"ok": true, "messages": messages, "has_more": false}),
    );
}

/// Every thread's replies answer `messages`, the parent first.
pub fn replies(fixture: &Fixture, messages: Vec<Value>) {
    answer(
        fixture,
        REPLIES,
        "slack",
        json!({"ok": true, "messages": messages, "has_more": false}),
    );
}

/// A Slack `ts` `minutes` before [`NOW_SECONDS`], with `n` as its microseconds.
pub fn ago(minutes: i64, n: u32) -> String {
    format!("{}.{n:06}", NOW_SECONDS - minutes * 60)
}

/// A message by `user` at `ts`.
pub fn message(user: &str, ts: &str, text: &str) -> Value {
    json!({"type": "message", "user": user, "text": text, "ts": ts})
}

/// The source `docs`: search only.
pub fn docs() -> DataSource {
    DataSource {
        name: SourceName("docs".to_owned()),
        adapter: AdapterAlias("docs".to_owned()),
        connection: ConnectionId("docs-fixture".to_owned()),
        list: None,
        search: Some(OperationId(SEARCH.to_owned())),
        get: None,
    }
}

/// The objectives every configuration weighs.
pub fn objectives() -> Vec<Objective> {
    [
        ("learn about dev stack", "0.2"),
        ("help people with cheap lookups", "0.9"),
        ("respond in slack when being tagged", "0.5"),
    ]
    .into_iter()
    .map(|(name, weight)| Objective {
        name: name.to_owned(),
        weight: Decimal(weight.to_owned()),
    })
    .collect()
}

/// The configuration of `fixture`: the `docs` source, the objectives, a 10-minute age rule, seed 7.
pub fn config(fixture: &Fixture) -> SlackConfig {
    let plugin = PluginConfig {
        objectives: objectives(),
        ..fixture.config(vec![docs()])
    };
    SlackConfig {
        plugin,
        adapter: AdapterAlias("slack".to_owned()),
        connection: ConnectionId("slack-fixture".to_owned()),
        list_channels: OperationId(LIST.to_owned()),
        history: OperationId(HISTORY.to_owned()),
        replies: OperationId(REPLIES.to_owned()),
        bot_user_id: BOT.to_owned(),
        min_age_minutes: 10,
        seed: 7,
        lookback_minutes: None,
        channels: Vec::new(),
    }
}

/// `channel` serves `objectives`.
pub fn serves(channel: &str, objectives: &[&str]) -> ChannelObjectives {
    ChannelObjectives {
        channel: channel.to_owned(),
        objectives: objectives.iter().map(|name| (*name).to_owned()).collect(),
    }
}

/// The plugin over `config` at [`NOW_SECONDS`], with no models.
pub fn handler(config: SlackConfig) -> SlackHandler {
    SlackHandler::new(config)
        .expect("the configuration is accepted")
        .with_clock(Box::new(|| NOW_SECONDS * 1_000_000))
}

/// Turn models taking the scripts in order, one per turn; past them a turn fails.
pub fn turns(scripts: Vec<Vec<(&'static str, Value)>>) -> (TurnModels, Arc<Mutex<usize>>) {
    let scripts = Arc::new(Mutex::new(VecDeque::from(scripts)));
    let made = Arc::new(Mutex::new(0_usize));
    let counted = Arc::clone(&made);
    let models: TurnModels = Box::new(move || {
        *counted.lock().unwrap() += 1;
        let script = scripts
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| PluginError::Turn("no further turn is scripted".to_owned()))?;
        Ok(Scripted::new(script).0.into_turn_model())
    });
    (models, made)
}

/// Every invoke the fake logged, in order: its operation and the input it read on stdin.
pub fn invokes(fixture: &Fixture) -> Vec<(String, Value)> {
    let inputs =
        fs::read_to_string(fixture.connectors_state().join("stdin.log")).unwrap_or_default();
    let operations: Vec<String> = fixture
        .runs("invoke")
        .into_iter()
        .map(|argv| {
            argv.windows(2)
                .find(|w| w[0] == "--operation")
                .map(|w| w[1].clone())
                .unwrap_or_default()
        })
        .collect();
    let inputs: Vec<Value> = inputs
        .lines()
        .map(|line| serde_json::from_str(line).expect("each input is JSON"))
        .collect();
    assert_eq!(operations.len(), inputs.len(), "one input per invoke");
    operations.into_iter().zip(inputs).collect()
}

/// The channels the history reads named, in order.
pub fn history_order(fixture: &Fixture) -> Vec<String> {
    invokes(fixture)
        .into_iter()
        .filter(|(operation, _)| operation == HISTORY)
        .map(|(_, input)| input["channel"].as_str().unwrap_or_default().to_owned())
        .collect()
}

// ---- a classifier answering by the item's text ---------------------------------------------------

/// A recorded classifier: `classify_item` answers the arguments of the first key the item's text
/// contains, `pick_protocol` answers [`PROTOCOL`]. It keeps every request.
pub struct Keyed {
    provenance: Provenance,
    capabilities: Capabilities,
    keys: Vec<(String, Value)>,
    seen: Mutex<Vec<TurnRequest>>,
}

impl Keyed {
    pub fn new(keys: Vec<(&str, Value)>) -> Self {
        let id = |value: &str| Id::new(value).expect("fixture identifier");
        Self {
            provenance: Provenance {
                protocol: Protocol::Responses,
                provider: id("recorded"),
                account: id("recorded"),
                endpoint: id("recorded"),
                model: id("recorded-classifier"),
                binding_revision: id("rev-1"),
            },
            capabilities: Capabilities {
                tools: true,
                tool_choice: true,
                temperature: false,
                top_p: false,
                reasoning_efforts: Vec::new(),
                context_window: 32_768,
                max_output_tokens: 2_048,
            },
            keys: keys
                .into_iter()
                .map(|(key, arguments)| (key.to_owned(), arguments))
                .collect(),
            seen: Mutex::new(Vec::new()),
        }
    }

    pub fn requests(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
}

impl Model for Keyed {
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
        self.seen.lock().unwrap().push(request.clone());
        let tool = request
            .tools
            .first()
            .map(|tool| tool.name.as_str().to_owned())
            .unwrap_or_default();
        let text: String = request
            .items
            .iter()
            .filter_map(|item| match item {
                ModelItem::UserText { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        let arguments = if tool == "pick_protocol" {
            json!({"protocol": PROTOCOL, "confidence": 0.8, "reasons": ["it changes code"]})
        } else {
            self.keys
                .iter()
                .find(|(key, _)| text.contains(key.as_str()))
                .map(|(_, arguments)| arguments.clone())
                .unwrap_or_else(|| panic!("no recorded classification for `{text}`"))
        };
        let mut observation = TurnObservation::new(self.provenance.clone());
        observation.final_usage = true;
        let outcome = TurnOutcome {
            stop_reason: StopReason::ToolCalls,
            items: vec![ModelItem::ToolCall(ToolCall {
                call_id: CallId::new("call_1").expect("call id"),
                name: ToolName::new(&tool).expect("tool name"),
                arguments,
            })],
            observation,
        };
        Box::pin(async move { Ok(outcome) })
    }
}

/// A [`Keyed`] classifier the plugin owns while the test still reads its requests.
pub struct Shared(pub Arc<Keyed>);

impl Model for Shared {
    fn provenance(&self) -> &Provenance {
        self.0.provenance()
    }
    fn capabilities(&self) -> &Capabilities {
        self.0.capabilities()
    }
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        sink: &'a mut dyn StreamSink,
        cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        self.0.turn(request, sink, cancel)
    }
}

/// The fixture run's classifier: the mention is an ask, the question a find, the request a task.
pub fn classifier() -> Keyed {
    Keyed::new(vec![
        (
            "rotate the deploy key",
            json!({"intent": "ask", "hints": ["docs"], "confidence": 0.9}),
        ),
        (
            "runbook for the staging database",
            json!({"intent": "find", "hints": ["docs"], "confidence": 0.85}),
        ),
        (
            "add a retry to the payment webhook",
            json!({"intent": "task", "hints": [], "confidence": 0.8}),
        ),
    ])
}

/// A turn that searches `docs` for `query`, then proposes `text`.
pub fn search_then_propose(query: &str, text: &str) -> Vec<(&'static str, Value)> {
    vec![
        (
            "source_read",
            json!({"source": "docs", "kind": "search", "input": {"query": query}}),
        ),
        ("reply_propose", json!({"text": text})),
    ]
}
