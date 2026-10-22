//! Adversary cases for `story:run-record-names-agent-sessions`: the `Session` record a host writes
//! through `EventStream::sessions` (`b10x_loom_cli::events`) for each agent session the session
//! executor runs.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner` replaying the committed recorded
//! Claude Code session `tests/fixtures/session/claude-code.jsonl`, synthetic ids): no harness
//! program runs, no model is called and nothing reaches the network.

use std::cell::{Cell, RefCell};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use b10x_loom_cli::events::{EventStream, line, session_event};
use b10x_loom_commission::model::json::Value as Json;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use intake_model::events::{RunEvent, RunEventLine, SchemaVersion};
use loom_sdk::session::metaharness::protocol::{
    EVENT_FORMAT, SummarizedModel, SummarizedToolCall, TokensBasis, TraceIrSummary,
    parse_event_line, project_document, summarize,
};
use loom_sdk::session::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptedLog, ScriptedRunner, VendorSource,
    project_vendor_record,
};
use loom_sdk::session::{CredentialSource, ProcessRunner, SessionExecutor, SessionTrace};
use serde_json::{Value, json};

const EDIT: &str = "implementation.edit";
const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";

fn recorded() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/session/claude-code.jsonl")
}

/// The recorded session's lines, each changed by `edit`, as the bytes of a vendor record.
fn record_with(edit: impl Fn(&mut Value)) -> Vec<u8> {
    let text = std::fs::read_to_string(recorded()).expect("the recorded session");
    let mut bytes = Vec::new();
    for line in text.lines() {
        let mut record: Value = serde_json::from_str(line).expect("a record line");
        edit(&mut record);
        bytes.extend_from_slice(record.to_string().as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

/// A scripted Claude Code session, as `session_events.rs` drives one: it fixes `add` in the copy,
/// leaves `record` as the vendor record metaharness keeps, and prints the recorded session's lines
/// with the copy as its `cwd`. Each call it is started for is counted, and the path metaharness
/// planned for the record is kept; a start past `starts` fails as a program that cannot start.
struct Recorded {
    record: Vec<u8>,
    starts: usize,
    calls: Rc<Cell<usize>>,
    transcripts: Rc<RefCell<Vec<PathBuf>>>,
}

impl ProcessRunner for Recorded {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> io::Result<Box<dyn HarnessProcess>> {
        self.calls.set(self.calls.get() + 1);
        if self.calls.get() > self.starts {
            return Err(io::Error::other("the program cannot be started"));
        }
        self.transcripts
            .borrow_mut()
            .push(plan.transcript.to_path_buf());
        std::fs::write(plan.cwd.join("src/lib.rs"), FIXED)?;
        std::fs::write(plan.transcript, &self.record)?;
        let printed = std::fs::read_to_string(recorded())?
            .lines()
            .map(|line| {
                let mut record: Value = serde_json::from_str(line).map_err(io::Error::other)?;
                if record.get("cwd").is_some() {
                    record["cwd"] = Value::String(plan.cwd.display().to_string());
                }
                Ok(record.to_string())
            })
            .collect::<io::Result<Vec<_>>>()?;
        ScriptedRunner::of_lines(printed, ScriptedLog::new()).start(plan)
    }
}

#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("the buffer").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Buffer {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().expect("the buffer").clone()).expect("UTF-8")
    }

    fn lines(&self) -> Vec<Value> {
        self.text()
            .lines()
            .map(|line| serde_json::from_str(line).expect("each line is one JSON object"))
            .collect()
    }
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-s".to_owned()),
        authority_context: AuthorityContext(Json::Null),
    })
}

fn frontier(case: &CaseId, id: u64) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(id)),
        case_id: case.clone(),
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![FrontierAction {
            action: EDIT.to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    })
}

/// A workspace holding the broken `add`, and a scratch directory, under one temporary root.
struct Fixture {
    root: tempfile::TempDir,
    workspace: PathBuf,
    scratch: PathBuf,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().expect("a temporary directory");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(workspace.join("src")).expect("the workspace");
    std::fs::write(
        workspace.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n",
    )
    .expect("the file");
    let scratch = root.path().join("scratch");
    std::fs::create_dir_all(&scratch).expect("the scratch directory");
    Fixture {
        root,
        workspace,
        scratch,
    }
}

/// The session executor over `fixture`, its sessions scripted to leave `record`; the first
/// `starts` calls start a session.
fn executor(
    fixture: &Fixture,
    brief: &str,
    record: Vec<u8>,
    starts: usize,
) -> (SessionExecutor, Rc<Cell<usize>>, Rc<RefCell<Vec<PathBuf>>>) {
    let calls = Rc::new(Cell::new(0));
    let transcripts = Rc::new(RefCell::new(Vec::new()));
    let mut executor =
        SessionExecutor::new(&fixture.workspace, vec!["src/lib.rs".to_owned()], brief).with_runner(
            Box::new(Recorded {
                record,
                starts,
                calls: calls.clone(),
                transcripts: transcripts.clone(),
            }),
        );
    executor.credentials = CredentialSource::None;
    executor.scratch = fixture.scratch.clone();
    (executor, calls, transcripts)
}

/// One call of the executor through `EventStream::sessions`, on a fresh stream; the stream's lines
/// and the step's outcome.
fn one_call(executor: &SessionExecutor, frontier_id: u64) -> (Buffer, ExecutorOutcome) {
    let buffer = Buffer::default();
    let stream = EventStream::new(buffer.clone());
    let case = CaseId("case-adversary".to_owned());
    let step = stream
        .sessions(executor)
        .run_with_usage(&commission(&case), &frontier(&case, frontier_id));
    (buffer, step.outcome)
}

/// `value` without its `null` members, at every depth.
fn without_nulls(value: Value) -> Value {
    match value {
        Value::Object(members) => Value::Object(
            members
                .into_iter()
                .filter(|(_, member)| !member.is_null())
                .map(|(name, member)| (name, without_nulls(member)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(without_nulls).collect()),
        other => other,
    }
}

/// A line's fields, without `schema_version` and `kind`.
fn fields(line: &Value) -> Value {
    let mut fields = line.as_object().expect("a line is an object").clone();
    fields.remove("schema_version");
    fields.remove("kind");
    Value::Object(fields)
}

/// The summary as `summarize` states it, without its `format` and its `null` members: what the
/// story says the line holds.
fn as_summarized(summary: &TraceIrSummary) -> Value {
    let mut expected = serde_json::to_value(summary).expect("the summary serializes");
    expected
        .as_object_mut()
        .expect("an object")
        .remove("format");
    without_nulls(expected)
}

/// The `Session` line `EventStream` writes for `trace`.
fn session_line(trace: &SessionTrace) -> Value {
    line(&RunEventLine {
        schema_version: SchemaVersion::V1,
        event: RunEvent::Session(session_event(trace)),
    })
}

/// A vendor record whose terminal record states token counts past the largest integer the line
/// type holds (`u64::MAX`, which the vendor's JSON spells and metaharness reads as stated). The
/// story says every field after `source` is the summary's field of the same name; the line must
/// hold the summary's figure or refuse the summary (`projection_error`), as it does a cost with no
/// JSON spelling. It must never state a count the record does not.
#[test]
fn adversary_a_token_count_past_the_largest_integer_is_never_written_as_another_count() {
    let record = record_with(|record| {
        if record["type"] == "result" {
            record["usage"]["input_tokens"] = json!(u64::MAX);
            record["modelUsage"]["a-model"]["inputTokens"] = json!(u64::MAX);
        }
    });
    let fixture = fixture();
    let kept = fixture.root.path().join("kept.jsonl");
    std::fs::write(&kept, &record).expect("the record kept for the expectation");
    let document =
        project_vendor_record(&kept, VendorSource::ClaudeCodeStreamJson).expect("projects");
    let summary = summarize(&document);
    assert_eq!(
        summary.input_tokens,
        Some(u64::MAX),
        "precondition: the summary states the record's figure"
    );

    let (executor, calls, _) = executor(&fixture, "fix add", record, 1);
    let (buffer, _) = one_call(&executor, 0xf1);
    assert_eq!(calls.get(), 1, "one session ran");
    let lines = buffer.lines();
    let sessions: Vec<&Value> = lines
        .iter()
        .filter(|line| line["kind"] == "Session")
        .collect();
    assert_eq!(sessions.len(), 1, "{lines:#?}");
    let session = sessions[0];
    if session.get("projection_error").is_none() {
        assert_eq!(
            fields(session),
            as_summarized(&summary),
            "the line states a token count the summary does not: {session}"
        );
    } else {
        assert!(
            session.get("input_tokens").is_none(),
            "a refused summary states no count: {session}"
        );
    }
}

/// A record metaharness does not project is named with why. The why is a run record's field, read
/// by whoever supervises the run and kept beside it; metaharness itself never puts a machine's
/// directories into what it projects ("never a canonical absolute path, which would put the
/// machine's directories into a document people commit"). The scratch record's path, removed with
/// the session when the call returns, names the operator's directories and nothing a reader can
/// open: it must not be in the line.
#[test]
fn adversary_a_projection_error_names_no_directory_of_the_machine() {
    let fixture = fixture();
    let (executor, calls, transcripts) =
        executor(&fixture, "fix add", b"not a session record\n".to_vec(), 1);
    let (buffer, _) = one_call(&executor, 0xf2);
    assert_eq!(calls.get(), 1, "one session ran");
    let lines = buffer.lines();
    let session = lines
        .iter()
        .find(|line| line["kind"] == "Session")
        .unwrap_or_else(|| panic!("a Session line: {lines:#?}"));
    let why = session["projection_error"]
        .as_str()
        .unwrap_or_else(|| panic!("a projection error: {session}"));
    let transcript = transcripts.borrow()[0].clone();
    let directories = [
        transcript.display().to_string(),
        transcript
            .parent()
            .expect("the record's directory")
            .display()
            .to_string(),
        fixture.root.path().display().to_string(),
        std::env::temp_dir().display().to_string(),
    ];
    for directory in directories {
        assert!(
            !why.contains(&directory),
            "the projection error names the machine's directory {directory:?}: {why}"
        );
    }
}

/// Every field a summary states is written as `summarize` states it, whatever its shape: costs at
/// the edges of a float's spelling (tiny, huge, subnormal, negative zero, negative, the largest),
/// models and tool calls in the summary's order, a tool call with no id, name or error, the
/// `sum-of-requests` basis, and timestamps at the integer's ends.
#[test]
fn adversary_every_summary_shape_is_written_as_summarize_states_it() {
    let costs = [
        0.0123,
        1e-7,
        0.1 + 0.2,
        1e21,
        5e-324,
        -0.0,
        -2.5,
        f64::MAX,
        f64::MIN_POSITIVE,
        123_456_789.123_456_78,
    ];
    for (at, cost) in costs.into_iter().enumerate() {
        let summary = TraceIrSummary {
            format: "trace-ir-summary/1",
            source: "claude-code-stream-json",
            transcript_digest: "ab".repeat(32),
            adapter: Some("claude".to_owned()),
            model: Some("z-model".to_owned()),
            models: vec![
                SummarizedModel {
                    name: "z-model".to_owned(),
                    input_tokens: Some(7),
                    output_tokens: None,
                    cache_read_input_tokens: Some(0),
                    cache_creation_input_tokens: None,
                    cost_usd: Some(cost),
                },
                SummarizedModel {
                    name: "a-model".to_owned(),
                    input_tokens: None,
                    output_tokens: None,
                    cache_read_input_tokens: None,
                    cache_creation_input_tokens: None,
                    cost_usd: None,
                },
            ],
            turns: Some(3),
            tokens_basis: Some(if at % 2 == 0 {
                TokensBasis::SumOfRequests
            } else {
                TokensBasis::Session
            }),
            input_tokens: Some(i64::MAX.unsigned_abs()),
            output_tokens: Some(0),
            cache_read_input_tokens: None,
            cache_creation_input_tokens: Some(1),
            cost_usd: Some(cost),
            duration_ms: Some(0),
            duration_api_ms: None,
            first_timestamp_ms: Some(i64::MIN),
            last_timestamp_ms: Some(i64::MAX),
            is_error: Some(true),
            stop_reason: Some("end_turn".to_owned()),
            tool_calls: vec![
                SummarizedToolCall {
                    call_id: Some("call-2".to_owned()),
                    name: Some("Edit".to_owned()),
                    answered: true,
                    error: Some(true),
                },
                SummarizedToolCall {
                    call_id: None,
                    name: None,
                    answered: false,
                    error: None,
                },
                SummarizedToolCall {
                    call_id: Some("call-1".to_owned()),
                    name: Some("Read".to_owned()),
                    answered: true,
                    error: Some(false),
                },
            ],
            unk: 4,
        };
        let trace = SessionTrace {
            transcript_digest: Some(summary.transcript_digest.clone()),
            source: Some(summary.source.to_owned()),
            summary: Ok(summary.clone()),
        };
        let written = session_line(&trace);
        assert_eq!(written["kind"], "Session");
        assert_eq!(
            fields(&written),
            as_summarized(&summary),
            "cost {cost:e}: the line differs from the summary"
        );
        let text = written.to_string();
        let reread: Value = serde_json::from_str(&text).expect("the line reads back");
        assert_eq!(
            reread, written,
            "cost {cost:e}: the line does not read back as written"
        );
    }
}

/// One Session line per session that ran, and none for a call refused before its session started:
/// neither for a program that could not be started nor for files not named plainly, even right
/// after a call whose session ran (the executor's last report is that call's, never the one before).
#[test]
fn adversary_a_call_refused_before_its_session_started_writes_no_session_line() {
    let fixture = fixture();
    let record = std::fs::read(recorded()).expect("the recorded session");
    let (mut executor, calls, _) = executor(&fixture, "fix add", record, 1);

    let (first, outcome) = one_call(&executor, 0xf3);
    assert!(
        matches!(outcome, ExecutorOutcome::ProposedAction(_)),
        "the first session proposed: {outcome:?}"
    );
    let sessions = |buffer: &Buffer| {
        buffer
            .lines()
            .into_iter()
            .filter(|line| line["kind"] == "Session")
            .count()
    };
    assert_eq!(sessions(&first), 1, "{}", first.text());

    let (second, outcome) = one_call(&executor, 0xf4);
    assert_eq!(calls.get(), 2, "the second call asked the runner to start");
    assert!(
        matches!(outcome, ExecutorOutcome::Suspended(_)),
        "the second call proposed nothing: {outcome:?}"
    );
    assert_eq!(
        second.text(),
        "",
        "a session that never started has no line"
    );

    executor.files = vec!["./src/lib.rs".to_owned()];
    let (third, outcome) = one_call(&executor, 0xf5);
    assert!(
        matches!(outcome, ExecutorOutcome::Suspended(_)),
        "{outcome:?}"
    );
    assert_eq!(
        calls.get(),
        2,
        "no runner start for files not named plainly"
    );
    assert_eq!(
        third.text(),
        "",
        "a call refused before the copy has no line"
    );
}

/// The Session line holds the summary and nothing else the record says: not the brief, not what
/// the session's model wrote, not a tool's input or its result, not the vendor's session id. Its
/// digest is the digest metaharness takes of the record's bytes.
#[test]
fn adversary_the_session_line_holds_nothing_the_record_says_beyond_its_summary() {
    let record = record_with(|record| match record["type"].as_str() {
        Some("assistant") => {
            if let Some(content) = record["message"]["content"].as_array_mut() {
                for part in content {
                    if part["type"] == "text" && part["text"] == "reading the function" {
                        part["text"] = json!("MARKER-model-text token=sk-MARKER-0001");
                    }
                    if part["type"] == "tool_use" {
                        part["input"]["file_path"] = json!("MARKER-tool-input");
                    }
                }
            }
        }
        Some("user") => {
            if let Some(content) = record["message"]["content"].as_array_mut() {
                for part in content {
                    part["content"] = json!("MARKER-tool-result password=MARKER-0002");
                }
            }
        }
        _ => {}
    });
    let fixture = fixture();
    let kept = fixture.root.path().join("kept.jsonl");
    std::fs::write(&kept, &record).expect("the record kept for the expectation");
    let document =
        project_vendor_record(&kept, VendorSource::ClaudeCodeStreamJson).expect("projects");
    let summary = summarize(&document);

    let (executor, calls, _) = executor(&fixture, "MARKER-brief fix add", record, 1);
    let (buffer, outcome) = one_call(&executor, 0xf6);
    assert_eq!(calls.get(), 1);
    assert!(
        matches!(outcome, ExecutorOutcome::ProposedAction(_)),
        "{outcome:?}"
    );
    let lines = buffer.lines();
    assert_eq!(lines.len(), 1, "one line: {lines:#?}");
    let session = &lines[0];
    assert_eq!(session["kind"], "Session");
    assert_eq!(fields(session), as_summarized(&summary));
    assert_eq!(
        session["transcript_digest"].as_str(),
        Some(document.transcript_digest.as_str()),
        "the digest is metaharness's of the bytes summarized"
    );
    let text = buffer.text();
    for said in ["MARKER", "session-0001", "record-000", "message-000"] {
        assert!(
            !text.contains(said),
            "the Session line carries {said:?} from the record: {text}"
        );
    }
}

/// Without a vendor record (the program left its record empty), the line summarizes the
/// `metaharness.event/1` events the executor received, under that source, and its digest is the
/// step's and metaharness's of exactly the bytes summarized.
#[test]
fn adversary_without_a_vendor_record_the_line_summarizes_the_events_the_step_digests() {
    let fixture = fixture();
    let (executor, calls, _) = executor(&fixture, "fix add", Vec::new(), 1);
    let buffer = Buffer::default();
    let stream = EventStream::new(buffer.clone());
    let case = CaseId("case-adversary".to_owned());
    let step = stream
        .sessions(&executor)
        .run_with_usage(&commission(&case), &frontier(&case, 0xf7));
    assert_eq!(calls.get(), 1);
    let bytes = executor
        .last_report()
        .expect("the call's report")
        .transcript;
    assert!(!bytes.is_empty(), "the session delivered events");
    let lines: Vec<_> = std::str::from_utf8(&bytes)
        .expect("UTF-8 events")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| parse_event_line(line).expect("an event line"))
        .collect();
    let document = project_document(&lines, &bytes);
    let summary = summarize(&document);

    let written = buffer.lines();
    assert_eq!(written.len(), 1, "{written:#?}");
    let session = &written[0];
    assert_eq!(session["source"], EVENT_FORMAT, "{session}");
    assert_eq!(fields(session), as_summarized(&summary));
    assert_eq!(
        session["transcript_digest"].as_str(),
        step.transcript_digest.as_deref(),
        "the line's digest is the step's"
    );
    assert_eq!(
        session["transcript_digest"].as_str(),
        Some(document.transcript_digest.as_str()),
        "the line's digest is metaharness's of the bytes summarized"
    );
}
