//! Adversary cases for `story:cli-catalog-model-route` (`b10x-loom run --catalog`).
//!
//! Every case runs the binary against a copy of `fixtures/catalog-route/catalog.toml`, with an
//! empty home, and at most a loopback socket for a model endpoint. Each case names what it
//! attacks: a catalog path that is not a plain file, a catalog that is oversized, deeply nested or
//! carries a secret-shaped value, the `--output jsonl` stream on both the refusal and the catalog
//! path, either flag order, and the Responses and Messages protocols the CHANGELOG says a
//! catalog route can drive a run over.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const CATALOG: &str = include_str!("fixtures/catalog-route/catalog.toml");
const CLASSIFY: &[u8] = include_bytes!("fixtures/catalog-route/classify.sse");
const SELECT: &[u8] = include_bytes!("fixtures/catalog-route/select.sse");
const ARGUMENTS: &[u8] = include_bytes!("fixtures/catalog-route/arguments.sse");
const QUERY: &str = "need to know the current time";
const SECRET: &str = "sk-ADVERSARY-0123456789abcdef";

/// A named pipe nobody writes is refused or at least not waited on forever: the catalog read
/// must end. Today `File::open` on a FIFO blocks until a writer appears, so the run hangs before
/// it can refuse anything.
#[test]
fn a_fifo_catalog_without_a_writer_does_not_hang_the_run() {
    let scratch = Scratch::new("fifo");
    let fifo = scratch.root.join("catalog.fifo");
    let made = Command::new("mkfifo").arg(&fifo).status().expect("mkfifo");
    assert!(made.success());
    let mut child = scratch
        .command(&[
            "--catalog",
            fifo.to_str().unwrap(),
            "--classifier-model",
            "lab-classifier",
            "--model",
            "lab-agent",
            QUERY,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn b10x-loom");
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(
        status.is_some(),
        "b10x-loom run --catalog <fifo> was still blocked after 10 s; it never refused the file"
    );
    assert_eq!(status.unwrap().code(), Some(1));
}

/// A directory, a file over llm's 2 MiB catalog bound and an endless device are refused naming
/// the file, with exit status 1 and no connection.
#[test]
fn a_catalog_that_is_not_a_bounded_file_is_refused_naming_it() {
    let scratch = Scratch::new("shapes");
    let directory = scratch.root.join("catalog.d");
    std::fs::create_dir_all(&directory).unwrap();
    let huge = scratch.root.join("huge.toml");
    let mut text = String::from("format = \"llm.catalog/1\"\n#");
    text.push_str(&"x".repeat(2 * 1024 * 1024));
    std::fs::write(&huge, text).unwrap();
    let zero = PathBuf::from("/dev/zero");
    for path in [&directory, &huge, &zero] {
        let listener = idle();
        let output = scratch.run(&[
            "--catalog",
            path.to_str().unwrap(),
            "--classifier-model",
            "lab-classifier",
            "--model",
            "lab-agent",
            QUERY,
        ]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{}: {stderr}",
            path.display()
        );
        assert!(
            stderr.contains(path.to_str().unwrap()),
            "{}: {stderr}",
            path.display()
        );
        assert_untouched(&listener);
    }
}

/// TOML nested far past 128 levels is refused as an invalid catalog, naming the file, and does
/// not take the process down.
#[test]
fn a_deeply_nested_catalog_is_refused_naming_it() {
    let scratch = Scratch::new("deep");
    let deep = scratch.root.join("deep.toml");
    let depth = 200_000;
    let text = format!(
        "format = \"llm.catalog/1\"\nproviders = {}{}\n",
        "[".repeat(depth),
        "]".repeat(depth)
    );
    std::fs::write(&deep, text).unwrap();
    let output = scratch.run(&[
        "--catalog",
        deep.to_str().unwrap(),
        "--classifier-model",
        "lab-classifier",
        "--model",
        "lab-agent",
        QUERY,
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{:?}: {stderr}",
        output.status
    );
    assert!(stderr.contains(deep.to_str().unwrap()), "{stderr}");
}

/// A catalog refusal never echoes a value from the file: not an unknown field's value, not a
/// base URL carrying userinfo, not a secret reference id used where an account id belongs.
#[test]
fn a_catalog_refusal_never_echoes_a_secret_shaped_value() {
    let scratch = Scratch::new("secret");
    let unknown_field = CATALOG.replacen(
        "auth_kind = \"anonymous\"",
        &format!("auth_kind = \"anonymous\"\napi_key = \"{SECRET}\""),
        1,
    );
    let userinfo = CATALOG.replacen(
        "http://127.0.0.1:1/v1",
        &format!("http://user:{SECRET}@127.0.0.1:1/v1"),
        1,
    );
    let wrong_reference = CATALOG.replacen(
        "account_id = \"local\"",
        &format!("account_id = \"{SECRET}\""),
        1,
    );
    let bad_toml = format!("format = \"llm.catalog/1\"\napi_key = \"{SECRET}\n");
    for (label, text) in [
        ("unknown-field", unknown_field),
        ("userinfo", userinfo),
        ("wrong-reference", wrong_reference),
        ("bad-toml", bad_toml),
    ] {
        let path = scratch.root.join(format!("{label}.toml"));
        std::fs::write(&path, text).unwrap();
        let output = scratch.run(&[
            "--output",
            "jsonl",
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
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        assert!(stderr.contains(path.to_str().unwrap()), "{label}: {stderr}");
        assert!(
            !stderr.contains(SECRET),
            "{label} echoes the value: {stderr}"
        );
        assert!(
            !stdout.contains(SECRET),
            "{label} echoes the value: {stdout}"
        );
    }
}

/// With `--output jsonl`, a catalog refusal writes the one terminal record and nothing else:
/// no `Route`, no `Turn`, exit status 1, the error naming the alias.
#[test]
fn a_jsonl_refusal_is_the_one_terminal_record() {
    let scratch = Scratch::new("jsonl-refusal");
    let listener = idle();
    let catalog = scratch.catalog(&format!("http://{}/v1", listener.local_addr().unwrap()));
    let output = scratch.run(&[
        "--output",
        "jsonl",
        "--catalog",
        catalog.to_str().unwrap(),
        "--model",
        "no-such-route",
        "--classifier-model",
        "lab-classifier",
        QUERY,
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "{stdout}");
    let lines: Vec<Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect();
    assert_eq!(lines.len(), 1, "{stdout}");
    assert_eq!(lines[0]["kind"], "Terminal", "{stdout}");
    assert_eq!(lines[0]["exit_status"], json!(1), "{stdout}");
    let error = lines[0]["error"].as_str().unwrap_or_default();
    assert!(error.contains("--model no-such-route"), "{stdout}");
    assert_untouched(&listener);
}

/// Omitting either flag with `--catalog` refuses its default Codex name as an unknown alias,
/// naming that flag, in either argument order, before any connection.
#[test]
fn an_omitted_flag_with_a_catalog_is_refused_naming_its_default() {
    for (label, args) in [
        ("classifier-omitted", ["--model", "lab-agent"]),
        ("agent-omitted", ["--classifier-model", "lab-classifier"]),
    ] {
        let scratch = Scratch::new(label);
        let listener = idle();
        let catalog = scratch.catalog(&format!("http://{}/v1", listener.local_addr().unwrap()));
        let mut all = vec![
            args[0],
            args[1],
            "--catalog",
            catalog.to_str().unwrap(),
            QUERY,
        ];
        if label == "agent-omitted" {
            all.rotate_left(2);
        }
        let output = scratch.run(&all);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        let flag = if label == "classifier-omitted" {
            "--classifier-model gpt-5.6-sol"
        } else {
            "--model gpt-5.6-sol"
        };
        assert!(stderr.contains(flag), "{label}: {stderr}");
        assert_untouched(&listener);
    }
}

/// On the catalog path the jsonl stream names, for each turn, the model of the binding that
/// answered: the classifier's for the classification, the agent's for the two agent turns.
#[test]
fn the_jsonl_stream_names_each_routes_model() {
    let scratch = Scratch::new("jsonl-models");
    let (base_url, server) = serve(vec![CLASSIFY, SELECT, ARGUMENTS]);
    let catalog = scratch.catalog(&base_url);
    let output = scratch.run(&[
        "--output",
        "jsonl",
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
    server.join().expect("served");
    let turns: Vec<(String, String)> = stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|line| line["kind"] == "Turn")
        .map(|line| {
            (
                line["phase"].as_str().unwrap().to_owned(),
                line["model"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(turns.len(), 3, "{stdout}");
    assert_eq!(turns[0].0, "Classification", "{stdout}");
    assert_ne!(
        turns[0].1, turns[1].1,
        "classifier and agent differ: {stdout}"
    );
    assert_eq!(turns[1].1, turns[2].1, "{stdout}");
}

/// The CHANGELOG: "a self-hosted Chat Completions, Responses or Messages endpoint can drive a
/// run". A Messages route and a Responses route each complete the recorded run.
#[test]
fn a_messages_and_a_responses_route_each_drive_the_recorded_run() {
    for protocol in ["messages", "responses"] {
        let scratch = Scratch::new(protocol);
        let responses: Vec<Vec<u8>> = match protocol {
            "messages" => vec![
                messages_call("pick_protocol", CLASSIFY_ARGUMENTS),
                messages_call("select_action", SELECT_ARGUMENTS),
                messages_call("action_arguments", "{}"),
            ],
            _ => vec![
                responses_call("pick_protocol", CLASSIFY_ARGUMENTS),
                responses_call("select_action", SELECT_ARGUMENTS),
                responses_call("action_arguments", "{}"),
            ],
        };
        let responses: Vec<&'static [u8]> = responses
            .into_iter()
            .map(|r| &*Box::leak(r.into_boxed_slice()))
            .collect();
        let (base_url, server) = serve(responses);
        let catalog = scratch.root.join("catalog.toml");
        std::fs::write(
            &catalog,
            CATALOG.replace("http://127.0.0.1:1/v1", &base_url).replace(
                "protocol = \"chat-completions\"",
                &format!("protocol = \"{protocol}\""),
            ),
        )
        .unwrap();
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
        assert_eq!(
            output.status.code(),
            Some(0),
            "{protocol}: {stdout}\n{stderr}"
        );
        assert!(
            stdout.contains("stopped: Completed (answered)"),
            "{protocol}: {stdout}\n{stderr}"
        );
        let sent = server.join().expect("served");
        for request in &sent {
            assert!(
                request.starts_with(&format!("POST /v1/{protocol} ")),
                "{protocol}: {request}"
            );
        }
    }
}

/// The fallback refusal is exactly "fallback enabled and more than one target": a route with
/// two targets and no fallback runs on its first target, and a fallback route with a single
/// target runs on it. Kills the mutants that drop either half of the condition.
#[test]
fn only_a_route_that_can_actually_fall_back_is_refused() {
    let extra = r#"
[[routes]]
id = "pinned"
alias = "lab-pinned"

[[targets]]
id = "pinned-first"
route_id = "pinned"
serving_model_id = "agent-chat"
position = 0

[[targets]]
id = "pinned-second"
route_id = "pinned"
serving_model_id = "classifier-chat"
position = 1

[[routes]]
id = "lonely"
alias = "lab-lonely"
fallback_enabled = true

[[targets]]
id = "lonely-only"
route_id = "lonely"
serving_model_id = "agent-chat"
position = 0
"#;
    for alias in ["lab-pinned", "lab-lonely"] {
        let scratch = Scratch::new(alias);
        let (base_url, server) = serve(vec![CLASSIFY, SELECT, ARGUMENTS]);
        let catalog = scratch.root.join("catalog.toml");
        std::fs::write(
            &catalog,
            format!(
                "{}{extra}",
                CATALOG.replace("http://127.0.0.1:1/v1", &base_url)
            ),
        )
        .unwrap();
        let output = scratch.run(&[
            "--catalog",
            catalog.to_str().unwrap(),
            "--classifier-model",
            "lab-classifier",
            "--model",
            alias,
            QUERY,
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(0), "{alias}: {stdout}\n{stderr}");
        let sent = server.join().expect("served");
        let agent_models: Vec<Value> = sent[1..]
            .iter()
            .map(|request| {
                let body: Value =
                    serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
                body["model"].clone()
            })
            .collect();
        assert_eq!(
            agent_models,
            [json!("example/Agent-Model"), json!("example/Agent-Model")],
            "{alias}: the first target serves the agent"
        );
    }
}

/// A model that takes tools but not a forced tool choice is refused like one that takes neither
/// (kills `!tools || !tool_choice` -> `&&`), and a credentialed account is refused with the
/// reason the command line gives, not llm's generic build failure.
#[test]
fn half_capable_and_credentialed_routes_are_refused_with_their_reasons() {
    let extra = r#"
[[serving_models]]
id = "agent-unforced"
endpoint_id = "loopback"
model_id = "agent"
protocol = "chat-completions"
[serving_models.capabilities]
tools = true
tool_choice = false
temperature = false
top_p = false
reasoning_efforts = []
context_window = 128000
max_output_tokens = 8192

[[routes]]
id = "unforced"
alias = "lab-unforced"

[[targets]]
id = "unforced-only"
route_id = "unforced"
serving_model_id = "agent-unforced"
position = 0
"#;
    for (alias, reason) in [
        ("lab-unforced", "cannot take a forced tool call"),
        ("lab-keyed", "which the command line cannot supply"),
    ] {
        let scratch = Scratch::new(alias);
        let listener = idle();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let catalog = scratch.root.join("catalog.toml");
        std::fs::write(
            &catalog,
            format!(
                "{}{extra}",
                CATALOG.replace("http://127.0.0.1:1/v1", &base_url)
            ),
        )
        .unwrap();
        let output = scratch.run(&[
            "--catalog",
            catalog.to_str().unwrap(),
            "--model",
            alias,
            "--classifier-model",
            "lab-classifier",
            QUERY,
        ]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{alias}: {stderr}");
        assert!(
            stderr.contains(&format!("--model {alias}")),
            "{alias}: {stderr}"
        );
        assert!(stderr.contains(reason), "{alias}: {stderr}");
        assert_untouched(&listener);
    }
}

/// The model a provider claims in its response never becomes the model the stream names: the
/// binding's model comes from the catalog, not from the wire.
#[test]
fn the_stream_never_names_the_model_the_server_claims() {
    let claimed = |bytes: &[u8]| -> &'static [u8] {
        let text = String::from_utf8_lossy(bytes)
            .replace("example/Classifier-Model", "server-claimed-model")
            .replace("example/Agent-Model", "server-claimed-model");
        Box::leak(text.into_bytes().into_boxed_slice())
    };
    let scratch = Scratch::new("claimed");
    let (base_url, server) = serve(vec![claimed(CLASSIFY), claimed(SELECT), claimed(ARGUMENTS)]);
    let catalog = scratch.catalog(&base_url);
    let output = scratch.run(&[
        "--output",
        "jsonl",
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
    server.join().expect("served");
    assert!(
        !stdout.contains("server-claimed-model"),
        "the stream names the server's claim: {stdout}\n{stderr}"
    );
}

const CLASSIFY_ARGUMENTS: &str =
    "{\"protocol\":\"system-query@1\",\"confidence\":0.99,\"reasons\":[\"asks for the time\"]}";
const SELECT_ARGUMENTS: &str = "{\"action\":\"system.time.read\"}";

fn sse(events: &[(&str, Value)]) -> Vec<u8> {
    let mut out = String::new();
    for (name, data) in events {
        if !name.is_empty() {
            out.push_str(&format!("event: {name}\n"));
        }
        out.push_str(&format!("data: {data}\n\n"));
    }
    out.into_bytes()
}

fn messages_call(name: &str, arguments: &str) -> Vec<u8> {
    sse(&[
        (
            "message_start",
            json!({"type":"message_start","message":{"id":"msg_1","type":"message",
                "role":"assistant","model":"example/Agent-Model","content":[],
                "stop_reason":null,"stop_sequence":null,
                "usage":{"input_tokens":120,"output_tokens":1}}}),
        ),
        (
            "content_block_start",
            json!({"type":"content_block_start","index":0,
                "content_block":{"type":"tool_use","id":format!("call-{name}"),"name":name,"input":{}}}),
        ),
        (
            "content_block_delta",
            json!({"type":"content_block_delta","index":0,
                "delta":{"type":"input_json_delta","partial_json":arguments}}),
        ),
        (
            "content_block_stop",
            json!({"type":"content_block_stop","index":0}),
        ),
        (
            "message_delta",
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},
                "usage":{"output_tokens":7}}),
        ),
        ("message_stop", json!({"type":"message_stop"})),
    ])
}

fn responses_call(name: &str, arguments: &str) -> Vec<u8> {
    let item = json!({"type":"function_call","id":"fc_1","call_id":format!("call-{name}"),
        "name":name,"arguments":arguments});
    sse(&[
        (
            "response.output_item.added",
            json!({"type":"response.output_item.added","item":{"type":"function_call",
                "id":"fc_1","call_id":format!("call-{name}"),"name":name}}),
        ),
        (
            "response.function_call_arguments.delta",
            json!({"type":"response.function_call_arguments.delta","item_id":"fc_1",
                "delta":arguments}),
        ),
        (
            "response.output_item.done",
            json!({"type":"response.output_item.done","item":item.clone()}),
        ),
        (
            "response.completed",
            json!({"type":"response.completed","response":{"id":"resp_1",
                "model":"example/Agent-Model","status":"completed","output":[item],
                "usage":{"input_tokens":120,"output_tokens":7}}}),
        ),
    ])
}

// ---------------------------------------------------------------------------------------------
// Fixtures, as in `catalog_route.rs`.

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
            "catalog-route-adv-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("home")).unwrap();
        Self { root }
    }

    fn catalog(&self, base_url: &str) -> PathBuf {
        let path = self.root.join("catalog.toml");
        std::fs::write(&path, CATALOG.replace("http://127.0.0.1:1/v1", base_url)).unwrap();
        path
    }

    fn command(&self, args: &[&str]) -> Command {
        let home = self.root.join("home");
        let mut command = Command::new(env!("CARGO_BIN_EXE_b10x-loom"));
        command
            .arg("run")
            .args(["--confinement", "none"])
            .args(args)
            .env("HOME", &home)
            .env("CODEX_HOME", &home)
            .env("XDG_DATA_HOME", &home)
            .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
            .env_remove("RUST_LOG");
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("run b10x-loom")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn serve(responses: Vec<&'static [u8]>) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let mut sent = Vec::new();
        for response in responses {
            let (mut socket, _) = listener.accept().expect("accepted");
            socket
                .set_read_timeout(Some(Duration::from_secs(20)))
                .unwrap();
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
