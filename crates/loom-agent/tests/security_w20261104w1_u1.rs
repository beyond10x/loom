//! Security review of `story:a2a-record-and-streaming` (wave 2026-11-04-w1, unit U1): integrity
//! conformance checks of the unit's own contract on a served spec.
//!
//! - Two different `messageId`s never share the record's line: the duplicate check
//!   (`loom.agent.RecordA2aTurn`) answers a message with the line of that message only.
//! - A stream's events never carry a host path of the machine, even when the turn cannot run
//!   because the state directory's record cannot be read.
//!
//! The listener runs on a loopback port this file binds; reads go to the fake `connectors` of
//! the shared fixtures; the models are recorded. Nothing reaches a network beyond 127.0.0.1.

#[path = "../../loom-plugin-slack/tests/support/mod.rs"]
mod support;

use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use b10x_loom_a2a::{Bearer, item_revision};
use b10x_loom_agent::SpecPlugin;
use b10x_loom_agent::serve::a2a::{Answering, listener};
use b10x_loom_plugin::Host;
use b10x_loom_plugin_slack::TurnModels;
use loom_sdk::agent::{AgentSpec, parse};
use serde_json::{Value, json};
use support::host::{Fixture, Recorded};

const WRITE: &str = "message.post";
const PATH: &str = "/a2a";
const REPLY: &str = "The deploy finished at 10:02; the logs show no error.";
/// The bearer of these cases: a throwaway value of this file only.
const TOKEN: &str = "a2a-security-review-bearer-u1";

fn fixture(test: &str) -> Fixture {
    let fixture = Fixture::new("agent-a2a-security", test);
    let fixtures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../loom-plugin-slack/tests/fixtures");
    for verb in ["describe", "invoke"] {
        fs::copy(
            fixtures.join(format!("{verb}-{}.json", support::SEARCH)),
            fixture
                .connectors_state()
                .join(format!("{verb}.{}.json", support::SEARCH)),
        )
        .unwrap();
    }
    let connectors =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../loom-connectors/tests/fixtures/connectors");
    fs::copy(
        connectors.join("describe-write.json"),
        fixture
            .connectors_state()
            .join(format!("describe.{WRITE}.json")),
    )
    .unwrap();
    fs::copy(
        connectors.join("invoke-route-applied.json"),
        fixture
            .connectors_state()
            .join(format!("invoke.{WRITE}.json")),
    )
    .unwrap();
    fs::create_dir_all(fixture.plugin_state()).unwrap();
    fixture
}

fn spec(fixture: &Fixture) -> AgentSpec {
    parse(&format!(
        "format: loom.agent/1
agent: {{id: test-operator, name: Test operator, brief: Answer questions about the deploy.}}
model:
  routes: routes.yaml
  roles: [{{role: agent, model: small}}, {{role: classifier, model: small}}]
loops:
  outer: {{poll_interval_seconds: 0, sources: [chat], min_age_minutes: 10, lookback_minutes: 120}}
  inner: {{protocol: operator-reply@1, step_budget: 8, model_budget: 16, classify_threshold: 0.5}}
states:
  - {{name: answer, when: {{intent: Ask}}, actions: [source.read, reply.propose, reply.send, reply.decline], sources: [docs]}}
bindings:
  - {{name: chat, kind: connectors, adapter: slack, connection: slack-fixture, list: conversations.list, get: conversations.history, search: conversations.replies, write: {WRITE}}}
  - {{name: docs, kind: connectors, adapter: docs, connection: docs-fixture, search: docs.search}}
channel: {{binding: chat, allow: [{channel}], bot_user_id: U0BOT}}
writes: {{mode: approval, max_per_hour: 5}}
policy: {{capabilities: [datasource.read, reply.propose, reply.send]}}
expose:
  a2a: {{port: 8080, path: {PATH}, card: {{name: test-operator, description: Answers about the deploy, skills: [logs, metrics]}}}}
runtime:
  state_dir: {state}
  connectors: {{program: {program}, state_dir: {connectors}}}
  confinement: none
",
        channel = support::CHANNEL,
        state = fixture.plugin_state().display(),
        program = fixture.root.join("connectors").display(),
        connectors = fixture.connectors_state().display(),
    ))
    .expect("the spec decodes")
}

/// One POST of `body` with the bearer; the answer's head and body, read whole.
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

struct StopOnDrop<'a>(&'a AtomicBool);

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Serves `plugin` on a loopback port while `case` runs against it.
fn serving<T: Send>(
    fixture: &Fixture,
    plugin: &SpecPlugin,
    case: impl FnOnce(SocketAddr) -> T + Send,
) -> T {
    let exposure = plugin.spec().expose.clone().unwrap().a2a.unwrap();
    let server = listener(&exposure, Bearer::new(TOKEN).unwrap()).expect("the listener is built");
    let host = Host::new(plugin.config()).with_environment(fixture.environment());
    let agent = Answering::with_host(plugin, host);
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    let stop = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let asking = scope.spawn(|| {
            let _stop = StopOnDrop(&stop);
            case(address)
        });
        server
            .serve(socket, &agent, &stop)
            .expect("the listener ends cleanly");
        asking.join().expect("the client asked")
    })
}

/// Invariant 3: the duplicate check must not answer one message with another message's line. A
/// `messageId` that is no plain token is keyed `m-<hash>`; a second caller whose `messageId` is
/// that very spelling, a plain token, is a different message and must run its own turn, not be
/// handed the first message's record (its question, its proposal).
#[test]
fn security_w20261104w1_u1_a_plain_message_id_never_replays_a_hashed_ones_record() {
    let fixture = fixture("hashed-vs-plain");
    let (turns, made): (TurnModels, _) = support::turns(vec![vec![
        (
            "source_read",
            json!({"source": "docs", "kind": "search", "input": {"query": "deploy"}}),
        ),
        ("reply_propose", json!({"text": REPLY})),
    ]]);
    let plugin = SpecPlugin::new(spec(&fixture), &fixture.plugin_state())
        .expect("the spec is served")
        .with_classifier(Box::new(Recorded::classifying("ask", &["docs"], 0.9)))
        .with_turn_models(turns);
    let unplain = "note 1: the deploy";
    let spelled = item_revision(unplain);
    assert_ne!(spelled, unplain, "the first id is hashed");

    let (first, second) = serving(&fixture, &plugin, |address| {
        let first = post(
            address,
            &send("SendMessage", unplain, "Is the deploy finished?"),
        );
        let second = post(
            address,
            &send("SendMessage", &spelled, "Which customer data did you read?"),
        );
        (first, second)
    });
    let record_of =
        |answer: &Value| answer["result"]["task"]["artifacts"][0]["parts"][1]["data"].clone();
    assert_eq!(record_of(&first)["proposal"], REPLY, "{first}");
    assert_ne!(
        record_of(&second),
        record_of(&first),
        "a different messageId was answered with another message's record line: {second}"
    );
    assert_eq!(
        *made.lock().unwrap(),
        2,
        "two different messageIds, two turns"
    );
}

/// Invariant 2: a stream's events never carry a host path of the machine. A record whose line the
/// reader refuses makes the turn fail before it runs; the failure the last `statusUpdate` carries
/// to the caller must not name the state directory.
#[test]
fn security_w20261104w1_u1_a_failed_streamed_turn_names_no_host_path() {
    let fixture = fixture("record-unreadable");
    let (turns, _made): (TurnModels, _) = support::turns(Vec::new());
    let plugin = SpecPlugin::new(spec(&fixture), &fixture.plugin_state())
        .expect("the spec is served")
        .with_classifier(Box::new(Recorded::classifying("ask", &["docs"], 0.9)))
        .with_turn_models(turns);
    fs::write(
        fixture.plugin_state().join("record.jsonl"),
        "{\"item\":\"C0:1\",\"outcome\":\"from-another-version\"}\n",
    )
    .unwrap();
    let state = fixture.plugin_state().display().to_string();

    let (head, body) = serving(&fixture, &plugin, |address| {
        post_raw(
            address,
            &send("SendStreamingMessage", "m-1", "Is the deploy finished?"),
        )
    });
    assert!(head.contains("text/event-stream"), "{head}");
    assert!(body.contains("TASK_STATE_FAILED"), "the turn fails: {body}");
    assert!(
        !body.contains(&state),
        "a stream event names the host path {state}: {body}"
    );
}
