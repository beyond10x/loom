//! `loom_governor::evaluate` decides a protocol from the host catalog over a case snapshot the
//! caller keeps and the evidence records that go with it, with no Commission type in its signature
//! (story `governor-evaluate`, `ess/domains/evaluation.yaml`).
//!
//! `CanonGovernor` is the oracle: for the same case after the same evidence, `evaluate` reports the
//! same action statuses, claims, obligations and completion as the governor's frontier and
//! completion.

use std::collections::BTreeMap;
use std::path::PathBuf;

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus as FrontierStatus, CaseId, CompletionDetermination, EvidenceData, EvidenceId,
    ObservationId, Truth,
};
use b10x_loom_commission::ports::evidence::submit_evidence;
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::model::evaluation::{
    ActionStatus, CaseSnapshot, ClaimValue, EvaluationDecision, EvaluationInput, EvaluationRefusal,
    EvaluationRequest, EvidenceRecord, ProtocolName,
};
use loom_governor::model::json;
use loom_governor::model::primitives::Timestamp;
use loom_governor::{CanonGovernor, MemoryCaseStore, evaluate};
use loom_protocols::ProtocolCatalog;
use serde_json::{Value, json};

const PRODUCER: &str = "service:ci";

/// The public signature names only the generated `loom.evaluation` types and the host catalog:
/// this coercion does not compile if any parameter or the result is another type.
#[test]
fn the_signature_names_no_commission_type() {
    let signature: fn(
        &ProtocolCatalog,
        &EvaluationRequest,
    ) -> Result<EvaluationDecision, EvaluationRefusal> = evaluate;
    let _ = signature;

    let source =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("the library source");
    let start = source
        .find("pub fn evaluate(")
        .expect("lib.rs declares `pub fn evaluate(`");
    let end = start + source[start..].find('{').expect("the body opens");
    let declared = &source[start..end];
    for commission in [
        "commission",
        "CaseId",
        "EvidenceData",
        "Frontier",
        "GovernorError",
        "CompletionDetermination",
        "CaseStore",
    ] {
        assert!(
            !declared.contains(commission),
            "`evaluate` names `{commission}` in its signature: {declared}"
        );
    }
}

/// For a bundled protocol, the decision equals the one `CanonGovernor` reports for the same case
/// after the same evidence, submitted through Commission's port.
#[test]
fn evaluate_reports_what_canon_governor_reports_for_the_same_case_after_the_same_evidence() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let revisions = software_change_revisions();
    let records = vec![
        record("tests-r2", "test_result", "pass", "implementation", "R2"),
        record(
            "health-d0",
            "operational_observation",
            "healthy",
            "deployment",
            "d0",
        ),
    ];
    let (governor, case) = governed("software-change@1", &catalog, &revisions, &records);

    let decision = evaluate(
        &catalog,
        &request(
            "software-change@1",
            snapshot("software.change", &revisions),
            &records,
            None,
        ),
    )
    .expect("the request is decided");

    assert_same_as_governor(&decision, &governor, &case);
    assert!(
        decision
            .actions
            .iter()
            .any(|action| action.status == ActionStatus::Admissible),
        "some action is admissible: {decision:?}"
    );
    assert_eq!(decision.outcome, None, "the change case is still open");
    assert_eq!(decision.protocol, ProtocolName("software-change@1".into()));
    assert_eq!(decision.case, "case-1");
    assert_eq!(
        decision.canon.member("format"),
        Some(&json::Value::Text("canon-decision/1".into())),
        "the decision carries Canon's whole document"
    );
}

/// A case with one legitimate outcome is complete in both, with that outcome.
#[test]
fn evaluate_reports_the_completion_canon_governor_reports() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let revisions = BTreeMap::from([("intent".to_owned(), "query-1".to_owned())]);
    let records = vec![record(
        "clock-1",
        "system_time",
        "observed",
        "intent",
        "query-1",
    )];
    let (governor, case) = governed("system-query@1", &catalog, &revisions, &records);

    let decision = evaluate(
        &catalog,
        &request(
            "system-query@1",
            snapshot("system.query", &revisions),
            &records,
            None,
        ),
    )
    .expect("the request is decided");

    assert_same_as_governor(&decision, &governor, &case);
    assert_eq!(decision.outcome.as_deref(), Some("answered"));

    let open = evaluate(
        &catalog,
        &request(
            "system-query@1",
            snapshot("system.query", &revisions),
            &[],
            None,
        ),
    )
    .expect("the request is decided");
    assert_eq!(open.outcome, None, "no evidence, no completion");
}

/// A protocol the catalog does not hold, a snapshot that is not a `canon-case/1` document and an
/// evidence record that is not a `canon-evidence/1` record are each refused, naming the input.
#[test]
fn each_unusable_input_is_refused_naming_it() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let revisions = BTreeMap::from([("intent".to_owned(), "query-1".to_owned())]);
    let good = record("clock-1", "system_time", "observed", "intent", "query-1");

    for unknown in ["unknown-protocol@1", "system-query@2", "system-query", ""] {
        let refusal = evaluate(
            &catalog,
            &request(
                unknown,
                snapshot("system.query", &revisions),
                std::slice::from_ref(&good),
                None,
            ),
        )
        .expect_err("an unknown protocol is refused");
        assert_eq!(refusal.input, EvaluationInput::Protocol, "{refusal:?}");
        assert_eq!(refusal.evidence_index, None);
        assert_eq!(refusal.code, "unknown-protocol");
    }

    let host_only = ProtocolCatalog::default();
    let refusal = evaluate(
        &host_only,
        &request(
            "system-query@1",
            snapshot("system.query", &revisions),
            &[],
            None,
        ),
    )
    .expect_err("a protocol outside the host's catalog is refused, bundled or not");
    assert_eq!(refusal.input, EvaluationInput::Protocol, "{refusal:?}");

    for malformed in [
        json!("not a case"),
        json!({"format": "canon-case/1"}),
        json!({"format": "canon-case/9", "id": "case-1", "protocol": "system.query",
               "artifacts": {"intent": {"revision": "query-1"}}}),
        json!({"format": "canon-case/1", "id": "case-1", "protocol": "software.change",
               "artifacts": {"intent": {"revision": "query-1"}}}),
    ] {
        let refusal = evaluate(
            &catalog,
            &request(
                "system-query@1",
                malformed.clone(),
                std::slice::from_ref(&good),
                None,
            ),
        )
        .expect_err("a malformed snapshot is refused");
        assert_eq!(
            refusal.input,
            EvaluationInput::Snapshot,
            "{malformed}: {refusal:?}"
        );
        assert_eq!(refusal.evidence_index, None);
    }

    for (index, bad) in [
        json!({"format": "canon-evidence/1"}),
        json!(["not", "a", "record"]),
        json!({"format": "canon-evidence/1", "id": "clock-2", "kind": "system_time"}),
        json!({"format": "canon-evidence/1", "id": "clock-2", "kind": "system_time",
               "result": ["observed"], "subject": "intent", "subject_revision": "query-1"}),
        record("clock-1", "system_time", "observed", "intent", "query-1"),
    ]
    .into_iter()
    .enumerate()
    {
        let records = vec![good.clone(), bad.clone()];
        let refusal = evaluate(
            &catalog,
            &request(
                "system-query@1",
                snapshot("system.query", &revisions),
                &records,
                None,
            ),
        )
        .expect_err("a malformed evidence record is refused");
        assert_eq!(
            refusal.input,
            EvaluationInput::Evidence,
            "case {index}, {bad}: {refusal:?}"
        );
        assert_eq!(
            refusal.evidence_index,
            Some(1),
            "case {index}, {bad}: {refusal:?}"
        );
    }

    let refusal = evaluate(
        &catalog,
        &request(
            "system-query@1",
            snapshot("system.query", &revisions),
            std::slice::from_ref(&good),
            Some("yesterday"),
        ),
    )
    .expect_err("a time that is not an instant is refused");
    assert_eq!(refusal.input, EvaluationInput::Time, "{refusal:?}");
}

/// A readable record Canon refuses for the case is set aside, as `CanonGovernor` sets it aside: the
/// decision is the one made without it.
#[test]
fn a_record_that_does_not_apply_is_set_aside() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let revisions = BTreeMap::from([("intent".to_owned(), "query-1".to_owned())]);
    let good = record("clock-1", "system_time", "observed", "intent", "query-1");
    let without = evaluate(
        &catalog,
        &request(
            "system-query@1",
            snapshot("system.query", &revisions),
            std::slice::from_ref(&good),
            None,
        ),
    )
    .expect("decided");
    for stray in [
        record("stray", "undeclared_kind", "observed", "intent", "query-1"),
        record(
            "stray",
            "system_time",
            "observed",
            "no_such_artifact",
            "query-1",
        ),
        record(
            "stray",
            "system_time",
            "observed",
            "intent",
            "not an identifier",
        ),
    ] {
        for records in [
            vec![good.clone(), stray.clone()],
            vec![stray.clone(), good.clone()],
        ] {
            let decision = evaluate(
                &catalog,
                &request(
                    "system-query@1",
                    snapshot("system.query", &revisions),
                    &records,
                    None,
                ),
            )
            .unwrap_or_else(|refusal| panic!("{stray}: refused {refusal:?}"));
            assert_eq!(
                (&decision.actions, &decision.claims, &decision.outcome),
                (&without.actions, &without.claims, &without.outcome),
                "{stray}"
            );
        }
    }
}

/// Over a terminated snapshot, every refusal still names its own input; the termination itself is
/// the snapshot's, refused only when the records do not make it legitimate.
#[test]
fn refusals_over_a_terminated_snapshot_name_their_input() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let revisions = BTreeMap::from([("intent".to_owned(), "query-1".to_owned())]);
    let mut terminated = snapshot("system.query", &revisions);
    terminated["termination"] = json!("answered");
    let good = record("clock-1", "system_time", "observed", "intent", "query-1");
    let stray = record("stray", "undeclared_kind", "observed", "intent", "query-1");
    let run = |case: &Value, records: &[Value], at: Option<&str>| {
        evaluate(
            &catalog,
            &request("system-query@1", case.clone(), records, at),
        )
    };

    let decided = run(&terminated, std::slice::from_ref(&good), None).expect("legitimate");
    assert_eq!(decided.outcome.as_deref(), Some("answered"));
    let decided = run(&terminated, &[stray.clone(), good.clone()], None).expect("set aside");
    assert_eq!(decided.outcome.as_deref(), Some("answered"));

    for records in [vec![], vec![stray.clone()]] {
        let refusal = run(&terminated, &records, None).expect_err("illegitimate termination");
        assert_eq!(
            (refusal.input, refusal.evidence_index, refusal.code.as_str()),
            (EvaluationInput::Snapshot, None, "illegitimate-termination"),
            "{refusal:?}"
        );
    }

    let refusal = run(
        &terminated,
        &[good.clone(), json!({"format": "canon-evidence/1"})],
        None,
    )
    .expect_err("unreadable record");
    assert_eq!(
        (refusal.input, refusal.evidence_index),
        (EvaluationInput::Evidence, Some(1)),
        "{refusal:?}"
    );

    let refusal = run(&terminated, &[], Some("yesterday")).expect_err("unreadable time");
    assert_eq!(refusal.input, EvaluationInput::Time, "{refusal:?}");

    let mut undeclared = terminated.clone();
    undeclared["termination"] = json!("no_such_outcome");
    let refusal = run(&undeclared, std::slice::from_ref(&good), None).expect_err("undeclared");
    assert_eq!(
        (refusal.input, refusal.evidence_index),
        (EvaluationInput::Snapshot, None),
        "{refusal:?}"
    );
}

/// The evaluation reads no clock: the same request gives the same decision, a supplied instant is
/// the only time Canon sees, and the crate names no clock, network or model API.
#[test]
fn the_evaluation_makes_no_clock_network_or_model_call() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let revisions = BTreeMap::from([("intent".to_owned(), "query-1".to_owned())]);
    let records = vec![record(
        "clock-1",
        "system_time",
        "observed",
        "intent",
        "query-1",
    )];
    let absent = request(
        "system-query@1",
        snapshot("system.query", &revisions),
        &records,
        None,
    );
    assert_eq!(evaluate(&catalog, &absent), evaluate(&catalog, &absent));
    let supplied = request(
        "system-query@1",
        snapshot("system.query", &revisions),
        &records,
        Some("2026-10-08T12:00:00Z"),
    );
    let first = evaluate(&catalog, &supplied).expect("decided");
    assert_eq!(Ok(first.clone()), evaluate(&catalog, &supplied));
    assert_eq!(first.outcome.as_deref(), Some("answered"));

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(manifest.join("src/lib.rs")).expect("the source");
    for forbidden in [
        "SystemTime",
        "Instant::now",
        "std::net",
        "TcpStream",
        "llm_",
        "reqwest",
        "chrono",
    ] {
        assert!(
            !source.contains(forbidden),
            "the governor names `{forbidden}`"
        );
    }
    let manifest = std::fs::read_to_string(manifest.join("Cargo.toml")).expect("the manifest");
    for forbidden in ["llm", "reqwest", "hyper", "ureq", "chrono", "time ="] {
        assert!(
            !manifest.contains(forbidden),
            "the governor depends on `{forbidden}`"
        );
    }
}

/// The decision's statuses, claims, obligations and completion are the governor's.
fn assert_same_as_governor(
    decision: &EvaluationDecision,
    governor: &CanonGovernor<MemoryCaseStore>,
    case: &CaseId,
) {
    let frontier = governor.frontier(case).expect("the frontier").into_data();
    let expected: Vec<(String, ActionStatus, Option<String>, usize)> = frontier
        .actions
        .iter()
        .map(|action| {
            (
                action.action.clone(),
                match action.status {
                    FrontierStatus::Admissible => ActionStatus::Admissible,
                    FrontierStatus::ApprovalRequired => ActionStatus::ApprovalRequired,
                    FrontierStatus::Blocked => ActionStatus::Blocked,
                },
                action.capability.clone(),
                action.reasons.len(),
            )
        })
        .collect();
    let actual: Vec<(String, ActionStatus, Option<String>, usize)> = decision
        .actions
        .iter()
        .map(|action| {
            (
                action.action.clone(),
                action.status,
                action.requires.first().cloned(),
                action.reasons.len(),
            )
        })
        .collect();
    assert_eq!(actual, expected, "actions");

    let expected: Vec<(String, ClaimValue)> = frontier
        .claims
        .iter()
        .map(|claim| {
            (
                claim.claim.clone(),
                match claim.value {
                    Truth::True => ClaimValue::True,
                    Truth::False => ClaimValue::False,
                    Truth::Unknown => ClaimValue::Unknown,
                },
            )
        })
        .collect();
    let actual: Vec<(String, ClaimValue)> = decision
        .claims
        .iter()
        .map(|claim| (claim.claim.clone(), claim.value))
        .collect();
    assert_eq!(actual, expected, "claims");

    let expected: Vec<(String, bool)> = frontier
        .obligations
        .iter()
        .map(|obligation| (obligation.obligation.clone(), obligation.open))
        .collect();
    let actual: Vec<(String, bool)> = decision
        .obligations
        .iter()
        .map(|obligation| (obligation.obligation.clone(), obligation.open))
        .collect();
    assert_eq!(actual, expected, "obligations");

    let expected = match governor.completion(case).expect("the completion") {
        CompletionDetermination::Complete(complete) => Some(complete.outcome),
        CompletionDetermination::Open(_) => None,
    };
    assert_eq!(decision.outcome, expected, "completion");
}

/// A governor over `catalog` holding `case-1` on `protocol`, with `records` submitted through
/// Commission's `submit_evidence`.
fn governed(
    protocol: &str,
    catalog: &ProtocolCatalog,
    revisions: &BTreeMap<String, String>,
    records: &[Value],
) -> (CanonGovernor<MemoryCaseStore>, CaseId) {
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(catalog)
        .expect("the catalog is admitted");
    let case = CaseId("case-1".into());
    governor
        .open_case(case.clone(), protocol, revisions.clone())
        .expect("the case opens");
    for (n, record) in records.iter().enumerate() {
        let evidence = EvidenceData {
            evidence_id: EvidenceId(uuid(2 * n as u64 + 1)),
            case_id: case.clone(),
            kind: record["kind"].as_str().expect("kind").to_owned(),
            subject_revision: governor.current_revision(&case).expect("revision"),
            producer: String::new(),
            observation_ids: vec![ObservationId(uuid(2 * n as u64 + 2))],
            facts: commission_json::parse(&record.to_string()).expect("facts are JSON"),
            provenance: commission_json::Value::Object(Vec::new()),
        };
        submit_evidence(&governor, PRODUCER, evidence).expect("the governor takes the evidence");
    }
    (governor, case)
}

/// The `canon-case/1` snapshot `CanonGovernor` evaluates for a case it opened as `case-1`.
fn snapshot(protocol_id: &str, revisions: &BTreeMap<String, String>) -> Value {
    let artifacts: serde_json::Map<String, Value> = revisions
        .iter()
        .map(|(artifact, revision)| (artifact.clone(), json!({"revision": revision})))
        .collect();
    json!({
        "format": "canon-case/1",
        "id": "case-1",
        "protocol": protocol_id,
        "artifacts": artifacts,
        "revision": "r1",
    })
}

fn request(
    protocol: &str,
    snapshot: Value,
    records: &[Value],
    at: Option<&str>,
) -> EvaluationRequest {
    let value = |value: &Value| json::parse(&value.to_string()).expect("JSON");
    EvaluationRequest {
        protocol: ProtocolName(protocol.into()),
        snapshot: CaseSnapshot(value(&snapshot)),
        evidence: records
            .iter()
            .map(|record| EvidenceRecord(value(record)))
            .collect(),
        at: at.map(|at| Timestamp(at.into())),
    }
}

fn record(id: &str, kind: &str, result: &str, subject: &str, revision: &str) -> Value {
    json!({
        "format": "canon-evidence/1",
        "id": id,
        "kind": kind,
        "result": result,
        "subject": subject,
        "subject_revision": revision,
    })
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// The artifact revisions of `tests/fixtures/chg-1842.fixture.yaml`.
fn software_change_revisions() -> BTreeMap<String, String> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chg-1842.fixture.yaml");
    let fixture: Value =
        serde_yaml_ng::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("YAML");
    fixture["case"]["artifacts"]
        .as_object()
        .expect("artifacts")
        .iter()
        .map(|(artifact, entry)| {
            (
                artifact.clone(),
                entry["revision"].as_str().expect("revision").to_owned(),
            )
        })
        .collect()
}
