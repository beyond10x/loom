// SPDX-License-Identifier: Apache-2.0

//! Acceptance for `story:harness-loop-port`: the ported Harness loop, wired to Loom's projection,
//! selection and revalidation, over a provider-emulated endpoint on the Responses wire, with the
//! Commission fake governor serving the software-change frontier.
//!
//! The frontiers are the two states of ELS `docs/examples/software-change.md` (case CHG-1842) that
//! `tests/agent_executor.rs` transcribes: "Initial", where merge is blocked, and "After `tests.run`
//! on R2", where merge needs approval. The governor answers "Initial" twice and "After" from then
//! on, so the frontier changes between the first run's two turns.
//!
//! The endpoint is a socket in this process speaking HTTP/1.1 with a chunked event stream, in the
//! shape of the pinned Responses subset (the endpoint of `tests/session_transcript_streaming.rs`,
//! without its lockstep). It records every request body it is sent and answers one scripted turn
//! per request.
//!
//! Two runs on one Loom and one session:
//!
//! 1. through `Loom::run_loop`: the model calls `repository_merge` while merge is blocked, is
//!    refused, and answers in prose on the next turn;
//! 2. through `LoopExecutor`, Commission's `AgentExecutor` port: the model calls `tests_run`, and
//!    Loom returns it as a `ProposedAction`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorOutcomeProposedAction, Frontier, FrontierAction,
    FrontierClaim, FrontierData, FrontierId, PrincipalId, ProposedActionArguments, Truth, Unit,
    commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};
use b10x_loom_executor::harness::governed::{LoopExecutor, LoopPorts, tool_name};
use b10x_loom_executor::harness::responses::{self, ResponsesClient};
use b10x_loom_executor::harness::turn_loop::{LoopConfig, LoopEvent, LoopStop, VecLoopSink};
use b10x_loom_executor::harness::wire::{CallId, Item, StaticBearer, ToolCall, ToolName, WireId};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{
    CatalogueId, CommissionRunId, RevalidateSelectionOutcome, SelectionState, SelectionStrategy,
    SessionData, SessionId, TurnId,
};
use b10x_loom_executor::projection::project;
use b10x_loom_executor::{EmptyObjectArguments, FirstAdmissibleSelector, Loom};
use serde_json::{Value, json};

const CASE: &str = "CHG-1842";
const MODEL: &str = "emulated-model";
const PROMPT: &str = "PROMPT-land-the-change";
const SESSION: &str = "00000000-0000-4000-8000-00000000c012";
const RUN: &str = "00000000-0000-4000-8000-00000000c0ab";
const MERGE: &str = "repository.merge";
const TESTS_RUN: &str = "tests.run";

/// The opaque reasoning item the first turn opens with, exactly as the endpoint emits it.
const REASONING_ITEM: &str =
    r#"{"encrypted_content":"OPAQUE-REASONING-1","id":"rs_1","summary":[],"type":"reasoning"}"#;
/// The prose the second turn answers with.
const ANSWER: &str = "NOTHING-TO-PROPOSE";

#[test]
fn ported_loop_round_trip() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [
            answer(1, Truth::Unknown, initial_actions()),
            answer(1, Truth::Unknown, initial_actions()),
            answer(2, Truth::True, after_tests_actions()),
        ],
    );
    let endpoint = Endpoint::start(vec![merge_call_turn(), answer_turn(), tests_run_turn()]);
    let commission = commission(&case);
    let session = SessionData {
        session_id: SessionId(Uuid(SESSION.to_owned())),
        commission_run: CommissionRunId(Uuid(RUN.to_owned())),
        wire: responses::WIRE.to_owned(),
    };
    // The Loom's own selector and generator serve `AgentExecutor::run` on `Loom`; in the wired loop
    // the model's tool call is the selection and carries the arguments.
    let loom =
        Loom::new(FirstAdmissibleSelector, EmptyObjectArguments, PROMPT).with_governor(&governor);

    // Run 1, through `Loom::run_loop`: a call outside the catalogue, then prose.
    let handed = issued(&governor, &case);
    let mut client = responses_client(&endpoint);
    let mut sink = VecLoopSink::new();
    let first = loom.run_loop(
        &session,
        LoopPorts {
            model: &mut client,
            config: config(),
            sink: &mut sink,
        },
        &commission,
        &handed,
    );
    let answered = first
        .run
        .expect("the loop ran")
        .expect("the first run answers");
    assert_eq!(answered.stop, LoopStop::Completed);
    assert_eq!(
        first.outcome,
        ExecutorOutcome::CompletedLocalReasoning(Unit(true)),
        "3. a call outside the catalogue proposes nothing"
    );
    assert_eq!(
        loom.selections(),
        [],
        "3. the refused call never reached the selector"
    );
    assert_eq!(loom.argument_requests(), []);
    assert!(
        sink.events().iter().any(|event| matches!(
            event,
            LoopEvent::Warning { code, message }
                if code == "unpublished-tool" && message.contains("repository_merge")
        )),
        "the record names the refused call: {:?}",
        sink.events()
    );

    // Run 2, through Commission's `AgentExecutor` port: a catalogue action is proposed.
    let executor = LoopExecutor::new(
        &loom,
        responses_client(&endpoint),
        config(),
        session.clone(),
    );
    let handed = issued(&governor, &case);
    let second = executor.run(&commission, &handed);

    // 2. The model's call of a catalogue action is Loom's proposal, with the model's arguments.
    assert_eq!(
        second,
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: TESTS_RUN.to_owned(),
            arguments: ProposedActionArguments(CommissionValue::Object(vec![(
                "suite".to_owned(),
                CommissionValue::Text("unit".to_owned()),
            )])),
        })
    );
    // 2. The call reached the selector once: one selection, made by the reasoning model, of the
    // called action, at the catalogue's revision, and admitted at revalidation.
    let selections = loom.selections();
    assert_eq!(selections.len(), 1, "{selections:?}");
    assert_eq!(selections[0].data.action, TESTS_RUN);
    assert_eq!(
        selections[0].data.strategy,
        SelectionStrategy::ReasoningModel
    );
    assert_eq!(selections[0].data.case_revision, 2);
    assert_eq!(selections[0].state, SelectionState::Admitted);
    // 2. ... and the argument generator once: one argument request, for that selection.
    let requests = loom.argument_requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!(
        requests[0].data.selection_id,
        selections[0].data.selection_id
    );
    let revalidations = loom.revalidations();
    assert_eq!(revalidations.len(), 1, "{revalidations:?}");
    assert!(
        matches!(
            &revalidations[0],
            RevalidateSelectionOutcome::Admitted { .. }
        ),
        "{revalidations:?}"
    );

    // 1. Every request's tool list is the catalogue projected from the frontier the governor
    // issued for that turn: Initial, then After for the second turn and for the second run.
    let sent = endpoint.requests();
    assert_eq!(sent.len(), 3, "one request per turn");
    let expected = [
        published(1, &initial_actions()),
        published(2, &after_tests_actions()),
        published(2, &after_tests_actions()),
    ];
    for (at, (body, want)) in sent.iter().zip(&expected).enumerate() {
        assert_eq!(&tool_names(body), want, "request {}", at + 1);
    }
    assert_eq!(
        expected[0],
        ["repository_inspect", "repository_edit", "tests_run"],
        "merge is blocked on Initial, so it is not in the first catalogue"
    );
    assert_eq!(
        expected[1],
        [
            "repository_inspect",
            "repository_edit",
            "tests_run",
            "repository_merge"
        ],
    );
    // The frontier was read from the governor before each turn and once at revalidation; never
    // from the model.
    assert_eq!(
        governor.calls(),
        vec![GovernorCall::Frontier(case.clone()); 6],
        "handed, turn 1, turn 2 (run 1); handed, turn 1, revalidation (run 2)"
    );
    assert!(
        input_text(&sent[0]).contains(PROMPT),
        "the run works on the Loom's prompt"
    );

    // 3. The call outside the catalogue was answered to the model as a refusal naming it.
    let refusal = function_call_output(&sent[1], "call_merge").expect("the refusal was sent");
    assert!(refusal.contains("\"ok\":false"), "{refusal}");
    assert!(refusal.contains("repository_merge"), "{refusal}");

    // Each completed turn is recorded, once, into the session (`loom.run.RecordTurn`), with the
    // provider items it added, verbatim.
    let turns = loom.turns();
    assert_eq!(turns.len(), 3, "{turns:?}");
    let reasoning = Item::Opaque {
        wire: WireId::new(responses::WIRE).expect("valid"),
        payload: serde_json::from_str(REASONING_ITEM).expect("valid"),
    };
    let expected_items = [
        vec![
            reasoning,
            Item::ToolCall(call("call_merge", "repository_merge", json!({}))),
        ],
        vec![Item::assistant(ANSWER)],
        vec![Item::ToolCall(call(
            "call_tests",
            "tests_run",
            json!({"suite": "unit"}),
        ))],
    ];
    for (at, (turn, items)) in turns.iter().zip(&expected_items).enumerate() {
        assert_eq!(turn.data.session_id, session.session_id, "turn {}", at + 1);
        assert_eq!(turn.data.index, i64::try_from(at + 1).expect("small"));
        let encoded: Vec<String> = items
            .iter()
            .map(|item| serde_json::to_string(item).expect("encodes"))
            .collect();
        assert_eq!(turn.data.items, encoded, "turn {}", at + 1);
    }
    assert!(
        turns[0].data.turn_id != turns[1].data.turn_id
            && turns[1].data.turn_id != turns[2].data.turn_id
            && turns[0].data.turn_id != turns[2].data.turn_id,
        "{turns:?}"
    );
}

// --- the case -----------------------------------------------------------------------------------

fn action(name: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

/// "Initial": merge is blocked while inspect, edit and `tests.run` are admissible.
fn initial_actions() -> Vec<FrontierAction> {
    vec![
        action("repository.inspect", ActionStatus::Admissible, None),
        action("repository.edit", ActionStatus::Admissible, None),
        action(TESTS_RUN, ActionStatus::Admissible, None),
        action(MERGE, ActionStatus::Blocked, None),
    ]
}

/// "After `tests.run` on R2": merge needs approval; the others stay admissible.
fn after_tests_actions() -> Vec<FrontierAction> {
    vec![
        action("repository.inspect", ActionStatus::Admissible, None),
        action("repository.edit", ActionStatus::Admissible, None),
        action(TESTS_RUN, ActionStatus::Admissible, None),
        action(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
        ),
    ]
}

fn answer(revision: i64, tests_pass: Truth, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(
        vec![FrontierClaim {
            claim: "tests.pass".to_owned(),
            value: tests_pass,
        }],
        Vec::new(),
        actions,
    )
}

/// The tool names a request assembled on a frontier at `revision` listing `actions` carries: the
/// projected catalogue's entries, in order, each under its published name.
fn published(revision: i64, actions: &[FrontierAction]) -> Vec<String> {
    let frontier = Frontier::new(FrontierData {
        frontier_id: FrontierId(CommissionUuid(
            "00000000-0000-4000-8000-0000000000f0".to_owned(),
        )),
        case_id: CaseId(CASE.to_owned()),
        case_revision: revision,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: actions.to_vec(),
    });
    let id = || Uuid("00000000-0000-4000-8000-0000000000f1".to_owned());
    project(&frontier, CatalogueId(id()), TurnId(id()))
        .data()
        .entries
        .iter()
        .map(|entry| {
            tool_name(&entry.action)
                .expect("publishable")
                .as_str()
                .to_owned()
        })
        .collect()
}

fn issued(governor: &FakeGovernor, case: &CaseId) -> Frontier<frontier_state::Issued> {
    governor
        .frontier(case)
        .unwrap_or_else(|error| panic!("frontier for {} failed: {error:?}", case.0))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000001".to_owned(),
        )),
        agent_revision_id: AgentRevisionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000002".to_owned(),
        )),
        case_id: case.clone(),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

fn config() -> LoopConfig {
    LoopConfig::new(MODEL, "INSTRUCTIONS-standing").with_retry_backoff(Duration::from_millis(1))
}

fn responses_client(endpoint: &Endpoint) -> ResponsesClient {
    ResponsesClient::new(
        responses::Endpoint::new(endpoint.base_url.clone(), MODEL, 200_000).expect("endpoint"),
        Arc::new(StaticBearer::new("CREDENTIAL-test-only")),
    )
    .expect("client")
}

fn call(id: &str, name: &str, arguments: Value) -> ToolCall {
    ToolCall {
        call_id: CallId::new(id).expect("valid"),
        name: ToolName::new(name).expect("valid"),
        arguments,
    }
}

// --- reading what was sent ----------------------------------------------------------------------

fn tool_names(body: &Value) -> Vec<String> {
    body["tools"]
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .map(|tool| tool["name"].as_str().expect("a named tool").to_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn input_text(body: &Value) -> String {
    body["input"].to_string()
}

fn function_call_output(body: &Value, call_id: &str) -> Option<String> {
    body["input"].as_array()?.iter().find_map(|item| {
        (item["type"] == "function_call_output" && item["call_id"] == call_id)
            .then(|| item["output"].as_str().unwrap_or_default().to_owned())
    })
}

// --- the scripted turns -------------------------------------------------------------------------

fn completed(id: &str, output: &[Value]) -> Value {
    json!({"type": "response.completed", "response": {
        "id": id, "status": "completed", "model": MODEL, "output": output,
        "usage": {"input_tokens": 10, "output_tokens": 5}}})
}

/// The opaque reasoning item, then a call of `repository_merge`, which the first catalogue does
/// not list.
fn merge_call_turn() -> Vec<Value> {
    let reasoning: Value = serde_json::from_str(REASONING_ITEM).expect("valid");
    let call = json!({"type": "function_call", "id": "fc_1", "call_id": "call_merge",
                      "name": "repository_merge", "arguments": "{}"});
    vec![
        json!({"type": "response.output_item.done", "item": reasoning}),
        json!({"type": "response.output_item.added", "item": call}),
        json!({"type": "response.output_item.done", "item": call}),
        completed("resp_1", &[reasoning, call]),
    ]
}

/// Prose and no call.
fn answer_turn() -> Vec<Value> {
    let message = json!({"type": "message", "id": "msg_1", "role": "assistant", "status": "completed",
                         "content": [{"type": "output_text", "text": ANSWER}]});
    vec![
        json!({"type": "response.output_text.delta", "delta": ANSWER}),
        json!({"type": "response.output_item.done", "item": message}),
        completed("resp_2", &[message]),
    ]
}

/// A call of `tests_run`, a catalogue action, with arguments.
fn tests_run_turn() -> Vec<Value> {
    let call = json!({"type": "function_call", "id": "fc_2", "call_id": "call_tests",
                      "name": "tests_run", "arguments": "{\"suite\":\"unit\"}"});
    vec![
        json!({"type": "response.output_item.added", "item": call}),
        json!({"type": "response.output_item.done", "item": call}),
        completed("resp_3", &[call]),
    ]
}

// --- the provider-emulated endpoint -------------------------------------------------------------

struct Endpoint {
    base_url: String,
    requests: Arc<Mutex<Vec<Value>>>,
}

impl Endpoint {
    /// Serves `turns` in order, one per request; a request past the script gets a dropped stream.
    fn start(turns: Vec<Vec<Value>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base_url = format!("http://{}/v1", listener.local_addr().expect("address"));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        std::thread::spawn(move || {
            let mut turns = turns.into_iter();
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                let script = turns.next().unwrap_or_default();
                serve(stream, &script, &recorded);
            }
        });
        Self { base_url, requests }
    }

    fn requests(&self) -> Vec<Value> {
        self.requests.lock().expect("lock").clone()
    }
}

fn serve(mut stream: TcpStream, frames: &[Value], requests: &Mutex<Vec<Value>>) {
    let Some(body) = read_body(&stream) else {
        return;
    };
    requests
        .lock()
        .expect("lock")
        .push(serde_json::from_slice(&body).expect("a JSON request"));
    if frames.is_empty() {
        let _ = stream.shutdown(std::net::Shutdown::Both);
        return;
    }
    let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                transfer-encoding: chunked\r\nconnection: close\r\n\r\n";
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    for frame in frames {
        let data = format!(
            "data: {}\n\n",
            serde_json::to_string(frame).expect("encodes")
        );
        if write_chunk(&mut stream, data.as_bytes()).is_err() {
            return;
        }
    }
    let _ = write_chunk(&mut stream, b"data: [DONE]\n\n");
    let _ = stream.write_all(b"0\r\n\r\n");
    let _ = stream.flush();
}

fn write_chunk(stream: &mut TcpStream, bytes: &[u8]) -> std::io::Result<()> {
    stream.write_all(format!("{:x}\r\n", bytes.len()).as_bytes())?;
    stream.write_all(bytes)?;
    stream.write_all(b"\r\n")?;
    stream.flush()
}

fn read_body(stream: &TcpStream) -> Option<Vec<u8>> {
    let mut reader = BufReader::new(stream);
    let mut length = 0usize;
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    loop {
        line.clear();
        reader.read_line(&mut line).ok()?;
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        let (name, value) = header.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("content-length") {
            length = value.trim().parse().ok()?;
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(body)
}
