//! Acceptance for `story:agent-serve`, the command line: `b10x-loom agent serve --spec <FILE>
//! --state <DIR>` hosts the agent the spec declares, polls only the channels `channel.allow`
//! lists, answers an item in a governed turn on the spec's protocol, holds its `reply.send` for a
//! person's approval in `writes.mode: approval`, and ends after the item it is handling on
//! SIGTERM. `b10x-loom approve <COMMISSION> reply.send --state <DIR>` grants the held send and
//! `agent resume` performs it, once.
//!
//! Every case runs the binary. The channel and `docs` reads and the write go to the fake
//! `connectors` of `loom-connectors`, answering from `loom-plugin-slack`'s and `loom-connectors`'
//! fixtures (one channel, one question an hour old). The models are the fixture catalog's
//! anonymous Chat Completions routes (`fixtures/catalog-route/catalog.toml`) at a loopback socket
//! this file opens, which replays recorded responses in order. No case reaches the network, a
//! credential or a Codex login.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const CATALOG: &str = include_str!("fixtures/catalog-route/catalog.toml");
const CLASSIFIER: &str = "example/Classifier-Model";
const AGENT: &str = "example/Agent-Model";
const CHANNEL: &str = "C0FIXTURE1";
const WRITE: &str = "message.post";
const REPLY: &str = "The deploy finished at 10:02; the logs show no error.";

/// The responses of one item asked and answered: classified, one read, the proposal, and the
/// send the host holds. Nothing is asked after the hold.
fn recorded(ts: &str) -> Vec<Vec<u8>> {
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
            json!({"source": "docs", "kind": "search", "input": {"query": "deploy"}}),
        ),
        select("reply.propose"),
        sse(AGENT, "action_arguments", json!({"text": REPLY})),
        select("reply.send"),
        sse(
            AGENT,
            "action_arguments",
            json!({"channel": CHANNEL, "thread_ts": ts, "text": REPLY}),
        ),
    ]
}

/// `--once` handles the question: the turn reads, proposes and ends held on `reply.send`, naming
/// its commission; nothing is written before the grant. `approve` grants it and `agent resume`
/// performs the write once, through the connectors command line with the grant as its proof;
/// a second resume finds nothing held.
#[test]
fn agent_serve_once_holds_the_send_and_resume_performs_it() {
    let scratch = Scratch::new("once", 0);
    let (base_url, server) = serve(recorded(&scratch.ts));

    let output = scratch.serve_once(&base_url);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stdout}\n{stderr}");
    let printed: Vec<&str> = stdout.lines().collect();
    assert_eq!(printed.len(), 1, "one line for the proposal:\n{stdout}");
    assert!(
        printed[0].starts_with(&format!(
            "{CHANNEL}:{} proposed ask reads=docs/search ",
            scratch.ts
        )),
        "{stdout}"
    );
    assert_eq!(server.join().expect("the server finished").len(), 7);
    assert!(
        scratch.invoked(WRITE).is_empty(),
        "nothing is sent before the grant"
    );
    let held = scratch.held();
    assert_eq!(held.len(), 1, "one send is held: {held:?}");
    let commission = &held[0];
    let record = fs::read_to_string(scratch.state().join("record.jsonl")).unwrap();
    assert!(
        record.contains(&format!(
            "reply.send awaits approval: commission {commission}"
        )),
        "{record}"
    );

    let early = scratch.resume(commission);
    assert_eq!(
        early.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&early.stderr)
    );
    assert!(
        scratch.invoked(WRITE).is_empty(),
        "a resume before the grant sends nothing"
    );

    let approved = scratch
        .command()
        .args(["approve", commission, "reply.send", "--state"])
        .arg(scratch.state())
        .output()
        .unwrap();
    assert_eq!(
        approved.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&approved.stderr)
    );

    let resumed = scratch.resume(commission);
    let stdout = String::from_utf8_lossy(&resumed.stdout);
    let stderr = String::from_utf8_lossy(&resumed.stderr);
    assert_eq!(resumed.status.code(), Some(0), "{stdout}\n{stderr}");
    assert!(
        stdout.starts_with(&format!("sent for commission {commission}: applied")),
        "{stdout}"
    );
    let writes = scratch.invoked(WRITE);
    assert_eq!(writes.len(), 1, "one write: {writes:?}");
    let proof = writes[0]
        .windows(2)
        .find(|w| w[0] == "--approval-file")
        .map(|w| PathBuf::from(&w[1]))
        .expect("the write carries its proof");
    assert!(
        proof.starts_with(scratch.state()),
        "the proof is the grant in the state directory: {}",
        proof.display()
    );
    assert!(proof.is_file());
    let stdin = fs::read_to_string(scratch.root.join("connectors-state/stdin.log")).unwrap();
    assert!(
        stdin.contains(REPLY),
        "the proposal is what is sent: {stdin}"
    );
    assert!(scratch.held().is_empty(), "nothing stays held");

    let again = scratch.resume(commission);
    assert_eq!(
        again.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
    assert_eq!(
        scratch.invoked(WRITE).len(),
        1,
        "a second resume sends nothing"
    );
}

/// Without `--once` the host polls until SIGTERM, which ends it after the item it is handling:
/// here after the first cycle, while it waits out a one-hour interval. It exits 0 and prints what
/// it recorded.
#[test]
fn sigterm_ends_the_host_after_the_item() {
    let scratch = Scratch::new("sigterm", 3600);
    let (base_url, server) = serve(recorded(&scratch.ts));
    let catalog = scratch.catalog(&base_url);
    let mut child = Reaped(
        scratch
            .serve_command(&catalog)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start b10x-loom"),
    );
    let record = scratch.state().join("record.jsonl");
    let deadline = Instant::now() + Duration::from_secs(60);
    while fs::read_to_string(&record).map_or(0, |text| text.matches('\n').count()) < 1 {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "the host ended before its first cycle was recorded"
        );
        assert!(
            Instant::now() < deadline,
            "the first cycle was not recorded in 60 s"
        );
        std::thread::sleep(Duration::from_millis(50));
    }

    let killed = Command::new("kill")
        .args(["-TERM", &child.0.id().to_string()])
        .status()
        .expect("kill");
    assert!(killed.success());
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "SIGTERM did not stop the host in 30 s"
        );
        std::thread::sleep(Duration::from_millis(50));
    };

    let mut stdout = String::new();
    let mut stderr = String::new();
    child
        .0
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    child
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert_eq!(status.code(), Some(0), "{stdout}\n{stderr}");
    assert!(
        stderr.contains("stopping after the current item"),
        "{stderr}"
    );
    assert_eq!(
        stdout.lines().count(),
        1,
        "it prints what it recorded:\n{stdout}"
    );
    assert_eq!(server.join().expect("the server finished").len(), 7);
    assert_eq!(scratch.held().len(), 1, "the send stays held");
}

/// `agent resume` holds the state directory as a serving host does, so the two never perform
/// one grant twice: while another process holds it, the resume exits 1 naming the directory and
/// performs nothing; once it is released, the resume performs the send.
#[test]
fn a_resume_while_the_state_is_held_is_refused() {
    let scratch = Scratch::new("held-state", 0);
    let (base_url, server) = serve(recorded(&scratch.ts));
    let output = scratch.serve_once(&base_url);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().expect("the server finished");
    let held = scratch.held();
    assert_eq!(held.len(), 1, "{held:?}");
    let commission = &held[0];
    let approved = scratch
        .command()
        .args(["approve", commission, "reply.send", "--state"])
        .arg(scratch.state())
        .output()
        .unwrap();
    assert_eq!(approved.status.code(), Some(0));
    let record = fs::OpenOptions::new()
        .append(true)
        .open(scratch.state().join("record.jsonl"))
        .unwrap();
    record
        .try_lock()
        .expect("this test holds the state directory");

    let refused = scratch.resume(commission);

    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert_eq!(refused.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("held by another host"),
        "the refusal names the hold: {stderr}"
    );
    assert!(scratch.invoked(WRITE).is_empty(), "nothing was sent");
    assert_eq!(scratch.held(), held, "the send stays held");
    record.unlock().unwrap();

    let resumed = scratch.resume(commission);
    assert_eq!(
        resumed.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    assert_eq!(scratch.invoked(WRITE).len(), 1);
}

/// A spec `agent check` refuses is not served: exit 3, the refusal on standard error, and no
/// read, no state directory.
#[test]
fn agent_serve_refuses_what_check_refuses() {
    let scratch = Scratch::new("refused", 0);
    let spec = fs::read_to_string(scratch.spec()).unwrap();
    fs::write(
        scratch.spec(),
        spec.replace("allow: [C0FIXTURE1]", "allow: []"),
    )
    .unwrap();
    let (base_url, _server) = serve(Vec::new());

    let output = scratch.serve_once(&base_url);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(3), "{stderr}");
    assert!(
        stderr.contains("EmptyChannelAllow at channel.allow"),
        "{stderr}"
    );
    assert!(scratch.argv().is_empty(), "nothing was read");
    assert!(!scratch.state().exists(), "no state directory was made");
}

/// `--state <DIR>` is a path like any other flag's: a relative one, given from the directory
/// that holds it, serves, approves and resumes the same way as an absolute one. The help text
/// names no restriction. The proof the write carries is derived from `--state` as given
/// (`crates/loom-agent/src/serve.rs:302-309`) and Connectors refuses a relative proof before
/// any process (`approval-not-absolute`), so the approved send must still be performed.
#[test]
fn a_relative_state_directory_performs_the_granted_send() {
    let scratch = Scratch::new("relative-state", 0);
    let (base_url, server) = serve(recorded(&scratch.ts));
    let catalog = scratch.catalog(&base_url);
    let relative = |command: &mut Command| {
        command.current_dir(&scratch.root);
    };
    let mut command = scratch.command();
    command
        .args(["agent", "serve", "--spec"])
        .arg(scratch.spec())
        .args(["--state", "agent-state", "--catalog"])
        .arg(&catalog)
        .args([
            "--classifier-model",
            "lab-classifier",
            "--model",
            "lab-agent",
            "--once",
        ]);
    relative(&mut command);
    let output = command.output().expect("run b10x-loom");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(server.join().expect("the server finished").len(), 7);
    let held = scratch.held();
    assert_eq!(held.len(), 1, "one send is held: {held:?}");
    let commission = &held[0];
    let mut approve = scratch.command();
    approve.args([
        "approve",
        commission,
        "reply.send",
        "--state",
        "agent-state",
    ]);
    relative(&mut approve);
    let approved = approve.output().unwrap();
    assert_eq!(
        approved.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&approved.stderr)
    );

    let mut resume = scratch.command();
    resume
        .args(["agent", "resume", commission, "--spec"])
        .arg(scratch.spec())
        .args(["--state", "agent-state"]);
    relative(&mut resume);
    let resumed = resume.output().unwrap();

    let stdout = String::from_utf8_lossy(&resumed.stdout);
    let stderr = String::from_utf8_lossy(&resumed.stderr);
    assert_eq!(
        resumed.status.code(),
        Some(0),
        "the granted send is performed from a relative --state:\n{stdout}\n{stderr}"
    );
    assert_eq!(
        scratch.invoked(WRITE).len(),
        1,
        "one write: {:?}",
        scratch.argv()
    );
}

// ---------------------------------------------------------------------------------------------
// Fixtures.

/// A scratch directory under `CARGO_TARGET_TMPDIR`: the home, the fake `connectors` and its state
/// directory, the spec, the catalog and the host's state directory; removed on drop.
struct Scratch {
    root: PathBuf,
    /// The question's `ts`, an hour ago.
    ts: String,
}

impl Scratch {
    fn new(label: &str, poll_interval_seconds: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "agent-serve-{label}-{}-{}",
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
            for operation in [
                "conversations.list",
                "conversations.history",
                "conversations.replies",
                "docs.search",
            ] {
                fs::copy(
                    fixtures.join(format!("{verb}-{operation}.json")),
                    connectors_state.join(format!("{verb}.{operation}.json")),
                )
                .unwrap();
            }
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
        let ts = format!("{}.000100", now.as_secs() - 3600);
        let answer = |operation: &str, result: Value| {
            let document = json!({"ok": true, "result": {"adapter": "slack", "operation": operation,
                                   "revision": "rev-1", "result": result}});
            fs::write(
                connectors_state.join(format!("invoke.{operation}.json")),
                document.to_string(),
            )
            .unwrap();
        };
        answer(
            "conversations.list",
            json!({"ok": true, "channels": [{"id": CHANNEL, "name": "fixture", "is_channel": true,
                   "is_member": true, "is_archived": false, "num_members": 5}],
                   "response_metadata": {"next_cursor": ""}}),
        );
        answer(
            "conversations.history",
            json!({"ok": true, "messages": [{"type": "message", "user": "U0ALICE",
                   "text": "Is the deploy finished?", "ts": ts}], "has_more": false}),
        );
        let scratch = Self { root, ts };
        let spec = format!(
            "format: loom.agent/1
agent: {{id: test-operator, name: Test operator, brief: Answer questions about the deploy.}}
model:
  routes: routes.yaml
  roles: [{{role: agent, model: small}}, {{role: classifier, model: small}}]
loops:
  outer: {{poll_interval_seconds: {poll_interval_seconds}, sources: [chat], min_age_minutes: 10, lookback_minutes: 120}}
  inner: {{protocol: operator-reply@1, step_budget: 8, model_budget: 16, classify_threshold: 0.5}}
states:
  - {{name: answer, when: {{intent: Ask}}, actions: [source.read, reply.propose, reply.send, reply.decline], sources: [docs]}}
  - {{name: hand-off, when: {{intent: Task}}, actions: [reply.decline], sources: []}}
bindings:
  - {{name: chat, kind: connectors, adapter: slack, connection: slack-fixture, list: conversations.list, get: conversations.history, search: conversations.replies, write: {WRITE}}}
  - {{name: docs, kind: connectors, adapter: docs, connection: docs-fixture, search: docs.search}}
channel: {{binding: chat, allow: [{CHANNEL}], bot_user_id: U0BOT}}
writes: {{mode: approval, max_per_hour: 5}}
policy: {{capabilities: [datasource.read, reply.propose, reply.send]}}
runtime:
  state_dir: {state}
  connectors: {{program: {program}, state_dir: {connectors}}}
  confinement: none
",
            state = scratch.state().display(),
            program = fake.display(),
            connectors = connectors_state.display(),
        );
        fs::write(scratch.spec(), spec).unwrap();
        scratch
    }

    fn spec(&self) -> PathBuf {
        self.root.join("agent.yaml")
    }

    fn state(&self) -> PathBuf {
        self.root.join("agent-state")
    }

    /// The fixture catalog with its endpoints at `base_url`.
    fn catalog(&self, base_url: &str) -> PathBuf {
        let path = self.root.join("catalog.toml");
        fs::write(&path, CATALOG.replace("http://127.0.0.1:1/v1", base_url)).unwrap();
        path
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

    /// `agent serve` on the catalog at `catalog`, without `--once`.
    fn serve_command(&self, catalog: &Path) -> Command {
        let mut command = self.command();
        command
            .args(["agent", "serve", "--spec"])
            .arg(self.spec())
            .arg("--state")
            .arg(self.state())
            .arg("--catalog")
            .arg(catalog)
            .args([
                "--classifier-model",
                "lab-classifier",
                "--model",
                "lab-agent",
            ]);
        command
    }

    /// `agent serve --once` on the catalog at `base_url`.
    fn serve_once(&self, base_url: &str) -> Output {
        let catalog = self.catalog(base_url);
        self.serve_command(&catalog)
            .arg("--once")
            .output()
            .expect("run b10x-loom")
    }

    /// `agent resume <commission>`.
    fn resume(&self, commission: &str) -> Output {
        self.command()
            .args(["agent", "resume", commission, "--spec"])
            .arg(self.spec())
            .arg("--state")
            .arg(self.state())
            .output()
            .expect("run b10x-loom")
    }

    /// The commissions whose sends the state directory holds.
    fn held(&self) -> Vec<String> {
        let mut held: Vec<String> = fs::read_dir(self.state().join("approvals"))
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter_map(|entry| {
                        entry
                            .file_name()
                            .to_str()?
                            .strip_suffix(".json")
                            .map(ToOwned::to_owned)
                    })
                    .collect()
            })
            .unwrap_or_default();
        held.sort();
        held
    }

    /// Every run of the fake, its arguments.
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

    /// The invocations of `operation`.
    fn invoked(&self, operation: &str) -> Vec<Vec<String>> {
        self.argv()
            .into_iter()
            .filter(|argv| argv.contains(&"invoke".to_owned()))
            .filter(|argv| {
                argv.windows(2)
                    .any(|w| w[0] == "--operation" && w[1] == operation)
            })
            .collect()
    }
}

/// A child process killed and reaped on drop, so a failed assertion leaves none alive.
struct Reaped(Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A recorded Chat Completions stream: `model` calls `tool` with `arguments`.
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

/// Answers one request per recorded response, in order, on a loopback socket, and returns what
/// each request carried.
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

/// Installs `source` as the executable `target` through `install(1)`, in a child process, so this
/// process never holds a writable descriptor on a program a test runs.
fn install_program(source: &Path, target: &Path) {
    let status = std::process::Command::new("/usr/bin/install")
        .args(["-m", "755"])
        .arg(source)
        .arg(target)
        .status()
        .unwrap();
    assert!(status.success(), "install {}: {status}", target.display());
}

// ---- story:agent-a2a ------------------------------------------------------------------------------

/// The bearer of the A2A cases: a throwaway value set only in the child's environment.
const A2A_TOKEN: &str = "a2a-cli-test-bearer-c51e";

impl Scratch {
    /// Declares `expose.a2a` on `port` in the spec.
    fn expose_a2a(&self, port: u16) {
        let mut spec = fs::read_to_string(self.spec()).unwrap();
        spec.push_str(&format!(
            "expose:\n  a2a: {{port: {port}, path: /a2a, card: {{name: test-operator, description: Answers about the deploy, skills: [logs, metrics]}}}}\n"
        ));
        fs::write(self.spec(), spec).unwrap();
    }

    /// The channel's history holds no message, so the poll finds no item.
    fn quiet_channel(&self) {
        let document = json!({"ok": true, "result": {"adapter": "slack",
            "operation": "conversations.history", "revision": "rev-1",
            "result": {"ok": true, "messages": [], "has_more": false}}});
        fs::write(
            self.root
                .join("connectors-state/invoke.conversations.history.json"),
            document.to_string(),
        )
        .unwrap();
    }
}

/// A loopback port nothing listens on at the time of asking.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// One HTTP/1.1 request to `127.0.0.1:port`: the status and the body, or `None` when nothing
/// listens yet.
fn a2a_request(
    port: u16,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    body: Option<&Value>,
) -> Option<(u16, String)> {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let body = body.map(Value::to_string).unwrap_or_default();
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n");
    if let Some(token) = bearer {
        head.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    head.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    ));
    stream.write_all(head.as_bytes()).ok()?;
    stream.write_all(body.as_bytes()).ok()?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw).ok()?;
    let (head, body) = raw.split_once("\r\n\r\n")?;
    let status = head.split(' ').nth(1)?.parse().ok()?;
    Some((status, body.to_owned()))
}

/// A spec declaring `expose.a2a` is not served without its bearer: the host exits 1 naming the
/// variable before any read, model call or record.
#[test]
fn agent_serve_refuses_to_start_without_the_a2a_bearer() {
    let scratch = Scratch::new("a2a-no-bearer", 3600);
    scratch.expose_a2a(free_port());
    let catalog = scratch.catalog("http://127.0.0.1:1/v1");

    for bearer in [None, Some("")] {
        let mut command = scratch.serve_command(&catalog);
        command.args(["--once", "--a2a-address", "127.0.0.1"]);
        match bearer {
            Some(value) => command.env("LOOM_A2A_BEARER", value),
            None => command.env_remove("LOOM_A2A_BEARER"),
        };
        let output = command.output().expect("run b10x-loom");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{stderr}");
        assert!(stderr.contains("LOOM_A2A_BEARER"), "{stderr}");
        assert!(scratch.argv().is_empty(), "nothing was read");
        assert!(
            !scratch.state().join("record.jsonl").exists(),
            "nothing was recorded"
        );
    }
}

/// `LOOM_HTTP_CA_FILE` naming a missing file, an empty file, a file holding no certificate or one
/// past 1048576 bytes stops the host with exit 1 naming the variable and the reason, before any
/// read, model call or record, and never prints the file (`story:http-binding-cluster-ca`).
#[test]
fn agent_serve_refuses_to_start_on_an_unusable_http_ca_file() {
    let scratch = Scratch::new("ca-file", 3600);
    let catalog = scratch.catalog("http://127.0.0.1:1/v1");
    let text = "the cluster CA is in the service account's ca.crt, not here\n";
    let mut large = text.repeat(1_048_576 / text.len() + 1).into_bytes();
    large.truncate(1_048_577);
    let cases: [(&str, Option<&[u8]>, &str); 4] = [
        ("missing.pem", None, "Unreadable"),
        ("empty.pem", Some(b""), "NoCertificate"),
        ("text.pem", Some(text.as_bytes()), "NoCertificate"),
        ("large.pem", Some(&large), "TooLarge"),
    ];
    for (name, bytes, reason) in cases {
        let path = scratch.root.join(name);
        if let Some(bytes) = bytes {
            fs::write(&path, bytes).unwrap();
        }
        let output = scratch
            .serve_command(&catalog)
            .arg("--once")
            .env("LOOM_HTTP_CA_FILE", &path)
            .output()
            .expect("run b10x-loom");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{name}: {stderr}");
        assert!(
            stderr.contains(&format!("LOOM_HTTP_CA_FILE: {reason}")),
            "{name}: {stderr}"
        );
        assert!(
            !stderr.contains("service account"),
            "the file is printed: {stderr}"
        );
        assert!(scratch.argv().is_empty(), "{name}: nothing was read");
        assert!(
            !scratch.state().join("record.jsonl").exists(),
            "{name}: nothing was recorded"
        );
    }
}

/// `LOOM_HTTP_CA_FILE` set to blank text is no setting: the host starts as without it, here
/// reaching the next start-up check, the A2A bearer.
#[test]
fn agent_serve_takes_a_blank_http_ca_file_as_no_setting() {
    let scratch = Scratch::new("ca-file-blank", 3600);
    scratch.expose_a2a(free_port());
    let catalog = scratch.catalog("http://127.0.0.1:1/v1");
    let output = scratch
        .serve_command(&catalog)
        .args(["--once", "--a2a-address", "127.0.0.1"])
        .env("LOOM_HTTP_CA_FILE", " \n")
        .env_remove("LOOM_A2A_BEARER")
        .output()
        .expect("run b10x-loom");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("LOOM_A2A_BEARER"), "{stderr}");
    assert!(!stderr.contains("LOOM_HTTP_CA_FILE"), "{stderr}");
}

/// With `expose.a2a` and its bearer the served agent answers A2A beside the poll: the probes
/// without a bearer, the card and every message only with it, and a message is one turn on
/// `inbound-answer@1` answered with its record. Nothing is sent through A2A; SIGTERM ends both.
#[test]
fn agent_serve_answers_a2a_beside_the_poll() {
    let scratch = Scratch::new("a2a-serve", 3600);
    scratch.quiet_channel();
    let port = free_port();
    scratch.expose_a2a(port);
    let select = |action: &str| sse(AGENT, "select_action", json!({ "action": action }));
    let (base_url, models) = serve(vec![
        sse(
            CLASSIFIER,
            "classify_item",
            json!({"intent": "ask", "hints": ["docs"], "confidence": 0.9}),
        ),
        select("source.read"),
        sse(
            AGENT,
            "action_arguments",
            json!({"source": "docs", "kind": "search", "input": {"query": "deploy"}}),
        ),
        select("reply.propose"),
        sse(AGENT, "action_arguments", json!({"text": REPLY})),
    ]);
    let catalog = scratch.catalog(&base_url);
    let mut child = Reaped(
        scratch
            .serve_command(&catalog)
            .args(["--a2a-address", "127.0.0.1"])
            .env("LOOM_A2A_BEARER", A2A_TOKEN)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start b10x-loom"),
    );
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some((200, _)) = a2a_request(port, "GET", "/ready", None, None) {
            break;
        }
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "the host ended before its listener answered"
        );
        assert!(Instant::now() < deadline, "no readiness in 60 s");
        std::thread::sleep(Duration::from_millis(50));
    }

    let card_path = "/.well-known/agent-card.json";
    let (status, refused) = a2a_request(port, "GET", card_path, None, None).unwrap();
    assert_eq!(status, 401, "{refused}");
    assert!(refused.contains("MissingBearer"), "{refused}");
    let (status, card) = a2a_request(port, "GET", card_path, Some(A2A_TOKEN), None).unwrap();
    assert_eq!(status, 200, "{card}");
    let card: Value = serde_json::from_str(&card).unwrap();
    assert_eq!(card["skills"][0]["id"], "logs", "{card}");
    assert_eq!(card["skills"][1]["id"], "metrics", "{card}");
    let send = json!({"jsonrpc": "2.0", "id": "q-1", "method": "message/send",
        "params": {"message": {"messageId": "m-1", "role": "ROLE_USER",
            "parts": [{"text": "Is the deploy finished?"}]}}});
    let (status, refused) =
        a2a_request(port, "POST", "/a2a", Some("not-the-bearer"), Some(&send)).unwrap();
    assert_eq!(status, 401, "{refused}");
    assert!(refused.contains("WrongBearer"), "{refused}");
    let (status, answer) = a2a_request(port, "POST", "/a2a", Some(A2A_TOKEN), Some(&send)).unwrap();
    assert_eq!(status, 200, "{answer}");
    let answer: Value = serde_json::from_str(&answer).unwrap();
    let task = &answer["result"]["task"];
    assert_eq!(task["status"]["state"], "TASK_STATE_COMPLETED", "{answer}");
    let record = &task["artifacts"][0]["parts"][1]["data"];
    assert_eq!(record["outcome"], "proposed", "{answer}");
    assert_eq!(record["proposal"], REPLY, "{answer}");
    assert_eq!(record["reads"][0]["source"], "docs", "{answer}");
    let get = json!({"jsonrpc": "2.0", "id": 2, "method": "GetTask",
        "params": {"id": task["id"]}});
    let (_, got) = a2a_request(port, "POST", "/a2a", Some(A2A_TOKEN), Some(&get)).unwrap();
    let got: Value = serde_json::from_str(&got).unwrap();
    assert_eq!(
        got["result"]["status"]["state"], "TASK_STATE_COMPLETED",
        "{got}"
    );

    let killed = Command::new("kill")
        .args(["-TERM", &child.0.id().to_string()])
        .status()
        .expect("kill");
    assert!(killed.success());
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "SIGTERM did not stop the host in 30 s"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut stderr = String::new();
    child
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert_eq!(status.code(), Some(0), "{stderr}");
    assert!(stderr.contains("A2A listening on 127.0.0.1:"), "{stderr}");
    assert!(!stderr.contains(A2A_TOKEN), "the bearer is never printed");
    assert_eq!(models.join().expect("the models answered").len(), 5);
    assert!(
        scratch.invoked(WRITE).is_empty(),
        "nothing is sent through A2A"
    );
    assert!(scratch.held().is_empty(), "no send is held either");
    assert!(
        a2a_request(port, "GET", "/live", None, None).is_none(),
        "the listener stopped with the host"
    );
}

impl Scratch {
    /// Rewrites the spec as an agent reached only over A2A on `port`: `expose.a2a`, the `docs`
    /// binding, no `channel`, no chat binding and no writes.
    fn a2a_only(&self, port: u16) {
        let spec = format!(
            "format: loom.agent/1
agent: {{id: test-operator, name: Test operator, brief: Answer questions about the deploy.}}
model:
  routes: routes.yaml
  roles: [{{role: agent, model: small}}, {{role: classifier, model: small}}]
loops:
  outer: {{poll_interval_seconds: 3600, sources: [docs], min_age_minutes: 0, lookback_minutes: 120}}
  inner: {{protocol: operator-reply@1, step_budget: 8, model_budget: 16, classify_threshold: 0.5}}
states:
  - {{name: answer, when: {{intent: Ask}}, actions: [source.read, reply.propose, reply.decline], sources: [docs]}}
bindings:
  - {{name: docs, kind: connectors, adapter: docs, connection: docs-fixture, search: docs.search}}
policy: {{capabilities: [datasource.read, reply.propose]}}
expose:
  a2a: {{port: {port}, path: /a2a, card: {{name: test-operator, description: Answers about the deploy, skills: [logs]}}}}
runtime:
  state_dir: {state}
  connectors: {{program: {program}, state_dir: {connectors}}}
  confinement: none
",
            state = self.state().display(),
            program = self.root.join("connectors").display(),
            connectors = self.root.join("connectors-state").display(),
        );
        fs::write(self.spec(), spec).unwrap();
    }
}

/// A spec with `expose.a2a` and no `channel` is served: the host polls nothing, its main loop is
/// the A2A listener, a message runs its turn, and SIGTERM ends the host with exit 0. `--once` has
/// no cycle to run there and is refused before anything is bound or read.
#[test]
fn agent_serve_answers_a2a_without_a_channel() {
    let scratch = Scratch::new("a2a-only", 3600);
    let port = free_port();
    scratch.a2a_only(port);

    let catalog = scratch.catalog("http://127.0.0.1:1/v1");
    let once = scratch
        .serve_command(&catalog)
        .args(["--once", "--a2a-address", "127.0.0.1"])
        .env("LOOM_A2A_BEARER", A2A_TOKEN)
        .output()
        .expect("run b10x-loom");
    let stderr = String::from_utf8_lossy(&once.stderr);
    assert_eq!(once.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("--once"), "{stderr}");
    assert!(scratch.argv().is_empty(), "nothing was read");

    let select = |action: &str| sse(AGENT, "select_action", json!({ "action": action }));
    let (base_url, models) = serve(vec![
        sse(
            CLASSIFIER,
            "classify_item",
            json!({"intent": "ask", "hints": ["docs"], "confidence": 0.9}),
        ),
        select("source.read"),
        sse(
            AGENT,
            "action_arguments",
            json!({"source": "docs", "kind": "search", "input": {"query": "deploy"}}),
        ),
        select("reply.propose"),
        sse(AGENT, "action_arguments", json!({"text": REPLY})),
    ]);
    let catalog = scratch.catalog(&base_url);
    let mut child = Reaped(
        scratch
            .serve_command(&catalog)
            .args(["--a2a-address", "127.0.0.1"])
            .env("LOOM_A2A_BEARER", format!("{A2A_TOKEN}\n"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start b10x-loom"),
    );
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some((200, _)) = a2a_request(port, "GET", "/ready", None, None) {
            break;
        }
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "the host ended before its listener answered"
        );
        assert!(Instant::now() < deadline, "no readiness in 60 s");
        std::thread::sleep(Duration::from_millis(50));
    }
    let send = json!({"jsonrpc": "2.0", "id": "q-1", "method": "SendMessage",
        "params": {"message": {"messageId": "m-1", "role": "ROLE_USER",
            "parts": [{"text": "Is the deploy finished?"}]}}});
    let (status, answer) = a2a_request(port, "POST", "/a2a", Some(A2A_TOKEN), Some(&send)).unwrap();
    assert_eq!(status, 200, "{answer}");
    let answer: Value = serde_json::from_str(&answer).unwrap();
    let task = &answer["result"]["task"];
    assert_eq!(task["status"]["state"], "TASK_STATE_COMPLETED", "{answer}");
    let record = &task["artifacts"][0]["parts"][1]["data"];
    assert_eq!(record["outcome"], "proposed", "{answer}");
    assert_eq!(record["proposal"], REPLY, "{answer}");

    let killed = Command::new("kill")
        .args(["-TERM", &child.0.id().to_string()])
        .status()
        .expect("kill");
    assert!(killed.success());
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "SIGTERM did not stop the host in 30 s"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut stderr = String::new();
    child
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert_eq!(status.code(), Some(0), "{stderr}");
    assert!(stderr.contains("A2A listening on 127.0.0.1:"), "{stderr}");
    assert_eq!(models.join().expect("the models answered").len(), 5);
    assert!(
        scratch
            .argv()
            .iter()
            .all(|argv| !argv.iter().any(|arg| arg.starts_with("conversations."))),
        "no channel is read: {:?}",
        scratch.argv()
    );
    assert!(
        a2a_request(port, "GET", "/live", None, None).is_none(),
        "the listener stopped with the host"
    );
}
