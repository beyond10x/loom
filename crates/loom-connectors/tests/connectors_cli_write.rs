//! Acceptance for `story:connectors-governed-write`: `ConnectorsCli::invoke_write` performs a write
//! an admitted request names through the `connectors` command line, `operations describe` then one
//! `operations invoke` with the person's approval proof (`--approval-file`) and an idempotency key
//! derived from the admitted request (`--idempotency-key`), and returns the write as Connectors
//! classifies it.
//!
//! The program is `tests/fixtures/connectors/fake-connectors`, as in `connectors_cli.rs`, copied
//! into each test's own directory under `CARGO_TARGET_TMPDIR/connectors_cli_write` (never under
//! `connectors_cli`, whose logs `a_resource_profile_read_is_invoked_without_an_approval_file`
//! scans for a read carrying `--approval-file`). It logs its argv and the document it read on
//! stdin. The invoke answers are the shapes `connectors --output json` prints for a write
//! (connectors `apps/connectors/src/local/operations.rs`, `project`: success adds `request_id`,
//! `mutation` and `source_audit`; a failure carries them in `error.data`).
//!
//! The admitted request is made by Commission's own `run_until_blocked` over the testkit's
//! scripted governor and executor: only the runtime makes one.

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
use b10x_loom_connectors::cli::{CliError, ConnectorsCli, WriteError, idempotency_key, json};

const ADAPTER: &str = "desk";
const CONNECTION: &str = "conn-desk";
const ROUTE: &str = "ticket.route";
const NOW: &str = "2026-10-17T09:00:00Z";

/// One test's directory: the fake program, its `--config` file, its `--state-dir` and `HOME`.
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(test: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("connectors_cli_write")
            .join(test);
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        for directory in ["state", "home", "approvals"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        install_program(
            &fixtures().join("fake-connectors"),
            &root.join("connectors"),
        );
        fs::write(root.join("connectors.toml"), "# not read by the fake\n").unwrap();
        Self { root }
    }

    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    fn cli(&self) -> ConnectorsCli {
        ConnectorsCli::new(
            ConnectorsCliConfig {
                program: self.root.join("connectors").display().to_string(),
                config: Some(self.root.join("connectors.toml").display().to_string()),
                state_dir: Some(self.state().display().to_string()),
                timeout_seconds: None,
            },
            Vec::new(),
        )
        .with_environment([
            (
                "HOME".to_owned(),
                self.root.join("home").display().to_string(),
            ),
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ])
    }

    /// The proof path a host got from `connectors approvals issue --proof-output`. Nothing is
    /// written there: Loom passes the path and never reads, writes or stores the proof.
    fn proof(&self) -> String {
        self.root
            .join("approvals/route.proof")
            .display()
            .to_string()
    }

    /// `operations <verb>` of [`ROUTE`] answers with the fixture `file`.
    fn answer(&self, verb: &str, file: &str) {
        fs::copy(
            fixtures().join(file),
            self.state().join(format!("{verb}.{ROUTE}.json")),
        )
        .unwrap();
    }

    /// `operations <verb>` exits with `status`.
    fn exit(&self, verb: &str, status: i32) {
        fs::write(
            self.state().join(format!("{verb}.exit")),
            status.to_string(),
        )
        .unwrap();
    }

    fn argv(&self) -> Vec<Vec<String>> {
        fs::read_to_string(self.state().join("argv.log"))
            .unwrap_or_default()
            .lines()
            .map(|line| {
                line.strip_suffix('\t')
                    .unwrap_or(line)
                    .split('\t')
                    .map(str::to_owned)
                    .collect()
            })
            .collect()
    }

    fn runs(&self, verb: &str) -> Vec<Vec<String>> {
        self.argv()
            .into_iter()
            .filter(|argv| {
                argv.windows(2)
                    .any(|w| w[0] == "operations" && w[1] == verb)
            })
            .collect()
    }

    fn stdin(&self) -> String {
        fs::read_to_string(self.state().join("stdin.log")).unwrap_or_default()
    }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/connectors")
}

/// Installs `source` as the executable `target` in a child process (see `connectors_cli.rs`).
fn install_program(source: &Path, target: &Path) {
    let status = std::process::Command::new("/usr/bin/install")
        .args(["-m", "755"])
        .arg(source)
        .arg(target)
        .status()
        .unwrap();
    assert!(status.success(), "install {}: {status}", target.display());
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
    json::parse(r#"{"ticket":"48213","queue":"billing"}"#).unwrap()
}

fn approval(path: impl Into<String>) -> WriteApproval {
    WriteApproval {
        proof_file: path.into(),
    }
}

fn value_of(argv: &[String], flag: &str) -> Option<String> {
    argv.windows(2).find(|w| w[0] == flag).map(|w| w[1].clone())
}

// ---------------------------------------------------------------------------------------------
// The admitted request, made by the runtime.

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// Ids from counters starting at `first`, and the one trusted time [`NOW`].
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

/// Keeps the request the runtime admitted and performs nothing.
#[derive(Default)]
struct Keep(Mutex<Vec<AdmittedRequest>>);

impl EffectPort for Keep {
    fn performs(&self, action: &str) -> bool {
        action == ROUTE
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

/// The request the runtime admits for [`ROUTE`], its id the `n`th the loop's context issues
/// after `first - 1`.
fn admitted(first: u64) -> AdmittedRequest {
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
        repeat_n(open(3), 5).chain([open(4), open(4).complete("routed")]),
    );
    let executor = ScriptedExecutor::new(vec![ExecutorOutcome::ProposedAction(
        ExecutorOutcomeProposedAction {
            action: ROUTE.to_owned(),
            arguments: ProposedActionArguments(
                commission_json::parse(r#"{"ticket":"48213","queue":"billing"}"#).unwrap(),
            ),
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
            requests: first - 1,
            observations: 0,
        },
    );
    let mut kept = keep.0.into_inner().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(kept.len(), 1, "the runtime admits the route once");
    kept.remove(0)
}

// ---------------------------------------------------------------------------------------------
// The acceptance.

/// An admitted write runs one describe and one invoke carrying the proof path and the key derived
/// from the request id; the input is on stdin, never in argv; the result is Connectors'
/// classification, `request_id` and audit record. The proof is never read, written or stored:
/// the path names no file before or after.
#[test]
fn an_admitted_write_passes_the_proof_path_and_the_derived_key() {
    let fixture = Fixture::new("an_admitted_write_passes_the_proof_path_and_the_derived_key");
    fixture.answer("describe", "describe-route.json");
    fixture.answer("invoke", "invoke-route-applied.json");
    let request = admitted(1);
    let proof = fixture.proof();

    let written = fixture
        .cli()
        .invoke_write(&request, &route(), &input(), &approval(proof.clone()))
        .unwrap();

    assert_eq!(written.classification, MutationClassification::Applied);
    assert_eq!(
        written.request_id.as_deref(),
        Some("00000000-0000-4000-8000-0000000000a1")
    );
    assert_eq!(
        written.audit_ref.as_deref(),
        Some("00000000-0000-4000-8000-0000000a0d1e")
    );
    assert_eq!(
        written.source_audit,
        Some(
            json::parse(
                r#"{"instance":"desk-fixture","audit_ref":"00000000-0000-4000-8000-0000000a0d1e","audit_status":"complete"}"#
            )
            .unwrap()
        )
    );
    assert_eq!(
        written.body,
        Some(json::parse(r#"{"ticket":"48213","queue":"billing"}"#).unwrap())
    );
    assert_eq!(written.idempotency_key, idempotency_key(&request));
    assert!(
        written
            .idempotency_key
            .contains(&request.data().action_request_id.0.0),
        "the key is derived from the admitted request id: {}",
        written.idempotency_key
    );

    assert_eq!(fixture.runs("describe").len(), 1, "{:?}", fixture.argv());
    let invokes = fixture.runs("invoke");
    assert_eq!(invokes.len(), 1, "{:?}", fixture.argv());
    assert_eq!(fixture.argv().len(), 2, "{:?}", fixture.argv());
    let invoke = &invokes[0];
    let at = invoke
        .iter()
        .position(|argument| argument == "operations")
        .unwrap();
    assert_eq!(
        invoke[at..],
        [
            "operations",
            "invoke",
            "--adapter",
            ADAPTER,
            "--connection",
            CONNECTION,
            "--operation",
            ROUTE,
            "--schema",
            "sch-route-1",
            "--revision",
            "rev-7",
            "--input-stdin",
            "--approval-file",
            proof.as_str(),
            "--idempotency-key",
            written.idempotency_key.as_str(),
        ]
    );
    assert_eq!(
        value_of(invoke, "--approval-file").as_deref(),
        Some(proof.as_str())
    );
    assert_eq!(
        fixture.stdin().trim(),
        r#"{"ticket":"48213","queue":"billing"}"#
    );
    for argument in invoke {
        assert!(
            !argument.contains("48213") && !argument.contains("billing"),
            "the input is never in argv: {argument}"
        );
    }
    assert!(
        !Path::new(&proof).exists(),
        "Loom writes nothing at the proof path"
    );
}

/// A write without a proof path, or with one that is not absolute, starts no process.
#[test]
fn a_write_without_an_absolute_proof_path_starts_nothing() {
    let fixture = Fixture::new("a_write_without_an_absolute_proof_path_starts_nothing");
    fixture.answer("describe", "describe-route.json");
    fixture.answer("invoke", "invoke-route-applied.json");
    let request = admitted(1);
    let cli = fixture.cli();

    for (path, code) in [
        ("", "approval-missing"),
        ("route.proof", "approval-not-absolute"),
        ("approvals/route.proof", "approval-not-absolute"),
        ("./route.proof", "approval-not-absolute"),
        ("~/approvals/route.proof", "approval-not-absolute"),
    ] {
        let answer = cli.invoke_write(&request, &route(), &input(), &approval(path));
        match answer {
            Err(WriteError::Refused(refusal)) => {
                assert_eq!(refusal.code, code, "{path:?}: {refusal:?}");
                assert_eq!(refusal.adapter, AdapterAlias(ADAPTER.to_owned()));
                assert_eq!(refusal.operation, Some(OperationId(ROUTE.to_owned())));
            }
            other => panic!("{path:?}: expected a refusal, got {other:?}"),
        }
    }
    assert!(fixture.argv().is_empty(), "{:?}", fixture.argv());
}

/// An operation `describe` does not report with a write profile is refused after the describe
/// and before any invoke.
#[test]
fn an_operation_not_described_as_a_write_is_refused_before_the_invoke() {
    let fixture =
        Fixture::new("an_operation_not_described_as_a_write_is_refused_before_the_invoke");
    fixture.answer("describe", "describe-route-read.json");
    fixture.answer("invoke", "invoke-route-applied.json");

    let answer =
        fixture
            .cli()
            .invoke_write(&admitted(1), &route(), &input(), &approval(fixture.proof()));

    match answer {
        Err(WriteError::Refused(refusal)) => {
            assert_eq!(refusal.code, "not-a-write", "{refusal:?}");
            assert!(refusal.message.contains("generic-http"), "{refusal:?}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(fixture.runs("describe").len(), 1, "{:?}", fixture.argv());
    assert_eq!(fixture.runs("invoke").len(), 0, "{:?}", fixture.argv());
}

/// The same admitted request yields the same key, so a second call can only observe the first
/// attempt; another admitted request yields another key.
#[test]
fn the_same_admitted_request_yields_the_same_idempotency_key() {
    let fixture = Fixture::new("the_same_admitted_request_yields_the_same_idempotency_key");
    fixture.answer("describe", "describe-route.json");
    fixture.answer("invoke", "invoke-route-applied.json");
    let cli = fixture.cli();
    let first = admitted(1);
    let again = admitted(1);
    let other = admitted(7);
    assert_eq!(first, again, "the runtime admits the same request twice");

    for request in [&first, &again, &other] {
        cli.invoke_write(request, &route(), &input(), &approval(fixture.proof()))
            .unwrap();
    }

    let keys: Vec<String> = fixture
        .runs("invoke")
        .iter()
        .map(|invoke| value_of(invoke, "--idempotency-key").unwrap())
        .collect();
    assert_eq!(keys.len(), 3, "{:?}", fixture.argv());
    assert_eq!(keys[0], keys[1], "the same admitted request, the same key");
    assert_ne!(keys[0], keys[2], "another admitted request, another key");
    assert_eq!(keys[0], idempotency_key(&first));
    assert_eq!(keys[2], idempotency_key(&other));
    assert_eq!(idempotency_key(&first), idempotency_key(&again));
}

/// A write Connectors classifies `unknown` is returned as unknown, and nothing is invoked again.
#[test]
fn an_unknown_write_is_returned_unknown_and_never_invoked_again() {
    let fixture = Fixture::new("an_unknown_write_is_returned_unknown_and_never_invoked_again");
    fixture.answer("describe", "describe-route.json");
    fixture.answer("invoke", "invoke-route-unknown.json");
    fixture.exit("invoke", 1);
    let request = admitted(1);

    let written = fixture
        .cli()
        .invoke_write(&request, &route(), &input(), &approval(fixture.proof()))
        .unwrap();

    assert_eq!(written.classification, MutationClassification::Unknown);
    assert_eq!(written.code.as_deref(), Some("outcome_unknown"));
    assert_eq!(written.idempotency_key, idempotency_key(&request));
    assert_eq!(written.body, None);
    assert_eq!(fixture.runs("describe").len(), 1, "{:?}", fixture.argv());
    assert_eq!(fixture.runs("invoke").len(), 1, "{:?}", fixture.argv());
    assert_eq!(fixture.argv().len(), 2, "{:?}", fixture.argv());
}

/// An invoke whose answer is lost (no JSON) is unknown, never a failure that invites a retry,
/// and is not invoked again.
#[test]
fn a_write_whose_answer_is_lost_is_unknown() {
    let fixture = Fixture::new("a_write_whose_answer_is_lost_is_unknown");
    fixture.answer("describe", "describe-route.json");
    fixture.answer("invoke", "invoke-route-lost.txt");
    fixture.exit("invoke", 1);

    let written = fixture
        .cli()
        .invoke_write(&admitted(1), &route(), &input(), &approval(fixture.proof()))
        .unwrap();

    assert_eq!(written.classification, MutationClassification::Unknown);
    assert_eq!(written.request_id, None);
    assert_eq!(fixture.runs("invoke").len(), 1, "{:?}", fixture.argv());
}

/// `refused` and `not_attempted` are returned as Connectors classifies them, with the failure
/// code, `request_id` and audit record Connectors names, and nothing is invoked again.
#[test]
fn refused_and_not_attempted_writes_are_returned_as_classified() {
    for (file, classification, code, request_id) in [
        (
            "invoke-route-refused.json",
            MutationClassification::Refused,
            "conflict",
            "00000000-0000-4000-8000-0000000000a2",
        ),
        (
            "invoke-route-not-attempted.json",
            MutationClassification::NotAttempted,
            "approval_invalid",
            "00000000-0000-4000-8000-0000000000a3",
        ),
    ] {
        let fixture = Fixture::new(&format!("classified_{}", file.trim_end_matches(".json")));
        fixture.answer("describe", "describe-route.json");
        fixture.answer("invoke", file);
        fixture.exit("invoke", 1);

        let written = fixture
            .cli()
            .invoke_write(&admitted(1), &route(), &input(), &approval(fixture.proof()))
            .unwrap();

        assert_eq!(written.classification, classification, "{file}");
        assert_eq!(written.code.as_deref(), Some(code), "{file}");
        assert_eq!(written.request_id.as_deref(), Some(request_id), "{file}");
        assert!(written.source_audit.is_some(), "{file}");
        assert_eq!(written.body, None, "{file}");
        assert_eq!(
            fixture.runs("invoke").len(),
            1,
            "{file}: {:?}",
            fixture.argv()
        );
    }
}

/// `invoke_read` still refuses an operation described with the `mutation` profile, and never
/// invokes it.
#[test]
fn invoke_read_still_refuses_a_mutation_operation() {
    let fixture = Fixture::new("invoke_read_still_refuses_a_mutation_operation");
    fixture.answer("describe", "describe-route.json");
    fixture.answer("invoke", "invoke-route-applied.json");

    let answer = fixture.cli().invoke_read(
        &AdapterAlias(ADAPTER.to_owned()),
        &ConnectionId(CONNECTION.to_owned()),
        &OperationId(ROUTE.to_owned()),
        &input(),
    );

    match answer {
        Err(CliError::Refused(refusal)) => assert_eq!(refusal.code, "write-operation"),
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(fixture.runs("invoke").len(), 0, "{:?}", fixture.argv());
}
