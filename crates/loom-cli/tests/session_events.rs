//! Acceptance for `story:run-record-names-agent-sessions`: a run whose agent work goes through
//! the session executor writes one `Session` record per agent session on the run event stream
//! (`intake.events.SessionEvent` in `ess/intake/domains/events.yaml`). It names the session's
//! `trace-ir/1` document by its digest and source and holds metaharness's `trace-ir-summary/1` of
//! it (`metaharness::protocol::summarize`); the session's own turns, tool calls and usage have no
//! `Turn`, `ToolCall` or `Usage` record, and Loom's own records are as without it.
//!
//! The session is the committed recorded Claude Code session `tests/fixtures/session/
//! claude-code.jsonl` (synthetic ids): a scripted program (metaharness's `ScriptedRunner`) writes
//! it as the vendor record metaharness keeps, replays its lines and makes the change its reply
//! names in the scratch copy. No harness program runs, no model is called and nothing reaches the
//! network.

use std::cell::Cell;
use std::io::{self, Write};
use std::iter::repeat_n;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use b10x_loom_cli::events::EventStream;
use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value as Json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, FrontierAction,
    ObservationId, PrincipalId, RunId, RunOutcome, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::runtime::{EXECUTOR_SOURCE, LoopContext, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_intake_slice::run::{ApprovalGate, SliceRun, StopReason};
use loom_sdk::session::metaharness::protocol::summarize;
use loom_sdk::session::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptedLog, ScriptedRunner, VendorSource,
    project_vendor_record,
};
use loom_sdk::session::{CredentialSource, ProcessRunner, SessionExecutor};
use serde_json::{Value, json};

const EDIT: &str = "implementation.edit";
const PROTOCOL: &str = "software-change@1";
const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";

/// The committed recorded session.
fn recorded() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/session/claude-code.jsonl")
}

/// A scripted Claude Code session: it fixes `add` in the copy it is started in, leaves `record`
/// as the vendor record metaharness keeps, and prints the recorded session's lines, its opening
/// record naming the copy it was started in as its `cwd`, as a program started there prints it
/// (the executor's hermetic floor holds a session to the directory metaharness planned).
struct Recorded {
    record: Vec<u8>,
    calls: Rc<Cell<usize>>,
}

impl ProcessRunner for Recorded {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> io::Result<Box<dyn HarnessProcess>> {
        self.calls.set(self.calls.get() + 1);
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

/// A writer the test reads back after the stream wrote to it.
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
    fn lines(&self) -> Vec<Value> {
        let bytes = self.0.lock().expect("the buffer").clone();
        String::from_utf8(bytes)
            .expect("the stream is UTF-8")
            .lines()
            .map(|line| serde_json::from_str(line).expect("each line is one JSON object"))
            .collect()
    }
}

/// An effect that performs the edit and changes nothing.
#[derive(Default)]
struct Effects {
    invoked: Cell<usize>,
}

impl EffectPort for Effects {
    fn performs(&self, action: &str) -> bool {
        action == EDIT
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.invoked.set(self.invoked.get() + 1);
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Json::Text("edited".to_owned()),
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// New ids from counters and one trusted time.
#[derive(Default)]
struct Context {
    requests: u64,
    observations: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x300 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x400 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-22T09:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// What one run left: the stream's lines, the digest the executor's step reported for each
/// session, and the edits performed.
struct Ran {
    lines: Vec<Value>,
    step_digests: Vec<Value>,
    performed: usize,
    sessions: usize,
}

/// One run of a commission whose agent is the session executor over the scripted session leaving
/// `record`, its records on a stream: the route, the run (through [`EventStream::sessions`] when
/// `named`, else the executor alone), then the terminal record.
fn drive(record: &[u8], named: bool) -> Ran {
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
    let calls = Rc::new(Cell::new(0));
    let mut executor = SessionExecutor::new(&workspace, vec!["src/lib.rs".to_owned()], "fix add")
        .with_runner(Box::new(Recorded {
            record: record.to_vec(),
            calls: calls.clone(),
        }));
    executor.credentials = CredentialSource::None;
    executor.scratch = scratch;

    let case = CaseId("case-session".to_owned());
    let admissible = FrontierAction {
        action: EDIT.to_owned(),
        status: ActionStatus::Admissible,
        capability: None,
        reasons: Vec::new(),
    };
    let open = Answer::at(1).with_items(Vec::new(), Vec::new(), vec![admissible]);
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        repeat_n(open.clone(), 5).chain([open.complete("D")]),
    );
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case,
        principal: PrincipalId("principal-s".to_owned()),
        authority_context: AuthorityContext(Json::Null),
    });

    let buffer = Buffer::default();
    let stream = EventStream::new(buffer.clone());
    stream.route(PROTOCOL, true).expect("the route record");
    let sessions = stream.sessions(&executor);
    let agent: &dyn AgentExecutor = if named { &sessions } else { &executor };
    let effects = Effects::default();
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x101))));
    let end = run_until_blocked(
        &governor,
        agent,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission,
        &mut runs,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"));
    assert!(
        matches!(end.outcome, RunOutcome::Completed(_)),
        "the run completes: {:?}",
        end.outcome
    );
    let run = SliceRun {
        protocol: PROTOCOL.to_owned(),
        steps: effects.invoked.get(),
        stop_reason: StopReason::Completed,
        approvals: Vec::new(),
        unperformed: Vec::new(),
        gate: ApprovalGate::Approvable,
    };
    stream.finish(Ok(&run)).expect("the terminal record");

    let step_digests = governor
        .observations()
        .into_iter()
        .filter(|observation| observation.source == EXECUTOR_SOURCE)
        .filter_map(|observation| match observation.payload {
            Json::Object(fields) => fields
                .into_iter()
                .find(|(name, _)| name == "transcript_digest")
                .map(|(_, value)| match value {
                    Json::Text(digest) => Value::String(digest),
                    other => panic!("a digest is text: {other:?}"),
                }),
            _ => None,
        })
        .collect();
    Ran {
        lines: buffer.lines(),
        step_digests,
        performed: effects.invoked.get(),
        sessions: calls.get(),
    }
}

/// `value` without its `null` members, at every depth: the stream writes no `null`.
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

/// A line's fields, without the stream's own `schema_version` and `kind`.
fn fields(line: &Value) -> Value {
    let mut fields = line.as_object().expect("a line is an object").clone();
    fields.remove("schema_version");
    fields.remove("kind");
    Value::Object(fields)
}

fn of_kind<'l>(lines: &'l [Value], kind: &str) -> Vec<&'l Value> {
    lines.iter().filter(|line| line["kind"] == kind).collect()
}

/// The recorded session's line holds exactly the summary metaharness makes of its record: every
/// `trace-ir-summary/1` field it states, under the same name, with the document's digest and
/// source; the session's turns, tool calls and usage have no records of their own.
#[test]
fn a_session_event_holds_the_summary_of_the_recorded_session() {
    let record = std::fs::read(recorded()).expect("the recorded session");
    let ran = drive(&record, true);
    for line in &ran.lines {
        println!("{line}");
    }
    assert_eq!(ran.sessions, 1, "one session ran");
    assert_eq!(ran.performed, 1, "its edit was performed");

    let sessions = of_kind(&ran.lines, "Session");
    assert_eq!(sessions.len(), 1, "one Session record: {:#?}", ran.lines);
    let document =
        project_vendor_record(&recorded(), VendorSource::ClaudeCodeStreamJson).expect("projects");
    let mut expected = serde_json::to_value(summarize(&document)).expect("the summary serializes");
    expected
        .as_object_mut()
        .expect("the summary is an object")
        .remove("format");
    assert_eq!(fields(sessions[0]), without_nulls(expected));

    assert_eq!(sessions[0]["source"], "claude-code-stream-json");
    assert_eq!(
        vec![sessions[0]["transcript_digest"].clone()],
        ran.step_digests,
        "the digest is the step's"
    );
    assert_eq!(sessions[0]["tokens_basis"], "session");
    assert_eq!(sessions[0]["models"][0]["name"], "a-model");
    assert_eq!(
        sessions[0]["tool_calls"],
        json!([{"call_id": "call-0001", "name": "Read", "answered": true, "error": false}])
    );
    assert!(sessions[0].get("projection_error").is_none());

    for kind in ["Turn", "ToolCall", "Usage"] {
        assert!(
            of_kind(&ran.lines, kind).is_empty(),
            "the session's own {kind}s have no record: {:#?}",
            ran.lines
        );
    }
}

/// The stream's other records are the ones the same run writes without the Session records: the
/// route first and the one terminal record last.
#[test]
fn loom_s_own_records_are_as_without_the_session_record() {
    let record = std::fs::read(recorded()).expect("the recorded session");
    let named = drive(&record, true);
    let alone = drive(&record, false);
    assert!(of_kind(&alone.lines, "Session").is_empty());
    let others: Vec<Value> = named
        .lines
        .iter()
        .filter(|line| line["kind"] != "Session")
        .cloned()
        .collect();
    assert_eq!(others, alone.lines);
    let kinds: Vec<&str> = named
        .lines
        .iter()
        .map(|line| line["kind"].as_str().expect("a kind"))
        .collect();
    assert_eq!(kinds, ["Route", "Session", "Terminal"]);
}

/// A record metaharness does not project still gets its Session record: its digest and source,
/// no summary field, and why; the run goes on as with any other session.
#[test]
fn a_record_that_does_not_project_is_named_with_why_and_fails_nothing() {
    let ran = drive(b"not a session record\n", true);
    assert_eq!(ran.performed, 1, "the run went on: {:#?}", ran.lines);
    let sessions = of_kind(&ran.lines, "Session");
    assert_eq!(sessions.len(), 1, "{:#?}", ran.lines);
    let mut names: Vec<&str> = sessions[0]
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "kind",
            "projection_error",
            "schema_version",
            "source",
            "transcript_digest"
        ]
    );
    assert_eq!(sessions[0]["source"], "claude-code-stream-json");
    assert_eq!(
        vec![sessions[0]["transcript_digest"].clone()],
        ran.step_digests
    );
    let why = sessions[0]["projection_error"].as_str().expect("a reason");
    assert!(why.contains("did not project"), "{why}");
    let terminal = of_kind(&ran.lines, "Terminal");
    assert_eq!(terminal.len(), 1);
    assert_eq!(terminal[0]["stop_reason"], "Completed");
}
