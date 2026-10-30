//! Security conformance checks for `story:session-executor-claude-loopback-endpoint`, written by
//! the wave's security reviewer. A Claude Code session pointed at an endpoint carries no operator
//! credential (no login copy, no API key, no OAuth token, nothing inherited from the environment),
//! its child is told to send nothing past the endpoint, only a loopback host is admitted, and a
//! keyed or foreign endpoint never reaches a launch.
//!
//! Every session is scripted (metaharness's `ScriptedRunner`): no harness program runs, no model
//! is called and nothing reaches the network. The environment check re-runs this test binary as a
//! child process with an operator environment set on it, so no test mutates its own process
//! environment.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
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
    HarnessProcess, LaunchPlanView, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::model::agent::ToolPolicy;
use b10x_loom_session_executor::{
    CredentialSource, Endpoint, Kind, ProcessRunner, Proposal, SessionExecutor,
    ToolPolicyDefaults as _,
};

const EDIT: &str = "implementation.edit";
const RELAY: &str = "http://127.0.0.1:8401/v1";
const PLACEHOLDER: &str = "metaharness-model-endpoint";
/// Set on the re-run child: the environment test runs its body there.
const CHILD: &str = "LOOM_SECURITY_W3B_CHILD";
/// A marker every operator credential in the child's environment carries.
const SENTINEL: &str = "operator-secret-sentinel-w3b";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-security-w3b-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    fn executor(&self, session: Session, endpoint: Option<Endpoint>) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(Kind::Claude)
                .with_runner(Box::new(session));
        executor.scratch = self.0.join("scratch");
        executor.endpoint = endpoint;
        executor
    }

    fn copies(&self) -> usize {
        std::fs::read_dir(self.0.join("scratch")).map_or(0, Iterator::count)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn endpoint(base_url: &str, wire: &str) -> Endpoint {
    Endpoint {
        base_url: base_url.to_owned(),
        wire: wire.to_owned(),
        model: "relay-model".to_owned(),
        token_file: PathBuf::new(),
        token_pointer: None,
    }
}

#[derive(Default)]
struct Seen {
    started: bool,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    credential_copies: usize,
}

struct Session {
    seen: Rc<RefCell<Seen>>,
}

impl Session {
    fn new() -> (Self, Rc<RefCell<Seen>>) {
        let seen = Rc::<RefCell<Seen>>::default();
        (
            Self {
                seen: Rc::clone(&seen),
            },
            seen,
        )
    }
}

impl ProcessRunner for Session {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        *self.seen.borrow_mut() = Seen {
            started: true,
            args: plan.args.to_vec(),
            env: plan.env.clone(),
            credential_copies: plan.credential_copies.len(),
        };
        std::fs::write(
            plan.cwd.join("src/lib.rs"),
            "pub fn a() -> i32 {\n    1\n}\n",
        )?;
        let init = serde_json::json!({
            "type": "system", "subtype": "init", "claude_code_version": "2.1.304",
            "cwd": plan.cwd.display().to_string(), "tools": ToolPolicy::claude_default().allow,
            "mcp_servers": [], "plugins": [], "output_style": "default",
            "apiKeySource": "ANTHROPIC_API_KEY", "model": "scripted",
            "permissionMode": "default", "session_id": "vendor-security-w3b",
        });
        let said = serde_json::json!({"action": EDIT, "summary": "fixed a"}).to_string();
        let reply = serde_json::json!({
            "type": "assistant",
            "message": {"content": [{"type": "text", "text": said}],
                        "usage": {"input_tokens": 100, "output_tokens": 20}},
        });
        let result = serde_json::json!({
            "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
            "duration_ms": 2000, "total_cost_usd": 0.0003,
            "usage": {"input_tokens": 100, "output_tokens": 20},
        });
        ScriptedRunner::of_lines(
            [init.to_string(), reply.to_string(), result.to_string()],
            ScriptedLog::new(),
        )
        .start(plan)
    }
}

fn step_of(executor: &SessionExecutor) -> ExecutorStep {
    let case = CaseId("case-security-w3b".to_owned());
    let commission = Commission::<commission_state::Assigned>::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-security-w3b".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    let frontier = Frontier::<frontier_state::Issued>::new(FrontierData {
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

/// `endpoint` on a Claude Code session suspends the step `ExternalAvailability`, starts nothing
/// and makes no copy.
fn never_launched(name: &str, endpoint: Endpoint) {
    let what = format!("{endpoint:?}");
    let scratch = Scratch::new(name);
    let (session, seen) = Session::new();
    let executor = scratch.executor(session, Some(endpoint));
    let step = step_of(&executor);
    assert!(
        matches!(
            &step.outcome,
            ExecutorOutcome::Suspended(suspended)
                if matches!(suspended.reason, SuspensionReason::ExternalAvailability(_))
        ),
        "{what} suspends the step, got {:?}",
        step.outcome
    );
    assert!(
        !seen.borrow().started,
        "{what} reaches no launch; it launched with {:?}",
        seen.borrow().env.get("ANTHROPIC_BASE_URL")
    );
    assert_eq!(scratch.copies(), 0, "{what}: no copy is made");
}

/// The operator environment the re-run child is given: every way an operator credential, a
/// foreign base URL or a proxy can sit in a shell, each value carrying [`SENTINEL`].
fn operator_environment(home: &std::path::Path) -> Vec<(String, String)> {
    let mut vars: Vec<(String, String)> = [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "ANTHROPIC_CUSTOM_HEADERS",
        "AWS_BEARER_TOKEN_BEDROCK",
        "AWS_SECRET_ACCESS_KEY",
        "GOOGLE_APPLICATION_CREDENTIALS",
        "OPENAI_API_KEY",
        "GITHUB_TOKEN",
    ]
    .iter()
    .map(|name| ((*name).to_owned(), format!("{SENTINEL}-{name}")))
    .collect();
    vars.extend(
        [
            ("ANTHROPIC_BASE_URL", "https://models.example.test/sentinel"),
            ("HTTPS_PROXY", "http://proxy.example.test/sentinel"),
            ("https_proxy", "http://proxy.example.test/sentinel"),
            ("ALL_PROXY", "http://proxy.example.test/sentinel"),
            ("CLAUDE_CODE_USE_BEDROCK", "1-sentinel"),
            ("CLAUDE_CODE_USE_VERTEX", "1-sentinel"),
            ("ANTHROPIC_MODEL", "sentinel-model"),
        ]
        .iter()
        .map(|(name, value)| ((*name).to_owned(), format!("{value}-{SENTINEL}"))),
    );
    vars.push((
        "CLAUDE_CONFIG_DIR".to_owned(),
        home.join(".claude").display().to_string(),
    ));
    vars.push(("HOME".to_owned(), home.display().to_string()));
    vars
}

/// The child's environment holds no operator credential and nothing the operator's shell set
/// beyond metaharness's allowlist, whatever credentials the executor itself names; the endpoint
/// is the only model address and the nonessential-traffic switch is on. The operator's login in
/// `$HOME/.claude/.credentials.json` is not copied.
#[test]
fn security_a_claude_endpoint_child_inherits_no_operator_credential() {
    if std::env::var_os(CHILD).is_none() {
        let home = Scratch::new("operator-home");
        std::fs::create_dir_all(home.0.join(".claude")).expect("an operator config dir");
        std::fs::write(
            home.0.join(".claude/.credentials.json"),
            format!("{{\"claudeAiOauth\":{{\"accessToken\":\"{SENTINEL}-login\"}}}}"),
        )
        .expect("an operator login");
        let exe = std::env::current_exe().expect("this test binary");
        let output = std::process::Command::new(exe)
            .args([
                "security_a_claude_endpoint_child_inherits_no_operator_credential",
                "--exact",
                "--test-threads=1",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .envs(operator_environment(&home.0))
            .output()
            .expect("the re-run child");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "the check failed in the child with an operator environment:\n{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let home = PathBuf::from(std::env::var("HOME").expect("HOME"));
    let allowed = [
        "HOME",
        "USER",
        "LOGNAME",
        "LANG",
        "LC_ALL",
        "TERM",
        "TZ",
        "PATH",
        "TMPDIR",
        "CLAUDE_CONFIG_DIR",
        "AEP_ACTOR",
        "ANTHROPIC_BASE_URL",
        "ANTHROPIC_API_KEY",
        "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
    ];
    for (at, credentials) in [
        CredentialSource::OperatorLogin,
        CredentialSource::ApiKey,
        CredentialSource::None,
        CredentialSource::Loopback,
    ]
    .into_iter()
    .enumerate()
    {
        let scratch = Scratch::new(&format!("inherit-{at}"));
        let (session, seen) = Session::new();
        let mut executor = scratch.executor(session, Some(endpoint(RELAY, "anthropic-messages")));
        executor.credentials = credentials;
        let step = step_of(&executor);
        assert!(
            Proposal::from_outcome(&step.outcome).is_some(),
            "{credentials:?}: the session proposes: {:?}; {:?}",
            step.outcome,
            executor.last_report().map(|report| report.error)
        );
        let seen = seen.borrow();
        assert!(seen.started, "{credentials:?}: started");
        for (name, value) in &seen.env {
            assert!(
                !value.contains(SENTINEL),
                "{credentials:?}: the child's {name} carries the operator's value {value:?}"
            );
            assert!(
                allowed.contains(&name.as_str()),
                "{credentials:?}: the child's environment holds {name}={value:?}, outside the \
                 allowlist"
            );
        }
        assert_eq!(
            seen.env.get("ANTHROPIC_BASE_URL").map(String::as_str),
            RELAY.strip_suffix("/v1"),
            "{credentials:?}: the endpoint's root is the only model address \
             (story:session-executor-claude-loopback-endpoint: Claude Code is given the root)"
        );
        assert_eq!(
            seen.env.get("ANTHROPIC_API_KEY").map(String::as_str),
            Some(PLACEHOLDER),
            "{credentials:?}: the key is metaharness's placeholder"
        );
        assert_eq!(
            seen.env
                .get("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC")
                .map(String::as_str),
            Some("1"),
            "{credentials:?}: nothing past the endpoint"
        );
        assert_eq!(
            seen.credential_copies, 0,
            "{credentials:?}: no login is copied"
        );
        let config = seen
            .env
            .get("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .expect("a config home of the run's own");
        assert!(
            !config.starts_with(&home),
            "{credentials:?}: the child's config home {config:?} is not the operator's"
        );
        assert!(
            !config.join(".credentials.json").exists(),
            "{credentials:?}: no login file in the child's config home"
        );
        assert!(
            !seen.args.iter().any(|arg| arg.contains(SENTINEL)),
            "{credentials:?}: no operator value in the argv: {:?}",
            seen.args
        );
    }
}

/// Every spelling of a host that is not loopback, every URL a parser could read two ways, and
/// every non-http scheme: refused before any copy, never launched.
#[test]
fn security_a_claude_endpoint_off_loopback_in_any_spelling_never_reaches_a_launch() {
    for (at, base_url) in [
        "http://127.0.0.1.example.test:8401/v1",
        "http://localhost.example.test:8401/v1",
        "http://localhost.:8401/v1",
        "http://127.0.0.1@models.example.test/v1",
        "http://localhost:8401@models.example.test/v1",
        "http://models.example.test\\@127.0.0.1:8401/v1",
        "http://models.example.test#@127.0.0.1:8401/v1",
        "http://models.example.test?@127.0.0.1:8401/v1",
        "http://:secret@127.0.0.1:8401/v1",
        "http://0.0.0.0:8401/v1",
        "http://[::ffff:127.0.0.1]:8401/v1",
        "http://[::]:8401/v1",
        "http://169.254.169.254/v1",
        "http://10.0.0.1:8401/v1",
        "ws://127.0.0.1:8401/v1",
        "file:///run/relay.sock",
        "127.0.0.1:8401/v1",
        "//127.0.0.1:8401/v1",
        " https://models.example.test/v1",
        "",
    ]
    .into_iter()
    .enumerate()
    {
        never_launched(
            &format!("foreign-{at}"),
            endpoint(base_url, "anthropic-messages"),
        );
    }
}

/// A loopback endpoint that names a key in any form, or speaks any wire but exactly
/// `anthropic-messages`: refused before any copy, never launched.
#[test]
fn security_a_keyed_or_mis_wired_loopback_endpoint_never_reaches_a_launch() {
    let mut keyed = Vec::new();
    for token_file in [".", "token.json", " "] {
        let mut named = endpoint(RELAY, "anthropic-messages");
        named.token_file = PathBuf::from(token_file);
        keyed.push(named);
    }
    for pointer in ["", "/token", "/"] {
        let mut pointed = endpoint(RELAY, "anthropic-messages");
        pointed.token_pointer = Some(pointer.to_owned());
        keyed.push(pointed);
    }
    for wire in [
        "",
        "Anthropic-Messages",
        "anthropic-messages ",
        "anthropic",
        "openai-chat",
        "openai-responses",
    ] {
        keyed.push(endpoint(RELAY, wire));
    }
    for (at, endpoint) in keyed.into_iter().enumerate() {
        never_launched(&format!("keyed-{at}"), endpoint);
    }
}

/// The endpoint's own `model` field, now read for a Claude Code session, adds no argument to
/// the argv: it is only ever the value of `--model`, or the launch is refused.
#[test]
fn security_a_claude_endpoints_model_is_only_the_model_value() {
    for (at, model) in [
        "--dangerously-skip-permissions",
        "--add-dir=/",
        "--settings=/etc/passwd",
        "relay-model --bare",
    ]
    .into_iter()
    .enumerate()
    {
        let scratch = Scratch::new(&format!("model-{at}"));
        let (session, seen) = Session::new();
        let mut endpoint = endpoint(RELAY, "anthropic-messages");
        endpoint.model = model.to_owned();
        let executor = scratch.executor(session, Some(endpoint));
        let _ = step_of(&executor);
        let seen = seen.borrow();
        eprintln!("endpoint model {model:?}: launched {}", seen.started);
        if !seen.started {
            continue;
        }
        let positions: Vec<usize> = seen
            .args
            .iter()
            .enumerate()
            .filter(|(_, arg)| arg.as_str() == model)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(positions.len(), 1, "{model}: {:?}", seen.args);
        assert_eq!(
            seen.args.get(positions[0] - 1).map(String::as_str),
            Some("--model"),
            "{model} is only the value of --model: {:?}",
            seen.args
        );
    }
}
