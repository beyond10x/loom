//! Adversary cases for `story:cli-resume-after-grant` (wave 2026-10-13-w2, U3): what `approve` and
//! `resume` do when the workspace, the model or another process does something the acceptance
//! test does not script.
//!
//! The fixtures are those of `resume_after_grant.rs` (a test binary cannot share them): a git
//! workspace with one failing check, the anonymous Chat Completions routes of the fixture catalog
//! at a loopback socket, `run --state` scripted to a Suspension on `repository.merge`.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, CommissionId, SuspensionKey, SuspensionState, TakeSuspension,
    TakeSuspensionOutcome,
};
use b10x_loom_commission::ports::suspension::SuspensionPort;
use b10x_loom_intake_slice::resume::ResumeContextStore;
use loom_governor::FileSuspensionStore;
use serde_json::{Value, json};

const CATALOG: &str = include_str!("fixtures/catalog-route/catalog.toml");
const INTENT: &str = "Make the failing check pass.";
const TEST_CMD: &str = "grep -qx fixed check.txt";

const NOTHING_SUSPENDED: i32 = 10;
const IN_DOUBT: i32 = 11;

fn approve(scratch: &Scratch, held: &Held) -> Output {
    scratch.command(&[
        "approve",
        &held.commission,
        "repository.merge",
        "--state",
        held.state_text(),
    ])
}

/// A commit someone makes in the workspace after the operator approved the merge is not what the
/// operator approved: the Suspension holds the request at the case revision whose implementation
/// is the HEAD the run left. A resume must either refuse (the case moved) or hand off the HEAD
/// that was approved, never a later one nobody saw.
#[test]
fn a_commit_made_after_approval_is_not_handed_off_as_the_approved_head() {
    let scratch = Scratch::new("head-moved");
    let held = scratch.suspend();
    let approved_head = scratch.head();
    let approved = approve(&scratch, &held);
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));

    // Somebody commits in the workspace between approve and resume.
    std::fs::write(scratch.workspace.join("later.txt"), "not reviewed\n").unwrap();
    scratch.git(&["add", "--all"]);
    scratch.git(&["commit", "--quiet", "--message", "an unreviewed change"]);
    let later_head = scratch.head();
    assert_ne!(approved_head, later_head);

    let (base_url, server) = Script::serve(vec![
        agent("select_action", json!({"action": "tests.run"})),
        agent("action_arguments", json!({})),
    ]);
    scratch.catalog(&base_url);
    let resumed = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    server.finish();

    let handoffs = scratch.documents(&held.state, "merge-");
    for handoff in &handoffs {
        assert_eq!(
            handoff["head"],
            json!(approved_head),
            "the merge hand-off names a HEAD committed after the approval ({later_head}), not \
             the approved {approved_head}\n{}",
            shown(&resumed)
        );
    }
}

/// After the hand-off the case does not move, so the agent may propose the merge again. That
/// proposal is not granted by the old grant; but if it is held as a new Suspension, a second
/// `approve` and `resume` hand the same work off a second time. "Exactly once means one merge
/// record" (decision-blocker:cli-merge-effect): one HEAD is handed off once.
#[test]
fn the_same_head_is_not_handed_off_twice_through_a_re_proposed_merge() {
    let scratch = Scratch::new("re-proposed");
    let held = scratch.suspend();
    let approved = approve(&scratch, &held);
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));

    let (base_url, server) = Script::serve(vec![
        agent("select_action", json!({"action": "repository.merge"})),
        agent("action_arguments", json!({})),
    ]);
    scratch.catalog(&base_url);
    let first = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    server.finish();
    assert_eq!(scratch.files(&held.state, "merge-"), 1, "{}", shown(&first));

    // The operator does what the stop line and `resume --help` (status 0: "reached its human
    // gate again") invite: approve and resume once more.
    let again = approve(&scratch, &held);
    let (base_url, server) = Script::serve(vec![
        agent("select_action", json!({"action": "tests.run"})),
        agent("action_arguments", json!({})),
    ]);
    scratch.catalog(&base_url);
    let second = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    server.finish();

    let handoffs = scratch.documents(&held.state, "merge-");
    let heads: Vec<&Value> = handoffs.iter().map(|handoff| &handoff["head"]).collect();
    let mut distinct = heads.clone();
    distinct.dedup();
    assert_eq!(
        heads.len(),
        distinct.len(),
        "HEAD {} handed off {} times for one commission\n--- first resume\n{}\n--- approve \
         again\n{}\n--- second resume\n{}",
        heads[0],
        heads.len(),
        shown(&first),
        shown(&again),
        shown(&second)
    );
}

/// `resume --help` says status 0 is "the resumed run reached its human gate again
/// (ApprovalRequired)", and `approve --help` says the capability is the one `run` named when it
/// stopped (ApprovalRequired). A caller that follows both and approves after such a resume must
/// find something to approve.
#[test]
fn a_resume_that_stops_approval_required_leaves_something_to_approve() {
    let scratch = Scratch::new("gate-again");
    let held = scratch.suspend();
    let approved = approve(&scratch, &held);
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));

    let (base_url, server) = Script::serve(vec![
        agent("select_action", json!({"action": "tests.run"})),
        agent("action_arguments", json!({})),
    ]);
    scratch.catalog(&base_url);
    let resumed = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    server.finish();
    assert_eq!(resumed.status.code(), Some(0), "{}", shown(&resumed));
    assert_eq!(
        String::from_utf8_lossy(&resumed.stdout).lines().last(),
        Some("stopped: ApprovalRequired (repository.merge)"),
        "{}",
        shown(&resumed)
    );

    let again = approve(&scratch, &held);
    assert_ne!(
        again.status.code(),
        Some(NOTHING_SUSPENDED),
        "the resume exited 0 printing `stopped: ApprovalRequired (repository.merge)`, yet the \
         approval it invites is refused NothingSuspended\n{}",
        shown(&again)
    );
}

/// A resume that took the Suspension and died before the hand-off (constructed here by taking it
/// through the store) leaves the request in doubt: a later resume and a later approve refuse
/// `InDoubt`, and nothing is handed off.
#[test]
fn a_resume_after_a_crash_between_take_and_hand_off_is_in_doubt() {
    let scratch = Scratch::new("in-doubt");
    let held = scratch.suspend();
    let approved = approve(&scratch, &held);
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));
    let taken = FileSuspensionStore::open(&held.state)
        .unwrap()
        .take(TakeSuspension {
            suspension: SuspensionKey {
                commission_id: CommissionId(Uuid(held.commission.clone())),
            },
            action_request_id: ActionRequestId(Uuid(held.action_request_id.clone())),
        });
    assert!(
        matches!(taken, TakeSuspensionOutcome::Taken { .. }),
        "{taken:?}"
    );

    let (base_url, server) = Script::serve(vec![]);
    scratch.catalog(&base_url);
    let resumed = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    let calls = server.finish();
    assert_eq!(resumed.status.code(), Some(IN_DOUBT), "{}", shown(&resumed));
    assert!(calls.is_empty(), "{calls:?}");
    let again = approve(&scratch, &held);
    assert_eq!(again.status.code(), Some(IN_DOUBT), "{}", shown(&again));
    assert_eq!(scratch.files(&held.state, "merge-"), 0);
    assert_eq!(held.suspension_state(), Some(SuspensionState::Resuming));
}

/// Two `resume` processes at once hand the merge off once.
#[test]
fn two_resumes_at_once_hand_off_once() {
    let scratch = Scratch::new("two-at-once");
    let held = scratch.suspend();
    let approved = approve(&scratch, &held);
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));
    let (base_url, server) = Script::serve(vec![
        agent("select_action", json!({"action": "tests.run"})),
        agent("action_arguments", json!({})),
        agent("select_action", json!({"action": "tests.run"})),
        agent("action_arguments", json!({})),
    ]);
    scratch.catalog(&base_url);
    let spawn = || {
        scratch
            .process(&["resume", &held.commission, "--state", held.state_text()])
            .spawn()
            .expect("spawn b10x-loom")
    };
    let (one, two) = (spawn(), spawn());
    let one = one.wait_with_output().unwrap();
    let two = two.wait_with_output().unwrap();
    server.finish();
    let codes = [one.status.code(), two.status.code()];
    assert_eq!(
        scratch.files(&held.state, "merge-"),
        1,
        "{}\n{}",
        shown(&one),
        shown(&two)
    );
    assert!(
        codes.contains(&Some(0)) && codes.iter().any(|code| *code != Some(0)),
        "one resume performs, the other is refused: {codes:?}\n{}\n{}",
        shown(&one),
        shown(&two)
    );
}

/// A grant file that is not a grant, and a workspace that is gone, resume nothing: no hand-off,
/// and the Suspension stays `Pending`.
#[test]
fn a_corrupt_grant_or_a_deleted_workspace_resumes_nothing() {
    let scratch = Scratch::new("corrupt");
    let held = scratch.suspend();
    let approved = approve(&scratch, &held);
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));
    let grant = scratch.paths(&held.state, "grant-");
    assert_eq!(grant.len(), 1);
    let text = std::fs::read_to_string(&grant[0]).unwrap();
    std::fs::write(&grant[0], &text[..text.len() / 2]).unwrap();

    let (base_url, server) = Script::serve(vec![]);
    scratch.catalog(&base_url);
    let resumed = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    let calls = server.finish();
    assert_ne!(resumed.status.code(), Some(0), "{}", shown(&resumed));
    assert!(calls.is_empty(), "{calls:?}");
    assert_eq!(scratch.files(&held.state, "merge-"), 0);
    assert_eq!(held.suspension_state(), Some(SuspensionState::Pending));

    std::fs::write(&grant[0], &text).unwrap();
    std::fs::remove_dir_all(&scratch.workspace).unwrap();
    let (base_url, server) = Script::serve(vec![]);
    scratch.catalog(&base_url);
    let resumed = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    let calls = server.finish();
    assert_ne!(resumed.status.code(), Some(0), "{}", shown(&resumed));
    assert!(calls.is_empty(), "{calls:?}");
    assert_eq!(scratch.files(&held.state, "merge-"), 0);
    assert_eq!(held.suspension_state(), Some(SuspensionState::Pending));
}

// ---------------------------------------------------------------------------------------------
// Fixtures, as `resume_after_grant.rs` has them.

struct Held {
    state: PathBuf,
    commission: String,
    action_request_id: String,
}

impl Held {
    fn state_text(&self) -> &str {
        self.state.to_str().unwrap()
    }

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
            .join(format!("adv-u3-{label}-{}-{nanos}", std::process::id()));
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
        let contexts = ResumeContextStore::open(&state)
            .expect("the resume context store opens")
            .list()
            .expect("the resume contexts read back");
        assert_eq!(contexts.len(), 1, "{contexts:?}");
        let commission = contexts[0].commission_id.0.0.clone();
        let suspension = FileSuspensionStore::open(&state)
            .expect("the suspension store opens")
            .get(&CommissionId(Uuid(commission.clone())))
            .expect("the suspension store reads")
            .expect("the commission holds a Suspension");
        assert_eq!(suspension.state, SuspensionState::Pending);
        Held {
            state,
            commission,
            action_request_id: suspension.data.action_request_id.0.0,
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

    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    fn catalog(&self, base_url: &str) -> PathBuf {
        let path = self.root.join("catalog.toml");
        std::fs::write(&path, CATALOG.replace("http://127.0.0.1:1/v1", base_url)).unwrap();
        path
    }

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

    fn process<A: AsRef<std::ffi::OsStr>>(&self, args: &[A]) -> Command {
        let home = self.root.join("home");
        let mut command = Command::new(env!("CARGO_BIN_EXE_b10x-loom"));
        command
            .args(args)
            .env("HOME", &home)
            .env("CODEX_HOME", &home)
            .env("XDG_DATA_HOME", &home)
            .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
            .env_remove("RUST_LOG")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        command
    }

    fn command<A: AsRef<std::ffi::OsStr>>(&self, args: &[A]) -> Output {
        self.process(args).output().expect("run b10x-loom")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

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
