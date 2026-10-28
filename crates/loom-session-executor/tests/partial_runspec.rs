//! Acceptance for `story:session-executor-partial-runspec`: a caller names a partial run
//! (`SessionOverrides`: effort, a budget in micro-USD, strict version, context files) and each
//! member it names wins over its kind's default, each it leaves unset keeps today's value, and
//! nothing Loom sets for its own guarantees is taken from it. A member the kind's launcher does not
//! apply, and a context file the copy does not hold, are refused before the copy is made.
//!
//! The runner sees the request metaharness built from the `RunSpec` (program, arguments,
//! environment, and Codex's scratch config), not the `RunSpec` itself, so each member is read off
//! its effect there. Every session is scripted (metaharness's `ScriptedRunner`): no harness
//! program runs, no model is called and nothing reaches the network.

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

const EDIT: &str = "implementation.edit";
const BRIEF: &str = "fix add";
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

/// A git workspace of two tracked files, `src/lib.rs` and `src/notes.md`, a scratch directory
/// for the copies and a token file, under the system's temporary directory; removed when
/// dropped. Not under the repository: metaharness refuses a Claude session whose copy has memory
/// files above it. A context file must be tracked, so the workspace is a repository.
struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "session-executor-partial-runspec-{}-{name}",
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
        git(&workspace, &["init", "-q"]);
        git(&workspace, &["add", "src/lib.rs", "src/notes.md"]);
        git(&workspace, &["commit", "-q", "-m", "fixture"]);
        std::fs::create_dir_all(root.join("scratch")).expect("the scratch directory");
        std::fs::write(root.join("token.json"), "{\"token\": \"scripted\"}\n").expect("a token");
        Self { root, workspace }
    }

    fn files() -> Vec<String> {
        vec!["src/lib.rs".to_owned(), "src/notes.md".to_owned()]
    }

    fn scratch(&self) -> PathBuf {
        self.root.join("scratch")
    }

    /// An executor of `kind` over the workspace, on no credential, its copies under the
    /// fixture's scratch, its session the one `recorder` plays; b10x-harness pointed at a closed
    /// local port with the fixture's token file.
    fn executor(&self, kind: Kind, recorder: &Recorder) -> SessionExecutor {
        let mut executor = SessionExecutor::new(&self.workspace, Self::files(), BRIEF)
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

    /// Whether the scratch directory is still empty: no copy was made in it.
    fn no_copy_made(&self) -> bool {
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

/// What one session was started with, as metaharness handed it to the runner: the program, its
/// arguments, its environment and, for Codex, the scratch `config.toml` it reads, each path under
/// the session's own directories written as `<run>`, `<cwd>` or `<root>`, so two sessions'
/// requests compare. `cwd` is the working directory as written, before that rewriting.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Request {
    program: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
    codex_config: Option<String>,
    cwd: PathBuf,
}

/// A scripted session of `kind` that keeps the request it was started with, fixes `add` in its
/// copy and replies naming the edit, in its kind's own lines.
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

    fn requests(&self) -> Vec<Request> {
        self.requests.borrow().clone()
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
        let run = plan.transcript.parent().map(Path::to_path_buf);
        let root = plan.cwd.parent().map(Path::to_path_buf);
        let at = |text: &str| {
            let mut text = text.to_owned();
            if let Some(run) = &run {
                text = text.replace(&run.display().to_string(), "<run>");
            }
            text = text.replace(&plan.cwd.display().to_string(), "<cwd>");
            if let Some(root) = &root {
                text = text.replace(&root.display().to_string(), "<root>");
            }
            text
        };
        let codex_config = plan
            .env
            .get("CODEX_HOME")
            .and_then(|home| std::fs::read_to_string(Path::new(home).join("config.toml")).ok())
            .map(|config| at(&config));
        self.requests.borrow_mut().push(Request {
            program: at(plan.program),
            args: plan.args.iter().map(|arg| at(arg)).collect(),
            env: plan
                .env
                .iter()
                .map(|(name, value)| (name.clone(), at(value)))
                .collect(),
            codex_config,
            cwd: plan.cwd.to_path_buf(),
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
        case_id: CaseId("case-partial".to_owned()),
        principal: PrincipalId("principal-partial".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn frontier() -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf1)),
        case_id: CaseId("case-partial".to_owned()),
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

/// The one request a session of `kind` under `overrides` was started with; it must propose.
fn request(fixture: &Fixture, kind: Kind, overrides: Option<SessionOverrides>) -> Request {
    let recorder = Recorder::new(kind);
    let mut executor = fixture.executor(kind, &recorder);
    if let Some(overrides) = overrides {
        executor = executor.with_overrides(overrides);
    }
    let step = step(&executor);
    let ExecutorOutcome::ProposedAction(_) = &step.outcome else {
        panic!(
            "{kind:?}: a proposal: {:?} ({:?})",
            step.outcome,
            executor
                .last_report()
                .map(|report| (report.error, report.rows))
        );
    };
    let requests = recorder.requests();
    assert_eq!(requests.len(), 1, "{kind:?}: one session");
    requests[0].clone()
}

/// Why a session of `kind` under `overrides` was suspended with `ExternalAvailability`, the
/// requests its runner saw, and whether the scratch directory stayed empty.
fn refused(fixture: &Fixture, kind: Kind, overrides: SessionOverrides) -> (String, usize, bool) {
    let recorder = Recorder::new(kind);
    let executor = fixture.executor(kind, &recorder).with_overrides(overrides);
    let step = step(&executor);
    let ExecutorOutcome::Suspended(suspended) = &step.outcome else {
        panic!("{kind:?}: a suspension: {:?}", step.outcome);
    };
    let SuspensionReason::ExternalAvailability(Value::Text(why)) = &suspended.reason else {
        panic!("{kind:?}: ExternalAvailability: {:?}", suspended.reason);
    };
    assert!(executor.sessions().is_empty(), "{kind:?}: nothing recorded");
    (
        why.clone(),
        recorder.requests().len(),
        fixture.no_copy_made(),
    )
}

/// `args` with each `flag value` pair named removed, so a request with a member named compares
/// with one without it.
fn without(args: &[String], pairs: &[(&str, &str)]) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < args.len() {
        if pairs.iter().any(|(flag, value)| {
            args[at] == *flag && args.get(at + 1).map(String::as_str) == Some(*value)
        }) {
            at += 2;
        } else {
            out.push(args[at].clone());
            at += 1;
        }
    }
    out
}

/// The value after `flag`, where the request names it.
fn after<'a>(request: &'a Request, flag: &str) -> Option<&'a str> {
    let at = request.args.iter().position(|arg| arg == flag)?;
    request.args.get(at + 1).map(String::as_str)
}

fn effort(level: &str) -> SessionOverrides {
    SessionOverrides {
        effort: Some(level.to_owned()),
        ..SessionOverrides::default()
    }
}

fn budget(micro_usd: u64) -> SessionOverrides {
    SessionOverrides {
        max_budget_micro_usd: Some(micro_usd),
        ..SessionOverrides::default()
    }
}

fn strict(strict: bool) -> SessionOverrides {
    SessionOverrides {
        strict_version: Some(strict),
        ..SessionOverrides::default()
    }
}

fn context(paths: &[&str]) -> SessionOverrides {
    SessionOverrides {
        context: paths.iter().map(|path| (*path).to_owned()).collect(),
        ..SessionOverrides::default()
    }
}

/// For Claude Code, Codex and b10x-harness: a caller naming no member gets the request a caller
/// of today's executor gets, member for member; each member it names reaches metaharness and
/// changes that member's effect only (effort, budget, strict version, context), the rest of the
/// request unchanged.
#[test]
fn a_partial_runspec_overrides_the_kind_defaults() {
    let fixture = Fixture::new("overrides");
    for kind in [Kind::Claude, Kind::Codex, Kind::B10x] {
        let today = request(&fixture, kind, None);
        let none = request(&fixture, kind, Some(SessionOverrides::default()));
        assert_eq!(
            none,
            Request {
                cwd: none.cwd.clone(),
                ..today.clone()
            },
            "{kind:?}"
        );
        assert_eq!(after(&today, "--effort"), None, "{kind:?}");
        assert_eq!(after(&today, "--max-budget-usd"), None, "{kind:?}");
        assert_eq!(after(&today, "--context"), None, "{kind:?}");
        assert!(
            !today.args.iter().any(|arg| arg.contains(NOTES.trim_end())),
            "{kind:?}: no context is handed by default"
        );
        let lenient = request(&fixture, kind, Some(strict(false)));
        assert_eq!(
            lenient,
            Request {
                cwd: lenient.cwd.clone(),
                ..today.clone()
            },
            "{kind:?}: strict version false is the default"
        );

        // Strict version: a scripted runner launches no executable, so it reports no version,
        // and metaharness refuses a strict run off its pin before the runner sees the plan.
        let (why, started, _) = refused(&fixture, kind, strict(true));
        assert!(why.contains("--strict-version"), "{kind:?}: {why}");
        assert_eq!(
            started, 0,
            "{kind:?}: the strict run is refused before it starts"
        );

        // Context: each named file reaches the request, handed from the copy.
        let handed = request(&fixture, kind, Some(context(&["src/notes.md"])));
        match kind {
            Kind::Claude | Kind::Codex => {
                let carrying: Vec<&String> = handed
                    .args
                    .iter()
                    .filter(|arg| arg.contains(NOTES.trim_end()))
                    .collect();
                assert_eq!(carrying.len(), 1, "{kind:?}: {:?}", handed.args);
                assert!(
                    carrying[0].contains("src/notes.md"),
                    "{kind:?}: {carrying:?}"
                );
            }
            Kind::B10x => {
                let path = after(&handed, "--context").expect("b10x-harness gets --context");
                assert!(path.ends_with("src/notes.md"), "{path}");
            }
        }
        assert_eq!(handed.env, today.env, "{kind:?}");
    }

    // Effort: Claude Code takes `--effort`, Codex `model_reasoning_effort` in its scratch config.
    let today = request(&fixture, Kind::Claude, None);
    let asked = request(&fixture, Kind::Claude, Some(effort("medium")));
    assert_eq!(after(&asked, "--effort"), Some("medium"));
    assert_eq!(without(&asked.args, &[("--effort", "medium")]), today.args);
    assert_eq!(asked.env, today.env);

    let today = request(&fixture, Kind::Codex, None);
    let asked = request(&fixture, Kind::Codex, Some(effort("medium")));
    let config = asked
        .codex_config
        .as_deref()
        .expect("Codex's scratch config");
    assert!(
        config.contains("model_reasoning_effort = \"medium\""),
        "{config}"
    );
    assert!(
        !today
            .codex_config
            .as_deref()
            .expect("Codex's scratch config")
            .contains("model_reasoning_effort"),
        "no effort by default"
    );
    assert_eq!(asked.args, today.args);
    assert_eq!(asked.env, today.env);

    // Budget: Claude Code takes `--max-budget-usd` in dollars, rendered from micro-USD exactly.
    let today = request(&fixture, Kind::Claude, None);
    for (micro_usd, usd) in [
        (12_500_000, "12.5"),
        (1, "0.000001"),
        (5_000_000, "5"),
        (1_234_567, "1.234567"),
        (100_000, "0.1"),
    ] {
        let capped = request(&fixture, Kind::Claude, Some(budget(micro_usd)));
        assert_eq!(after(&capped, "--max-budget-usd"), Some(usd), "{micro_usd}");
        assert_eq!(
            without(&capped.args, &[("--max-budget-usd", usd)]),
            today.args
        );
    }

    // All four at once on Claude Code (strict version named false, so the run starts).
    let all = request(
        &fixture,
        Kind::Claude,
        Some(SessionOverrides {
            effort: Some("low".to_owned()),
            max_budget_micro_usd: Some(2_000_000),
            strict_version: Some(false),
            context: vec!["src/notes.md".to_owned()],
        }),
    );
    assert_eq!(after(&all, "--effort"), Some("low"));
    assert_eq!(after(&all, "--max-budget-usd"), Some("2"));
    assert!(all.args.iter().any(|arg| arg.contains(NOTES.trim_end())));
}

/// Whatever a caller names, the request carries the executor's own credentials, working
/// directory (the scratch copy, under the executor's scratch), hermetic mode and decision mode:
/// the partial run has no member for them, and with every member it has named the request
/// differs from today's only by those members' flags, and the hermetic verdict is the same.
#[test]
fn a_partial_runspec_cannot_override_what_loom_guarantees() {
    let fixture = Fixture::new("guarantees");
    for (kind, overrides, flags) in [
        (
            Kind::Claude,
            SessionOverrides {
                effort: Some("high".to_owned()),
                max_budget_micro_usd: Some(3_000_000),
                strict_version: Some(false),
                context: Vec::new(),
            },
            vec![("--effort", "high"), ("--max-budget-usd", "3")],
        ),
        (
            Kind::Codex,
            SessionOverrides {
                effort: Some("high".to_owned()),
                strict_version: Some(false),
                ..SessionOverrides::default()
            },
            Vec::<(&str, &str)>::new(),
        ),
        (
            Kind::B10x,
            SessionOverrides {
                strict_version: Some(false),
                ..SessionOverrides::default()
            },
            Vec::<(&str, &str)>::new(),
        ),
    ] {
        let today_recorder = Recorder::new(kind);
        let today_executor = fixture.executor(kind, &today_recorder);
        let today_step = step(&today_executor);
        let named_recorder = Recorder::new(kind);
        let named_executor = fixture
            .executor(kind, &named_recorder)
            .with_overrides(overrides);
        let named_step = step(&named_executor);
        assert!(
            matches!(named_step.outcome, ExecutorOutcome::ProposedAction(_)),
            "{kind:?}: {:?}",
            named_step.outcome
        );
        assert_eq!(
            today_step.outcome, named_step.outcome,
            "{kind:?}: the same proposal"
        );

        let today = today_recorder.requests().remove(0);
        let named = named_recorder.requests().remove(0);
        assert_eq!(named.program, today.program, "{kind:?}");
        assert_eq!(named.env, today.env, "{kind:?}: the executor's credentials");
        for (flag, value) in &flags {
            assert_eq!(
                after(&named, flag),
                Some(*value),
                "{kind:?}: the member is applied"
            );
        }
        if kind == Kind::Codex {
            assert!(
                named
                    .codex_config
                    .as_deref()
                    .is_some_and(|config| config.contains("model_reasoning_effort = \"high\"")),
                "the member is applied"
            );
        }
        assert_eq!(without(&named.args, &flags), today.args, "{kind:?}");
        assert!(
            named
                .cwd
                .starts_with(fixture.scratch().canonicalize().expect("scratch")),
            "{kind:?}: the working directory is the executor's copy: {}",
            named.cwd.display()
        );
        let today_report = today_executor.last_report().expect("a report");
        let named_report = named_executor.last_report().expect("a report");
        assert_eq!(
            named_report.rows, today_report.rows,
            "{kind:?}: the hermetic verdict is the executor's"
        );
    }
    let request = request(&fixture, Kind::Claude, Some(effort("high")));
    assert!(
        request
            .env
            .iter()
            .all(|(name, _)| name != "ANTHROPIC_API_KEY"),
        "the executor's credential source (none) stands"
    );
}

/// A member the kind's launcher does not apply is refused before the copy, naming the member and
/// the kind: a budget on Codex and on b10x-harness, an effort on b10x-harness.
#[test]
fn a_member_the_kind_does_not_apply_is_refused_before_the_copy() {
    let fixture = Fixture::new("unapplied");
    for (kind, overrides, member, named) in [
        (
            Kind::Codex,
            budget(1_000_000),
            "max_budget_micro_usd",
            "Codex",
        ),
        (
            Kind::B10x,
            budget(1_000_000),
            "max_budget_micro_usd",
            "b10x-harness",
        ),
        (Kind::B10x, effort("medium"), "effort", "b10x-harness"),
    ] {
        let (why, started, no_copy) = refused(&fixture, kind, overrides);
        assert!(why.contains(member), "{kind:?}: {why}");
        assert!(why.contains(named), "{kind:?}: {why}");
        assert_eq!(started, 0, "{kind:?}");
        assert!(no_copy, "{kind:?}: no copy is made");
    }
}

/// A context path the copy does not hold, or one not spelled as one plain relative path, is
/// refused before the copy is made, naming the path.
#[test]
fn a_context_path_outside_the_copied_files_is_refused_before_the_copy() {
    let fixture = Fixture::new("context");
    std::fs::write(
        fixture.workspace.join("src/other.rs"),
        "pub fn other() {}\n",
    )
    .expect("a file");
    for path in [
        "src/other.rs",
        "./src/notes.md",
        "src//notes.md",
        "../workspace/src/notes.md",
        "/etc/passwd",
        "",
    ] {
        for kind in [Kind::Claude, Kind::Codex, Kind::B10x] {
            let (why, started, no_copy) = refused(&fixture, kind, context(&[path]));
            assert!(
                why.contains(&format!("{path:?}")),
                "{kind:?} {path:?}: {why}"
            );
            assert_eq!(started, 0, "{kind:?} {path:?}");
            assert!(no_copy, "{kind:?} {path:?}: no copy is made");
        }
    }
}

fn agent_spec(kind: SessionKind, session: impl FnOnce(&mut SessionSpec)) -> AgentSpec {
    let decisions = match kind {
        SessionKind::Harness => ToolPolicy {
            allow: Vec::new(),
            deny: Vec::new(),
            egress: Vec::new(),
            decisions: b10x_loom_session_executor::model::agent::DecisionMode::Observe,
        },
        SessionKind::Codex => ToolPolicy::codex_default(),
        SessionKind::ClaudeCode => ToolPolicy::claude_default(),
    };
    let mut spec = SessionSpec {
        kind,
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
        tools: decisions,
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

/// `SessionExecutor::from_spec` applies the spec's overrides, builds the executor it builds
/// today from a spec naming none, and refuses with a `SpecError` what the executor would refuse.
#[test]
fn from_spec_applies_the_specs_overrides() {
    let fixture = Fixture::new("from-spec");
    let build =
        |spec: &AgentSpec| SessionExecutor::from_spec(spec, &fixture.workspace, Fixture::files());

    let plain = build(&agent_spec(SessionKind::ClaudeCode, |_| {})).expect("builds");
    assert_eq!(plain.overrides, SessionOverrides::default());

    let named = build(&agent_spec(SessionKind::ClaudeCode, |session| {
        session.effort = Some("medium".to_owned());
        session.max_budget_micro_usd = Some(12_500_000);
        session.strict_version = Some(false);
        session.context = vec!["src/notes.md".to_owned()];
    }))
    .expect("builds");
    assert_eq!(
        named.overrides,
        SessionOverrides {
            effort: Some("medium".to_owned()),
            max_budget_micro_usd: Some(12_500_000),
            strict_version: Some(false),
            context: vec!["src/notes.md".to_owned()],
        }
    );
    let recorder = Recorder::new(Kind::Claude);
    let mut executor = named.with_runner(Box::new(recorder.clone()));
    executor.credentials = CredentialSource::None;
    executor.scratch = fixture.scratch();
    let step = step(&executor);
    assert!(
        matches!(step.outcome, ExecutorOutcome::ProposedAction(_)),
        "{:?}",
        step.outcome
    );
    let request = recorder.requests().remove(0);
    assert_eq!(after(&request, "--effort"), Some("medium"));
    assert_eq!(after(&request, "--max-budget-usd"), Some("12.5"));
    assert!(
        request
            .args
            .iter()
            .any(|arg| arg.contains(NOTES.trim_end()))
    );

    let refusals = [
        (
            agent_spec(SessionKind::Codex, |session| {
                session.max_budget_micro_usd = Some(1);
            }),
            SpecError::OverrideUnapplied {
                agent: "fixer".to_owned(),
                kind: SessionKind::Codex,
                member: "max_budget_micro_usd".to_owned(),
            },
        ),
        (
            agent_spec(SessionKind::Harness, |session| {
                session.max_budget_micro_usd = Some(1);
            }),
            SpecError::OverrideUnapplied {
                agent: "fixer".to_owned(),
                kind: SessionKind::Harness,
                member: "max_budget_micro_usd".to_owned(),
            },
        ),
        (
            agent_spec(SessionKind::Harness, |session| {
                session.effort = Some("low".to_owned());
            }),
            SpecError::OverrideUnapplied {
                agent: "fixer".to_owned(),
                kind: SessionKind::Harness,
                member: "effort".to_owned(),
            },
        ),
        (
            agent_spec(SessionKind::ClaudeCode, |session| {
                session.max_budget_micro_usd = Some(-1);
            }),
            SpecError::BudgetNegative {
                agent: "fixer".to_owned(),
                budget: -1,
            },
        ),
        (
            agent_spec(SessionKind::ClaudeCode, |session| {
                session.context = vec!["src/other.rs".to_owned()];
            }),
            SpecError::ContextNotCopied {
                agent: "fixer".to_owned(),
                path: "src/other.rs".to_owned(),
            },
        ),
        (
            agent_spec(SessionKind::Codex, |session| {
                session.context = vec!["../src/notes.md".to_owned()];
            }),
            SpecError::ContextNotCopied {
                agent: "fixer".to_owned(),
                path: "../src/notes.md".to_owned(),
            },
        ),
    ];
    for (spec, expected) in refusals {
        let refused = build(&spec).expect_err("refused");
        assert_eq!(refused, expected);
        let said = refused.to_string();
        assert!(said.contains("fixer"), "{said}");
    }
}
