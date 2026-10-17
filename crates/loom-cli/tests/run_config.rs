//! Acceptance for `story:cli-run-config`: with `--config <PATH>`, `b10x-loom run` takes the
//! agent's and the classifier's models from a models configuration through `loom_sdk::models`:
//! each role's model is the configured route of an `llm.routes/1` document, with the credential
//! its `secret:` reference names. `--model` and `--classifier-model`, when given, override one
//! role each with a name of the same document. Both models are settled before any model call.
//!
//! Every case runs the binary. The routes document declares one Chat Completions endpoint at a
//! loopback socket this file opens, authenticated by a bearer token read from an environment
//! variable; the socket replays the recorded responses of `fixtures/catalog-route/*.sse` and
//! records what it was sent. No case reaches the network or a Codex login (`HOME` and
//! `CODEX_HOME` are an empty scratch directory). "No model call" is read twice: the context
//! report counts zero model calls, and the socket accepted no connection.
//!
//! Acceptance for `story:cli-run-config-resume`: a case `run --config --state` kept, stopped at
//! an approval, resumes on the configuration's routes; a resume whose configuration is gone, no
//! longer serves the agent, or whose credential does not resolve exits 1 before any model call.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use b10x_loom_intake_slice::resume::ResumeContextStore;
use serde_json::{Value, json};

const CLASSIFY: &[u8] = include_bytes!("fixtures/catalog-route/classify.sse");
const SELECT: &[u8] = include_bytes!("fixtures/catalog-route/select.sse");
const ARGUMENTS: &[u8] = include_bytes!("fixtures/catalog-route/arguments.sse");
const QUERY: &str = "need to know the current time";
const TOKEN_VARIABLE: &str = "LOOM_CLI_RUN_CONFIG_TOKEN";
const TOKEN: &str = "run-config-test-token";

/// The routes document, its endpoint at `BASE_URL`: three routes that take a forced tool call
/// and one, `text`, that cannot.
const ROUTES: &str = "format: llm.routes/1
endpoints:
  lab:
    base_url: BASE_URL
    protocol: chat-completions
    auth: {kind: bearer, secret: env:LOOM_CLI_RUN_CONFIG_TOKEN}
routes:
  judge: lab/example/Classifier-Model
  worker: lab/example/Agent-Model
  other: lab/example/Other-Model
  text:
    model: lab/example/Text-Model
    capabilities: {tools: false, tool_choice: false, temperature: true, top_p: true,
                   reasoning_efforts: [], context_window: 8192, max_output_tokens: 2048}
";

const BOTH_ROLES: &str = "routes: routes.yaml
roles:
  - {role: agent, model: worker}
  - {role: classifier, model: judge}
";

/// The classification goes to the classifier's route and both agent requests to the agent's, each
/// with the bearer token of the variable the routes document names, and the run completes.
#[test]
fn run_config_uses_the_routes_of_each_role() {
    let scratch = Scratch::new("roles");
    let (base_url, server) = serve(vec![CLASSIFY, SELECT, ARGUMENTS]);
    let config = scratch.config(&base_url, BOTH_ROLES);
    let output = scratch.run(&["--config", config.to_str().unwrap(), QUERY]);
    assert_completed(&output);
    let sent = server.join().expect("the server finished");
    assert_eq!(
        calls(&sent),
        [
            ("example/Classifier-Model".into(), "pick_protocol".into()),
            ("example/Agent-Model".into(), "select_action".into()),
            ("example/Agent-Model".into(), "action_arguments".into()),
        ]
    );
    for request in &sent {
        let head = request.split_once("\r\n\r\n").expect("a whole request").0;
        assert!(
            head.lines()
                .any(|line| line.eq_ignore_ascii_case(&format!("authorization: Bearer {TOKEN}"))),
            "{head}"
        );
    }
}

/// `--model other` with `--config` sends the agent's turns to `other` and keeps the classifier's
/// configured route; `--classifier-model` does the same for the classifier.
#[test]
fn a_flag_overrides_one_role() {
    for (flag, expected) in [
        (
            "--model",
            [
                "example/Classifier-Model",
                "example/Other-Model",
                "example/Other-Model",
            ],
        ),
        (
            "--classifier-model",
            [
                "example/Other-Model",
                "example/Agent-Model",
                "example/Agent-Model",
            ],
        ),
    ] {
        let scratch = Scratch::new("override");
        let (base_url, server) = serve(vec![CLASSIFY, SELECT, ARGUMENTS]);
        let config = scratch.config(&base_url, BOTH_ROLES);
        let output = scratch.run(&["--config", config.to_str().unwrap(), flag, "other", QUERY]);
        assert_completed(&output);
        let sent = server.join().expect("the server finished");
        let models: Vec<String> = calls(&sent).into_iter().map(|(model, _)| model).collect();
        assert_eq!(models, expected, "{flag}");
    }
}

/// A flag fills a role the configuration leaves out.
#[test]
fn a_flag_serves_a_role_the_config_leaves_out() {
    let scratch = Scratch::new("filled");
    let (base_url, server) = serve(vec![CLASSIFY, SELECT, ARGUMENTS]);
    let config = scratch.config(
        &base_url,
        "routes: routes.yaml\nroles:\n  - {role: agent, model: worker}\n",
    );
    let output = scratch.run(&[
        "--config",
        config.to_str().unwrap(),
        "--classifier-model",
        "lab/example/Direct-Model",
        QUERY,
    ]);
    assert_completed(&output);
    let sent = server.join().expect("the server finished");
    let models: Vec<String> = calls(&sent).into_iter().map(|(model, _)| model).collect();
    assert_eq!(
        models,
        [
            "example/Direct-Model",
            "example/Agent-Model",
            "example/Agent-Model"
        ]
    );
}

/// A configuration that declares no `classifier`, run without `--classifier-model`, exits 1
/// naming the role and the flag, and the endpoint receives no request; the same for the agent.
#[test]
fn a_role_neither_configured_nor_given_is_refused_before_any_model_call() {
    for (label, roles, role, flag) in [
        (
            "no-classifier",
            "  - {role: agent, model: worker}\n",
            "classifier",
            "--classifier-model",
        ),
        (
            "no-agent",
            "  - {role: classifier, model: judge}\n",
            "agent",
            "--model",
        ),
    ] {
        let scratch = Scratch::new(label);
        let listener = idle();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let config = scratch.config(&base_url, &format!("routes: routes.yaml\nroles:\n{roles}"));
        let report = scratch.root.join("report.json");
        let output = scratch.run(&[
            "--context-report",
            report.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            QUERY,
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        assert!(
            stderr.contains(&format!("`{role}`")) && stderr.contains(flag),
            "{label}: {stderr}"
        );
        assert!(!stdout.contains("picked"), "{label}: {stdout}");
        assert_eq!(model_calls(&report), 0, "{label}");
        assert_untouched(&listener);
    }
}

/// A role whose model cannot take a forced tool call is refused before any model call, naming
/// the role, whether the configuration or a flag gave the model; so is a flag naming a model the
/// routes document does not serve, naming the flag.
#[test]
fn a_model_without_a_forced_tool_call_is_refused_naming_the_role() {
    for (label, roles, extra, named) in [
        (
            "configured",
            "  - {role: agent, model: worker}\n  - {role: classifier, model: text}\n",
            &[][..],
            &["`classifier`", "text", "forced tool call"][..],
        ),
        (
            "flag",
            "  - {role: agent, model: worker}\n  - {role: classifier, model: judge}\n",
            &["--model", "text"][..],
            &["`agent`", "--model", "text", "forced tool call"][..],
        ),
        (
            "unknown",
            "  - {role: agent, model: worker}\n  - {role: classifier, model: judge}\n",
            &["--model", "nowhere"][..],
            &["--model", "nowhere"][..],
        ),
    ] {
        let scratch = Scratch::new(label);
        let listener = idle();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let config = scratch.config(&base_url, &format!("routes: routes.yaml\nroles:\n{roles}"));
        let report = scratch.root.join("report.json");
        let mut args = vec![
            "--context-report",
            report.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
        ];
        args.extend_from_slice(extra);
        args.push(QUERY);
        let output = scratch.run(&args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        for name in named {
            assert!(stderr.contains(name), "{label} names {name}: {stderr}");
        }
        assert_eq!(model_calls(&report), 0, "{label}");
        assert_untouched(&listener);
    }
}

/// A configuration that cannot be loaded stops the run before any model call, naming the file.
#[test]
fn an_unloadable_config_is_refused_naming_the_file() {
    let scratch = Scratch::new("unloadable");
    let listener = idle();
    let missing = scratch.root.join("missing.yaml");
    let report = scratch.root.join("report.json");
    let output = scratch.run(&[
        "--context-report",
        report.to_str().unwrap(),
        "--config",
        missing.to_str().unwrap(),
        QUERY,
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains(missing.to_str().unwrap()), "{stderr}");
    assert_eq!(model_calls(&report), 0);
    assert_untouched(&listener);
}

/// `--config` and `--catalog` together are refused by the command line.
#[test]
fn config_and_catalog_together_are_refused() {
    let scratch = Scratch::new("catalog");
    let listener = idle();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    let config = scratch.config(&base_url, BOTH_ROLES);
    let output = scratch.run(&[
        "--config",
        config.to_str().unwrap(),
        "--catalog",
        config.to_str().unwrap(),
        QUERY,
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("--config") && stderr.contains("--catalog"),
        "{stderr}"
    );
    assert_untouched(&listener);
}

/// A case started with `run --config` and `--state`, here a relative path, and stopped at an
/// approval keeps the configuration's absolute path and the agent's served name; `resume`, from
/// another directory, continues on the routes the configuration names. The stub endpoint sees
/// both runs' requests, each with the configured credential.
#[test]
fn a_case_kept_with_config_resumes_on_its_configured_models() {
    let scratch = Scratch::new("resume");
    let held = scratch.suspend();
    assert_eq!(
        held.config.as_deref(),
        Some(scratch.root.join("loom.yaml").to_str().unwrap()),
        "the context keeps the configuration's absolute path"
    );
    assert_eq!(
        held.model, "worker",
        "the context keeps the agent's served name"
    );
    assert_eq!(held.catalog, None);
    for request in &held.sent {
        assert!(has_token(request), "{request}");
    }
    let first = calls(&held.sent);
    assert_eq!(
        first[0],
        ("example/Classifier-Model".into(), "pick_protocol".into())
    );
    assert!(
        first[1..]
            .iter()
            .all(|(model, _)| model == "example/Agent-Model"),
        "{first:?}"
    );

    let approved = scratch.command(&held.approve_args());
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));

    let (base_url, server) = Stub::serve(vec![
        tool_call(
            "example/Agent-Model",
            "select_action",
            &json!({"action": "tests.run"}),
        ),
        tool_call("example/Agent-Model", "action_arguments", &json!({})),
    ]);
    scratch.config(&base_url, BOTH_ROLES);
    let resumed = scratch.command(&held.resume_args());
    let sent = server.finish();
    assert_eq!(resumed.status.code(), Some(0), "{}", shown(&resumed));
    assert!(
        String::from_utf8_lossy(&resumed.stdout)
            .lines()
            .any(|line| line.starts_with("step 1: repository.merge")),
        "{}",
        shown(&resumed)
    );
    assert_eq!(
        calls(&sent),
        [
            ("example/Agent-Model".to_owned(), "select_action".to_owned()),
            (
                "example/Agent-Model".to_owned(),
                "action_arguments".to_owned()
            ),
        ],
        "the resume ran on the configured agent route and classified nothing"
    );
    for request in &sent {
        assert!(has_token(request), "{request}");
    }
}

/// A resume whose configuration file is gone exits 1 naming the file and the role, before any
/// model call.
#[test]
fn a_resume_whose_config_is_gone_exits_1_before_any_model_call() {
    let scratch = Scratch::new("resume-gone");
    let held = scratch.suspend();
    let approved = scratch.command(&held.approve_args());
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));
    let listener = idle();
    scratch.config(
        &format!("http://{}/v1", listener.local_addr().unwrap()),
        BOTH_ROLES,
    );
    let config = scratch.root.join("loom.yaml");
    std::fs::remove_file(&config).unwrap();
    let resumed = scratch.command(&held.resume_args());
    assert_refused_naming(&resumed, &config);
    assert_untouched(&listener);
}

/// A resume whose configuration no longer serves the agent's kept name, or whose credential no
/// longer resolves, exits 1 naming the file and the role, before any model call.
#[test]
fn a_resume_whose_config_no_longer_serves_the_role_exits_1_before_any_model_call() {
    let scratch = Scratch::new("resume-unserved");
    let held = scratch.suspend();
    let approved = scratch.command(&held.approve_args());
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));
    let config = scratch.root.join("loom.yaml");
    let listener = idle();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());

    // The routes document no longer declares `worker`.
    scratch.config(&base_url, BOTH_ROLES);
    let routes = std::fs::read_to_string(scratch.root.join("routes.yaml")).unwrap();
    std::fs::write(
        scratch.root.join("routes.yaml"),
        routes.replace("  worker: lab/example/Agent-Model\n", ""),
    )
    .unwrap();
    let resumed = scratch.command(&held.resume_args());
    assert_refused_naming(&resumed, &config);
    assert!(
        String::from_utf8_lossy(&resumed.stderr).contains("worker"),
        "{}",
        shown(&resumed)
    );

    // The routes are back, but the credential they name is not set.
    scratch.config(&base_url, BOTH_ROLES);
    let resumed = scratch.command_with(&held.resume_args(), false);
    assert_refused_naming(&resumed, &config);
    assert!(
        String::from_utf8_lossy(&resumed.stderr).contains("secret"),
        "{}",
        shown(&resumed)
    );
    assert_untouched(&listener);
}

/// `run --help` documents the flag.
#[test]
fn run_help_documents_the_config_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
        .args(["run", "--help"])
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("--config <PATH>"), "{help}");
    assert!(help.contains("llm.routes/1"), "{help}");
    assert!(help.contains("classifier"), "{help}");
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
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("run-config-{label}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(root.join("home")).unwrap();
        Self { root }
    }

    /// Writes the routes document, its endpoint at `base_url`, and the configuration `text`
    /// beside it; returns the configuration's path.
    fn config(&self, base_url: &str, text: &str) -> PathBuf {
        std::fs::write(
            self.root.join("routes.yaml"),
            ROUTES.replace("BASE_URL", base_url),
        )
        .unwrap();
        let path = self.root.join("loom.yaml");
        std::fs::write(&path, text).unwrap();
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
            .env(TOKEN_VARIABLE, TOKEN)
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

fn assert_completed(output: &Output) {
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
}

/// The upstream model and the forced tool of each request, in order.
fn calls(sent: &[String]) -> Vec<(String, String)> {
    sent.iter()
        .map(|request| {
            let (head, body) = request.split_once("\r\n\r\n").expect("a whole request");
            assert!(head.starts_with("POST /v1/chat/completions "), "{head}");
            let body: Value = serde_json::from_str(body).expect("a JSON body");
            (
                body["model"].as_str().unwrap_or_default().to_owned(),
                body["tool_choice"]["function"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            )
        })
        .collect()
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

// ---------------------------------------------------------------------------------------------
// Fixtures of the resume cases.

const INTENT: &str = "Make the failing check pass.";
const TEST_CMD: &str = "grep -qx fixed check.txt";

/// A commission `run --config --state` left suspended on the merge, what its resume context
/// keeps of the models, and the requests the run sent.
struct Held {
    state: PathBuf,
    commission: String,
    config: Option<String>,
    catalog: Option<String>,
    model: String,
    sent: Vec<String>,
}

impl Held {
    fn approve_args(&self) -> Vec<String> {
        [
            "approve",
            &self.commission,
            "repository.merge",
            "--state",
            self.state.to_str().unwrap(),
        ]
        .map(str::to_owned)
        .to_vec()
    }

    fn resume_args(&self) -> Vec<String> {
        [
            "resume",
            &self.commission,
            "--state",
            self.state.to_str().unwrap(),
        ]
        .map(str::to_owned)
        .to_vec()
    }
}

impl Scratch {
    /// Runs `run --config loom.yaml --state state`, both relative to the scratch root, in a git
    /// workspace with one failing check: the agent edits the check, runs the tests and proposes
    /// `repository.merge`, which stops the run `ApprovalRequired`. Reads what the run kept.
    fn suspend(&self) -> Held {
        let workspace = self.root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .args(args)
                .current_dir(&workspace)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .output()
                .expect("run git");
            assert!(output.status.success(), "git {args:?}: {}", shown(&output));
        };
        git(&["init", "--quiet", "--initial-branch=main"]);
        git(&["config", "user.name", "Fixture Author"]);
        git(&["config", "user.email", "fixture@example.invalid"]);
        git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(workspace.join("check.txt"), "broken\n").unwrap();
        git(&["add", "--all"]);
        git(&["commit", "--quiet", "--message", "a failing check"]);

        let agent =
            |name: &str, arguments: Value| tool_call("example/Agent-Model", name, &arguments);
        let (base_url, server) = Stub::serve(vec![
            tool_call(
                "example/Classifier-Model",
                "pick_protocol",
                &json!({"protocol": "software-change@1", "confidence": 0.99,
                        "reasons": ["changes the repository"]}),
            ),
            agent("select_action", json!({"action": "implementation.edit"})),
            agent(
                "action_arguments",
                json!({"files": [{"path": "check.txt", "contents": "fixed\n"}],
                       "message": "fix the check"}),
            ),
            agent("select_action", json!({"action": "tests.run"})),
            agent("action_arguments", json!({})),
            agent("select_action", json!({"action": "repository.merge"})),
            agent("action_arguments", json!({})),
        ]);
        self.config(&base_url, BOTH_ROLES);
        let home = self.root.join("home");
        let output = Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
            .args(["run", "--confinement", "none", "--config", "loom.yaml"])
            .args([
                "--state",
                "state",
                "--workspace",
                workspace.to_str().unwrap(),
            ])
            .args(["--test-cmd", TEST_CMD, "--max-steps", "3", INTENT])
            .current_dir(&self.root)
            .env("HOME", &home)
            .env("CODEX_HOME", &home)
            .env("XDG_DATA_HOME", &home)
            .env(TOKEN_VARIABLE, TOKEN)
            .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
            .env_remove("RUST_LOG")
            .output()
            .expect("run b10x-loom");
        let sent = server.finish();
        assert_eq!(output.status.code(), Some(0), "{}", shown(&output));
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).lines().last(),
            Some("stopped: ApprovalRequired (repository.merge)"),
            "{}",
            shown(&output)
        );
        let state = self.root.join("state");
        let contexts = ResumeContextStore::open(&state)
            .expect("the resume context store opens")
            .list()
            .expect("the resume contexts read back");
        assert_eq!(contexts.len(), 1, "{contexts:?}");
        let context = &contexts[0];
        Held {
            commission: context.commission_id.0.0.clone(),
            config: context.config.clone(),
            catalog: context.catalog.clone(),
            model: context.model.clone(),
            state,
            sent,
        }
    }

    /// Runs the binary with `args`, from the current directory, with the credential set.
    fn command(&self, args: &[String]) -> Output {
        self.command_with(args, true)
    }

    /// Runs the binary with `args`, from the current directory, with the credential set only
    /// when `token`.
    fn command_with(&self, args: &[String], token: bool) -> Output {
        let home = self.root.join("home");
        let mut command = Command::new(env!("CARGO_BIN_EXE_b10x-loom"));
        command
            .args(args)
            .env("HOME", &home)
            .env("CODEX_HOME", &home)
            .env("XDG_DATA_HOME", &home)
            .env_remove(TOKEN_VARIABLE)
            .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
            .env_remove("RUST_LOG");
        if token {
            command.env(TOKEN_VARIABLE, TOKEN);
        }
        command.output().expect("run b10x-loom")
    }
}

/// A refused resume: exit 1, naming the configuration file and the role `agent`.
fn assert_refused_naming(output: &Output, config: &Path) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{}", shown(output));
    assert!(
        stderr.contains(config.to_str().unwrap()),
        "names the file\n{}",
        shown(output)
    );
    assert!(
        stderr.contains("`agent`"),
        "names the role\n{}",
        shown(output)
    );
}

/// Whether `request` carries the configured bearer token.
fn has_token(request: &str) -> bool {
    let head = request.split_once("\r\n\r\n").expect("a whole request").0;
    head.lines()
        .any(|line| line.eq_ignore_ascii_case(&format!("authorization: Bearer {TOKEN}")))
}

fn shown(output: &Output) -> String {
    format!(
        "status {:?}\n--- stdout\n{}--- stderr\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// One scripted model answer: a forced tool call in the Chat Completions stream format of the
/// recorded fixtures.
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

/// Answers one request per scripted response, in order, on a loopback socket, until told to
/// finish, and records each whole request. A request past the script is answered with an error
/// status and recorded, so the case sees it.
struct Stub {
    done: Arc<AtomicBool>,
    handle: JoinHandle<Vec<String>>,
}

impl Stub {
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
                let mut request = Vec::new();
                let mut buffer = [0_u8; 8192];
                loop {
                    let read = socket.read(&mut buffer).expect("read");
                    request.extend_from_slice(&buffer[..read]);
                    if read == 0 || complete(&request) {
                        break;
                    }
                }
                sent.push(String::from_utf8_lossy(&request).into_owned());
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

    /// Stops serving and returns every request, in order.
    fn finish(self) -> Vec<String> {
        self.done.store(true, Ordering::SeqCst);
        self.handle.join().expect("the stub finished")
    }
}
