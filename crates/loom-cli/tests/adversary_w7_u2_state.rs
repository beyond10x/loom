//! Adversary cases for `story:cli-run-holds-suspension` (wave 2026-10-13-w2, U2): `run --state`
//! over a directory that is read-only, a file, already used, or given to a run that ends without
//! a Suspension; and `run` without `--state`.
//!
//! The harness is `run_state.rs`'s, copied: the binary runs as a separate process against scripted
//! Chat Completions routes on a loopback socket, with `HOME` and `CODEX_HOME` an empty scratch
//! directory, and the state directory is read back through the stores.

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
use loom_governor::{FileCaseStore, FileSuspensionStore};
use serde_json::{Value, json};

const CATALOG: &str = include_str!("fixtures/catalog-route/catalog.toml");
const INTENT: &str = "Make the failing check pass.";
const TEST_CMD: &str = "grep -qx fixed check.txt";

/// The script `run_state.rs` replays: classify, edit, run the tests, propose the merge.
fn merge_script() -> Vec<Vec<u8>> {
    vec![
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
    ]
}

fn run_args<'a>(
    state: Option<&'a str>,
    workspace: &'a str,
    catalog: &'a str,
    steps: &'a str,
) -> Vec<&'a str> {
    let mut args = Vec::new();
    if let Some(state) = state {
        args.extend(["--state", state]);
    }
    args.extend([
        "--workspace",
        workspace,
        "--test-cmd",
        TEST_CMD,
        "--max-steps",
        steps,
        "--catalog",
        catalog,
        "--classifier-model",
        "lab-classifier",
        "--model",
        "lab-agent",
        INTENT,
    ]);
    args
}

fn files_named(dir: &Path, prefix: &str) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().starts_with(prefix))
                .count()
        })
        .unwrap_or(0)
}

/// The code's own comment (`crates/loom-cli/src/main.rs`, before `FileCaseStore::open`): "The state
/// directory is opened before any model call: one that cannot be used stops here." A directory an
/// earlier `run --state` used, and which is now read-only, opens (its `.lock` is there and
/// writable), so the classifier is called and the run fails only when the case is written.
#[test]
fn a_read_only_state_directory_a_run_already_used_stops_before_any_model_call() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("read-only");
    let state = scratch.root.join("state");
    // What an earlier run left: the stores opened over it.
    FileCaseStore::open(&state).expect("the case store opens");
    ResumeContextStore::open(&state).expect("the resume context store opens");
    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o555)).unwrap();
    // Running as a user the mode binds (not root).
    let probe = state.join(".probe");
    if std::fs::write(&probe, "").is_ok() {
        let _ = std::fs::remove_file(&probe);
        std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("skipped: the directory mode does not bind this user");
        return;
    }
    let (base_url, server) = Script::serve(merge_script());
    let catalog = scratch.catalog(&base_url);
    let output = scratch.run(&run_args(
        state.to_str(),
        scratch.workspace.to_str().unwrap(),
        catalog.to_str().unwrap(),
        "6",
    ));
    let calls = server.finish();
    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o755)).unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_ne!(output.status.code(), Some(0), "{stdout}\n{stderr}");
    assert!(
        calls.is_empty(),
        "an unusable state directory must stop the run before any model call; the models were \
         called {calls:?}\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// A state path that is a regular file stops the run before any model call, and the file is
/// left as it was.
#[test]
fn a_state_path_that_is_a_file_stops_before_any_model_call() {
    let scratch = Scratch::new("file");
    let state = scratch.root.join("state");
    std::fs::write(&state, "not a directory\n").unwrap();
    let (base_url, server) = Script::serve(merge_script());
    let catalog = scratch.catalog(&base_url);
    let output = scratch.run(&run_args(
        state.to_str(),
        scratch.workspace.to_str().unwrap(),
        catalog.to_str().unwrap(),
        "6",
    ));
    let calls = server.finish();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(calls.is_empty(), "{calls:?}");
    assert!(stderr.contains("the state directory"), "{stderr}");
    assert_eq!(
        std::fs::read_to_string(&state).unwrap(),
        "not a directory\n"
    );
}

/// A second `run --state d` over a directory already holding a Pending Suspension and a case
/// keeps both commissions: two cases, two Pending Suspensions, two contexts, the first unchanged.
#[test]
fn a_second_run_over_a_held_directory_overwrites_nothing() {
    let first = Scratch::new("second-a");
    let second = Scratch::new("second-b");
    let state = first.root.join("state");
    for scratch in [&first, &second] {
        let (base_url, server) = Script::serve(merge_script());
        let catalog = scratch.catalog(&base_url);
        let output = scratch.run(&run_args(
            state.to_str(),
            scratch.workspace.to_str().unwrap(),
            catalog.to_str().unwrap(),
            "6",
        ));
        server.finish();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(0), "{stdout}\n{stderr}");
        assert_eq!(
            stdout.lines().last(),
            Some("stopped: ApprovalRequired (repository.merge)"),
            "{stdout}\n{stderr}"
        );
    }
    let contexts = ResumeContextStore::open(&state).unwrap().list().unwrap();
    assert_eq!(contexts.len(), 2, "{contexts:?}");
    assert_ne!(contexts[0].case_id, contexts[1].case_id);
    let suspensions = FileSuspensionStore::open(&state).unwrap();
    let cases = FileCaseStore::open(&state).unwrap();
    for context in &contexts {
        let commission = CommissionId(Uuid(context.commission_id.0.0.clone()));
        let held = suspensions.get(&commission).unwrap().expect("a Suspension");
        assert_eq!(held.state, SuspensionState::Pending);
        assert_eq!(held.data.case_id.0, context.case_id.0);
        let case = loom_governor::FallibleCaseStore::get(
            &cases,
            &b10x_loom_commission::model::responsibility::CaseId(context.case_id.0.clone()),
        )
        .unwrap()
        .expect("the case");
        assert_eq!(case.revision, held.data.expected_case_revision);
    }
    assert_eq!(files_named(&state, "suspension-"), 2);
    assert_eq!(files_named(&state, "case-"), 2);
}

/// The gap the implementor noted: the resume context is kept before the commission runs, so a run
/// that ends without a Suspension (here on its step budget) leaves a context nothing can resume,
/// indistinguishable by its store from one that can.
#[test]
fn a_run_ending_without_a_suspension_leaves_no_resume_context() {
    let scratch = Scratch::new("budget");
    let state = scratch.root.join("state");
    let (base_url, server) = Script::serve(merge_script());
    let catalog = scratch.catalog(&base_url);
    let output = scratch.run(&run_args(
        state.to_str(),
        scratch.workspace.to_str().unwrap(),
        catalog.to_str().unwrap(),
        "1",
    ));
    server.finish();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stdout.lines().last(),
        Some("stopped: StepBudget"),
        "{stdout}\n{stderr}"
    );
    assert_eq!(
        files_named(&state, "suspension-"),
        0,
        "no Suspension on a step budget"
    );
    let contexts = ResumeContextStore::open(&state).unwrap().list().unwrap();
    assert!(
        contexts.is_empty(),
        "a run that held no Suspension left {} resume context(s): {contexts:?}",
        contexts.len()
    );
}

/// Without `--state`, nothing the state stores write appears anywhere under the scratch root, the
/// working directory or the home.
#[test]
fn run_without_state_writes_no_store_file() {
    let scratch = Scratch::new("no-state");
    let (base_url, server) = Script::serve(merge_script());
    let catalog = scratch.catalog(&base_url);
    let output = scratch.run(&run_args(
        None,
        scratch.workspace.to_str().unwrap(),
        catalog.to_str().unwrap(),
        "6",
    ));
    server.finish();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout.lines().last(),
        Some("stopped: ApprovalRequired (repository.merge)")
    );
    let mut found = Vec::new();
    let mut pending = vec![scratch.root.clone()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == ".git" {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                pending.push(entry.path());
            } else if name == ".lock"
                || name.starts_with("resume-")
                || name.starts_with("suspension-")
                || name.starts_with("case-")
                || name == "observations.json"
            {
                found.push(entry.path());
            }
        }
    }
    assert!(found.is_empty(), "{found:?}");
}

// ---------------------------------------------------------------------------------------------
// Fixtures, as in run_state.rs.

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
            .join(format!("adv-u2-{label}-{}-{nanos}", std::process::id()));
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

    fn catalog(&self, base_url: &str) -> PathBuf {
        let path = self.root.join(format!(
            "catalog-{}.toml",
            base_url
                .rsplit(':')
                .next()
                .unwrap_or("0")
                .replace("/v1", "")
        ));
        std::fs::write(&path, CATALOG.replace("http://127.0.0.1:1/v1", base_url)).unwrap();
        path
    }

    fn run(&self, args: &[&str]) -> Output {
        let home = self.root.join("home");
        Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
            .arg("run")
            .args(["--confinement", "none"])
            .args(args)
            .current_dir(&self.root)
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
