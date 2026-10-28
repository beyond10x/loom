//! Adversary cases for `story:session-lineage`: `SessionExecutor::continuing` and the
//! `loom.session.AgentSession.continues` it records.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, the records a Claude Code
//! session prints): no harness program runs, no model is called and nothing reaches the network.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    SuspensionReason, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::model::agent::ToolPolicy;
use b10x_loom_session_executor::model::primitives::Uuid as SessionUuid;
use b10x_loom_session_executor::model::session::{AgentSessions, SessionId};
use b10x_loom_session_executor::{
    CredentialSource, ProcessRunner, SessionExecutor, ToolPolicyDefaults as _,
};

const EDIT: &str = "implementation.edit";
const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
const COMMISSION: u64 = 0xc1;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn session_id(n: u64) -> SessionId {
    SessionId(SessionUuid(format!("00000000-0000-4000-9000-{n:012x}")))
}

struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "session-executor-adv-w3u2-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let workspace = root.join("workspace");
        std::fs::create_dir_all(workspace.join("src")).expect("a directory");
        std::fs::write(
            workspace.join("src/lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n",
        )
        .expect("a fixture file");
        std::fs::create_dir_all(root.join("scratch")).expect("the scratch directory");
        Self { root, workspace }
    }

    fn executor(&self, script: &Script) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(&self.workspace, vec!["src/lib.rs".to_owned()], "fix add")
                .with_runner(Box::new(script.clone()));
        executor.credentials = CredentialSource::None;
        executor.scratch = self.root.join("scratch");
        executor
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A scripted Claude Code session that fixes `add` in its copy. The first `fail` starts fail
/// (the session never starts); the first `clobber` sessions also write the workspace's own file
/// while they run, so the step suspends after the session ran.
#[derive(Clone, Default)]
struct Script {
    starts: Rc<Cell<u32>>,
    fail: u32,
    clobber: Option<(PathBuf, u32)>,
}

impl ProcessRunner for Script {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        let n = self.starts.get();
        self.starts.set(n + 1);
        if n < self.fail {
            return Err(std::io::Error::other("scripted start failure"));
        }
        std::fs::write(plan.cwd.join("src/lib.rs"), FIXED)?;
        if let Some((workspace, times)) = &self.clobber
            && n < *times
        {
            std::fs::write(workspace.join("src/lib.rs"), "// written while it ran\n")?;
        }
        let init = serde_json::json!({
            "type": "system", "subtype": "init", "claude_code_version": "2.1.304",
            "cwd": plan.cwd.display().to_string(), "tools": ToolPolicy::claude_default().allow,
            "mcp_servers": [], "plugins": [], "output_style": "default", "apiKeySource": "none",
            "model": "scripted", "permissionMode": "default", "session_id": "vendor-1",
        });
        let said = serde_json::json!({"action": EDIT, "summary": "fixed add"}).to_string();
        let reply = serde_json::json!({
            "type": "assistant",
            "message": {"content": [{"type": "text", "text": said}],
                        "usage": {"input_tokens": 10, "output_tokens": 5}},
        });
        let result = serde_json::json!({
            "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
            "duration_ms": 20, "total_cost_usd": 0.0001,
            "usage": {"input_tokens": 10, "output_tokens": 5},
        });
        ScriptedRunner::of_lines(
            [init.to_string(), reply.to_string(), result.to_string()],
            ScriptedLog::new(),
        )
        .start(plan)
    }
}

fn commission_spelled(id: &str) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(Uuid(id.to_owned())),
        agent_revision_id: AgentRevisionId(uuid(0xa1)),
        case_id: CaseId("case-1".to_owned()),
        principal: PrincipalId("principal-s".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn commission(n: u64) -> Commission<commission_state::Assigned> {
    commission_spelled(&uuid(n).0)
}

fn frontier() -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf1)),
        case_id: CaseId("case-1".to_owned()),
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

fn proposes(executor: &SessionExecutor, on: &Commission<commission_state::Assigned>) -> Value {
    let step = executor.run_with_usage(on, &frontier());
    let ExecutorOutcome::ProposedAction(proposed) = step.outcome else {
        panic!(
            "a proposal: {:?} ({:?})",
            step.outcome,
            executor.last_report().map(|report| report.error)
        );
    };
    proposed.arguments.0
}

fn suspends(executor: &SessionExecutor, on: &Commission<commission_state::Assigned>) -> String {
    let step = executor.run_with_usage(on, &frontier());
    let ExecutorOutcome::Suspended(suspended) = step.outcome else {
        panic!("a suspension: {:?}", step.outcome);
    };
    let SuspensionReason::ExternalAvailability(Value::Text(why)) = suspended.reason else {
        panic!("an external availability: {:?}", suspended.reason);
    };
    why
}

/// The first session of the commission, recorded by an executor of its own.
fn first_session(fixture: &Fixture) -> AgentSessions {
    let executor = fixture.executor(&Script::default());
    proposes(&executor, &commission(COMMISSION));
    executor.sessions().pop().expect("the first session")
}

/// Lineage within one executor's record: every `continues` chain ends, and no row names itself.
fn assert_no_loop(rows: &[AgentSessions]) {
    for row in rows {
        let mut seen = BTreeSet::new();
        let mut at = Some(row.session_id.clone());
        while let Some(id) = at {
            assert!(
                seen.insert(id.0.0.clone()),
                "the lineage of {} loops back to {}: {rows:#?}",
                row.session_id.0.0,
                id.0.0
            );
            at = rows
                .iter()
                .find(|held| held.session_id == id)
                .and_then(|held| held.continues.clone());
        }
    }
}

/// A host assigning session ids (`with_session_ids`) hands out the predecessor's own id: the
/// session recorded must not name itself as the session it continues, and must not replace the
/// predecessor's row with one that does.
#[test]
fn adversary_a_session_never_continues_itself() {
    let fixture = Fixture::new("itself");
    let script = Script::default();
    let executor = fixture.executor(&script).with_session_ids(|| session_id(1));
    proposes(&executor, &commission(COMMISSION));
    let first = executor.sessions().pop().expect("the first session");
    assert_eq!(first.session_id, session_id(1));

    let executor = executor.continuing(&first);
    let step = executor.run_with_usage(&commission(COMMISSION), &frontier());
    let rows = executor.sessions();
    assert!(
        rows.iter()
            .all(|row| row.continues.as_ref() != Some(&row.session_id)),
        "a session continues itself ({:?}): {rows:#?}",
        step.outcome
    );
    assert!(
        rows.iter()
            .any(|row| row.session_id == first.session_id && row.continues.is_none()),
        "the predecessor's row is kept as recorded: {rows:#?}"
    );
}

/// Two continuing calls of one executor whose host-assigned ids come round again: its own record
/// must not hold a lineage that loops.
#[test]
fn adversary_one_executors_lineage_never_loops() {
    let fixture = Fixture::new("loop");
    let ids = Rc::new(RefCell::new(vec![1u64, 2, 1].into_iter()));
    let next = Rc::clone(&ids);
    let executor = fixture
        .executor(&Script::default())
        .with_session_ids(move || session_id(next.borrow_mut().next().expect("an id")));
    proposes(&executor, &commission(COMMISSION));
    let a = executor.sessions().pop().expect("a");
    let executor = executor.continuing(&a);
    proposes(&executor, &commission(COMMISSION));
    let b = executor
        .sessions()
        .into_iter()
        .find(|row| row.session_id == session_id(2))
        .expect("b");
    assert_eq!(b.continues, Some(a.session_id.clone()));
    let executor = executor.continuing(&b);
    let _ = executor.run_with_usage(&commission(COMMISSION), &frontier());
    assert_no_loop(&executor.sessions());
}

/// A commission id is a UUID: the same commission written in capitals is the same commission,
/// as `security_an_approval_is_bound_to_its_commission_however_its_id_is_written` holds for
/// approvals. A predecessor recorded under one spelling is not of "another commission".
#[test]
fn adversary_a_commission_written_in_capitals_is_the_same_commission() {
    let fixture = Fixture::new("capitals");
    let first = first_session(&fixture);
    let upper = uuid(COMMISSION).0.to_uppercase();
    let executor = fixture.executor(&Script::default()).continuing(&first);
    let step = executor.run_with_usage(&commission_spelled(&upper), &frontier());
    assert!(
        matches!(step.outcome, ExecutorOutcome::ProposedAction(_)),
        "the same commission, spelled {upper}, is refused as another: {:?}",
        step.outcome
    );
}

/// Holds: a call whose session never started keeps the predecessor for the next call.
#[test]
fn adversary_a_failed_start_keeps_the_predecessor() {
    let fixture = Fixture::new("failed");
    let first = first_session(&fixture);
    let script = Script {
        fail: 1,
        ..Script::default()
    };
    let executor = fixture.executor(&script).continuing(&first);
    suspends(&executor, &commission(COMMISSION));
    assert!(executor.sessions().is_empty());
    proposes(&executor, &commission(COMMISSION));
    let rows = executor.sessions();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].continues, Some(first.session_id));
}

/// Holds: a session that ran but whose step suspended (the workspace changed under it) is
/// recorded as failed, naming the predecessor, and keeps the predecessor for the next call
/// (story:executor-reports-harness-child-failure; it recorded nothing before).
#[test]
fn adversary_a_session_suspended_after_it_ran_keeps_the_predecessor() {
    let fixture = Fixture::new("suspended");
    let first = first_session(&fixture);
    let script = Script {
        clobber: Some((fixture.workspace.clone(), 1)),
        ..Script::default()
    };
    let executor = fixture.executor(&script).continuing(&first);
    suspends(&executor, &commission(COMMISSION));
    let failed = executor.sessions();
    assert_eq!(failed.len(), 1, "{failed:#?}");
    assert!(failed[0].failure.is_some(), "{failed:#?}");
    assert_eq!(failed[0].continues, Some(first.session_id.clone()));
    std::fs::write(
        fixture.workspace.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n",
    )
    .expect("restore the workspace");
    proposes(&executor, &commission(COMMISSION));
    let rows = executor.sessions();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].failure, None, "the retry proposed");
    assert_eq!(rows[1].continues, Some(first.session_id));
}

/// Holds: a refusal for another commission consumes nothing; the next call for the
/// predecessor's commission continues it, and a refused call starts no session.
#[test]
fn adversary_a_refused_call_keeps_the_predecessor() {
    let fixture = Fixture::new("refused-keeps");
    let first = first_session(&fixture);
    let script = Script::default();
    let executor = fixture.executor(&script).continuing(&first);
    let why = suspends(&executor, &commission(0xc2));
    assert!(why.contains(&first.session_id.0.0), "{why}");
    assert!(why.contains(&uuid(COMMISSION).0), "{why}");
    assert!(why.contains(&uuid(0xc2).0), "{why}");
    assert_eq!(script.starts.get(), 0, "no session started");
    proposes(&executor, &commission(COMMISSION));
    assert_eq!(
        executor.sessions()[0].continues,
        Some(first.session_id.clone())
    );
}

/// Holds: the predecessor reaches neither the proposal's arguments nor the step's summary.
#[test]
fn adversary_continues_never_reaches_the_proposal() {
    let fixture = Fixture::new("arguments");
    let first = first_session(&fixture);
    let executor = fixture.executor(&Script::default()).continuing(&first);
    let step = executor.run_with_usage(&commission(COMMISSION), &frontier());
    let ExecutorOutcome::ProposedAction(proposed) = &step.outcome else {
        panic!("a proposal: {:?}", step.outcome);
    };
    let Value::Object(members) = &proposed.arguments.0 else {
        panic!("an object: {:?}", proposed.arguments.0);
    };
    let names: Vec<&str> = members.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["files"]);
    let text = format!("{:?} {:?}", proposed.arguments.0, step.summary);
    assert!(!text.contains(&first.session_id.0.0), "{text}");
}

/// Holds: a predecessor built for one executor does not leak into another built from the same
/// fixture, and `continuing` twice keeps only the last.
#[test]
fn adversary_continuing_twice_keeps_the_last() {
    let fixture = Fixture::new("twice");
    let a = first_session(&fixture);
    let b = first_session(&fixture);
    let executor = fixture
        .executor(&Script::default())
        .continuing(&a)
        .continuing(&b);
    proposes(&executor, &commission(COMMISSION));
    assert_eq!(executor.sessions()[0].continues, Some(b.session_id));
    let other = fixture.executor(&Script::default());
    proposes(&other, &commission(COMMISSION));
    assert_eq!(other.sessions()[0].continues, None);
}
