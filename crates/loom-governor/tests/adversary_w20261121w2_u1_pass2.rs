//! Adversary, wave 2026-11-21-w2, unit 1 (`story:canon-0-9-0-pin`), pass 2.
//!
//! `evaluate` now judges records Canon refuses only together (two `case_outcome` records for one
//! child) in request order, each with those kept before it. These cases hold it against the
//! governor over histories the unit's own cases do not build, and measure what the new fallback
//! costs: `evaluate_adversary_2.rs` bounds the set-aside path at a multiple of deciding, but only
//! with a record Canon refuses alone.
//!
//! No model or network call is made.

use std::collections::BTreeMap;
use std::time::Instant;

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, Observation, ObservationData, ObservationId,
};
use b10x_loom_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::model::evaluation::{
    CaseSnapshot, EvaluationRequest, EvidenceRecord, ProtocolName,
};
use loom_governor::model::json;
use loom_governor::{CanonGovernor, CaseInputs, MemoryCaseStore, evaluate};
use loom_protocols::{ProtocolCatalog, ProtocolSource, SourceKind};
use serde_json::{Value, json};

const NAME: &str = "audit@1";

/// The protocol of `case_composition.rs`.
const PROTOCOL: &str = r#"
format: protocol/1
protocol: {id: audit, revision: 1}
artifacts: {findings: {}}
evidence_kinds: {case_outcome: {}}
claims:
  findings.resolved:
    true_when:
      children: {in: [declined, accepted]}
actions:
  open_change:
    requires:
      - capability: case.open
    case_open: {protocol: software.change, subject: findings}
    may_produce:
      - evidence: case_outcome
  close:
    precondition:
      children: {in: [accepted, declined]}
outcomes:
  resolved:
    requires: {claim: findings.resolved}
"#;

const PRODUCER: &str = "trusted:child-case-runtime";

fn catalog() -> ProtocolCatalog {
    let mut catalog = ProtocolCatalog::default();
    catalog
        .add_yaml(
            NAME,
            PROTOCOL,
            ProtocolSource {
                kind: SourceKind::Memory,
                location: "adversary-w20261121w2-u1-pass2".into(),
                revision: String::new(),
                path: String::new(),
            },
        )
        .expect("added");
    catalog
}

fn snapshot() -> Value {
    json!({
        "format": "canon-case/1",
        "id": "case-1",
        "protocol": "audit",
        "artifacts": {"findings": {"revision": "f1"}},
        "revision": "r1",
    })
}

fn request(records: &[Value]) -> EvaluationRequest {
    EvaluationRequest {
        protocol: ProtocolName(NAME.into()),
        snapshot: CaseSnapshot(json::parse(&snapshot().to_string()).expect("JSON")),
        evidence: records
            .iter()
            .map(|record| EvidenceRecord(json::parse(&record.to_string()).expect("JSON")))
            .collect(),
        at: None,
        decisions: Vec::new(),
    }
}

/// A `case_outcome` record about `findings` at `f1`, naming `child` and giving `result` when given.
fn outcome(id: &str, child: Option<&str>, result: Option<&str>) -> Value {
    let mut record = json!({
        "format": "canon-evidence/1",
        "id": id,
        "kind": "case_outcome",
        "subject": "findings",
        "subject_revision": "f1",
    });
    if let Some(child) = child {
        record["child_case"] = json!(child);
    }
    if let Some(result) = result {
        record["result"] = json!(result);
    }
    record
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn receive(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId, n: u64, record: &Value) {
    let observation = ObservationId(uuid(0x10000 + n));
    ObservationPort::observe(
        governor,
        Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: PRODUCER.into(),
            subject: "findings".into(),
            observed_at: Timestamp("2026-11-21T10:00:00Z".into()),
            payload: commission_json::Value::Null,
        }),
    )
    .expect("the observation is received");
    submit_evidence(
        governor,
        PRODUCER,
        EvidenceData {
            evidence_id: EvidenceId(uuid(n)),
            case_id: case.clone(),
            kind: "case_outcome".to_owned(),
            subject_revision: governor.current_revision(case).expect("revision"),
            producer: String::new(),
            facts: commission_json::parse(&record.to_string()).expect("facts are JSON"),
            provenance: commission_json::Value::Null,
            observation_ids: vec![observation],
        },
    )
    .expect("the record is received");
}

/// `evaluate`'s Canon document over `records`, and the governor's for the same records received in
/// the same order, must be equal.
fn assert_evaluate_equals_the_governor(label: &str, records: &[Value]) {
    let decision = evaluate(&catalog(), &request(records))
        .unwrap_or_else(|refusal| panic!("{label}: evaluate refused {refusal:?}"));
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml(NAME, PROTOCOL)
        .expect("registers");
    let case = governor
        .open(
            NAME,
            BTreeMap::from([("findings".to_owned(), "f1".to_owned())]),
            CaseInputs::none(),
        )
        .expect("the case opens");
    assert_eq!(case.0, "case-1", "{label}");
    for (n, record) in records.iter().enumerate() {
        receive(&governor, &case, n as u64 + 1, record);
    }
    let frontier = governor.frontier(&case).expect("the frontier");
    let document = governor
        .decision_document(&case, &frontier.data().frontier_id)
        .expect("held")
        .expect("the document just issued");
    assert_eq!(
        decision.canon, document,
        "{label}: evaluate decides what the governor decides for the same history"
    );
}

/// Histories the unit's cases do not build: three records for one child; duplicates for two
/// children interleaved; a record refused alone between two records for one child; a child whose
/// first record is refused alone (no result) and whose second applies.
#[test]
fn evaluate_equals_the_governor_over_histories_with_several_duplicates() {
    let histories: Vec<(&str, Vec<Value>)> = vec![
        (
            "three records for one child",
            vec![
                outcome("o1", Some("c1"), Some("open")),
                outcome("o2", Some("c1"), Some("accepted")),
                outcome("o3", Some("c1"), Some("declined")),
            ],
        ),
        (
            "duplicates for two children, interleaved",
            vec![
                outcome("o1", Some("c1"), Some("open")),
                outcome("o2", Some("c2"), Some("accepted")),
                outcome("o3", Some("c1"), Some("accepted")),
                outcome("o4", Some("c2"), Some("abandoned")),
                outcome("o5", Some("c3"), Some("declined")),
            ],
        ),
        (
            "a record refused alone between two for one child",
            vec![
                outcome("o1", Some("c1"), Some("accepted")),
                outcome("o2", None, Some("accepted")),
                outcome("o3", Some("c1"), Some("abandoned")),
            ],
        ),
        (
            "the child's first record refused alone, its second applies",
            vec![
                outcome("o1", Some("c1"), None),
                outcome("o2", Some("c1"), Some("accepted")),
                outcome("o3", Some("c1"), Some("open")),
            ],
        ),
    ];
    for (label, records) in &histories {
        assert_evaluate_equals_the_governor(label, records);
    }
}

/// Records Canon refuses only together cost a bounded multiple of deciding them, as
/// `adversary2_the_set_aside_path_costs_a_bounded_multiple_of_deciding` holds for a record Canon
/// refuses alone (the same bound, 40 times plus 0.25 s): 1000 records for 1000 children decide
/// once; one more record for the first child sends `evaluate` into the in-order fallback, which
/// decides every prefix of the 1000.
#[test]
fn a_second_record_for_one_child_costs_a_bounded_multiple_of_deciding() {
    let catalog = catalog();
    let n = 1000;
    let mut records: Vec<Value> = (0..n)
        .map(|i| outcome(&format!("o{i}"), Some(&format!("c{i}")), Some("accepted")))
        .collect();
    let decided = request(&records);
    records.push(outcome("dup", Some("c0"), Some("open")));
    let set_aside = request(&records);
    let start = Instant::now();
    let fast = evaluate(&catalog, &decided).expect("decided");
    let decide = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let slow = evaluate(&catalog, &set_aside).expect("decided with one set aside");
    let filter = start.elapsed().as_secs_f64();
    eprintln!(
        "n={n}: decide {decide:.4}s, set aside {filter:.4}s, ratio {:.1}",
        filter / decide
    );
    assert_eq!(slow.canon, fast.canon, "the second record leaves no trace");
    assert!(
        filter <= 40.0 * decide + 0.25,
        "setting one duplicate child record aside among {n} took {filter:.3}s against \
         {decide:.3}s to decide them"
    );
}
