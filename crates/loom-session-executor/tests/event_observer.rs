//! Acceptance for `story:session-executor-event-observer`: a caller of the session executor is
//! handed each `metaharness.event/1` line of a session while it runs
//! (`SessionExecutor::with_observer`), with the session's scratch copy, in the order the report's
//! transcript holds them. The observer reads and changes nothing; a panic in it halts the session
//! and leaves the call.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, the lines a Codex rollout and
//! its hook requests print): no harness program runs, no model is called and nothing reaches the
//! network.

use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorStep, Frontier, FrontierAction, FrontierData,
    FrontierId, PrincipalId,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::protocol::{Event, EventLine};
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptStep, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::{
    CredentialSource, Kind, ProcessRunner, SessionExecutor, SessionReport,
};

const EDIT: &str = "implementation.edit";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A directory of the test's own under the system's temporary directory: `tree/`, the workspace
/// with `src/lib.rs`, and `scratch/` for the copies; removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-observer-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    /// A Codex executor over the workspace, its session the one `session` plays.
    fn executor(&self, session: Session) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(Kind::Codex)
                .with_runner(Box::new(session));
        executor.credentials = CredentialSource::None;
        executor.scratch = self.0.join("scratch");
        executor
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A scripted Codex session ([`rollout`]), leaving `kept` where metaharness keeps the vendor
/// record when it is given, and logging what the child was sent and whether it was killed.
struct Session {
    kept: Option<Vec<u8>>,
    log: ScriptedLog,
}

impl Session {
    fn plain() -> Self {
        Self {
            kept: None,
            log: ScriptedLog::new(),
        }
    }
}

impl ProcessRunner for Session {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        if let Some(bytes) = &self.kept {
            std::fs::write(plan.transcript, bytes)?;
        }
        ScriptedRunner::new(rollout(plan.cwd), self.log.clone()).start(plan)
    }
}

/// One Codex `PreToolUse` hook request, as metaharness's codex hook relays it to the seam,
/// waiting under the rendezvous name `key` for the decision.
fn hook(key: &str, tool: &str, input: &serde_json::Value, cwd: &Path) -> [ScriptStep; 2] {
    let raw = serde_json::json!({
        "session_id": "codex-1", "turn_id": "t1", "cwd": cwd.display().to_string(),
        "hook_event_name": "PreToolUse", "model": "scripted", "permission_mode": "default",
        "tool_name": tool, "tool_input": input, "tool_use_id": key,
    });
    let line = serde_json::json!({
        "metaharness.codex/1": "hook_request", "key": key, "input": raw,
    });
    [
        ScriptStep::line(line.to_string()),
        ScriptStep::awaiting(key),
    ]
}

/// A pinned Codex rollout: its opening record, one call the executor decides (a listing of the
/// copy), the turn's token count, and its completion, whose last message names the action.
fn rollout(cwd: &Path) -> Vec<ScriptStep> {
    let meta = serde_json::json!({
        "timestamp": "2026-08-22T10:00:00.000Z", "type": "session_meta",
        "payload": {"id": "codex-1", "session_id": "codex-1", "cli_version": "0.153.4",
                    "cwd": cwd.display().to_string(), "originator": "codex_exec",
                    "model_provider": "openai"},
    });
    let started = serde_json::json!({
        "timestamp": "2026-08-22T10:00:01.000Z", "type": "event_msg",
        "payload": {"type": "task_started", "turn_id": "t1"},
    });
    let tokens = serde_json::json!({
        "timestamp": "2026-08-22T10:00:04.000Z", "type": "event_msg",
        "payload": {"type": "token_count", "info": {"total_token_usage": {
            "input_tokens": 100, "cached_input_tokens": 40, "output_tokens": 20,
            "reasoning_output_tokens": 6, "total_tokens": 120}}},
    });
    let action = serde_json::json!({"action": EDIT, "summary": "looked"}).to_string();
    let completed = serde_json::json!({
        "timestamp": "2026-08-22T10:00:05.000Z", "type": "event_msg",
        "payload": {"type": "task_complete", "turn_id": "t1", "duration_ms": 4200,
                    "error": null, "last_agent_message": action},
    });
    let mut steps = vec![
        ScriptStep::line(meta.to_string()),
        ScriptStep::line(started.to_string()),
    ];
    steps.extend(hook(
        "call-ls",
        "Bash",
        &serde_json::json!({"command": "ls src"}),
        cwd,
    ));
    steps.push(ScriptStep::line(tokens.to_string()));
    steps.push(ScriptStep::line(completed.to_string()));
    steps
}

fn step_of(executor: &SessionExecutor) -> ExecutorStep {
    let case = CaseId("case-observer".to_owned());
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-observer".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    let frontier = Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf1)),
        case_id: case,
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![FrontierAction {
            action: EDIT.to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    });
    executor.run_with_usage(&commission, &frontier)
}

fn report_of(executor: &SessionExecutor) -> SessionReport {
    executor.last_report().expect("a report")
}

/// What an observer was handed at one call: the event, the path, whether that path was a
/// directory then, and whether the executor's call had returned by then.
struct Seen {
    line: EventLine,
    copy: PathBuf,
    copy_existed: bool,
    returned: bool,
}

/// An observer recording what it is handed into `seen`, reading `returned` at each call.
fn recording(
    seen: &Rc<RefCell<Vec<Seen>>>,
    returned: &Rc<RefCell<bool>>,
) -> impl FnMut(&EventLine, &Path) + 'static {
    let seen = Rc::clone(seen);
    let returned = Rc::clone(returned);
    move |line, copy| {
        seen.borrow_mut().push(Seen {
            line: line.clone(),
            copy: copy.to_path_buf(),
            copy_existed: copy.is_dir(),
            returned: *returned.borrow(),
        });
    }
}

/// The session's tool call and its usage reach the observer before `run_with_usage` returns,
/// while the session's copy is still there: the observer is called as the session runs.
#[test]
fn the_observer_sees_tool_calls_and_usage_while_the_session_runs() {
    let scratch = Scratch::new("live");
    let seen = Rc::new(RefCell::new(Vec::new()));
    let returned = Rc::new(RefCell::new(false));
    let executor = scratch
        .executor(Session::plain())
        .with_observer(recording(&seen, &returned));

    let step = step_of(&executor);
    *returned.borrow_mut() = true;

    assert!(
        matches!(step.outcome, ExecutorOutcome::ProposedAction(_)),
        "the session proposes as it does without an observer: {:?}",
        step.outcome
    );
    let seen = seen.borrow();
    let tool = seen
        .iter()
        .find(
            |seen| matches!(&seen.line.event, Event::ToolRequested { name, .. } if name == "Bash"),
        )
        .expect("the observer was handed the session's tool call");
    let usage = seen
        .iter()
        .find(|seen| matches!(seen.line.event, Event::Usage { .. }))
        .expect("the observer was handed the session's usage");
    for observed in [tool, usage] {
        assert!(!observed.returned, "handed before the call returned");
        assert!(
            observed.copy_existed,
            "handed while the session's copy existed: {}",
            observed.copy.display()
        );
    }
    assert!(
        seen.iter().all(|seen| !seen.returned && seen.copy_existed),
        "every event is handed while the session runs"
    );
}

/// With no vendor record kept, the report's transcript is the events the executor received, one
/// JSON line each: the observer was handed exactly those, in that order, each serialized as the
/// executor serializes it. With a vendor record kept, the report's transcript is that record,
/// unchanged by the observer, and the observer was handed the same events in the same order.
#[test]
fn the_observed_events_are_the_transcript_in_order() {
    let scratch = Scratch::new("order");
    let seen = Rc::new(RefCell::new(Vec::new()));
    let returned = Rc::new(RefCell::new(false));
    let executor = scratch
        .executor(Session::plain())
        .with_observer(recording(&seen, &returned));

    step_of(&executor);

    let report = report_of(&executor);
    let transcript = String::from_utf8(report.transcript.clone()).expect("UTF-8 event lines");
    let lines: Vec<&str> = transcript.lines().collect();
    let observed: Vec<String> = seen
        .borrow()
        .iter()
        .map(|seen| serde_json::to_string(&seen.line).expect("an event serializes"))
        .collect();
    assert!(!observed.is_empty(), "the observer was handed events");
    assert_eq!(
        observed, lines,
        "the observed events are the transcript's lines, in order"
    );

    let kept = b"{\"type\":\"session_meta\"}\n".to_vec();
    let kept_scratch = Scratch::new("kept");
    let kept_seen = Rc::new(RefCell::new(Vec::new()));
    let kept_executor = kept_scratch
        .executor(Session {
            kept: Some(kept.clone()),
            log: ScriptedLog::new(),
        })
        .with_observer(recording(&kept_seen, &returned));

    step_of(&kept_executor);

    let kept_report = report_of(&kept_executor);
    assert_eq!(
        kept_report.transcript, kept,
        "the report's transcript is the vendor record metaharness kept"
    );
    let order = |seen: &Rc<RefCell<Vec<Seen>>>| -> Vec<(u64, serde_json::Value)> {
        seen.borrow()
            .iter()
            .map(|seen| {
                let value = serde_json::to_value(&seen.line).expect("an event serializes");
                (seen.line.seq, value["event"].clone())
            })
            .collect()
    };
    assert_eq!(
        order(&kept_seen),
        order(&seen),
        "the observer is handed the same events whether or not a vendor record is kept"
    );
}

/// The path handed with every event is the session's copy, the report's `cwd`, and the copy is
/// gone once the call returns, as without an observer.
#[test]
fn the_observer_is_handed_the_sessions_copy() {
    let scratch = Scratch::new("copy");
    let seen = Rc::new(RefCell::new(Vec::new()));
    let returned = Rc::new(RefCell::new(false));
    let executor = scratch
        .executor(Session::plain())
        .with_observer(recording(&seen, &returned));

    step_of(&executor);

    let copy = report_of(&executor).cwd.expect("the session's copy");
    let seen = seen.borrow();
    assert!(!seen.is_empty(), "the observer was handed events");
    for observed in seen.iter() {
        assert_eq!(observed.copy, copy, "the path handed is the session's copy");
    }
    assert!(!copy.exists(), "the copy is removed when the call returns");
}

/// A panic in the observer is not swallowed: the session is halted, its harness child killed,
/// and the panic leaves the call with the observer's own payload.
#[test]
fn a_panicking_observer_halts_the_session_and_propagates() {
    let scratch = Scratch::new("panics");
    let log = ScriptedLog::new();
    let executor = scratch
        .executor(Session {
            kept: None,
            log: log.clone(),
        })
        .with_observer(|line: &EventLine, _: &Path| {
            if matches!(line.event, Event::ToolRequested { .. }) {
                panic!("the observer gave up");
            }
        });

    let left = catch_unwind(AssertUnwindSafe(|| step_of(&executor)));

    let payload = left.expect_err("the observer's panic leaves the call");
    let said = payload
        .downcast_ref::<&str>()
        .map(|said| (*said).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned());
    assert_eq!(said.as_deref(), Some("the observer gave up"));
    assert!(log.killed(), "the harness child is killed");
}
