//! Adversary cases for `story:plugin-host` (wave 2026-10-09-w2, unit U3), second pass, against the
//! correction of round 1: the model budget across Runs, the record as the truth of handled items,
//! and retried failures. Each case asserts what the crate's own documentation, the ESS domain
//! `loom.plugin` or the coordinator's correction says, and is red against the tree it was written
//! for. `story:chooser-executor-for-plugin-turns` moved the turn to Loom's chooser path over an llm
//! `Model`, and the model-budget case with it.

mod support;

use std::fs;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use b10x_loom_plugin::project::project;
use b10x_loom_plugin::state::{RECORD_FILE, StateDir};
use b10x_loom_plugin::turn::{TURN_MODEL_BUDGET, TURN_STEP_BUDGET, turn};
use b10x_loom_plugin::{
    Cursor, Host, InboundItem, Intent, ItemId, MAX_FAILURES, Objective, Plugin, PluginError,
    PluginState, Poll, RecordLine, RecordOutcome, TurnModel, encode_record_line, run_plugin_on,
};
use llm_core::{
    BoxFuture, Cancel, Capabilities, Dispatch, Error, ErrorCode, Model, Provenance, StreamSink,
    TurnOutcome, TurnRequest,
};
use serde_json::json;
use support::{FakePlugin, Fixture, Recorded, Scripted, Seen, chat, classification, item};

/// A turn model that answers as its `Scripted` model does for its first `answered` requests and
/// fails every later one with a retriable transport error, as a network that went down does.
/// Every request it is sent is kept.
struct Flaky {
    scripted: Scripted,
    answered: usize,
    seen: Seen,
}

impl Model for Flaky {
    fn provenance(&self) -> &Provenance {
        self.scripted.provenance()
    }

    fn capabilities(&self) -> &Capabilities {
        self.scripted.capabilities()
    }

    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        sink: &'a mut dyn StreamSink,
        cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        let sent = {
            let mut seen = self.seen.lock().unwrap();
            seen.push(request.clone());
            seen.len()
        };
        if sent <= self.answered {
            return self.scripted.turn(request, sink, cancel);
        }
        Box::pin(async {
            Err(Error::new(ErrorCode::Transport, "connection reset by peer")
                .with_dispatch(Dispatch::Unknown)
                .with_retriable(true))
        })
    }
}

/// `TURN_MODEL_BUDGET` is "the most requests one turn sends to its model, across every step of its
/// Run, a retried attempt included". Each request is sent with Loom's caller retries, which attempt
/// a request again after a retriable transport error, so a retried attempt is one more request. A
/// model that reads on every step and goes down with two requests of the budget left is attempted
/// twice for the next selection, the second attempt retrying the first; the third attempt is past
/// the budget and is never sent.
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
    let (scripted, _) = Scripted::new(vec![read; TURN_STEP_BUDGET]);
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let budget = usize::try_from(TURN_MODEL_BUDGET).unwrap();
    let flaky = Flaky {
        scripted,
        answered: budget - 2,
        seen: Arc::clone(&seen),
    };

    let ended = turn(
        &host,
        TurnModel {
            model: Arc::new(flaky),
        },
        &item,
        &classified,
        &projection,
    );

    let asked = seen.lock().unwrap().len();
    assert!(
        asked <= budget,
        "the model was sent {asked} requests in one turn, past TURN_MODEL_BUDGET \
         ({TURN_MODEL_BUDGET}); the turn ended {ended:?}"
    );
    assert_eq!(
        asked, budget,
        "the failed selection was attempted again within the budget; the turn ended {ended:?}"
    );
    let ended = ended.expect("a spent budget is no failure of the turn");
    assert_eq!(ended.outcome, RecordOutcome::Stopped, "{ended:?}");
    assert_eq!(
        ended.detail.as_deref(),
        Some(format!("the turn used its {TURN_MODEL_BUDGET} model calls").as_str()),
        "{ended:?}"
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
