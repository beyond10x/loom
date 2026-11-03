//! Acceptance for `story:agent-run-once`, the command line: `b10x-loom agent run --spec <FILE>
//! --state <DIR> --input <FILE>` runs one turn of a `loom.agent/1` spec on the task text the
//! caller gives, records it in the state directory as `agent serve` records a handled item,
//! performs at most the sends `writes` admits, prints one line per proposal and exits with a status
//! saying how the turn ended. It needs neither a `channel` nor `expose.a2a`.
//!
//! The decision of how a run ends is `loom.agent.RunOnce`, held to its scenarios under
//! `ess/scenarios/agent-run-once/` by `task conform`. The conformance target cannot run the binary,
//! so each scenario's end-to-end half is a case here, named after it:
//!
//! | Scenario | Case |
//! |---|---|
//! | `agent-run-once/one-turn-recorded` | [`one_turn_recorded`] |
//! | `agent-run-once/send-under-writes` | [`send_under_writes`] |
//! | `agent-run-once/writes-zero-sends-nothing` | [`writes_zero_sends_nothing`] |
//! | `agent-run-once/input-missing` | [`input_missing`] |
//! | `agent-run-once/refused-spec` | [`refused_spec`] |
//! | `agent-run-once/budgets-hold` | [`budgets_hold`] |
//!
//! Every case runs the binary. Reads and writes go to the fake `connectors` of `loom-connectors`,
//! answering from `loom-plugin-slack`'s and `loom-connectors`' fixtures. The models are the fixture
//! catalog's anonymous Chat Completions routes (`fixtures/catalog-route/catalog.toml`) at a
//! loopback socket this file opens, which replays recorded responses in order. No case reaches the
//! network, a credential or a Codex login.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const CATALOG: &str = include_str!("fixtures/catalog-route/catalog.toml");
const CLASSIFIER: &str = "example/Classifier-Model";
const AGENT: &str = "example/Agent-Model";
const CHANNEL: &str = "C0FIXTURE1";
const WRITE: &str = "message.post";
const TASK: &str = "Summarise what the runbooks say about the deploy key.";
const REPLY: &str = "The runbook on rotating the deploy key is the one to follow.";

/// The responses of one turn that reads, proposes and, when `send` is true, sends the proposal
/// to the channel.
fn answered(send: bool) -> Vec<Vec<u8>> {
    let select = |action: &str| sse(AGENT, "select_action", json!({ "action": action }));
    let mut responses = vec![
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
    ];
    if send {
        responses.push(select("reply.send"));
        responses.push(sse(
            AGENT,
            "action_arguments",
            json!({"channel": CHANNEL, "text": REPLY}),
        ));
    }
    responses
}

/// `agent-run-once/one-turn-recorded`: a spec with no `channel` and no `expose.a2a`, given an
/// `--input` file, runs one turn on its text against recorded model responses, appends one line to
/// `record.jsonl`, prints one proposal line and exits 0. Nothing is sent.
#[test]
fn one_turn_recorded() {
    let scratch = Scratch::new("one-turn", &Spec::default());
    let (base_url, server) = serve(answered(false));

    let output = scratch.run(&base_url, Some(&scratch.input(TASK)));

    let (stdout, stderr) = texts(&output);
    assert_eq!(output.status.code(), Some(0), "{stdout}\n{stderr}");
    let printed: Vec<&str> = stdout.lines().collect();
    assert_eq!(printed.len(), 1, "one line for the proposal:\n{stdout}");
    assert!(printed[0].starts_with("run-"), "{stdout}");
    assert!(
        printed[0].contains(" proposed ask reads=docs/search "),
        "{stdout}"
    );
    assert!(printed[0].contains(REPLY), "{stdout}");
    let requests = server.join().expect("the server finished");
    assert_eq!(requests.len(), 5, "classified, read, proposed");
    assert!(
        requests[0].contains(TASK),
        "the input's text is the item the classifier reads: {}",
        requests[0]
    );
    let record = scratch.record();
    assert_eq!(record.len(), 1, "one line recorded: {record:?}");
    assert_eq!(record[0]["outcome"], "proposed", "{record:?}");
    assert!(scratch.invoked(WRITE).is_empty(), "nothing is sent");
}

/// `agent-run-once/send-under-writes`: with `writes: {mode: auto, max_per_hour: 1}` the turn
/// proposes and sends, and the send is a governed write through the connectors command line as a
/// served turn's is, counted in `sends.jsonl`. A second run inside the hour proposes again and is
/// refused `RateLimited`: it is recorded, nothing more is sent, and the run exits 3.
#[test]
fn send_under_writes() {
    let scratch = Scratch::new(
        "send",
        &Spec {
            channel: true,
            max_per_hour: 1,
            ..Spec::default()
        },
    );
    let (base_url, server) = serve(answered(true));

    let first = scratch.run(&base_url, Some(&scratch.input(TASK)));

    let (stdout, stderr) = texts(&first);
    assert_eq!(first.status.code(), Some(0), "{stdout}\n{stderr}");
    assert_eq!(stdout.lines().count(), 1, "{stdout}");
    assert_eq!(server.join().expect("the server finished").len(), 7);
    let writes = scratch.invoked(WRITE);
    assert_eq!(writes.len(), 1, "one write: {writes:?}");
    let stdin = fs::read_to_string(scratch.root.join("connectors-state/stdin.log")).unwrap();
    assert!(
        stdin.contains(REPLY),
        "the proposal is what is sent: {stdin}"
    );
    assert!(stdin.contains(CHANNEL), "to the channel: {stdin}");
    let sends = fs::read_to_string(scratch.state().join("sends.jsonl")).unwrap();
    assert_eq!(sends.lines().count(), 1, "the send is counted: {sends}");
    let record = scratch.record();
    assert_eq!(record.len(), 1, "{record:?}");
    assert!(
        record[0]["detail"]
            .as_str()
            .is_some_and(|detail| detail.starts_with("the reply was sent")),
        "{record:?}"
    );

    let (base_url, server) = serve(answered(true));
    let second = scratch.run(&base_url, Some(&scratch.input(TASK)));

    let (stdout, stderr) = texts(&second);
    assert_eq!(second.status.code(), Some(3), "{stdout}\n{stderr}");
    assert!(stderr.contains("RateLimited"), "{stderr}");
    assert_eq!(
        stdout.lines().count(),
        1,
        "the proposal is printed: {stdout}"
    );
    assert_eq!(server.join().expect("the server finished").len(), 7);
    assert_eq!(scratch.invoked(WRITE).len(), 1, "nothing more is sent");
    let record = scratch.record();
    assert_eq!(record.len(), 2, "the refused run is recorded: {record:?}");
    assert!(
        record[1]["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("RateLimited")),
        "{record:?}"
    );
    assert_ne!(
        record[0]["item"], record[1]["item"],
        "each run is an item of its own"
    );
}

/// `agent-run-once/writes-zero-sends-nothing`: `writes.max_per_hour: 0` admits no send. The turn
/// proposes, its send is refused `RateLimited` before any write, the refusal is recorded, and the
/// run exits 3.
#[test]
fn writes_zero_sends_nothing() {
    let scratch = Scratch::new(
        "zero",
        &Spec {
            channel: true,
            max_per_hour: 0,
            ..Spec::default()
        },
    );
    let (base_url, server) = serve(answered(true));

    let output = scratch.run(&base_url, Some(&scratch.input(TASK)));

    let (stdout, stderr) = texts(&output);
    assert_eq!(output.status.code(), Some(3), "{stdout}\n{stderr}");
    assert!(stderr.contains("RateLimited"), "{stderr}");
    server.join().expect("the server finished");
    assert!(scratch.invoked(WRITE).is_empty(), "nothing is sent");
    assert!(!scratch.state().join("sends.jsonl").exists());
    let record = scratch.record();
    assert_eq!(record.len(), 1, "{record:?}");
    assert!(
        record[0]["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("RateLimited")),
        "{record:?}"
    );
}

/// `agent-run-once/input-missing`: without `--input`, or with a file that does not exist, is no
/// regular file, or holds no text, the run exits 1 naming the flag or the file, before any model
/// call and before the state directory is made.
#[test]
fn input_missing() {
    let scratch = Scratch::new("input", &Spec::default());
    let missing = scratch.root.join("no-such-task.txt");
    let empty = scratch.input("");
    let blank = scratch.root.join("blank.txt");
    fs::write(&blank, " \n\t\n").unwrap();
    let directory = scratch.root.join("home");
    let cases: [(&str, Option<&Path>, String); 5] = [
        ("absent", None, "--input".to_owned()),
        ("missing", Some(&missing), missing.display().to_string()),
        (
            "directory",
            Some(&directory),
            directory.display().to_string(),
        ),
        ("empty", Some(&empty), empty.display().to_string()),
        ("blank", Some(&blank), blank.display().to_string()),
    ];
    for (case, input, named) in cases {
        let (base_url, listener) = silent();

        let output = scratch.run(&base_url, input);

        let (stdout, stderr) = texts(&output);
        assert_eq!(output.status.code(), Some(1), "{case}: {stdout}\n{stderr}");
        assert!(stderr.contains(&named), "{case}: names {named}: {stderr}");
        assert!(stdout.is_empty(), "{case}: {stdout}");
        assert!(listener.accept().is_err(), "{case}: no model was called");
        assert!(!scratch.state().exists(), "{case}: no state directory");
    }
}

/// `agent-run-once/refused-spec`: a spec `agent check` refuses is refused by the run with the same
/// refusal on standard error and exit 3, before any model call, read or state directory, whether
/// or not `--input` is given.
#[test]
fn refused_spec() {
    let scratch = Scratch::new(
        "refused",
        &Spec {
            channel: true,
            max_per_hour: 1,
            ..Spec::default()
        },
    );
    let spec = fs::read_to_string(scratch.spec()).unwrap();
    fs::write(
        scratch.spec(),
        spec.replace(&format!("allow: [{CHANNEL}]"), "allow: []"),
    )
    .unwrap();
    let checked = scratch
        .command()
        .args(["agent", "check", "--spec"])
        .arg(scratch.spec())
        .output()
        .unwrap();
    assert_eq!(checked.status.code(), Some(3));
    let refusal = String::from_utf8_lossy(&checked.stderr).into_owned();
    assert!(refusal.contains("EmptyChannelAllow"), "{refusal}");
    let input = scratch.input(TASK);
    for given in [Some(input.as_path()), None] {
        let (base_url, listener) = silent();

        let output = scratch.run(&base_url, given);

        let (stdout, stderr) = texts(&output);
        assert_eq!(output.status.code(), Some(3), "{stdout}\n{stderr}");
        assert_eq!(stderr, refusal, "the refusal `agent check` gives");
        assert!(listener.accept().is_err(), "no model was called");
        assert!(scratch.argv().is_empty(), "nothing was read");
        assert!(!scratch.state().exists(), "no state directory was made");
    }
}

/// `agent-run-once/budgets-hold`: the turn stops at the spec's inner-loop budgets as a served
/// turn does. With `model_budget: 2` the turn asks the model twice (a selection and its
/// arguments), is recorded `stopped` naming the budget, prints no proposal and exits 4; with
/// `step_budget: 1` it performs one step and is recorded `stopped`, exiting 4.
#[test]
fn budgets_hold() {
    let read_only = || {
        let mut responses = answered(false);
        responses.truncate(3);
        responses
    };
    let by_models = Scratch::new(
        "model-budget",
        &Spec {
            model_budget: 2,
            ..Spec::default()
        },
    );
    let (base_url, server) = serve(read_only());

    let output = by_models.run(&base_url, Some(&by_models.input(TASK)));

    let (stdout, stderr) = texts(&output);
    assert_eq!(output.status.code(), Some(4), "{stdout}\n{stderr}");
    assert!(stdout.is_empty(), "no proposal: {stdout}");
    assert_eq!(server.join().expect("the server finished").len(), 3);
    let record = by_models.record();
    assert_eq!(record.len(), 1, "{record:?}");
    assert_eq!(record[0]["outcome"], "stopped", "{record:?}");
    assert_eq!(
        record[0]["detail"],
        "the turn used its 2 model calls (model_budget) after 1 of its 8 steps (step_budget) without an answer; each step asks the model twice, a selection and its arguments, so 2 model calls allow 1 step",
        "story:turn-budget-stop-names-the-arithmetic: {record:?}"
    );

    let by_steps = Scratch::new(
        "step-budget",
        &Spec {
            step_budget: 1,
            ..Spec::default()
        },
    );
    let (base_url, server) = serve(read_only());

    let output = by_steps.run(&base_url, Some(&by_steps.input(TASK)));

    let (stdout, stderr) = texts(&output);
    assert_eq!(output.status.code(), Some(4), "{stdout}\n{stderr}");
    assert!(stdout.is_empty(), "no proposal: {stdout}");
    assert_eq!(server.join().expect("the server finished").len(), 3);
    let record = by_steps.record();
    assert_eq!(record.len(), 1, "{record:?}");
    assert_eq!(record[0]["outcome"], "stopped", "{record:?}");
}

// ---------------------------------------------------------------------------------------------
// Fixtures.

/// What the spec of a case declares beyond the common part.
struct Spec {
    /// A channel binding with a `write`, the channel allowing [`CHANNEL`], `writes.mode: auto` and
    /// `reply.send` among the capabilities; without it the spec declares no channel and sends
    /// nothing.
    channel: bool,
    max_per_hour: i64,
    step_budget: i64,
    model_budget: i64,
}

impl Default for Spec {
    fn default() -> Self {
        Self {
            channel: false,
            max_per_hour: 0,
            step_budget: 8,
            model_budget: 16,
        }
    }
}

/// A scratch directory under `CARGO_TARGET_TMPDIR`: the home, the fake `connectors` and its state
/// directory, the spec, the catalog, the input and the run's state directory; removed on drop.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(label: &str, spec: &Spec) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "agent-run-{label}-{}-{}",
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
        let (actions, chat, channel, writes, capabilities) = if spec.channel {
            (
                "[source.read, reply.propose, reply.send, reply.decline]",
                format!(
                    "  - {{name: chat, kind: connectors, adapter: slack, connection: slack-fixture, list: conversations.list, get: conversations.history, search: conversations.replies, write: {WRITE}}}\n"
                ),
                format!("channel: {{binding: chat, allow: [{CHANNEL}], bot_user_id: U0BOT}}\n"),
                format!(
                    "writes: {{mode: auto, max_per_hour: {}}}\n",
                    spec.max_per_hour
                ),
                "[datasource.read, reply.propose, reply.send]",
            )
        } else {
            (
                "[source.read, reply.propose, reply.decline]",
                String::new(),
                String::new(),
                String::new(),
                "[datasource.read, reply.propose]",
            )
        };
        let text = format!(
            "format: loom.agent/1
agent: {{id: test-operator, name: Test operator, brief: Answer questions about the deploy.}}
model:
  routes: routes.yaml
  roles: [{{role: agent, model: small}}, {{role: classifier, model: small}}]
loops:
  outer: {{poll_interval_seconds: 60, sources: [docs], min_age_minutes: 0, lookback_minutes: 120}}
  inner: {{protocol: operator-reply@1, step_budget: {step_budget}, model_budget: {model_budget}, classify_threshold: 0.5}}
states:
  - {{name: answer, when: {{intent: Ask}}, actions: {actions}, sources: [docs]}}
  - {{name: hand-off, when: {{intent: Task}}, actions: [reply.decline], sources: []}}
bindings:
{chat}  - {{name: docs, kind: connectors, adapter: docs, connection: docs-fixture, search: docs.search}}
{channel}{writes}policy: {{capabilities: {capabilities}}}
runtime:
  state_dir: {state}
  connectors: {{program: {program}, state_dir: {connectors}}}
  confinement: none
",
            step_budget = spec.step_budget,
            model_budget = spec.model_budget,
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

    /// A task file holding `text`.
    fn input(&self, text: &str) -> PathBuf {
        let path = self.root.join("task.txt");
        fs::write(&path, text).unwrap();
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

    /// `agent run` with the fixture catalog at `base_url` and `input` as `--input`.
    fn run(&self, base_url: &str, input: Option<&Path>) -> Output {
        let catalog = self.root.join("catalog.toml");
        fs::write(&catalog, CATALOG.replace("http://127.0.0.1:1/v1", base_url)).unwrap();
        let mut command = self.command();
        command
            .args(["agent", "run", "--spec"])
            .arg(self.spec())
            .arg("--state")
            .arg(self.state())
            .arg("--catalog")
            .arg(&catalog)
            .args([
                "--classifier-model",
                "lab-classifier",
                "--model",
                "lab-agent",
            ]);
        if let Some(input) = input {
            command.arg("--input").arg(input);
        }
        command.output().expect("run b10x-loom")
    }

    /// Every line of `record.jsonl`, as JSON.
    fn record(&self) -> Vec<Value> {
        fs::read_to_string(self.state().join("record.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).expect("a record line is JSON"))
            .collect()
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

    /// The invocations of `operation`: `invoke <connector>/<instance> <operation>`, after the
    /// global flags.
    fn invoked(&self, operation: &str) -> Vec<Vec<String>> {
        self.argv()
            .into_iter()
            .filter(|argv| {
                argv.iter()
                    .position(|argument| argument == "invoke")
                    .filter(|at| *at == 0 || argv[at - 1] != "operations")
                    .and_then(|at| argv.get(at + 2))
                    .is_some_and(|invoked| invoked == operation)
            })
            .collect()
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

/// A loopback socket that answers nothing: a model call would wait in its backlog, where a
/// non-blocking `accept` after the run finds it.
fn silent() -> (String, TcpListener) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    (base_url, listener)
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
