//! Acceptance for `story:cli-catalog-model-route`: with `--catalog <PATH>`, `b10x-loom run` builds
//! the classifier (`--classifier-model`) and the agent (`--model`) from route aliases of an llm
//! catalog, through llm's catalog-to-`Model` function; without it both flags name Codex models as
//! before. An unknown alias or an unreadable catalog stops before any model call, naming the alias
//! or the file.
//!
//! Every case runs the binary. The fixture catalog (`fixtures/catalog-route/catalog.toml`) serves
//! anonymous Chat Completions models at a loopback socket this file opens; the test writes that
//! socket's port into a copy of the catalog. The socket replays recorded responses
//! (`fixtures/catalog-route/*.sse`) and records what it was sent; no case reaches the network, a
//! credential or a Codex login (`HOME` and `CODEX_HOME` are an empty scratch directory). "No
//! model call" is read twice: the context report counts zero model calls, and the socket accepted
//! no connection.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

const CATALOG: &str = include_str!("fixtures/catalog-route/catalog.toml");
const CLASSIFY: &[u8] = include_bytes!("fixtures/catalog-route/classify.sse");
const SELECT: &[u8] = include_bytes!("fixtures/catalog-route/select.sse");
const ARGUMENTS: &[u8] = include_bytes!("fixtures/catalog-route/arguments.sse");
const QUERY: &str = "need to know the current time";

/// Both aliases resolve: the classification goes to the classifier route's upstream model and both
/// agent requests to the agent route's, each through the Chat Completions port the catalog
/// declares, and the recorded run completes.
#[test]
fn both_flags_take_a_catalog_route_alias() {
    let scratch = Scratch::new("resolves");
    let (base_url, server) = serve(vec![CLASSIFY, SELECT, ARGUMENTS]);
    let catalog = scratch.catalog(&base_url);
    let output = scratch.run(&[
        "--catalog",
        catalog.to_str().unwrap(),
        "--classifier-model",
        "lab-classifier",
        "--model",
        "lab-agent",
        QUERY,
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stdout}\n{stderr}");
    assert!(
        stdout.contains("picked system-query@1"),
        "{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("stopped: Completed (answered)"),
        "{stdout}\n{stderr}"
    );
    let sent = server.join().expect("the server finished");
    let calls: Vec<(String, String)> = sent
        .iter()
        .map(|request| {
            let (head, body) = request.split_once("\r\n\r\n").expect("a whole request");
            assert!(head.starts_with("POST /v1/chat/completions "), "{head}");
            assert!(
                !head.to_ascii_lowercase().contains("authorization"),
                "{head}"
            );
            let body: Value = serde_json::from_str(body).expect("a JSON body");
            (
                body["model"].as_str().unwrap_or_default().to_owned(),
                body["tool_choice"]["function"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            )
        })
        .collect();
    assert_eq!(
        calls,
        [
            ("example/Classifier-Model".into(), "pick_protocol".into()),
            ("example/Agent-Model".into(), "select_action".into()),
            ("example/Agent-Model".into(), "action_arguments".into()),
        ]
    );
}

/// Without `--catalog` both flags still name Codex models: the run asks the Codex login for its
/// token, finds none in the empty home, and never opens the loopback socket.
#[test]
fn a_codex_model_name_without_a_catalog_is_unchanged() {
    let scratch = Scratch::new("codex");
    let listener = idle();
    let report = scratch.root.join("report.json");
    let output = scratch.run(&[
        "--context-report",
        report.to_str().unwrap(),
        "--classifier-model",
        "lab-classifier",
        "--model",
        "lab-agent",
        QUERY,
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("no Codex access token"), "{stderr}");
    assert_untouched(&listener);
}

/// An alias the catalog does not declare is refused before any model call, naming the alias and
/// the flag that gave it, for either flag. So are an alias whose account needs a credential the
/// command line cannot supply, one whose model cannot take the forced tool call, and one that
/// would fall back to a second target.
#[test]
fn an_unusable_alias_is_refused_naming_it_before_any_model_call() {
    for (label, classifier, agent, flag, alias, reason) in [
        (
            "unknown-agent",
            "lab-classifier",
            "no-such-route",
            "--model",
            "no-such-route",
            "declares no route alias",
        ),
        (
            "unknown-classifier",
            "no-such-route",
            "lab-agent",
            "--classifier-model",
            "no-such-route",
            "declares no route alias",
        ),
        (
            "keyed",
            "lab-classifier",
            "lab-keyed",
            "--model",
            "lab-keyed",
            "credential",
        ),
        (
            "text",
            "lab-text",
            "lab-agent",
            "--classifier-model",
            "lab-text",
            "tool",
        ),
        (
            "fallback",
            "lab-classifier",
            "lab-fallback",
            "--model",
            "lab-fallback",
            "fall",
        ),
    ] {
        let scratch = Scratch::new(label);
        let listener = idle();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let catalog = scratch.catalog(&base_url);
        let report = scratch.root.join("report.json");
        let output = scratch.run(&[
            "--context-report",
            report.to_str().unwrap(),
            "--catalog",
            catalog.to_str().unwrap(),
            "--classifier-model",
            classifier,
            "--model",
            agent,
            QUERY,
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        for named in [alias, flag, reason] {
            assert!(stderr.contains(named), "{label} names {named}: {stderr}");
        }
        assert!(!stdout.contains("picked"), "{label}: {stdout}");
        assert_eq!(model_calls(&report), 0, "{label}");
        assert_untouched(&listener);
    }
}

/// A catalog that cannot be read, or is not a valid llm catalog, is refused before any model
/// call, naming the file.
#[test]
fn an_unreadable_catalog_is_refused_naming_the_file() {
    let scratch = Scratch::new("unreadable");
    let missing = scratch.root.join("missing-catalog.toml");
    let invalid = scratch.root.join("invalid-catalog.toml");
    std::fs::write(&invalid, "format = \"llm.catalog/1\"\nproviders = 3\n").unwrap();
    for path in [&missing, &invalid] {
        let report = scratch.root.join("report.json");
        let output = scratch.run(&[
            "--context-report",
            report.to_str().unwrap(),
            "--catalog",
            path.to_str().unwrap(),
            "--classifier-model",
            "lab-classifier",
            "--model",
            "lab-agent",
            QUERY,
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{}: {stderr}",
            path.display()
        );
        assert!(
            stderr.contains(path.to_str().unwrap()),
            "names {}: {stderr}",
            path.display()
        );
        assert!(!stdout.contains("picked"), "{stdout}");
        assert_eq!(model_calls(&report), 0, "{}", path.display());
    }
}

/// The catalog is a regular file: a named pipe is refused naming it even while a writer holds it
/// open with a valid catalog, which is what `--catalog <(cmd)` hands the run.
#[test]
fn a_catalog_pipe_with_a_writer_is_refused_naming_it() {
    let scratch = Scratch::new("pipe");
    let listener = idle();
    let source = scratch.catalog(&format!("http://{}/v1", listener.local_addr().unwrap()));
    let fifo = scratch.root.join("catalog.fifo");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo")
            .success()
    );
    // `exec` keeps the writer one process: the shell opens the pipe itself, blocking there, then
    // becomes `cat`, so the guard's kill reaches whatever holds the pipe on every path.
    let _writer = Reaped(
        Command::new("sh")
            .args(["-c", "exec cat \"$1\" > \"$2\"", "writer"])
            .arg(&source)
            .arg(&fifo)
            .spawn()
            .expect("a writer"),
    );
    let report = scratch.root.join("report.json");
    let output = scratch.run(&[
        "--context-report",
        report.to_str().unwrap(),
        "--catalog",
        fifo.to_str().unwrap(),
        "--classifier-model",
        "lab-classifier",
        "--model",
        "lab-agent",
        QUERY,
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains(fifo.to_str().unwrap()) && stderr.contains("not a regular file"),
        "{stderr}"
    );
    assert_eq!(model_calls(&report), 0);
    assert_untouched(&listener);
}

/// `run --help` documents the flag.
#[test]
fn run_help_documents_the_catalog_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
        .args(["run", "--help"])
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("--catalog <PATH>"), "{help}");
    assert!(help.contains("route alias"), "{help}");
    assert!(help.contains("regular file"), "{help}");
}

// ---------------------------------------------------------------------------------------------
// Fixtures.

/// A scratch directory under `CARGO_TARGET_TMPDIR`, used as the home, the Codex home and the
/// protocol store; removed on drop.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "catalog-route-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("home")).unwrap();
        Self { root }
    }

    /// The fixture catalog with its endpoints at `base_url`.
    fn catalog(&self, base_url: &str) -> PathBuf {
        let path = self.root.join("catalog.toml");
        std::fs::write(&path, CATALOG.replace("http://127.0.0.1:1/v1", base_url)).unwrap();
        path
    }

    fn run(&self, args: &[&str]) -> Output {
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

/// A child process killed and reaped on drop, so a failed assertion or a panic leaves none alive.
struct Reaped(Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Answers one request per recorded response, in order, on a loopback socket, and returns what
/// each request carried.
fn serve(responses: Vec<&'static [u8]>) -> (String, JoinHandle<Vec<String>>) {
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
            answer.extend_from_slice(response);
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

/// A loopback socket nothing should connect to.
fn idle() -> TcpListener {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}

fn assert_untouched(listener: &TcpListener) {
    match listener.accept() {
        Err(error) if error.kind() == ErrorKind::WouldBlock => {}
        other => panic!("the model endpoint was contacted: {other:?}"),
    }
}

fn model_calls(report: &Path) -> u64 {
    let report: Value = serde_json::from_slice(&std::fs::read(report).expect("a context report"))
        .expect("a JSON report");
    report["model_calls"].as_u64().expect("a model call count")
}
