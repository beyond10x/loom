//! The record a host holds, shared with another thread of its process (`story:a2a-record-and-
//! streaming`): `HeldRecord::of` finds it by its directory, lines appended through it land in the
//! holder's `record.jsonl` beside the holder's own, and the lock stays the holder's, so a second
//! host is still refused while either lives.

use std::fs;
use std::path::PathBuf;

use b10x_loom_plugin::datasource::ConnectorsCliConfig;
use b10x_loom_plugin::state::{HeldRecord, RECORD_FILE, StateDir};
use b10x_loom_plugin::{ItemId, PluginConfig, PluginError, RecordLine, RecordOutcome};

fn config() -> PluginConfig {
    PluginConfig {
        connectors: ConnectorsCliConfig {
            program: "connectors".to_owned(),
            config: None,
            state_dir: None,
            timeout_seconds: None,
        },
        sources: Vec::new(),
        objectives: Vec::new(),
        classify_threshold: None,
        poll_interval_seconds: 0,
        workspace_roots: Vec::new(),
        checkouts: Vec::new(),
    }
}

fn state(test: &str) -> PathBuf {
    let state = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("held-record")
        .join(test);
    let _ = fs::remove_dir_all(&state);
    state
}

fn line(item: &str, detail: &str) -> RecordLine {
    RecordLine {
        item: ItemId(item.to_owned()),
        intent: None,
        confidence: None,
        outcome: RecordOutcome::Stopped,
        reads: Vec::new(),
        proposal: None,
        proposed_case: None,
        detail: Some(detail.to_owned()),
        steps: Vec::new(),
    }
}

#[test]
fn a_held_record_is_shared_within_the_process_and_a_second_host_is_still_refused() {
    let state = state("shared");
    let config = config();
    assert!(HeldRecord::of(&state).is_none(), "nothing is held yet");
    let host = StateDir::open(&state, &config).expect("the first host");
    let shared = HeldRecord::of(&state.join(".")).expect("the host's record, found by its path");
    assert_eq!(shared.path(), state.as_path());

    host.append(&line("C1:1.0", "the host's")).unwrap();
    shared.append(&line("a2a:m-1", "the answer's")).unwrap();
    shared
        .append(&line("a2a:m-1", "a second line for the item"))
        .unwrap();
    let lines = host.record().unwrap();
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert_eq!(
        shared
            .line_of(&ItemId("a2a:m-1".to_owned()))
            .unwrap()
            .and_then(|line| line.detail),
        Some("the answer's".to_owned()),
        "the first line of the item"
    );
    assert!(
        shared
            .line_of(&ItemId("a2a:m-2".to_owned()))
            .unwrap()
            .is_none()
    );
    let text = fs::read_to_string(state.join(RECORD_FILE)).unwrap();
    assert_eq!(text.lines().count(), 3, "{text}");

    assert!(
        matches!(
            StateDir::open(&state, &config),
            Err(PluginError::StateInUse { .. })
        ),
        "a second host is refused while the first holds the record"
    );
    drop(host);
    assert!(
        HeldRecord::of(&state).is_some(),
        "the handle keeps the record held"
    );
    assert!(
        matches!(
            StateDir::open(&state, &config),
            Err(PluginError::StateInUse { .. })
        ),
        "a second host is refused while a handle on the record lives"
    );
    drop(shared);
    assert!(HeldRecord::of(&state).is_none(), "nothing is held any more");
    let next = StateDir::open(&state, &config).expect("the next host");
    assert_eq!(next.record().unwrap().len(), 3);
}
