//! Adversarial cases for `story:mandate-approval-on-resume` at the command line: a grant that
//! names a Mandate approval resumed without `--mandate`, a log whose lines were duplicated and
//! reordered after a crash, and a torn log line. Every command runs the binary, a separate
//! process; the fixtures below are those of `resume_after_grant.rs`, copied.

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

/// The exit status of a resume refused `NotGranted`.
const NOT_GRANTED: i32 = 16;

const MANDATE_LOG: &str = "mandate-approvals.jsonl";
const SUBJECT: &str = "22222222-2222-4222-8222-222222222222";
const APPROVER: &str = "99999999-9999-4999-8999-999999999999";

/// `approve --help`: "The grant names it (mandate_approval), and only `resume --mandate` clears
/// it." A grant `approve --mandate` wrote names a Mandate approval; a `resume` without `--mandate`
/// must not perform the held request on it, or Mandate is bypassed by leaving out one flag, and
/// the approval stays unconsumed in the log.
#[test]
fn adversary_a_resume_without_mandate_does_not_perform_a_grant_naming_a_mandate_approval() {
    let scratch = Scratch::new("adv-plain-resume");
    let held = scratch.suspend();
    let host = scratch.mandate_host(3600);
    let approved = approve_with(&scratch, &held, Some(&host));
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));
    let grants = scratch.documents(&held.state, "grant-");
    assert!(
        grants[0].get("mandate_approval").is_some(),
        "the grant names its Mandate approval: {grants:?}"
    );

    let resumed = resume_with(&scratch, &held, None, script_after_merge());
    let handoffs = scratch.files(&held.state, "merge-");
    assert_eq!(
        (resumed.status.code(), handoffs),
        (Some(NOT_GRANTED), 0),
        "a resume without --mandate performed a grant that names Mandate approval {} \
         (handed off {handoffs}, the approval log holds {} entries and none consumes it)\n{}",
        grants[0]["mandate_approval"],
        scratch.mandate_log(&held.state).len(),
        shown(&resumed)
    );
}

/// After a crash that kept the consumption and performed nothing, the log's lines are duplicated
/// and reordered (the consumption first, the approval twice after it); the same approval still
/// does not clear, and nothing is performed.
#[test]
fn adversary_a_reordered_and_duplicated_log_does_not_replay_a_consumed_approval() {
    let scratch = Scratch::new("adv-reordered");
    let held = scratch.suspend();
    let host = scratch.mandate_host(3600);
    let approved = approve_with(&scratch, &held, Some(&host));
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));
    let before = scratch.root.join("state-before-resume");
    copy_dir(&held.state, &before);

    let first = resume_with(&scratch, &held, Some(&host), script_after_merge());
    assert_eq!(first.status.code(), Some(0), "{}", shown(&first));
    let text = std::fs::read_to_string(held.state.join(MANDATE_LOG)).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    std::fs::write(
        before.join(MANDATE_LOG),
        format!("{}\n{}\n{}\n", lines[1], lines[0], lines[0]),
    )
    .unwrap();

    let crashed = Held {
        state: before,
        commission: held.commission.clone(),
        action_request_id: held.action_request_id.clone(),
        workspace: held.workspace.clone(),
    };
    assert_eq!(crashed.suspension_state(), Some(SuspensionState::Pending));
    let second = resume_with(&scratch, &crashed, Some(&host), Vec::new());
    assert_eq!(
        (second.status.code(), scratch.files(&crashed.state, "merge-")),
        (Some(NOT_GRANTED), 0),
        "a consumed approval cleared again over a reordered, duplicated log\n{}",
        shown(&second)
    );
    assert_eq!(crashed.suspension_state(), Some(SuspensionState::Pending));
}

/// A torn last line (a write cut short by hand or by a filesystem that does not honour the rename)
/// makes the log unreadable: the resume is refused NotGranted and performs nothing, and the
/// approval is not consumed.
#[test]
fn adversary_a_torn_log_line_resumes_nothing() {
    let scratch = Scratch::new("adv-torn");
    let held = scratch.suspend();
    let host = scratch.mandate_host(3600);
    let approved = approve_with(&scratch, &held, Some(&host));
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));
    let path = held.state.join(MANDATE_LOG);
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("{\"kind\":\"ApprovedDecisionRecorded\",\"appro");
    std::fs::write(&path, &text).unwrap();

    let head = scratch.head();
    let refs = scratch.refs();
    let resumed = resume_with(&scratch, &held, Some(&host), Vec::new());
    assert_eq!(scratch.head(), head, "no commit");
    assert_eq!(scratch.refs(), refs, "no Git ref moved");
    assert_eq!(
        (resumed.status.code(), scratch.files(&held.state, "merge-")),
        (Some(NOT_GRANTED), 0),
        "a torn approval log did not refuse the resume\n{}",
        shown(&resumed)
    );
    assert_eq!(held.suspension_state(), Some(SuspensionState::Pending));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        text,
        "nothing was appended"
    );
}

/// The resumed agent's answers after the merge: it runs the tests, and the run stops at the
/// merge's gate at the HEAD it handed off.
fn script_after_merge() -> Vec<Vec<u8>> {
    vec![
        agent("select_action", json!({"action": "tests.run"})),
        agent("action_arguments", json!({})),
    ]
}

fn approve_with(scratch: &Scratch, held: &Held, mandate: Option<&Path>) -> Output {
    let mut args = vec![
        "approve",
        &held.commission,
        "repository.merge",
        "--state",
        held.state_text(),
    ];
    if let Some(mandate) = mandate {
        args.extend(["--mandate", mandate.to_str().unwrap()]);
    }
    scratch.command(&args)
}

fn resume_with(
    scratch: &Scratch,
    held: &Held,
    mandate: Option<&Path>,
    script: Vec<Vec<u8>>,
) -> Output {
    let (base_url, server) = Script::serve(script);
    scratch.catalog(&base_url);
    let mut args = vec!["resume", &held.commission, "--state", held.state_text()];
    if let Some(mandate) = mandate {
        args.extend(["--mandate", mandate.to_str().unwrap()]);
    }
    let output = scratch.command(&args);
    server.finish();
    output
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
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
        self.suspend_with(3)
    }

    /// Runs `run`, with `--state <root>/state` when `state`, for `--max-steps max_steps`: the
    /// agent edits the check and runs the tests, then answers `last` (the merge, or the tests
    /// again).
    fn run(&self, max_steps: usize, state: bool, last: &str) -> Output {
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
            agent("select_action", json!({"action": last})),
            agent("action_arguments", json!({})),
        ]);
        let catalog = self.catalog(&base_url);
        let state_dir = self.root.join("state");
        let max_steps = max_steps.to_string();
        let mut args = vec!["run", "--confinement", "none"];
        if state {
            args.extend(["--state", state_dir.to_str().unwrap()]);
        }
        args.extend([
            "--workspace",
            self.workspace.to_str().unwrap(),
            "--test-cmd",
            TEST_CMD,
            "--max-steps",
            &max_steps,
            "--catalog",
            catalog.to_str().unwrap(),
            "--classifier-model",
            "lab-classifier",
            "--model",
            "lab-agent",
            INTENT,
        ]);
        let output = self.command(&args);
        server.finish();
        output
    }

    /// Runs `run --state` to a Suspension on the merge, with a step budget of `max_steps`, and
    /// reads what it left.
    fn suspend_with(&self, max_steps: usize) -> Held {
        let state = self.root.join("state");
        let output = self.run(max_steps, true, "repository.merge");
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

    /// A host-trusted Mandate file (`loom.authority.MandateHost`) whose approvals last `seconds`:
    /// the subject holds `maintainer` on the case for `repository.merge` and `tests.run`, and
    /// `repository.merge` needs an approval.
    fn mandate_host(&self, seconds: i64) -> PathBuf {
        let path = self.root.join(format!("mandate-{seconds}.json"));
        let host = json!({
            "verified": {
                "subject": SUBJECT,
                "organization": "11111111-1111-4111-8111-111111111111",
                "audience": "loom",
                "credential": "77777777-7777-4777-8777-777777777777",
                "correlation": "correlation-resume"
            },
            "approver": APPROVER,
            "approver_policy": "40404040-4040-4040-8040-404040404040",
            "relation": "maintainer",
            "capabilities": ["repository.merge", "tests.run"],
            "approval_required": ["repository.merge"],
            "approval_seconds": seconds
        });
        std::fs::write(&path, format!("{host:#}\n")).unwrap();
        path
    }

    /// The entries of `dir`'s Mandate approval log, in order.
    fn mandate_log(&self, dir: &Path) -> Vec<Value> {
        match std::fs::read_to_string(dir.join(MANDATE_LOG)) {
            Ok(text) => text
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect(),
            Err(_) => Vec::new(),
        }
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
