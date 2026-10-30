//! Security conformance cases for `story:session-executor-keyless-loopback`: a b10x-harness
//! endpoint naming no key at all is admitted only on a loopback host, and the loop is then
//! pointed at no credential.
//!
//! What these measure that `endpoint_keyless.rs` does not:
//!
//! - the keyless route overrides whatever `SessionExecutor::credentials` the caller left set, so a
//!   caller's `ApiKey` (metaharness's `--api-key-env OPENAI_API_KEY` road) never reaches the loop;
//! - with real keys in the caller's environment, none reaches the child's environment or argv,
//!   and the child's environment is metaharness's constructed base only;
//! - the launch hands the loop the string the executor judged, verbatim, once;
//! - a loop that states it read an API key on this route is never proposed;
//! - host spellings a resolver or a non-WHATWG reader could take elsewhere are refused with the
//!   error a keyless endpoint always had, before any copy.
//!
//! Every session is scripted (metaharness's `ScriptedRunner`): no harness program runs, no model is
//! called, no credential file is read and nothing reaches the network. The environment case starts
//! this test binary again with a synthetic environment, so the caller's variables are the test's
//! own.

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
const RELAY: &str = "http://127.0.0.1:8401/v1";
/// The error a keyless endpoint the executor does not admit carries, as it did before the story.
const REFUSAL: &str = "a b10x-harness session's endpoint names no token file and no API key \
                       secret is named; name one of the two";
/// Every credential flag b10x-harness reads, bare or attached.
const CREDENTIAL_FLAGS: &[&str] = &[
    "--api-key-secret",
    "--api-key-env",
    "--api-key-file",
    "--oauth-token-file",
    "--oauth-token-env",
    "--oauth-token-pointer",
];

/// Set only in the re-started test binary: the environment case runs its body there.
const PROBE: &str = "LOOM_SECURITY_W20261030W2_U1_PROBE";
/// A synthetic key value placed in the re-started binary's environment; it must reach nothing.
const SENTINEL: &str = "sk-security-w20261030w2-sentinel-000000";
const BUS: &str = "unix:path=/run/user/4243/bus-security";
const RUNTIME: &str = "/run/user/4243";
const KEY_VARIABLES: [&str; 5] = [
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "B10X_API_KEY",
    "LOOM_API_KEY",
    "CODEX_API_KEY",
];

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A directory of the test's own under the system's temporary directory: `tree/`, the workspace
/// with `src/lib.rs`, and `scratch/` for the copies; removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-security-w20261030w2-u1-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    /// A b10x-harness executor over the tree, its loop pointed at `base_url` with no token file,
    /// no pointer and no secret, the caller's credential source left at `credentials`.
    fn executor(
        &self,
        session: Session,
        base_url: &str,
        credentials: CredentialSource,
    ) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(Kind::B10x)
                .with_runner(Box::new(session));
        executor.credentials = credentials;
        executor.scratch = self.0.join("scratch");
        executor.endpoint = Some(Endpoint {
            base_url: base_url.to_owned(),
            wire: "openai-responses".to_owned(),
            model: "synthetic".to_owned(),
            token_file: PathBuf::new(),
            token_pointer: None,
        });
        executor.api_key_secret = None;
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
    credential_copies: usize,
}

/// A scripted b10x-harness session whose opening record states `credential_source` as `source`.
struct Session {
    seen: Rc<RefCell<Seen>>,
    log: ScriptedLog,
    source: &'static str,
}

impl Session {
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
            credential_copies: plan.credential_copies.len(),
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
        principal: PrincipalId("principal-security-keyless".to_owned()),
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
    let case = CaseId("case-security-keyless".to_owned());
    executor.run_with_usage(&commission(&case), &frontier(&case))
}

/// Every value `flag` carries in `args`, bare (`--flag value`) or attached (`--flag=value`).
fn flag_values<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
    let attached = format!("{flag}=");
    args.iter()
        .enumerate()
        .filter_map(|(at, arg)| {
            if arg == flag {
                Some(args.get(at + 1).map_or("", String::as_str))
            } else {
                arg.strip_prefix(&attached)
            }
        })
        .collect()
}

fn credential_flags(args: &[String]) -> Vec<&str> {
    CREDENTIAL_FLAGS
        .iter()
        .copied()
        .filter(|flag| !flag_values(args, flag).is_empty())
        .collect()
}

fn proposed(executor: &SessionExecutor, step: &ExecutorStep, at: &str) -> Proposal {
    match Proposal::from_outcome(&step.outcome) {
        Some(read) => read.expect("a proposal's arguments"),
        None => panic!(
            "{at}: no proposal: {:?}; the session reported {:?}",
            step.outcome,
            executor.last_report().map(|report| report.error)
        ),
    }
}

/// The caller's credential sources an executor can be left at; the keyless route overrides each.
const CALLER_SOURCES: [CredentialSource; 4] = [
    CredentialSource::None,
    CredentialSource::ApiKey,
    CredentialSource::OperatorLogin,
    CredentialSource::Loopback,
];

/// Invariant 4: whatever source the caller left on the executor, a keyless loopback launch names
/// no credential, copies none, and hands the loop the endpoint string it judged, once.
#[test]
fn security_the_keyless_route_overrides_the_callers_credential_source() {
    for (at, credentials) in CALLER_SOURCES.into_iter().enumerate() {
        let scratch = Scratch::new(&format!("override-{at}"));
        let (session, seen) = Session::stating("none");
        let executor = scratch.executor(session, RELAY, credentials);
        let step = step_of(&executor);
        let proposal = proposed(&executor, &step, &format!("{credentials:?}"));
        assert_eq!(proposal.action, EDIT);
        let seen = seen.borrow();
        assert!(seen.started, "{credentials:?}: the session was started");
        assert_eq!(
            credential_flags(&seen.args),
            Vec::<&str>::new(),
            "story:session-executor-keyless-loopback: with the caller's source left at \
             {credentials:?}, the keyless loop is pointed at no credential: {:?}",
            seen.args
        );
        assert_eq!(
            seen.credential_copies, 0,
            "{credentials:?}: no credential is copied for a keyless loop"
        );
        assert_eq!(
            flag_values(&seen.args, "--base-url"),
            vec![RELAY],
            "{credentials:?}: the loop is handed the admitted string, verbatim and once: {:?}",
            seen.args
        );
    }
}

/// Starts this test binary again running `test`, with real-looking keys in its environment.
fn reexec(test: &str) {
    let mut command = std::process::Command::new(std::env::current_exe().expect("the binary"));
    command
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(PROBE, "1")
        .env("DBUS_SESSION_BUS_ADDRESS", BUS)
        .env("XDG_RUNTIME_DIR", RUNTIME)
        .env("HTTP_PROXY", "http://proxy.example.test:3128")
        .env("HTTPS_PROXY", "http://proxy.example.test:3128")
        .env("ALL_PROXY", "http://proxy.example.test:3128");
    for variable in KEY_VARIABLES {
        command.env(variable, SENTINEL);
    }
    let output = command.output().expect("the probe runs");
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

/// Invariant 4: with keys, a session bus and proxies in the caller's environment, the keyless
/// child's environment is metaharness's constructed base (`PATH`, scratch `XDG_CONFIG_HOME`) and
/// nothing else, whatever source the caller left on the executor.
#[test]
fn security_no_key_of_the_caller_reaches_the_child_on_the_keyless_route() {
    reexec("probe_keyless_route_environment");
}

#[test]
fn probe_keyless_route_environment() {
    if std::env::var_os(PROBE).is_none() {
        return;
    }
    for (at, credentials) in CALLER_SOURCES.into_iter().enumerate() {
        let scratch = Scratch::new(&format!("probe-{at}"));
        let (session, seen) = Session::stating("none");
        let executor = scratch.executor(session, RELAY, credentials);
        let step = step_of(&executor);
        let _ = proposed(&executor, &step, &format!("{credentials:?}"));
        let seen = seen.borrow();
        for (name, value) in &seen.env {
            assert!(
                !value.contains(SENTINEL),
                "{credentials:?}: the caller's key reached the child's environment as {name}"
            );
        }
        for arg in &seen.args {
            assert!(
                !arg.contains(SENTINEL),
                "{credentials:?}: the caller's key reached argv"
            );
        }
        let extra: Vec<&String> = seen
            .env
            .keys()
            .filter(|name| !["PATH", "XDG_CONFIG_HOME"].contains(&name.as_str()))
            .collect();
        assert!(
            extra.is_empty(),
            "story:session-executor-keyless-loopback: with the caller's source left at \
             {credentials:?}, the keyless child is given only PATH and XDG_CONFIG_HOME; it was \
             also given {extra:?}"
        );
    }
    println!("probe ran");
}

/// Invariant 1, the executor's half: a keyless loop that states it read an API key from anywhere
/// is never proposed (H4 on a run that declared no key).
#[test]
fn security_a_keyless_loop_that_read_an_api_key_is_never_proposed() {
    for (at, source) in ["api-key:environment", "api-key:file", "api-key:secrets"]
        .into_iter()
        .enumerate()
    {
        let scratch = Scratch::new(&format!("read-key-{at}"));
        let (session, seen) = Session::stating(source);
        let executor = scratch.executor(session, RELAY, CredentialSource::None);
        let step = step_of(&executor);
        assert!(seen.borrow().started, "{source}: the session was started");
        assert!(
            Proposal::from_outcome(&step.outcome).is_none(),
            "{source}: a keyless loop that names a key it read is not proposed: {:?}",
            step.outcome
        );
    }
}

/// Invariants 2 and 3: host spellings that are not loopback as the WHATWG URL reader b10x-harness
/// uses sees them, or that a resolver may send elsewhere (`localhost.` is an absolute name a
/// resolver may take to DNS), suspend the step with today's error, before any session and copy.
#[test]
fn security_spellings_that_leave_loopback_are_refused_before_the_copy() {
    for (at, base_url) in [
        "http://localhost./v1",
        "http://LOCALHOST.:8401/v1",
        "http://localhost.localdomain:8401/v1",
        "http://ip6-localhost:8401/v1",
        "http://models.example.test\\@127.0.0.1:8401/v1",
        "http://models.example.test\\@localhost:8401/v1",
        "http://127.0.0.1%2F@models.example.test/v1",
        "http://[::1]:8401@models.example.test/v1",
        "http://localhost:8401@127.0.0.1.example.test/v1",
        "http://127.0.0.1:8401:@models.example.test/v1",
        "ws://127.0.0.1:8401/v1",
        "file://localhost/v1",
        "http+unix://127.0.0.1/v1",
    ]
    .into_iter()
    .enumerate()
    {
        let scratch = Scratch::new(&format!("leave-{at}"));
        let (session, seen) = Session::stating("none");
        let executor = scratch.executor(session, base_url, CredentialSource::ApiKey);
        let step = step_of(&executor);
        assert!(
            matches!(
                &step.outcome,
                ExecutorOutcome::Suspended(suspended)
                    if matches!(suspended.reason, SuspensionReason::ExternalAvailability(_))
            ),
            "{base_url}: a keyless endpoint off loopback suspends the step, got {:?}",
            step.outcome
        );
        assert!(
            !seen.borrow().started,
            "{base_url}: no session is started; it launched {:?}",
            seen.borrow().args
        );
        let report = executor.last_report().expect("a report");
        assert_eq!(report.cwd, None, "{base_url}: no copy was made");
        assert_eq!(
            scratch.scratch_entries(),
            Vec::<PathBuf>::new(),
            "{base_url}: nothing is left in the scratch root"
        );
        assert_eq!(
            report.error.as_deref(),
            Some(REFUSAL),
            "{base_url}: refused with the error a keyless endpoint always had"
        );
    }
}
