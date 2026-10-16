//! Acceptance for `story:cli-custom-protocol-commands`: `b10x-loom run --action-bindings <FILE>`
//! drives a protocol installed with `protocols add` through the commands the file binds to its
//! actions (`intake.bindings` in `ess/intake/domains/bindings.yaml`). Each admitted action starts
//! its command with the arguments as JSON on standard input; the command's report on standard
//! output is the action's performed report, and the evidence it states is submitted. An action the
//! file binds no command to ends the run `NoLocalExecutor`, naming the action.
//!
//! Every case runs the binary. The models are the fixture catalog of `catalog_route.rs`
//! (`fixtures/catalog-route/catalog.toml`) at a loopback socket this file opens, replaying
//! responses written here; no case reaches the network, a credential or a Codex login (`HOME` and
//! `CODEX_HOME` are an empty scratch directory). The bound commands are scripts in the scratch
//! directory that log their standard input.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const CATALOG: &str = include_str!("fixtures/catalog-route/catalog.toml");
const INTENT: &str = "record a note that the deploy finished and publish the summary";

/// Two actions: `note.record`, then `summary.publish` once the note is recorded. The case is done
/// when both have evidence.
const PROTOCOL: &str = r"format: protocol/1
protocol:
  id: team.ops
  revision: 1
  description: Record a team note and publish its summary.
artifacts:
  intent:
    description: the request
evidence_kinds:
  note_recorded:
    description: the note was recorded
  summary_published:
    description: the summary was published
claims:
  note.recorded:
    true_when:
      evidence:
        kind: note_recorded
        result: recorded
        subject: intent
  summary.published:
    true_when:
      evidence:
        kind: summary_published
        result: published
        subject: intent
actions:
  note.record:
    description: record the note
    effect: write
    may_produce:
      - evidence: note_recorded
  summary.publish:
    description: publish the summary of the recorded note
    effect: write
    precondition:
      claim: note.recorded
      is: true
    may_produce:
      - evidence: summary_published
outcomes:
  done:
    description: the note is recorded and its summary published
    requires:
      all:
        - claim: note.recorded
        - claim: summary.published
";

#[test]
fn a_custom_protocol_runs_through_declared_commands() {
    let scratch = Scratch::new("bound");
    scratch.install();
    let record = scratch.script(
        "record-note",
        r#"{"note":"n-1","evidence":[{"kind":"note_recorded","result":"recorded"}]}"#,
    );
    let publish = scratch.script(
        "publish-summary",
        r#"{"published":true,"evidence":[{"kind":"summary_published","result":"published"}]}"#,
    );
    let bindings = scratch.bindings(&json!({
        "protocol": "team-ops@1",
        "bindings": [
            {"action": "note.record", "argv": [record, "--team", "ops"]},
            {"action": "summary.publish", "argv": [publish]},
        ],
    }));
    let (base_url, server) = serve(vec![
        classify(),
        select("note.record"),
        arguments(&json!({"text": "deploy finished"})),
        select("summary.publish"),
        arguments(&json!({"channel": "ops"})),
    ]);
    let output = scratch.run(&base_url, &bindings);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stdout}\n{stderr}");
    assert!(stdout.contains("picked team-ops@1"), "{stdout}");
    assert!(stdout.contains("stopped: Completed (done)"), "{stdout}");
    // Each report is the action's performed report, printed as the step's effect.
    assert!(stdout.contains(r#"effect: {"note":"n-1""#), "{stdout}");
    assert!(stdout.contains(r#"effect: {"published":true"#), "{stdout}");
    assert!(
        stdout.contains("evidence: note_recorded recorded"),
        "{stdout}"
    );
    assert!(
        stdout.contains("evidence: summary_published published"),
        "{stdout}"
    );
    // Each command ran once, with its argv as written and the arguments on standard input.
    assert_eq!(
        scratch.calls("record-note"),
        [r#"--team ops|{"text":"deploy finished"}"#]
    );
    assert_eq!(scratch.calls("publish-summary"), [r#"|{"channel":"ops"}"#]);
    assert_eq!(server.join().expect("the server finished").len(), 5);
}

#[test]
fn an_action_with_no_binding_ends_no_local_executor_naming_it() {
    let scratch = Scratch::new("unbound");
    scratch.install();
    let record = scratch.script("record-note", r#"{"note":"n-1"}"#);
    let bindings = scratch.bindings(&json!({
        "protocol": "team-ops@1",
        "bindings": [{"action": "note.record", "argv": [record]}],
    }));
    let (base_url, server) = serve(vec![classify()]);
    let output = scratch.run(&base_url, &bindings);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(3), "{stdout}\n{stderr}");
    assert!(stdout.contains("stopped: NoLocalExecutor"), "{stdout}");
    assert!(stdout.contains("summary.publish"), "{stdout}");
    assert!(scratch.calls("record-note").is_empty());
    assert_eq!(server.join().expect("the server finished").len(), 1);
}

#[test]
fn a_binding_for_an_undeclared_action_is_refused_before_any_model_call() {
    let scratch = Scratch::new("undeclared");
    scratch.install();
    let bindings = scratch.bindings(&json!({
        "protocol": "team-ops@1",
        "bindings": [{"action": "note.erase", "argv": ["/bin/true"]}],
    }));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    let output = scratch.run(&base_url, &bindings);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("note.erase"), "{stderr}");
    assert!(
        listener.accept().is_err(),
        "the model endpoint was contacted"
    );
}

/// A scratch directory under the build's temporary directory, removed on drop.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "custom-protocol-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("home")).unwrap();
        Self { root }
    }

    /// Installs [`PROTOCOL`] as `team-ops@1` in the scratch user's store.
    fn install(&self) {
        let source = self.root.join("team-ops.yaml");
        std::fs::write(&source, PROTOCOL).unwrap();
        let output = self
            .command()
            .args(["protocols", "add", "team-ops@1", "--file"])
            .arg(&source)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// A script that appends `<its arguments>|<its standard input>` as one line to
    /// `<name>.calls` and prints `report`.
    fn script(&self, name: &str, report: &str) -> String {
        let path = self.root.join(name);
        let calls = self.root.join(format!("{name}.calls"));
        let body = format!(
            "#!/bin/sh\nset -e\ninput=$(cat)\nprintf '%s|%s\\n' \"$*\" \"$input\" >> '{}'\nprintf '%s\\n' '{report}'\n",
            calls.display()
        );
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_str().unwrap().to_owned()
    }

    fn calls(&self, name: &str) -> Vec<String> {
        std::fs::read_to_string(self.root.join(format!("{name}.calls")))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn bindings(&self, value: &Value) -> PathBuf {
        let path = self.root.join("action-bindings.json");
        std::fs::write(&path, value.to_string()).unwrap();
        path
    }

    fn command(&self) -> Command {
        let home = self.root.join("home");
        let mut command = Command::new(env!("CARGO_BIN_EXE_b10x-loom"));
        command
            .env("HOME", &home)
            .env("CODEX_HOME", &home)
            .env("XDG_DATA_HOME", &home)
            .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
            .env_remove("RUST_LOG");
        command
    }

    fn run(&self, base_url: &str, bindings: &Path) -> Output {
        let catalog = self.root.join("catalog.toml");
        std::fs::write(&catalog, CATALOG.replace("http://127.0.0.1:1/v1", base_url)).unwrap();
        self.command()
            .args(["run", "--confinement", "none", "--catalog"])
            .arg(&catalog)
            .args([
                "--classifier-model",
                "lab-classifier",
                "--model",
                "lab-agent",
            ])
            .arg("--action-bindings")
            .arg(bindings)
            .arg(INTENT)
            .output()
            .expect("run b10x-loom")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn classify() -> String {
    tool_call(
        "example/Classifier-Model",
        "pick_protocol",
        &json!({"protocol": "team-ops@1", "confidence": 0.99, "reasons": ["a team note"]}),
    )
}

fn select(action: &str) -> String {
    tool_call(
        "example/Agent-Model",
        "select_action",
        &json!({"action": action}),
    )
}

fn arguments(value: &Value) -> String {
    tool_call("example/Agent-Model", "action_arguments", value)
}

/// One streamed Chat Completions answer calling `tool` with `arguments`.
fn tool_call(model: &str, tool: &str, arguments: &Value) -> String {
    let chunk = |choices: Value, usage: Option<Value>| {
        let mut chunk = json!({"id": "chatcmpl-1", "object": "chat.completion.chunk",
            "model": model, "choices": choices});
        if let Some(usage) = usage {
            chunk["usage"] = usage;
        }
        format!("data: {chunk}\n\n")
    };
    [
        chunk(
            json!([{"index": 0, "delta": {"role": "assistant", "tool_calls": [{"index": 0,
                "id": "call-1", "type": "function", "function": {"name": tool, "arguments": ""}}]},
                "finish_reason": null}]),
            None,
        ),
        chunk(
            json!([{"index": 0, "delta": {"tool_calls": [{"index": 0,
                "function": {"arguments": arguments.to_string()}}]}, "finish_reason": null}]),
            None,
        ),
        chunk(
            json!([{"index": 0, "delta": {}, "finish_reason": "tool_calls"}]),
            None,
        ),
        chunk(
            json!([]),
            Some(json!({"prompt_tokens": 120, "completion_tokens": 7, "total_tokens": 127})),
        ),
        "data: [DONE]\n\n".to_owned(),
    ]
    .concat()
}

/// Answers one request per response, in order, on a loopback socket, and returns what each
/// request carried.
fn serve(responses: Vec<String>) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let handle = std::thread::spawn(move || {
        let mut sent = Vec::new();
        for response in responses {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(_) if std::time::Instant::now() < deadline => {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    Err(_) => return sent,
                }
            };
            socket.set_nonblocking(false).unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 8192];
            loop {
                let read = socket.read(&mut buffer).expect("read");
                request.extend_from_slice(&buffer[..read]);
                if read == 0 || complete(&request) {
                    break;
                }
            }
            let mut answer = Vec::from(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            );
            answer.extend_from_slice(response.as_bytes());
            socket.write_all(&answer).expect("written");
            let _ = socket.shutdown(std::net::Shutdown::Both);
            sent.push(String::from_utf8_lossy(&request).into_owned());
        }
        sent
    });
    (base_url, handle)
}

/// Whether `request` holds its whole body, by its `Content-Length`.
fn complete(request: &[u8]) -> bool {
    let text = String::from_utf8_lossy(request);
    text.split_once("\r\n\r\n").is_some_and(|(head, rest)| {
        head.to_ascii_lowercase()
            .split_once("content-length:")
            .and_then(|(_, tail)| tail.split('\r').next())
            .and_then(|value| value.trim().parse::<usize>().ok())
            .is_some_and(|length| rest.len() >= length)
    })
}
