//! A b10x-harness session whose endpoint's API key is named in the operator's secret store
//! (`SessionExecutor::api_key_secret`): the loop is launched with `--api-key-secret` and the
//! name, and the key enters neither Loom, metaharness nor the child's environment. A secret beside
//! a subscription token, for a vendor session, or not spelled `<namespace>/<name>` starts no
//! session.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, b10x-harness's own event
//! lines): no harness program runs, no model is called, no secret store is opened and nothing
//! reaches the network.

use std::cell::RefCell;
use std::collections::BTreeMap;
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
use b10x_loom_session_executor::{
    CredentialSource, Endpoint, Kind, ProcessRunner, Proposal, SessionExecutor,
};

const EDIT: &str = "implementation.edit";
const EDITED: &str = "pub fn a() -> i32 {\n    1\n}\n";
const SECRET: &str = "llm/loom-endpoint-key";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A directory of the test's own under the system's temporary directory: `tree/`, the workspace
/// with `src/lib.rs`, and `scratch/` for the copies; removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-endpoint-secret-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    /// An executor of `kind` over the tree, started by `session`, its loop (for b10x-harness)
    /// pointed at a closed local port with no subscription token, and `secret` as its key's name.
    fn executor(&self, kind: Kind, session: Session, secret: &str) -> SessionExecutor {
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
        executor.api_key_secret = Some(secret.to_owned());
        executor
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What a [`Session`] saw when it was started: its arguments and its environment, or nothing
/// when it was never started.
#[derive(Default)]
struct Seen {
    started: bool,
    args: Vec<String>,
    env: BTreeMap<String, String>,
}

/// A scripted b10x-harness session: started, it keeps what it was started with, edits
/// `src/lib.rs` in its copy and prints its own event lines, one call, completed.
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
        ScriptedRunner::new(b10x_lines(plan.cwd), self.log.clone()).start(plan)
    }
}

/// b10x-harness's own lines for one call: started, the reply naming the action, its usage, its
/// cost, finished.
fn b10x_lines(_cwd: &Path) -> Vec<ScriptStep> {
    let action = serde_json::json!({"action": EDIT, "summary": "fixed a"}).to_string();
    [
        serde_json::json!({"kind": "started", "model": "synthetic", "published_tools": [],
                           "credential_source": "secret"}),
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
        principal: PrincipalId("principal-endpoint-secret".to_owned()),
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
    let case = CaseId("case-endpoint-secret".to_owned());
    executor.run_with_usage(&commission(&case), &frontier(&case))
}

/// The value following `flag` in `args`, or the value of `flag=value`.
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

/// The step suspended for an external availability, the session never started, and the report's
/// error, which every one of `named` appears in.
fn refused_before_any_session(
    executor: &SessionExecutor,
    seen: &Rc<RefCell<Seen>>,
    named: &[&str],
) {
    let step = step_of(executor);
    assert!(
        matches!(
            &step.outcome,
            ExecutorOutcome::Suspended(suspended)
                if matches!(suspended.reason, SuspensionReason::ExternalAvailability(_))
        ),
        "a refused credential suspends the step, got {:?}",
        step.outcome
    );
    assert!(!seen.borrow().started, "no session is started");
    let error = executor
        .last_report()
        .and_then(|report| report.error)
        .expect("the report says why");
    for name in named {
        assert!(error.contains(name), "{error:?} names {name:?}");
    }
}

#[test]
fn a_named_secret_launches_the_loop_with_api_key_secret() {
    let scratch = Scratch::new("named");
    let (session, seen) = Session::new();
    let executor = scratch.executor(Kind::B10x, session, SECRET);

    let step = step_of(&executor);

    let proposal = match Proposal::from_outcome(&step.outcome) {
        Some(read) => read.expect("a proposal's arguments"),
        None => panic!(
            "no proposal: {:?}; the session reported {:?}",
            step.outcome,
            executor.last_report().map(|report| report.error)
        ),
    };
    assert_eq!(proposal.action, EDIT);
    let seen = seen.borrow();
    assert!(seen.started, "the session was started");
    assert_eq!(
        flag_value(&seen.args, "--api-key-secret"),
        Some(SECRET),
        "the loop is told the secret's name: {:?}",
        seen.args
    );
    for flag in [
        "--oauth-token-file",
        "--oauth-token-env",
        "--api-key-env",
        "--api-key-file",
    ] {
        assert!(
            flag_value(&seen.args, flag).is_none(),
            "no other credential flag ({flag}): {:?}",
            seen.args
        );
    }
    for variable in ["OPENAI_API_KEY", "ANTHROPIC_API_KEY"] {
        assert!(
            !seen.env.contains_key(variable),
            "the child's environment holds no key ({variable})"
        );
    }
}

#[test]
fn a_secret_beside_a_subscription_token_is_refused() {
    let scratch = Scratch::new("beside");
    let token = scratch.0.join("token.json");
    std::fs::write(&token, "{\"token\": \"scripted\"}\n").expect("a token file");

    let (session, seen) = Session::new();
    let mut executor = scratch.executor(Kind::B10x, session, SECRET);
    executor.endpoint.as_mut().expect("an endpoint").token_file = token;
    refused_before_any_session(&executor, &seen, &[SECRET, "token"]);

    let (session, seen) = Session::new();
    let mut executor = scratch.executor(Kind::B10x, session, SECRET);
    executor.endpoint.as_mut().expect("an endpoint").token_pointer = Some("/token".to_owned());
    refused_before_any_session(&executor, &seen, &[SECRET, "token"]);
}

#[test]
fn a_secret_for_a_vendor_session_is_refused() {
    let scratch = Scratch::new("vendor");
    for kind in [Kind::Claude, Kind::Codex] {
        let (session, seen) = Session::new();
        let mut executor = scratch.executor(kind, session, SECRET);
        executor.endpoint = None;
        refused_before_any_session(&executor, &seen, &[SECRET]);
    }
}

#[test]
fn a_secret_not_namespace_slash_name_is_refused() {
    let scratch = Scratch::new("shape");
    for name in ["no-slash", "/name", "ns/", ""] {
        let (session, seen) = Session::new();
        let executor = scratch.executor(Kind::B10x, session, name);
        refused_before_any_session(&executor, &seen, &[name]);
    }
}
