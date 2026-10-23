//! Acceptance for `story:session-executor-from-spec`: `SessionExecutor::from_spec` builds the
//! session executor from an agent's data (its brief, session kind, model and `ToolPolicy`), and
//! the tools a session is offered are the policy's.
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
    commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::model::agent::{
    AgentId, AgentIdentity, AgentSpec, BreachStep, ConfinementMode, DecisionMode, RunBudget,
    SessionSpec, StopPolicy, ToolPolicy,
};
use b10x_loom_session_executor::model::models::{ModelRole, ModelsConfig, RoleModel};
use b10x_loom_session_executor::model::session::SessionKind;
use b10x_loom_session_executor::{
    CredentialSource, ProcessRunner, Proposal, SessionExecutor, ToolPolicyDefaults as _,
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
            "session-executor-from-spec-{}-{name}",
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

    fn files() -> Vec<String> {
        vec!["src/lib.rs".to_owned()]
    }

    /// `executor` on the operator's nothing, its copies under the fixture's scratch, its
    /// sessions the ones `script` plays.
    fn prepare(&self, executor: SessionExecutor, script: &Script) -> SessionExecutor {
        let mut executor = executor.with_runner(Box::new(script.clone()));
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

/// An agent spec running a Claude Code session under `tools`, its brief [`BRIEF`] and the model
/// serving its `agent` role `sonnet`.
fn spec(tools: ToolPolicy) -> AgentSpec {
    AgentSpec {
        format: "loom.agent/1".to_owned(),
        agent: AgentIdentity {
            id: AgentId("fixer".to_owned()),
            name: "Fixer".to_owned(),
            brief: BRIEF.to_owned(),
        },
        model: ModelsConfig {
            routes: "routes.yaml".to_owned(),
            roles: vec![
                RoleModel {
                    role: ModelRole::Classifier,
                    model: "haiku".to_owned(),
                },
                RoleModel {
                    role: ModelRole::Agent,
                    model: "sonnet".to_owned(),
                },
            ],
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

/// One session run by `executor`: the request it was started with and the proposal it made.
fn one_session(executor: &SessionExecutor, script: &Script) -> (Request, Proposal) {
    let step = executor.run_with_usage(&commission(), &frontier());
    let ExecutorOutcome::ProposedAction(_) = &step.outcome else {
        panic!(
            "a proposal: {:?} ({:?})",
            step.outcome,
            executor.last_report().map(|report| report.error)
        );
    };
    let proposal = Proposal::from_outcome(&step.outcome)
        .expect("a proposed action")
        .expect("its arguments");
    let requests = script.requests();
    assert_eq!(requests.len(), 1, "one session");
    (requests[0].clone(), proposal)
}

/// The tools a request offers the session: the value after `--tools`, split at commas.
fn offered(request: &Request) -> Vec<String> {
    let at = request
        .args
        .iter()
        .position(|arg| arg == "--tools")
        .unwrap_or_else(|| panic!("the request names the tools it offers: {:?}", request.args));
    request.args[at + 1]
        .split(',')
        .filter(|tool| !tool.is_empty())
        .map(str::to_owned)
        .collect()
}

#[test]
fn a_spec_with_the_default_policy_runs_as_new_does() {
    let fixture = Fixture::new("default");

    let built_script = Script::default();
    let built = fixture.prepare(
        SessionExecutor::from_spec(
            &spec(ToolPolicy::claude_default()),
            &fixture.workspace,
            Fixture::files(),
        )
        .expect("a spec with a session builds"),
        &built_script,
    );
    let (built_request, built_proposal) = one_session(&built, &built_script);

    let new_script = Script::default();
    let mut new = SessionExecutor::new(&fixture.workspace, Fixture::files(), BRIEF);
    new.model = Some("sonnet".to_owned());
    let new = fixture.prepare(new, &new_script);
    let (new_request, new_proposal) = one_session(&new, &new_script);

    assert_eq!(built_request, new_request, "the same session request");
    assert_eq!(built_proposal, new_proposal, "the same proposal");
    assert_eq!(offered(&built_request), ToolPolicy::claude_default().allow);
    assert_eq!(built.brief, BRIEF);
    assert_eq!(built.model.as_deref(), Some("sonnet"));

    let sessions = built.sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].agent, Some(AgentId("fixer".to_owned())));
    assert_eq!(new.sessions()[0].agent, None, "new runs under no spec");
}

#[test]
fn a_denied_tool_is_never_offered() {
    let fixture = Fixture::new("denied");
    let mut tools = ToolPolicy::claude_default();
    tools.allow.push("Bash".to_owned());
    tools.deny = vec!["Bash".to_owned(), "Write".to_owned()];
    let script = Script::default();
    let executor = fixture.prepare(
        SessionExecutor::from_spec(&spec(tools), &fixture.workspace, Fixture::files())
            .expect("a spec with a session builds"),
        &script,
    );
    let (request, _) = one_session(&executor, &script);
    let offered = offered(&request);
    assert_eq!(
        offered,
        ["Read", "Edit", "Glob", "Grep"],
        "{:?}",
        request.args
    );
    for denied in ["Bash", "Write"] {
        assert!(
            request
                .args
                .iter()
                .all(|arg| !arg.split(',').any(|tool| tool == denied)),
            "{denied} is in the request: {:?}",
            request.args
        );
    }
}

#[test]
fn a_spec_without_a_session_is_refused() {
    let mut without = spec(ToolPolicy::claude_default());
    without.session = None;
    let refused = SessionExecutor::from_spec(&without, "/nowhere", Fixture::files())
        .expect_err("no session to run");
    assert!(refused.to_string().contains("session"), "{refused}");
}

#[test]
fn the_default_policies_carry_the_tools_and_ask() {
    let claude = ToolPolicy::claude_default();
    assert_eq!(claude.allow, ["Read", "Edit", "Write", "Glob", "Grep"]);
    assert_eq!(claude.decisions, DecisionMode::Ask);
    assert!(claude.deny.is_empty() && claude.egress.is_empty());
    let codex = ToolPolicy::codex_default();
    assert_eq!(codex.allow, ["Bash", "apply_patch"]);
    assert_eq!(codex.decisions, DecisionMode::Ask);
    assert!(codex.deny.is_empty() && codex.egress.is_empty());
}
