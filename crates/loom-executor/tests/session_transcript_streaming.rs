// SPDX-License-Identifier: Apache-2.0

//! `story:session-transcript-streaming`, acceptance 1–7: sessions, transcripts and streaming over
//! the ported loop, against a provider-emulated endpoint on each wire.
//!
//! The endpoint is a socket in this process speaking HTTP/1.1 with a chunked event stream, in the
//! shape of the pinned Responses and Messages subsets. It records every request it is sent, and it
//! emits a scripted stream one frame at a time: after a frame carrying a delta it waits for the
//! loop's sink to report that delta before it emits the next frame (acceptance 1).
//!
//! No frontier tool is published. The run that drops mid-stream publishes one local test tool, so
//! that one turn completes before the drop.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use b10x_loom_executor::harness::messages::{self, MessagesClient};
use b10x_loom_executor::harness::responses::{self, ResponsesClient};
use b10x_loom_executor::harness::turn_loop::{
    ApproveAll, LoopConfig, LoopEvent, LoopSink, LoopStop,
};
use b10x_loom_executor::harness::wire::{
    Approval, Item, ModelPort, StaticBearer, ToolCall, ToolName, ToolOutcome, ToolPort, ToolSpec,
};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{CommissionRunId, RunEnding, SessionData, SessionId};
use b10x_loom_executor::session::{RunPorts, SessionError, SessionFile, run_and_file};
use serde_json::{Value, json};

const MODEL: &str = "emulated-model";
const CREDENTIAL: &str = "CREDENTIAL-7f3a9c-never-filed";
const INSTRUCTIONS: &str = "INSTRUCTION-TEXT-standing-instruction-never-filed";
const SESSION: &str = "00000000-0000-4000-8000-00000000c011";
const RUN: &str = "00000000-0000-4000-8000-00000000c0aa";
/// How long the endpoint waits for the sink to report a delta before it records a violation.
const LOCKSTEP: Duration = Duration::from_secs(10);

/// The opaque reasoning item the first turn ends with, exactly as the endpoint emits it: as a
/// provider sends it, its `summary` carries the text the turn streamed. Compact, keys in sorted
/// order, so its bytes are the same whatever map order a JSON encoder keeps.
const REASONING_ITEM: &str = r#"{"encrypted_content":"OPAQUE-REASONING-BLOB-1","id":"rs_1","summary":[{"text":"REASONING-SUMMARY-ALPHA REASONING-SUMMARY-BETA","type":"summary_text"}],"type":"reasoning"}"#;
const REASONING_DELTAS: [&str; 2] = ["REASONING-SUMMARY-ALPHA ", "REASONING-SUMMARY-BETA"];
const THINKING_DELTAS: [&str; 2] = ["THINKING-ALPHA ", "THINKING-BETA"];

#[test]
fn session_transcript_streaming() {
    let root = scratch("session_transcript_streaming");
    let workspace = root.join("workspace");
    let sessions = root.join("state").join("sessions");
    fs_create(&workspace);

    // A run that answered, on the Responses wire.
    let (first_endpoint, first_sink) = Emulator::start(Wire::Responses, vec![answered_turn()]);
    let mut session = SessionFile::open(
        &SessionData {
            session_id: session_id(),
            commission_run: CommissionRunId(Uuid(RUN.to_owned())),
            wire: responses::WIRE.to_owned(),
            boundary_refusals: 0,
        },
        MODEL,
        first_endpoint.base_url(),
        &workspace,
    )
    .expect("a session opens");
    let mut client = responses_client(&first_endpoint);
    let mut sink = StreamSink::new(first_sink);
    let filed = run_with(
        &mut client,
        None,
        &mut session,
        &sessions,
        "FIRST-QUESTION",
        &mut sink,
    );
    let answered = filed.run.expect("the first run answers");
    assert!(answered.stop == LoopStop::Completed, "{:?}", answered.stop);
    let path = filed.filed.expect("the answered run files its session");

    // 1. Each streamed delta reaches the stream sink before the endpoint emits the next one.
    first_endpoint.assert_lockstep();
    assert_eq!(
        sink.deltas,
        [REASONING_DELTAS[0], REASONING_DELTAS[1], "ANSWER-ONE"],
        "every delta reached the sink, in order"
    );

    // 2. Filed outside the workspace; directory 0700, file 0600.
    assert!(
        !path.starts_with(workspace.canonicalize().expect("workspace resolves")),
        "{} is inside the workspace",
        path.display()
    );
    assert_eq!(mode(path.parent().expect("a directory")), 0o700);
    assert_eq!(mode(&path), 0o600);
    let refused = session
        .clone()
        .file(&workspace.join(".sessions"), RunEnding::Answered)
        .expect_err("a directory inside the workspace is refused");
    assert!(
        refused.to_string().contains("inside workspace"),
        "{refused}"
    );
    assert!(
        !workspace.join(".sessions").exists(),
        "nothing written there"
    );

    // 3. Neither the credential the run used nor its instruction text is in the file.
    let first_requests = first_endpoint.requests();
    assert_eq!(first_requests.len(), 1);
    assert_eq!(
        first_requests[0].authorization.as_deref(),
        Some(format!("Bearer {CREDENTIAL}").as_str()),
        "the run used the credential"
    );
    assert!(
        contains(&first_requests[0].body, INSTRUCTIONS.as_bytes()),
        "the run sent the instruction text"
    );
    let filed_bytes = std::fs::read(&path).expect("the session file reads");
    assert!(
        !contains(&filed_bytes, CREDENTIAL.as_bytes()),
        "credential filed"
    );
    assert!(
        !contains(&filed_bytes, INSTRUCTIONS.as_bytes()),
        "instructions filed"
    );

    // 4. No streamed reasoning text outside the provider's opaque reasoning item; that item, byte
    // for byte.
    let stored = SessionFile::load(&sessions, &session_id()).expect("the session loads");
    assert_reasoning_only_inside_opaque(&stored, &filed_bytes, &REASONING_DELTAS);
    assert_eq!(stored.run_ending(), Some(RunEnding::Answered));
    let opaque: Vec<String> = stored
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Opaque { payload, .. } => Some(compact(payload)),
            _ => None,
        })
        .collect();
    assert_eq!(opaque, [REASONING_ITEM], "the opaque item, byte for byte");
    assert_eq!(
        stored.items,
        [
            Item::user("FIRST-QUESTION"),
            opaque_item(responses::WIRE, REASONING_ITEM),
            Item::assistant("ANSWER-ONE"),
        ]
    );

    // 6. Resuming on the other wire is refused before any request, naming both wires.
    let (messages_endpoint, _) = Emulator::start(Wire::Messages, vec![]);
    let other = messages_client(&messages_endpoint);
    let refusal = SessionFile::resume(&sessions, &session_id(), other.wire(), &workspace)
        .expect_err("a session recorded on another wire is refused");
    let SessionError::WireMismatch(mismatch) = &refusal else {
        panic!("not the cross-wire refusal: {refusal}");
    };
    assert_eq!(mismatch.session_id, session_id());
    assert_eq!(mismatch.session_wire, responses::WIRE);
    assert_eq!(mismatch.wire, messages::WIRE);
    let message = refusal.to_string();
    assert!(
        message.contains(responses::WIRE) && message.contains(messages::WIRE),
        "{message}"
    );
    assert!(messages_endpoint.requests().is_empty(), "nothing was sent");

    // 5. A second run resumes by id and replays the stored items, in order, byte for byte; the
    // resume continues the same session.
    let (second_endpoint, second_sink) = Emulator::start(Wire::Responses, vec![answered_turn()]);
    let mut client = responses_client(&second_endpoint);
    let mut resumed = SessionFile::resume(&sessions, &session_id(), client.wire(), &workspace)
        .expect("the session resumes on its own wire");
    let mut sink = StreamSink::new(second_sink);
    let filed = run_with(
        &mut client,
        None,
        &mut resumed,
        &sessions,
        "SECOND-QUESTION",
        &mut sink,
    );
    filed.run.expect("the resumed run answers");
    let second_path = filed.filed.expect("the resumed run files its session");
    let second_requests = second_endpoint.requests();
    assert_eq!(second_requests.len(), 1);
    let sent = input(&second_requests[0].body);
    // The standing instruction heads `input`; the replayed history follows; the new input ends it.
    assert_eq!(sent.len(), 1 + stored.items.len() + 1, "{sent:?}");
    let replayed = &sent[1..=stored.items.len()];
    let first_sent = input(&first_requests[0].body);
    assert_eq!(
        compact(&replayed[0]),
        compact(&first_sent[1]),
        "the user item"
    );
    assert_eq!(compact(&replayed[1]), REASONING_ITEM, "the opaque item");
    assert!(contains(
        &second_requests[0].body,
        REASONING_ITEM.as_bytes()
    ));
    assert_eq!(
        replayed[2],
        json!({"type": "message", "role": "assistant",
               "content": [{"type": "output_text", "text": "ANSWER-ONE"}]}),
        "the assistant item"
    );
    assert_eq!(
        sent[sent.len() - 1]["content"][0]["text"],
        "SECOND-QUESTION",
        "the new input follows the replayed history"
    );
    // The same session: same identity, same file, the first run's items carried on.
    assert_eq!(second_path, path, "filed to the same file");
    assert_eq!(
        session_files(&sessions),
        [format!("{SESSION}.json")],
        "no second session"
    );
    let continued = SessionFile::load(&sessions, &session_id()).expect("the session loads");
    assert_eq!(continued.id, SESSION);
    assert_eq!(continued.created_unix, stored.created_unix);
    assert_eq!(continued.items[..stored.items.len()], stored.items[..]);
    assert_eq!(continued.items.len(), stored.items.len() + 3);

    // 7. A run whose endpoint drops mid-stream still files its session, holding every turn
    // completed before the drop.
    let dropped_id = SessionId(Uuid("00000000-0000-4000-8000-00000000d0e9".to_owned()));
    let (drop_endpoint, _) = Emulator::start(Wire::Responses, vec![tool_turn(), dropped_turn()]);
    let mut dropping = SessionFile::open(
        &SessionData {
            session_id: dropped_id.clone(),
            commission_run: CommissionRunId(Uuid(RUN.to_owned())),
            wire: responses::WIRE.to_owned(),
            boundary_refusals: 0,
        },
        MODEL,
        drop_endpoint.base_url(),
        &workspace,
    )
    .expect("a session opens");
    let mut client = responses_client(&drop_endpoint);
    let mut tools = Clock::new();
    let mut sink = StreamSink::quiet();
    let filed = run_with(
        &mut client,
        Some(&mut tools),
        &mut dropping,
        &sessions,
        "DROP-QUESTION",
        &mut sink,
    );
    filed.run.expect_err("the dropped stream fails the run");
    let dropped_path = filed.filed.expect("the failed run still files its session");
    assert!(
        drop_endpoint.requests().len() >= 2,
        "the second turn was sent"
    );
    let after_drop = SessionFile::load(&sessions, &dropped_id).expect("the session loads");
    assert_eq!(
        dropped_path,
        sessions
            .canonicalize()
            .expect("resolves")
            .join(format!("{}.json", dropped_id.0.0))
    );
    assert_eq!(after_drop.run_ending(), Some(RunEnding::Failed));
    let call = ToolCall {
        call_id: b10x_loom_executor::harness::wire::CallId::new("call_1").expect("valid"),
        name: ToolName::new("clock").expect("valid"),
        arguments: json!({}),
    };
    assert_eq!(
        after_drop.items,
        [
            Item::user("DROP-QUESTION"),
            opaque_item(responses::WIRE, REASONING_ITEM),
            Item::ToolCall(call.clone()),
            Item::result(call.call_id, ToolOutcome::ok(json!({"now": "fixed"}))),
        ],
        "the completed turn and its tool result, and nothing of the dropped turn"
    );
    let dropped_bytes = std::fs::read(&dropped_path).expect("reads");
    assert!(!contains(&dropped_bytes, b"PARTIAL-ANSWER"));
}

/// Acceptance 1 on the Messages wire: `thinking_delta` and `text_delta` reach the sink in lockstep.
#[test]
fn session_transcript_streaming_messages_deltas() {
    let root = scratch("session_transcript_streaming_messages_deltas");
    let workspace = root.join("workspace");
    let sessions = root.join("state").join("sessions");
    fs_create(&workspace);
    let (endpoint, deltas) = Emulator::start(Wire::Messages, vec![thinking_turn()]);
    let mut session = SessionFile::open(
        &SessionData {
            session_id: session_id(),
            commission_run: CommissionRunId(Uuid(RUN.to_owned())),
            wire: messages::WIRE.to_owned(),
            boundary_refusals: 0,
        },
        MODEL,
        endpoint.base_url(),
        &workspace,
    )
    .expect("a session opens");
    let mut client = messages_client(&endpoint);
    let mut sink = StreamSink::new(deltas);
    let filed = run_with(
        &mut client,
        None,
        &mut session,
        &sessions,
        "QUESTION",
        &mut sink,
    );
    filed.run.expect("the run answers");
    let path = filed.filed.expect("the run files its session");
    endpoint.assert_lockstep();
    // The thinking text is stored only inside the provider's opaque thinking block.
    let stored = SessionFile::load(&sessions, &session_id()).expect("the session loads");
    let bytes = std::fs::read(&path).expect("reads");
    assert_reasoning_only_inside_opaque(&stored, &bytes, &THINKING_DELTAS);
    assert_eq!(
        sink.deltas,
        [THINKING_DELTAS[0], THINKING_DELTAS[1], "ANSWER-M"],
        "every delta reached the sink, in order"
    );
}

/// Coordinator decision 14 (wave 2026-10-04-w15): a temporary file another filer left or holds is
/// refused by name and never removed or overwritten.
#[test]
fn session_transcript_streaming_refuses_a_temporary_file_it_did_not_create() {
    let root = scratch("session_transcript_streaming_stale_tmp");
    let workspace = root.join("workspace");
    let sessions = root.join("state").join("sessions");
    fs_create(&workspace);
    fs_create(&sessions);
    let temporary = sessions.join(format!("{SESSION}.json.tmp"));
    std::fs::write(&temporary, b"ANOTHER-FILER").expect("plant");
    let mut session = SessionFile::open(
        &SessionData {
            session_id: session_id(),
            commission_run: CommissionRunId(Uuid(RUN.to_owned())),
            wire: responses::WIRE.to_owned(),
            boundary_refusals: 0,
        },
        MODEL,
        "http://127.0.0.1:9/v1",
        &workspace,
    )
    .expect("a session opens");
    let refused = session
        .file(&sessions, RunEnding::Answered)
        .expect_err("a temporary file it did not create is refused");
    assert!(refused.to_string().contains(".json.tmp"), "{refused}");
    assert_eq!(
        std::fs::read(&temporary).expect("still there"),
        b"ANOTHER-FILER"
    );
    assert!(!sessions.join(format!("{SESSION}.json")).exists());
}

/// Coordinator decision 9 (wave 2026-10-04-w15): a resumed session whose run is refused for
/// speaking another wire is filed back unchanged, so its claim is released and it resumes again.
#[test]
fn session_transcript_streaming_a_cross_wire_run_releases_its_claim() {
    let root = scratch("session_transcript_streaming_cross_wire_claim");
    let workspace = root.join("workspace");
    let sessions = root.join("state").join("sessions");
    fs_create(&workspace);
    let mut opened = SessionFile::open(
        &SessionData {
            session_id: session_id(),
            commission_run: CommissionRunId(Uuid(RUN.to_owned())),
            wire: responses::WIRE.to_owned(),
            boundary_refusals: 0,
        },
        MODEL,
        "http://127.0.0.1:9/v1",
        &workspace,
    )
    .expect("a session opens");
    opened.items.push(Item::user("FILED-TURN"));
    opened
        .file(&sessions, RunEnding::Answered)
        .expect("the first run files");
    let responses_wire =
        b10x_loom_executor::harness::wire::WireId::new(responses::WIRE).expect("valid");
    let mut claimed = SessionFile::resume(&sessions, &session_id(), &responses_wire, &workspace)
        .expect("resumes on its own wire");

    let (endpoint, _) = Emulator::start(Wire::Messages, vec![]);
    let mut other = messages_client(&endpoint);
    let mut sink = StreamSink::quiet();
    let filed = run_with(
        &mut other,
        None,
        &mut claimed,
        &sessions,
        "QUESTION",
        &mut sink,
    );
    filed.run.expect_err("refused before the loop");
    assert!(
        matches!(filed.filed, Err(SessionError::WireMismatch(_))),
        "{:?}",
        filed.filed
    );
    assert!(endpoint.requests().is_empty(), "nothing was sent");
    let stored = SessionFile::load(&sessions, &session_id()).expect("loads");
    assert_eq!(stored.run_ending(), Some(RunEnding::Answered), "unchanged");
    assert_eq!(stored.items, [Item::user("FILED-TURN")], "unchanged");
    SessionFile::resume(&sessions, &session_id(), &responses_wire, &workspace)
        .expect("the claim was released, so the session resumes again");
}

// --- the run ------------------------------------------------------------------------------------

fn run_with(
    client: &mut dyn ModelPort,
    tools: Option<&mut Clock>,
    session: &mut SessionFile,
    sessions: &Path,
    input: &str,
    sink: &mut StreamSink,
) -> b10x_loom_executor::session::FiledRun {
    let mut none = Clock::none();
    let tools: &mut dyn ToolPort = match tools {
        Some(tools) => tools,
        None => &mut none,
    };
    let mut approvals = ApproveAll;
    let config = LoopConfig::new(MODEL, INSTRUCTIONS).with_retry_backoff(Duration::from_millis(1));
    let ports = RunPorts {
        model: client,
        tools,
        approvals: &mut approvals,
        config,
    };
    run_and_file(ports, session, sessions, input, sink)
}

fn responses_client(endpoint: &Emulator) -> ResponsesClient {
    ResponsesClient::new(
        responses::Endpoint::new(endpoint.base_url(), MODEL, 200_000).expect("endpoint"),
        Arc::new(StaticBearer::new(CREDENTIAL)),
    )
    .expect("client")
}

fn messages_client(endpoint: &Emulator) -> MessagesClient {
    MessagesClient::new(
        messages::Endpoint::new(endpoint.base_url(), MODEL, 200_000).expect("endpoint"),
        Arc::new(StaticBearer::new(CREDENTIAL)),
    )
    .expect("client")
}

/// The loop's sink: every delta's text, in order, reported to the endpoint as it arrives.
struct StreamSink {
    report: Option<Sender<String>>,
    deltas: Vec<String>,
}

impl StreamSink {
    fn new(report: Sender<String>) -> Self {
        Self {
            report: Some(report),
            deltas: Vec::new(),
        }
    }

    fn quiet() -> Self {
        Self {
            report: None,
            deltas: Vec::new(),
        }
    }
}

impl LoopSink for StreamSink {
    fn emit(&mut self, event: LoopEvent) {
        let text = match event {
            LoopEvent::TextDelta { text } | LoopEvent::ReasoningDelta { text } => text,
            _ => return,
        };
        if let Some(report) = &self.report {
            let _ = report.send(text.clone());
        }
        self.deltas.push(text);
    }
}

/// A local test tool, `clock`: a pure read answering a fixed value. Not a frontier tool.
struct Clock {
    specs: Vec<ToolSpec>,
}

impl Clock {
    fn new() -> Self {
        Self {
            specs: vec![ToolSpec {
                name: ToolName::new("clock").expect("valid"),
                description: "The time, fixed.".to_owned(),
                input_schema: json!({"type": "object", "properties": {},
                                     "additionalProperties": false}),
                approval: Approval::NotRequired,
                envelope: b10x_loom_executor::harness::wire::Envelope::default(),
            }],
        }
    }

    fn none() -> Self {
        Self { specs: Vec::new() }
    }
}

impl ToolPort for Clock {
    fn specs(&self) -> &[ToolSpec] {
        &self.specs
    }

    fn call(&mut self, _call: &ToolCall) -> ToolOutcome {
        ToolOutcome::ok(json!({"now": "fixed"}))
    }
}

// --- the scripted turns -------------------------------------------------------------------------

/// One step of a scripted response stream.
enum Step {
    /// A frame whose delta the sink must report before the next frame is emitted.
    Delta(Value, &'static str),
    /// A frame nothing waits on.
    Frame(Value),
    /// The connection drops: no terminal event, no final chunk.
    Drop,
}

/// Responses: two reasoning-summary deltas, the opaque reasoning item, an answer.
fn answered_turn() -> Vec<Step> {
    let reasoning: Value = serde_json::from_str(REASONING_ITEM).expect("valid");
    let message = json!({"type": "message", "id": "msg_1", "role": "assistant", "status": "completed",
                         "content": [{"type": "output_text", "text": "ANSWER-ONE"}]});
    vec![
        Step::Frame(json!({"type": "response.created", "response": {"id": "resp_1"}})),
        Step::Delta(
            json!({"type": "response.reasoning_summary_text.delta", "delta": REASONING_DELTAS[0]}),
            REASONING_DELTAS[0],
        ),
        Step::Delta(
            json!({"type": "response.reasoning_summary_text.delta", "delta": REASONING_DELTAS[1]}),
            REASONING_DELTAS[1],
        ),
        Step::Frame(json!({"type": "response.output_item.done", "item": reasoning})),
        Step::Delta(
            json!({"type": "response.output_text.delta", "delta": "ANSWER-ONE"}),
            "ANSWER-ONE",
        ),
        Step::Frame(json!({"type": "response.output_item.done", "item": message})),
        Step::Frame(json!({"type": "response.completed", "response": {
            "id": "resp_1", "status": "completed", "model": MODEL,
            "output": [reasoning, message],
            "usage": {"input_tokens": 10, "output_tokens": 5}}})),
    ]
}

/// Responses: the opaque reasoning item and a call of the local `clock` tool.
fn tool_turn() -> Vec<Step> {
    let reasoning: Value = serde_json::from_str(REASONING_ITEM).expect("valid");
    let call = json!({"type": "function_call", "id": "fc_1", "call_id": "call_1", "name": "clock",
                      "arguments": "{}"});
    vec![
        Step::Frame(json!({"type": "response.output_item.done", "item": reasoning})),
        Step::Frame(json!({"type": "response.output_item.added", "item": call})),
        Step::Frame(json!({"type": "response.output_item.done", "item": call})),
        Step::Frame(json!({"type": "response.completed", "response": {
            "id": "resp_2", "status": "completed", "model": MODEL,
            "output": [reasoning, call],
            "usage": {"input_tokens": 10, "output_tokens": 5}}})),
    ]
}

/// Responses: part of an answer, then the connection drops.
fn dropped_turn() -> Vec<Step> {
    vec![
        Step::Frame(json!({"type": "response.output_text.delta", "delta": "PARTIAL-ANSWER"})),
        Step::Drop,
    ]
}

/// Messages: a thinking block streamed in two `thinking_delta`s, then a text answer.
fn thinking_turn() -> Vec<Step> {
    vec![
        Step::Frame(json!({"type": "message_start", "message": {
            "id": "msg_1", "type": "message", "role": "assistant", "model": MODEL, "content": [],
            "usage": {"input_tokens": 10, "output_tokens": 1}}})),
        Step::Frame(json!({"type": "content_block_start", "index": 0,
                           "content_block": {"type": "thinking", "thinking": "", "signature": ""}})),
        Step::Delta(
            json!({"type": "content_block_delta", "index": 0,
                   "delta": {"type": "thinking_delta", "thinking": THINKING_DELTAS[0]}}),
            THINKING_DELTAS[0],
        ),
        Step::Delta(
            json!({"type": "content_block_delta", "index": 0,
                   "delta": {"type": "thinking_delta", "thinking": THINKING_DELTAS[1]}}),
            THINKING_DELTAS[1],
        ),
        Step::Frame(json!({"type": "content_block_delta", "index": 0,
                           "delta": {"type": "signature_delta", "signature": "SIG-1"}})),
        Step::Frame(json!({"type": "content_block_stop", "index": 0})),
        Step::Frame(json!({"type": "content_block_start", "index": 1,
                           "content_block": {"type": "text", "text": ""}})),
        Step::Delta(
            json!({"type": "content_block_delta", "index": 1,
                   "delta": {"type": "text_delta", "text": "ANSWER-M"}}),
            "ANSWER-M",
        ),
        Step::Frame(json!({"type": "content_block_stop", "index": 1})),
        Step::Frame(
            json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"},
                           "usage": {"output_tokens": 5}}),
        ),
        Step::Frame(json!({"type": "message_stop"})),
    ]
}

// --- the provider-emulated endpoint -------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Wire {
    Responses,
    Messages,
}

/// One request the endpoint was sent.
#[derive(Clone)]
struct Recorded {
    authorization: Option<String>,
    body: Vec<u8>,
}

struct Emulator {
    base_url: String,
    requests: Arc<Mutex<Vec<Recorded>>>,
    violations: Arc<Mutex<Vec<String>>>,
}

impl Emulator {
    /// Serves `turns` in order, one per request; a request past the script gets a dropped stream.
    /// Answers the sender the loop's sink reports deltas on.
    fn start(wire: Wire, turns: Vec<Vec<Step>>) -> (Self, Sender<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base_url = format!("http://{}/v1", listener.local_addr().expect("address"));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let violations = Arc::new(Mutex::new(Vec::new()));
        let (report, reported) = channel();
        let (thread_requests, thread_violations) = (requests.clone(), violations.clone());
        std::thread::spawn(move || {
            let mut turns = turns.into_iter();
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                let script = turns.next().unwrap_or_else(dropped_turn);
                serve(
                    stream,
                    wire,
                    &script,
                    &reported,
                    &thread_requests,
                    &thread_violations,
                );
            }
        });
        (
            Self {
                base_url,
                requests,
                violations,
            },
            report,
        )
    }

    fn base_url(&self) -> String {
        self.base_url.clone()
    }

    fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().expect("lock").clone()
    }

    fn assert_lockstep(&self) {
        let violations = self.violations.lock().expect("lock");
        assert!(violations.is_empty(), "{violations:#?}");
    }
}

fn serve(
    mut stream: TcpStream,
    wire: Wire,
    script: &[Step],
    reported: &Receiver<String>,
    requests: &Mutex<Vec<Recorded>>,
    violations: &Mutex<Vec<String>>,
) {
    let Some(recorded) = read_request(&stream) else {
        return;
    };
    requests.lock().expect("lock").push(recorded);
    let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                transfer-encoding: chunked\r\nconnection: close\r\n\r\n";
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    let _ = stream.flush();
    for step in script {
        let (frame, awaited) = match step {
            Step::Frame(frame) => (frame, None),
            Step::Delta(frame, text) => (frame, Some(*text)),
            Step::Drop => {
                let _ = stream.shutdown(Shutdown::Both);
                return;
            }
        };
        if write_chunk(&mut stream, &encode(wire, frame)).is_err() {
            return;
        }
        if let Some(text) = awaited {
            match reported.recv_timeout(LOCKSTEP) {
                Ok(seen) if seen == text => {}
                Ok(seen) => violations.lock().expect("lock").push(format!(
                    "the sink reported `{seen}` where `{text}` was emitted"
                )),
                Err(_) => violations.lock().expect("lock").push(format!(
                    "`{text}` had not reached the sink when the next event was due"
                )),
            }
        }
    }
    if wire == Wire::Responses {
        let _ = write_chunk(&mut stream, b"data: [DONE]\n\n");
    }
    let _ = stream.write_all(b"0\r\n\r\n");
    let _ = stream.flush();
}

fn encode(wire: Wire, frame: &Value) -> Vec<u8> {
    let data = compact(frame);
    match wire {
        Wire::Responses => format!("data: {data}\n\n").into_bytes(),
        Wire::Messages => {
            let kind = frame["type"].as_str().unwrap_or("unknown");
            format!("event: {kind}\ndata: {data}\n\n").into_bytes()
        }
    }
}

fn write_chunk(stream: &mut TcpStream, bytes: &[u8]) -> std::io::Result<()> {
    stream.write_all(format!("{:x}\r\n", bytes.len()).as_bytes())?;
    stream.write_all(bytes)?;
    stream.write_all(b"\r\n")?;
    stream.flush()
}

fn read_request(stream: &TcpStream) -> Option<Recorded> {
    let mut reader = BufReader::new(stream);
    let mut length = 0usize;
    let mut authorization = None;
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
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => length = value.trim().parse().ok()?,
            "authorization" => authorization = Some(value.trim().to_owned()),
            _ => {}
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Recorded {
        authorization,
        body,
    })
}

// --- helpers ------------------------------------------------------------------------------------

fn session_id() -> SessionId {
    SessionId(Uuid(SESSION.to_owned()))
}

fn opaque_item(wire: &str, payload: &str) -> Item {
    Item::Opaque {
        wire: b10x_loom_executor::harness::wire::WireId::new(wire).expect("valid"),
        payload: serde_json::from_str(payload).expect("valid"),
    }
}

fn compact(value: &Value) -> String {
    serde_json::to_string(value).expect("encodes")
}

fn input(body: &[u8]) -> Vec<Value> {
    let body: Value = serde_json::from_slice(body).expect("a JSON request");
    body["input"].as_array().expect("an input array").clone()
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn session_files(dir: &Path) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .expect("the session directory reads")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    files.sort();
    files
}

#[cfg(unix)]
fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777
}

fn scratch(name: &str) -> PathBuf {
    let root =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("clear scratch");
    }
    root
}

fn fs_create(dir: &Path) {
    std::fs::create_dir_all(dir).expect("create");
}

/// Acceptance 4: each streamed reasoning text appears in the filed file exactly once, and only
/// inside an opaque provider item: the session with its opaque items removed holds none of it.
fn assert_reasoning_only_inside_opaque(stored: &SessionFile, bytes: &[u8], deltas: &[&str]) {
    let mut without_opaque = stored.clone();
    without_opaque
        .items
        .retain(|item| !matches!(item, Item::Opaque { .. }));
    let outside = serde_json::to_vec(&without_opaque).expect("encodes");
    let inside: Vec<u8> = stored
        .items
        .iter()
        .filter(|item| matches!(item, Item::Opaque { .. }))
        .flat_map(|item| serde_json::to_vec(item).expect("encodes"))
        .collect();
    for delta in deltas {
        let text = delta.trim().as_bytes();
        assert!(
            !contains(&outside, text),
            "`{delta}` filed outside an opaque item"
        );
        assert!(
            contains(&inside, text),
            "precondition: the opaque item carries `{delta}`"
        );
        assert_eq!(
            bytes
                .windows(text.len())
                .filter(|window| *window == text)
                .count(),
            1,
            "`{delta}` filed more than once"
        );
    }
}
