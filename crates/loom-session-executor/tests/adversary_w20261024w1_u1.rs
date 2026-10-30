//! Adversary cases for `SessionExecutor::api_key_secret` (story:endpoint-api-key-secret): a
//! b10x-harness session whose endpoint's key is named in the operator's secret store.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, b10x-harness's own event
//! lines): no harness program runs, no model is called, no secret store is opened, no credential
//! file is read and nothing reaches the network. The environment cases start this test binary
//! again with a synthetic environment, so the caller's variables are the test's own.

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
use b10x_loom_session_executor::{
    CredentialSource, Endpoint, Kind, ProcessRunner, Proposal, SessionExecutor,
};

const EDIT: &str = "implementation.edit";
const EDITED: &str = "pub fn a() -> i32 {\n    1\n}\n";
const SECRET: &str = "llm/loom-endpoint-key";
const SECRETS_SOURCE: &str = "api-key:secrets";

/// Set only in the re-started test binary: the environment cases run their body there.
const PROBE: &str = "LOOM_ADVERSARY_W20261024W1_U1_PROBE";
/// A synthetic key value placed in the re-started binary's environment; it must reach nothing.
const SENTINEL: &str = "sk-adversary-sentinel-0000000000000000";
const BUS: &str = "unix:path=/run/user/4242/bus-adversary";
const RUNTIME: &str = "/run/user/4242";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-adversary-w20261024w1-u1-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    fn executor(&self, kind: Kind, session: Session, secret: Option<&str>) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(kind)
                .with_runner(Box::new(session));
        executor.credentials = CredentialSource::None;
        executor.scratch = self.0.join("scratch");
        executor.endpoint = Some(Endpoint {
            base_url: "http://127.0.0.1:9/v1".to_owned(),
            wire: "openai-responses".to_owned(),
            model: "synthetic".to_owned(),
            token_file: PathBuf::new(),
            token_pointer: None,
        });
        executor.api_key_secret = secret.map(str::to_owned);
        executor
    }

    /// What the scratch root holds after a call: nothing when no copy was left behind.
    fn scratch_entries(&self) -> Vec<PathBuf> {
        std::fs::read_dir(self.0.join("scratch"))
            .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
struct Seen {
    started: bool,
    args: Vec<String>,
    env: BTreeMap<String, String>,
}

/// A scripted b10x-harness session that states `credential_source` as `source`.
struct Session {
    seen: Rc<RefCell<Seen>>,
    log: ScriptedLog,
    source: &'static str,
}

impl Session {
    fn new() -> (Self, Rc<RefCell<Seen>>) {
        Self::stating(SECRETS_SOURCE)
    }

    fn stating(source: &'static str) -> (Self, Rc<RefCell<Seen>>) {
        let seen = Rc::<RefCell<Seen>>::default();
        (
            Self {
                seen: Rc::clone(&seen),
                log: ScriptedLog::new(),
                source,
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
        };
        std::fs::write(plan.cwd.join("src/lib.rs"), EDITED)?;
        ScriptedRunner::new(lines(self.source), self.log.clone()).start(plan)
    }
}

fn lines(source: &str) -> Vec<ScriptStep> {
    let action = serde_json::json!({"action": EDIT, "summary": "fixed a"}).to_string();
    [
        serde_json::json!({"kind": "started", "model": "synthetic", "published_tools": [],
                           "credential_source": source}),
        serde_json::json!({"kind": "text-delta", "text": action}),
        serde_json::json!({"kind": "usage", "model": "synthetic", "input_tokens": 100,
                           "output_tokens": 20, "cached_input_tokens": 0}),
        serde_json::json!({"kind": "cost", "micro_usd": 1234}),
        serde_json::json!({"kind": "finished", "stop": {"kind": "completed"}}),
    ]
    .into_iter()
    .map(|line| ScriptStep::line(line.to_string()))
    .collect()
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adversary-secret".to_owned()),
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
    let case = CaseId("case-adversary-secret".to_owned());
    executor.run_with_usage(&commission(&case), &frontier(&case))
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let attached = format!("{flag}=");
    args.iter().enumerate().find_map(|(at, arg)| {
        if arg == flag {
            args.get(at + 1).map(String::as_str)
        } else {
            arg.strip_prefix(&attached)
        }
    })
}

/// The credential flags b10x-harness reads; a launch carries at most one.
const CREDENTIAL_FLAGS: [&str; 5] = [
    "--api-key-secret",
    "--api-key-env",
    "--api-key-file",
    "--oauth-token-file",
    "--oauth-token-env",
];

fn credential_flags(args: &[String]) -> Vec<&str> {
    CREDENTIAL_FLAGS
        .into_iter()
        .filter(|flag| flag_value(args, flag).is_some())
        .collect()
}

fn proposed(executor: &SessionExecutor, step: &ExecutorStep) -> Proposal {
    match Proposal::from_outcome(&step.outcome) {
        Some(read) => read.expect("a proposal's arguments"),
        None => panic!(
            "no proposal: {:?}; the session reported {:?}",
            step.outcome,
            executor.last_report().map(|report| report.error)
        ),
    }
}

/// The refusal's text, from the report and from the Suspension, after asserting the step was
/// refused before any session and before any copy.
fn refusal_before_the_copy(
    scratch: &Scratch,
    executor: &SessionExecutor,
    seen: &Rc<RefCell<Seen>>,
) -> (String, String) {
    let step = step_of(executor);
    let detail = match &step.outcome {
        ExecutorOutcome::Suspended(suspended) => match &suspended.reason {
            SuspensionReason::ExternalAvailability(detail) => format!("{detail:?}"),
            other => panic!("suspended for an external availability, got {other:?}"),
        },
        other => panic!("a refused credential suspends the step, got {other:?}"),
    };
    assert!(!seen.borrow().started, "no session is started");
    let report = executor.last_report().expect("a report");
    assert_eq!(report.cwd, None, "no copy was made: {:?}", report.cwd);
    assert_eq!(
        scratch.scratch_entries(),
        Vec::<PathBuf>::new(),
        "nothing is left in the scratch root"
    );
    (report.error.expect("the report says why"), detail)
}

/// Whether `text` holds a character a terminal reads as a control or a reordering.
fn unescaped(text: &str) -> Vec<char> {
    text.chars()
        .filter(|c| {
            c.is_control()
                || matches!(*c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}')
                || matches!(*c, '\u{2066}'..='\u{2069}')
        })
        .collect()
}

#[test]
fn adversary_a_name_starting_with_a_dash_is_attached_to_its_flag() {
    let scratch = Scratch::new("dash");
    for name in ["-ns/key", "--yes/x", "--api-key-env=OPENAI_API_KEY/x"] {
        let (session, seen) = Session::new();
        let executor = scratch.executor(Kind::B10x, session, Some(name));
        let step = step_of(&executor);
        let _ = proposed(&executor, &step);
        let seen = seen.borrow();
        let attached = format!("--api-key-secret={name}");
        assert!(
            seen.args.contains(&attached),
            "the name is attached to its flag: {:?}",
            seen.args
        );
        assert!(
            !seen.args.iter().any(|arg| arg == name),
            "the name is never an argument of its own: {:?}",
            seen.args
        );
        assert!(
            !seen.args.iter().any(|arg| arg == "--yes"),
            "no option is injected: {:?}",
            seen.args
        );
        assert_eq!(credential_flags(&seen.args), vec!["--api-key-secret"]);
    }
}

#[test]
fn adversary_a_refusal_escapes_control_characters_in_the_name() {
    let scratch = Scratch::new("escape");
    // Each refusal path: a vendor kind, a secret beside a token, and a shape metaharness refuses.
    let cases: [(Kind, bool, &str); 6] = [
        (Kind::Codex, false, "ns/a\u{1b}[2Jb"),
        (Kind::Claude, false, "ns/a\nb\rc"),
        (Kind::B10x, true, "ns/a\u{1b}]0;title\u{7}b"),
        (Kind::B10x, true, "ns/\u{202e}cba"),
        (Kind::B10x, false, "bad\u{1b}[2Jname"),
        (Kind::B10x, false, "no-slash\n\u{2067}x"),
    ];
    for (kind, beside_token, name) in cases {
        let (session, seen) = Session::new();
        let mut executor = scratch.executor(kind, session, Some(name));
        if kind != Kind::B10x {
            executor.endpoint = None;
        }
        if beside_token {
            executor
                .endpoint
                .as_mut()
                .expect("an endpoint")
                .token_pointer = Some("/token".to_owned());
        }
        let (error, detail) = refusal_before_the_copy(&scratch, &executor, &seen);
        assert_eq!(
            unescaped(&error),
            Vec::<char>::new(),
            "the report's error quotes {name:?} escaped: {error:?}"
        );
        let escaped = name.escape_debug().to_string();
        assert!(
            error.contains(&escaped),
            "the report's error names {name:?} as {escaped:?}: {error:?}"
        );
        assert!(
            detail.contains(&escaped.escape_debug().to_string()) || detail.contains(&escaped),
            "the Suspension names it too: {detail}"
        );
    }
}

#[test]
fn adversary_the_executors_own_credentials_never_reach_the_secret_route() {
    let scratch = Scratch::new("own-credentials");
    for credentials in [
        CredentialSource::None,
        CredentialSource::OperatorLogin,
        CredentialSource::Loopback,
        CredentialSource::ApiKey,
    ] {
        let (session, seen) = Session::new();
        let mut executor = scratch.executor(Kind::B10x, session, Some(SECRET));
        executor.credentials = credentials;
        let step = step_of(&executor);
        let _ = proposed(&executor, &step);
        let seen = seen.borrow();
        assert_eq!(
            flag_value(&seen.args, "--api-key-secret"),
            Some(SECRET),
            "{credentials:?}: {:?}",
            seen.args
        );
        assert_eq!(
            credential_flags(&seen.args),
            vec!["--api-key-secret"],
            "{credentials:?}: one credential flag: {:?}",
            seen.args
        );
    }
}

#[test]
fn adversary_each_name_refusal_is_before_the_copy() {
    let scratch = Scratch::new("before-copy");
    let token = scratch.0.join("token.json");
    std::fs::write(&token, "{\"token\": \"scripted\"}\n").expect("a token file");
    for (kind, token_file, name) in [
        (Kind::Codex, None, SECRET),
        (Kind::Claude, None, SECRET),
        (Kind::B10x, Some(token.clone()), SECRET),
        (Kind::B10x, None, "no-slash"),
        (Kind::B10x, None, ""),
    ] {
        let (session, seen) = Session::new();
        let mut executor = scratch.executor(kind, session, Some(name));
        if let Some(file) = token_file {
            executor.endpoint.as_mut().expect("an endpoint").token_file = file;
        }
        let _ = refusal_before_the_copy(&scratch, &executor, &seen);
    }
}

#[test]
fn adversary_a_secret_with_no_endpoint_starts_nothing_and_leaves_no_copy() {
    let scratch = Scratch::new("no-endpoint");
    let (session, seen) = Session::new();
    let mut executor = scratch.executor(Kind::B10x, session, Some(SECRET));
    executor.endpoint = None;
    let step = step_of(&executor);
    assert!(
        matches!(
            &step.outcome,
            ExecutorOutcome::Suspended(suspended)
                if matches!(suspended.reason, SuspensionReason::ExternalAvailability(_))
        ),
        "{:?}",
        step.outcome
    );
    assert!(!seen.borrow().started, "no session is started");
    let error = executor
        .last_report()
        .and_then(|report| report.error)
        .expect("the report says why");
    assert!(error.contains("endpoint"), "{error:?}");
    assert_eq!(scratch.scratch_entries(), Vec::<PathBuf>::new());
}

#[test]
fn adversary_a_loop_that_read_its_key_elsewhere_is_never_proposed() {
    let scratch = Scratch::new("other-road");
    for source in ["api-key:environment", "api-key:file", "none", "oauth:file"] {
        let (session, seen) = Session::stating(source);
        let executor = scratch.executor(Kind::B10x, session, Some(SECRET));
        let step = step_of(&executor);
        assert!(seen.borrow().started, "{source}: the session was started");
        assert!(
            Proposal::from_outcome(&step.outcome).is_none(),
            "{source}: a loop that names another road than the store is not proposed: {:?}",
            step.outcome
        );
    }
}

#[test]
fn adversary_without_a_secret_the_subscription_route_is_unchanged() {
    let scratch = Scratch::new("subscription");
    let token = scratch.0.join("token.json");
    std::fs::write(&token, "{\"token\": \"scripted\"}\n").expect("a token file");
    let (session, seen) = Session::stating("none");
    let mut executor = scratch.executor(Kind::B10x, session, None);
    executor.credentials = CredentialSource::ApiKey;
    let endpoint = executor.endpoint.as_mut().expect("an endpoint");
    endpoint.token_file = token.clone();
    endpoint.token_pointer = Some("/token".to_owned());
    let step = step_of(&executor);
    let _ = proposed(&executor, &step);
    let seen = seen.borrow();
    let path = token.display().to_string();
    assert_eq!(
        flag_value(&seen.args, "--oauth-token-file"),
        Some(path.as_str())
    );
    assert_eq!(
        flag_value(&seen.args, "--oauth-token-pointer"),
        Some("/token")
    );
    assert_eq!(credential_flags(&seen.args), vec!["--oauth-token-file"]);
}

/// Without a secret, an endpoint whose `token_file` is empty (the secret route's spelling of "no
/// token") names no credential at all; the call must not launch the loop pointed at an empty path.
/// Since story:session-executor-keyless-loopback a keyless endpoint on a loopback host is admitted
/// with no credential (tests/endpoint_keyless.rs), so this case points at a host that is not one.
#[test]
fn adversary_an_empty_token_file_without_a_secret_starts_no_session() {
    let scratch = Scratch::new("empty-token");
    let (session, seen) = Session::stating("none");
    let mut executor = scratch.executor(Kind::B10x, session, None);
    executor.endpoint.as_mut().expect("an endpoint").base_url =
        "https://models.example.test/v1".to_owned();
    let step = step_of(&executor);
    let args = seen.borrow().args.clone();
    assert!(
        !seen.borrow().started,
        "story:session-executor-keyless-loopback: a call naming neither a secret nor a token \
         file, at a host that is not loopback, starts no session; it launched {:?} and ended {:?}",
        args,
        step.outcome
    );
}

/// Starts this test binary again running `test`, with the synthetic environment the probe reads.
fn reexec(test: &str) {
    let output = std::process::Command::new(std::env::current_exe().expect("the test binary"))
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(PROBE, "1")
        .env("OPENAI_API_KEY", SENTINEL)
        .env("ANTHROPIC_API_KEY", SENTINEL)
        .env("B10X_API_KEY", SENTINEL)
        .env("LOOM_API_KEY", SENTINEL)
        .env("DBUS_SESSION_BUS_ADDRESS", BUS)
        .env("XDG_RUNTIME_DIR", RUNTIME)
        .output()
        .expect("the probe runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "the probe {test} failed:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("probe ran"),
        "the probe {test} ran its body:\n{stdout}"
    );
}

fn holds_no_sentinel(seen: &Seen) {
    for (name, value) in &seen.env {
        assert!(
            !value.contains(SENTINEL),
            "the caller's key reached the child's environment as {name}"
        );
    }
    for arg in &seen.args {
        assert!(!arg.contains(SENTINEL), "the caller's key reached argv");
    }
    for variable in [
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "B10X_API_KEY",
        "LOOM_API_KEY",
        PROBE,
    ] {
        assert!(
            !seen.env.contains_key(variable),
            "{variable} is not inherited: {:?}",
            seen.env.keys().collect::<Vec<_>>()
        );
    }
}

#[test]
fn adversary_no_key_of_the_caller_reaches_the_child_on_the_secret_route() {
    reexec("probe_secret_route_environment");
}

#[test]
fn adversary_the_session_bus_is_forwarded_only_on_the_secret_route() {
    reexec("probe_subscription_route_environment");
}

#[test]
fn probe_secret_route_environment() {
    if std::env::var_os(PROBE).is_none() {
        return;
    }
    let scratch = Scratch::new("probe-secret");
    let (session, seen) = Session::new();
    let mut executor = scratch.executor(Kind::B10x, session, Some(SECRET));
    executor.credentials = CredentialSource::ApiKey;
    let step = step_of(&executor);
    let _ = proposed(&executor, &step);
    let seen = seen.borrow();
    holds_no_sentinel(&seen);
    assert_eq!(
        seen.env.get("DBUS_SESSION_BUS_ADDRESS").map(String::as_str),
        Some(BUS),
        "the session bus is forwarded by name: {:?}",
        seen.env.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        seen.env.get("XDG_RUNTIME_DIR").map(String::as_str),
        Some(RUNTIME)
    );
    let allowed = [
        "PATH",
        "XDG_CONFIG_HOME",
        "DBUS_SESSION_BUS_ADDRESS",
        "XDG_RUNTIME_DIR",
    ];
    let extra: Vec<&String> = seen
        .env
        .keys()
        .filter(|name| !allowed.contains(&name.as_str()))
        .collect();
    assert!(extra.is_empty(), "nothing else is forwarded: {extra:?}");
    println!("probe ran");
}

#[test]
fn probe_subscription_route_environment() {
    if std::env::var_os(PROBE).is_none() {
        return;
    }
    let scratch = Scratch::new("probe-subscription");
    let token = scratch.0.join("token.json");
    std::fs::write(&token, "{\"token\": \"scripted\"}\n").expect("a token file");
    let (session, seen) = Session::stating("none");
    let mut executor = scratch.executor(Kind::B10x, session, None);
    executor.credentials = CredentialSource::ApiKey;
    executor.endpoint.as_mut().expect("an endpoint").token_file = token;
    let step = step_of(&executor);
    let _ = proposed(&executor, &step);
    let seen = seen.borrow();
    holds_no_sentinel(&seen);
    for variable in ["DBUS_SESSION_BUS_ADDRESS", "XDG_RUNTIME_DIR"] {
        assert!(
            !seen.env.contains_key(variable),
            "{variable} is forwarded only on the secret route"
        );
    }
    println!("probe ran");
}
