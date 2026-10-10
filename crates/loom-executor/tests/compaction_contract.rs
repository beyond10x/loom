// SPDX-License-Identifier: Apache-2.0

//! Acceptance for `story:compaction-contract`: a governed run (`Loom::run_loop`) whose session
//! crosses its compaction trigger, over a provider-emulated endpoint on the Responses wire with a
//! declared context window, with the Commission fake governor.
//!
//! The endpoint is a socket in this process speaking HTTP/1.1 with a chunked event stream, in the
//! shape of the pinned Responses subset (the endpoint of `tests/harness_loop_port.rs`). It answers
//! one scripted turn per request and **counts** each request's input itself, one token per
//! [`BYTES_PER_TOKEN`] bytes of its `input` and `tools`, the way a provider reports what it was
//! sent. It records every request body with the usage it reported for it.
//!
//! The run, on a window of [`WINDOW`] tokens:
//!
//! 1. turn 1, on the frontier at case revision 1: the model writes a long plan and calls
//!    `repository_merge`, which that frontier blocks, so the call is refused to the model and the
//!    run goes on. The plan takes the conversation past 80 % of the window;
//! 2. before turn 2 the loop compacts: the plan is folded into a summary the model writes on a
//!    request of its own, the compaction request;
//! 3. turn 2, on the frontier the governor has moved to case revision 2: the model answers.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    Unit, commission_state, frontier_state,
};
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};
use b10x_loom_executor::harness::governed::{LoopPorts, tool_name};
use b10x_loom_executor::harness::responses::{self, ResponsesClient};
use b10x_loom_executor::harness::turn_loop::{
    LoopConfig, LoopEvent, LoopStop, SUMMARY_MARKER, VecLoopSink,
};
use b10x_loom_executor::harness::wire::StaticBearer;
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{
    CatalogueId, CommissionRunId, ReportedUsage, SessionData, SessionId, SessionState, TurnId,
};
use b10x_loom_executor::projection::project;
use b10x_loom_executor::{EmptyObjectArguments, FirstAdmissibleSelector, Loom};
use serde_json::{Value, json};

const CASE: &str = "CMP-0012";
const MODEL: &str = "emulated-model";
const PROMPT: &str = "PROMPT-land-the-change";
const INSTRUCTIONS: &str = "INSTRUCTIONS-standing";
const SESSION: &str = "00000000-0000-4000-8000-00000000c0c1";
const RUN: &str = "00000000-0000-4000-8000-00000000c0c2";
/// The context window the endpoint declares, in tokens; the loop is run on the same window.
const WINDOW: u64 = 4_000;
/// How many bytes of a request the emulated provider counts as one token.
const BYTES_PER_TOKEN: u64 = 4;
/// What the model writes when it is asked for a summary.
const SUMMARY: &str = "SUMMARY-by-the-model: a plan was written and repository_merge was refused.";
/// What the model answers on the first turn after compaction.
const ANSWER: &str = "ANSWER-after-compaction";

#[test]
fn compaction_contract() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    // Read for the frontier the run is handed, then once before each conversation turn: revision 1
    // for the handed frontier and turn 1, revision 2 from turn 2 on.
    governor.script(
        case.clone(),
        [
            answer(1, before_actions()),
            answer(1, before_actions()),
            answer(2, after_actions()),
        ],
    );
    let endpoint = Endpoint::start(vec![
        Scripted {
            output: vec![message("msg_plan", &plan()), merge_call()],
            output_tokens: 3_517,
            cached: 0,
        },
        Scripted {
            output: vec![message("msg_summary", SUMMARY)],
            output_tokens: 19,
            cached: 11,
        },
        Scripted {
            output: vec![message("msg_answer", ANSWER)],
            output_tokens: 5,
            cached: 3,
        },
    ]);
    let session = SessionData {
        session_id: SessionId(Uuid(SESSION.to_owned())),
        commission_run: CommissionRunId(Uuid(RUN.to_owned())),
        wire: responses::WIRE.to_owned(),
        boundary_refusals: 0,
    };
    let loom =
        Loom::new(FirstAdmissibleSelector, EmptyObjectArguments, PROMPT).with_governor(&governor);

    let handed = issued(&governor, &case);
    let mut client = responses_client(&endpoint);
    let mut sink = VecLoopSink::new();
    let run = loom.run_loop(
        &session,
        LoopPorts {
            model: &mut client,
            config: LoopConfig::new(MODEL, INSTRUCTIONS)
                .with_retry_backoff(Duration::from_millis(1))
                .with_context_window(Some(WINDOW)),
            sink: &mut sink,
        },
        &commission(&case),
        &handed,
    );
    let answered = run
        .run
        .expect("the loop ran")
        .expect("the run answers after compacting");
    assert_eq!(answered.stop, LoopStop::Completed);
    assert_eq!(
        run.outcome,
        ExecutorOutcome::CompletedLocalReasoning(Unit(true))
    );

    let sent = endpoint.requests();
    assert_eq!(
        sent.len(),
        3,
        "turn 1, the compaction request, turn 2: {:?}",
        sink.events()
    );
    let (before, compaction, after) = (&sent[0], &sent[1], &sent[2]);
    assert!(
        tool_names(&compaction.body).is_empty(),
        "the compaction request is the summary turn, which publishes no tool"
    );
    assert!(
        input_text(&compaction.body).contains("PLAN-step"),
        "the compaction request folds the plan"
    );
    let compacted: Vec<(usize, usize, bool)> = sink
        .events()
        .iter()
        .filter_map(|event| match event {
            LoopEvent::Compacted {
                bytes_before,
                bytes_after,
                summary_turn,
                ..
            } => Some((*bytes_before, *bytes_after, *summary_turn)),
            _ => None,
        })
        .collect();
    let [(bytes_before, bytes_after, true)] = compacted.as_slice() else {
        panic!("one compaction, made with a summary turn: {compacted:?}");
    };

    // 1. The session ran past the trigger (80 % of the window) and, after compaction, is at or
    // below 50 % of it: by the loop's own measure of the conversation, and by the endpoint's
    // count of the first request after compaction, instructions and tools included.
    assert!(
        tokens(*bytes_before) * 100 >= WINDOW * 80,
        "the session was past the trigger: {bytes_before} bytes of a {WINDOW} token window"
    );
    assert!(
        tokens(*bytes_after) * 100 <= WINDOW * 50,
        "{bytes_after} bytes after compaction"
    );
    let counted = after.usage["input_tokens"]
        .as_u64()
        .expect("the endpoint counted the request");
    assert!(
        counted * 100 <= WINDOW * 50,
        "the first request after compaction is {counted} tokens of a {WINDOW} token window"
    );

    // 2. The governor moved the frontier to a new case revision whose admissible set differs
    // between the last request before compaction and the first after it. The first request after
    // compaction carries the catalogue projected from the new frontier, not the one before it.
    assert_eq!(
        governor.calls(),
        vec![GovernorCall::Frontier(case.clone()); 3],
        "handed, turn 1, turn 2: read before each turn, never carried over a compaction"
    );
    let catalogue_before = published(1, &before_actions());
    let catalogue_after = published(2, &after_actions());
    assert_eq!(tool_names(&before.body), catalogue_before);
    assert_eq!(tool_names(&after.body), catalogue_after);
    assert_ne!(
        tool_names(&after.body),
        tool_names(&before.body),
        "the tool list moved with the frontier"
    );
    assert_eq!(
        catalogue_before,
        ["repository_inspect", "repository_edit", "tests_run"]
    );
    assert_eq!(
        catalogue_after,
        ["repository_inspect", "tests_run", "repository_merge"]
    );

    // 3. The filed session holds exactly one compaction record, and its usage is the usage the
    // endpoint reported for the compaction request.
    let filed = loom
        .sessions()
        .into_iter()
        .find(|held| held.data.session_id == session.session_id)
        .expect("the session is held");
    assert_eq!(filed.state, SessionState::Filed);
    let records = loom.compactions();
    assert_eq!(records.len(), 1, "{records:?}");
    assert_eq!(records[0].data.session_id, session.session_id);
    let reported = reported_usage(&compaction.usage);
    assert_eq!(records[0].data.usage, Some(reported.clone()));
    for other in [before, after] {
        assert_ne!(
            reported_usage(&other.usage),
            reported,
            "the record is the compaction request's usage, not a conversation turn's"
        );
    }

    // 4. The instruction text of the first request after compaction is that of the last request
    // before it, byte for byte; the summary the model wrote appears only as conversation content,
    // verbatim under the loop's summary marker, never in the instruction text or a tool.
    let instruction = |body: &Value| -> String {
        let head = &body["input"][0];
        assert_eq!(head["role"], "developer", "the instruction heads the input");
        serde_json::to_string(head).expect("encodes")
    };
    assert_eq!(instruction(&after.body), instruction(&before.body));
    assert_eq!(
        after.body["input"][0]["content"][0]["text"], INSTRUCTIONS,
        "the run's own standing instruction"
    );
    for request in &sent {
        assert!(
            !instruction(&request.body).contains("SUMMARY-by-the-model"),
            "no instruction carries what the model wrote"
        );
        assert!(
            !request.body["tools"]
                .to_string()
                .contains("SUMMARY-by-the-model"),
            "no tool carries what the model wrote"
        );
    }
    assert_eq!(
        after.body["input"][1], before.body["input"][1],
        "the task is never folded"
    );
    let input = after.body["input"].as_array().expect("an input array");
    let carrying: Vec<&Value> = input
        .iter()
        .filter(|item| item.to_string().contains("SUMMARY-by-the-model"))
        .collect();
    assert_eq!(carrying.len(), 1, "{input:?}");
    assert_ne!(carrying[0]["role"], "developer");
    assert_eq!(
        carrying[0]["content"][0]["text"],
        format!("{SUMMARY_MARKER}\n{SUMMARY}"),
        "the summary stands as the model wrote it, under the loop's marker"
    );
    assert!(
        !input_text(&after.body).contains("PLAN-step"),
        "the plan itself was folded away"
    );
    // The summary turn is not a turn of the session: two turns are recorded, neither with it.
    let turns = loom.turns();
    assert_eq!(turns.len(), 2, "{turns:?}");
    assert!(
        turns
            .iter()
            .all(|turn| !turn.data.items.concat().contains("SUMMARY-by-the-model"))
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

/// Revision 1: inspect, edit and `tests.run` are admissible, merge is blocked.
fn before_actions() -> Vec<FrontierAction> {
    vec![
        action("repository.inspect", ActionStatus::Admissible, None),
        action("repository.edit", ActionStatus::Admissible, None),
        action("tests.run", ActionStatus::Admissible, None),
        action("repository.merge", ActionStatus::Blocked, None),
    ]
}

/// Revision 2: edit is blocked and merge needs approval; inspect and `tests.run` stay admissible.
fn after_actions() -> Vec<FrontierAction> {
    vec![
        action("repository.inspect", ActionStatus::Admissible, None),
        action("repository.edit", ActionStatus::Blocked, None),
        action("tests.run", ActionStatus::Admissible, None),
        action(
            "repository.merge",
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
        ),
    ]
}

fn answer(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
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

fn responses_client(endpoint: &Endpoint) -> ResponsesClient {
    ResponsesClient::new(
        responses::Endpoint::new(endpoint.base_url.clone(), MODEL, WINDOW).expect("endpoint"),
        Arc::new(StaticBearer::new("CREDENTIAL-test-only")),
    )
    .expect("client")
}

/// Tokens of `bytes`, by the emulated provider's count.
fn tokens(bytes: usize) -> u64 {
    u64::try_from(bytes).expect("small") / BYTES_PER_TOKEN
}

/// The usage the wire reads from what the endpoint reported, as the run model records it.
fn reported_usage(usage: &Value) -> ReportedUsage {
    let count = |value: &Value| value.as_i64().expect("a count");
    ReportedUsage {
        model: MODEL.to_owned(),
        input_tokens: count(&usage["input_tokens"]),
        output_tokens: count(&usage["output_tokens"]),
        cached_input_tokens: count(&usage["input_tokens_details"]["cached_tokens"]),
        cache_creation_input_tokens: None,
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

// --- the scripted turns -------------------------------------------------------------------------

/// The plan turn 1 writes: heavy enough to take the conversation past the trigger, and text, so
/// eliding tool results cannot reach it and only a summary can.
fn plan() -> String {
    "PLAN-step: read the change, run the tests, ask for the merge. ".repeat(230)
}

fn message(id: &str, text: &str) -> Value {
    json!({"type": "message", "id": id, "role": "assistant", "status": "completed",
           "content": [{"type": "output_text", "text": text}]})
}

/// A call of `repository_merge`, which the frontier at revision 1 blocks.
fn merge_call() -> Value {
    json!({"type": "function_call", "id": "fc_1", "call_id": "call_merge",
           "name": "repository_merge", "arguments": "{}"})
}

/// One scripted turn: what the model outputs, and the output and cached counts the endpoint
/// reports beside the input count it measures.
struct Scripted {
    output: Vec<Value>,
    output_tokens: u64,
    cached: u64,
}

// --- the provider-emulated endpoint -------------------------------------------------------------

/// One request the endpoint answered: its body, and the usage it reported for it.
#[derive(Debug, Clone)]
struct Sent {
    body: Value,
    usage: Value,
}

struct Endpoint {
    base_url: String,
    requests: Arc<Mutex<Vec<Sent>>>,
}

impl Endpoint {
    /// Serves `turns` in order, one per request; a request past the script gets a dropped stream.
    fn start(turns: Vec<Scripted>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base_url = format!("http://{}/v1", listener.local_addr().expect("address"));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        std::thread::spawn(move || {
            let mut turns = turns.into_iter();
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                serve(stream, turns.next(), &recorded);
            }
        });
        Self { base_url, requests }
    }

    fn requests(&self) -> Vec<Sent> {
        self.requests.lock().expect("lock").clone()
    }
}

/// The input tokens the emulated provider counts for a request: its `input` and its `tools`.
fn counted_input(body: &Value) -> u64 {
    let bytes = body["input"].to_string().len() + body["tools"].to_string().len();
    u64::try_from(bytes).expect("small") / BYTES_PER_TOKEN
}

fn serve(mut stream: TcpStream, turn: Option<Scripted>, requests: &Mutex<Vec<Sent>>) {
    let Some(body) = read_body(&stream) else {
        return;
    };
    let body: Value = serde_json::from_slice(&body).expect("a JSON request");
    let Some(turn) = turn else {
        requests.lock().expect("lock").push(Sent {
            body,
            usage: Value::Null,
        });
        let _ = stream.shutdown(std::net::Shutdown::Both);
        return;
    };
    let usage = json!({
        "input_tokens": counted_input(&body),
        "input_tokens_details": {"cached_tokens": turn.cached},
        "output_tokens": turn.output_tokens,
    });
    requests.lock().expect("lock").push(Sent {
        body,
        usage: usage.clone(),
    });
    let mut frames = Vec::new();
    for item in &turn.output {
        if item["type"] == "function_call" {
            frames.push(json!({"type": "response.output_item.added", "item": item}));
        } else if let Some(text) = item["content"][0]["text"].as_str() {
            frames.push(json!({"type": "response.output_text.delta", "delta": text}));
        }
        frames.push(json!({"type": "response.output_item.done", "item": item}));
    }
    frames.push(json!({"type": "response.completed", "response": {
        "id": "resp", "status": "completed", "model": MODEL, "output": turn.output,
        "usage": usage}}));

    let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                transfer-encoding: chunked\r\nconnection: close\r\n\r\n";
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    for frame in &frames {
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
