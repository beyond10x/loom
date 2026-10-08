//! Adversarial cases for `loom_governor::evaluate` (story `governor-evaluate`): refusal naming on a
//! terminated snapshot, equivalence with `CanonGovernor` for every bundled protocol and for evidence
//! the governor sets aside, protocol-name spoofing, and the cost of naming a refused record.

use std::collections::BTreeMap;
use std::time::Instant;

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus as FrontierStatus, CaseId, CompletionDetermination, EvidenceData, EvidenceId,
    ObservationId, Truth,
};
use b10x_loom_commission::ports::evidence::submit_evidence;
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::model::evaluation::{
    ActionStatus, CaseSnapshot, ClaimValue, EvaluationDecision, EvaluationInput, EvaluationRequest,
    EvidenceRecord, ProtocolName,
};
use loom_governor::model::json;
use loom_governor::model::primitives::Timestamp;
use loom_governor::{CanonGovernor, MemoryCaseStore, evaluate};
use loom_protocols::ProtocolCatalog;
use serde_json::{Value, json};

fn query_revisions() -> BTreeMap<String, String> {
    BTreeMap::from([("intent".to_owned(), "query-1".to_owned())])
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

fn clock(id: &str) -> Value {
    record(id, "system_time", "observed", "intent", "query-1")
}

fn snapshot(protocol_id: &str, revisions: &BTreeMap<String, String>, case_revision: &str) -> Value {
    let artifacts: serde_json::Map<String, Value> = revisions
        .iter()
        .map(|(artifact, revision)| (artifact.clone(), json!({"revision": revision})))
        .collect();
    json!({
        "format": "canon-case/1",
        "id": "case-1",
        "protocol": protocol_id,
        "artifacts": artifacts,
        "revision": case_revision,
    })
}

fn terminated_query() -> Value {
    let mut case = snapshot("system.query", &query_revisions(), "r1");
    case["termination"] = json!("answered");
    case
}

fn request(
    protocol: &str,
    snapshot: &Value,
    records: &[Value],
    at: Option<&str>,
) -> EvaluationRequest {
    let value = |value: &Value| json::parse(&value.to_string()).expect("JSON");
    EvaluationRequest {
        protocol: ProtocolName(protocol.into()),
        snapshot: CaseSnapshot(value(snapshot)),
        evidence: records.iter().map(|r| EvidenceRecord(value(r))).collect(),
        at: at.map(|at| Timestamp(at.into())),
    }
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn submit(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId, n: u64, record: &Value) {
    let evidence = EvidenceData {
        evidence_id: EvidenceId(uuid(2 * n + 1)),
        case_id: case.clone(),
        kind: record["kind"].as_str().expect("kind").to_owned(),
        subject_revision: governor.current_revision(case).expect("revision"),
        producer: String::new(),
        observation_ids: vec![ObservationId(uuid(2 * n + 2))],
        facts: commission_json::parse(&record.to_string()).expect("facts are JSON"),
        provenance: commission_json::Value::Object(Vec::new()),
    };
    submit_evidence(governor, "service:ci", evidence).expect("the governor takes the evidence");
}

fn reason_text(value: &json::Value) -> Value {
    let mut out = String::new();
    json::push_value(&mut out, value);
    serde_json::from_str(&out).expect("a reason is JSON")
}

/// Everything the governor's frontier and completion report, including each reason's content.
fn assert_same_as_governor(
    label: &str,
    decision: &EvaluationDecision,
    governor: &CanonGovernor<MemoryCaseStore>,
    case: &CaseId,
) {
    let frontier = governor.frontier(case).expect("the frontier").into_data();
    let expected: Vec<(String, ActionStatus, Option<String>, Vec<Value>)> = frontier
        .actions
        .iter()
        .map(|a| {
            (
                a.action.clone(),
                match a.status {
                    FrontierStatus::Admissible => ActionStatus::Admissible,
                    FrontierStatus::ApprovalRequired => ActionStatus::ApprovalRequired,
                    FrontierStatus::Blocked => ActionStatus::Blocked,
                },
                a.capability.clone(),
                a.reasons
                    .iter()
                    .map(|r| serde_json::from_str(r).expect("reason JSON"))
                    .collect(),
            )
        })
        .collect();
    let actual: Vec<(String, ActionStatus, Option<String>, Vec<Value>)> = decision
        .actions
        .iter()
        .map(|a| {
            (
                a.action.clone(),
                a.status,
                a.requires.first().cloned(),
                a.reasons.iter().map(reason_text).collect(),
            )
        })
        .collect();
    assert_eq!(actual, expected, "{label}: actions");
    let expected: Vec<(String, ClaimValue)> = frontier
        .claims
        .iter()
        .map(|c| {
            (
                c.claim.clone(),
                match c.value {
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
        .map(|c| (c.claim.clone(), c.value))
        .collect();
    assert_eq!(actual, expected, "{label}: claims");
    let expected: Vec<(String, bool)> = frontier
        .obligations
        .iter()
        .map(|o| (o.obligation.clone(), o.open))
        .collect();
    let actual: Vec<(String, bool)> = decision
        .obligations
        .iter()
        .map(|o| (o.obligation.clone(), o.open))
        .collect();
    assert_eq!(actual, expected, "{label}: obligations");
    let expected = match governor.completion(case).expect("the completion") {
        CompletionDetermination::Complete(complete) => Some(complete.outcome),
        CompletionDetermination::Open(_) => None,
    };
    assert_eq!(decision.outcome, expected, "{label}: completion");
}

/// A terminated case whose evidence repeats a record id: Canon refuses the second record as a
/// duplicate, and the refusal must name that record, not the snapshot.
#[test]
fn adversary_a_duplicate_record_beside_a_terminated_snapshot_is_named_as_the_record() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let refusal = evaluate(
        &catalog,
        &request(
            "system-query@1",
            &terminated_query(),
            &[clock("clock-1"), clock("clock-1")],
            None,
        ),
    )
    .expect_err("a duplicate record id is refused");
    assert_eq!(
        (refusal.input, refusal.evidence_index, refusal.code.as_str()),
        (EvaluationInput::Evidence, Some(1), "duplicate-identifier"),
        "{refusal:?}"
    );
}

/// A terminated case evaluated at a time Canon cannot read: the refusal must name the time.
#[test]
fn adversary_an_unreadable_time_beside_a_terminated_snapshot_is_named_as_the_time() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let refusal = evaluate(
        &catalog,
        &request(
            "system-query@1",
            &terminated_query(),
            &[clock("clock-1")],
            Some("yesterday"),
        ),
    )
    .expect_err("an unreadable time is refused");
    assert_eq!(refusal.input, EvaluationInput::Time, "{refusal:?}");
}

/// Story Acceptance: the decision `CanonGovernor` reports for the same case after the same evidence.
/// The governor takes a record of a kind the protocol does not declare and sets it aside; for the
/// same case after the same evidence `evaluate` must report the governor's decision.
#[test]
fn adversary_evidence_the_governor_sets_aside_still_gives_the_governors_decision() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog)
        .expect("catalog");
    let case = CaseId("case-1".into());
    governor
        .open_case(case.clone(), "system-query@1", query_revisions())
        .expect("open");
    let records = vec![
        clock("clock-1"),
        record(
            "stray-1",
            "undeclared_kind",
            "observed",
            "intent",
            "query-1",
        ),
    ];
    for (n, r) in records.iter().enumerate() {
        submit(&governor, &case, n as u64, r);
    }
    assert!(matches!(
        governor.completion(&case),
        Ok(CompletionDetermination::Complete(_))
    ));
    let decided = evaluate(
        &catalog,
        &request(
            "system-query@1",
            &snapshot("system.query", &query_revisions(), "r1"),
            &records,
            None,
        ),
    );
    match decided {
        Ok(decision) => assert_same_as_governor("set-aside", &decision, &governor, &case),
        Err(refusal) => panic!(
            "the governor decides this case after this evidence; evaluate refuses: {refusal:?}"
        ),
    }
}

/// Every bundled protocol: a fresh case, then the same case after a record about each declared
/// evidence kind (sent in reverse order), a revision move that supersedes the first record, and
/// the evidence again. `evaluate` reports what the governor reports at every step.
#[test]
fn adversary_every_bundled_protocol_matches_the_governor_through_a_superseded_revision() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let mut seen = 0;
    for entry in catalog.iter() {
        let name = entry.name().to_owned();
        let protocol_id = entry.model.protocol.id.as_str().to_owned();
        let artifacts: Vec<String> = entry
            .model
            .artifacts
            .ids()
            .map(|a| a.as_str().to_owned())
            .collect();
        let kinds: Vec<String> = entry
            .model
            .evidence_kinds
            .ids()
            .map(|k| k.as_str().to_owned())
            .collect();
        let mut revisions: BTreeMap<String, String> = artifacts
            .iter()
            .map(|a| (a.clone(), "v1".to_owned()))
            .collect();
        let governor = CanonGovernor::new(MemoryCaseStore::default())
            .with_catalog(&catalog)
            .expect("catalog");
        let case = CaseId("case-1".into());
        governor
            .open_case(case.clone(), &name, revisions.clone())
            .expect("open");
        let check =
            |label: &str, revisions: &BTreeMap<String, String>, rev: &str, records: &[Value]| {
                let decision = evaluate(
                    &catalog,
                    &request(
                        &name,
                        &snapshot(&protocol_id, revisions, rev),
                        records,
                        None,
                    ),
                )
                .unwrap_or_else(|r| panic!("{name} {label}: refused {r:?}"));
                assert_same_as_governor(&format!("{name} {label}"), &decision, &governor, &case);
            };
        check("fresh", &revisions, "r1", &[]);
        let mut records = Vec::new();
        for (n, kind) in kinds.iter().enumerate().rev() {
            for (m, result) in ["pass", "approved", "healthy", "satisfied", "observed"]
                .iter()
                .enumerate()
            {
                let artifact = &artifacts[(n + m) % artifacts.len()];
                records.push(record(
                    &format!("e-{n}-{m}"),
                    kind,
                    result,
                    artifact,
                    &revisions[artifact],
                ));
            }
        }
        for (n, r) in records.iter().enumerate() {
            submit(&governor, &case, n as u64, r);
        }
        check("after evidence", &revisions, "r1", &records);
        let moved = artifacts[0].clone();
        governor.update_revision(&case, &moved, "v2").expect("move");
        revisions.insert(moved, "v2".into());
        check("after a superseding revision", &revisions, "r2", &records);
        seen += 1;
    }
    assert_eq!(
        seen,
        catalog.iter().count(),
        "every bundled protocol is checked"
    );
    assert!(
        seen >= 3,
        "the bundled catalog holds the engineering protocols and system-query: {seen}"
    );
}

/// Names that look like a catalog protocol but are not its exact identity are refused as unknown.
#[test]
fn adversary_spoofed_protocol_names_are_refused_as_unknown() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let case = snapshot("system.query", &query_revisions(), "r1");
    for spoof in [
        "System-Query@1",
        "SYSTEM-QUERY@1",
        " system-query@1",
        "system-query@1 ",
        "system-query@01",
        "system-query@+1",
        "system.query@1",
        "system.query",
        "protocols/system-query/1.yaml",
        "system-query@1\u{0}",
        loom_protocols::SYSTEM_QUERY_YAML,
    ] {
        let refusal = evaluate(&catalog, &request(spoof, &case, &[], None))
            .expect_err("a spoofed name is refused");
        assert_eq!(
            (refusal.input, refusal.code.as_str()),
            (EvaluationInput::Protocol, "unknown-protocol"),
            "{spoof:?}: {refusal:?}"
        );
    }
}

fn duplicate_cost(n: usize) -> (f64, f64) {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let case = snapshot("system.query", &query_revisions(), "r1");
    let mut records: Vec<Value> = (0..n).map(|i| clock(&format!("clock-{i}"))).collect();
    let decided = request("system-query@1", &case, &records, None);
    records.push(clock("clock-0"));
    let refused = request("system-query@1", &case, &records, None);
    let start = Instant::now();
    evaluate(&catalog, &decided).expect("decided");
    let decide = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let refusal = evaluate(&catalog, &refused).expect_err("refused");
    let refuse = start.elapsed().as_secs_f64();
    assert_eq!(refusal.evidence_index, Some(n as i64));
    (decide, refuse)
}

/// Naming the refused record must not cost more than a bounded multiple of deciding the same
/// records: a duplicate id at the end of N records is found by Canon in one pass.
#[test]
fn adversary_naming_a_refused_record_costs_a_bounded_multiple_of_deciding() {
    let n = 1000;
    let (decide, refuse) = duplicate_cost(n);
    eprintln!(
        "n={n}: decide {decide:.4}s, refuse {refuse:.4}s, ratio {:.1}",
        refuse / decide
    );
    assert!(
        refuse <= 20.0 * decide + 0.05,
        "refusing {n} records took {refuse:.3}s against {decide:.3}s to decide them"
    );
}
