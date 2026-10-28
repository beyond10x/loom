//! Adversary a3 of wave 2026-10-28-w1, against `story:session-executor-event-observer`
//! (`SessionExecutor::with_observer`).
//!
//! Every session is scripted (metaharness's `ScriptedRunner`, the lines a Codex rollout and its
//! hook requests print): no harness program runs, no model is called and nothing reaches the
//! network. Scratch lives under Cargo's per-test temporary directory.

use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorStep, Frontier, FrontierAction, FrontierData,
    FrontierId, PrincipalId, SuspensionReason,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::protocol::{Event, EventLine};
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptStep, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::{
    CredentialSource, Kind, ProcessRunner, Proposal, SessionExecutor,
};

const EDIT: &str = "implementation.edit";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("adv-w20261028w1-a3-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    fn executor(&self, runner: Box<dyn ProcessRunner>) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(Kind::Codex)
                .with_runner(runner);
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

/// A scripted Codex session; every child it starts gets a log of its own, kept in `logs`.
#[derive(Clone, Default)]
struct Sessions {
    logs: Rc<RefCell<Vec<ScriptedLog>>>,
}

impl ProcessRunner for Sessions {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        let log = ScriptedLog::new();
        self.logs.borrow_mut().push(log.clone());
        ScriptedRunner::new(rollout(plan.cwd), log).start(plan)
    }
}

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

fn proposed_paths(step: &ExecutorStep) -> Vec<String> {
    Proposal::from_outcome(&step.outcome)
        .expect("the session proposes")
        .expect("a proposal's arguments")
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect()
}

fn payload_text(payload: &(dyn std::any::Any + Send)) -> Option<String> {
    payload
        .downcast_ref::<&str>()
        .map(|said| (*said).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
}

/// `with_observer`'s doc, as decided: the observer is handed the session's scratch copy to read
/// and must not write there; a file it writes there anyway is part of what the session proposes,
/// which Commission revalidates like any change.
#[test]
fn adversary_a_file_the_observer_writes_into_the_copy_is_part_of_the_proposal() {
    let plain_scratch = Scratch::new("plain");
    let plain = plain_scratch.executor(Box::new(Sessions::default()));
    let without = proposed_paths(&step_of(&plain));

    let scratch = Scratch::new("writes");
    let wrote = Rc::new(Cell::new(false));
    let flag = Rc::clone(&wrote);
    let executor = scratch
        .executor(Box::new(Sessions::default()))
        .with_observer(move |_: &EventLine, copy: &Path| {
            if !flag.get() {
                std::fs::write(copy.join("src/planted.rs"), "pub fn planted() {}\n")
                    .expect("the observer writes into the copy");
                flag.set(true);
            }
        });
    let with = proposed_paths(&step_of(&executor));

    assert!(wrote.get(), "the observer was handed the copy");
    let mut expected = without.clone();
    expected.push("src/planted.rs".to_owned());
    expected.sort();
    let mut with = with;
    with.sort();
    assert_eq!(
        with, expected,
        "story:session-executor-event-observer: a file the observer writes into the copy is \
         part of what the session proposes, beside what it proposes without one ({without:?})"
    );
}

/// `with_observer`'s doc: the panicking call "records no session and leaves `last_report` as it
/// was", and the session is halted (the halt reaches the child before it is killed).
#[test]
fn adversary_a_panicking_observer_records_no_session_and_keeps_the_last_report() {
    let scratch = Scratch::new("panic-record");
    let sessions = Sessions::default();
    let panics = Rc::new(Cell::new(false));
    let armed = Rc::clone(&panics);
    let executor = scratch.executor(Box::new(sessions.clone())).with_observer(
        move |line: &EventLine, _: &Path| {
            if armed.get() && matches!(line.event, Event::ToolRequested { .. }) {
                panic!("adversary observer gave up");
            }
        },
    );

    step_of(&executor);
    let report = executor.last_report().expect("the first call's report");
    let recorded = executor.sessions();
    let proposed = executor.proposed();
    assert_eq!(recorded.len(), 1, "the first call records its session");

    panics.set(true);
    let left = catch_unwind(AssertUnwindSafe(|| step_of(&executor)));
    let payload = left.expect_err("the observer's panic leaves the call");
    assert_eq!(
        payload_text(&*payload).as_deref(),
        Some("adversary observer gave up")
    );

    assert_eq!(
        executor.last_report(),
        Some(report),
        "the last report is the first call's"
    );
    assert_eq!(executor.sessions(), recorded, "no session is recorded");
    assert_eq!(executor.proposed(), proposed, "no proposal is recorded");
    assert!(
        executor.failed().is_empty(),
        "no failed session is recorded"
    );

    let logs = sessions.logs.borrow();
    assert_eq!(logs.len(), 2, "one child per call");
    let second = &logs[1];
    assert!(second.killed(), "the panicking call's child is killed");
    assert!(
        second
            .written()
            .iter()
            .any(|line| line.contains("\"halt\":true")),
        "the halt was written to the child: {:?}",
        second.written()
    );
    assert!(
        second
            .written()
            .iter()
            .filter(|line| line.contains("call-ls"))
            .all(|line| line.contains("\"permissionDecision\":\"deny\"")),
        "the call the observer panicked on is never allowed: {:?}",
        second.written()
    );
}

/// `with_observer`'s doc: each event is handed "before the executor acts on it". The tool call
/// reaches the observer before its decision reaches the child.
#[test]
fn adversary_the_observer_sees_a_tool_call_before_the_executor_decides_it() {
    let scratch = Scratch::new("before-decision");
    let sessions = Sessions::default();
    let logs = Rc::clone(&sessions.logs);
    let all_logs = Rc::clone(&sessions.logs);
    let at_request: Rc<RefCell<Option<Vec<String>>>> = Rc::new(RefCell::new(None));
    let snapshot = Rc::clone(&at_request);
    let executor = scratch.executor(Box::new(sessions)).with_observer(
        move |line: &EventLine, _: &Path| {
            if matches!(&line.event, Event::ToolRequested { call_id, .. } if call_id == "call-ls") {
                let written = logs.borrow().last().expect("a child").written();
                *snapshot.borrow_mut() = Some(written);
            }
        },
    );

    step_of(&executor);

    let at_request = at_request
        .borrow()
        .clone()
        .expect("the tool call was observed");
    assert!(
        !at_request.iter().any(|line| line.contains("call-ls")),
        "no decision for the call was written when the observer saw it: {at_request:?}"
    );
    let after = all_logs.borrow().last().expect("a child").written();
    assert!(
        after.iter().any(|line| line.contains("call-ls")),
        "the decision for the call was written by the end: {after:?}"
    );
}

/// Reading the executor from inside the observer while its session runs borrows nothing the
/// session holds.
#[test]
fn adversary_an_observer_reads_the_executor_while_the_session_runs() {
    let scratch = Scratch::new("reads");
    let slot: Rc<RefCell<Weak<SessionExecutor>>> = Rc::new(RefCell::new(Weak::new()));
    let reads = Rc::new(Cell::new(0_u32));
    let (inner, counted) = (Rc::clone(&slot), Rc::clone(&reads));
    let executor = Rc::new(
        scratch
            .executor(Box::new(Sessions::default()))
            .with_observer(move |_: &EventLine, _: &Path| {
                if let Some(executor) = inner.borrow().upgrade() {
                    let _ = executor.last_report();
                    let _ = executor.sessions();
                    let _ = executor.proposed();
                    let _ = executor.failed();
                    counted.set(counted.get() + 1);
                }
            }),
    );
    *slot.borrow_mut() = Rc::downgrade(&executor);

    let step = catch_unwind(AssertUnwindSafe(|| step_of(&executor)));

    assert!(
        step.is_ok(),
        "reading the executor from the observer does not panic"
    );
    assert!(reads.get() > 0, "the observer read the executor");
    assert_eq!(executor.sessions().len(), 1, "the session is recorded");
}

/// `with_observer`'s doc, as decided: a call made while this executor already runs a session (an
/// observer that runs the executor again) is refused before any copy is made or child started,
/// suspended with `ExternalAvailability` naming why; the outer session goes on and is recorded,
/// and the executor serves the next call.
#[test]
fn adversary_a_session_started_from_inside_the_observer_is_refused_and_the_outer_goes_on() {
    let scratch = Scratch::new("reentrant");
    let copies = scratch.0.join("scratch");
    let sessions = Sessions::default();
    let logs = Rc::clone(&sessions.logs);
    let slot: Rc<RefCell<Weak<SessionExecutor>>> = Rc::new(RefCell::new(Weak::new()));
    let nested: Rc<RefCell<Option<(ExecutorStep, usize, usize)>>> = Rc::new(RefCell::new(None));
    let (inner, seen) = (Rc::clone(&slot), Rc::clone(&nested));
    let count = move || std::fs::read_dir(&copies).map_or(0, Iterator::count);
    let executor = Rc::new(scratch.executor(Box::new(sessions)).with_observer(
        move |line: &EventLine, _: &Path| {
            if seen.borrow().is_none() && matches!(line.event, Event::ToolRequested { .. }) {
                let before = count();
                let step = inner.borrow().upgrade().map(|executor| step_of(&executor));
                let step = step.expect("the executor is alive");
                *seen.borrow_mut() = Some((step, before, count()));
            }
        },
    ));
    *slot.borrow_mut() = Rc::downgrade(&executor);

    let left = catch_unwind(AssertUnwindSafe(|| step_of(&executor)));
    let said = left
        .as_ref()
        .err()
        .and_then(|payload| payload_text(&**payload));
    let outer = left.unwrap_or_else(|_| {
        panic!(
            "story:session-executor-event-observer: the outer call goes on, it left with {said:?}"
        )
    });

    let (step, before, after) = nested
        .borrow_mut()
        .take()
        .expect("the observer ran the executor");
    let why = match &step.outcome {
        ExecutorOutcome::Suspended(suspended) => match &suspended.reason {
            SuspensionReason::ExternalAvailability(Value::Text(why)) => why.clone(),
            other => panic!("story:session-executor-event-observer: suspended for {other:?}"),
        },
        other => panic!("story:session-executor-event-observer: the nested call ran: {other:?}"),
    };
    assert!(
        why.contains("already running a session"),
        "story:session-executor-event-observer: the nested call names why it is refused: {why}"
    );
    assert_eq!(
        (before, logs.borrow().len()),
        (after, 1),
        "story:session-executor-event-observer: the nested call makes no copy and starts no child"
    );
    assert_eq!(
        proposed_paths(&outer),
        Vec::<String>::new(),
        "story:session-executor-event-observer: the outer session proposes what it would"
    );
    assert_eq!(
        executor.sessions().len(),
        1,
        "story:session-executor-event-observer: the outer session is recorded"
    );
    assert_eq!(
        executor.last_report().and_then(|report| report.error),
        None,
        "story:session-executor-event-observer: the last report is the outer session's"
    );

    proposed_paths(&step_of(&executor));
    assert_eq!(
        (executor.sessions().len(), logs.borrow().len()),
        (2, 2),
        "story:session-executor-event-observer: the executor serves the next call"
    );
}
