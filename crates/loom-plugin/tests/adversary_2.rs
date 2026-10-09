//! Adversary cases for `story:plugin-host` (wave 2026-10-09-w2, unit U3), second pass, against the
//! correction of round 1: the model budget across Runs, the record as the truth of handled items,
//! and retried failures. Each case asserts what the crate's own documentation, the ESS domain
//! `loom.plugin` or the coordinator's correction says, and is red against the tree it was written
//! for.

mod support;

use std::collections::VecDeque;
use std::fs;
use std::sync::atomic::AtomicBool;

use b10x_loom_plugin::project::project;
use b10x_loom_plugin::state::{RECORD_FILE, StateDir};
use b10x_loom_plugin::turn::{TURN_MODEL_BUDGET, turn};
use b10x_loom_plugin::{
    Cursor, Host, InboundItem, Intent, ItemId, MAX_FAILURES, Objective, Plugin, PluginError,
    PluginState, Poll, RecordLine, RecordOutcome, TurnModel, encode_record_line, run_plugin_on,
};
use llm_core::Model;
use loom_sdk::loom::harness::responses;
use loom_sdk::loom::harness::wire as port;
use serde_json::json;
use support::{FakePlugin, Fixture, MODEL, Recorded, Seen, chat, classification, item};

/// A model port that fails each turn's first `transient` attempts with a retriable transport
/// error, as a flaky network or a provider's 5xx does, and then answers the next call of its
/// script. Every request it is sent is kept.
struct Flaky {
    wire: port::WireId,
    script: VecDeque<(String, serde_json::Value)>,
    transient: usize,
    failed: usize,
    seen: Seen,
}

impl Flaky {
    fn new(script: Vec<(&str, serde_json::Value)>, transient: usize) -> (Self, Seen) {
        let seen = Seen::default();
        (
            Self {
                wire: port::WireId::new(responses::WIRE).expect("valid"),
                script: script
                    .into_iter()
                    .map(|(tool, arguments)| (tool.to_owned(), arguments))
                    .collect(),
                transient,
                failed: 0,
                seen: std::sync::Arc::clone(&seen),
            },
            seen,
        )
    }
}

impl port::ModelPort for Flaky {
    fn wire(&self) -> &port::WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        request: &port::TurnRequest,
        _sink: &mut dyn port::StreamSink,
    ) -> Result<port::TurnOutcome, port::WireError> {
        self.seen.lock().unwrap().push(request.clone());
        if self.failed < self.transient {
            self.failed += 1;
            return Err(port::WireError::transport("connection reset by peer"));
        }
        self.failed = 0;
        let (tool, arguments) = self
            .script
            .pop_front()
            .ok_or_else(|| port::WireError::protocol("the script has no further turn"))?;
        Ok(port::TurnOutcome {
            stop_reason: port::StopReason::ToolCalls,
            items: vec![port::Item::ToolCall(port::ToolCall {
                call_id: port::CallId::new(format!("call_{}", self.seen.lock().unwrap().len()))
                    .expect("valid"),
                name: port::ToolName::new(tool).expect("valid"),
                arguments,
            })],
            usage: None,
        })
    }
}

/// `TURN_MODEL_BUDGET` is "the most model calls one turn makes, across all its Runs and every loop
/// of each". The budget is handed to each loop as `max_turns`, which counts loop turns, while the
/// loop attempts a turn again on a retriable wire error without counting it (`MAX_TURN_RETRIES`).
/// A model that reads on every step over a wire that fails twice before each answer is sent 3, 3
/// and then, with 2 calls left, 3 more requests: 9 in one turn.
#[test]
fn retried_wire_attempts_count_against_the_turn_model_budget() {
    let fixture = Fixture::new("adversary_2", "flaky_wire_budget");
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let item = item("item-1");
    let classified = classification(Intent::Ask, &["chat"], "0.9");
    let projection = project(&config, &classified);
    let read = (
        "source_read",
        json!({"source": "chat", "kind": "list", "input": {"channel": "C0FIXTURE1"}}),
    );
    let (port, seen) = Flaky::new(vec![read; 8], 2);

    let ended = turn(
        &host,
        TurnModel {
            port: Box::new(port),
            model: MODEL.to_owned(),
        },
        &item,
        &classified,
        &projection,
    );

    let asked = seen.lock().unwrap().len();
    assert!(
        u64::try_from(asked).unwrap() <= TURN_MODEL_BUDGET,
        "the model was sent {asked} requests in one turn, past TURN_MODEL_BUDGET \
         ({TURN_MODEL_BUDGET}); the turn ended {ended:?}"
    );
}

/// A plugin whose poll honours its cursor as the ESS declares it ("the opaque value the next poll
/// starts after"): it answers only the items after the cursor of `C0FIXTURE1`, and the cursor
/// after them is the last item's id.
struct Cursored {
    items: Vec<InboundItem>,
    classifier: Recorded,
}

impl Plugin for Cursored {
    fn name(&self) -> &str {
        "cursored"
    }

    fn poll(
        &self,
        _host: &Host<'_>,
        state: &PluginState,
        _objectives: &[Objective],
    ) -> Result<Poll, PluginError> {
        let after = state
            .cursors
            .iter()
            .find(|cursor| cursor.name == "C0FIXTURE1")
            .map(|cursor| cursor.value.clone());
        let start = after
            .and_then(|after| self.items.iter().position(|item| item.id.0 == after))
            .map_or(0, |at| at + 1);
        let items: Vec<InboundItem> = self.items[start..].to_vec();
        let cursors = items
            .last()
            .map(|last| Cursor {
                name: "C0FIXTURE1".to_owned(),
                value: last.id.0.clone(),
            })
            .into_iter()
            .collect();
        Ok(Poll { items, cursors })
    }

    fn classifier(&self) -> Result<&dyn Model, PluginError> {
        Ok(&self.classifier)
    }
}

/// The correction: "A failure is retried, not dropped ... after 3 failed attempts the item is
/// recorded `stopped` and marked handled." The host saves the poll's cursors after every cycle,
/// past the item that failed, so a poll that starts after its cursor never answers that item
/// again: it is tried once, never recorded, and sits in `failing` for good.
#[test]
fn a_failed_item_behind_the_cursor_is_still_retried_until_stopped() {
    let fixture = Fixture::new("adversary_2", "failed_behind_cursor");
    let config = fixture.config(vec![chat()]);
    let plugin = Cursored {
        items: vec![item("item-1")],
        // An intent outside the four: classify fails on every attempt.
        classifier: Recorded::answering(vec![(
            "classify_item",
            json!({"intent": "chat", "hints": [], "confidence": 0.9}),
        )]),
    };
    let host = fixture.host(&config);
    let state = fixture.plugin_state();

    for _ in 0..usize::try_from(MAX_FAILURES).unwrap() + 1 {
        run_plugin_on(&plugin, &host, &state, &AtomicBool::new(true))
            .expect("a failing item is no host error");
    }

    let held = StateDir::open(&state, &config).unwrap();
    let record = held.record().unwrap();
    let current = held.load().unwrap();
    assert_eq!(
        plugin.classifier.requests().len(),
        usize::try_from(MAX_FAILURES).unwrap(),
        "item-1 is classified {MAX_FAILURES} times before it is stopped; the record holds \
         {record:?}, the state {current:?}"
    );
    assert_eq!(record.len(), 1, "item-1 is recorded: {record:?}");
    assert_eq!(record[0].item, ItemId("item-1".to_owned()));
    assert_eq!(record[0].outcome, RecordOutcome::Stopped);
}

/// `StateDir::append` writes one line and its newline; a write the disk could only partly take (a
/// full disk, the condition of pass 1's failed-save case) leaves a torn last line and an error.
/// The record is the truth of handled items, read on every start, so the torn line refuses every
/// later start: the host never runs again until a person repairs the file. The torn item was never
/// saved as handled, so the next run handles it again and the record decodes whole.
#[test]
fn a_torn_last_record_line_does_not_stop_the_host_for_good() {
    let fixture = Fixture::new("adversary_2", "torn_last_line");
    let config = fixture.config(vec![chat()]);
    let state = fixture.plugin_state();
    fs::create_dir_all(&state).unwrap();
    let whole = RecordLine {
        item: ItemId("item-1".to_owned()),
        intent: None,
        confidence: None,
        outcome: RecordOutcome::Unclassified,
        reads: Vec::new(),
        proposal: None,
        proposed_case: None,
        detail: None,
    };
    let torn = RecordLine {
        item: ItemId("item-2".to_owned()),
        ..whole.clone()
    };
    let torn = encode_record_line(&torn);
    fs::write(
        state.join(RECORD_FILE),
        format!(
            "{}\n{}",
            encode_record_line(&whole),
            &torn[..torn.len() / 2]
        ),
    )
    .unwrap();
    let plugin = FakePlugin::new(
        vec![item("item-1"), item("item-2")],
        Recorded::classifying("ask", &["chat"], 0.1),
    );

    let run = run_plugin_on(
        &plugin,
        &fixture.host(&config),
        &state,
        &AtomicBool::new(true),
    );

    assert!(
        run.is_ok(),
        "a torn last line, the trace of an append that failed, refuses the host: {run:?}"
    );
    let record = StateDir::open(&state, &config).unwrap().record().unwrap();
    let ids: Vec<&str> = record.iter().map(|line| line.item.0.as_str()).collect();
    assert_eq!(ids, ["item-1", "item-2"], "each item once: {record:?}");
}
