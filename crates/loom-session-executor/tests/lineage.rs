//! Acceptance for `story:session-lineage`: `SessionExecutor::continuing` records that a session
//! continues an earlier one (`loom.session.AgentSession.continues`), refuses a predecessor of
//! another commission before any copy or session, and leaves the request to metaharness as it is
//! for a fresh session: no resume flag, the harness session itself is not resumed.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, the records a Claude Code
//! session prints): no harness program runs, no model is called and nothing reaches the network.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
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
use b10x_loom_session_executor::model::session::AgentSessions;
use b10x_loom_session_executor::{
    CredentialSource, ProcessRunner, SessionExecutor, ToolPolicyDefaults as _,
};

const EDIT: &str = "implementation.edit";
const BRIEF: &str = "fix add";
const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A workspace of one file and a scratch directory, under the system's temporary directory;
/// removed when dropped. Not under the repository: metaharness refuses a Claude session whose
/// copy has memory files above it.
struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "session-executor-lineage-{}-{name}",
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

    fn scratch(&self) -> PathBuf {
        self.root.join("scratch")
    }

    /// An executor over the workspace's one file, on no credential, its copies under the
    /// fixture's scratch, its sessions the ones `script` plays.
    fn executor(&self, script: &Script) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(&self.workspace, vec!["src/lib.rs".to_owned()], BRIEF)
                .with_runner(Box::new(script.clone()));
        executor.credentials = CredentialSource::None;
        executor.scratch = self.scratch();
        executor
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// What one session was started with, as metaharness handed it to the runner: the program, its
/// arguments and its environment, each path under the session's own directories written as
/// `<cwd>` or `<run>`, so two sessions' requests compare.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Request {
    program: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
}

/// A scripted Claude Code session that keeps the request it was started with, fixes `add` in its
/// copy and replies naming the edit.
#[derive(Clone, Default)]
struct Script {
    requests: Rc<RefCell<Vec<Request>>>,
}

impl Script {
    fn requests(&self) -> Vec<Request> {
        self.requests.borrow().clone()
    }
}

/// `text` with every spelling of `cwd` and of the directory metaharness made for the run (the
/// decision channel's and the transcript's parent) replaced by a placeholder.
fn normalized(text: &str, cwd: &Path, run: &[&Path]) -> String {
    let mut text = text.replace(&cwd.display().to_string(), "<cwd>");
    for dir in run {
        text = text.replace(&dir.display().to_string(), "<run>");
    }
    text
}

impl ProcessRunner for Script {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        let run: Vec<&Path> = [plan.decision_channel, plan.transcript]
            .into_iter()
            .filter_map(Path::parent)
            .collect();
        let at = |text: &str| normalized(text, plan.cwd, &run);
        self.requests.borrow_mut().push(Request {
            program: at(plan.program),
            args: plan.args.iter().map(|arg| at(arg)).collect(),
            env: plan
                .env
                .iter()
                .map(|(name, value)| (name.clone(), at(value)))
                .collect(),
        });
        std::fs::write(plan.cwd.join("src/lib.rs"), FIXED)?;
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
                        "usage": {"input_tokens": 1000, "output_tokens": 50}},
        });
        let result = serde_json::json!({
            "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
            "duration_ms": 2000, "total_cost_usd": 0.0003,
            "usage": {"input_tokens": 1000, "output_tokens": 50},
        });
        ScriptedRunner::of_lines(
            [init.to_string(), reply.to_string(), result.to_string()],
            ScriptedLog::new(),
        )
        .start(plan)
    }
}

fn commission(n: u64) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(n)),
        agent_revision_id: AgentRevisionId(uuid(0xa1)),
        case_id: CaseId("case-1".to_owned()),
        principal: PrincipalId("principal-s".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
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

const COMMISSION: u64 = 0xc1;
const OTHER_COMMISSION: u64 = 0xc2;

/// One session run by `executor` for commission `n`, which proposes; the row it recorded.
fn one_session(executor: &SessionExecutor, n: u64) -> AgentSessions {
    let step = executor.run_with_usage(&commission(n), &frontier());
    let ExecutorOutcome::ProposedAction(_) = &step.outcome else {
        panic!(
            "a proposal: {:?} ({:?})",
            step.outcome,
            executor.last_report().map(|report| report.error)
        );
    };
    executor
        .sessions()
        .pop()
        .expect("the session the executor recorded")
}

/// The first session of a commission, run by its own executor: the row a host holds.
fn first_session(fixture: &Fixture, n: u64) -> AgentSessions {
    let script = Script::default();
    let first = one_session(&fixture.executor(&script), n);
    assert_eq!(first.continues, None, "the first session continues none");
    first
}

#[test]
fn a_continuing_session_names_its_predecessor() {
    let fixture = Fixture::new("names");
    let first = first_session(&fixture, COMMISSION);

    let script = Script::default();
    let executor = fixture.executor(&script).continuing(&first);
    let second = one_session(&executor, COMMISSION);

    assert_ne!(second.session_id, first.session_id, "a session of its own");
    assert_eq!(second.continues, Some(first.session_id.clone()));
    let proposed = executor.proposed();
    assert_eq!(proposed.len(), 1);
    assert_eq!(
        proposed[0].continues,
        Some(first.session_id),
        "the event names the predecessor too"
    );
}

#[test]
fn a_session_without_continuing_records_no_predecessor() {
    let fixture = Fixture::new("none");
    let script = Script::default();
    let executor = fixture.executor(&script);
    let first = one_session(&executor, COMMISSION);
    let second = one_session(&executor, COMMISSION);
    assert_eq!(first.continues, None);
    assert_eq!(second.continues, None, "lineage is never inferred");
    assert!(
        executor
            .proposed()
            .iter()
            .all(|event| event.continues.is_none())
    );
}

#[test]
fn the_predecessor_names_only_the_next_session() {
    let fixture = Fixture::new("next");
    let first = first_session(&fixture, COMMISSION);

    let script = Script::default();
    let executor = fixture.executor(&script).continuing(&first);
    let second = one_session(&executor, COMMISSION);
    let third = one_session(&executor, COMMISSION);
    assert_eq!(second.continues, Some(first.session_id));
    assert_eq!(
        third.continues, None,
        "the predecessor is the next session's"
    );
}

#[test]
fn a_predecessor_of_another_commission_is_refused_naming_it() {
    let fixture = Fixture::new("refused");
    let first = first_session(&fixture, COMMISSION);
    let copies_before: Vec<PathBuf> = std::fs::read_dir(fixture.scratch())
        .expect("the scratch directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();

    let script = Script::default();
    let executor = fixture.executor(&script).continuing(&first);
    let step = executor.run_with_usage(&commission(OTHER_COMMISSION), &frontier());

    let ExecutorOutcome::Suspended(suspended) = &step.outcome else {
        panic!("a refusal: {:?}", step.outcome);
    };
    let SuspensionReason::ExternalAvailability(Value::Text(why)) = &suspended.reason else {
        panic!(
            "suspended for an external availability: {:?}",
            suspended.reason
        );
    };
    for named in [
        &first.session_id.0.0,
        &uuid(COMMISSION).0,
        &uuid(OTHER_COMMISSION).0,
    ] {
        assert!(why.contains(named.as_str()), "{named} is named: {why}");
    }
    assert!(script.requests().is_empty(), "no session ran");
    assert!(executor.sessions().is_empty(), "nothing is recorded");
    assert!(executor.proposed().is_empty(), "nothing is published");
    let copies_after: Vec<PathBuf> = std::fs::read_dir(fixture.scratch())
        .expect("the scratch directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(copies_after, copies_before, "no copy was made");
    let report = executor.last_report().expect("the call's report");
    assert!(!report.started, "the session never started");
    assert_eq!(report.error.as_deref(), Some(why.as_str()));
}

#[test]
fn the_request_of_a_continuing_session_equals_a_fresh_ones() {
    let fixture = Fixture::new("request");
    let first = first_session(&fixture, COMMISSION);

    let fresh_script = Script::default();
    one_session(&fixture.executor(&fresh_script), COMMISSION);
    let continuing_script = Script::default();
    one_session(
        &fixture.executor(&continuing_script).continuing(&first),
        COMMISSION,
    );

    let fresh = fresh_script.requests();
    let continuing = continuing_script.requests();
    assert_eq!(fresh.len(), 1);
    assert_eq!(continuing, fresh, "no resume flag, no other request");
    assert!(
        continuing[0]
            .args
            .iter()
            .chain(continuing[0].env.iter().map(|(_, value)| value))
            .all(|text| !text.contains(&first.session_id.0.0)),
        "the predecessor's id never reaches metaharness"
    );
}
