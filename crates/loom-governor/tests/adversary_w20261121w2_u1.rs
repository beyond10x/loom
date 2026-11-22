//! Adversary, wave 2026-11-21-w2, unit 1 (`story:canon-0-9-0-pin`), pass 1.
//!
//! Canon 0.9.0 adds the first refusal over a whole set of evidence records: two `case_outcome`
//! records for one child case are refused together (`duplicate-child-case`), though each passes
//! Canon's check alone. The governor decides whether a record applies against the records that
//! already apply, so it keeps the first and holds the second without applying it. `evaluate`
//! promises the same set-aside ("a readable record Canon refuses for this case is set aside, as
//! `CanonGovernor` sets it aside when it arrives") but tests each record alone, and then refuses
//! the whole request, naming the snapshot.
//!
//! The other cases pin the `children` arm of `evidence_reads` where the story's own test does not
//! reach: a `case_outcome` record about an artifact other than the opening's subject, and a
//! `children` form found only in an action precondition under `any` and `not`.
//!
//! No model or network call is made.

use std::collections::BTreeMap;

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, Observation, ObservationData, ObservationId, Truth,
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

const AUDIT: &str = "audit@1";

/// The protocol of `case_composition.rs`: Canon's `fixtures/investigation/case-composition.yaml`
/// with `open_change` also declaring that it may produce `case_outcome`.
const AUDIT_YAML: &str = r#"
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

const TWO_ARTIFACTS: &str = "ledger@1";

/// The child is opened about `findings`; its outcome record may be about any artifact, and Canon's
/// `children` form reads it whatever its subject.
const TWO_ARTIFACTS_YAML: &str = r#"
format: protocol/1
protocol: {id: ledger, revision: 1}
artifacts: {findings: {}, report: {}}
evidence_kinds: {case_outcome: {}}
claims:
  findings.resolved:
    true_when:
      children: {in: [accepted]}
actions:
  open_change:
    requires:
      - capability: case.open
    case_open: {protocol: software.change, subject: findings}
    may_produce:
      - evidence: case_outcome
outcomes:
  resolved:
    requires: {claim: findings.resolved}
"#;

const NESTED: &str = "nested@1";

/// The only `children` form is in an action's precondition, under `any` and `not`.
const NESTED_YAML: &str = r#"
format: protocol/1
protocol: {id: nested, revision: 1}
artifacts: {findings: {}}
evidence_kinds: {case_outcome: {}, review: {}}
claims:
  findings.reviewed:
    true_when:
      evidence: {kind: review, subject: findings}
actions:
  open_change:
    requires:
      - capability: case.open
    case_open: {protocol: software.change, subject: findings}
    may_produce:
      - evidence: case_outcome
  close:
    precondition:
      any:
        - claim: findings.reviewed
        - not:
            children: {in: [declined]}
outcomes:
  done:
    requires: {claim: findings.reviewed}
"#;

const PRODUCER: &str = "trusted:child-case-runtime";

fn source() -> ProtocolSource {
    ProtocolSource {
        kind: SourceKind::Memory,
        location: "adversary-w20261121w2-u1".into(),
        revision: String::new(),
        path: String::new(),
    }
}

fn catalog(name: &str, yaml: &str) -> ProtocolCatalog {
    let mut catalog = ProtocolCatalog::default();
    catalog.add_yaml(name, yaml, source()).expect("added");
    catalog
}

fn governor(name: &str, yaml: &str) -> CanonGovernor<MemoryCaseStore> {
    CanonGovernor::new(MemoryCaseStore::default())
        .with_host_case_outcomes()
        .with_protocol_yaml(name, yaml)
        .expect("registers")
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A `canon-evidence/1` `case_outcome` record naming `child`.
fn outcome(id: &str, subject: &str, revision: &str, child: &str, result: &str) -> Value {
    json!({
        "format": "canon-evidence/1",
        "id": id,
        "kind": "case_outcome",
        "result": result,
        "subject": subject,
        "subject_revision": revision,
        "child_case": child,
    })
}

/// Submits `record` through Commission's evidence port, citing an observation the governor
/// received first (as `case_composition.rs` does).
fn receive(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId, n: u64, record: &Value) {
    let observation = ObservationId(uuid(0x100 + n));
    ObservationPort::observe(
        governor,
        Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: PRODUCER.into(),
            subject: record["subject"].as_str().expect("subject").into(),
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

fn open_evidence(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId) -> Vec<String> {
    governor
        .action_facts(case)
        .expect("action facts")
        .actions
        .into_iter()
        .find(|facts| facts.action == "open_change")
        .expect("the facts name open_change")
        .open_evidence
}

fn resolved(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId) -> Truth {
    governor
        .frontier(case)
        .expect("frontier")
        .data()
        .claims
        .iter()
        .find(|claim| claim.claim == "findings.resolved")
        .expect("findings.resolved")
        .value
}

/// A child reported `open` and then `accepted`, each record carried in the history the caller
/// keeps. The governor keeps the first and holds the second without applying it, and decides.
/// `evaluate` is documented to set aside a readable record Canon refuses, as the governor does,
/// and to refuse the snapshot only for its termination; given the same two records it refuses the
/// request naming the snapshot, `duplicate-child-case`.
#[test]
fn evaluate_sets_aside_a_second_record_for_one_child_as_the_governor_does() {
    let first = outcome("o1", "findings", "f1", "CHG-1", "open");
    let second = outcome("o2", "findings", "f1", "CHG-1", "accepted");

    // The governor: the first applies, the second is held and does not apply, and it decides.
    let held = governor(AUDIT, AUDIT_YAML);
    let case = held
        .open(
            AUDIT,
            BTreeMap::from([("findings".to_owned(), "f1".to_owned())]),
            CaseInputs::none(),
        )
        .expect("the case opens");
    receive(&held, &case, 1, &first);
    receive(&held, &case, 2, &second);
    assert_eq!(held.evidence(&case).expect("evidence").len(), 2);
    assert_eq!(
        resolved(&held, &case),
        Truth::Unknown,
        "the governor keeps the open record and decides"
    );

    // `evaluate`, over the same two records.
    let catalog = catalog(AUDIT, AUDIT_YAML);
    let snapshot = json!({
        "format": "canon-case/1",
        "id": "case-1",
        "protocol": "audit",
        "artifacts": {"findings": {"revision": "f1"}},
        "revision": "r1",
    });
    let request = |records: &[&Value]| EvaluationRequest {
        protocol: ProtocolName(AUDIT.into()),
        snapshot: CaseSnapshot(json::parse(&snapshot.to_string()).expect("JSON")),
        evidence: records
            .iter()
            .map(|record| EvidenceRecord(json::parse(&record.to_string()).expect("JSON")))
            .collect(),
        at: None,
        decisions: Vec::new(),
    };
    let alone = evaluate(&catalog, &request(&[&first])).expect("one record for the child");
    let both = evaluate(&catalog, &request(&[&first, &second]));
    let both = match both {
        Ok(decision) => decision,
        Err(refusal) => panic!(
            "evaluate refused a snapshot with no termination because of two evidence records, \
             where the governor sets the second aside and decides: input {:?}, index {:?}, \
             code {}, message {}",
            refusal.input, refusal.evidence_index, refusal.code, refusal.message
        ),
    };
    assert_eq!(
        (&both.actions, &both.claims, &both.outcome),
        (&alone.actions, &alone.claims, &alone.outcome),
        "the second record for the child is set aside, as the governor sets it aside"
    );
}

/// Canon's `children` form reads every accepted `case_outcome` record whatever its subject
/// (`eval/composition.rs`, `value`). A record about `report`, not the opening's subject, counts
/// for `open_change` and keeps counting once `report` moves. An arm that read the kind only about
/// the opening's subject, or only at the current revision, fails here.
#[test]
fn a_case_outcome_record_about_another_artifact_counts_at_any_revision() {
    let governor = governor(TWO_ARTIFACTS, TWO_ARTIFACTS_YAML);
    let case = governor
        .open(
            TWO_ARTIFACTS,
            BTreeMap::from([
                ("findings".to_owned(), "f1".to_owned()),
                ("report".to_owned(), "p1".to_owned()),
            ]),
            CaseInputs::none(),
        )
        .expect("the case opens");
    assert_eq!(open_evidence(&governor, &case), ["case_outcome"]);
    receive(
        &governor,
        &case,
        1,
        &outcome("o1", "report", "p1", "CHG-1", "accepted"),
    );
    assert_eq!(resolved(&governor, &case), Truth::True, "Canon reads it");
    assert!(open_evidence(&governor, &case).is_empty(), "it counts");
    governor
        .update_revision(&case, "report", "p2")
        .expect("report moves");
    assert_eq!(
        resolved(&governor, &case),
        Truth::True,
        "Canon still reads it"
    );
    assert!(
        open_evidence(&governor, &case).is_empty(),
        "it still counts once its subject moved, as Canon still reads it"
    );
}

/// The protocol's only `children` form sits in an action precondition under `any` and `not`. The
/// walk reaches it, so `case_outcome` is read as the form reads it: a record still counts once its
/// subject moved. A walk that skipped action preconditions, `any` or `not` would leave the kind
/// to the rule for a kind nothing reads, which drops a record at a superseded revision.
#[test]
fn a_children_form_only_in_a_nested_precondition_reads_case_outcome_at_any_revision() {
    let governor = governor(NESTED, NESTED_YAML);
    let case = governor
        .open(
            NESTED,
            BTreeMap::from([("findings".to_owned(), "f1".to_owned())]),
            CaseInputs::none(),
        )
        .expect("the case opens");
    assert_eq!(open_evidence(&governor, &case), ["case_outcome"]);
    receive(
        &governor,
        &case,
        1,
        &outcome("o1", "findings", "f1", "CHG-1", "accepted"),
    );
    assert!(open_evidence(&governor, &case).is_empty(), "it counts");
    governor
        .update_revision(&case, "findings", "f2")
        .expect("findings move");
    assert!(
        open_evidence(&governor, &case).is_empty(),
        "the children form reads it at any revision, so it still counts"
    );
}
