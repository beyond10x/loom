//! Security review of `story:a2a-turn-error-names-no-host-path` (wave 2026-11-05-w1, U1): an
//! error from an A2A turn reaches the caller as a fixed reason naming no directory of the host,
//! on the executor's own failure paths and not only through the turn's model.
//!
//! The implementor's cases fail the turn through its model port (`PluginError::Unavailable`
//! built by the test). These cases fail it where a served agent fails: the connectors program the
//! spec names by an absolute path does not start, so the sources cannot be described; and the
//! state directory does not take the case store, so the governed turn cannot open it. Each error
//! names an absolute path of the fixture; none of it may reach the caller.
//!
//! The listener runs on a loopback port; the models are recorded. Nothing reaches a network
//! beyond 127.0.0.1, a credential or a real channel. Every path is built at run time.

#[path = "../../loom-plugin-slack/tests/support/mod.rs"]
mod support;

use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use b10x_loom_a2a::{Answerer, Bearer};
use b10x_loom_agent::SpecPlugin;
use b10x_loom_agent::serve::a2a::{Answering, TURN_STATE, TURN_UNAVAILABLE, listener};
use b10x_loom_plugin::Host;
use b10x_loom_plugin_slack::TurnModels;
use loom_sdk::agent::{AgentSpec, parse};
use loom_sdk::commission::model::responsibility::RuntimeEvent;
use serde_json::{Value, json};
use support::host::{Fixture, Recorded};

const PATH: &str = "/a2a";
/// The bearer of these cases: a throwaway value of this file only.
const TOKEN: &str = "a2a-security-u1-bearer-7c3e";

/// A fixture directory with the plugin's state directory created.
fn fixture(test: &str) -> Fixture {
    let fixture = Fixture::new("agent-a2a-security-u1", test);
    fs::create_dir_all(fixture.plugin_state()).unwrap();
    fixture
}

/// A spec exposing A2A whose connectors command line is `program`, an absolute path.
fn spec(fixture: &Fixture, program: &Path) -> AgentSpec {
    parse(&format!(
        "format: loom.agent/1
agent: {{id: test-operator, name: Test operator, brief: Answer questions about the deploy.}}
model:
  routes: routes.yaml
  roles: [{{role: agent, model: small}}, {{role: classifier, model: small}}]
loops:
  outer: {{poll_interval_seconds: 60, sources: [docs], min_age_minutes: 0, lookback_minutes: 120}}
  inner: {{protocol: operator-reply@1, step_budget: 8, model_budget: 16, classify_threshold: 0.5}}
states:
  - {{name: answer, when: {{intent: Ask}}, actions: [source.read, reply.propose, reply.decline], sources: [docs]}}
bindings:
  - {{name: docs, kind: connectors, adapter: docs, connection: docs-fixture, search: docs.search}}
policy: {{capabilities: [datasource.read, reply.propose]}}
expose:
  a2a: {{port: 8080, path: {PATH}, card: {{name: test-operator, description: Answers about the deploy, skills: [logs]}}}}
runtime:
  state_dir: {state}
  connectors: {{program: {program}, state_dir: {connectors}}}
  confinement: none
",
        state = fixture.plugin_state().display(),
        program = program.display(),
        connectors = fixture.connectors_state().display(),
    ))
    .expect("the spec decodes")
}

/// The served plugin of `spec`, classifying every message `ask` and with a turn model that is
/// never asked (each case fails before the first model request).
fn plugin(fixture: &Fixture, spec: AgentSpec) -> SpecPlugin {
    let (turns, _made): (TurnModels, _) = support::turns(vec![Vec::new(), Vec::new()]);
    SpecPlugin::new(spec, &fixture.plugin_state())
        .expect("the spec is served")
        .with_classifier(Box::new(Recorded::classifying("ask", &["docs"], 0.9)))
        .with_turn_models(turns)
}

/// One POST of `body` with the bearer: the answer's head and body.
fn post_raw(address: SocketAddr, body: &Value) -> (String, String) {
    let mut stream = TcpStream::connect(address).expect("the listener accepts");
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let body = body.to_string();
    let head = format!(
        "POST {PATH} HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body.as_bytes()).unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("the answer is read");
    let (head, body) = raw.split_once("\r\n\r\n").expect("a head and a body");
    (head.to_owned(), body.to_owned())
}

fn post(address: SocketAddr, body: &Value) -> Value {
    let (head, body) = post_raw(address, body);
    assert!(head.starts_with("HTTP/1.1 200 "), "{head}");
    serde_json::from_str(&body).unwrap_or_else(|error| panic!("not JSON ({error}): {body}"))
}

fn send(method: &str, message_id: &str, text: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": {"message": {
        "messageId": message_id, "role": "ROLE_USER", "parts": [{"text": text}]}}})
}

/// Sets `stop` when dropped, so a failed assertion still ends the listener.
struct StopOnDrop<'a>(&'a AtomicBool);

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// The text of a task status's message.
fn reason_of(status: &Value) -> String {
    status["message"]["parts"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// The fixture's root as text: every path these cases make the turn fail on lies under it.
fn root_of(fixture: &Fixture) -> String {
    let root: PathBuf = fixture.root.clone();
    let root = root.display().to_string();
    assert!(
        root.starts_with('/'),
        "the fixture root is absolute: {root}"
    );
    root
}

/// The connectors program the spec names by an absolute path does not exist, as on a host whose
/// image lacks it: the sources cannot be described and the turn cannot run, with an error naming
/// that path. `SendMessage`, `GetTask` and the stream's last `statusUpdate` all carry the fixed
/// reason, and no answer names the fixture's directory.
#[test]
fn a_connectors_program_that_does_not_start_names_no_host_path_to_the_caller() {
    let fixture = fixture("missing-program");
    let root = root_of(&fixture);
    let program = fixture.root.join("no-such-connectors");
    assert!(!program.exists());
    let plugin = plugin(&fixture, spec(&fixture, &program));
    let exposure = plugin.spec().expose.clone().unwrap().a2a.unwrap();
    let server = listener(&exposure, Bearer::new(TOKEN).unwrap()).expect("the listener is built");
    let host = Host::new(plugin.config()).with_environment(fixture.environment());
    let agent = Answering::with_host(&plugin, host);
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    let stop = AtomicBool::new(false);

    let (sent, got, (head, body)) = std::thread::scope(|scope| {
        let asking = scope.spawn(|| {
            let _stop = StopOnDrop(&stop);
            let sent = post(address, &send("SendMessage", "m-1", "Is the deploy done?"));
            let task = sent["result"]["task"]["id"].clone();
            let got = post(
                address,
                &json!({"jsonrpc": "2.0", "id": 2, "method": "GetTask", "params": {"id": task}}),
            );
            let streamed = post_raw(address, &send("SendStreamingMessage", "m-2", "And now?"));
            (sent, got, streamed)
        });
        server
            .serve(socket, &agent, &stop)
            .expect("the listener ends cleanly");
        asking.join().expect("the client asked")
    });

    assert!(head.contains("text/event-stream"), "{head}");
    let last = body
        .split("\n\n")
        .filter(|event| !event.is_empty())
        .map(|event| serde_json::from_str::<Value>(event.trim_start_matches("data: ")).unwrap())
        .last()
        .expect("the stream carries events");
    let expected = format!("the turn could not run: {TURN_UNAVAILABLE}");
    for (surface, status) in [
        ("SendMessage", &sent["result"]["task"]["status"]),
        ("GetTask", &got["result"]["status"]),
        (
            "the stream's last statusUpdate",
            &last["result"]["statusUpdate"]["status"],
        ),
    ] {
        assert_eq!(status["state"], "TASK_STATE_FAILED", "{surface}: {status}");
        assert_eq!(reason_of(status), expected, "{surface}: {status}");
    }
    for (surface, answer) in [
        ("SendMessage", sent.to_string()),
        ("GetTask", got.to_string()),
        ("the stream", body),
    ] {
        assert!(
            !answer.contains(&root),
            "{surface} names a directory of the host ({root}): {answer}"
        );
    }
}

/// The state directory holds what the case store takes for a temporary file an earlier life left
/// behind and cannot remove (a directory), so the governed turn cannot open its case store and
/// fails naming that path. The record of the same directory opens, so the turn itself is what
/// fails: its caller is given the fixed reason of a state failure, naming no path.
#[test]
fn a_case_store_that_cannot_be_opened_names_no_host_path_to_the_caller() {
    let fixture = fixture("case-store");
    let root = root_of(&fixture);
    let blocked = fixture.plugin_state().join(".tmp-left-behind");
    fs::create_dir_all(blocked.join("inside")).unwrap();
    let program = fixture.root.join("connectors");
    let plugin = plugin(&fixture, spec(&fixture, &program));
    let host = Host::new(plugin.config()).with_environment(fixture.environment());
    let agent = Answering::with_host(&plugin, host);
    let mut events: Vec<RuntimeEvent> = Vec::new();

    let failed = agent
        .answer(&support::host::item("a2a:m-1"), &mut events)
        .expect_err("the turn cannot open its case store");

    assert!(
        !failed.contains(&root),
        "the caller is given a path: {failed}"
    );
    assert_eq!(failed, TURN_STATE, "{failed}");
}
