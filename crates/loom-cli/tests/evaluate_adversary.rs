//! Adversarial cases for `b10x-loom evaluate`: the 16 MiB bound exactly at and one over, a stream
//! past it, empty and non-UTF-8 input, nesting past the JSON depth bound, and stdout/stderr
//! separation for each exit status `evaluate --help` documents.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

const LIMIT: usize = 16 * 1024 * 1024;

fn evaluate(root: &Path, stdin: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
        .arg("evaluate")
        .env("XDG_DATA_HOME", root)
        .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("b10x-loom starts");
    let mut input = child.stdin.take().expect("stdin");
    let bytes = stdin.to_vec();
    let writer = std::thread::spawn(move || {
        // A process that stops reading at its bound closes the pipe; that is not a test failure.
        let _ = input.write_all(&bytes);
    });
    let output = child.wait_with_output().expect("b10x-loom ends");
    writer.join().expect("the writer ends");
    output
}

fn valid_request() -> String {
    json!({
        "protocol": "system-query@1",
        "snapshot": {
            "format": "canon-case/1", "id": "case-1", "protocol": "system.query",
            "artifacts": {"intent": {"revision": "query-1"}}, "revision": "r1"
        },
        "evidence": [{
            "format": "canon-evidence/1", "id": "clock-1", "kind": "system_time",
            "result": "observed", "subject": "intent", "subject_revision": "query-1"
        }]
    })
    .to_string()
}

fn padded(len: usize) -> Vec<u8> {
    let mut text = valid_request().into_bytes();
    text.resize(len, b' ');
    text
}

#[test]
fn adversary_a_request_of_exactly_16_mib_is_decided() {
    let root = tempfile::tempdir().expect("root");
    let output = evaluate(root.path(), &padded(LIMIT));
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decision: Value = serde_json::from_slice(&output.stdout).expect("one JSON decision");
    assert_eq!(decision["outcome"], "answered");
    assert!(
        output.stderr.is_empty(),
        "a decision writes nothing on stderr"
    );
}

#[test]
fn adversary_one_byte_over_16_mib_and_a_longer_stream_exit_1_with_nothing_on_stdout() {
    let root = tempfile::tempdir().expect("root");
    for len in [LIMIT + 1, 2 * LIMIT] {
        let output = evaluate(root.path(), &padded(len));
        assert_eq!(output.status.code(), Some(1), "{len} bytes");
        assert!(
            output.stdout.is_empty(),
            "{len} bytes: stdout {:?}",
            output.stdout.len()
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("16 MiB"));
    }
}

#[test]
fn adversary_non_utf8_exits_1_and_empty_input_is_a_request_refusal() {
    let root = tempfile::tempdir().expect("root");
    let output = evaluate(root.path(), b"{\"protocol\": \"\xff\"}");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());

    let output = evaluate(root.path(), b"");
    assert_eq!(output.status.code(), Some(3));
    let refusal: Value = serde_json::from_slice(&output.stdout).expect("one JSON refusal");
    assert_eq!(refusal["input"], "Request");
    assert!(String::from_utf8_lossy(&output.stderr).contains("request"));
}

#[test]
fn adversary_nesting_past_the_depth_bound_is_refused_not_crashed() {
    let root = tempfile::tempdir().expect("root");
    for depth in [129usize, 100_000] {
        let deep = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
        let text = valid_request().replacen("\"evidence\":[", &format!("\"evidence\":[{deep},"), 1);
        let output = evaluate(root.path(), text.as_bytes());
        assert_eq!(output.status.code(), Some(3), "depth {depth}");
        let refusal: Value = serde_json::from_slice(&output.stdout).expect("one JSON refusal");
        assert_eq!(refusal["code"], "malformed", "depth {depth}");
    }
}
