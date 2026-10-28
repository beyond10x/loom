//! Adversary a2, wave 2026-10-28-w1, against `story:session-executor-partial-runspec`.
//!
//! AGENTS.md § Session executor: "a context path that is not one of the copied files as one plain
//! relative path suspends the step with `ExternalAvailability` before any copy". A file the
//! executor names but does not copy (one reached through a link in the workspace, or one absent
//! from it) is not one of the copied files, yet `SessionOverrides::uncopied` checks only that the
//! path is plain and named in `files`. Whether a copy was made is read off the session report's
//! `cwd`, which the executor sets right after the copy: the unit's own probe (an empty scratch
//! directory after the step) cannot tell, because the copy is removed when it drops.
//!
//! Every session is scripted (metaharness's `ScriptedRunner`): no harness program runs, no model
//! is called and nothing reaches the network.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorStep, Frontier, FrontierAction, FrontierData,
    FrontierId, PrincipalId, SuspensionReason, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptStep, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::model::agent::{
    AgentId, AgentIdentity, AgentSpec, BreachStep, ConfinementMode, RunBudget, SessionSpec,
    StopPolicy, ToolPolicy,
};
use b10x_loom_session_executor::model::models::{ModelRole, ModelsConfig, RoleModel};
use b10x_loom_session_executor::model::session::SessionKind;
use b10x_loom_session_executor::{
    CredentialSource, Endpoint, Kind, ProcessRunner, SessionExecutor, SessionOverrides, SpecError,
    ToolPolicyDefaults as _,
};

const EDIT: &str = "edit";
const BRIEF: &str = "Fix add so that it adds.";
const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
const NOTES: &str = "The notes the session is handed: add adds.\n";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn git(dir: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A git workspace holding `src/lib.rs`, `src/notes.md`, a link `src/link.md -> notes.md` and a
/// linked directory `docs -> src`, all committed; a scratch directory and a token file beside it.
struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "adversary-w20261028w1-a2-{}-{name}",
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
        std::fs::write(workspace.join("src/notes.md"), NOTES).expect("a fixture file");
        std::os::unix::fs::symlink("notes.md", workspace.join("src/link.md")).expect("a link");
        std::os::unix::fs::symlink("src", workspace.join("docs")).expect("a linked directory");
        git(&workspace, &["init", "-q"]);
        git(
            &workspace,
            &["add", "src/lib.rs", "src/notes.md", "src/link.md", "docs"],
        );
        git(&workspace, &["commit", "-q", "-m", "fixture"]);
        std::fs::create_dir_all(root.join("scratch")).expect("the scratch directory");
        std::fs::write(root.join("token.json"), "{\"token\": \"scripted\"}\n").expect("a token");
        Self { root, workspace }
    }

    fn scratch(&self) -> PathBuf {
        self.root.join("scratch")
    }

    fn executor(&self, kind: Kind, files: &[&str], recorder: &Recorder) -> SessionExecutor {
        let files: Vec<String> = files.iter().map(|file| (*file).to_owned()).collect();
        let mut executor = SessionExecutor::new(&self.workspace, files, BRIEF)
            .of_kind(kind)
            .with_runner(Box::new(recorder.clone()));
        executor.credentials = CredentialSource::None;
        executor.scratch = self.scratch();
        if kind == Kind::B10x {
            executor.endpoint = Some(Endpoint {
                base_url: "http://127.0.0.1:9/v1".to_owned(),
                wire: "openai-responses".to_owned(),
                model: "synthetic".to_owned(),
                token_file: self.root.join("token.json"),
                token_pointer: Some("/token".to_owned()),
            });
        }
        executor
    }

    fn scratch_is_empty(&self) -> bool {
        std::fs::read_dir(self.scratch())
            .expect("the scratch directory")
            .next()
            .is_none()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug, Clone)]
struct Request {
    args: Vec<String>,
    codex_config: Option<String>,
}

#[derive(Clone)]
struct Recorder {
    kind: Kind,
    requests: Rc<RefCell<Vec<Request>>>,
}

impl Recorder {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            requests: Rc::default(),
        }
    }
}

fn action_line() -> String {
    serde_json::json!({"action": EDIT, "summary": "fixed add"}).to_string()
}

fn claude_lines(cwd: &Path) -> Vec<String> {
    let init = serde_json::json!({
        "type": "system", "subtype": "init", "claude_code_version": "2.1.304",
        "cwd": cwd.display().to_string(), "tools": ToolPolicy::claude_default().allow,
        "mcp_servers": [], "plugins": [], "output_style": "default", "apiKeySource": "none",
        "model": "scripted", "permissionMode": "default", "session_id": "vendor-1",
    });
    let reply = serde_json::json!({
        "type": "assistant",
        "message": {"content": [{"type": "text", "text": action_line()}],
                    "usage": {"input_tokens": 1000, "output_tokens": 50}},
    });
    let result = serde_json::json!({
        "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
        "duration_ms": 2000, "total_cost_usd": 0.0003,
        "usage": {"input_tokens": 1000, "output_tokens": 50},
    });
    vec![init.to_string(), reply.to_string(), result.to_string()]
}

fn codex_lines(cwd: &Path) -> Vec<String> {
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
    let complete = serde_json::json!({
        "timestamp": "2026-08-22T10:00:05.000Z", "type": "event_msg",
        "payload": {"type": "task_complete", "turn_id": "t1", "duration_ms": 4200,
                    "error": null, "last_agent_message": action_line()},
    });
    vec![meta.to_string(), started.to_string(), complete.to_string()]
}

fn b10x_lines() -> Vec<String> {
    [
        serde_json::json!({"kind": "started", "model": "synthetic", "published_tools": [],
                           "credential_source": "none"}),
        serde_json::json!({"kind": "text-delta", "text": action_line()}),
        serde_json::json!({"kind": "usage", "model": "synthetic", "input_tokens": 100,
                           "output_tokens": 20, "cached_input_tokens": 0}),
        serde_json::json!({"kind": "finished", "stop": {"kind": "completed"}}),
    ]
    .iter()
    .map(ToString::to_string)
    .collect()
}

impl ProcessRunner for Recorder {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        let codex_config = plan
            .env
            .get("CODEX_HOME")
            .and_then(|home| std::fs::read_to_string(Path::new(home).join("config.toml")).ok());
        self.requests.borrow_mut().push(Request {
            args: plan.args.to_vec(),
            codex_config,
        });
        std::fs::write(plan.cwd.join("src/lib.rs"), FIXED)?;
        let lines = match self.kind {
            Kind::Claude => claude_lines(plan.cwd),
            Kind::Codex => codex_lines(plan.cwd),
            Kind::B10x => b10x_lines(),
        };
        let steps: Vec<ScriptStep> = lines.into_iter().map(ScriptStep::line).collect();
        ScriptedRunner::new(steps, ScriptedLog::new()).start(plan)
    }
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: CaseId("case-a2".to_owned()),
        principal: PrincipalId("principal-a2".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn frontier() -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf1)),
        case_id: CaseId("case-a2".to_owned()),
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

fn step(executor: &SessionExecutor) -> ExecutorStep {
    executor.run_with_usage(&commission(), &frontier())
}

fn context(paths: &[&str]) -> SessionOverrides {
    SessionOverrides {
        context: paths.iter().map(|path| (*path).to_owned()).collect(),
        ..SessionOverrides::default()
    }
}

/// The refusal text of a step suspended with `ExternalAvailability`, and the copy the session
/// report says was made, if any.
fn suspended(executor: &SessionExecutor, what: &str) -> (String, Option<PathBuf>) {
    let step = step(executor);
    let ExecutorOutcome::Suspended(suspended) = &step.outcome else {
        panic!("{what}: a suspension: {:?}", step.outcome);
    };
    let SuspensionReason::ExternalAvailability(Value::Text(why)) = &suspended.reason else {
        panic!("{what}: ExternalAvailability: {:?}", suspended.reason);
    };
    let cwd = executor.last_report().and_then(|report| report.cwd);
    (why.clone(), cwd)
}

/// AGENTS.md § Session executor: a context path that is not one of the copied files suspends the
/// step before any copy, with Loom's own refusal naming it. A file named in `files` that the copy
/// does not hold (reached through a link, or absent from the workspace) is such a path.
#[test]
fn a_named_file_the_copy_does_not_hold_is_refused_as_context_before_the_copy() {
    let fixture = Fixture::new("uncopied");
    let mut after_the_copy = Vec::new();
    for (path, why_uncopied) in [
        ("src/link.md", "a link in the workspace"),
        ("docs/notes.md", "under a linked directory"),
        ("src/absent.md", "absent from the workspace"),
    ] {
        for kind in [Kind::Claude, Kind::Codex, Kind::B10x] {
            let what = format!("{kind:?} {path} ({why_uncopied})");
            let recorder = Recorder::new(kind);
            let executor = fixture
                .executor(kind, &["src/lib.rs", "src/notes.md", path], &recorder)
                .with_overrides(context(&[path]));
            let (why, copy) = suspended(&executor, &what);
            assert!(recorder.requests.borrow().is_empty(), "{what}: not started");
            let loom_refused = why.contains("is not one of the files the session's copy holds");
            if !loom_refused || copy.is_some() {
                after_the_copy.push(format!(
                    "{what}: copy made: {}; refusal: {why}",
                    copy.is_some()
                ));
            }
        }
    }
    assert!(
        after_the_copy.is_empty(),
        "refused after the copy, not by Loom before it:\n{}",
        after_the_copy.join("\n")
    );
}

/// The unit's "no copy is made" probe reads the scratch directory after the step. A refusal after
/// the copy (here metaharness's strict-version refusal) leaves the report naming the copy it made,
/// and the scratch directory empty, because the copy is removed when it drops: the probe holds
/// whether or not a copy was made. Green: it measures the probe, not the code.
#[test]
fn the_scratch_probe_reads_empty_after_a_copy_was_made() {
    let fixture = Fixture::new("probe");
    let recorder = Recorder::new(Kind::Claude);
    let executor = fixture
        .executor(Kind::Claude, &["src/lib.rs", "src/notes.md"], &recorder)
        .with_overrides(SessionOverrides {
            strict_version: Some(true),
            ..SessionOverrides::default()
        });
    let (why, copy) = suspended(&executor, "strict");
    assert!(why.contains("--strict-version"), "{why}");
    assert!(copy.is_some(), "a copy was made before the refusal");
    assert!(
        fixture.scratch_is_empty(),
        "yet the scratch directory reads empty"
    );
}

/// The context paths `plain_relative` must refuse, before the copy, in spellings the unit's test
/// does not name.
#[test]
fn unplain_context_spellings_are_refused_before_the_copy() {
    let fixture = Fixture::new("unplain");
    for path in [
        ".git/HEAD",
        "src/.git",
        "src/notes.md/",
        "src/../src/notes.md",
        "src/notes.md\0",
    ] {
        let recorder = Recorder::new(Kind::Claude);
        let executor = fixture
            .executor(Kind::Claude, &["src/lib.rs", "src/notes.md"], &recorder)
            .with_overrides(context(&[path]));
        let (why, copy) = suspended(&executor, path);
        assert!(why.contains(&format!("{path:?}")), "{path:?}: {why}");
        assert_eq!(copy, None, "{path:?}");
    }
}

fn claude_args(fixture: &Fixture, overrides: SessionOverrides) -> Vec<String> {
    let recorder = Recorder::new(Kind::Claude);
    let executor = fixture
        .executor(Kind::Claude, &["src/lib.rs", "src/notes.md"], &recorder)
        .with_overrides(overrides);
    let step = step(&executor);
    assert!(
        matches!(step.outcome, ExecutorOutcome::ProposedAction(_)),
        "{:?}",
        step.outcome
    );
    recorder.requests.borrow()[0].args.clone()
}

fn after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let at = args.iter().position(|arg| arg == flag)?;
    args.get(at + 1).map(String::as_str)
}

/// The micro-USD budget renders exactly at its bounds, and from a spec its i64 bounds are taken
/// or refused.
#[test]
fn the_budget_renders_exactly_at_its_bounds() {
    let fixture = Fixture::new("budget");
    for (micro_usd, usd) in [
        (0, "0"),
        (999_999, "0.999999"),
        (1_000_000, "1"),
        (1_000_001, "1.000001"),
        (10_000_000, "10"),
        (u64::MAX, "18446744073709.551615"),
    ] {
        let args = claude_args(
            &fixture,
            SessionOverrides {
                max_budget_micro_usd: Some(micro_usd),
                ..SessionOverrides::default()
            },
        );
        assert_eq!(after(&args, "--max-budget-usd"), Some(usd), "{micro_usd}");
    }

    let build = |budget: i64| {
        SessionExecutor::from_spec(
            &agent_spec(|session| session.max_budget_micro_usd = Some(budget)),
            &fixture.workspace,
            vec!["src/lib.rs".to_owned()],
        )
    };
    let top = build(i64::MAX).expect("i64::MAX builds");
    assert_eq!(top.overrides.max_budget_micro_usd, Some(i64::MAX as u64));
    assert_eq!(
        build(i64::MIN).expect_err("i64::MIN is refused"),
        SpecError::BudgetNegative {
            agent: "fixer".to_owned(),
            budget: i64::MIN,
        }
    );
}

/// An effort carrying TOML syntax stays one quoted value in Codex's scratch config, and an effort
/// spelling Claude Code's seam-deleting flag is refused before the session starts.
#[test]
fn an_effort_cannot_inject_config_or_flags() {
    let fixture = Fixture::new("effort");
    let hostile = "high\"\n[mcp_servers.evil]\ncommand = \"sh\"\n#";
    let recorder = Recorder::new(Kind::Codex);
    let executor = fixture
        .executor(Kind::Codex, &["src/lib.rs", "src/notes.md"], &recorder)
        .with_overrides(SessionOverrides {
            effort: Some(hostile.to_owned()),
            ..SessionOverrides::default()
        });
    let step = step(&executor);
    assert!(
        matches!(step.outcome, ExecutorOutcome::ProposedAction(_)),
        "{:?}",
        step.outcome
    );
    let config = recorder.requests.borrow()[0]
        .codex_config
        .clone()
        .expect("Codex's scratch config");
    assert!(
        !config
            .lines()
            .any(|line| line.trim_start().starts_with("[mcp_servers.evil]")),
        "{config}"
    );
    assert_eq!(
        config
            .lines()
            .filter(|line| line.starts_with("model_reasoning_effort = "))
            .count(),
        1,
        "{config}"
    );

    // An effort spelling the flag that deletes the control seam: metaharness's H8 refuses the
    // argv that carries it, even as the value of `--effort`, before the runner sees the plan.
    let recorder = Recorder::new(Kind::Claude);
    let executor = fixture
        .executor(Kind::Claude, &["src/lib.rs", "src/notes.md"], &recorder)
        .with_overrides(SessionOverrides {
            effort: Some("--dangerously-skip-permissions".to_owned()),
            ..SessionOverrides::default()
        });
    let (why, _) = suspended(&executor, "a flag as effort");
    assert!(why.contains("H8"), "{why}");
    assert!(recorder.requests.borrow().is_empty(), "not started");
}

fn agent_spec(session: impl FnOnce(&mut SessionSpec)) -> AgentSpec {
    let mut spec = SessionSpec {
        kind: SessionKind::ClaudeCode,
        budget: RunBudget {
            max_steps: None,
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
        tools: ToolPolicy::claude_default(),
        effort: None,
        max_budget_micro_usd: None,
        strict_version: None,
        context: Vec::new(),
    };
    session(&mut spec);
    AgentSpec {
        format: "loom.agent/1".to_owned(),
        agent: AgentIdentity {
            id: AgentId("fixer".to_owned()),
            name: "Fixer".to_owned(),
            brief: BRIEF.to_owned(),
        },
        model: ModelsConfig {
            routes: "routes.yaml".to_owned(),
            roles: vec![RoleModel {
                role: ModelRole::Agent,
                model: "sonnet".to_owned(),
            }],
        },
        session: Some(spec),
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
