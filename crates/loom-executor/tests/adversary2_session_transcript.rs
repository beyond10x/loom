// SPDX-License-Identifier: Apache-2.0

//! Adversary pass 2 on `story:session-transcript-streaming`: the pass-1 fixes in
//! `crates/loom-executor/src/session.rs` (the lifecycle state and its `.json.lock` claim, the hard-link
//! first filing, panic filing, `RunEnding::Stopped`, `RunPorts`) driven against the unit's own
//! specification (`ess/domains/run.yaml`: `loom.run.OpenSession`, `loom.run.FileSession`,
//! `loom.run.ResumeSession`, `loom.run.Turn`) and its own documentation.
//!
//! Every model here is a scripted [`ModelPort`] in this process: no socket, no frontier tool.

use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

use b10x_loom_executor::harness::turn_loop::{
    ApprovalDecision, ApprovalPort, ApproveAll, LoopConfig, LoopEvent, LoopSink,
};
use b10x_loom_executor::harness::wire::{
    Approval, CallId, Envelope, Item, ModelPort, StopReason, StreamSink, ToolCall, ToolName,
    ToolOutcome, ToolPort, ToolSpec, TurnOutcome, TurnRequest, Usage, WireError, WireId,
};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{
    CommissionRunId, RunEnding, SessionData, SessionId, SessionState,
};
use b10x_loom_executor::session::{FiledRun, RunPorts, SessionError, SessionFile, run_and_file};
use serde_json::json;

const RESPONSES: &str = b10x_loom_executor::harness::responses::WIRE;
const RUN: &str = "00000000-0000-4000-8000-0000000adbbb";

// --- the lifecycle the pass-1 fix wrote to disk -------------------------------------------------

/// A resume writes the session `Active` on disk, and only a filing by the same value writes it
/// `Filed` again. A run that dies without unwinding (SIGKILL, an OOM kill, a power cut, and a
/// Ctrl-C: `run_and_file` offers no cancel handle) never files, so the file says `Active` with no
/// run behind it. On disk that is exactly a claimed value dropped without filing. The story says a
/// session is "resumable by id" (§ Outcome), and Harness's was; this one is refused for ever as
/// "another run holds it", and no public call clears it (`load` holds nothing, `file` refuses a
/// value it does not hold, `open` answers `session-exists`).
#[test]
fn a_session_whose_resuming_run_died_can_be_resumed_again() {
    let root = scratch("dead_resumer");
    let (workspace, sessions) = layout(&root);
    let id = id("00000000-0000-4000-8000-0000000ae001");
    let mut opened = open(&id, &workspace);
    opened.items.push(Item::user("FILED-TURN"));
    opened
        .file(&sessions, RunEnding::Answered)
        .expect("the first run files");

    let wire = WireId::new(RESPONSES).expect("a wire");
    let claimed = SessionFile::resume(&sessions, &id, &wire, &workspace).expect("first resume");
    // The resuming process dies mid-run: nothing unwinds, nothing files.
    drop(claimed);
    // Adapted to coordinator decision 1 (wave 2026-10-04-w15): the operator releases the session
    // (loom.run.ReleaseSession), filing it back as Failed; the resume below then goes through.
    SessionFile::release(&sessions, &id).expect("the operator releases the dead run's session");
    assert_eq!(
        SessionFile::load(&sessions, &id)
            .expect("loads")
            .run_ending(),
        Some(RunEnding::Failed)
    );

    let again = SessionFile::resume(&sessions, &id, &wire, &workspace);
    assert!(
        again.is_ok(),
        "a session whose resuming run died is never resumable again, and nothing in the API can \
         release it: {}",
        again
            .map(|_| String::new())
            .unwrap_or_else(|e| e.to_string())
    );
}

/// `loom.run.FileSession` answers `wrong-state` with `loom.run.SessionStateConflict` for a session
/// resting in `Filed` (`ess/domains/run.yaml`; `website/docs/reference/ess/loom-run.md`
/// § `FileSession`). Filing a session already filed, or one only read, is that case.
#[test]
fn filing_a_session_already_filed_is_the_specified_wrong_state() {
    let root = scratch("file_wrong_state");
    let (workspace, sessions) = layout(&root);
    let id = id("00000000-0000-4000-8000-0000000ae002");
    let mut opened = open(&id, &workspace);
    opened
        .file(&sessions, RunEnding::Answered)
        .expect("the first filing");

    let twice = opened.file(&sessions, RunEnding::Answered);
    let mut read = SessionFile::load(&sessions, &id).expect("loads");
    let read_filed = read.file(&sessions, RunEnding::Answered);
    for (what, result) in [("filed twice", twice), ("read, then filed", read_filed)] {
        match result {
            Err(SessionError::StateConflict(conflict)) => {
                assert_eq!(conflict.state, SessionState::Filed, "{what}");
            }
            other => panic!(
                "{what}: FileSession on a Filed session answered {other:?}, not \
                 SessionStateConflict {{ state: Filed }} (FileSession `wrong-state`)"
            ),
        }
    }
}

/// `loom.run.OpenSession` answers `session-exists` *instead of* opening: "the first call creates
/// the record, the second is answered by this branch" (`loom-run.md` § `OpenSession`). The fix
/// refuses only at the first filing, after the whole run, so the model is asked and billed for a
/// run whose conversation can then never be filed.
#[test]
fn opening_a_filed_identity_is_refused_before_anything_is_sent() {
    let root = scratch("open_existing_sends");
    let (workspace, sessions) = layout(&root);
    let id = id("00000000-0000-4000-8000-0000000ae003");
    let mut original = open(&id, &workspace);
    original
        .file(&sessions, RunEnding::Answered)
        .expect("the original files");

    let mut again = open(&id, &workspace);
    let mut model = Script::new(vec![Turn::Answer]);
    let filed = run(
        &mut model,
        &mut NoTools::default(),
        &mut ApproveAll,
        &mut again,
        &sessions,
    );
    assert!(
        matches!(filed.filed, Err(SessionError::Exists(_))),
        "precondition: the filing is refused as session-exists: {:?}",
        filed.filed
    );
    assert_eq!(
        model.requests.len(),
        0,
        "OpenSession `session-exists` came after the run: the model was asked {} time(s) and the \
         run's answer ({:?}) has nowhere to be filed",
        model.requests.len(),
        filed.run.as_ref().map(|outcome| outcome.text.clone())
    );
}

// --- RunEnding::Stopped and the conversation it files -------------------------------------------

/// A run that stops `AwaitingApproval` is filed `Stopped`, holding the approval-gated call with no
/// result after it. `loom.run.Turn` says "a turn that did not complete is never recorded, so a
/// filed session holds exactly the turns completed before the run ended", and the loop's own rule
/// (`turn_loop/mod.rs` at the cancel check) is that a `function_call` replayed without its output
/// is a provider error on the next turn. The checkpoint that could complete it is not in the
/// session, so resuming this session sends a conversation no provider accepts.
#[test]
fn a_run_suspended_for_approval_is_filed_as_a_replayable_conversation() {
    let root = scratch("awaiting_approval");
    let (workspace, sessions) = layout(&root);
    let id = id("00000000-0000-4000-8000-0000000ae004");
    let mut session = open(&id, &workspace);
    let mut model = Script::new(vec![Turn::Call("call-1", "fs.write"), Turn::Answer]);
    let mut tools = NoTools::with(vec![spec("fs.write", Approval::Required)]);
    let mut approvals = Defer;
    let filed = run(
        &mut model,
        &mut tools,
        &mut approvals,
        &mut session,
        &sessions,
    );
    let outcome = filed
        .run
        .expect("the loop returns the suspension as an outcome");
    assert!(
        outcome.checkpoint.is_some(),
        "precondition: the run is suspended at a checkpoint: {:?}",
        outcome.stop
    );
    filed.filed.expect("files");
    let stored = SessionFile::load(&sessions, &id).expect("loads");
    assert_eq!(
        stored.run_ending(),
        Some(RunEnding::Stopped),
        "precondition"
    );
    let dangling: Vec<&CallId> = stored
        .items
        .iter()
        .enumerate()
        .filter_map(|(at, item)| {
            match item {
            Item::ToolCall(call)
                if !stored.items[at + 1..].iter().any(|later| {
                    matches!(later, Item::ToolResult { call_id, .. } if call_id == &call.call_id)
                }) =>
            {
                Some(&call.call_id)
            }
            _ => None,
        }
        })
        .collect();
    assert!(
        dangling.is_empty(),
        "a session filed Stopped after AwaitingApproval holds call(s) {dangling:?} with no result; \
         resuming it replays a function_call without its output: {:?}",
        stored.items
    );
    // Adapted to coordinator decision 4 (wave 2026-10-04-w15): the session is filed up to its last
    // completed turn; the unfinished turn (the call awaiting approval) is not stored.
    assert_eq!(stored.items, [Item::user("QUESTION")]);
}

// --- panic filing -------------------------------------------------------------------------------

/// A panic on turn two, after turn one was billed: the `LoopEvent::Usage` for turn one has gone
/// out on the sink, and `RunLedger`'s own docs say "a session file holding the conversation but not
/// the figures would report a failed run as free". `run_and_file` sees every event (it holds the
/// sink), yet the session it files on a panic records no turn and no usage.
#[test]
fn a_run_whose_loop_panics_files_what_it_already_spent() {
    let root = scratch("panic_spend");
    let (workspace, sessions) = layout(&root);
    let id = id("00000000-0000-4000-8000-0000000ae005");
    let mut session = open(&id, &workspace);
    let mut model = Script::new(vec![Turn::Call("call-1", "workspace_read"), Turn::Panic]);
    let mut tools = NoTools::with(vec![spec("workspace_read", Approval::NotRequired)]);
    let mut sink = Usages::default();
    let caught = catch_unwind(AssertUnwindSafe(|| {
        run_and_file(
            ports(&mut model, &mut tools, &mut ApproveAll),
            &mut session,
            &sessions,
            "QUESTION",
            &mut sink,
        )
    }));
    assert!(caught.is_err(), "precondition: the panic carried on");
    assert_eq!(
        sink.usage.len(),
        1,
        "precondition: turn one was billed on the sink"
    );
    let stored = SessionFile::load(&sessions, &id).expect("the panicking run was filed");
    assert_eq!(stored.run_ending(), Some(RunEnding::Failed), "precondition");
    assert!(
        stored.turns >= 1 && stored.usage == sink.usage,
        "the session filed after a panic reports the run as free: turns {}, usage {:?}, while the \
         sink saw {:?}",
        stored.turns,
        stored.usage,
        sink.usage
    );
}

/// The panic that carries on is the loop's own, payload included, even when the payload is not a
/// string. Expected green: evidence for the "the panic then carries on" promise.
#[test]
fn the_panic_run_and_file_carries_on_is_the_loops_own_payload() {
    #[derive(Debug, PartialEq)]
    struct Marker(u32);

    let root = scratch("panic_payload");
    let (workspace, sessions) = layout(&root);
    let id = id("00000000-0000-4000-8000-0000000ae006");
    let mut session = open(&id, &workspace);
    let mut model = Script::new(vec![Turn::PanicWith(Box::new(|| panic_any(Marker(42))))]);
    let caught = catch_unwind(AssertUnwindSafe(|| {
        run(
            &mut model,
            &mut NoTools::default(),
            &mut ApproveAll,
            &mut session,
            &sessions,
        )
    }));
    let payload = caught.expect_err("the panic carried on");
    assert_eq!(payload.downcast_ref::<Marker>(), Some(&Marker(42)));
    let stored = SessionFile::load(&sessions, &id).expect("filed");
    assert_eq!(stored.run_ending(), Some(RunEnding::Failed));
}

// --- the claim, concurrently --------------------------------------------------------------------

/// Sixteen resumers released at once on one filed session, 400 rounds: at most one claims it.
/// Expected green. Without `Claim::take` (adversary pass 2, m-lock) it went red at round 213.
#[test]
fn concurrent_resumers_claim_a_filed_session_exactly_once() {
    let root = scratch("concurrent_resume");
    let (workspace, sessions) = layout(&root);
    let wire = WireId::new(RESPONSES).expect("a wire");
    for round in 0..400_u32 {
        let id = id(&format!("00000000-0000-4000-8000-0000001{round:05}"));
        let mut opened = open(&id, &workspace);
        opened.file(&sessions, RunEnding::Answered).expect("filed");
        let barrier = Arc::new(Barrier::new(16));
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let (barrier, sessions, workspace, wire, id) = (
                    Arc::clone(&barrier),
                    sessions.clone(),
                    workspace.clone(),
                    wire.clone(),
                    id.clone(),
                );
                std::thread::spawn(move || {
                    barrier.wait();
                    SessionFile::resume(&sessions, &id, &wire, &workspace).is_ok()
                })
            })
            .collect();
        let claimed = handles
            .into_iter()
            .map(|handle| handle.join().expect("joins"))
            .filter(|claimed| *claimed)
            .count();
        assert!(
            claimed <= 1,
            "round {round}: {claimed} resumers claimed one session"
        );
    }
}

/// A `<id>.json.lock` left by a resume that crashed inside its claim is refused by name, and the
/// refusal says how to recover; once the file is removed the session resumes. Expected green: no
/// other test in the suite kills a mutant that drops `Claim::take` (adversary pass 2, m-lock), so
/// this is the case that says the lock exists.
#[test]
fn a_lock_left_by_a_crashed_resume_is_refused_by_name_and_recoverable() {
    let root = scratch("stale_lock");
    let (workspace, sessions) = layout(&root);
    let id = id("00000000-0000-4000-8000-0000000ae007");
    let mut opened = open(&id, &workspace);
    opened.file(&sessions, RunEnding::Answered).expect("filed");
    let lock = sessions.join(format!("{}.json.lock", id.0.0));
    std::fs::write(&lock, b"").expect("a crashed resume's lock");

    let wire = WireId::new(RESPONSES).expect("a wire");
    let refused = SessionFile::resume(&sessions, &id, &wire, &workspace)
        .map(|_| ())
        .expect_err("a resume past a held lock");
    let message = refused.to_string();
    assert!(
        message.contains(&*lock.to_string_lossy()) && message.contains("remove that file"),
        "{message}"
    );
    let stored = SessionFile::load(&sessions, &id).expect("loads");
    assert_eq!(
        SessionState::from(stored.state),
        SessionState::Filed,
        "the refused resume wrote nothing"
    );
    std::fs::remove_file(&lock).expect("the operator removes it");
    SessionFile::resume(&sessions, &id, &wire, &workspace).expect("resumes once the lock is gone");
    assert!(!lock.exists(), "the claim removes its own lock");
}

// --- scripted ports -----------------------------------------------------------------------------

enum Turn {
    Answer,
    Call(&'static str, &'static str),
    Panic,
    PanicWith(Box<dyn Fn()>),
}

struct Script {
    wire: WireId,
    turns: std::collections::VecDeque<Turn>,
    requests: Vec<TurnRequest>,
}

impl Script {
    fn new(turns: Vec<Turn>) -> Self {
        Self {
            wire: WireId::new(RESPONSES).expect("a wire"),
            turns: turns.into(),
            requests: Vec::new(),
        }
    }
}

impl ModelPort for Script {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.requests.push(request.clone());
        let usage = Some(Usage {
            model: "adversary-model".to_owned(),
            input_tokens: 10,
            output_tokens: 5,
            cached_input_tokens: 0,
            cache_creation_input_tokens: None,
        });
        match self.turns.pop_front().unwrap_or(Turn::Answer) {
            Turn::Answer => Ok(TurnOutcome {
                stop_reason: StopReason::EndTurn,
                items: vec![Item::assistant("ANSWER")],
                usage,
            }),
            Turn::Call(call_id, name) => Ok(TurnOutcome {
                stop_reason: StopReason::ToolCalls,
                items: vec![Item::ToolCall(ToolCall {
                    call_id: CallId::new(call_id).expect("a call id"),
                    name: ToolName::new(name).expect("a tool name"),
                    arguments: json!({"path": "a"}),
                })],
                usage,
            }),
            Turn::Panic => panic!("the model port panicked on a later turn"),
            Turn::PanicWith(raise) => {
                raise();
                unreachable!("the closure panics")
            }
        }
    }
}

#[derive(Default)]
struct NoTools {
    specs: Vec<ToolSpec>,
}

impl NoTools {
    fn with(specs: Vec<ToolSpec>) -> Self {
        Self { specs }
    }
}

impl ToolPort for NoTools {
    fn specs(&self) -> &[ToolSpec] {
        &self.specs
    }

    fn call(&mut self, _call: &ToolCall) -> ToolOutcome {
        ToolOutcome::ok(json!({"read": "contents"}))
    }
}

struct Defer;

impl ApprovalPort for Defer {
    fn decide(&mut self, _: &ToolCall, _: &ToolSpec) -> ApprovalDecision {
        ApprovalDecision::deferred("checkpoint-1")
    }
}

#[derive(Default)]
struct Usages {
    usage: Vec<Usage>,
}

impl LoopSink for Usages {
    fn emit(&mut self, event: LoopEvent) {
        if let LoopEvent::Usage(usage) = event {
            self.usage.push(usage);
        }
    }
}

fn spec(name: &str, approval: Approval) -> ToolSpec {
    ToolSpec {
        name: ToolName::new(name).expect("a tool name"),
        description: format!("the {name} tool"),
        envelope: Envelope::default(),
        input_schema: json!({"type": "object"}),
        approval,
    }
}

// --- helpers ------------------------------------------------------------------------------------

fn ports<'a>(
    model: &'a mut Script,
    tools: &'a mut NoTools,
    approvals: &'a mut dyn ApprovalPort,
) -> RunPorts<'a> {
    RunPorts {
        model,
        tools,
        approvals,
        config: LoopConfig::new("adversary-model", "ADVERSARY-INSTRUCTIONS"),
    }
}

fn run(
    model: &mut Script,
    tools: &mut NoTools,
    approvals: &mut dyn ApprovalPort,
    session: &mut SessionFile,
    sessions: &Path,
) -> FiledRun {
    run_and_file(
        ports(model, tools, approvals),
        session,
        sessions,
        "QUESTION",
        &mut Usages::default(),
    )
}

fn open(id: &SessionId, workspace: &Path) -> SessionFile {
    SessionFile::open(
        &SessionData {
            session_id: id.clone(),
            commission_run: CommissionRunId(Uuid(RUN.to_owned())),
            wire: RESPONSES.to_owned(),
        },
        "adversary-model",
        "http://127.0.0.1:9/v1",
        workspace,
    )
    .expect("a session opens")
}

fn id(text: &str) -> SessionId {
    SessionId(Uuid(text.to_owned()))
}

fn layout(root: &Path) -> (PathBuf, PathBuf) {
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create");
    (workspace, root.join("state").join("sessions"))
}

fn scratch(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adversary2-{name}-{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("clear scratch");
    }
    root
}
