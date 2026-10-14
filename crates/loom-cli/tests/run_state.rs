//! Acceptance for `story:cli-run-holds-suspension`: `b10x-loom run --state <dir>` keeps the case
//! (`loom_governor::FileCaseStore`) and the commission's Suspension
//! (`loom_governor::FileSuspensionStore`) in `<dir>`, runs the commission through Commission's
//! `run_with_suspension`, and keeps a resume context (`intake.routing.ResumeContext`) there too,
//! so a withheld approval leaves what a later resume continues from.
//!
//! The case runs the binary, a separate process, and reads `<dir>` back afterwards through the
//! stores themselves, never by matching text in their files. The models are the anonymous Chat
//! Completions routes of the fixture catalog (`fixtures/catalog-route/catalog.toml`) at a loopback
//! socket this file opens, scripted in the recorded stream format `catalog_route.rs` replays; no
//! case reaches the network, a credential or a Codex login (`HOME` and `CODEX_HOME` are an empty
//! scratch directory).
//!
//! The workspace is a git repository with one failing check (`grep -qx fixed check.txt`, no
//! shell). The agent edits the check, runs the tests, then proposes `repository.merge`, which the
//! frontier lists as needing approval: the run stops `ApprovalRequired` and the commission holds
//! its Suspension.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{CaseId, CommissionId, SuspensionState};
use b10x_loom_commission::ports::suspension::SuspensionPort;
use b10x_loom_intake_slice::resume::ResumeContextStore;
use intake_model::confinement::Backend;
use intake_model::context::ContextPolicy;
use loom_governor::{FallibleCaseStore, FileCaseStore, FileSuspensionStore};
use serde_json::{Value, json};

const CATALOG: &str = include_str!("fixtures/catalog-route/catalog.toml");
const INTENT: &str = "Make the failing check pass.";
const TEST_CMD: &str = "grep -qx fixed check.txt";

#[test]
fn run_with_state_holds_a_suspension_on_a_withheld_approval() {
    let scratch = Scratch::new("holds");
    let (base_url, server) = Script::serve(vec![
        classify("software-change@1"),
        agent("select_action", json!({"action": "implementation.edit"})),
        agent(
            "action_arguments",
            json!({
                "files": [{"path": "check.txt", "contents": "fixed\n"}],
                "message": "fix the check"
            }),
        ),
        agent("select_action", json!({"action": "tests.run"})),
        agent("action_arguments", json!({})),
        agent("select_action", json!({"action": "repository.merge"})),
        agent("action_arguments", json!({})),
    ]);
    let catalog = scratch.catalog(&base_url);
    let state = scratch.root.join("state");

    let output = scratch.run(&[
        "--state",
        state.to_str().unwrap(),
        "--workspace",
        scratch.workspace.to_str().unwrap(),
        "--test-cmd",
        TEST_CMD,
        "--max-steps",
        "6",
        "--catalog",
        catalog.to_str().unwrap(),
        "--classifier-model",
        "lab-classifier",
        "--model",
        "lab-agent",
        INTENT,
    ]);
    let calls = server.finish();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stdout}\n{stderr}");
    assert_eq!(
        stdout.lines().last(),
        Some("stopped: ApprovalRequired (repository.merge)"),
        "{stdout}\n{stderr}"
    );
    assert_eq!(
        calls.first().map(String::as_str),
        Some("pick_protocol"),
        "{calls:?}"
    );
    assert!(
        calls.len() >= 6,
        "the agent selected the merge: {calls:?}\n{stdout}"
    );

    // The resume context names the commission, and what the run was given.
    let contexts = ResumeContextStore::open(&state)
        .expect("the resume context store opens")
        .list()
        .expect("the resume contexts read back");
    assert_eq!(contexts.len(), 1, "{contexts:?}");
    let context = &contexts[0];
    assert_eq!(context.intent, INTENT);
    assert_eq!(context.route.protocol, "software-change@1");
    assert_eq!(context.route.context_policy, ContextPolicy::Legacy);
    assert_eq!(
        context.workspace.as_deref(),
        scratch.workspace.to_str(),
        "{context:?}"
    );
    assert_eq!(context.test_command, ["grep", "-qx", "fixed", "check.txt"]);
    assert_eq!(
        context.test_paths,
        b10x_loom_intake_slice::test_paths::TestPaths::DEFAULTS,
        "the context keeps the test-path patterns the run held tests.write to"
    );
    assert_eq!(context.max_steps, 6);
    assert_eq!(context.confinement, Backend::None);
    assert_eq!(context.cgroup_root, None);
    assert_eq!(context.model, "lab-agent");
    assert_eq!(context.catalog.as_deref(), catalog.to_str());

    // One Pending Suspension, for that commission, on the merge.
    let commission = CommissionId(Uuid(context.commission_id.0.0.clone()));
    let suspensions = FileSuspensionStore::open(&state).expect("the suspension store opens");
    let held = suspensions
        .get(&commission)
        .expect("the suspension store reads")
        .expect("the commission holds a Suspension");
    assert_eq!(held.state, SuspensionState::Pending);
    assert_eq!(held.data.commission_id, commission);
    assert_eq!(held.data.action, "repository.merge");
    assert_eq!(held.data.case_id.0, context.case_id.0);
    let suspension_files = std::fs::read_dir(&state)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("suspension-")
        })
        .count();
    assert_eq!(suspension_files, 1, "one Suspension in the state directory");

    // The case is held at the revision the request was made at.
    let cases = FileCaseStore::open(&state).expect("the case store opens");
    let case = cases
        .get(&CaseId(context.case_id.0.clone()))
        .expect("the case store reads")
        .expect("the case is held");
    assert_eq!(case.protocol, "software-change@1");
    assert_eq!(case.revision, held.data.expected_case_revision);
}

/// A resume context keeps the workspace, the cgroup root and the catalog path as text, so with
/// `--state` a path that is not UTF-8 is refused before the classifier is called and before any
/// store opens the state directory: the run names the path, calls no model and leaves no state
/// directory behind.
#[test]
fn run_with_state_refuses_a_path_that_is_not_utf8_before_any_model_call() {
    use std::ffi::{OsStr, OsString};
    use std::os::unix::ffi::OsStrExt as _;

    for (label, what) in [
        ("workspace", "the workspace"),
        ("cgroup", "the cgroup root"),
        ("catalog", "the catalog"),
    ] {
        let scratch = Scratch::new(&format!("non-utf8-{label}"));
        let (base_url, server) = Script::serve(vec![classify("software-change@1")]);
        let odd = |name: &str| {
            let mut bytes = name.as_bytes().to_vec();
            bytes.extend_from_slice(b"-\xff");
            scratch.root.join(OsStr::from_bytes(&bytes))
        };
        let mut workspace = scratch.workspace.clone();
        if label == "workspace" {
            workspace = odd("workspace");
            std::fs::rename(&scratch.workspace, &workspace).unwrap();
        }
        let mut catalog = scratch.catalog(&base_url);
        if label == "catalog" {
            let moved = odd("catalog.toml");
            std::fs::rename(&catalog, &moved).unwrap();
            catalog = moved;
        }
        let state = scratch.root.join("state");
        let mut args: Vec<OsString> = vec!["--state".into(), state.clone().into()];
        if label == "cgroup" {
            args.extend(["--cgroup-root".into(), odd("cgroup").into()]);
        }
        args.extend([
            "--workspace".into(),
            workspace.into_os_string(),
            "--test-cmd".into(),
            TEST_CMD.into(),
            "--catalog".into(),
            catalog.into_os_string(),
            "--classifier-model".into(),
            "lab-classifier".into(),
            "--model".into(),
            "lab-agent".into(),
            INTENT.into(),
        ]);
        let output = scratch.run(&args);
        let calls = server.finish();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stdout}\n{stderr}");
        assert!(
            calls.is_empty(),
            "{label}: no model is called; the models were called {calls:?}\n{stderr}"
        );
        assert!(
            stderr.contains(what) && stderr.contains("is not UTF-8"),
            "{label}: the refusal names {what}: {stderr}"
        );
        assert!(
            !state.exists(),
            "{label}: no store opened the state directory"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fixtures.

/// A scratch directory under `CARGO_TARGET_TMPDIR`: the home, the Codex home, the protocol store
/// and a git workspace with one failing check; removed on drop.
struct Scratch {
    root: PathBuf,
    workspace: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("run-state-{label}-{}-{nanos}", std::process::id()));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(root.join("home")).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        let scratch = Self { root, workspace };
        scratch.git(&["init", "--quiet", "--initial-branch=main"]);
        scratch.git(&["config", "user.name", "Fixture Author"]);
        scratch.git(&["config", "user.email", "fixture@example.invalid"]);
        scratch.git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(scratch.workspace.join("check.txt"), "broken\n").unwrap();
        scratch.git(&["add", "--all"]);
        scratch.git(&["commit", "--quiet", "--message", "a failing check"]);
        scratch
    }

    fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.workspace)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// The fixture catalog with its endpoints at `base_url`.
    fn catalog(&self, base_url: &str) -> PathBuf {
        let path = self.root.join("catalog.toml");
        std::fs::write(&path, CATALOG.replace("http://127.0.0.1:1/v1", base_url)).unwrap();
        path
    }

    fn run<A: AsRef<std::ffi::OsStr>>(&self, args: &[A]) -> Output {
        let home = self.root.join("home");
        Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
            .arg("run")
            .args(["--confinement", "none"])
            .args(args)
            .env("HOME", &home)
            .env("CODEX_HOME", &home)
            .env("XDG_DATA_HOME", &home)
            .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
            .env_remove("RUST_LOG")
            .output()
            .expect("run b10x-loom")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// One scripted model answer: a forced tool call in the Chat Completions stream format the
/// recorded fixtures of `catalog_route.rs` use.
fn tool_call(model: &str, name: &str, arguments: &Value) -> Vec<u8> {
    let chunk = |delta: Value, finish: Value| {
        json!({
            "id": format!("chatcmpl-call-{name}"),
            "object": "chat.completion.chunk",
            "model": model,
            "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]
        })
    };
    let events = [
        chunk(
            json!({"role": "assistant", "tool_calls": [{"index": 0, "id": format!("call-{name}"),
                "type": "function", "function": {"name": name, "arguments": ""}}]}),
            Value::Null,
        ),
        chunk(
            json!({"tool_calls": [{"index": 0, "function": {"arguments": arguments.to_string()}}]}),
            Value::Null,
        ),
        chunk(json!({}), json!("tool_calls")),
        json!({
            "id": format!("chatcmpl-call-{name}"),
            "object": "chat.completion.chunk",
            "model": model,
            "choices": [],
            "usage": {"prompt_tokens": 120, "completion_tokens": 7, "total_tokens": 127}
        }),
    ];
    let mut stream = String::new();
    for event in events {
        stream.push_str(&format!("data: {event}\n\n"));
    }
    stream.push_str("data: [DONE]\n\n");
    stream.into_bytes()
}

fn classify(protocol: &str) -> Vec<u8> {
    tool_call(
        "example/Classifier-Model",
        "pick_protocol",
        &json!({"protocol": protocol, "confidence": 0.99, "reasons": ["changes the repository"]}),
    )
}

fn agent(name: &str, arguments: Value) -> Vec<u8> {
    tool_call("example/Agent-Model", name, &arguments)
}

/// Answers one request per scripted response, in order, on a loopback socket, until told to
/// finish; records the forced tool each request named.
struct Script {
    done: Arc<AtomicBool>,
    handle: JoinHandle<Vec<String>>,
}

impl Script {
    fn serve(responses: Vec<Vec<u8>>) -> (String, Self) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let done = Arc::new(AtomicBool::new(false));
        let finished = Arc::clone(&done);
        let handle = std::thread::spawn(move || {
            let mut sent = Vec::new();
            let mut responses = responses.into_iter();
            while !finished.load(Ordering::SeqCst) {
                let mut socket = match listener.accept() {
                    Ok((socket, _)) => socket,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(error) => panic!("accept: {error}"),
                };
                socket.set_nonblocking(false).unwrap();
                let request = read_request(&mut socket);
                sent.push(tool_named(&request));
                let response = responses
                    .next()
                    .unwrap_or_else(|| panic!("more requests than scripted: {sent:?}"));
                let mut answer = Vec::from(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                );
                answer.extend_from_slice(&response);
                socket.write_all(&answer).expect("written");
                let _ = socket.shutdown(std::net::Shutdown::Both);
            }
            sent
        });
        (base_url, Self { done, handle })
    }

    /// Stops serving and returns the tool each request forced, in order.
    fn finish(self) -> Vec<String> {
        self.done.store(true, Ordering::SeqCst);
        self.handle.join().expect("the script finished")
    }
}

fn read_request(socket: &mut std::net::TcpStream) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = socket.read(&mut buffer).expect("read");
        request.extend_from_slice(&buffer[..read]);
        if read == 0 || complete(&request) {
            break;
        }
    }
    String::from_utf8_lossy(&request).into_owned()
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

/// The forced tool a request names.
fn tool_named(request: &str) -> String {
    let body = request.split_once("\r\n\r\n").map_or("", |(_, body)| body);
    serde_json::from_str::<Value>(body).map_or_else(
        |_| String::new(),
        |body| {
            body["tool_choice"]["function"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        },
    )
}
