//! Security cases for `SessionExecutor::api_key_secret` (story:endpoint-api-key-secret).
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, b10x-harness's own event
//! lines): no harness program runs, no model is called, no secret store is opened, no credential
//! file is read and nothing reaches the network.

use std::cell::RefCell;
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
    CredentialSource, Endpoint, Kind, ProcessRunner, SessionExecutor,
};

const EDIT: &str = "implementation.edit";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-security-w20261024w1-u1-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    fn executor(&self, session: Session, secret: &str) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(Kind::B10x)
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

#[derive(Default)]
struct Seen {
    started: bool,
    args: Vec<String>,
}

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
        };
        ScriptedRunner::new(lines(), self.log.clone()).start(plan)
    }
}

fn lines() -> Vec<ScriptStep> {
    let action = serde_json::json!({"action": EDIT, "summary": "fixed a"}).to_string();
    [
        serde_json::json!({"kind": "started", "model": "synthetic", "published_tools": [],
                           "credential_source": "api-key:secrets"}),
        serde_json::json!({"kind": "text-delta", "text": action}),
        serde_json::json!({"kind": "finished", "stop": {"kind": "completed"}}),
    ]
    .into_iter()
    .map(|line| ScriptStep::line(line.to_string()))
    .collect()
}

fn step_of(executor: &SessionExecutor) -> ExecutorStep {
    let case = CaseId("case-security-secret".to_owned());
    let commission = Commission::<commission_state::Assigned>::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-security-secret".to_owned()),
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

/// A name holding a NUL byte can never be carried by `execve`: a real spawn fails only after the
/// copy was made. The story promises a malformed name is refused before any copy or session, and
/// a name holding any other control character, a newline included, is refused the same way.
#[test]
fn security_a_name_holding_a_control_character_is_refused_before_the_copy() {
    let scratch = Scratch::new("nul");
    for name in ["llm/key\0name", "llm/a\nb", "llm/a\u{1b}b"] {
        let (session, seen) = Session::new();
        let executor = scratch.executor(session, name);
        let step = step_of(&executor);
        let launched = seen.borrow().args.clone();
        assert!(
            !seen.borrow().started,
            "a name holding a control character reached the launch plan {launched:?}; outcome {:?}",
            step.outcome
        );
        assert!(
            matches!(
                &step.outcome,
                ExecutorOutcome::Suspended(suspended)
                    if matches!(suspended.reason, SuspensionReason::ExternalAvailability(_))
            ),
            "{:?}",
            step.outcome
        );
        let report = executor.last_report().expect("a report");
        assert_eq!(report.cwd, None, "no copy was made: {:?}", report.cwd);
        let error = report.error.expect("the report says why");
        assert!(
            error.contains(&name.escape_debug().to_string()),
            "{error:?} names {name:?} escaped"
        );
    }
}

/// Whatever a well-shaped name holds (spaces, an `=`, a second `/`), it reaches the
/// loop as exactly one argument after its flag, never split into or read as another option.
#[test]
fn security_a_well_shaped_name_is_one_argument_after_its_flag() {
    let scratch = Scratch::new("one-argument");
    for name in [
        "llm/a b",
        "llm/a=--api-key-env=OPENAI_API_KEY",
        "llm/team/openai",
    ] {
        let (session, seen) = Session::new();
        let executor = scratch.executor(session, name);
        let _ = step_of(&executor);
        let seen = seen.borrow();
        assert!(seen.started, "{name:?}: the session started");
        let at = seen
            .args
            .iter()
            .position(|arg| arg == "--api-key-secret")
            .unwrap_or_else(|| panic!("{name:?}: the flag is passed: {:?}", seen.args));
        assert_eq!(seen.args.get(at + 1).map(String::as_str), Some(name));
        for flag in [
            "--api-key-env",
            "--api-key-file",
            "--oauth-token-file",
            "--oauth-token-env",
        ] {
            assert!(
                !seen.args.iter().any(|arg| arg == flag),
                "{name:?}: {flag} is never passed: {:?}",
                seen.args
            );
        }
    }
}
