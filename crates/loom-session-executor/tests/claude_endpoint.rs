//! A Claude Code session given an `Endpoint` (`story:session-executor-claude-loopback-endpoint`).
//! In a credential-free pod the model is reached only through the boundary's loopback relay,
//! which adds the credential. A keyless endpoint on a loopback host that speaks
//! `anthropic-messages` becomes metaharness's `model_endpoint` with `credentials: none`, so the
//! child gets `ANTHROPIC_BASE_URL`, metaharness's placeholder key and the nonessential-traffic
//! switch, and no operator credential. Any other endpoint on a Claude Code session (a key, a token
//! pointer, a host that is not loopback, another wire) suspends the step before any copy, naming
//! why. A Claude Code session with no endpoint is unchanged.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, the records a Claude Code
//! session prints): no harness program runs, no model is called and nothing reaches the network.

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
const EDITED: &str = "pub fn a() -> i32 {\n    1\n}\n";
const RELAY: &str = "http://127.0.0.1:8401/v1";
/// The key metaharness gives a child pointed at a declared endpoint: a marker, not a secret.
const PLACEHOLDER: &str = "metaharness-model-endpoint";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A directory of the test's own under the system's temporary directory: `tree/`, the workspace
/// with `src/lib.rs`, and `scratch/` for the copies; removed when dropped. Not under the
/// repository: metaharness refuses a Claude session whose copy has memory files above it.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-claude-endpoint-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    /// A Claude Code executor over the tree, started by `session`, with the credentials
    /// `SessionExecutor::new` gives it (the operator's login) and the endpoint `endpoint`.
    fn executor(&self, session: Session, endpoint: Option<Endpoint>) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(Kind::Claude)
                .with_runner(Box::new(session));
        executor.scratch = self.0.join("scratch");
        executor.endpoint = endpoint;
        executor
    }

    /// Whether the executor left a copy of the tree behind in its scratch directory.
    fn copies(&self) -> usize {
        std::fs::read_dir(self.0.join("scratch")).map_or(0, Iterator::count)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A keyless endpoint at `base_url` speaking `wire`.
fn endpoint(base_url: &str, wire: &str) -> Endpoint {
    Endpoint {
        base_url: base_url.to_owned(),
        wire: wire.to_owned(),
        model: "relay-model".to_owned(),
        token_file: PathBuf::new(),
        token_pointer: None,
    }
}

/// What a [`Session`] saw when it was started.
#[derive(Default)]
struct Seen {
    started: bool,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    credential_copies: usize,
}

/// A scripted Claude Code session: started, it keeps what it was started with, edits
/// `src/lib.rs` in its copy and prints the records the vendor prints under a declared endpoint
/// (`apiKeySource: "ANTHROPIC_API_KEY"`, the placeholder metaharness put there).
struct Session {
    seen: Rc<RefCell<Seen>>,
    api_key_source: &'static str,
}

impl Session {
    fn new(api_key_source: &'static str) -> (Self, Rc<RefCell<Seen>>) {
        let seen = Rc::<RefCell<Seen>>::default();
        (
            Self {
                seen: Rc::clone(&seen),
                api_key_source,
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
        std::fs::write(plan.cwd.join("src/lib.rs"), EDITED)?;
        let init = serde_json::json!({
            "type": "system", "subtype": "init", "claude_code_version": "2.1.304",
            "cwd": plan.cwd.display().to_string(), "tools": ToolPolicy::claude_default().allow,
            "mcp_servers": [], "plugins": [], "output_style": "default",
            "apiKeySource": self.api_key_source, "model": "scripted",
            "permissionMode": "default", "session_id": "vendor-claude-endpoint",
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

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-claude-endpoint".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn frontier(case: &CaseId) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf1)),
        case_id: case.clone(),
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
    let case = CaseId("case-claude-endpoint".to_owned());
    executor.run_with_usage(&commission(&case), &frontier(&case))
}

/// The value following `flag` in `args`.
fn after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let at = args.iter().position(|arg| arg == flag)?;
    args.get(at + 1).map(String::as_str)
}

/// The step proposed the session's edit; `what` names the case.
fn proposed(executor: &SessionExecutor, step: &ExecutorStep, what: &str) {
    match Proposal::from_outcome(&step.outcome) {
        Some(read) => assert_eq!(read.expect("a proposal's arguments").action, EDIT),
        None => panic!(
            "{what}: no proposal: {:?}; the session reported {:?}",
            step.outcome,
            executor.last_report().map(|report| report.error)
        ),
    }
}

/// The step suspended for an external availability, the session never started, no copy made,
/// and the report's error holds every one of `names`.
fn refused(scratch: &Scratch, endpoint: Endpoint, names: &[&str]) {
    let what = format!("{endpoint:?}");
    let (session, seen) = Session::new("ANTHROPIC_API_KEY");
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
        "{what} starts no session; it launched {:?}",
        seen.borrow().args
    );
    assert_eq!(scratch.copies(), 0, "{what}: no copy is made");
    let error = executor
        .last_report()
        .and_then(|report| report.error)
        .expect("the report says why");
    assert!(
        error.contains("Claude Code"),
        "{what}: the error names the session's kind: {error}"
    );
    for name in names {
        assert!(
            error.contains(name),
            "{what}: the error names {name}: {error}"
        );
    }
}

#[test]
fn a_claude_session_with_a_keyless_loopback_endpoint_gets_it_as_its_model_endpoint() {
    for (at, (base_url, model, expected_model)) in [
        (RELAY, None, "relay-model"),
        (
            RELAY,
            Some("claude-sonnet-scripted"),
            "claude-sonnet-scripted",
        ),
        ("http://localhost:8401/v1/", None, "relay-model"),
        ("http://[::1]:8401/v1", None, "relay-model"),
    ]
    .into_iter()
    .enumerate()
    {
        let scratch = Scratch::new(&format!("loopback-{at}"));
        let (session, seen) = Session::new("ANTHROPIC_API_KEY");
        let mut executor =
            scratch.executor(session, Some(endpoint(base_url, "anthropic-messages")));
        executor.model = model.map(str::to_owned);
        assert_eq!(
            executor.credentials,
            CredentialSource::OperatorLogin,
            "the executor's own credentials are the operator's login; the endpoint replaces them"
        );

        let step = step_of(&executor);

        proposed(&executor, &step, base_url);
        let seen = seen.borrow();
        assert!(seen.started, "{base_url}: the session was started");
        assert_eq!(
            seen.env.get("ANTHROPIC_BASE_URL").map(String::as_str),
            Some(base_url.trim_end_matches('/')),
            "{base_url}: the child is pointed at the endpoint"
        );
        assert_eq!(
            seen.env.get("ANTHROPIC_API_KEY").map(String::as_str),
            Some(PLACEHOLDER),
            "{base_url}: the child's key is metaharness's placeholder"
        );
        assert_eq!(
            seen.env
                .get("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC")
                .map(String::as_str),
            Some("1"),
            "{base_url}: the child sends nothing past the endpoint"
        );
        assert!(
            !seen.env.contains_key("ANTHROPIC_AUTH_TOKEN"),
            "{base_url}: the child holds no bearer token"
        );
        assert_eq!(
            seen.credential_copies, 0,
            "{base_url}: no operator credential is copied to the child (credentials: none)"
        );
        assert_eq!(
            after(&seen.args, "--model"),
            Some(expected_model),
            "{base_url}: the model is the executor's or else the endpoint's: {:?}",
            seen.args
        );
    }
}

#[test]
fn a_claude_endpoint_that_is_not_loopback_or_names_a_key_is_refused() {
    let scratch = Scratch::new("foreign");
    refused(
        &scratch,
        endpoint("https://api.example.test/v1", "anthropic-messages"),
        &["https://api.example.test/v1", "loopback"],
    );

    let scratch = Scratch::new("userinfo");
    refused(
        &scratch,
        endpoint("http://user@127.0.0.1:8401/v1", "anthropic-messages"),
        &["loopback"],
    );

    let scratch = Scratch::new("token-file");
    let mut keyed = endpoint(RELAY, "anthropic-messages");
    keyed.token_file = scratch.0.join("token.json");
    refused(&scratch, keyed, &["token_file"]);

    let scratch = Scratch::new("token-pointer");
    let mut pointed = endpoint(RELAY, "anthropic-messages");
    pointed.token_pointer = Some("/token".to_owned());
    refused(&scratch, pointed, &["token_pointer"]);
}

#[test]
fn a_claude_endpoint_on_another_wire_is_refused() {
    let scratch = Scratch::new("wire");
    refused(
        &scratch,
        endpoint(RELAY, "openai-responses"),
        &["openai-responses", "anthropic-messages"],
    );
}

#[test]
fn a_claude_session_with_no_endpoint_is_unchanged() {
    let scratch = Scratch::new("none");
    let (session, seen) = Session::new("none");
    let mut executor = scratch.executor(session, None);
    executor.credentials = CredentialSource::None;
    let step = step_of(&executor);
    proposed(&executor, &step, "no endpoint");
    let seen = seen.borrow();
    for variable in [
        "ANTHROPIC_BASE_URL",
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
    ] {
        assert!(
            !seen.env.contains_key(variable),
            "a Claude Code session with no endpoint is given no {variable}"
        );
    }
    assert_eq!(after(&seen.args, "--model"), None, "{:?}", seen.args);
}
