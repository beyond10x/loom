//! Recorded vendor sessions exercise the CLI host, durable replies, and governed proposals.
use super::{Reaped, a2a_request, free_port, install_program, loom_bin};
use serde_json::{Value, json};
use std::{
    fs,
    process::Stdio,
    time::{Duration, Instant},
};

#[test]
fn served_claude_proposes_without_applying_and_replays_one_record() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let bin = home.join(".local/bin");
    let workspace = root.path().join("workspace");
    let state = root.path().join("state");
    let seen = root.path().join("calls");
    for dir in [&bin, &workspace, &home.join(".claude")] {
        fs::create_dir_all(dir).unwrap();
    }
    fs::write(home.join(".claude/.credentials.json"), "{}").unwrap();
    fs::write(workspace.join("work.txt"), "original\n").unwrap();
    // Use the recorded adapter output, replacing its action and working directory for this host.
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/session/claude-code.jsonl");
    let double = root.path().join("program");
    let script = r#"#!/bin/sh
echo ran >> '@SEEN@'
echo changed > work.txt
/bin/sed -e "s|/scratch/session-1/copy|$PWD|g" -e 's/implementation.edit/session.propose/g' '@FIXTURE@'
"#;
    fs::write(
        &double,
        script
            .replace("@SEEN@", seen.to_str().unwrap())
            .replace("@FIXTURE@", fixture.to_str().unwrap()),
    )
    .unwrap();
    install_program(&double, &bin.join("claude"));
    let port = free_port();
    let spec = root.path().join("review.yaml");
    let source = format!(
        r#"format: loom.agent/1
agent: {{id: review, name: Review, brief: Review the file.}}
model: {{routes: unused.yaml, roles: [{{role: agent, model: a-model}}]}}
session:
  kind: ClaudeCode
  budget: {{max_steps: 1, max_duration_ms: 6000}}
  stop: {{approval_gate: true, on_breach: DiscardStep}}
  confinement: none
  tools: {{allow: [Read, Edit, Write, Glob, Grep], deny: [], decisions: ask}}
states: []
bindings: []
policy: {{capabilities: [session.propose]}}
expose:
  a2a: {{port: {port}, path: /a2a, card: {{name: review, description: Review, skills: [review]}}}}
"#
    );
    fs::write(&spec, &source).unwrap();
    let launch = || {
        let mut cmd = loom_bin::loom();
        cmd.env_clear()
            .env("HOME", &home)
            .env("PATH", &bin)
            .env("LOOM_DASHBOARD_DIR", root.path().join("dashboard"))
            .env("LOOM_A2A_BEARER", "test-agent-token")
            .args(["agent", "serve", "--spec"])
            .arg(&spec)
            .arg("--state")
            .arg(&state)
            .args(["--a2a-address", "127.0.0.1", "--session-workspace"])
            .arg(&workspace)
            .args(["--session-file", "work.txt"])
            .stderr(Stdio::inherit());
        let mut child = Reaped(cmd.spawn().unwrap());
        let until = Instant::now() + Duration::from_secs(4);
        loop {
            if a2a_request(port, "GET", "/ready", None, None).is_some_and(|r| r.0 == 200) {
                break;
            }
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "host exited before readiness"
            );
            assert!(Instant::now() < until, "host did not start");
            std::thread::sleep(Duration::from_millis(15));
        }
        child
    };
    let send = |id: &str| {
        let body = json!({"jsonrpc":"2.0","id":1,"method":"SendMessage","params":{"message":{"messageId":id,"role":"ROLE_USER","parts":[{"text":"Review work.txt"}]}}});
        let (status, raw) =
            a2a_request(port, "POST", "/a2a", Some("test-agent-token"), Some(&body)).unwrap();
        assert_eq!(status, 200, "{raw}");
        serde_json::from_str::<Value>(&raw).unwrap()["result"]["task"].clone()
    };
    let child = launch();
    let first = send("review-1");
    assert_eq!(first["status"]["state"], "TASK_STATE_COMPLETED", "{first}");
    let receipt: Value =
        serde_json::from_str(first["artifacts"][0]["parts"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        receipt["files"],
        json!([{"path":"work.txt","content":"changed\n"}])
    );
    let dashboard = root.path().join("dashboard");
    let addr: std::net::SocketAddr = fs::read_to_string(dashboard.join("address"))
        .unwrap()
        .parse()
        .unwrap();
    let viewer = fs::read_to_string(dashboard.join("viewer-token")).unwrap();
    let (status, raw) =
        a2a_request(addr.port(), "GET", "/api/snapshot", Some(&viewer), None).unwrap();
    assert_eq!(status, 200);
    let observed: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(observed["runs"][0]["input_tokens"], 2500);
    assert_eq!(observed["runs"][0]["output_tokens"], 78);
    assert_eq!(observed["stats"]["cost_micro_usd"], 12300);
    assert!(observed["runs"][0]["model_calls"].is_null());
    assert!(
        !raw.contains("fixed add")
            && !raw.contains("reading the function")
            && !raw.contains("transcript_digest")
    );
    drop(child);
    fs::write(
        &spec,
        source.replace("capabilities: [session.propose]", "capabilities: []"),
    )
    .unwrap();
    let next = launch();
    let replay = send("review-1");
    assert_eq!(
        replay["artifacts"][0]["parts"],
        first["artifacts"][0]["parts"]
    );
    let refused = send("review-denied");
    assert_eq!(refused["status"]["state"], "TASK_STATE_FAILED", "{refused}");
    assert_eq!(fs::read_to_string(&seen).unwrap(), "ran\n");
    assert_eq!(
        fs::read_to_string(workspace.join("work.txt")).unwrap(),
        "original\n"
    );
    drop(next);
    fs::create_dir(home.join(".codex")).unwrap();
    fs::write(home.join(".codex/auth.json"), "{}").unwrap();
    let codex_calls = root.path().join("codex-calls");
    let vendor = include_str!("../fixtures/session/served-codex.sh")
        .replace("@SEEN@", codex_calls.to_str().unwrap());
    fs::write(&double, vendor).unwrap();
    install_program(&double, &bin.join("codex"));
    let codex_spec = source
        .replace("kind: ClaudeCode", "kind: Codex")
        .replace("[Read, Edit, Write, Glob, Grep]", "[Bash, apply_patch]");
    fs::write(&spec, codex_spec).unwrap();
    let _codex = launch();
    let completed = send("codex-no-change");
    assert_eq!(
        completed["status"]["state"], "TASK_STATE_COMPLETED",
        "{completed}"
    );
    let receipt: Value = serde_json::from_str(
        completed["artifacts"][0]["parts"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(receipt["files"], json!([]));
    assert_eq!(receipt["summary"], "Codex fixture reviewed the file");
    assert_eq!(
        send("codex-no-change")["artifacts"][0]["parts"],
        completed["artifacts"][0]["parts"]
    );
    assert_eq!(fs::read_to_string(codex_calls).unwrap(), "ran\n");
    assert_eq!(
        fs::read_to_string(workspace.join("work.txt")).unwrap(),
        "original\n"
    );
}
