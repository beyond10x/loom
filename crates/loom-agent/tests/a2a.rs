//! Acceptance for `story:agent-a2a` on a served spec: an A2A message runs one governed turn of the
//! spec's agent on `inbound-answer@1`, answered with the turn's record, and nothing is sent through
//! A2A even when the spec grants `reply.send`. The listener refuses to start without its bearer.
//!
//! The listener runs on a loopback port this file binds; every read goes to the fake `connectors`
//! of `loom-connectors`, answering the slack-handler's `docs` fixtures; the models are recorded.
//! Nothing reaches a network beyond 127.0.0.1, a credential or a real channel.

#[path = "../../loom-plugin-slack/tests/support/mod.rs"]
mod support;

use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use b10x_loom_a2a::{Answerer, Bearer};
use b10x_loom_agent::SpecPlugin;
use b10x_loom_agent::serve::a2a::{
    Answering, BEARER_ENV, answer_projection, bearer, listener, port,
};
use b10x_loom_plugin::{Host, Projection, RecordOutcome};
use b10x_loom_plugin_slack::TurnModels;
use loom_sdk::agent::{A2aExposure, AgentSpec, parse};
use loom_sdk::commission::model::responsibility::RuntimeEvent;
use serde_json::{Value, json};
use support::host::{Fixture, Recorded};

const WRITE: &str = "message.post";
const PATH: &str = "/a2a";
const REPLY: &str = "The deploy finished at 10:02; the logs show no error.";
/// The bearer of these cases: a throwaway value of this file only.
const TOKEN: &str = "a2a-agent-test-bearer-41d9";

/// A fixture directory whose fake answers the `docs` search and the write from the shared
/// fixtures, so a send, were one made, would be logged.
fn fixture(test: &str) -> Fixture {
    let fixture = Fixture::new("agent-a2a", test);
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

/// A spec granting `reply.send` in `writes.mode: approval`, exposing A2A on `port`.
fn spec(fixture: &Fixture, port: i64) -> AgentSpec {
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
  a2a: {{port: {port}, path: {PATH}, card: {{name: test-operator, description: Answers about the deploy, skills: [logs, metrics]}}}}
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

fn plugin(fixture: &Fixture, scripts: Vec<Vec<(&'static str, Value)>>) -> SpecPlugin {
    let (turns, _made): (TurnModels, _) = support::turns(scripts);
    SpecPlugin::new(spec(fixture, 8080), &fixture.plugin_state())
        .expect("the spec is served")
        .with_classifier(Box::new(Recorded::classifying("ask", &["docs"], 0.9)))
        .with_turn_models(turns)
}

/// One POST of `body` with the bearer; the answer's JSON.
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

/// Sets `stop` when dropped, so a failed assertion still ends the listener.
struct StopOnDrop<'a>(&'a AtomicBool);

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn a_message_runs_one_governed_turn_and_sends_nothing() {
    let fixture = fixture("a2a-one-turn");
    let plugin = plugin(
        &fixture,
        vec![vec![
            (
                "source_read",
                json!({"source": "docs", "kind": "search", "input": {"query": "deploy"}}),
            ),
            ("reply_propose", json!({"text": REPLY})),
            (
                "reply_send",
                json!({"channel": support::CHANNEL, "thread_ts": "1.2", "text": REPLY}),
            ),
        ]],
    );
    let exposure = plugin.spec().expose.clone().unwrap().a2a.unwrap();
    let server = listener(&exposure, Bearer::new(TOKEN).unwrap()).expect("the listener is built");
    let host = Host::new(plugin.config()).with_environment(fixture.environment());
    let agent = Answering::with_host(&plugin, host);
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    let stop = AtomicBool::new(false);

    let answer = std::thread::scope(|scope| {
        // The agent is not shared between threads: it serves on this one, the client asks from
        // another.
        let asking = scope.spawn(|| {
            let _stop = StopOnDrop(&stop);
            post(
                address,
                &json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "SendMessage",
                    "params": {"message": {
                        "messageId": "m-1",
                        "role": "ROLE_USER",
                        "parts": [{"text": "Is the deploy finished?"}],
                    }},
                }),
            )
        });
        server
            .serve(socket, &agent, &stop)
            .expect("the listener ends cleanly");
        asking.join().expect("the client asked")
    });

    let task = &answer["result"]["task"];
    assert_eq!(task["status"]["state"], "TASK_STATE_COMPLETED", "{answer}");
    let parts = &task["artifacts"][0]["parts"];
    assert_eq!(parts[0]["text"], REPLY, "{answer}");
    let record = &parts[1]["data"];
    assert_eq!(record["outcome"], "proposed", "{record}");
    assert_eq!(record["intent"], "ask", "{record}");
    assert_eq!(record["reads"][0]["source"], "docs", "{record}");
    assert_eq!(
        record["item"],
        format!("a2a:{}", task["id"].as_str().unwrap()),
        "{record}"
    );
    assert!(
        fixture.runs("invoke").iter().all(|argv| !argv
            .windows(2)
            .any(|w| w[0] == "--operation" && w[1] == WRITE)),
        "nothing is sent through A2A"
    );
    assert!(
        fixture
            .runs("invoke")
            .iter()
            .any(|argv| argv.iter().any(|arg| arg == support::SEARCH)),
        "the turn read its source"
    );
    assert!(
        !fixture.plugin_state().join("approvals").exists(),
        "no send is held either"
    );
}

#[test]
fn the_listener_needs_its_bearer_and_a_port() {
    for value in [None, Some(""), Some("  ")] {
        let refused = bearer(value).expect_err("refused").to_string();
        assert!(refused.contains(BEARER_ENV), "{refused}");
    }
    assert!(bearer(Some(TOKEN)).is_ok());

    let fixture = fixture("a2a-port");
    let declared = spec(&fixture, 8080).expose.unwrap().a2a.unwrap();
    // The decoder refuses these already; the listener does not rely on it.
    for wrong in [0, -1, 65_536] {
        let exposure = A2aExposure {
            port: wrong,
            ..declared.clone()
        };
        let refused = port(&exposure).expect_err("refused").to_string();
        assert!(refused.contains("expose.a2a.port"), "{refused}");
        assert!(listener(&exposure, Bearer::new(TOKEN).unwrap()).is_err());
    }
    assert_eq!(port(&declared).unwrap(), 8080);
}

#[test]
fn the_turns_runtime_events_reach_the_sink() {
    let fixture = fixture("a2a-events");
    let plugin = plugin(
        &fixture,
        vec![vec![
            (
                "source_read",
                json!({"source": "docs", "kind": "search", "input": {"query": "deploy"}}),
            ),
            ("reply_propose", json!({"text": REPLY})),
        ]],
    );
    let host = Host::new(plugin.config()).with_environment(fixture.environment());
    let agent = Answering::with_host(&plugin, host);
    let mut events: Vec<RuntimeEvent> = Vec::new();

    let record = agent
        .answer(&support::host::item("a2a:events"), &mut events)
        .expect("the turn runs");

    assert_eq!(record.outcome, RecordOutcome::Proposed, "{record:?}");
    assert!(
        matches!(events.first(), Some(RuntimeEvent::StepStarted(_))),
        "{events:?}"
    );
    assert!(
        matches!(events.last(), Some(RuntimeEvent::Stopped(_))),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::ActionSelected(selected) if selected.action == "reply.propose")),
        "{events:?}"
    );
}

// ---- adversary pass 1 (story:agent-a2a) -----------------------------------------------------------

/// A Kubernetes Secret written from a file (`kubectl create secret --from-file`, a sealed secret
/// made from `echo token > file`) carries the file's trailing newline into `LOOM_A2A_BEARER`, and
/// no HTTP header can carry one back. The host either refuses such a value at start, naming the
/// variable, or holds the token without it; it never starts a listener that refuses every client.
#[test]
fn adversary_a_bearer_with_a_secret_files_trailing_newline_still_admits_its_token() {
    use b10x_loom_agent::serve::a2a::AdmitA2aRequestOutcome;

    let value = format!("{TOKEN}\n");
    match bearer(Some(&value)) {
        Err(refused) => assert!(refused.to_string().contains(BEARER_ENV), "{refused}"),
        Ok(held) => assert_eq!(
            b10x_loom_a2a::admit::admit(&held, false, Some(&format!("Bearer {TOKEN}"))),
            AdmitA2aRequestOutcome::Admitted,
            "the listener started with a bearer no request can present"
        ),
    }
}

/// The first deployed agent of the design (slice A: http reads and the A2A answer, no chat
/// channel) declares `expose.a2a` and no `channel`. It is served: its A2A messages run their turn.
#[test]
fn adversary_a_spec_exposing_a2a_without_a_channel_answers_its_messages() {
    let fixture = fixture("a2a-no-channel");
    let spec = parse(&format!(
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
  a2a: {{port: 8080, path: {PATH}, card: {{name: test-operator, description: Answers about the deploy, skills: [logs, metrics]}}}}
runtime:
  state_dir: {state}
  connectors: {{program: {program}, state_dir: {connectors}}}
  confinement: none
",
        state = fixture.plugin_state().display(),
        program = fixture.root.join("connectors").display(),
        connectors = fixture.connectors_state().display(),
    ))
    .expect("the slice A spec decodes");
    let (turns, _made): (TurnModels, _) = support::turns(vec![vec![
        (
            "source_read",
            json!({"source": "docs", "kind": "search", "input": {"query": "deploy"}}),
        ),
        ("reply_propose", json!({"text": REPLY})),
    ]]);
    let plugin = SpecPlugin::new(spec, &fixture.plugin_state())
        .unwrap_or_else(|refused| {
            panic!("a spec with expose.a2a and no channel cannot be served: {refused}")
        })
        .with_classifier(Box::new(Recorded::classifying("ask", &["docs"], 0.9)))
        .with_turn_models(turns);
    let host = Host::new(plugin.config()).with_environment(fixture.environment());
    let agent = Answering::with_host(&plugin, host);
    let mut events: Vec<RuntimeEvent> = Vec::new();
    let record = agent
        .answer(&support::host::item("a2a:no-channel"), &mut events)
        .expect("the turn runs");
    assert_eq!(record.outcome, RecordOutcome::Proposed, "{record:?}");
    assert_eq!(record.proposal.as_deref(), Some(REPLY), "{record:?}");
}

// ---- fix round 1 (story:agent-a2a) ----------------------------------------------------------------

/// Nothing is sent through A2A by two locks, each held here on its own: the A2A turn's protocol
/// declares no `reply.send`, so its frontier never offers one, and the projection the turn runs on
/// holds none even when the state the spec routes the intent to grants it.
#[test]
fn an_a2a_turn_is_never_offered_reply_send() {
    let catalog = loom_sdk::ProtocolCatalog::plugins().expect("the plugin catalog");
    let protocol = catalog
        .get(b10x_loom_plugin::PROTOCOL)
        .expect("the A2A turn's protocol is in the catalog")
        .actions();
    assert!(
        !protocol.iter().any(|action| action == "reply.send"),
        "{protocol:?}"
    );

    let fixture = fixture("a2a-projection");
    let spec = spec(&fixture, 8080);
    let granted = spec.states[0].actions.clone();
    assert!(
        granted.iter().any(|action| action == "reply.send"),
        "{granted:?}"
    );
    let projection = answer_projection(Projection {
        actions: granted,
        sources: Vec::new(),
    });
    assert_eq!(
        projection.actions,
        ["source.read", "reply.propose", "reply.decline"],
        "the A2A projection keeps every action but reply.send"
    );
}

/// A spec with neither a `channel` nor `expose.a2a` is not served: `nothing-to-serve`
/// (`loom.agent.AdmitServe`), before anything runs.
#[test]
fn a_spec_with_neither_a_channel_nor_a2a_is_not_served() {
    let fixture = fixture("a2a-nothing");
    let mut spec = spec(&fixture, 8080);
    spec.expose = None;
    spec.channel = None;
    let refused = SpecPlugin::new(spec, &fixture.plugin_state())
        .err()
        .expect("refused")
        .to_string();
    assert!(
        refused.contains("nothing would be polled or answered"),
        "{refused}"
    );
}
