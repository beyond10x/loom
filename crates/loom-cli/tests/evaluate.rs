//! `b10x-loom evaluate` reads an evaluation request as JSON and writes Canon's decision as JSON:
//! the decision `loom_governor::evaluate` returns for the same request, and the one
//! `CanonGovernor` reports for the same case after the same evidence (story `governor-evaluate`).
//! It runs with an isolated installation store and no model access.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use b10x_loom_cli::evaluate::{decision_json, request_from_json};
use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, CaseId, CompletionDetermination, EvidenceData, EvidenceId, ObservationId,
};
use b10x_loom_commission::ports::evidence::submit_evidence;
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::{CanonGovernor, MemoryCaseStore};
use loom_protocols::ProtocolCatalog;
use serde_json::{Value, json};

/// Runs `b10x-loom evaluate` with `args`, `stdin` on its standard input.
fn evaluate(root: &Path, args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
        .arg("evaluate")
        .args(args)
        .env("XDG_DATA_HOME", root)
        .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("b10x-loom starts");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("the request is written");
    child.wait_with_output().expect("b10x-loom ends")
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "standard output is one JSON document ({error}): {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

/// The subcommand writes exactly the decision the library returns for the same request, from
/// standard input and from `--input`.
#[test]
fn the_subcommand_writes_the_decision_the_library_returns() {
    let root = tempfile::tempdir().expect("a store root");
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let clock = request(
        "system-query@1",
        snapshot("system.query", &query_revisions()),
        &[clock_record("clock-1")],
        Some("2026-10-08T12:00:00Z"),
    );
    let change = request(
        "software-change@1",
        snapshot("software.change", &change_revisions()),
        &change_records(),
        None,
    );
    for request in [&clock, &change] {
        let text = request.to_string();
        let library = loom_governor::evaluate(
            &catalog,
            &request_from_json(&text).expect("the request decodes"),
        )
        .expect("the library decides");

        let output = evaluate(root.path(), &[], &text);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout.clone()).expect("UTF-8"),
            decision_json(&library),
            "standard input"
        );

        let file = root.path().join("request.json");
        std::fs::write(&file, &text).expect("the request file");
        let from_file = evaluate(root.path(), &["--input", file.to_str().expect("UTF-8")], "");
        assert_eq!(from_file.status.code(), Some(0));
        assert_eq!(from_file.stdout, output.stdout, "--input");
    }

    let decided = stdout_json(&evaluate(root.path(), &[], &clock.to_string()));
    assert_eq!(decided["protocol"], "system-query@1");
    assert_eq!(decided["case"], "case-1");
    assert_eq!(decided["outcome"], "answered");
    assert_eq!(decided["actions"][0]["action"], "system.time.read");
    assert_eq!(decided["actions"][0]["status"], "Admissible");
    assert_eq!(
        decided["claims"][0],
        json!({"claim": "time.observed", "value": "True"})
    );
    assert_eq!(decided["canon"]["format"], "canon-decision/1");
}

/// For a bundled protocol the subcommand reports the action statuses and the completion
/// `CanonGovernor` reports for the same case after the same evidence.
#[test]
fn the_subcommand_decides_what_canon_governor_decides() {
    let root = tempfile::tempdir().expect("a store root");
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    for (protocol, id, revisions, records) in [
        (
            "software-change@1",
            "software.change",
            change_revisions(),
            change_records(),
        ),
        (
            "system-query@1",
            "system.query",
            query_revisions(),
            vec![clock_record("clock-1")],
        ),
    ] {
        let governor = CanonGovernor::new(MemoryCaseStore::default())
            .with_catalog(&catalog)
            .expect("the catalog is admitted");
        let case = CaseId("case-1".into());
        governor
            .open_case(case.clone(), protocol, revisions.clone())
            .expect("the case opens");
        for (n, record) in records.iter().enumerate() {
            submit(&governor, &case, n as u64, record);
        }
        let frontier = governor.frontier(&case).expect("the frontier").into_data();
        let completion = match governor.completion(&case).expect("the completion") {
            CompletionDetermination::Complete(complete) => Value::from(complete.outcome),
            CompletionDetermination::Open(_) => Value::Null,
        };

        let output = evaluate(
            root.path(),
            &[],
            &request(protocol, snapshot(id, &revisions), &records, None).to_string(),
        );
        assert_eq!(output.status.code(), Some(0), "{protocol}");
        let decided = stdout_json(&output);
        let statuses: Vec<(String, String)> = decided["actions"]
            .as_array()
            .expect("actions")
            .iter()
            .map(|action| {
                (
                    action["action"].as_str().expect("action").to_owned(),
                    action["status"].as_str().expect("status").to_owned(),
                )
            })
            .collect();
        let expected: Vec<(String, String)> = frontier
            .actions
            .iter()
            .map(|action| {
                let status = match action.status {
                    ActionStatus::Admissible => "Admissible",
                    ActionStatus::ApprovalRequired => "ApprovalRequired",
                    ActionStatus::Blocked => "Blocked",
                };
                (action.action.clone(), status.to_owned())
            })
            .collect();
        assert_eq!(statuses, expected, "{protocol}: action statuses");
        assert_eq!(
            decided.get("outcome").cloned().unwrap_or(Value::Null),
            completion,
            "{protocol}: completion"
        );
    }
}

/// An unknown protocol, a malformed snapshot, a malformed evidence record and a document that is
/// not a request are each refused with a non-zero exit, naming the input on standard error and in
/// the refusal on standard output.
#[test]
fn each_unusable_input_is_refused_naming_it() {
    let root = tempfile::tempdir().expect("a store root");
    let revisions = query_revisions();
    let good = clock_record("clock-1");
    let cases: Vec<(String, &str, Option<i64>, &str)> = vec![
        (
            request(
                "no-such-protocol@1",
                snapshot("system.query", &revisions),
                std::slice::from_ref(&good),
                None,
            )
            .to_string(),
            "Protocol",
            None,
            "protocol",
        ),
        (
            request(
                "system-query@1",
                json!({"format": "canon-case/1"}),
                std::slice::from_ref(&good),
                None,
            )
            .to_string(),
            "Snapshot",
            None,
            "snapshot",
        ),
        (
            request(
                "system-query@1",
                snapshot("system.query", &revisions),
                &[good.clone(), json!({"format": "canon-evidence/1"})],
                None,
            )
            .to_string(),
            "Evidence",
            Some(1),
            "evidence record 1",
        ),
        ("not json".to_owned(), "Request", None, "request"),
        (
            json!({"protocol": "system-query@1"}).to_string(),
            "Snapshot",
            None,
            "snapshot",
        ),
        (
            json!({"protocol": "system-query@1", "snapshot": snapshot("system.query", &revisions),
                   "evidence": [], "authority": {}})
            .to_string(),
            "Request",
            None,
            "authority",
        ),
    ];
    for (stdin, input, index, named) in cases {
        let output = evaluate(root.path(), &[], &stdin);
        assert_eq!(output.status.code(), Some(3), "{stdin}");
        let refusal = stdout_json(&output);
        assert_eq!(refusal["input"], input, "{stdin}: {refusal}");
        assert_eq!(
            refusal.get("evidence_index").and_then(Value::as_i64),
            index,
            "{stdin}: {refusal}"
        );
        assert!(refusal["code"].is_string() && refusal["message"].is_string());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(named), "{stdin}: {stderr}");
    }

    let missing = evaluate(root.path(), &["--input", "/nonexistent/request.json"], "");
    assert_eq!(missing.status.code(), Some(1));
}

/// `--input` names a regular file or is refused, naming the path, before anything is read: a
/// named pipe nobody writes ends the command with exit status 1 instead of blocking it forever,
/// and so do a directory and an endless device.
#[test]
fn an_input_that_is_not_a_regular_file_is_refused_naming_it() {
    use std::time::{Duration, Instant};
    let root = tempfile::tempdir().expect("a store root");
    let fifo = root.path().join("request.fifo");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo")
            .success()
    );
    let directory = root.path().join("request.d");
    std::fs::create_dir_all(&directory).unwrap();
    for path in [fifo.as_path(), directory.as_path(), Path::new("/dev/zero")] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
            .args(["evaluate", "--input", path.to_str().unwrap()])
            .env("XDG_DATA_HOME", root.path())
            .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("b10x-loom starts");
        let deadline = Instant::now() + Duration::from_secs(10);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "evaluate --input {} still blocked after 10 s",
                    path.display()
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let output = child.wait_with_output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{}: {stderr}",
            path.display()
        );
        assert!(
            stderr.contains(path.to_str().unwrap()) && stderr.contains("not a regular file"),
            "{}: {stderr}",
            path.display()
        );
        assert!(output.stdout.is_empty(), "{}", path.display());
    }
}

/// Submits `record` through Commission's `submit_evidence`, bound to the case's current revision.
fn submit(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId, n: u64, record: &Value) {
    let uuid = |k: u64| Uuid(format!("00000000-0000-4000-8000-{k:012x}"));
    let evidence = EvidenceData {
        evidence_id: EvidenceId(uuid(2 * n + 1)),
        case_id: case.clone(),
        kind: record["kind"].as_str().expect("kind").to_owned(),
        subject_revision: governor.current_revision(case).expect("revision"),
        producer: String::new(),
        observation_ids: vec![ObservationId(uuid(2 * n + 2))],
        facts: commission_json::parse(&record.to_string()).expect("facts are JSON"),
        provenance: commission_json::Value::Object(Vec::new()),
    };
    submit_evidence(governor, "service:ci", evidence).expect("the governor takes the evidence");
}

fn request(protocol: &str, snapshot: Value, records: &[Value], at: Option<&str>) -> Value {
    let mut request = json!({"protocol": protocol, "snapshot": snapshot, "evidence": records});
    if let Some(at) = at {
        request["at"] = Value::from(at);
    }
    request
}

/// The `canon-case/1` snapshot `CanonGovernor` evaluates for a case it opened as `case-1`.
fn snapshot(protocol_id: &str, revisions: &BTreeMap<String, String>) -> Value {
    let artifacts: serde_json::Map<String, Value> = revisions
        .iter()
        .map(|(artifact, revision)| (artifact.clone(), json!({"revision": revision})))
        .collect();
    json!({
        "format": "canon-case/1",
        "id": "case-1",
        "protocol": protocol_id,
        "artifacts": artifacts,
        "revision": "r1",
    })
}

fn query_revisions() -> BTreeMap<String, String> {
    BTreeMap::from([("intent".to_owned(), "query-1".to_owned())])
}

fn change_revisions() -> BTreeMap<String, String> {
    [
        ("intent", "i1"),
        ("system_specification", "s1"),
        ("plan", "p1"),
        ("implementation", "R2"),
        ("release", "v0"),
        ("deployment", "d0"),
    ]
    .into_iter()
    .map(|(artifact, revision)| (artifact.to_owned(), revision.to_owned()))
    .collect()
}

fn change_records() -> Vec<Value> {
    vec![
        record("tests-r2", "test_result", "pass", "implementation", "R2"),
        record(
            "health-d0",
            "operational_observation",
            "healthy",
            "deployment",
            "d0",
        ),
    ]
}

fn clock_record(id: &str) -> Value {
    record(id, "system_time", "observed", "intent", "query-1")
}

fn record(id: &str, kind: &str, result: &str, subject: &str, revision: &str) -> Value {
    json!({
        "format": "canon-evidence/1",
        "id": id,
        "kind": kind,
        "result": result,
        "subject": subject,
        "subject_revision": revision,
    })
}
