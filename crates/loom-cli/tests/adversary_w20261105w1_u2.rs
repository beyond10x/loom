//! Adversary cases for `story:agent-run-without-loops` (wave 2026-11-05-w1, unit 2), through the
//! binary.
//!
//! - A spec with no `loops` in `writes.mode: approval`, its channel binding a full Slack binding
//!   with a `write`: `agent run` proposes, holds nothing and sends nothing; `agent resume` on the
//!   same state refuses the spec before anything is read.
//! - A spec with no `loops` that only answers A2A (`expose.a2a`, no `channel`): `agent serve`
//!   without `--once` refuses it before any model call, state directory or listener.
//!
//! Models are recorded Chat Completions streams on a loopback socket this file opens; reads and
//! writes go to the fake `connectors` of `loom-connectors`. No case reaches the network.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const CATALOG: &str = include_str!("fixtures/catalog-route/catalog.toml");
const CLASSIFIER: &str = "example/Classifier-Model";
const AGENT: &str = "example/Agent-Model";
const CHANNEL: &str = "C0FIXTURE1";
const WRITE: &str = "message.post";
const TASK: &str = "Summarise what the runbooks say about the deploy key.";
const REPLY: &str = "The runbook on rotating the deploy key is the one to follow.";

struct Scratch {
    root: PathBuf,
}

impl Scratch {
    /// A scratch root with the fake `connectors`, its fixtures and a spec with no `loops`;
    /// `a2a_port` declares `expose.a2a` and no `channel`, else the channel is a full Slack binding
    /// with a `write` in `writes.mode: approval`.
    fn new(label: &str, a2a_port: Option<u16>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "adversary-u2-{label}-{}-{}",
            std::process::id(),
            now.as_nanos()
        ));
        for directory in ["home", "connectors-state"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let fake = root.join("connectors");
        install_program(
            &crates.join("loom-connectors/tests/fixtures/connectors/fake-connectors"),
            &fake,
        );
        let connectors_state = root.join("connectors-state");
        let fixtures = crates.join("loom-plugin-slack/tests/fixtures");
        for verb in ["describe", "invoke"] {
            fs::copy(
                fixtures.join(format!("{verb}-docs.search.json")),
                connectors_state.join(format!("{verb}.docs.search.json")),
            )
            .unwrap();
        }
        let writes = crates.join("loom-connectors/tests/fixtures/connectors");
        let describe = fs::read_to_string(writes.join("describe-write.json")).unwrap();
        fs::write(
            connectors_state.join(format!("describe.{WRITE}.json")),
            describe.replace("\"adapter\":\"chat\"", "\"adapter\":\"slack\""),
        )
        .unwrap();
        fs::copy(
            writes.join("invoke-route-applied.json"),
            connectors_state.join(format!("invoke.{WRITE}.json")),
        )
        .unwrap();
        let scratch = Self { root };
        let (chat, channel, writes, caps, expose) = match a2a_port {
            None => (
                format!(
                    "  - {{name: chat, kind: connectors, adapter: slack, connection: slack-fixture, list: conversations.list, get: conversations.history, search: conversations.replies, write: {WRITE}}}\n"
                ),
                format!("channel: {{binding: chat, allow: [{CHANNEL}], bot_user_id: U0BOT}}\n"),
                "writes: {mode: approval, max_per_hour: 5}\n".to_owned(),
                "[datasource.read, reply.propose, reply.send]",
                String::new(),
            ),
            Some(port) => (
                String::new(),
                String::new(),
                String::new(),
                "[datasource.read, reply.propose]",
                format!(
                    "expose:\n  a2a: {{port: {port}, path: /a2a, card: {{name: adv-operator, description: Answers about the deploy, skills: [logs]}}}}\n"
                ),
            ),
        };
        let text = format!(
            "format: loom.agent/1
agent: {{id: adv-operator, name: Adversary operator, brief: Answer questions about the deploy.}}
model:
  routes: routes.yaml
  roles: [{{role: agent, model: small}}, {{role: classifier, model: small}}]
states:
  - {{name: answer, when: {{intent: Ask}}, actions: [source.read, reply.propose, reply.decline], sources: [docs]}}
  - {{name: hand-off, when: {{intent: Task}}, actions: [reply.decline], sources: []}}
bindings:
{chat}  - {{name: docs, kind: connectors, adapter: docs, connection: docs-fixture, search: docs.search}}
{channel}{writes}policy: {{capabilities: {caps}}}
{expose}runtime:
  state_dir: {state}
  connectors: {{program: {program}, state_dir: {connectors}}}
  confinement: none
",
            state = scratch.state().display(),
            program = fake.display(),
            connectors = connectors_state.display(),
        );
        fs::write(scratch.spec(), text).unwrap();
        scratch
    }

    fn spec(&self) -> PathBuf {
        self.root.join("agent.yaml")
    }

    fn state(&self) -> PathBuf {
        self.root.join("agent-state")
    }

    fn catalog(&self, base_url: &str) -> PathBuf {
        let catalog = self.root.join("catalog.toml");
        fs::write(&catalog, CATALOG.replace("http://127.0.0.1:1/v1", base_url)).unwrap();
        catalog
    }

    fn command(&self) -> Command {
        let home = self.root.join("home");
        let mut command = Command::new(env!("CARGO_BIN_EXE_b10x-loom"));
        command
            .env("HOME", &home)
            .env("CODEX_HOME", &home)
            .env("XDG_DATA_HOME", &home)
            .env_remove("RUST_LOG")
            .env_remove("LOOM_HTTP_CA_FILE");
        command
    }

    fn models(command: &mut Command, catalog: &Path) {
        command.arg("--catalog").arg(catalog).args([
            "--classifier-model",
            "lab-classifier",
            "--model",
            "lab-agent",
        ]);
    }

    fn argv(&self) -> Vec<Vec<String>> {
        fs::read_to_string(self.root.join("connectors-state/argv.log"))
            .unwrap_or_default()
            .lines()
            .map(|line| {
                line.trim_end_matches('\t')
                    .split('\t')
                    .map(str::to_owned)
                    .collect()
            })
            .collect()
    }

    fn invoked(&self, operation: &str) -> usize {
        self.argv()
            .into_iter()
            .filter(|argv| {
                argv.iter()
                    .position(|argument| argument == "invoke")
                    .filter(|at| *at == 0 || argv[at - 1] != "operations")
                    .and_then(|at| argv.get(at + 2))
                    .is_some_and(|invoked| invoked == operation)
            })
            .count()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn texts(output: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Green: `writes.mode: approval` and a channel binding that could poll and write do not make a
/// run of a spec with no `loops` hold or send: the turn ends on its proposal, exit 0, no
/// `approvals/`, no grant, no `sends.jsonl`, no write invoked. `agent resume` on that state
/// refuses the spec (exit 1, naming the loops) before it reads anything.
#[test]
fn approval_mode_with_no_loops_holds_nothing_and_resume_refuses() {
    let scratch = Scratch::new("approval", None);
    let (base_url, server) = serve(answered());
    let catalog = scratch.catalog(&base_url);
    let input = scratch.root.join("task.txt");
    fs::write(&input, TASK).unwrap();

    let mut command = scratch.command();
    command
        .args(["agent", "run", "--spec"])
        .arg(scratch.spec())
        .arg("--state")
        .arg(scratch.state())
        .arg("--input")
        .arg(&input);
    Scratch::models(&mut command, &catalog);
    let output = command.output().expect("run b10x-loom");

    let (stdout, stderr) = texts(&output);
    assert_eq!(output.status.code(), Some(0), "{stdout}\n{stderr}");
    assert!(stdout.contains(REPLY), "{stdout}");
    assert_eq!(server.join().expect("the server finished").len(), 5);
    assert!(!scratch.state().join("approvals").exists(), "nothing held");
    assert!(
        !scratch.state().join("sends.jsonl").exists(),
        "no send counted"
    );
    assert_eq!(scratch.invoked(WRITE), 0, "nothing sent");
    for polled in [
        "conversations.list",
        "conversations.history",
        "conversations.replies",
    ] {
        assert_eq!(scratch.invoked(polled), 0, "{polled}: a run polls nothing");
    }

    let reads = scratch.argv().len();
    let resumed = scratch
        .command()
        .args(["agent", "resume", "--spec"])
        .arg(scratch.spec())
        .arg("--state")
        .arg(scratch.state())
        .arg("00000000-0000-4000-8000-000000000001")
        .output()
        .expect("run b10x-loom");
    let (stdout, stderr) = texts(&resumed);
    assert_eq!(resumed.status.code(), Some(1), "{stdout}\n{stderr}");
    assert!(stderr.contains("`loops` is undeclared"), "{stderr}");
    assert_eq!(scratch.argv().len(), reads, "resume read nothing");
    assert_eq!(scratch.invoked(WRITE), 0, "nothing sent");
}

/// Green: an A2A-only spec with no `loops` is refused by `agent serve` without `--once` (which
/// would otherwise listen until a signal) with exit 1 naming the loops, before any model call,
/// any state directory or any listener on its port.
#[test]
fn an_a2a_only_spec_with_no_loops_is_refused_by_serve_before_it_listens() {
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let scratch = Scratch::new("a2a-only", Some(port));
    let silent = TcpListener::bind("127.0.0.1:0").unwrap();
    silent.set_nonblocking(true).unwrap();
    let catalog = scratch.catalog(&format!("http://{}/v1", silent.local_addr().unwrap()));

    let mut command = scratch.command();
    command
        .args(["agent", "serve", "--spec"])
        .arg(scratch.spec())
        .arg("--state")
        .arg(scratch.state())
        .args(["--a2a-address", "127.0.0.1"])
        .env("LOOM_A2A_BEARER", "adversary-u2-bearer-not-a-secret")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Scratch::models(&mut command, &catalog);
    let mut child = command.spawn().expect("run b10x-loom");
    let deadline = Instant::now() + Duration::from_secs(30);
    let listened = loop {
        if child.try_wait().unwrap().is_some() {
            break false;
        }
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() || Instant::now() > deadline {
            let _ = child.kill();
            break true;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let output = child.wait_with_output().unwrap();
    let (stdout, stderr) = texts(&output);
    assert!(!listened, "the host listened or kept running: {stderr}");
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    assert!(stderr.contains("`loops` is undeclared"), "{stderr}");
    assert!(!scratch.state().exists(), "no state directory");
    assert!(silent.accept().is_err(), "no model was called");
    assert!(scratch.argv().is_empty(), "nothing was read");
}

// ---------------------------------------------------------------------------------------------
// Recorded models.

fn answered() -> Vec<Vec<u8>> {
    let select = |action: &str| sse(AGENT, "select_action", json!({ "action": action }));
    vec![
        sse(
            CLASSIFIER,
            "classify_item",
            json!({"intent": "ask", "hints": ["docs"], "confidence": 0.9}),
        ),
        select("source.read"),
        sse(
            AGENT,
            "action_arguments",
            json!({"source": "docs", "kind": "search", "input": {"query": "deploy key"}}),
        ),
        select("reply.propose"),
        sse(AGENT, "action_arguments", json!({"text": REPLY})),
    ]
}

fn sse(model: &str, tool: &str, arguments: Value) -> Vec<u8> {
    let chunk = |delta: Value, finish: Value| {
        json!({"id": format!("chatcmpl-{tool}"), "object": "chat.completion.chunk", "model": model,
               "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]})
    };
    let events = [
        chunk(
            json!({"role": "assistant", "tool_calls": [{"index": 0, "id": format!("call-{tool}"),
                   "type": "function", "function": {"name": tool, "arguments": ""}}]}),
            Value::Null,
        ),
        chunk(
            json!({"tool_calls": [{"index": 0, "function": {"arguments": arguments.to_string()}}]}),
            Value::Null,
        ),
        chunk(json!({}), json!("tool_calls")),
        json!({"id": format!("chatcmpl-{tool}"), "object": "chat.completion.chunk", "model": model,
               "choices": [], "usage": {"prompt_tokens": 120, "completion_tokens": 7, "total_tokens": 127}}),
    ];
    let mut stream = String::new();
    for event in events {
        stream.push_str(&format!("data: {event}\n\n"));
    }
    stream.push_str("data: [DONE]\n\n");
    stream.into_bytes()
}

fn serve(responses: Vec<Vec<u8>>) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let mut sent = Vec::new();
        for response in responses {
            let (mut socket, _) = listener.accept().expect("accepted");
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
            answer.extend_from_slice(&response);
            socket.write_all(&answer).expect("written");
            let _ = socket.shutdown(std::net::Shutdown::Both);
            sent.push(String::from_utf8_lossy(&request).into_owned());
        }
        sent
    });
    (base_url, handle)
}

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

fn install_program(source: &Path, target: &Path) {
    let status = Command::new("/usr/bin/install")
        .args(["-m", "755"])
        .arg(source)
        .arg(target)
        .status()
        .unwrap();
    assert!(status.success(), "install {}: {status}", target.display());
}
