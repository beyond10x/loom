//! Adversarial cases for `story:connectors-governed-write`: `ConnectorsCli::invoke_write` and the
//! host evidence adapter `WriteEvidenceAdapter`, driven against the invariants the decision on the
//! governed write path states.
//!
//! The admitted requests are made by Commission's own `run_until_blocked`, as in
//! `connectors_cli_write.rs`; the program is the fake `connectors` of `connectors_cli.rs` (or, for
//! the timeout, a script written here), each test under
//! `CARGO_TARGET_TMPDIR/adversary_w172_governed_write`.

use std::collections::BTreeMap;
use std::fs;
use std::iter::repeat_n;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict, CaseId,
    Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, EvidenceData,
    EvidenceId, ExecutorOutcome, ExecutorOutcomeProposedAction, FrontierAction, GovernorError,
    Observation, ObservationData, ObservationId, PrincipalId, ProposedActionArguments, RunId, Unit,
    commission_state, observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::{
    AttributedEvidence, EvidencePort, ObservationPort, submit_evidence,
};
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_commission::runtime::{LoopContext, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_connectors::cli::datasource::{
    AdapterAlias, ConnectionId, ConnectorsCliConfig, MutationClassification, OperationId,
    WriteApproval, WriteOperation, WriteResult,
};
use b10x_loom_connectors::cli::{ConnectorsCli, WriteError, idempotency_key, json};
use b10x_loom_connectors::evidence::{WriteEvidenceAdapter, WriteEvidenceError};
use loom_governor::{CanonGovernor, CaseInputs, MemoryCaseStore};

const ADAPTER: &str = "desk";
const CONNECTION: &str = "conn-desk";
const ROUTE: &str = "ticket.route";
const ARGUMENTS: &str = r#"{"ticket":"48213","queue":"billing"}"#;
const NOW: &str = "2026-10-17T09:00:00Z";
const PRODUCER: &str = "service:helpdesk";

// ---------------------------------------------------------------------------------------------
// The fake `connectors`.

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(test: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("adversary_w172_governed_write")
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
        fixture.answer("describe", "describe-route.json");
        fixture.answer("invoke", "invoke-route-applied.json");
        fixture
    }

    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    fn cli_with(&self, timeout_seconds: Option<i64>, extra: &[(&str, &str)]) -> ConnectorsCli {
        let mut environment = vec![
            (
                "HOME".to_owned(),
                self.root.join("home").display().to_string(),
            ),
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ];
        environment.extend(
            extra
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned())),
        );
        ConnectorsCli::new(
            ConnectorsCliConfig {
                program: self.root.join("connectors").display().to_string(),
                config: None,
                state_dir: Some(self.state().display().to_string()),
                timeout_seconds,
            },
            Vec::new(),
        )
        .with_environment(environment)
    }

    fn cli(&self) -> ConnectorsCli {
        self.cli_with(None, &[])
    }

    fn proof(&self) -> String {
        self.root
            .join("approvals/route.proof")
            .display()
            .to_string()
    }

    fn answer(&self, verb: &str, file: &str) {
        fs::copy(
            fixtures().join(file),
            self.state().join(format!("{verb}.{ROUTE}.json")),
        )
        .unwrap();
    }

    fn answer_text(&self, verb: &str, text: &str) {
        fs::write(self.state().join(format!("{verb}.{ROUTE}.json")), text).unwrap();
    }

    fn argv(&self) -> Vec<String> {
        fs::read_to_string(self.state().join("argv.log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn invokes(&self) -> usize {
        self.argv()
            .iter()
            .filter(|line| line.contains("\tinvoke\t"))
            .count()
    }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/connectors")
}

fn route() -> WriteOperation {
    WriteOperation {
        adapter: AdapterAlias(ADAPTER.to_owned()),
        connection: ConnectionId(CONNECTION.to_owned()),
        operation: OperationId(ROUTE.to_owned()),
    }
}

fn input(text: &str) -> json::Value {
    json::parse(text).unwrap()
}

fn approval(path: impl Into<String>) -> WriteApproval {
    WriteApproval {
        proof_file: path.into(),
    }
}

// ---------------------------------------------------------------------------------------------
// The admitted request, made by the runtime.

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

/// The request the runtime admits for `action` with `arguments`, its id the `first`th.
fn admitted_for(action: &str, arguments: &str, first: u64) -> AdmittedRequest {
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
                action: action.to_owned(),
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
            action: action.to_owned(),
            arguments: ProposedActionArguments(commission_json::parse(arguments).unwrap()),
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
    assert_eq!(kept.len(), 1, "the runtime admits {action} once");
    kept.remove(0)
}

fn admitted(first: u64) -> AdmittedRequest {
    admitted_for(ROUTE, ARGUMENTS, first)
}

// ---------------------------------------------------------------------------------------------
// Invariant 2: the proof path.

/// A proof path holding a control character is refused before any process starts, as the
/// story's contract says of every argv value (`argument` in `cli.rs` refuses them for ids, but the
/// proof path never reaches it).
#[test]
fn a_control_character_proof_path_starts_no_process() {
    let fixture = Fixture::new("control_character_proof_path");
    let request = admitted(1);
    let cli = fixture.cli();
    for path in [
        format!("{}\n--idempotency-key\nforged", fixture.proof()),
        format!("{}\u{7}", fixture.proof()),
        "/approvals/route\u{0}.proof".to_owned(),
        "/approvals/\u{1b}[2Jroute.proof".to_owned(),
    ] {
        let answer = cli.invoke_write(&request, &route(), &input(ARGUMENTS), &approval(&path));
        assert!(
            matches!(answer, Err(WriteError::Refused(_) | WriteError::Failed(_))),
            "{path:?}: expected a refusal, got {answer:?}"
        );
    }
    assert!(
        fixture.argv().is_empty(),
        "a control-character proof path started connectors: {:?}",
        fixture.argv()
    );
}

/// A proof path that would read as a flag, or is relative, starts no process (probe; holds).
#[test]
fn a_flag_like_or_relative_proof_path_starts_no_process() {
    let fixture = Fixture::new("flag_like_proof_path");
    let request = admitted(1);
    let cli = fixture.cli();
    for path in ["-", "--approval-file", "-/x/route.proof", "..", " /x"] {
        let answer = cli.invoke_write(&request, &route(), &input(ARGUMENTS), &approval(path));
        match answer {
            Err(WriteError::Refused(refusal)) => {
                assert_eq!(refusal.code, "approval-not-absolute", "{path:?}")
            }
            other => panic!("{path:?}: expected a refusal, got {other:?}"),
        }
    }
    assert!(fixture.argv().is_empty(), "{:?}", fixture.argv());
}

// ---------------------------------------------------------------------------------------------
// Invariant 1: the admitted request is the only way to a write.

/// The input written is the input the runtime admitted. A host that passes other input than the
/// admitted request's arguments writes something the governor and the authority never saw;
/// `ConnectorsInvoker` (the service transport) writes `request.data().arguments`, and nothing
/// else.
#[test]
fn input_the_runtime_did_not_admit_is_never_written() {
    let fixture = Fixture::new("input_not_admitted");
    let request = admitted(1);

    let answer = fixture.cli().invoke_write(
        &request,
        &route(),
        &input(r#"{"ticket":"99999","queue":"root"}"#),
        &approval(fixture.proof()),
    );

    assert!(
        answer.is_err(),
        "input other than the admitted arguments was written: {answer:?}"
    );
    assert_eq!(
        fixture.invokes(),
        0,
        "invoked with input the runtime never admitted: {:?}",
        fixture.argv()
    );
}

/// A request the runtime admitted for another action does not admit a write of `ticket.route`.
#[test]
fn a_request_admitted_for_another_action_writes_nothing() {
    let fixture = Fixture::new("other_action");
    let request = admitted_for("ticket.read", ARGUMENTS, 1);
    assert_eq!(request.data().action, "ticket.read");

    let answer = fixture.cli().invoke_write(
        &request,
        &route(),
        &input(ARGUMENTS),
        &approval(fixture.proof()),
    );

    assert!(
        answer.is_err(),
        "a `ticket.read` admission wrote `ticket.route`: {answer:?}"
    );
    assert_eq!(fixture.invokes(), 0, "{:?}", fixture.argv());
}

// ---------------------------------------------------------------------------------------------
// Invariant 3: the key, and unknown.

/// Distinct admitted requests have distinct keys; every key is safe in argv and within the 256
/// characters Connectors accepts (probe; holds).
#[test]
fn idempotency_keys_are_distinct_and_argv_safe() {
    let requests: Vec<AdmittedRequest> = (1..=4).map(|n| admitted(n * 3)).collect();
    let keys: Vec<String> = requests.iter().map(idempotency_key).collect();
    for (i, key) in keys.iter().enumerate() {
        assert!(!key.starts_with('-'), "{key}");
        assert!(key.len() <= 256, "{key}");
        assert!(
            !key.chars().any(|c| c.is_whitespace() || c.is_control()),
            "{key}"
        );
        assert_eq!(*key, idempotency_key(&requests[i].clone()), "pure");
        for other in &keys[i + 1..] {
            assert_ne!(key, other);
        }
    }
}

/// An invoke still running at the bound is unknown, and is invoked once (probe; holds).
#[test]
fn a_timed_out_write_is_unknown_and_invoked_once() {
    let fixture = Fixture::new("timed_out");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >>'{state}/argv.log'\ncase \"$*\" in\n  *' describe '*) \
         cat '{state}/describe.{ROUTE}.json' ;;\n  *) exec sleep 5 ;;\nesac\n",
        state = fixture.state().display()
    );
    let program = fixture.root.join("connectors");
    fs::remove_file(&program).unwrap();
    fs::write(fixture.root.join("slow.sh"), script).unwrap();
    let status = std::process::Command::new("/usr/bin/install")
        .args(["-m", "755"])
        .arg(fixture.root.join("slow.sh"))
        .arg(&program)
        .status()
        .unwrap();
    assert!(status.success());
    let request = admitted(1);

    let written = fixture
        .cli_with(Some(1), &[])
        .invoke_write(
            &request,
            &route(),
            &input(ARGUMENTS),
            &approval(fixture.proof()),
        )
        .unwrap();

    assert_eq!(written.classification, MutationClassification::Unknown);
    assert_eq!(written.idempotency_key, idempotency_key(&request));
    let invokes = fixture
        .argv()
        .iter()
        .filter(|line| line.contains(" invoke "))
        .count();
    assert_eq!(invokes, 1, "{:?}", fixture.argv());
}

/// A success without a `mutation` member, or with a classification Connectors does not state,
/// is unknown, never applied (probe; holds).
#[test]
fn an_answer_without_a_stated_classification_is_unknown() {
    for (name, answer) in [
        (
            "no_mutation",
            r#"{"ok":true,"result":{"result":{},"request_id":"r-1","source_audit":{"audit_ref":"a-1"}}}"#,
        ),
        (
            "upper_case",
            r#"{"ok":true,"result":{"request_id":"r-1","mutation":{"classification":"APPLIED"}}}"#,
        ),
        (
            "not_text",
            r#"{"ok":true,"result":{"request_id":"r-1","mutation":{"classification":true}}}"#,
        ),
        (
            "not_ok",
            r#"{"ok":false,"result":{"mutation":{"classification":"applied"}}}"#,
        ),
    ] {
        let fixture = Fixture::new(&format!("unstated_{name}"));
        fixture.answer_text("invoke", answer);
        let written = fixture
            .cli()
            .invoke_write(
                &admitted(1),
                &route(),
                &input(ARGUMENTS),
                &approval(fixture.proof()),
            )
            .unwrap();
        assert_eq!(
            written.classification,
            MutationClassification::Unknown,
            "{name}"
        );
        assert_eq!(fixture.invokes(), 1, "{name}");
    }
}

// ---------------------------------------------------------------------------------------------
// Invariant 4: the environment.

/// A credential the host hands the client never reaches the write's process (probe; holds).
#[test]
fn the_write_environment_carries_no_credential() {
    let fixture = Fixture::new("environment");
    fixture
        .cli_with(
            None,
            &[
                ("CONNECTORS_TOKEN", "secret-token-w172"),
                ("GITLAB_TOKEN", "secret-token-w172"),
                ("AWS_SECRET_ACCESS_KEY", "secret-token-w172"),
            ],
        )
        .invoke_write(
            &admitted(1),
            &route(),
            &input(ARGUMENTS),
            &approval(fixture.proof()),
        )
        .unwrap();
    let seen = fs::read_to_string(fixture.state().join("env.log")).unwrap();
    assert!(!seen.contains("secret-token-w172"), "{seen}");
    assert_eq!(fixture.invokes(), 1);
}

// ---------------------------------------------------------------------------------------------
// Invariant 5: the evidence adapter.

#[derive(Default)]
struct Ledger {
    observations: Mutex<Vec<ObservationData>>,
    evidence: Mutex<Vec<EvidenceData>>,
}

impl ObservationPort for Ledger {
    fn observe(
        &self,
        observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        self.observations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(observation.into_data());
        Ok(())
    }
}

impl EvidencePort for Ledger {
    fn receive(&self, evidence: AttributedEvidence) -> Result<(), GovernorError> {
        self.evidence
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(evidence.into_evidence().into_data());
        Ok(())
    }
}

impl Ledger {
    fn delivered(&self) -> (usize, usize) {
        (
            self.observations
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .len(),
            self.evidence
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .len(),
        )
    }
}

fn result_of(request: &AdmittedRequest, classification: MutationClassification) -> WriteResult {
    WriteResult {
        classification,
        idempotency_key: idempotency_key(request),
        request_id: Some("00000000-0000-4000-8000-0000000000a1".to_owned()),
        body: None,
        source_audit: Some(json::parse(r#"{"audit_ref":"audit-w172"}"#).unwrap()),
        audit_ref: Some("audit-w172".to_owned()),
        code: None,
    }
}

fn record(
    ledger: &Ledger,
    request: &AdmittedRequest,
    written: &WriteResult,
) -> Result<Option<EvidenceId>, WriteEvidenceError> {
    WriteEvidenceAdapter::support_triage_route().record(
        ledger,
        PRODUCER,
        request,
        written,
        ObservationId(uuid(0x700)),
        Timestamp(NOW.to_owned()),
        EvidenceId(uuid(0x701)),
        "c3",
    )
}

/// The adapter records the write Connectors reported for the admitted request it is given, not
/// the write of another one: Connectors' answer for request A (its key, its request id) is not
/// evidence that request B was performed.
#[test]
fn the_adapter_refuses_the_write_of_another_admitted_request() {
    let first = admitted(1);
    let other = admitted(9);
    assert_ne!(idempotency_key(&first), idempotency_key(&other));
    let written_for_first = result_of(&first, MutationClassification::Applied);
    let ledger = Ledger::default();

    let answer = record(&ledger, &other, &written_for_first);

    assert!(
        answer.is_err(),
        "A's applied write was recorded as evidence of B: {answer:?}"
    );
    assert_eq!(
        ledger.delivered(),
        (0, 0),
        "nothing is delivered for a write of another request"
    );
}

/// Only `applied` is recorded; the record names `ticket.route`, the admitted request and
/// Connectors' request id and audit ref (probe; holds).
#[test]
fn the_adapter_records_only_applied_naming_the_request_and_audit() {
    let request = admitted(1);
    for classification in [
        MutationClassification::NotAttempted,
        MutationClassification::Refused,
        MutationClassification::Unknown,
    ] {
        let ledger = Ledger::default();
        let answer = record(&ledger, &request, &result_of(&request, classification));
        assert_eq!(answer, Ok(None), "{classification:?}");
        assert_eq!(ledger.delivered(), (0, 0), "{classification:?}");
    }

    let ledger = Ledger::default();
    let answer = record(
        &ledger,
        &request,
        &result_of(&request, MutationClassification::Applied),
    );
    assert_eq!(answer, Ok(Some(EvidenceId(uuid(0x701)))));
    let evidence = ledger.evidence.into_inner().unwrap();
    assert_eq!(evidence.len(), 1);
    let evidence = &evidence[0];
    assert_eq!(evidence.kind, "routing_record");
    assert_eq!(evidence.producer, PRODUCER);
    let text = |value: &str| commission_json::Value::Text(value.to_owned());
    let provenance = &evidence.provenance;
    assert_eq!(provenance.member("action"), Some(&text(ROUTE)));
    assert_eq!(
        provenance.member("action_request_id"),
        Some(&text(&request.data().action_request_id.0.0))
    );
    assert_eq!(
        provenance.member("request_id"),
        Some(&text("00000000-0000-4000-8000-0000000000a1"))
    );
    assert_eq!(
        provenance
            .member("source_audit")
            .and_then(|audit| audit.member("audit_ref")),
        Some(&text("audit-w172"))
    );
    assert_eq!(evidence.facts.member("kind"), Some(&text("routing_record")));
}

/// A request admitted for another action records nothing (probe; holds).
#[test]
fn the_adapter_refuses_a_request_of_another_action() {
    let request = admitted_for("ticket.read", ARGUMENTS, 1);
    let ledger = Ledger::default();
    let answer = record(
        &ledger,
        &request,
        &result_of(&request, MutationClassification::Applied),
    );
    assert!(
        matches!(answer, Err(WriteEvidenceError::OtherAction { .. })),
        "{answer:?}"
    );
    assert_eq!(ledger.delivered(), (0, 0));
}

// ---------------------------------------------------------------------------------------------
// Invariant 6: support-triage@1.

type Store = CanonGovernor<MemoryCaseStore>;

fn open_case() -> (Store, CaseId) {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = governor
        .open(
            "support-triage@1",
            BTreeMap::from([
                ("ticket".to_owned(), "c3".to_owned()),
                ("requester".to_owned(), "u1".to_owned()),
            ]),
            CaseInputs::none(),
        )
        .expect("a support-triage@1 case opens");
    (governor, case)
}

fn submit(
    governor: &Store,
    case: &CaseId,
    n: &mut u64,
    at: &str,
    (kind, result, subject, revision): (&str, Option<&str>, &str, &str),
) {
    *n += 1;
    let observation = ObservationId(uuid(0x2000 + *n));
    governor
        .observe(Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: "host/fixture".to_owned(),
            subject: subject.to_owned(),
            observed_at: Timestamp(at.to_owned()),
            payload: commission_json::Value::Null,
        }))
        .unwrap();
    *n += 1;
    let id = uuid(0x2000 + *n);
    let mut facts = vec![
        ("format", "canon-evidence/1".to_owned()),
        ("id", format!("{kind}-{}", id.0)),
        ("kind", kind.to_owned()),
        ("subject", subject.to_owned()),
        ("subject_revision", revision.to_owned()),
        ("observed_at", at.to_owned()),
    ];
    if let Some(result) = result {
        facts.push(("result", result.to_owned()));
    }
    let evidence = EvidenceData {
        evidence_id: EvidenceId(id),
        case_id: case.clone(),
        kind: kind.to_owned(),
        subject_revision: governor.current_revision(case).unwrap(),
        producer: String::new(),
        observation_ids: vec![observation],
        facts: commission_json::Value::Object(
            facts
                .into_iter()
                .map(|(name, value)| (name.to_owned(), commission_json::Value::Text(value)))
                .collect(),
        ),
        provenance: commission_json::Value::Null,
    };
    submit_evidence(governor, PRODUCER, evidence).unwrap();
}

/// The requests the runtime admits for a commission proposing `ticket.route` with its authority
/// granted.
fn admitted_routes(governor: &Store, case: &CaseId) -> usize {
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x80)),
        agent_revision_id: AgentRevisionId(uuid(0x90)),
        case_id: case.clone(),
        principal: PrincipalId("principal-desk".to_owned()),
        authority_context: AuthorityContext(commission_json::Value::Null),
    });
    let authority =
        StaticAuthorityProvider::new().answer(ROUTE, AuthorityVerdict::Allow(Unit(true)));
    let executor = ScriptedExecutor::new([
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: ROUTE.to_owned(),
            arguments: ProposedActionArguments(commission_json::parse(ARGUMENTS).unwrap()),
        }),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
    ]);
    let keep = Keep::default();
    let mut issued = 0u64;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));
    let _ = run_until_blocked(
        governor,
        &executor,
        &authority,
        &keep,
        &commission,
        &mut runs,
        &mut Context {
            requests: 0,
            observations: 0,
        },
    );
    keep.0.into_inner().unwrap().len()
}

fn route_status(governor: &Store, case: &CaseId) -> ActionStatus {
    governor
        .frontier(case)
        .unwrap()
        .into_data()
        .actions
        .iter()
        .find(|entry| entry.action == ROUTE)
        .unwrap()
        .status
}

/// A rejected review or an unidentified requester never admits `ticket.route`; the approved
/// review and identified requester do (probe; holds).
#[test]
fn support_triage_admits_the_route_only_after_approval_and_a_known_requester() {
    for (name, review, requester, admits) in [
        ("rejected", "rejected", "identified", false),
        ("unidentified", "approved", "unidentified", false),
        ("both", "approved", "identified", true),
    ] {
        let (governor, case) = open_case();
        let mut n = 0;
        submit(
            &governor,
            &case,
            &mut n,
            "2026-10-17T08:55:00Z",
            ("ticket_snapshot", Some("within_target"), "ticket", "c3"),
        );
        submit(
            &governor,
            &case,
            &mut n,
            "2026-10-17T08:56:00Z",
            ("classification", None, "ticket", "c3"),
        );
        submit(
            &governor,
            &case,
            &mut n,
            "2026-10-17T08:57:00Z",
            ("requester_profile", Some(requester), "requester", "u1"),
        );
        submit(
            &governor,
            &case,
            &mut n,
            "2026-10-17T08:58:00Z",
            ("classification_review", Some(review), "ticket", "c3"),
        );
        let status = route_status(&governor, &case);
        if admits {
            assert_ne!(status, ActionStatus::Blocked, "{name}");
            assert_eq!(admitted_routes(&governor, &case), 1, "{name}");
        } else {
            assert_eq!(status, ActionStatus::Blocked, "{name}");
            assert_eq!(admitted_routes(&governor, &case), 0, "{name}");
        }
    }
}
