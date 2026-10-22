//! Security review of `story:checked-timeout-deadlines` in `ConnectorsCli`: the governed write
//! (`invoke_write`) takes its deadline with checked arithmetic as reads do. A `timeout_seconds`
//! the clock cannot hold, zero or negative starts no process, describe included; a large one the
//! clock can hold runs the write to its answer without a panic.
//!
//! The program is the fake `connectors` of `connectors_cli.rs`, copied into each test's directory
//! under `CARGO_TARGET_TMPDIR/security_w20261022w2_u1`; the admitted request is made by
//! Commission's own `run_until_blocked`, as in `connectors_cli_write.rs`.

use std::fs;
use std::iter::repeat_n;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeProposedAction, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_connectors::cli::datasource::{
    AdapterAlias, ConnectionId, ConnectorsCliConfig, MutationClassification, OperationId,
    WriteApproval, WriteOperation,
};
use b10x_loom_connectors::cli::{ConnectorsCli, WriteError, json};

const ADAPTER: &str = "desk";
const CONNECTION: &str = "conn-desk";
const ROUTE: &str = "ticket.route";
const ARGUMENTS: &str = r#"{"ticket":"48213","queue":"billing"}"#;
const NOW: &str = "2026-10-17T09:00:00Z";

/// A whole-second timeout a monotonic clock that started at boot can hold: half of `i64::MAX`.
const HOLDABLE: i64 = i64::MAX / 2;

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(test: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("security_w20261022w2_u1")
            .join(test);
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        for directory in ["state", "home", "approvals"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        let status = std::process::Command::new("/usr/bin/install")
            .args(["-m", "755"])
            .arg(fixtures().join("fake-connectors"))
            .arg(root.join("connectors"))
            .status()
            .unwrap();
        assert!(status.success());
        let fixture = Self { root };
        for (verb, file) in [
            ("describe", "describe-route.json"),
            ("invoke", "invoke-route-applied.json"),
        ] {
            fs::copy(
                fixtures().join(file),
                fixture.state().join(format!("{verb}.{ROUTE}.json")),
            )
            .unwrap();
        }
        fixture
    }

    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    fn cli(&self, timeout_seconds: Option<i64>) -> ConnectorsCli {
        ConnectorsCli::new(
            ConnectorsCliConfig {
                program: self.root.join("connectors").display().to_string(),
                config: None,
                state_dir: Some(self.state().display().to_string()),
                timeout_seconds,
            },
            Vec::new(),
        )
        .with_environment(vec![
            (
                "HOME".to_owned(),
                self.root.join("home").display().to_string(),
            ),
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ])
    }

    fn proof(&self) -> String {
        self.root
            .join("approvals/route.proof")
            .display()
            .to_string()
    }

    fn argv(&self) -> Vec<String> {
        fs::read_to_string(self.state().join("argv.log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/connectors")
}

fn route() -> WriteOperation {
    WriteOperation {
        action: ROUTE.to_owned(),
        adapter: AdapterAlias(ADAPTER.to_owned()),
        connection: ConnectionId(CONNECTION.to_owned()),
        operation: OperationId(ROUTE.to_owned()),
    }
}

fn input() -> json::Value {
    json::parse(ARGUMENTS).unwrap()
}

fn approval(path: String) -> WriteApproval {
    WriteApproval { proof_file: path }
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Context {
    requests: u64,
    observations: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x400 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(NOW.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// Keeps every request the runtime admitted and performs nothing.
#[derive(Default)]
struct Keep(Mutex<Vec<AdmittedRequest>>);

impl EffectPort for Keep {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: commission_json::Value::Null,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// The request the runtime admits for the route.
fn admitted() -> AdmittedRequest {
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x81)),
        agent_revision_id: AgentRevisionId(uuid(0x90)),
        case_id: CaseId("case-route".to_owned()),
        principal: PrincipalId("principal-desk".to_owned()),
        authority_context: AuthorityContext(commission_json::Value::Null),
    });
    let open = |revision| {
        Answer::at(revision).with_items(
            Vec::new(),
            Vec::new(),
            vec![FrontierAction {
                action: ROUTE.to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            }],
        )
    };
    let governor = FakeGovernor::new();
    governor.script(
        commission.data().case_id.clone(),
        repeat_n(open(3), 5).chain([open(4), open(4).complete("done")]),
    );
    let executor = ScriptedExecutor::new(vec![ExecutorOutcome::ProposedAction(
        ExecutorOutcomeProposedAction {
            action: ROUTE.to_owned(),
            arguments: ProposedActionArguments(commission_json::parse(ARGUMENTS).unwrap()),
        },
    )]);
    let keep = Keep::default();
    let mut issued = 0u64;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));
    let _ = run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &keep,
        &commission,
        &mut runs,
        &mut Context {
            requests: 0,
            observations: 0,
        },
    );
    let mut kept = keep.0.into_inner().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(kept.len(), 1, "the runtime admits the route once");
    kept.remove(0)
}

/// The governed write with the largest `timeout_seconds` is refused naming it, with no panic, and
/// starts no process: neither the describe nor the invoke.
#[test]
fn security_a_write_whose_timeout_the_clock_cannot_hold_starts_no_process() {
    let fixture = Fixture::new("write_past_the_clock");
    let request = admitted();
    let answer = fixture.cli(Some(i64::MAX)).invoke_write(
        &request,
        &route(),
        &input(),
        &approval(fixture.proof()),
    );
    match answer {
        Err(WriteError::Failed(why)) => assert!(
            why.contains(&format!("timeout_seconds {}", i64::MAX)),
            "the refusal names the timeout: {why}"
        ),
        other => panic!("the write is refused: {other:?}"),
    }
    assert!(
        fixture.argv().is_empty(),
        "a process started: {:?}",
        fixture.argv()
    );
}

/// A zero or negative `timeout_seconds` starts no write process either.
#[test]
fn security_a_write_whose_timeout_is_not_positive_starts_no_process() {
    for seconds in [0, -1, i64::MIN] {
        let fixture = Fixture::new(&format!("write_not_positive_{}", seconds.unsigned_abs()));
        let request = admitted();
        let answer = fixture.cli(Some(seconds)).invoke_write(
            &request,
            &route(),
            &input(),
            &approval(fixture.proof()),
        );
        assert!(
            matches!(answer, Err(WriteError::Failed(_))),
            "{seconds}: the write is refused: {answer:?}"
        );
        assert!(
            fixture.argv().is_empty(),
            "{seconds}: a process started: {:?}",
            fixture.argv()
        );
    }
}

/// A write whose timeout the clock can hold, however large, runs to its answer: the deadline and
/// every wait derived from it are no panic and no refusal.
#[test]
fn security_a_write_with_a_large_holdable_timeout_is_applied() {
    let fixture = Fixture::new("write_large_holdable");
    let request = admitted();
    let written = fixture
        .cli(Some(HOLDABLE))
        .invoke_write(&request, &route(), &input(), &approval(fixture.proof()))
        .expect("a timeout the clock holds runs the write");
    assert_eq!(written.classification, MutationClassification::Applied);
}
