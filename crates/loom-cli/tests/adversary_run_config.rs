//! Adversary cases for `b10x-loom run --config <PATH>`: an explicit model flag equal to the old
//! default, a credential that cannot be resolved for one role, the forced-tool-call check on each
//! of its two capabilities, the `--output jsonl` contract under a configuration, and secret values
//! in what the run prints.
//!
//! Every case runs the binary against loopback sockets this file opens; `HOME` and `CODEX_HOME`
//! are an empty scratch directory, so no case reaches the network or a Codex login.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

const CLASSIFY: &[u8] = include_bytes!("fixtures/catalog-route/classify.sse");
const SELECT: &[u8] = include_bytes!("fixtures/catalog-route/select.sse");
const ARGUMENTS: &[u8] = include_bytes!("fixtures/catalog-route/arguments.sse");
const QUERY: &str = "need to know the current time";
const TOKEN_VARIABLE: &str = "LOOM_ADV_RUN_CONFIG_TOKEN";
const TOKEN: &str = "adv-run-config-SECRET-token";
const UNSET_VARIABLE: &str = "LOOM_ADV_RUN_CONFIG_UNSET";

/// One endpoint with a bearer secret; `tools_only` takes tools but no forced tool choice.
const ROUTES: &str = "format: llm.routes/1
endpoints:
  lab:
    base_url: BASE_URL
    protocol: chat-completions
    auth: {kind: bearer, secret: env:LOOM_ADV_RUN_CONFIG_TOKEN}
routes:
  judge: lab/example/Classifier-Model
  worker: lab/example/Agent-Model
  tools_only:
    model: lab/example/Tools-Only-Model
    capabilities: {tools: true, tool_choice: false, temperature: true, top_p: true,
                   reasoning_efforts: [], context_window: 8192, max_output_tokens: 2048}
";

/// The classifier on an anonymous endpoint, the agent on one whose secret variable is unset.
const SPLIT_ROUTES: &str = "format: llm.routes/1
endpoints:
  open:
    base_url: BASE_URL
    protocol: chat-completions
    auth: {kind: anonymous}
  locked:
    base_url: BASE_URL
    protocol: chat-completions
    auth: {kind: bearer, secret: env:LOOM_ADV_RUN_CONFIG_UNSET}
routes:
  judge: open/example/Classifier-Model
  worker: locked/example/Agent-Model
";

const BOTH_ROLES: &str = "routes: routes.yaml
roles:
  - {role: agent, model: worker}
  - {role: classifier, model: judge}
";

/// Under `--config` the old default is no longer a default: `--model gpt-5.6-sol` given
/// explicitly is a name of the routes document, which does not serve it, so the run is refused
/// naming the flag and the name, before any model call; the same for `--classifier-model`.
#[test]
fn an_explicit_old_default_under_config_is_a_name_of_the_document() {
    for flag in ["--model", "--classifier-model"] {
        let scratch = Scratch::new("explicit-default");
        let listener = idle();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let config = scratch.config(ROUTES, &base_url, BOTH_ROLES);
        let output = scratch.run(
            &[
                "--config",
                config.to_str().unwrap(),
                flag,
                "gpt-5.6-sol",
                QUERY,
            ],
            &[(TOKEN_VARIABLE, TOKEN)],
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{flag}: {stderr}");
        assert!(
            stderr.contains(&format!("{flag} gpt-5.6-sol")),
            "{flag}: {stderr}"
        );
        assert_untouched(&listener);
    }
}

/// "Both models are settled before any model call": a run whose agent route names a secret
/// variable that is unset must not send the classifier's request first and only then fail on the
/// agent's first turn. The endpoint receives no request, and the run exits 1.
#[test]
fn an_unresolvable_agent_credential_stops_the_run_before_the_classifier_is_called() {
    let scratch = Scratch::new("unset-secret");
    let server = Server::start(vec![CLASSIFY, SELECT, ARGUMENTS], 200);
    let config = scratch.config(SPLIT_ROUTES, &server.base_url, BOTH_ROLES);
    let output = scratch.run(&["--config", config.to_str().unwrap(), QUERY], &[]);
    let sent = server.stop();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    assert!(
        sent.is_empty(),
        "the endpoint received {} request(s) before the agent's credential was found missing \
         (forced tools: {:?})\nstdout: {stdout}\nstderr: {stderr}",
        sent.len(),
        sent.iter().map(|r| forced_tool(r)).collect::<Vec<_>>()
    );
}

/// The forced-tool-call check takes both capabilities: a route that takes tools but no forced
/// tool choice is refused for the role it serves, configured or given by flag, before any model
/// call.
#[test]
fn a_route_with_tools_but_no_forced_choice_is_refused_for_each_role() {
    for (label, roles, extra, named) in [
        (
            "configured-agent",
            "  - {role: agent, model: tools_only}\n  - {role: classifier, model: judge}\n",
            &[][..],
            &["`agent`", "forced tool call"][..],
        ),
        (
            "flag-classifier",
            "  - {role: agent, model: worker}\n  - {role: classifier, model: judge}\n",
            &["--classifier-model", "tools_only"][..],
            &["`classifier`", "--classifier-model", "forced tool call"][..],
        ),
    ] {
        let scratch = Scratch::new(label);
        let listener = idle();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let config = scratch.config(
            ROUTES,
            &base_url,
            &format!("routes: routes.yaml\nroles:\n{roles}"),
        );
        let mut args = vec!["--config", config.to_str().unwrap()];
        args.extend_from_slice(extra);
        args.push(QUERY);
        let output = scratch.run(&args, &[(TOKEN_VARIABLE, TOKEN)]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        for name in named {
            assert!(stderr.contains(name), "{label} names {name}: {stderr}");
        }
        assert_untouched(&listener);
    }
}

/// `--output jsonl` under `--config` writes only the kinds and fields of
/// `docs/contracts/run-events-v1.json`, every required field present; each turn names the
/// binding of its role's model (a route's binding is the route's name, as a catalog alias is);
/// and the bearer token appears neither on standard output nor on standard error.
#[test]
fn jsonl_under_config_keeps_the_contract_and_carries_no_secret() {
    let contract: Value = serde_json::from_slice(
        &std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs/contracts/run-events-v1.json"),
        )
        .expect("the contract"),
    )
    .expect("a JSON contract");
    let scratch = Scratch::new("jsonl");
    let server = Server::start(vec![CLASSIFY, SELECT, ARGUMENTS], 200);
    let config = scratch.config(ROUTES, &server.base_url, BOTH_ROLES);
    let output = scratch.run(
        &[
            "--output",
            "jsonl",
            "--config",
            config.to_str().unwrap(),
            QUERY,
        ],
        &[(TOKEN_VARIABLE, TOKEN)],
    );
    let sent = server.stop();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stdout}\n{stderr}");
    assert_eq!(sent.len(), 3, "{stderr}");
    assert!(!stdout.contains(TOKEN) && !stderr.contains(TOKEN));
    let mut turns = Vec::new();
    let lines: Vec<Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect();
    for line in &lines {
        assert_eq!(line["schema_version"], 1, "{line}");
        let kind = line["kind"].as_str().expect("a kind");
        let fields = contract["kinds"][kind]["fields"]
            .as_object()
            .unwrap_or_else(|| panic!("a contract kind {kind}"));
        for (name, _) in line.as_object().unwrap() {
            if name != "schema_version" && name != "kind" {
                assert!(
                    fields.contains_key(name),
                    "{kind}.{name} not in the contract"
                );
            }
        }
        for (name, field) in fields {
            if field["optional"] == false {
                assert!(line.get(name).is_some(), "{kind}.{name} missing: {line}");
            }
        }
        if kind == "Turn" {
            turns.push((
                line["phase"].as_str().unwrap().to_owned(),
                line["model"].as_str().unwrap().to_owned(),
            ));
        }
    }
    assert_eq!(
        turns,
        [
            ("Classification".into(), "judge".into()),
            ("Selection".into(), "worker".into()),
            ("Arguments".into(), "worker".into()),
        ]
    );
    let terminal = lines.last().expect("a terminal line");
    assert_eq!(terminal["kind"], "Terminal");
    assert_eq!(terminal["exit_status"], 0);
    assert_eq!(terminal["stop_reason"], "Completed");
}

/// A secret value the request cannot carry (a control character, a line break that would inject
/// a header) fails the run with exit 1, and neither standard error nor the jsonl terminal record
/// echoes the value; no request carries an injected header.
#[test]
fn an_unsendable_secret_value_is_never_echoed_nor_injected() {
    for (label, token) in [
        ("control", "adv-SECRET-value-\u{7f}-tail"),
        ("injection", "adv-SECRET-value\r\nX-Injected: yes"),
    ] {
        let scratch = Scratch::new(label);
        let server = Server::start(Vec::new(), 401);
        let config = scratch.config(ROUTES, &server.base_url, BOTH_ROLES);
        let output = scratch.run(
            &[
                "--output",
                "jsonl",
                "--config",
                config.to_str().unwrap(),
                QUERY,
            ],
            &[(TOKEN_VARIABLE, token)],
        );
        let sent = server.stop();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stdout}\n{stderr}");
        assert!(
            !stdout.contains("adv-SECRET") && !stderr.contains("adv-SECRET"),
            "{label}: {stdout}\n{stderr}"
        );
        for request in &sent {
            assert!(
                !request.to_ascii_lowercase().contains("\r\nx-injected:"),
                "{label}: {request}"
            );
        }
    }
}

/// Without `--config` the help still gives both model flags the Codex default.
#[test]
fn without_config_both_model_flags_keep_their_default() {
    let output = Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
        .args(["run", "--help"])
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&output.stdout);
    for flag in ["      --model <ID>", "      --classifier-model <ID>"] {
        let start = help.find(flag).unwrap_or_else(|| panic!("{flag}: {help}"));
        let rest = &help[start + flag.len()..];
        let end = rest.find("\n      --").unwrap_or(rest.len());
        assert!(
            rest[..end].contains("[default: gpt-5.6-sol]"),
            "{flag}: {}",
            &rest[..end]
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fixtures.

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
            "adv-run-config-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("home")).unwrap();
        Self { root }
    }

    fn config(&self, routes: &str, base_url: &str, text: &str) -> PathBuf {
        std::fs::write(
            self.root.join("routes.yaml"),
            routes.replace("BASE_URL", base_url),
        )
        .unwrap();
        let path = self.root.join("loom.yaml");
        std::fs::write(&path, text).unwrap();
        path
    }

    fn run(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let home = self.root.join("home");
        let mut command = Command::new(env!("CARGO_BIN_EXE_b10x-loom"));
        command
            .arg("run")
            .args(["--confinement", "none"])
            .args(args)
            .env("HOME", &home)
            .env("CODEX_HOME", &home)
            .env("XDG_DATA_HOME", &home)
            .env_remove(TOKEN_VARIABLE)
            .env_remove(UNSET_VARIABLE)
            .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
            .env_remove("RUST_LOG");
        for (name, value) in env {
            command.env(name, value);
        }
        command.output().expect("run b10x-loom")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A loopback endpoint that answers every connection until stopped: the next recorded response
/// with status 200, or, when none is left or `status` is not 200, that status with no body.
struct Server {
    base_url: String,
    stop: Arc<AtomicBool>,
    handle: JoinHandle<Vec<String>>,
}

impl Server {
    fn start(responses: Vec<&'static [u8]>, status: u16) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            let mut responses = responses.into_iter();
            let mut sent = Vec::new();
            loop {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        socket.set_nonblocking(false).unwrap();
                        socket
                            .set_read_timeout(Some(Duration::from_secs(10)))
                            .unwrap();
                        let mut request = Vec::new();
                        let mut buffer = [0_u8; 8192];
                        loop {
                            match socket.read(&mut buffer) {
                                Ok(0) | Err(_) => break,
                                Ok(read) => request.extend_from_slice(&buffer[..read]),
                            }
                            if complete(&request) {
                                break;
                            }
                        }
                        let answer = match (status, responses.next()) {
                            (200, Some(body)) => {
                                let mut answer = Vec::from(
                                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                                );
                                answer.extend_from_slice(body);
                                answer
                            }
                            (code, _) => format!(
                                "HTTP/1.1 {code} Refused\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            )
                            .into_bytes(),
                        };
                        let _ = socket.write_all(&answer);
                        let _ = socket.shutdown(std::net::Shutdown::Both);
                        sent.push(String::from_utf8_lossy(&request).into_owned());
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        if stopped.load(Ordering::SeqCst) {
                            return sent;
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("accept: {error}"),
                }
            }
        });
        Self {
            base_url,
            stop,
            handle,
        }
    }

    fn stop(self) -> Vec<String> {
        self.stop.store(true, Ordering::SeqCst);
        self.handle.join().expect("the server finished")
    }
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

/// The forced tool a request names, for the failure message.
fn forced_tool(request: &str) -> String {
    request
        .split_once("\r\n\r\n")
        .and_then(|(_, body)| serde_json::from_str::<Value>(body).ok())
        .and_then(|body| {
            body["tool_choice"]["function"]["name"]
                .as_str()
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

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
