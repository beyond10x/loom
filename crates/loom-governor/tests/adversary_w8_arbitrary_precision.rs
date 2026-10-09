//! Adversary pass 1 for the engineering-protocols 0.3.0 pin: `serde_json`'s
//! `arbitrary_precision`.
//!
//! engineering-protocols 0.3.0 (`b10x-assertion-providers`, `b10x-canon-engineering-assertions`)
//! and Canon 0.1.0 (`b10x-canon-expr`) turn the feature on. Cargo unifies features, so every
//! build that links `b10x-loom-governor` beside `b10x-loom-executor` (the CLI, the SDK, this test
//! binary through the governor's dev-dependency) builds the executor's serde types against a
//! `serde_json` that has it, while `cargo test -p b10x-loom-executor` alone does not.
//!
//! Under the feature `serde_json` hands every number to a visitor as a private one-entry map
//! (`{"$serde_json::private::Number": "…"}`) whenever serde buffers the input, which it does for
//! every internally tagged enum (`#[serde(tag = "kind")]`) and every `#[serde(flatten)]`. A
//! `u64` field inside one then reads a map and refuses it. From a `Value` it hands an integer
//! above `u64::MAX` to `visit_u128`, which serde's buffer does not implement.
//!
//! Each case writes a value the executor produced (or a model could), reads it back the way a
//! consumer does, and asserts it survives.

use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::turn_loop::{
    LoopEvent, LoopStop, LoopStopContextAboveTrigger, LoopStopDeadline, LoopStopMaxCost,
    LoopStopMaxInputTokens, LoopStopMaxOutputTokens, LoopStopMaxTurns, LoopStopUnstructured,
};
use b10x_loom_executor::harness::wire::{CallId, Item, ToolCall, ToolName};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{CommissionRunId, RunEnding, SessionData, SessionId};
use b10x_loom_executor::session::SessionFile;
use serde_json::Value;

/// Numbers a model puts in tool-call arguments: an integer, a negative, the largest `u64`, one
/// past it, a decimal and an exponent form.
const ARGUMENTS: &str = r#"{"int":7,"neg":-42,"max":18446744073709551615,"big":18446744073709551616,"dec":0.1,"exp":1e3}"#;

fn arguments() -> Value {
    serde_json::from_str(ARGUMENTS).expect("the arguments are JSON")
}

fn call(arguments: Value) -> ToolCall {
    ToolCall {
        call_id: CallId::new("call-1").expect("valid"),
        name: ToolName::new("fs.write").expect("valid"),
        arguments,
    }
}

fn assert_numbers(arguments: &Value) {
    assert_eq!(arguments["int"].as_u64(), Some(7), "{arguments}");
    assert_eq!(arguments["neg"].as_i64(), Some(-42), "{arguments}");
    assert_eq!(arguments["max"].as_u64(), Some(u64::MAX), "{arguments}");
    assert_eq!(arguments["dec"].as_f64(), Some(0.1), "{arguments}");
    assert_eq!(arguments["exp"].as_f64(), Some(1000.0), "{arguments}");
    assert!(arguments["big"].is_number(), "{arguments}");
}

/// `ToolRequested` flattens its `ToolCall` into an internally tagged event (`event.rs:287`): the
/// JSONL record a run writes must read back with its arguments intact.
#[test]
fn a_tool_request_read_back_from_the_record_keeps_its_numeric_arguments() {
    let event = LoopEvent::ToolRequested {
        call: call(arguments()),
        operation: None,
        subjects: Vec::new(),
    };
    let line = serde_json::to_string(&event).expect("serializes");
    let read: LoopEvent = serde_json::from_str(&line).expect("the record line reads back");
    assert_eq!(read, event, "{line}");
    let LoopEvent::ToolRequested { call, .. } = read else {
        panic!("not a tool request: {line}");
    };
    assert_numbers(&call.arguments);
}

/// The record is JSONL a consumer reads line by line: every event the loop writes with an integer
/// in it must read back from its own line. `TurnRetried { turn: u64 }` sits in the internally
/// tagged `LoopEvent`.
#[test]
fn a_retried_turn_read_back_from_the_record_keeps_its_turn() {
    let event = LoopEvent::TurnRetried {
        turn: 3,
        attempt: 1,
        reason: "the stream broke".to_owned(),
    };
    let line = serde_json::to_string(&event).expect("serializes");
    let read = serde_json::from_str::<LoopEvent>(&line);
    assert_eq!(
        read.as_ref().ok(),
        Some(&event),
        "{line} reads back as {read:?}"
    );
}

/// `Cost { micro_usd: u64 }`: the figure a priced run reports per turn.
#[test]
fn a_cost_read_back_from_the_record_keeps_its_figure() {
    let event = LoopEvent::Cost {
        model: "gpt-5".to_owned(),
        micro_usd: 1_234,
    };
    let line = serde_json::to_string(&event).expect("serializes");
    let read = serde_json::from_str::<LoopEvent>(&line);
    assert_eq!(
        read.as_ref().ok(),
        Some(&event),
        "{line} reads back as {read:?}"
    );
}

/// A `LoopStop` is written inside an internally tagged event with integer figures; it is how a run
/// says why it ended. Its JSON is the executor's own codec, which reads every figure as a JSON
/// number: each cause with a figure, carried by both events that hold a stop, must read back.
#[test]
fn a_stop_read_back_from_json_text_keeps_its_figures() {
    let stops = [
        LoopStop::MaxTurns(LoopStopMaxTurns { limit: 20 }),
        LoopStop::MaxInputTokens(LoopStopMaxInputTokens {
            limit: 1000,
            reported: 1200,
        }),
        LoopStop::MaxOutputTokens(LoopStopMaxOutputTokens {
            limit: 500,
            reported: 512,
        }),
        LoopStop::MaxCost(LoopStopMaxCost {
            limit_micro_usd: 250_000,
            spent_micro_usd: i64::MAX,
        }),
        LoopStop::Deadline(LoopStopDeadline { limit_ms: 60_000 }),
        LoopStop::Unstructured(LoopStopUnstructured { asked_again: 2 }),
        LoopStop::ContextAboveTrigger(LoopStopContextAboveTrigger {
            window: 200_000,
            target: 100_000,
            occupied: 190_000,
        }),
    ];
    for stop in stops {
        for event in [
            LoopEvent::Finished {
                stop: stop.clone(),
                turns: 3,
            },
            LoopEvent::DelegateFinished {
                call_id: CallId::new("call-1").expect("valid"),
                stop: stop.clone(),
                turns: 3,
            },
        ] {
            let text = serde_json::to_string(&event).expect("serializes");
            let read = serde_json::from_str::<LoopEvent>(&text);
            assert_eq!(
                read.as_ref().ok(),
                Some(&event),
                "{text} reads back as {read:?}"
            );
        }
    }
}

/// The conversation as JSON text: `Item` is internally tagged and its tool calls and results carry
/// `Value`s.
#[test]
fn items_read_back_from_json_text_keep_their_numeric_arguments_and_outputs() {
    let items = vec![
        Item::ToolCall(call(arguments())),
        Item::ToolResult {
            call_id: CallId::new("call-1").expect("valid"),
            output: arguments(),
            failed: false,
        },
    ];
    let text = serde_json::to_string(&items).expect("serializes");
    let read: Vec<Item> = serde_json::from_str(&text).expect("the items read back");
    assert_eq!(read, items, "{text}");
    let Item::ToolCall(read_call) = &read[0] else {
        panic!("not a tool call: {text}");
    };
    assert_numbers(&read_call.arguments);
}

/// A filed session holds the conversation verbatim; `SessionFile::parse` reads only the version
/// through a `Value` and reads the session itself from the text. A tool call whose arguments hold
/// an integer one past `u64::MAX` must not make the session unreadable.
#[test]
fn a_filed_session_whose_tool_call_holds_a_large_integer_loads() {
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adversary_w8_ap_{}", std::process::id()));
    let workspace = root.join("workspace");
    let sessions = root.join("sessions");
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::create_dir_all(&sessions).expect("sessions");

    let data = SessionData {
        session_id: SessionId(Uuid("0190a5b2-0000-7000-8000-00000000a8a1".to_owned())),
        commission_run: CommissionRunId(Uuid("0190a5b2-0000-7000-8000-00000000a8a2".to_owned())),
        wire: responses::WIRE.to_owned(),
    };
    let mut session =
        SessionFile::open(&data, "gpt-5", "https://example.invalid", &workspace).expect("opens");
    session.items = vec![Item::ToolCall(call(arguments()))];
    session.file(&sessions, RunEnding::Answered).expect("files");

    let loaded = SessionFile::load(&sessions, &data.session_id);
    let _ = std::fs::remove_dir_all(&root);
    let loaded = loaded.expect("the filed session loads");
    assert_eq!(loaded.items, vec![Item::ToolCall(call(arguments()))]);
}
