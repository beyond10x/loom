//! Acceptance for `story:tool-decisions-read-back`: after a session, the executor's report lists
//! each tool call's decision and who made it, read from the `trace-ir/1` document of the events
//! metaharness delivered, in the order the record holds them; a record that holds no decision
//! census reports the decisions as unknown, never as none. Loom decides nothing here that
//! metaharness does not record: the executor answers metaharness's `tool.decide`, and the record
//! says so.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, the lines a Codex rollout and
//! its hook requests print): no harness program runs, no model is called and nothing reaches the
//! network.

use std::path::{Path, PathBuf};

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorStep, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptStep, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::model::session::{CallDecision, DecidedBy, ToolDecision};
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
            "session-executor-decisions-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    fn executor(&self, script: fn(&Path) -> Vec<ScriptStep>) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(Kind::Codex)
                .with_runner(Box::new(Session(script)));
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

/// A scripted session printing the steps its function makes of the copy it is started in.
struct Session(fn(&Path) -> Vec<ScriptStep>);

impl ProcessRunner for Session {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        ScriptedRunner::new((self.0)(plan.cwd), ScriptedLog::new()).start(plan)
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

/// A pinned Codex rollout with two calls the executor decides: a listing of the copy, which it
/// allows, then a read of the user's private key, which it denies. With `complete`, the turn
/// completes and the rollout's terminal record carries metaharness's decision census; without
/// it, the rollout has no terminal record, so the record holds the decisions and no census.
fn rollout(cwd: &Path, complete: bool) -> Vec<ScriptStep> {
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
    steps.extend(hook(
        "call-key",
        "Bash",
        &serde_json::json!({"command": "cat ~/.ssh/id_ed25519"}),
        cwd,
    ));
    if complete {
        steps.push(ScriptStep::line(completed.to_string()));
    }
    steps
}

fn completed(cwd: &Path) -> Vec<ScriptStep> {
    rollout(cwd, true)
}

fn without_terminal_record(cwd: &Path) -> Vec<ScriptStep> {
    rollout(cwd, false)
}

fn step_of(executor: &SessionExecutor) -> ExecutorStep {
    let case = CaseId("case-decisions".to_owned());
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-decisions".to_owned()),
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

/// A session whose record holds an allowed and a denied call reports both, with who decided
/// each, in the order the record holds them; the denial carries its reason.
#[test]
fn each_tool_decision_of_the_record_is_in_the_report() {
    let scratch = Scratch::new("both");
    let executor = scratch.executor(completed);

    let step = step_of(&executor);

    let report = report_of(&executor);
    let decisions = report.tool_decisions.clone().unwrap_or_else(|| {
        panic!(
            "the decisions are unknown; the session reported {:?}, step {:?}",
            report.error, step.outcome
        )
    });
    let reason = decisions
        .get(1)
        .and_then(|denied| denied.reason.clone())
        .expect("the denial's reason");
    assert!(!reason.trim().is_empty(), "a denial states why");
    assert_eq!(
        decisions,
        vec![
            ToolDecision {
                call_id: "call-ls".to_owned(),
                tool: Some("Bash".to_owned()),
                decision: CallDecision::Allow,
                reason: None,
                decided_by: DecidedBy::Embedder,
            },
            ToolDecision {
                call_id: "call-key".to_owned(),
                tool: Some("Bash".to_owned()),
                decision: CallDecision::Deny,
                reason: Some(reason),
                decided_by: DecidedBy::Embedder,
            },
        ]
    );
    // The step's observation names the record the decisions were read from; nothing of them is
    // a proposal's argument, so none reaches evidence through an effect.
    assert_eq!(
        step.transcript_digest,
        report
            .trace
            .as_ref()
            .and_then(|trace| trace.transcript_digest.clone()),
        "the step names the record the report read"
    );
    if let b10x_loom_commission::model::responsibility::ExecutorOutcome::ProposedAction(proposed) =
        &step.outcome
    {
        let arguments = format!("{:?}", proposed.arguments);
        assert!(
            !arguments.contains("call-ls") && !arguments.contains("call-key"),
            "no decision rides in the proposal: {arguments}"
        );
    }
}

/// A record that holds decided calls but no decision census reports the decisions as unknown:
/// never as none, and never as a list that may be missing some.
#[test]
fn a_record_without_decisions_says_so() {
    let scratch = Scratch::new("no-census");
    let executor = scratch.executor(without_terminal_record);

    let _ = step_of(&executor);

    let report = report_of(&executor);
    assert!(report.started, "the session ran: {:?}", report.error);
    assert!(
        !report.transcript.is_empty(),
        "the session left a record whose census is missing"
    );
    assert_eq!(
        report.tool_decisions, None,
        "a record with no census reports its decisions as unknown"
    );
}
