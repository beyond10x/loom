//! Adversary pass 1 for the engineering-protocols 0.3.0 pin: the control for
//! `crates/loom-governor/tests/adversary_w8_arbitrary_precision.rs`.
//!
//! The same filed session, in the build `cargo test -p b10x-loom-executor` makes, where nothing
//! turns on `serde_json`'s `arbitrary_precision`. Here the session loads; in a build that links
//! `b10x-loom-governor` (engineering-protocols 0.3.0 and Canon 0.1.0 turn the feature on) the same
//! file is refused.

use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::wire::{CallId, Item, ToolCall, ToolName};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{CommissionRunId, RunEnding, SessionData, SessionId};
use b10x_loom_executor::session::SessionFile;
use serde_json::Value;

const ARGUMENTS: &str = r#"{"int":7,"max":18446744073709551615,"big":18446744073709551616}"#;

fn call() -> ToolCall {
    ToolCall {
        call_id: CallId::new("call-1").expect("valid"),
        name: ToolName::new("fs.write").expect("valid"),
        arguments: serde_json::from_str::<Value>(ARGUMENTS).expect("the arguments are JSON"),
    }
}

#[test]
fn a_filed_session_whose_tool_call_holds_a_large_integer_loads() {
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adversary_w8_ap_exec_{}", std::process::id()));
    let workspace = root.join("workspace");
    let sessions = root.join("sessions");
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::create_dir_all(&sessions).expect("sessions");

    let data = SessionData {
        session_id: SessionId(Uuid("0190a5b2-0000-7000-8000-00000000a8b1".to_owned())),
        commission_run: CommissionRunId(Uuid("0190a5b2-0000-7000-8000-00000000a8b2".to_owned())),
        wire: responses::WIRE.to_owned(),
        boundary_refusals: 0,
    };
    let mut session =
        SessionFile::open(&data, "gpt-5", "https://example.invalid", &workspace).expect("opens");
    session.items = vec![Item::ToolCall(call())];
    session.file(&sessions, RunEnding::Answered).expect("files");

    let loaded = SessionFile::load(&sessions, &data.session_id);
    let _ = std::fs::remove_dir_all(&root);
    let loaded = loaded.expect("the filed session loads");
    assert_eq!(loaded.items, vec![Item::ToolCall(call())]);
}
