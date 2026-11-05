//! Adversary pass 1 on `story:a2a-record-and-streaming` (wave 2026-11-04-w1, unit U1), driven
//! through the served agent's `Answering` and its record.
//!
//! - A `messageId` spelled as another's hashed revision is another message: it must run its own
//!   turn, not be answered with the other caller's record line.
//! - Repeats of one `messageId` sent at once run one turn and append one line.
//!
//! The listener runs on a loopback port this file binds; reads go to the fake `connectors` of
//! `loom-connectors`; the models are recorded. Nothing reaches a network beyond 127.0.0.1.

#[path = "../../loom-plugin-slack/tests/support/mod.rs"]
mod support;

use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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
const OTHER: &str = "You are the second caller; this is your own answer.";
const TOKEN: &str = "a2a-agent-adversary-bearer-u1";

fn fixture(test: &str) -> Fixture {
    let fixture = Fixture::new("agent-a2a-adversary-u1", test);
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

fn post(address: SocketAddr, body: &Value) -> Value {
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
    assert!(head.starts_with("HTTP/1.1 200 "), "{head}");
    serde_json::from_str(body).unwrap_or_else(|error| panic!("not JSON ({error}): {body}"))
}

fn send(message_id: &str, text: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": "SendMessage", "params": {"message": {
        "messageId": message_id, "role": "ROLE_USER", "parts": [{"text": text}]}}})
}

struct StopOnDrop<'a>(&'a AtomicBool);

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn script(reply: &str) -> Vec<(&'static str, Value)> {
    vec![
        (
            "source_read",
            json!({"source": "docs", "kind": "search", "input": {"query": "deploy"}}),
        ),
        ("reply_propose", json!({"text": reply})),
    ]
}

/// Serves `plugin` and runs `asking` against it; the answers `asking` returns.
fn served<T: Send>(
    fixture: &Fixture,
    plugin: &SpecPlugin,
    asking: impl FnOnce(SocketAddr) -> T + Send,
) -> T {
    let exposure = plugin.spec().expose.clone().unwrap().a2a.unwrap();
    let server = listener(&exposure, Bearer::new(TOKEN).unwrap()).expect("the listener is built");
    let host = Host::new(plugin.config()).with_environment(fixture.environment());
    let agent = Answering::with_host(plugin, host);
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    let stop = AtomicBool::new(false);
    let answers = std::thread::scope(|scope| {
        let asked = scope.spawn(|| {
            let _stop = StopOnDrop(&stop);
            asking(address)
        });
        server
            .serve(socket, &agent, &stop)
            .expect("the listener ends cleanly");
        asked.join().expect("the client asked")
    });
    drop(agent);
    answers
}

fn record_lines(fixture: &Fixture) -> Vec<Value> {
    fs::read_to_string(fixture.plugin_state().join("record.jsonl"))
        .expect("the record is written")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect()
}

fn record_of(answer: &Value) -> Value {
    answer["result"]["task"]["artifacts"][0]["parts"][1]["data"].clone()
}

/// The first caller's `messageId` is no plain token, so its revision is `m-` and a hash; the
/// second caller's `messageId` is that spelling, a plain token kept as it is. The two are
/// different messages: each must run its own turn and append its own line. Instead the second is
/// `replayed` the first caller's record line, its own question never asked.
#[test]
fn adversary_a_message_id_spelling_another_ids_revision_runs_its_own_turn() {
    let fixture = fixture("collision");
    let (turns, made): (TurnModels, Arc<Mutex<usize>>) =
        support::turns(vec![script(REPLY), script(OTHER)]);
    let plugin = SpecPlugin::new(spec(&fixture), &fixture.plugin_state())
        .expect("the spec is served")
        .with_classifier(Box::new(Recorded::classifying("ask", &["docs"], 0.9)))
        .with_turn_models(turns);
    let first_id = "deploy status, please";
    let second_id = item_revision(first_id);
    let (first, second) = served(&fixture, &plugin, |address| {
        let first = post(address, &send(first_id, "Is the deploy finished?"));
        let second = post(address, &send(&second_id, "What did the logs say?"));
        (first, second)
    });
    assert_eq!(record_of(&first)["proposal"], REPLY, "{first}");
    assert_eq!(
        *made.lock().unwrap(),
        2,
        "messageId {second_id:?} ran no turn; it was answered with {}",
        record_of(&second)
    );
    assert_eq!(record_of(&second)["proposal"], OTHER, "{second}");
    assert_eq!(record_lines(&fixture).len(), 2);
}

/// Repeats of one `messageId` sent at once are decided one at a time on the thread that runs
/// turns: one turn, one line, every caller answered with it.
#[test]
fn adversary_repeats_of_one_message_id_sent_at_once_run_one_turn() {
    let fixture = fixture("concurrent");
    let (turns, made): (TurnModels, Arc<Mutex<usize>>) = support::turns(vec![script(REPLY)]);
    let plugin = SpecPlugin::new(spec(&fixture), &fixture.plugin_state())
        .expect("the spec is served")
        .with_classifier(Box::new(Recorded::classifying("ask", &["docs"], 0.9)))
        .with_turn_models(turns);
    let answers = served(&fixture, &plugin, |address| {
        std::thread::scope(|scope| {
            let asking: Vec<_> = (0..6)
                .map(|_| scope.spawn(move || post(address, &send("dup-1", "Is it finished?"))))
                .collect();
            asking
                .into_iter()
                .map(|asked| asked.join().expect("asked"))
                .collect::<Vec<_>>()
        })
    });
    assert_eq!(*made.lock().unwrap(), 1, "{answers:?}");
    for answer in &answers {
        assert_eq!(record_of(answer)["proposal"], REPLY, "{answer}");
    }
    assert_eq!(record_lines(&fixture).len(), 1);
}
