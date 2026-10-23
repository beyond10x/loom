//! Adversary cases for `story:session-executor-from-spec`: `SessionExecutor::from_spec` and the
//! tool policy a session runs under.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`): no harness program runs, no
//! model is called and nothing reaches the network.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorStep, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptStep, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::model::agent::{
    AgentId, AgentIdentity, AgentSpec, BreachStep, ConfinementMode, DecisionMode, RunBudget,
    SessionSpec, StopPolicy, ToolPolicy,
};
use b10x_loom_session_executor::model::models::{ModelRole, ModelsConfig, RoleModel};
use b10x_loom_session_executor::model::session::{CallDecision, SessionKind};
use b10x_loom_session_executor::{
    CredentialSource, Endpoint, ProcessRunner, SessionExecutor, ToolPolicyDefaults as _,
};

const EDIT: &str = "implementation.edit";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A workspace of one file and a scratch directory under the system's temporary directory;
/// removed when dropped.
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "session-executor-adv-w20261023w1-u2-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("workspace/src")).expect("a directory");
        std::fs::write(
            root.join("workspace/src/lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n",
        )
        .expect("a fixture file");
        std::fs::create_dir_all(root.join("scratch")).expect("the scratch directory");
        Self { root }
    }

    fn workspace(&self) -> PathBuf {
        self.root.join("workspace")
    }

    fn prepare(
        &self,
        executor: SessionExecutor,
        runner: Box<dyn ProcessRunner>,
    ) -> SessionExecutor {
        let mut executor = executor.with_runner(runner);
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

fn files() -> Vec<String> {
    vec!["src/lib.rs".to_owned()]
}

fn spec(kind: SessionKind, tools: ToolPolicy) -> AgentSpec {
    AgentSpec {
        format: "loom.agent/1".to_owned(),
        agent: AgentIdentity {
            id: AgentId("adversary".to_owned()),
            name: "Adversary".to_owned(),
            brief: "fix add".to_owned(),
        },
        model: ModelsConfig {
            routes: "routes.yaml".to_owned(),
            roles: vec![RoleModel {
                role: ModelRole::Agent,
                model: "sonnet".to_owned(),
            }],
        },
        session: Some(SessionSpec {
            kind,
            budget: RunBudget {
                max_steps: Some(4),
                max_tokens_in: None,
                max_tokens_out: None,
                max_duration_ms: None,
                max_cost_micro_usd: None,
                max_tokens: None,
                deadline: None,
                per_action: Vec::new(),
            },
            stop: StopPolicy {
                approval_gate: true,
                idle_bound: None,
                on_breach: BreachStep::DiscardStep,
                breach_by_limit: Vec::new(),
            },
            confinement: ConfinementMode::None,
            tools,
        }),
        loops: None,
        states: Vec::new(),
        bindings: Vec::new(),
        channel: None,
        writes: None,
        policy: None,
        expose: None,
        runtime: None,
        deploy: None,
    }
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: CaseId("case-1".to_owned()),
        principal: PrincipalId("principal-adv".to_owned()),
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

fn step_of(executor: &SessionExecutor) -> ExecutorStep {
    executor.run_with_usage(&commission(), &frontier())
}

// ------------------------------------------------------------------ Codex under frame/observe

/// A scripted Codex rollout with one `PreToolUse` hook request: `Bash` reading the user's
/// private key, a tool the policies below do not allow.
struct CodexSession;

impl ProcessRunner for CodexSession {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        let cwd = plan.cwd.display().to_string();
        let meta = serde_json::json!({
            "timestamp": "2026-08-22T10:00:00.000Z", "type": "session_meta",
            "payload": {"id": "codex-1", "session_id": "codex-1", "cli_version": "0.153.4",
                        "cwd": cwd, "originator": "codex_exec", "model_provider": "openai"},
        });
        let started = serde_json::json!({
            "timestamp": "2026-08-22T10:00:01.000Z", "type": "event_msg",
            "payload": {"type": "task_started", "turn_id": "t1"},
        });
        let raw = serde_json::json!({
            "session_id": "codex-1", "turn_id": "t1", "cwd": cwd,
            "hook_event_name": "PreToolUse", "model": "scripted", "permission_mode": "default",
            "tool_name": "Bash", "tool_input": {"command": "cat ~/.ssh/id_ed25519"},
            "tool_use_id": "call-key",
        });
        let hook = serde_json::json!({
            "metaharness.codex/1": "hook_request", "key": "call-key", "input": raw,
        });
        let action = serde_json::json!({"action": EDIT, "summary": "looked"}).to_string();
        let completed = serde_json::json!({
            "timestamp": "2026-08-22T10:00:05.000Z", "type": "event_msg",
            "payload": {"type": "task_complete", "turn_id": "t1", "duration_ms": 4200,
                        "error": null, "last_agent_message": action},
        });
        ScriptedRunner::new(
            vec![
                ScriptStep::line(meta.to_string()),
                ScriptStep::line(started.to_string()),
                ScriptStep::line(hook.to_string()),
                ScriptStep::awaiting("call-key"),
                ScriptStep::line(completed.to_string()),
            ],
            ScriptedLog::new(),
        )
        .start(plan)
    }
}

/// A Codex spec that allows only `apply_patch` under `decisions`: either `from_spec` refuses it,
/// as it refuses a Codex spec that denies tools outside `ask` (`SpecError::DenyUnenforced`), or a
/// call of `Bash`, which the policy does not allow, is denied. What must not happen is a session
/// in which the narrowed policy reaches nobody.
fn a_codex_policy_narrowed_under(mode: DecisionMode, name: &str) {
    let fixture = Fixture::new(name);
    let tools = ToolPolicy {
        allow: vec!["apply_patch".to_owned()],
        deny: Vec::new(),
        egress: Vec::new(),
        decisions: mode,
    };
    let Ok(executor) = SessionExecutor::from_spec(
        &spec(SessionKind::Codex, tools),
        fixture.workspace(),
        files(),
    ) else {
        return;
    };
    let executor = fixture.prepare(executor, Box::new(CodexSession));
    let step = step_of(&executor);
    let report = executor.last_report().expect("a report");
    let decisions = report.tool_decisions.clone().unwrap_or_else(|| {
        panic!(
            "the decisions are unknown; error {:?}, outcome {:?}",
            report.error, step.outcome
        )
    });
    let call = decisions
        .iter()
        .find(|decision| decision.call_id == "call-key")
        .unwrap_or_else(|| panic!("the Bash call is in the record: {decisions:?}"));
    assert_eq!(
        call.decision,
        CallDecision::Deny,
        "from_spec built a Codex executor whose policy allows only apply_patch under \
         {mode:?}, and a Bash call reading ~/.ssh was not denied: {call:?}"
    );
}

#[test]
fn adversary_a_codex_spec_narrowing_allow_under_frame_is_refused_or_enforced() {
    a_codex_policy_narrowed_under(DecisionMode::Frame, "codex-frame");
}

#[test]
fn adversary_a_codex_spec_narrowing_allow_under_observe_is_refused_or_enforced() {
    a_codex_policy_narrowed_under(DecisionMode::Observe, "codex-observe");
}

// ------------------------------------------------------------------ a comma in a tool name

/// A scripted Claude Code session that keeps the arguments it was started with and replies
/// naming the edit.
#[derive(Clone, Default)]
struct ClaudeSession {
    args: Rc<RefCell<Vec<Vec<String>>>>,
}

impl ProcessRunner for ClaudeSession {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        self.args.borrow_mut().push(plan.args.to_vec());
        let init = serde_json::json!({
            "type": "system", "subtype": "init", "claude_code_version": "2.1.304",
            "cwd": plan.cwd.display().to_string(), "tools": ToolPolicy::claude_default().allow,
            "mcp_servers": [], "plugins": [], "output_style": "default", "apiKeySource": "none",
            "model": "scripted", "permissionMode": "default", "session_id": "vendor-1",
        });
        let said = serde_json::json!({"action": EDIT, "summary": "nothing"}).to_string();
        let reply = serde_json::json!({
            "type": "assistant",
            "message": {"content": [{"type": "text", "text": said}],
                        "usage": {"input_tokens": 10, "output_tokens": 5}},
        });
        let result = serde_json::json!({
            "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
            "duration_ms": 20, "total_cost_usd": 0.0, "usage": {"input_tokens": 10, "output_tokens": 5},
        });
        ScriptedRunner::of_lines(
            [init.to_string(), reply.to_string(), result.to_string()],
            ScriptedLog::new(),
        )
        .start(plan)
    }
}

/// `story:session-executor-from-spec` acceptance: a tool in `deny` is absent from the session
/// request. A policy entry holding a comma (which `loom_sdk::agent::parse` accepts: it refuses
/// only empty or blank text) is joined unescaped into `--tools`, which Claude Code splits at
/// commas, so `Bash` is offered although the policy denies it.
#[test]
fn adversary_a_denied_tool_is_not_offered_through_a_comma_in_an_allowed_entry() {
    let fixture = Fixture::new("comma");
    let tools = ToolPolicy {
        allow: vec!["Read,Bash".to_owned()],
        deny: vec!["Bash".to_owned()],
        egress: Vec::new(),
        decisions: DecisionMode::Frame,
    };
    let script = ClaudeSession::default();
    let executor = fixture.prepare(
        SessionExecutor::from_spec(
            &spec(SessionKind::ClaudeCode, tools),
            fixture.workspace(),
            files(),
        )
        .expect("a Claude Code spec builds"),
        Box::new(script.clone()),
    );
    let step = step_of(&executor);
    let args = script.args.borrow().first().cloned().unwrap_or_else(|| {
        panic!(
            "the session started: {:?} ({:?})",
            step.outcome,
            executor.last_report().map(|report| report.error)
        )
    });
    let at = args
        .iter()
        .position(|arg| arg == "--tools")
        .unwrap_or_else(|| panic!("the request names its tools: {args:?}"));
    let offered: Vec<&str> = args[at + 1].split(',').collect();
    assert!(
        !offered.contains(&"Bash"),
        "Bash is denied by the policy and offered by the session request: {args:?}"
    );
}

// ------------------------------------------------------------------ b10x-harness under ask

/// A b10x-harness spec denying a tool under `decisions: ask` is what `SpecError::DenyUnenforced`
/// tells its author to write, and `from_spec` builds it. metaharness's b10x adapter delivers
/// `observe` only and refuses `tool.decide`, so such an executor's every session is refused
/// before it starts. `from_spec` should refuse the spec instead of building it.
#[test]
fn adversary_a_harness_spec_from_spec_builds_can_start_a_session() {
    let fixture = Fixture::new("harness-ask");
    let tools = ToolPolicy {
        allow: Vec::new(),
        deny: vec!["shell".to_owned()],
        egress: Vec::new(),
        decisions: DecisionMode::Ask,
    };
    let Ok(mut executor) = SessionExecutor::from_spec(
        &spec(SessionKind::Harness, tools),
        fixture.workspace(),
        files(),
    ) else {
        return;
    };
    executor.endpoint = Some(Endpoint {
        base_url: "http://127.0.0.1:9/v1".to_owned(),
        wire: "anthropic-messages".to_owned(),
        model: "scripted".to_owned(),
        token_file: fixture.root.join("no-token.json"),
        token_pointer: None,
    });
    let executor = fixture.prepare(executor, Box::new(ClaudeSession::default()));
    let step = step_of(&executor);
    let report = executor.last_report().expect("a report");
    assert!(
        report.started,
        "from_spec built a b10x-harness executor whose session never starts: {:?} ({:?})",
        report.error, step.outcome
    );
}
