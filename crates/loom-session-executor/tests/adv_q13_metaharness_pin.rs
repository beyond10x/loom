//! Adversary cases for `story:metaharness-0-19-pin`: under metaharness 0.19.0 the native Claude
//! surface plans `--tools <whole pinned set>` (amendment a53), and the executor's `Restricted`
//! runner must leave the child exactly one `--tools`, the offered list, last, for every policy:
//! the default one, one offering nothing, and one offering a tool outside the file tools.
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
    CommissionId, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::model::agent::{
    AgentId, AgentIdentity, AgentSpec, BreachStep, ConfinementMode, RunBudget, SessionSpec,
    StopPolicy, ToolPolicy,
};
use b10x_loom_session_executor::model::models::{ModelRole, ModelsConfig, RoleModel};
use b10x_loom_session_executor::model::session::SessionKind;
use b10x_loom_session_executor::{
    CredentialSource, ProcessRunner, SessionExecutor, ToolPolicyDefaults as _,
};

const EDIT: &str = "implementation.edit";
const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";

/// Tools in metaharness 0.19.0's pinned native list (`metaharness_claude`'s
/// `NATIVE_2_1_304_2_1_305_AND_2_1_309`) that no policy below offers.
const NATIVE_ONLY: [&str; 11] = [
    "Task",
    "CronCreate",
    "CronDelete",
    "CronList",
    "EnterWorktree",
    "ExitWorktree",
    "ListAgents",
    "NotebookEdit",
    "ReportFindings",
    "ScheduleWakeup",
    "SendMessage",
];

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "session-executor-adv-q13-{}-{name}",
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
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[derive(Clone, Default)]
struct Script {
    args: Rc<RefCell<Vec<Vec<String>>>>,
}

impl ProcessRunner for Script {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        self.args.borrow_mut().push(plan.args.to_vec());
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

fn spec(tools: ToolPolicy) -> AgentSpec {
    AgentSpec {
        format: "loom.agent/1".to_owned(),
        agent: AgentIdentity {
            id: AgentId("fixer".to_owned()),
            name: "Fixer".to_owned(),
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
            kind: SessionKind::ClaudeCode,
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

/// The argv the child is started with under `tools`.
fn child_args(name: &str, tools: ToolPolicy) -> Vec<String> {
    let fixture = Fixture::new(name);
    let script = Script::default();
    let mut executor = SessionExecutor::from_spec(
        &spec(tools),
        &fixture.workspace,
        vec!["src/lib.rs".to_owned()],
    )
    .expect("a spec with a session builds")
    .with_runner(Box::new(script.clone()));
    executor.credentials = CredentialSource::None;
    executor.scratch = fixture.root.join("scratch");
    let step = executor.run_with_usage(&commission(), &frontier());
    let args = script.args.borrow().first().cloned();
    args.unwrap_or_else(|| {
        panic!(
            "the session started: {:?} ({:?})",
            step.outcome,
            executor.last_report().map(|report| report.error)
        )
    })
}

/// One `--tools`, last, naming exactly `offered`, after `--restricted`; no native-only tool
/// anywhere in the argv, and no `--allowedTools` grant.
fn assert_offers_exactly(args: &[String], offered: &str) {
    let tools: Vec<usize> = args
        .iter()
        .enumerate()
        .filter(|(_, arg)| arg.as_str() == "--tools" || arg.starts_with("--tools="))
        .map(|(at, _)| at)
        .collect();
    assert_eq!(tools.len(), 1, "one --tools: {args:?}");
    let n = args.len();
    assert_eq!(tools[0], n - 2, "--tools is the last pair: {args:?}");
    assert_eq!(args[n - 1], offered, "the offered list: {args:?}");
    assert_eq!(args[n - 3], "--restricted", "{args:?}");
    assert!(
        !args
            .iter()
            .any(|arg| arg == "--allowedTools" || arg.starts_with("--allowedTools=")),
        "no --allowedTools grant: {args:?}"
    );
    for tool in NATIVE_ONLY {
        assert!(
            args.iter()
                .all(|arg| !arg.split(',').any(|item| item == tool)),
            "{tool}, in metaharness's native list and not offered, reaches the child: {args:?}"
        );
    }
}

#[test]
fn adv_q13_default_policy_child_sees_only_the_offered_tools() {
    let args = child_args("default", ToolPolicy::claude_default());
    assert_offers_exactly(&args, "Read,Edit,Write,Glob,Grep");
}

#[test]
fn adv_q13_nothing_offered_child_sees_empty_tools_only() {
    let mut tools = ToolPolicy::claude_default();
    tools.deny = tools.allow.clone();
    let args = child_args("none", tools);
    assert_offers_exactly(&args, "");
}

#[test]
fn adv_q13_bash_offered_child_sees_policy_list_not_native_list() {
    let mut tools = ToolPolicy::claude_default();
    tools.allow.push("Bash".to_owned());
    tools.deny = vec!["Write".to_owned()];
    let args = child_args("bash", tools);
    assert_offers_exactly(&args, "Read,Edit,Glob,Grep,Bash");
}
