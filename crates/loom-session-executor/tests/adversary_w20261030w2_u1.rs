//! Adversary cases for `story:session-executor-keyless-loopback` (wave 2026-10-30-w2, unit 1).
//!
//! The claim under attack: a b10x-harness endpoint naming no secret, no token file and no token
//! pointer is admitted only when `base_url`'s parsed host is loopback (`127.0.0.0/8`, `::1`,
//! `localhost`), and the loop is then launched with no credential flag; every other keyless
//! endpoint is refused before any copy with the error it always had; keyed endpoints and Claude
//! Code / Codex sessions are unchanged.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, or a runner that records the
//! launch and refuses to start): no harness program runs, no model is called, nothing reaches the
//! network, and no credential file is read.

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
/// The error a keyless endpoint carried before the story (`703a283c`, `check_api_key_secret`).
const REFUSAL: &str = "a b10x-harness session's endpoint names no token file and no API key \
                       secret is named; name one of the two";
const CREDENTIAL_FLAGS: &[&str] = &[
    "--oauth-token-file",
    "--oauth-token-env",
    "--oauth-token-pointer",
    "--api-key-env",
    "--api-key-file",
    "--api-key-secret",
];

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-adversary-w20261030w2-u1-{}-{name}",
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
        base_url: &str,
    ) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(kind)
                .with_runner(runner);
        executor.credentials = CredentialSource::None;
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

    fn copies(&self) -> usize {
        std::fs::read_dir(self.0.join("scratch")).map_or(0, Iterator::count)
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

/// A scripted b10x-harness session: records its launch, edits `src/lib.rs`, prints its lines.
struct Session {
    seen: Rc<RefCell<Seen>>,
    log: ScriptedLog,
    /// When false, the launch is recorded and the start fails: for kinds whose lines are not
    /// scripted here.
    runs: bool,
    /// The `credential_source` the scripted loop records in its opening line.
    source: &'static str,
}

impl Session {
    fn plain(runs: bool) -> (Box<dyn ProcessRunner>, Rc<RefCell<Seen>>) {
        Self::recording(runs, "none")
    }

    fn recording(runs: bool, source: &'static str) -> (Box<dyn ProcessRunner>, Rc<RefCell<Seen>>) {
        let seen = Rc::<RefCell<Seen>>::default();
        (
            Box::new(Self {
                seen: Rc::clone(&seen),
                log: ScriptedLog::new(),
                runs,
                source,
            }),
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
        if !self.runs {
            return Err(std::io::Error::other(
                "adversary: launch recorded, not started",
            ));
        }
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
        principal: PrincipalId("principal-adversary-keyless".to_owned()),
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
    let case = CaseId("case-adversary-keyless".to_owned());
    executor.run_with_usage(&commission(&case), &frontier(&case))
}

fn credential_flags(args: &[String]) -> Vec<&str> {
    CREDENTIAL_FLAGS
        .iter()
        .copied()
        .filter(|flag| {
            let attached = format!("{flag}=");
            args.iter()
                .any(|arg| arg == flag || arg.starts_with(&attached))
        })
        .collect()
}

fn value_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|at| args.get(at + 1))
        .map(String::as_str)
}

/// Whether the url crate (the parser reqwest, and so the loop's HTTP client, reads a URL with)
/// dials a loopback address for `base_url`.
fn dials_loopback(base_url: &str) -> bool {
    match url::Url::parse(base_url)
        .ok()
        .and_then(|url| url.host().map(|h| h.to_owned()))
    {
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        Some(url::Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        None => false,
    }
}

/// The session ran and proposed, the loop was given no credential flag, `--base-url` is
/// `base_url` verbatim and dials loopback, and the child's environment names no key.
fn admitted_with_no_credential(scratch: &Scratch, base_url: &str, credentials: CredentialSource) {
    let (session, seen) = Session::plain(true);
    let mut executor = scratch.executor(Kind::B10x, session, base_url);
    executor.credentials = credentials;
    let step = step_of(&executor);
    let proposed = Proposal::from_outcome(&step.outcome).is_some();
    assert!(
        proposed,
        "{base_url:?} ({credentials:?}): no proposal: {:?}; the session reported {:?}",
        step.outcome,
        executor.last_report().map(|report| report.error)
    );
    let seen = seen.borrow();
    assert!(seen.started, "{base_url:?}: the session was started");
    assert_eq!(
        credential_flags(&seen.args),
        Vec::<&str>::new(),
        "{base_url:?} ({credentials:?}): the loop is pointed at no credential: {:?}",
        seen.args
    );
    assert!(
        !seen
            .args
            .iter()
            .any(|arg| arg.contains("api-key") || arg.contains("oauth")),
        "{base_url:?} ({credentials:?}): no credential word on argv: {:?}",
        seen.args
    );
    let dialled = value_after(&seen.args, "--base-url");
    assert_eq!(
        dialled,
        Some(base_url),
        "{base_url:?}: the loop gets base_url verbatim"
    );
    assert!(
        dials_loopback(base_url),
        "{base_url:?}: the loop's parser dials loopback"
    );
    let keyed: Vec<&String> = seen
        .env
        .keys()
        .filter(|name| {
            let upper = name.to_ascii_uppercase();
            upper.contains("KEY") || upper.contains("TOKEN") || upper.contains("SECRET")
        })
        .collect();
    assert!(
        keyed.is_empty(),
        "{base_url:?} ({credentials:?}): the child's environment names no key: {keyed:?}"
    );
}

fn refused_with_todays_error(scratch: &Scratch, base_url: &str) {
    let (session, seen) = Session::plain(true);
    let executor = scratch.executor(Kind::B10x, session, base_url);
    let step = step_of(&executor);
    assert!(
        matches!(
            &step.outcome,
            ExecutorOutcome::Suspended(suspended)
                if matches!(suspended.reason, SuspensionReason::ExternalAvailability(_))
        ),
        "{base_url:?}: a keyless endpoint off loopback suspends the step, got {:?}",
        step.outcome
    );
    assert!(
        !seen.borrow().started,
        "{base_url:?}: a keyless endpoint off loopback starts no session; it launched {:?}",
        seen.borrow().args
    );
    let error = executor.last_report().and_then(|report| report.error);
    assert_eq!(
        error.as_deref(),
        Some(REFUSAL),
        "{base_url:?}: today's error"
    );
    assert_eq!(scratch.copies(), 0, "{base_url:?}: no copy is made");
}

/// Spellings the url crate reads as a loopback host (IPv4 shorthand, hex, octal, a decimal
/// integer, percent-encoding, uppercase scheme, full-width IDNA, backslashes, embedded tab, the
/// /8's last address, empty userinfo, an `@` after a backslash or in the fragment). Admitted with
/// no credential flag, and the loop, whose HTTP client parses with the same crate, dials loopback.
#[test]
fn adversary_spellings_the_url_crate_reads_as_loopback_are_admitted_and_dial_loopback() {
    for (at, base_url) in [
        "http://127.1:8401/v1",
        "http://0x7f.0.0.1:8401/v1",
        "http://2130706433:8401/v1",
        "http://0177.0.0.1:8401/v1",
        "http://127.255.255.255:8401/v1",
        "http://%6c%6f%63%61%6c%68%6f%73%74:8401/v1",
        "http://127%2E0%2E0%2E1:8401/v1",
        "HTTP://127.0.0.1:8401/v1",
        "http://\u{ff2c}\u{ff2f}\u{ff23}\u{ff21}\u{ff2c}\u{ff28}\u{ff2f}\u{ff33}\u{ff34}:8401/v1",
        "http:\\\\127.0.0.1:8401\\v1",
        "http://loc\talhost:8401/v1",
        "http://[0:0:0:0:0:0:0:1]:8401/v1",
        "http://127.0.0.1:8401/v1#@models.example.test",
        "http://127.0.0.1:8401\\@models.example.test/v1",
        "http://:@127.0.0.1:8401/v1",
        "http://@127.0.0.1:8401/v1",
    ]
    .into_iter()
    .enumerate()
    {
        let scratch = Scratch::new(&format!("loopback-spelling-{at}"));
        admitted_with_no_credential(&scratch, base_url, CredentialSource::None);
    }
}

/// A leading or trailing space: the url crate trims it, so the host is loopback.
#[test]
fn adversary_a_loopback_url_padded_with_spaces() {
    let scratch = Scratch::new("padded");
    admitted_with_no_credential(
        &scratch,
        " http://127.0.0.1:8401/v1 ",
        CredentialSource::None,
    );
}

/// Spellings whose host is not loopback, or that are not an http(s) URL with no user name and
/// no password: refused before the copy with today's error.
#[test]
fn adversary_spellings_off_loopback_are_refused_with_todays_error() {
    for (at, base_url) in [
        "http://127.0.0.1\t@models.example.test/v1",
        "http://models.example.test\\@127.0.0.1:8401/v1",
        "http://127.0.0.1%2Emodels.example.test/v1",
        "http://localhost%2Emodels.example.test/v1",
        "http://0x7f.0.0.1.example.test/v1",
        "http://[::1]@models.example.test/v1",
        "http://localhost./v1",
        "http://[::1%25lo]:8401/v1",
        "http://127.0.0.1:99999/v1",
        "http://local%00host:8401/v1",
        "http://128.0.0.1:8401/v1",
        "http://126.255.255.255:8401/v1",
        "http://[::2]:8401/v1",
        "http://[::ffff:7f00:1]:8401/v1",
        "http://user:@127.0.0.1:8401/v1",
        "ws://127.0.0.1:8401/v1",
        "ftp://localhost:8401/v1",
        "file://localhost/v1",
    ]
    .into_iter()
    .enumerate()
    {
        let scratch = Scratch::new(&format!("off-loopback-{at}"));
        refused_with_todays_error(&scratch, base_url);
    }
}

/// A loopback URL carrying a password and no user name names a key: refused. The unit's own
/// userinfo cases (`user:secret@`, `user@`) both carry a user name, so they stay red only on the
/// user-name half of the check; this one stays red only on the password half.
#[test]
fn adversary_a_loopback_url_with_a_password_and_no_user_is_refused() {
    let scratch = Scratch::new("password-only");
    refused_with_todays_error(&scratch, "http://:secret@127.0.0.1:8401/v1");
}

/// The scheme check is the only thing that refuses `ws://127.0.0.1`: the unit's own non-http
/// cases (`unix:///run/model.sock`, `127.0.0.1:8401/v1`) have no host or do not parse, so they
/// are refused with the scheme check removed too. This case is refused by the scheme check alone.
#[test]
fn adversary_the_units_refusal_list_does_not_reach_the_scheme_check() {
    let host_only = |base_url: &str| dials_loopback(base_url);
    for base_url in ["unix:///run/model.sock", "127.0.0.1:8401/v1", ""] {
        assert!(
            !host_only(base_url),
            "{base_url:?} is refused without the scheme check, so it does not test it"
        );
    }
    assert!(
        host_only("ws://127.0.0.1:8401/v1"),
        "only the scheme check refuses this one"
    );
    let scratch = Scratch::new("ws");
    refused_with_todays_error(&scratch, "ws://127.0.0.1:8401/v1");
}

/// The executor's own `credentials` is overwritten on the keyless route: whichever source the
/// embedder left set, the loop gets no credential flag and the child's environment no key.
#[test]
fn adversary_the_executors_credential_source_does_not_reach_a_keyless_launch() {
    for (at, credentials) in [
        CredentialSource::None,
        CredentialSource::OperatorLogin,
        CredentialSource::ApiKey,
        CredentialSource::Loopback,
    ]
    .into_iter()
    .enumerate()
    {
        let scratch = Scratch::new(&format!("source-{at}"));
        admitted_with_no_credential(&scratch, "http://127.0.0.1:8401/v1", credentials);
    }
}

/// A named secret beside a keyless loopback endpoint takes the secret route, not the keyless one.
#[test]
fn adversary_a_secret_beside_a_keyless_loopback_endpoint_takes_the_secret_route() {
    let scratch = Scratch::new("secret");
    let (session, seen) = Session::recording(true, "api-key:secrets");
    let mut executor = scratch.executor(Kind::B10x, session, "http://127.0.0.1:8401/v1");
    executor.api_key_secret = Some("llm/adversary-key".to_owned());
    let step = step_of(&executor);
    assert!(
        Proposal::from_outcome(&step.outcome).is_some(),
        "the secret route proposes, got {:?}; {:?}",
        step.outcome,
        executor.last_report().map(|report| report.error)
    );
    let seen = seen.borrow();
    assert_eq!(
        credential_flags(&seen.args),
        vec!["--api-key-secret"],
        "{:?}",
        seen.args
    );
    assert_eq!(
        value_after(&seen.args, "--api-key-secret"),
        Some("llm/adversary-key")
    );
}

/// A keyless loopback launch whose loop says it found a key anyway (`api-key:environment`) is
/// held back by the hermetic floor's H4 row: no proposal, the step suspended.
#[test]
fn adversary_a_keyless_launch_whose_loop_records_a_key_is_held_back() {
    let scratch = Scratch::new("loop-found-a-key");
    let (session, seen) = Session::recording(true, "api-key:environment");
    let executor = scratch.executor(Kind::B10x, session, "http://127.0.0.1:8401/v1");
    let step = step_of(&executor);
    assert!(seen.borrow().started, "the keyless launch was started");
    assert!(
        Proposal::from_outcome(&step.outcome).is_none(),
        "a loop that used an undeclared key proposes nothing, got {:?}",
        step.outcome
    );
    let error = executor
        .last_report()
        .and_then(|report| report.error)
        .unwrap_or_default();
    assert!(error.contains("H4"), "the H4 row holds it back: {error:?}");
}

/// A Codex session with a keyless endpoint off loopback set: the endpoint is not read, so the
/// keyless refusal is not raised and the endpoint never reaches the launch. A Claude Code session
/// reads its endpoint since `story:session-executor-claude-loopback-endpoint`, and one off loopback
/// is refused before the copy (`tests/claude_endpoint.rs`).
#[test]
fn adversary_codex_ignores_a_keyless_foreign_endpoint() {
    let foreign = "https://models.example.test/v1";
    for (at, kind) in [Kind::Codex].into_iter().enumerate() {
        let scratch = Scratch::new(&format!("other-kind-{at}"));
        let (session, seen) = Session::plain(false);
        let executor = scratch.executor(kind, session, foreign);
        let step = step_of(&executor);
        let error = executor.last_report().and_then(|report| report.error);
        assert_ne!(
            error.as_deref(),
            Some(REFUSAL),
            "{kind:?}: the keyless refusal is b10x-harness only; outcome {:?}",
            step.outcome
        );
        let seen = seen.borrow();
        assert!(
            seen.started,
            "{kind:?}: the launch reaches the runner; error {error:?}"
        );
        assert!(
            !seen
                .args
                .iter()
                .any(|arg| arg.contains("models.example.test")),
            "{kind:?}: the endpoint does not reach the launch: {:?}",
            seen.args
        );
        assert_eq!(credential_flags(&seen.args), Vec::<&str>::new(), "{kind:?}");
    }
}

/// A Claude Code session with a keyless endpoint off loopback set: refused before the copy with
/// an error of its own, never the b10x-harness keyless refusal, and nothing is launched.
#[test]
fn adversary_claude_refuses_a_keyless_foreign_endpoint_with_its_own_error() {
    let scratch = Scratch::new("claude-foreign");
    let (session, seen) = Session::plain(false);
    let executor = scratch.executor(Kind::Claude, session, "https://models.example.test/v1");
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
    assert!(
        !seen.borrow().started,
        "nothing is launched (story:session-executor-claude-loopback-endpoint)"
    );
    let error = executor
        .last_report()
        .and_then(|report| report.error)
        .expect("the report says why");
    assert_ne!(error, REFUSAL, "the b10x-harness refusal is not reused");
    assert!(
        error.contains("loopback"),
        "story:session-executor-claude-loopback-endpoint: {error}"
    );
}
