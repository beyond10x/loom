//! Acceptance for `story:agent-a2a` on a served spec: an A2A message runs one governed turn of the
//! spec's agent on `inbound-answer@1`, answered with the turn's record, and nothing is sent through
//! A2A even when the spec grants `reply.send`. The listener refuses to start without its bearer.
//! The turn's line is appended to the state directory's `record.jsonl` once per `messageId`
//! (`story:a2a-record-and-streaming`). A turn that cannot run answers its caller with a fixed reason
//! naming no path of the host, the full error on standard error
//! (`story:a2a-turn-error-names-no-host-path`).
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
use b10x_loom_plugin::{Host, PluginError, Projection, RecordOutcome};
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
    assert_eq!(record["item"], "a2a:m-1", "{record}");
    assert!(
        fixture
            .runs("invoke")
            .iter()
            .all(|argv| support::host::operation_of(argv) != Some(WRITE)),
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

// ---- story:a2a-record-and-streaming --------------------------------------------------------------

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

fn send(method: &str, message_id: &str, text: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": {"message": {
        "messageId": message_id, "role": "ROLE_USER", "parts": [{"text": text}]}}})
}

/// A `SendMessage` turn appends one line to the state directory's `record.jsonl`, its item keyed
/// by the revision the `messageId` gives (`loom.agent.RecordA2aTurn`, `turned`); the same
/// `messageId` again, sent or streamed, runs no second turn and is answered with that line
/// (`replayed`). A turn that could not run records nothing.
#[test]
fn an_a2a_turn_is_recorded_once() {
    let fixture = fixture("a2a-recorded-once");
    let (turns, made): (TurnModels, _) = support::turns(vec![vec![
        (
            "source_read",
            json!({"source": "docs", "kind": "search", "input": {"query": "deploy"}}),
        ),
        ("reply_propose", json!({"text": REPLY})),
    ]]);
    let plugin = SpecPlugin::new(spec(&fixture, 8080), &fixture.plugin_state())
        .expect("the spec is served")
        .with_classifier(Box::new(Recorded::classifying("ask", &["docs"], 0.9)))
        .with_turn_models(turns);
    let exposure = plugin.spec().expose.clone().unwrap().a2a.unwrap();
    let server = listener(&exposure, Bearer::new(TOKEN).unwrap()).expect("the listener is built");
    let host = Host::new(plugin.config()).with_environment(fixture.environment());
    let agent = Answering::with_host(&plugin, host);
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    let stop = AtomicBool::new(false);
    let made_after = |made: &std::sync::Arc<std::sync::Mutex<usize>>| *made.lock().unwrap();

    let (first, again, streamed, other, made_before_other) = std::thread::scope(|scope| {
        let asking = scope.spawn(|| {
            let _stop = StopOnDrop(&stop);
            let first = post(
                address,
                &send("SendMessage", "m-1", "Is the deploy finished?"),
            );
            let again = post(address, &send("SendMessage", "m-1", "Is it finished now?"));
            let streamed = post_raw(
                address,
                &send("SendStreamingMessage", "m-1", "Is the deploy finished?"),
            );
            let made_before_other = made_after(&made);
            let other = post(address, &send("SendMessage", "m-2", "And the logs?"));
            (first, again, streamed, other, made_before_other)
        });
        server
            .serve(socket, &agent, &stop)
            .expect("the listener ends cleanly");
        asking.join().expect("the client asked")
    });
    drop(agent);

    let record_of =
        |answer: &Value| answer["result"]["task"]["artifacts"][0]["parts"][1]["data"].clone();
    let recorded = record_of(&first);
    assert_eq!(
        first["result"]["task"]["status"]["state"], "TASK_STATE_COMPLETED",
        "{first}"
    );
    assert_eq!(recorded["item"], "a2a:m-1", "{recorded}");
    assert_eq!(recorded["proposal"], REPLY, "{recorded}");
    assert_eq!(
        again["result"]["task"]["status"]["state"], "TASK_STATE_COMPLETED",
        "{again}"
    );
    assert_eq!(
        record_of(&again),
        recorded,
        "the repeat is answered with the line"
    );
    assert_ne!(
        again["result"]["task"]["id"], first["result"]["task"]["id"],
        "each message is its own task"
    );

    let (head, body) = streamed;
    assert!(head.contains("text/event-stream"), "{head}");
    let events: Vec<Value> = body
        .split("\n\n")
        .filter(|event| !event.is_empty())
        .map(|event| serde_json::from_str(event.trim_start_matches("data: ")).unwrap())
        .collect();
    assert_eq!(
        events.len(),
        3,
        "submitted, the record, the end; no turn: {body}"
    );
    assert_eq!(
        events[1]["result"]["artifactUpdate"]["artifact"]["parts"][1]["data"], recorded,
        "{body}"
    );
    assert_eq!(
        events[2]["result"]["statusUpdate"]["status"]["state"], "TASK_STATE_COMPLETED",
        "{body}"
    );
    assert_eq!(made_before_other, 1, "one turn for one messageId");

    assert_eq!(
        other["result"]["task"]["status"]["state"], "TASK_STATE_FAILED",
        "no second turn is scripted: {other}"
    );
    assert_eq!(made_after(&made), 2, "a new messageId runs a turn");

    let lines: Vec<Value> = fs::read_to_string(fixture.plugin_state().join("record.jsonl"))
        .expect("the record is written")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect();
    assert_eq!(
        lines.len(),
        1,
        "one line, and none for a turn that could not run: {lines:?}"
    );
    assert_eq!(lines[0], recorded);
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

// ---- story:a2a-turn-error-names-no-host-path ----------------------------------------------------

/// Makes this test binary the child of
/// `a_turn_that_cannot_run_writes_its_full_error_to_standard_error`.
const TURN_ERROR_CHILD: &str = "LOOM_A2A_TEST_TURN_ERROR_CHILD";

/// What the child prints on standard output ahead of its full error, which the parent then looks
/// for on standard error; libtest may print the case's name ahead of it on the same line.
const FULL_ERROR: &str = "the full error: ";

/// The fixed reason an A2A caller is given for a turn whose plugin reports something it needs
/// unavailable (`serve::a2a::TURN_UNAVAILABLE`), spelt here as the caller reads it.
const UNAVAILABLE: &str = "something the turn needs is unavailable";

/// A served spec whose every turn fails as one whose model routes cannot be read does: the error
/// names the state directory, an absolute path of the host. Also that error's text.
fn failing_plugin(fixture: &Fixture) -> (SpecPlugin, String) {
    let state = fixture.plugin_state().display().to_string();
    let why = format!("the model routes {state}/routes.yaml cannot be read: permission denied");
    let error = why.clone();
    let turns: TurnModels = Box::new(move || Err(PluginError::Unavailable(error.clone())));
    let plugin = SpecPlugin::new(spec(fixture, 8080), &fixture.plugin_state())
        .expect("the spec is served")
        .with_classifier(Box::new(Recorded::classifying("ask", &["docs"], 0.9)))
        .with_turn_models(turns);
    (plugin, PluginError::Unavailable(why).to_string())
}

/// Whether `text` names a path of the host: a word that starts at `/` or `~/`.
fn names_a_path(text: &str) -> bool {
    text.split(|c: char| c.is_whitespace() || "`'\"(),;:=".contains(c))
        .any(|word| word.starts_with('/') || word.starts_with("~/"))
}

/// The text of a task status's message.
fn reason_of(status: &Value) -> String {
    status["message"]["parts"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// A turn whose executor fails with an error naming an absolute path answers the caller with a
/// fixed reason that names no path: on `SendMessage`, on `GetTask` of that task, and on a
/// stream's last `statusUpdate`.
#[test]
fn a_turn_that_cannot_run_answers_the_caller_naming_no_host_path() {
    let fixture = fixture("a2a-turn-error-no-path");
    let (plugin, why) = failing_plugin(&fixture);
    let state = fixture.plugin_state().display().to_string();
    assert!(
        state.starts_with('/') && why.contains(&state) && names_a_path(&why),
        "the turn's error names an absolute path: {why}"
    );
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
            let sent = post(
                address,
                &send("SendMessage", "m-1", "Is the deploy finished?"),
            );
            let task = sent["result"]["task"]["id"].clone();
            let got = post(
                address,
                &json!({"jsonrpc": "2.0", "id": 2, "method": "GetTask", "params": {"id": task}}),
            );
            let streamed = post_raw(
                address,
                &send("SendStreamingMessage", "m-2", "And the logs?"),
            );
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
    let expected = format!("the turn could not run: {UNAVAILABLE}");
    for (surface, status) in [
        ("SendMessage", &sent["result"]["task"]["status"]),
        ("GetTask", &got["result"]["status"]),
        (
            "the stream's last statusUpdate",
            &last["result"]["statusUpdate"]["status"],
        ),
    ] {
        assert_eq!(status["state"], "TASK_STATE_FAILED", "{surface}: {status}");
        let reason = reason_of(status);
        assert!(
            !names_a_path(&reason),
            "{surface} names a path of the host: {reason}"
        );
        assert_eq!(reason, expected, "{surface}: {status}");
    }
    for (surface, answer) in [
        ("SendMessage", sent.to_string()),
        ("GetTask", got.to_string()),
        ("the stream", body.clone()),
    ] {
        assert!(
            !answer.contains(&state),
            "{surface} names the state directory {state}: {answer}"
        );
    }
}

/// The full error of a turn that cannot run, its path included, goes to standard error. The case
/// runs its turn in a child of this test binary and reads the child's standard error.
#[test]
fn a_turn_that_cannot_run_writes_its_full_error_to_standard_error() {
    if std::env::var_os(TURN_ERROR_CHILD).is_some() {
        let fixture = fixture("a2a-turn-error-stderr");
        let (plugin, why) = failing_plugin(&fixture);
        let host = Host::new(plugin.config()).with_environment(fixture.environment());
        let agent = Answering::with_host(&plugin, host);
        let mut events: Vec<RuntimeEvent> = Vec::new();
        let failed = agent
            .answer(&support::host::item("a2a:m-1"), &mut events)
            .expect_err("the turn cannot run");
        assert!(!names_a_path(&failed), "{failed}");
        println!("{FULL_ERROR}{why}");
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().expect("the test binary"))
        .args([
            "a_turn_that_cannot_run_writes_its_full_error_to_standard_error",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(TURN_ERROR_CHILD, "1")
        .output()
        .expect("the child starts");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "the child ran its turn: {}\n{stdout}\n{stderr}",
        output.status
    );
    let why = stdout
        .lines()
        .find_map(|line| line.split_once(FULL_ERROR).map(|(_, why)| why))
        .expect("the child names its full error");
    assert!(names_a_path(why), "{why}");
    assert!(
        stderr.contains(&format!("b10x-loom: A2A: {UNAVAILABLE}: {why}")),
        "the full error reaches standard error: {stderr}"
    );
}

// ---- story:a2a-stderr-line-escapes-error-text ---------------------------------------------------

/// Makes this test binary the child of
/// `a_turn_error_holding_a_newline_writes_one_line_to_standard_error`.
const ESCAPED_ERROR_CHILD: &str = "LOOM_A2A_TEST_ESCAPED_ERROR_CHILD";

/// A turn whose error text holds a newline, a carriage return and an escape sequence, as text a
/// model produced might, and forges a second `b10x-loom: A2A:` line after the newline, writes
/// exactly one `b10x-loom: A2A:` line to standard error: no control character of the error
/// reaches it raw, and the newline is there escaped. The case runs its turn in a child of this
/// test binary and reads the child's standard error.
#[test]
fn a_turn_error_holding_a_newline_writes_one_line_to_standard_error() {
    if std::env::var_os(ESCAPED_ERROR_CHILD).is_some() {
        let fixture = fixture("a2a-turn-error-escaped");
        let why = "the intent was \"ask\nb10x-loom: A2A: forged: line\"\r\u{1b}[2K".to_owned();
        let turns: TurnModels = Box::new(move || Err(PluginError::Unavailable(why.clone())));
        let plugin = SpecPlugin::new(spec(&fixture, 8080), &fixture.plugin_state())
            .expect("the spec is served")
            .with_classifier(Box::new(Recorded::classifying("ask", &["docs"], 0.9)))
            .with_turn_models(turns);
        let host = Host::new(plugin.config()).with_environment(fixture.environment());
        let agent = Answering::with_host(&plugin, host);
        let mut events: Vec<RuntimeEvent> = Vec::new();
        agent
            .answer(&support::host::item("a2a:m-1"), &mut events)
            .expect_err("the turn cannot run");
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().expect("the test binary"))
        .args([
            "a_turn_error_holding_a_newline_writes_one_line_to_standard_error",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ESCAPED_ERROR_CHILD, "1")
        .output()
        .expect("the child starts");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "the child ran its turn: {}\n{stdout}\n{stderr}",
        output.status
    );
    let lines: Vec<&str> = stderr
        .lines()
        .filter(|line| line.contains("b10x-loom: A2A:"))
        .collect();
    assert_eq!(lines.len(), 1, "one A2A line on standard error: {stderr:?}");
    let line = lines[0];
    assert!(
        line.starts_with(&format!("b10x-loom: A2A: {UNAVAILABLE}: ")),
        "{line:?}"
    );
    assert!(
        !line.chars().any(|c| c.is_control() && c != '\t'),
        "no control character of the error reaches the line raw: {line:?}"
    );
    assert!(
        line.contains("\\u{a}b10x-loom: A2A: forged: line"),
        "the newline is escaped in the line: {line:?}"
    );
}
