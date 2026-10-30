//! A b10x-harness session whose endpoint names no key at all: no token file, no token pointer and
//! no secret (`story:session-executor-keyless-loopback`). In a credential-free pod the agent
//! reaches the model through the boundary's loopback relay, which adds the credential, so the
//! endpoint is admitted when its host is a loopback address (`127.0.0.0/8`, `::1`, `localhost`)
//! and the loop is launched with no credential flag. Any other keyless endpoint is refused before
//! the copy with the error it always had, and a keyed endpoint is unchanged.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, b10x-harness's own event
//! lines): no harness program runs, no model is called and nothing reaches the network.

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
/// The error a keyless endpoint the executor does not admit carries, as it did before.
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

/// A directory of the test's own under the system's temporary directory: `tree/`, the workspace
/// with `src/lib.rs`, and `scratch/` for the copies; removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-endpoint-keyless-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    /// A b10x-harness executor over the tree, started by `session`, its loop pointed at
    /// `base_url` with no token file, no pointer and no secret.
    fn executor(&self, session: Session, base_url: &str) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(Kind::B10x)
                .with_runner(Box::new(session));
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

#[derive(Default)]
struct Seen {
    started: bool,
    args: Vec<String>,
    env: BTreeMap<String, String>,
}

/// A scripted b10x-harness session that states no credential source: started, it keeps what it
/// was started with, edits `src/lib.rs` in its copy and prints its own event lines.
struct Session {
    seen: Rc<RefCell<Seen>>,
    log: ScriptedLog,
}

impl Session {
    fn new() -> (Self, Rc<RefCell<Seen>>) {
        let seen = Rc::<RefCell<Seen>>::default();
        (
            Self {
                seen: Rc::clone(&seen),
                log: ScriptedLog::new(),
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
        ScriptedRunner::new(lines(), self.log.clone()).start(plan)
    }
}

fn lines() -> Vec<ScriptStep> {
    let action = serde_json::json!({"action": EDIT, "summary": "fixed a"}).to_string();
    [
        serde_json::json!({"kind": "started", "model": "synthetic", "published_tools": [],
                           "credential_source": "none"}),
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
        principal: PrincipalId("principal-endpoint-keyless".to_owned()),
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
    let case = CaseId("case-endpoint-keyless".to_owned());
    executor.run_with_usage(&commission(&case), &frontier(&case))
}

/// Every credential flag `args` carries, bare or attached.
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

/// The step suspended for an external availability, the session never started, no copy made,
/// and the report's error is `REFUSAL`, unchanged.
fn refused_with_todays_error(scratch: &Scratch, base_url: &str) {
    let (session, seen) = Session::new();
    let executor = scratch.executor(session, base_url);
    let step = step_of(&executor);
    assert!(
        matches!(
            &step.outcome,
            ExecutorOutcome::Suspended(suspended)
                if matches!(suspended.reason, SuspensionReason::ExternalAvailability(_))
        ),
        "a keyless endpoint at {base_url} suspends the step, got {:?}",
        step.outcome
    );
    assert!(
        !seen.borrow().started,
        "a keyless endpoint at {base_url} starts no session; it launched {:?}",
        seen.borrow().args
    );
    let error = executor
        .last_report()
        .and_then(|report| report.error)
        .expect("the report says why");
    assert_eq!(error, REFUSAL, "{base_url} is refused with today's error");
    assert_eq!(scratch.copies(), 0, "{base_url}: no copy is made");
}

#[test]
fn a_keyless_loopback_endpoint_launches_the_loop_with_no_credential() {
    for (at, base_url) in [
        "http://127.0.0.1:8401/v1",
        "http://[::1]:8401/v1",
        "http://localhost:8401/v1",
        "http://LOCALHOST:8401/v1",
        "http://127.0.0.2:8401/v1",
        "https://127.0.0.1:8401/v1",
    ]
    .into_iter()
    .enumerate()
    {
        let scratch = Scratch::new(&format!("loopback-{at}"));
        let (session, seen) = Session::new();
        let executor = scratch.executor(session, base_url);

        let step = step_of(&executor);

        let proposal = match Proposal::from_outcome(&step.outcome) {
            Some(read) => read.expect("a proposal's arguments"),
            None => panic!(
                "{base_url}: no proposal: {:?}; the session reported {:?}",
                step.outcome,
                executor.last_report().map(|report| report.error)
            ),
        };
        assert_eq!(proposal.action, EDIT);
        let seen = seen.borrow();
        assert!(seen.started, "{base_url}: the session was started");
        assert_eq!(
            credential_flags(&seen.args),
            Vec::<&str>::new(),
            "{base_url}: the loop is pointed at no credential: {:?}",
            seen.args
        );
        for variable in ["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "B10X_API_KEY"] {
            assert!(
                !seen.env.contains_key(variable),
                "{base_url}: the child's environment holds no key ({variable})"
            );
        }
    }
}

#[test]
fn a_keyless_non_loopback_endpoint_is_refused() {
    for (at, base_url) in [
        "https://models.example.test/v1",
        "http://10.0.0.1:8401/v1",
        "http://127.0.0.1.example.test/v1",
        "http://localhost.example.test:8401/v1",
        "http://127.0.0.1:8401@models.example.test/v1",
        "http://localhost@models.example.test/v1",
        "http://[::ffff:127.0.0.1]:8401/v1",
        "http://[::]:8401/v1",
        "http://0.0.0.0:8401/v1",
        "127.0.0.1:8401/v1",
        "unix:///run/model.sock",
        "",
    ]
    .into_iter()
    .enumerate()
    {
        let scratch = Scratch::new(&format!("foreign-{at}"));
        refused_with_todays_error(&scratch, base_url);
    }
}

/// A loopback URL that carries credentials of its own is not keyless.
#[test]
fn a_keyless_loopback_endpoint_with_userinfo_is_refused() {
    for (at, base_url) in [
        "http://user:secret@127.0.0.1:8401/v1",
        "http://user@localhost:8401/v1",
    ]
    .into_iter()
    .enumerate()
    {
        let scratch = Scratch::new(&format!("userinfo-{at}"));
        refused_with_todays_error(&scratch, base_url);
    }
}

/// A pointer into a token file that is not named is no keyless endpoint: it is refused as before.
#[test]
fn a_keyless_loopback_endpoint_with_a_pointer_is_refused() {
    let scratch = Scratch::new("pointer");
    let (session, seen) = Session::new();
    let mut executor = scratch.executor(session, "http://127.0.0.1:8401/v1");
    executor
        .endpoint
        .as_mut()
        .expect("an endpoint")
        .token_pointer = Some("/token".to_owned());
    let step = step_of(&executor);
    assert!(
        matches!(&step.outcome, ExecutorOutcome::Suspended(_)),
        "a pointer with no token file suspends the step, got {:?}",
        step.outcome
    );
    assert!(!seen.borrow().started, "no session is started");
    let error = executor
        .last_report()
        .and_then(|report| report.error)
        .expect("the report says why");
    assert_eq!(error, REFUSAL);
}

/// A loopback endpoint that names its token file still points the loop at it.
#[test]
fn a_keyed_loopback_endpoint_is_unchanged() {
    let scratch = Scratch::new("keyed");
    let token = scratch.0.join("token.json");
    std::fs::write(&token, "{\"token\": \"scripted\"}\n").expect("a token file");
    let (session, seen) = Session::new();
    let mut executor = scratch.executor(session, "http://127.0.0.1:8401/v1");
    executor.endpoint.as_mut().expect("an endpoint").token_file = token.clone();
    let step = step_of(&executor);
    assert!(
        Proposal::from_outcome(&step.outcome).is_some(),
        "a keyed endpoint proposes, got {:?}",
        step.outcome
    );
    let seen = seen.borrow();
    assert_eq!(credential_flags(&seen.args), vec!["--oauth-token-file"]);
}
