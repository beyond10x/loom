//! Adversary cases for `story:session-executor-claude-loopback-endpoint` (wave 2026-10-30-w3,
//! unit B).
//!
//! The claim under attack: a Claude Code session given a keyless endpoint on a loopback host that
//! speaks `anthropic-messages` is pointed at it through metaharness's `model_endpoint` with
//! `credentials: none`, so the child reaches the model only through the endpoint and holds no
//! operator credential; any other Claude Code endpoint suspends the step `ExternalAvailability`
//! before any copy, naming why; Codex and b10x-harness are unchanged.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, or a runner that records the
//! launch and refuses to start): no harness program runs, no model is called, nothing reaches the
//! network.

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
    HarnessProcess, LaunchPlanView, ScriptStep, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::model::agent::ToolPolicy;
use b10x_loom_session_executor::{
    CredentialSource, Endpoint, Kind, ProcessRunner, Proposal, SessionExecutor,
    ToolPolicyDefaults as _,
};

const EDIT: &str = "implementation.edit";
const EDITED: &str = "pub fn a() -> i32 {\n    1\n}\n";
/// The boundary's model relay as the pod hands it out (`LOOM_BOUNDARY_MODEL_URL`, AGENTS.md).
const RELAY: &str = "http://127.0.0.1:8401/v1";
/// The key metaharness gives a child pointed at a declared endpoint.
const PLACEHOLDER: &str = "metaharness-model-endpoint";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-adversary-w20261030w3-b-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    fn executor(
        &self,
        kind: Kind,
        runner: Box<dyn ProcessRunner>,
        endpoint: Option<Endpoint>,
    ) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(kind)
                .with_runner(runner);
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

/// How a recording runner behaves once it has recorded the launch.
#[derive(Clone, Copy)]
enum Script {
    /// Prints a Claude Code session's records (`apiKeySource` the placeholder's variable).
    Claude,
    /// Prints a b10x-harness loop's lines.
    B10x,
    /// Records the launch and fails to start.
    Refuse,
}

struct Session {
    seen: Rc<RefCell<Seen>>,
    script: Script,
}

fn session(script: Script) -> (Box<dyn ProcessRunner>, Rc<RefCell<Seen>>) {
    let seen = Rc::<RefCell<Seen>>::default();
    (
        Box::new(Session {
            seen: Rc::clone(&seen),
            script,
        }),
        seen,
    )
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
        let said = serde_json::json!({"action": EDIT, "summary": "fixed a"}).to_string();
        match self.script {
            Script::Refuse => Err(std::io::Error::other(
                "adversary: launch recorded, not started",
            )),
            Script::Claude => {
                std::fs::write(plan.cwd.join("src/lib.rs"), EDITED)?;
                let init = serde_json::json!({
                    "type": "system", "subtype": "init", "claude_code_version": "2.1.304",
                    "cwd": plan.cwd.display().to_string(),
                    "tools": ToolPolicy::claude_default().allow,
                    "mcp_servers": [], "plugins": [], "output_style": "default",
                    "apiKeySource": "ANTHROPIC_API_KEY", "model": "scripted",
                    "permissionMode": "default", "session_id": "vendor-adversary-w3-b",
                });
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
            Script::B10x => {
                std::fs::write(plan.cwd.join("src/lib.rs"), EDITED)?;
                let lines = [
                    serde_json::json!({"kind": "started", "model": "relay-model",
                                       "published_tools": [], "credential_source": "none"}),
                    serde_json::json!({"kind": "text-delta", "text": said}),
                    serde_json::json!({"kind": "usage", "model": "relay-model",
                                       "input_tokens": 100, "output_tokens": 20,
                                       "cached_input_tokens": 0}),
                    serde_json::json!({"kind": "cost", "micro_usd": 1234}),
                    serde_json::json!({"kind": "finished", "stop": {"kind": "completed"}}),
                ]
                .into_iter()
                .map(|line| ScriptStep::line(line.to_string()))
                .collect();
                ScriptedRunner::new(lines, ScriptedLog::new()).start(plan)
            }
        }
    }
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adversary-w3-b".to_owned()),
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
    let case = CaseId("case-adversary-w3-b".to_owned());
    executor.run_with_usage(&commission(&case), &frontier(&case))
}

fn after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let at = args.iter().position(|arg| arg == flag)?;
    args.get(at + 1).map(String::as_str)
}

fn proposed(executor: &SessionExecutor, step: &ExecutorStep, what: &str) {
    match Proposal::from_outcome(&step.outcome) {
        Some(read) => assert_eq!(read.expect("a proposal's arguments").action, EDIT, "{what}"),
        None => panic!(
            "{what}: no proposal: {:?}; the session reported {:?}",
            step.outcome,
            executor.last_report().map(|report| report.error)
        ),
    }
}

/// The suspension's own text, as the embedder records it.
fn suspension_text(step: &ExecutorStep) -> Option<String> {
    match &step.outcome {
        ExecutorOutcome::Suspended(suspended) => match &suspended.reason {
            SuspensionReason::ExternalAvailability(why) => Some(format!("{why:?}")),
            _ => None,
        },
        _ => None,
    }
}

/// A Claude Code executor with `endpoint` is refused before any copy and before any launch; the
/// error names the session's kind and `names`. Returns the error.
fn refused_claude(name: &str, endpoint: Endpoint, names: &[&str]) -> String {
    let scratch = Scratch::new(name);
    let what = format!("{endpoint:?}");
    let (runner, seen) = session(Script::Claude);
    let executor = scratch.executor(Kind::Claude, runner, Some(endpoint));
    let step = step_of(&executor);
    assert!(
        suspension_text(&step).is_some(),
        "{what} suspends the step ExternalAvailability, got {:?}",
        step.outcome
    );
    assert!(
        !seen.borrow().started,
        "{what} starts no session; it launched {:?} with {:?}",
        seen.borrow().args,
        seen.borrow().env
    );
    assert_eq!(scratch.copies(), 0, "{what}: no copy is made");
    let error = executor
        .last_report()
        .and_then(|report| report.error)
        .expect("the report says why");
    assert!(error.contains("Claude Code"), "{what}: {error}");
    for name in names {
        assert!(
            error.contains(name),
            "{what}: the error names {name}: {error}"
        );
    }
    error
}

/// A Claude Code executor with `endpoint` launches, proposes, and the child is pointed at the
/// endpoint with the placeholder and no other credential. Returns what the session saw.
fn admitted_claude(name: &str, endpoint: Endpoint) -> Seen {
    let scratch = Scratch::new(name);
    let base_url = endpoint.base_url.clone();
    let (runner, seen) = session(Script::Claude);
    let executor = scratch.executor(Kind::Claude, runner, Some(endpoint));
    let step = step_of(&executor);
    proposed(&executor, &step, &base_url);
    let seen = std::mem::take(&mut *seen.borrow_mut());
    assert!(seen.started, "{base_url}: the session was started");
    assert_eq!(
        seen.env.get("ANTHROPIC_API_KEY").map(String::as_str),
        Some(PLACEHOLDER),
        "{base_url}"
    );
    assert_eq!(
        seen.env
            .get("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC")
            .map(String::as_str),
        Some("1"),
        "{base_url}"
    );
    assert_eq!(seen.credential_copies, 0, "{base_url}");
    seen
}

/// The relay the story names is `http://127.0.0.1:8401/v1`, and it admits paths under `/v1/`
/// (`crates/loom-boundary/src/route.rs:81`). metaharness reads `model_endpoint` as the gateway's
/// *root*, "no `/v1`": "Claude Code speaks Anthropic messages at `{root}/v1/messages`"
/// (metaharness 0.23.0 `crates/metaharness-protocol/src/spec.rs:304-307`). The b10x loop, by
/// contrast, posts to `{base-url}/messages` (`metaharness-b10x/src/launch.rs:113`), which is why
/// `Endpoint::base_url` carries the `/v1` prefix. Handing that base URL to a Claude Code child
/// unchanged sends its messages to `/v1/v1/messages` on the relay, and on to the upstream.
#[test]
fn adversary_the_claude_child_posts_its_messages_to_the_relays_v1_messages() {
    for (at, base_url) in [RELAY, "http://127.0.0.1:8401/v1/", "http://[::1]:8401/v1"]
        .into_iter()
        .enumerate()
    {
        let seen = admitted_claude(
            &format!("messages-path-{at}"),
            endpoint(base_url, "anthropic-messages"),
        );
        let root = seen
            .env
            .get("ANTHROPIC_BASE_URL")
            .expect("the child is given a base URL");
        let messages = format!("{root}/v1/messages");
        let expected = format!(
            "{}/messages",
            base_url.trim_end_matches('/') // the endpoint's own API prefix, once
        );
        assert_eq!(
            messages, expected,
            "{base_url}: Claude Code posts to ANTHROPIC_BASE_URL + /v1/messages \
             (metaharness spec.rs: model_endpoint is the root, no /v1), so the child's \
             base URL must be the endpoint's root, not its /v1 prefix"
        );
    }
}

/// Loopback spelled every way the URL parser normalises to a loopback host is admitted, and the
/// child is launched pointed at it.
#[test]
fn adversary_loopback_spellings_are_admitted() {
    for (at, base_url) in [
        "http://127.1:8401/v1",
        "http://0x7f000001:8401/v1",
        "HTTP://LOCALHOST:8401/v1",
        "http://LocalHost:8401/v1",
        "http://[0:0:0:0:0:0:0:1]:8401/v1",
        "https://127.0.0.1:8401/v1",
        "http://127.255.255.254:8401/v1",
        "http://127.0.0.1/v1",
    ]
    .into_iter()
    .enumerate()
    {
        let seen = admitted_claude(
            &format!("spelling-{at}"),
            endpoint(base_url, "anthropic-messages"),
        );
        assert!(
            seen.env.contains_key("ANTHROPIC_BASE_URL"),
            "{base_url}: {:?}",
            seen.env
        );
    }
}

/// URL tricks that put a foreign host behind a loopback-looking string, or no host at all: each
/// is refused before any copy, naming loopback.
#[test]
fn adversary_url_tricks_off_loopback_are_refused_before_any_copy() {
    for (at, base_url) in [
        "http://localhost:8401@models.example.test/v1",
        "http://127.0.0.1:8401@models.example.test/v1",
        "http://127.0.0.1.models.example.test/v1",
        "http://localhost.models.example.test/v1",
        "http://models.example.test/#@127.0.0.1:8401/v1",
        "http://models.example.test/?@127.0.0.1:8401/v1",
        "http://models.example.test\\@127.0.0.1:8401/v1",
        "http://:pw@127.0.0.1:8401/v1",
        "http://u:pw@localhost:8401/v1",
        "127.0.0.1:8401/v1",
        "localhost:8401/v1",
        "//127.0.0.1:8401/v1",
        "ftp://127.0.0.1:8401/v1",
        "ws://127.0.0.1:8401/v1",
        "file:///v1",
        "",
        "http://0.0.0.0:8401/v1",
        "http://[::]:8401/v1",
        "http://128.0.0.1:8401/v1",
        "http://126.255.255.255:8401/v1",
        "http://[::2]:8401/v1",
    ]
    .into_iter()
    .enumerate()
    {
        refused_claude(
            &format!("trick-{at}"),
            endpoint(base_url, "anthropic-messages"),
            &["loopback"],
        );
    }
}

/// Every wire spelling other than exactly `anthropic-messages` is refused before any copy.
#[test]
fn adversary_wire_spellings_other_than_anthropic_messages_are_refused() {
    for (at, wire) in [
        "Anthropic-Messages",
        "ANTHROPIC-MESSAGES",
        "anthropic_messages",
        "anthropic messages",
        " anthropic-messages",
        "anthropic-messages ",
        "anthropic-messages\n",
        "anthropic",
        "messages",
        "",
        "openai-responses",
        "openai-chat",
    ]
    .into_iter()
    .enumerate()
    {
        refused_claude(
            &format!("wire-{at}"),
            endpoint(RELAY, wire),
            &["anthropic-messages only"],
        );
    }
}

/// An endpoint whose base URL carries a password in its user information, or a key in its query,
/// is refused, and the refusal does not repeat the secret: the suspension reason and the report's
/// error are records the embedder keeps. The b10x-harness refusal names no URL at all; the
/// boundary names a flag "and never its value" (AGENTS.md § boundary).
#[test]
fn adversary_a_refused_claude_endpoint_does_not_echo_a_secret_in_its_url() {
    for (at, (base_url, secret)) in [
        ("http://relay:s3cr3t-pw@127.0.0.1:8401/v1", "s3cr3t-pw"),
        (
            "https://api.example.test/v1?key=sk-ant-adversary-0001",
            "sk-ant-adversary-0001",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let scratch = Scratch::new(&format!("echo-{at}"));
        let (runner, seen) = session(Script::Claude);
        let executor = scratch.executor(
            Kind::Claude,
            runner,
            Some(endpoint(base_url, "anthropic-messages")),
        );
        let step = step_of(&executor);
        assert!(!seen.borrow().started, "{base_url}: refused");
        let suspension = suspension_text(&step).expect("suspended ExternalAvailability");
        let error = executor
            .last_report()
            .and_then(|report| report.error)
            .expect("the report says why");
        assert!(
            !suspension.contains(secret) && !error.contains(secret),
            "{base_url}: the refusal repeats the secret {secret:?}: {error}"
        );
    }
}

/// The operator's credentials exported in the embedder's environment reach no Claude Code child
/// pointed at the endpoint, whatever credential source the executor was given: the child's key is
/// the placeholder, its base URL the endpoint's, and no operator value is in its environment or
/// arguments.
#[test]
fn adversary_operator_credentials_in_the_environment_do_not_reach_the_claude_child() {
    let operator = [
        ("ANTHROPIC_API_KEY", "sk-ant-operator-adversary-key"),
        ("ANTHROPIC_AUTH_TOKEN", "operator-adversary-bearer"),
        ("CLAUDE_CODE_OAUTH_TOKEN", "operator-adversary-oauth"),
        (
            "ANTHROPIC_BASE_URL",
            "https://operator-adversary.example.test",
        ),
        ("OPENAI_API_KEY", "sk-operator-adversary-openai"),
    ];
    for (name, value) in operator {
        // SAFETY: this test binary reads these variables only through metaharness's launch, and
        // no case in this file depends on their absence.
        unsafe { std::env::set_var(name, value) };
    }
    for (at, credentials) in [CredentialSource::ApiKey, CredentialSource::OperatorLogin]
        .into_iter()
        .enumerate()
    {
        let scratch = Scratch::new(&format!("operator-env-{at}"));
        let (runner, seen) = session(Script::Claude);
        let mut executor = scratch.executor(
            Kind::Claude,
            runner,
            Some(endpoint(RELAY, "anthropic-messages")),
        );
        executor.credentials = credentials;
        let step = step_of(&executor);
        proposed(&executor, &step, &format!("{credentials:?}"));
        let seen = seen.borrow();
        assert_eq!(
            seen.env.get("ANTHROPIC_API_KEY").map(String::as_str),
            Some(PLACEHOLDER),
            "{credentials:?}"
        );
        assert!(
            seen.env
                .get("ANTHROPIC_BASE_URL")
                .is_some_and(|url| url.starts_with("http://127.0.0.1:8401")),
            "{credentials:?}: {:?}",
            seen.env.get("ANTHROPIC_BASE_URL")
        );
        assert_eq!(seen.credential_copies, 0, "{credentials:?}");
        for (name, value) in operator {
            assert!(
                !seen.env.values().any(|held| held.contains(value)),
                "{credentials:?}: the operator's {name} reached the child's environment"
            );
            assert!(
                !seen.args.iter().any(|arg| arg.contains(value)),
                "{credentials:?}: the operator's {name} reached the child's arguments"
            );
        }
        for name in [
            "ANTHROPIC_AUTH_TOKEN",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "OPENAI_API_KEY",
        ] {
            assert!(!seen.env.contains_key(name), "{credentials:?}: {name}");
        }
    }
}

/// A key on the endpoint is refused even when every other part is right, and the endpoint's model
/// is not the reason: an empty pointer and a whitespace token file both name a key.
#[test]
fn adversary_any_named_key_is_refused_before_any_copy() {
    let mut empty_pointer = endpoint(RELAY, "anthropic-messages");
    empty_pointer.token_pointer = Some(String::new());
    refused_claude("key-empty-pointer", empty_pointer, &["names a key"]);

    let mut blank_file = endpoint(RELAY, "anthropic-messages");
    blank_file.token_file = PathBuf::from(" ");
    refused_claude("key-blank-file", blank_file, &["names a key"]);

    let mut both_wrong = endpoint("https://api.example.test/v1", "openai-responses");
    both_wrong.token_file = PathBuf::from("token.json");
    refused_claude("key-and-foreign-and-wire", both_wrong, &["names a key"]);
}

/// A refused Claude Code endpoint stays refused on a second call of the same executor, still
/// with no copy and no launch.
#[test]
fn adversary_a_refused_claude_endpoint_is_refused_again_on_a_second_call() {
    let scratch = Scratch::new("twice");
    let (runner, seen) = session(Script::Claude);
    let executor = scratch.executor(
        Kind::Claude,
        runner,
        Some(endpoint(
            "https://api.example.test/v1",
            "anthropic-messages",
        )),
    );
    for call in 0..2 {
        let step = step_of(&executor);
        assert!(
            suspension_text(&step).is_some(),
            "call {call}: {:?}",
            step.outcome
        );
        assert!(!seen.borrow().started, "call {call}");
        assert_eq!(scratch.copies(), 0, "call {call}");
    }
}

/// Codex is unchanged: it reads no endpoint, whatever its shape (keyed, keyless, either wire), is
/// not refused by the Claude Code check, and is launched with no endpoint in its environment or
/// arguments.
#[test]
fn adversary_codex_takes_no_endpoint_whatever_its_shape() {
    let mut keyed = endpoint(RELAY, "anthropic-messages");
    keyed.token_file = PathBuf::from("token.json");
    keyed.token_pointer = Some("/token".to_owned());
    for (at, endpoint) in [
        keyed,
        endpoint(RELAY, "openai-responses"),
        endpoint(RELAY, "anthropic-messages"),
        endpoint("https://api.example.test/v1", "Anthropic-Messages"),
    ]
    .into_iter()
    .enumerate()
    {
        let scratch = Scratch::new(&format!("codex-{at}"));
        let (runner, seen) = session(Script::Refuse);
        let mut executor = scratch.executor(Kind::Codex, runner, Some(endpoint.clone()));
        executor.credentials = CredentialSource::None;
        let step = step_of(&executor);
        let error = executor
            .last_report()
            .and_then(|report| report.error)
            .unwrap_or_default();
        assert!(
            seen.borrow().started,
            "{endpoint:?}: Codex is launched as before; the step said {error}"
        );
        assert!(
            !error.contains("Claude Code"),
            "{endpoint:?}: Codex met the Claude Code check: {error}"
        );
        assert!(suspension_text(&step).is_some(), "{:?}", step.outcome);
        let seen = seen.borrow();
        let host = url_host(&endpoint.base_url);
        assert!(
            !seen.env.values().any(|value| value.contains(&host)),
            "{endpoint:?}: the endpoint reached Codex's environment: {:?}",
            seen.env
        );
        assert!(
            !seen.args.iter().any(|arg| arg.contains(&host)),
            "{endpoint:?}: the endpoint reached Codex's arguments: {:?}",
            seen.args
        );
    }
}

fn url_host(base_url: &str) -> String {
    base_url
        .split("//")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or_default()
        .to_owned()
}

/// b10x-harness is unchanged: a keyless loopback endpoint on `openai-responses` (refused for
/// Claude Code) is launched, with the endpoint's `/v1` base URL as `--base-url` and its wire.
#[test]
fn adversary_b10x_keyless_loopback_on_another_wire_is_unchanged() {
    let scratch = Scratch::new("b10x");
    let (runner, seen) = session(Script::B10x);
    let mut executor = scratch.executor(
        Kind::B10x,
        runner,
        Some(endpoint(RELAY, "openai-responses")),
    );
    executor.credentials = CredentialSource::None;
    let step = step_of(&executor);
    proposed(&executor, &step, "b10x-harness");
    let seen = seen.borrow();
    assert_eq!(
        after(&seen.args, "--base-url"),
        Some(RELAY),
        "{:?}",
        seen.args
    );
    assert!(
        seen.args.iter().any(|arg| arg.contains("openai-responses")),
        "{:?}",
        seen.args
    );
    assert_eq!(
        after(&seen.args, "--model"),
        Some("relay-model"),
        "{:?}",
        seen.args
    );
}
