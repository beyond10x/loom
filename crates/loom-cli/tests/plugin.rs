//! Acceptance for `story:slack-plugin`, the command line: `b10x-loom plugin run slack-handler
//! --config <file> --state <dir> --once` runs the fixture run, `plugin report` prints one line per
//! proposal, an unknown plugin name is refused naming the registered ones, and the turn's model is
//! built from a catalog route alias (story `plugin-host`'s carried bridge).
//!
//! Every case runs the binary. The Slack and `docs` reads go to the fake `connectors` of
//! `loom-connectors`, answering from `loom-plugin-slack`'s fixtures (one channel: a mention asking
//! a question, a question, an answered thread, a bot message and a task request). The models are
//! the fixture catalog's anonymous Chat Completions routes (`fixtures/catalog-route/catalog.toml`)
//! at a loopback socket this file opens, which replays recorded responses in order and keeps what
//! it was sent. No case reaches the network, a credential or a Codex login.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const CATALOG: &str = include_str!("fixtures/catalog-route/catalog.toml");
const MENTION: &str = "C0FIXTURE1:1700000100.000100";
const QUESTION: &str = "C0FIXTURE1:1700000200.000200";
const TASK: &str = "C0FIXTURE1:1700000500.000500";
const CLASSIFIER: &str = "example/Classifier-Model";
const AGENT: &str = "example/Agent-Model";

/// The responses of the fixture run, in the order its requests are sent: each item is classified
/// and answered before the next; an answer is one read and one proposal, a task one protocol pick.
fn recorded() -> Vec<Vec<u8>> {
    let read = |query: &str| json!({"source": "docs", "kind": "search", "input": {"query": query}});
    vec![
        sse(
            CLASSIFIER,
            "classify_item",
            json!({"intent": "ask", "hints": ["docs"], "confidence": 0.9}),
        ),
        sse(AGENT, "source_read", read("rotate deploy key")),
        sse(
            AGENT,
            "reply_propose",
            json!({"text": "Rotate it with the deploy-key runbook (runbooks/deploy-key.md)."}),
        ),
        sse(
            CLASSIFIER,
            "classify_item",
            json!({"intent": "find", "hints": ["docs"], "confidence": 0.85}),
        ),
        sse(AGENT, "source_read", read("staging database runbook")),
        sse(
            AGENT,
            "reply_propose",
            json!({"text": "The staging database runbook is runbooks/staging-database.md."}),
        ),
        sse(
            CLASSIFIER,
            "classify_item",
            json!({"intent": "task", "hints": [], "confidence": 0.8}),
        ),
        sse(
            CLASSIFIER,
            "pick_protocol",
            json!({"protocol": "software-change@1", "confidence": 0.8, "reasons": ["it changes code"]}),
        ),
    ]
}

#[test]
fn plugin_run_slack_handler_once() {
    let scratch = Scratch::new("run-once");
    let (base_url, server) = serve(recorded());

    let output = scratch.run_plugin(&base_url);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stdout}\n{stderr}");
    let printed: Vec<&str> = stdout.lines().collect();
    assert_eq!(printed.len(), 3, "one line per proposal:\n{stdout}");
    assert!(printed[0].starts_with(&format!("{MENTION} proposed ask reads=docs/search ")));
    assert!(printed[1].starts_with(&format!("{QUESTION} proposed find reads=docs/search ")));
    assert!(printed[2].starts_with(&format!("{TASK} proposed_case task case=software-change@1")));
    let record = fs::read_to_string(scratch.state().join("record.jsonl")).unwrap();
    assert_eq!(record.lines().count(), 3, "{record}");
    assert_eq!(server.join().expect("the server finished").len(), 8);
    let invoked: Vec<String> = scratch
        .argv()
        .iter()
        .filter(|argv| argv.contains(&"invoke".to_owned()))
        .filter_map(|argv| {
            argv.windows(2)
                .find(|w| w[0] == "--operation")
                .map(|w| w[1].clone())
        })
        .collect();
    assert_eq!(
        invoked,
        [
            "conversations.list",
            "conversations.history",
            "conversations.replies",
            "docs.search",
            "docs.search"
        ]
    );
}

/// The classifier route serves each classification and the task's pick, and the agent route
/// each turn request, which publishes the turn's actions as tools: the turn's model is the
/// catalog's, through the bridge to the harness model port.
#[test]
fn plugin_run_builds_the_turn_model_from_the_catalog() {
    let scratch = Scratch::new("turn-model");
    let (base_url, server) = serve(recorded());

    let output = scratch.run_plugin(&base_url);

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let sent = server.join().expect("the server finished");
    let calls: Vec<(String, Vec<String>, String)> = sent
        .iter()
        .map(|request| {
            let (head, body) = request.split_once("\r\n\r\n").expect("a whole request");
            assert!(head.starts_with("POST /v1/chat/completions "), "{head}");
            assert!(
                !head.to_ascii_lowercase().contains("authorization"),
                "{head}"
            );
            let body: Value = serde_json::from_str(body).expect("a JSON body");
            let tools = body["tools"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|tool| {
                    tool["function"]["name"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned()
                })
                .collect();
            (
                body["model"].as_str().unwrap_or_default().to_owned(),
                tools,
                body.to_string(),
            )
        })
        .collect();
    let models: Vec<&str> = calls.iter().map(|(model, _, _)| model.as_str()).collect();
    assert_eq!(
        models,
        [
            CLASSIFIER, AGENT, AGENT, CLASSIFIER, AGENT, AGENT, CLASSIFIER, CLASSIFIER
        ]
    );
    let turn_tools = ["reply_decline", "reply_propose", "source_read"];
    for (model, tools, body) in &calls {
        if model == AGENT {
            let mut tools = tools.clone();
            tools.sort();
            assert_eq!(tools, turn_tools, "a turn publishes the frontier's actions");
            assert!(body.contains("inbound-answer@1"), "the turn's instructions");
        }
    }
    assert!(
        calls[2].2.contains("Rotating the deploy key"),
        "the second step of the first turn is told the read it made: {}",
        calls[2].2
    );
}

#[test]
fn plugin_report_prints_proposals() {
    let scratch = Scratch::new("report");
    let empty = scratch.report();
    assert_eq!(empty.status.code(), Some(0), "nothing recorded is no error");
    assert!(empty.stdout.is_empty());
    let (base_url, server) = serve(recorded());
    let ran = scratch.run_plugin(&base_url);
    assert_eq!(ran.status.code(), Some(0));
    server.join().unwrap();

    let output = scratch.report();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        stdout,
        String::from_utf8_lossy(&ran.stdout),
        "the report prints what the run recorded"
    );
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 3, "{stdout}");
    assert!(
        lines[0].ends_with("\"Rotate it with the deploy-key runbook (runbooks/deploy-key.md).\"")
    );
    assert!(lines[2].contains("case=software-change@1 confidence=0.8"));
}

#[test]
fn plugin_run_unknown_name_is_refused() {
    let scratch = Scratch::new("unknown");
    for verb in ["run", "report"] {
        let mut args = vec!["plugin", verb, "mail-handler"];
        let config = scratch.config();
        let config = config.to_str().unwrap();
        if verb == "run" {
            args.extend(["--config", config, "--once"]);
        }
        let state = scratch.state();
        args.extend(["--state", state.to_str().unwrap()]);
        let output = scratch.command().args(&args).output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{verb}: {stderr}");
        assert!(
            stderr.contains("mail-handler") && stderr.contains("slack-handler"),
            "{verb}: names the unknown and the registered plugins: {stderr}"
        );
    }
    assert!(scratch.argv().is_empty(), "nothing was read");
    assert!(!scratch.state().exists(), "no state directory was made");
}

/// Without `--once` the host polls until SIGTERM, which ends it after the item it is handling:
/// here after the first cycle, while it waits out a one-hour interval. It exits 0 and prints what
/// it recorded.
#[test]
fn plugin_run_stops_on_sigterm() {
    let scratch = Scratch::new("sigterm");
    scratch.poll_every(3600);
    let (base_url, server) = serve(recorded());
    let catalog = scratch.catalog(&base_url);
    let mut child = Reaped(
        scratch
            .command()
            .args(["plugin", "run", "slack-handler", "--config"])
            .arg(scratch.config())
            .arg("--state")
            .arg(scratch.state())
            .arg("--catalog")
            .arg(catalog)
            .args([
                "--classifier-model",
                "lab-classifier",
                "--model",
                "lab-agent",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start b10x-loom"),
    );
    let record = scratch.state().join("record.jsonl");
    let deadline = Instant::now() + Duration::from_secs(60);
    while fs::read_to_string(&record).map_or(0, |text| text.matches('\n').count()) < 3 {
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
        3,
        "it prints what it recorded:\n{stdout}"
    );
    assert_eq!(server.join().expect("the server finished").len(), 8);
}

// ---------------------------------------------------------------------------------------------
// Fixtures.

/// A scratch directory under `CARGO_TARGET_TMPDIR`: the home, the fake `connectors` and its state
/// directory, the configuration, the catalog and the plugin's state directory; removed on drop.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("plugin-{label}-{}-{nanos}", std::process::id()));
        for directory in ["home", "connectors-state"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let fake = root.join("connectors");
        install_program(
            &crates.join("loom-connectors/tests/fixtures/connectors/fake-connectors"),
            &fake,
        );
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
                    root.join("connectors-state")
                        .join(format!("{verb}.{operation}.json")),
                )
                .unwrap();
            }
        }
        let scratch = Self { root };
        let config = json!({
            "plugin": {
                "connectors": {
                    "program": fake.display().to_string(),
                    "state_dir": scratch.root.join("connectors-state").display().to_string()
                },
                "sources": [{"name": "docs", "adapter": "docs", "connection": "docs-fixture",
                             "search": "docs.search"}],
                "objectives": [{"name": "help people with cheap lookups", "weight": 0.9}],
                "classify_threshold": 0.5,
                "poll_interval_seconds": 0
            },
            "adapter": "slack",
            "connection": "slack-fixture",
            "list_channels": "conversations.list",
            "history": "conversations.history",
            "replies": "conversations.replies",
            "bot_user_id": "U0BOT",
            "min_age_minutes": 10,
            "seed": 7,
            "channels": [{"channel": "C0FIXTURE1", "objectives": ["help people with cheap lookups"]}]
        });
        fs::write(scratch.config(), config.to_string()).unwrap();
        scratch
    }

    /// The configuration polls every `seconds`.
    fn poll_every(&self, seconds: u64) {
        let mut config: Value =
            serde_json::from_str(&fs::read_to_string(self.config()).unwrap()).unwrap();
        config["plugin"]["poll_interval_seconds"] = json!(seconds);
        fs::write(self.config(), config.to_string()).unwrap();
    }

    fn config(&self) -> PathBuf {
        self.root.join("slack-handler.json")
    }

    fn state(&self) -> PathBuf {
        self.root.join("plugin-state")
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
            .env_remove("RUST_LOG");
        command
    }

    /// `plugin run slack-handler --once` on the catalog at `base_url`.
    fn run_plugin(&self, base_url: &str) -> Output {
        let catalog = self.catalog(base_url);
        self.command()
            .args(["plugin", "run", "slack-handler", "--config"])
            .arg(self.config())
            .arg("--state")
            .arg(self.state())
            .arg("--once")
            .arg("--catalog")
            .arg(catalog)
            .args([
                "--classifier-model",
                "lab-classifier",
                "--model",
                "lab-agent",
            ])
            .output()
            .expect("run b10x-loom")
    }

    fn report(&self) -> Output {
        self.command()
            .args(["plugin", "report", "slack-handler", "--state"])
            .arg(self.state())
            .output()
            .expect("run b10x-loom")
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
/// process never holds a writable descriptor on a program a test runs. A child that a test running
/// in parallel forks inherits every descriptor open at that moment; while it holds one on the
/// program, executing the program fails with "Text file busy".
fn install_program(source: &Path, target: &Path) {
    let status = std::process::Command::new("/usr/bin/install")
        .args(["-m", "755"])
        .arg(source)
        .arg(target)
        .status()
        .unwrap();
    assert!(status.success(), "install {}: {status}", target.display());
}
