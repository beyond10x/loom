//! Acceptance for `story:cli-resume-after-grant`: after `run --state <dir>` left a commission's
//! Suspension in `<dir>` (`story:cli-run-holds-suspension`), `b10x-loom approve <commission>
//! <capability> --state <dir>` writes a grant record (`intake.routing.Grant`) and performs
//! nothing, and `b10x-loom resume <commission> --state <dir>` performs the suspended
//! `repository.merge` once, as a merge hand-off record (`intake.routing.MergeHandoff`) that moves
//! no Git ref, releases the Suspension and runs on. Each refusal of a resume has its own exit
//! status.
//!
//! Every command runs the binary, a separate process. The stores are read back through the stores
//! themselves where they exist (`FileSuspensionStore`, `ResumeContextStore`); the hand-off is read
//! as the JSON document `resume` leaves in `<dir>`, one `merge-*.json` file per handed-off request.
//! The models are the anonymous Chat Completions routes of the fixture catalog
//! (`fixtures/catalog-route/catalog.toml`) at a loopback socket this file opens, scripted in the
//! recorded stream format `run_state.rs` replays; no case reaches the network, a credential or a
//! Codex login (`HOME` and `CODEX_HOME` are an empty scratch directory). Before each command the
//! catalog file the resume context names is rewritten to point at that command's socket.
//!
//! The workspace is a git repository with one failing check (`grep -qx fixed check.txt`, no
//! shell). The agent edits the check, runs the tests, then proposes `repository.merge`, which the
//! frontier lists as needing approval: the run stops `ApprovalRequired` and the commission holds
//! its Suspension.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{CommissionId, SuspensionState};
use b10x_loom_commission::ports::suspension::SuspensionPort;
use b10x_loom_intake_slice::resume::ResumeContextStore;
use loom_governor::FileSuspensionStore;
use serde_json::{Value, json};

const CATALOG: &str = include_str!("fixtures/catalog-route/catalog.toml");
const INTENT: &str = "Make the failing check pass.";
const TEST_CMD: &str = "grep -qx fixed check.txt";

/// The exit status of a resume refused `NothingSuspended`.
const NOTHING_SUSPENDED: i32 = 10;
/// The exit status of a resume refused `NotGranted`.
const NOT_GRANTED: i32 = 16;

#[test]
fn resume_after_approve_merges_once() {
    let scratch = Scratch::new("merges-once");
    let held = scratch.suspend();

    // Approval writes the grant and performs nothing.
    let approved = scratch.command(&[
        "approve",
        &held.commission,
        "repository.merge",
        "--state",
        held.state_text(),
    ]);
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));
    assert_eq!(scratch.files(&held.state, "grant-"), 1, "one grant record");
    assert_eq!(
        scratch.files(&held.state, "merge-"),
        0,
        "approve merges nothing"
    );
    assert_eq!(
        held.suspension_state(),
        Some(SuspensionState::Pending),
        "approve leaves the Suspension as it was"
    );

    // The resume performs the merge as a hand-off, releases the Suspension and runs on: the agent
    // is asked again after the merge.
    let refs = scratch.refs();
    let (base_url, server) = Script::serve(vec![
        agent("select_action", json!({"action": "tests.run"})),
        agent("action_arguments", json!({})),
    ]);
    scratch.catalog(&base_url);
    let resumed = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    let calls = server.finish();
    let stdout = String::from_utf8_lossy(&resumed.stdout);
    assert_eq!(resumed.status.code(), Some(0), "{}", shown(&resumed));
    assert_eq!(
        calls,
        ["select_action", "action_arguments"],
        "the run went on after the merge, and classified nothing\n{}",
        shown(&resumed)
    );
    assert!(
        stdout
            .lines()
            .any(|line| line.starts_with("step 1: repository.merge")),
        "the first step is the merge\n{stdout}"
    );
    assert_eq!(
        stdout.lines().last(),
        Some("stopped: ApprovalRequired (repository.merge)"),
        "{}",
        shown(&resumed)
    );
    assert_eq!(held.suspension_state(), None, "the Suspension is released");

    // Exactly one hand-off, for the granted request, naming the workspace and its HEAD; no Git ref
    // moved.
    let handoffs = scratch.documents(&held.state, "merge-");
    assert_eq!(handoffs.len(), 1, "one merge hand-off: {handoffs:?}");
    let handoff = &handoffs[0];
    assert_eq!(handoff["commission_id"], json!(held.commission));
    assert_eq!(handoff["action_request_id"], json!(held.action_request_id));
    assert_eq!(handoff["workspace"], json!(held.workspace));
    assert_eq!(handoff["head"], json!(scratch.head()));
    assert_eq!(scratch.refs(), refs, "the resume moved no Git ref");

    // A second resume finds nothing suspended and performs nothing again.
    let (base_url, server) = Script::serve(vec![]);
    scratch.catalog(&base_url);
    let again = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    let calls = server.finish();
    assert_eq!(
        again.status.code(),
        Some(NOTHING_SUSPENDED),
        "{}",
        shown(&again)
    );
    assert!(
        String::from_utf8_lossy(&again.stderr).contains("NothingSuspended"),
        "{}",
        shown(&again)
    );
    assert!(calls.is_empty(), "no model is called: {calls:?}");
    assert_eq!(
        scratch.files(&held.state, "merge-"),
        1,
        "still one hand-off"
    );
}

#[test]
fn resume_without_approve_is_not_granted_and_merges_nothing() {
    let scratch = Scratch::new("not-granted");
    let held = scratch.suspend();
    let (base_url, server) = Script::serve(vec![]);
    scratch.catalog(&base_url);
    let resumed = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    let calls = server.finish();
    assert_eq!(
        resumed.status.code(),
        Some(NOT_GRANTED),
        "{}",
        shown(&resumed)
    );
    assert!(
        String::from_utf8_lossy(&resumed.stderr).contains("NotGranted"),
        "{}",
        shown(&resumed)
    );
    assert!(calls.is_empty(), "no model is called: {calls:?}");
    assert_eq!(
        scratch.files(&held.state, "merge-"),
        0,
        "nothing handed off"
    );
    assert_eq!(held.suspension_state(), Some(SuspensionState::Pending));
}

#[test]
fn a_grant_for_another_request_does_not_allow_this_one() {
    let scratch = Scratch::new("other-request");
    let held = scratch.suspend();
    let approved = scratch.command(&[
        "approve",
        &held.commission,
        "repository.merge",
        "--state",
        held.state_text(),
    ]);
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));

    // The grant names another request than the one the Suspension holds.
    let grant = scratch.paths(&held.state, "grant-");
    assert_eq!(grant.len(), 1, "one grant record");
    let mut document: Value =
        serde_json::from_str(&std::fs::read_to_string(&grant[0]).unwrap()).unwrap();
    assert_eq!(document["action_request_id"], json!(held.action_request_id));
    document["action_request_id"] = json!("00000000-0000-4000-8000-0000000000ff");
    std::fs::write(&grant[0], format!("{document:#}\n")).unwrap();

    let (base_url, server) = Script::serve(vec![]);
    scratch.catalog(&base_url);
    let resumed = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    let calls = server.finish();
    assert_eq!(
        resumed.status.code(),
        Some(NOT_GRANTED),
        "{}",
        shown(&resumed)
    );
    assert!(calls.is_empty(), "no model is called: {calls:?}");
    assert_eq!(
        scratch.files(&held.state, "merge-"),
        0,
        "nothing handed off"
    );
    assert_eq!(held.suspension_state(), Some(SuspensionState::Pending));
}

#[test]
fn approve_refuses_another_capability_and_a_commission_with_nothing_suspended() {
    let scratch = Scratch::new("approve-refusals");
    let held = scratch.suspend();

    let other = scratch.command(&[
        "approve",
        &held.commission,
        "repository.edit",
        "--state",
        held.state_text(),
    ]);
    assert_ne!(other.status.code(), Some(0), "{}", shown(&other));
    assert_ne!(other.status.code(), None, "{}", shown(&other));

    let nobody = scratch.command(&[
        "approve",
        "00000000-0000-4000-8000-0000000000aa",
        "repository.merge",
        "--state",
        held.state_text(),
    ]);
    assert_eq!(
        nobody.status.code(),
        Some(NOTHING_SUSPENDED),
        "{}",
        shown(&nobody)
    );
    assert_eq!(
        scratch.files(&held.state, "grant-"),
        0,
        "no grant is written"
    );
    assert_eq!(held.suspension_state(), Some(SuspensionState::Pending));
}

// ---------------------------------------------------------------------------------------------
// Fixtures.

/// A commission `run --state` left suspended on the merge.
struct Held {
    state: PathBuf,
    commission: String,
    action_request_id: String,
    workspace: String,
}

impl Held {
    fn state_text(&self) -> &str {
        self.state.to_str().unwrap()
    }

    /// The state of the commission's Suspension, read through the store, or `None`.
    fn suspension_state(&self) -> Option<SuspensionState> {
        FileSuspensionStore::open(&self.state)
            .expect("the suspension store opens")
            .get(&CommissionId(Uuid(self.commission.clone())))
            .expect("the suspension store reads")
            .map(|held| held.state)
    }
}

fn shown(output: &Output) -> String {
    format!(
        "status {:?}\n--- stdout\n{}--- stderr\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

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
            .join(format!("resume-{label}-{}-{nanos}", std::process::id()));
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

    /// Runs `run --state` to a Suspension on the merge, and reads what it left.
    fn suspend(&self) -> Held {
        let (base_url, server) = Script::serve(vec![
            classify("software-change@1"),
            agent("select_action", json!({"action": "repository.edit"})),
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
        let catalog = self.catalog(&base_url);
        let state = self.root.join("state");
        let output = self.command(&[
            "run",
            "--confinement",
            "none",
            "--state",
            state.to_str().unwrap(),
            "--workspace",
            self.workspace.to_str().unwrap(),
            "--test-cmd",
            TEST_CMD,
            "--max-steps",
            "3",
            "--catalog",
            catalog.to_str().unwrap(),
            "--classifier-model",
            "lab-classifier",
            "--model",
            "lab-agent",
            INTENT,
        ]);
        server.finish();
        assert_eq!(output.status.code(), Some(0), "{}", shown(&output));
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).lines().last(),
            Some("stopped: ApprovalRequired (repository.merge)"),
            "{}",
            shown(&output)
        );
        let contexts = ResumeContextStore::open(&state)
            .expect("the resume context store opens")
            .list()
            .expect("the resume contexts read back");
        assert_eq!(contexts.len(), 1, "{contexts:?}");
        let commission = contexts[0].commission_id.0.0.clone();
        let workspace = contexts[0].workspace.clone().expect("a workspace is kept");
        let suspension = FileSuspensionStore::open(&state)
            .expect("the suspension store opens")
            .get(&CommissionId(Uuid(commission.clone())))
            .expect("the suspension store reads")
            .expect("the commission holds a Suspension");
        assert_eq!(suspension.state, SuspensionState::Pending);
        assert_eq!(suspension.data.action, "repository.merge");
        Held {
            state,
            commission,
            action_request_id: suspension.data.action_request_id.0.0,
            workspace,
        }
    }

    fn git(&self, args: &[&str]) -> String {
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
        String::from_utf8(output.stdout).unwrap()
    }

    /// The workspace's `HEAD` commit.
    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    /// Every ref of the workspace and what it points at, and what `HEAD` names.
    fn refs(&self) -> String {
        format!(
            "{}{}",
            self.git(&["for-each-ref", "--format=%(refname) %(objectname)"]),
            self.git(&["symbolic-ref", "HEAD"])
        )
    }

    /// The fixture catalog, at the one path every command names, with its endpoints at
    /// `base_url`.
    fn catalog(&self, base_url: &str) -> PathBuf {
        let path = self.root.join("catalog.toml");
        std::fs::write(&path, CATALOG.replace("http://127.0.0.1:1/v1", base_url)).unwrap();
        path
    }

    /// The files in `dir` whose name starts with `prefix` and ends `.json`, in name order.
    fn paths(&self, dir: &Path, prefix: &str) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.starts_with(prefix) && name.ends_with(".json")
            })
            .map(|entry| entry.path())
            .collect();
        paths.sort();
        paths
    }

    fn files(&self, dir: &Path, prefix: &str) -> usize {
        self.paths(dir, prefix).len()
    }

    fn documents(&self, dir: &Path, prefix: &str) -> Vec<Value> {
        self.paths(dir, prefix)
            .iter()
            .map(|path| serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap())
            .collect()
    }

    fn command<A: AsRef<std::ffi::OsStr>>(&self, args: &[A]) -> Output {
        let home = self.root.join("home");
        Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
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
/// finish; records the forced tool each request named. A request past the script is answered
/// with an error status and recorded, so the case sees it.
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
                let answer = match responses.next() {
                    Some(response) => {
                        let mut answer = Vec::from(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                        );
                        answer.extend_from_slice(&response);
                        answer
                    }
                    None => Vec::from(
                        "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ),
                };
                let _ = socket.write_all(&answer);
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
