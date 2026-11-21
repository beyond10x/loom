//! Acceptance for `story:canon-0-9-0-pin`: on Canon 0.9.0 a protocol that declares `case_open` on
//! an action and a `children` form in a claim registers through
//! `CanonGovernor::with_protocol_yaml` and through `ProtocolCatalog::add_yaml`, a case opens under
//! it, and its frontier lists the opening action. A `case_outcome` record naming its child, with
//! the child still `open`, applies and leaves the `children` claim `unknown`; one naming no child
//! is refused by Canon (`missing-child-case`), set aside by `evaluate` and held without applying
//! by the governor. `CanonGovernor::action_facts` reads the `case_outcome` kind as the `children`
//! form reads it: every applying record, at any revision of its subject.
//!
//! The protocol is Canon's own `fixtures/investigation/case-composition.yaml` at tag `0.9.0`, the
//! opening action also declaring that it may produce `case_outcome`, so its open evidence shows
//! whether a record counts.

use std::collections::BTreeMap;

use b10x_canon::eval::{Supplied, case_from_value, evaluate_with, evidence_from_value};
use b10x_canon::ir::compile;
use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionStatus, CaseId, CompletionDetermination, EvidenceData, EvidenceId, Frontier, Observation,
    ObservationData, ObservationId, Truth, frontier_state,
};
use b10x_loom_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::model::evaluation::{
    CaseSnapshot, ClaimValue, EvaluationRequest, EvidenceRecord, ProtocolName,
};
use loom_governor::model::json;
use loom_governor::{CanonGovernor, CaseInputs, MemoryCaseStore, evaluate};
use loom_protocols::{ProtocolCatalog, ProtocolSource, SourceKind};
use serde_json::{Value, json};

const NAME: &str = "audit@1";

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

fn source() -> ProtocolSource {
    ProtocolSource {
        kind: SourceKind::Memory,
        location: "case-composition-fixture".into(),
        revision: String::new(),
        path: String::new(),
    }
}

fn catalog() -> ProtocolCatalog {
    let mut catalog = ProtocolCatalog::default();
    catalog
        .add_yaml(NAME, PROTOCOL, source())
        .expect("a protocol with case_open and a children form is added to the catalog");
    catalog
}

/// The two admissions: the protocol registered by its YAML, and the catalog holding it admitted.
fn governors() -> Vec<(&'static str, CanonGovernor<MemoryCaseStore>)> {
    vec![
        (
            "with_protocol_yaml",
            CanonGovernor::new(MemoryCaseStore::default())
                .with_protocol_yaml(NAME, PROTOCOL)
                .expect("a protocol with case_open and a children form registers"),
        ),
        (
            "with_catalog",
            CanonGovernor::new(MemoryCaseStore::default())
                .with_catalog(&catalog())
                .expect("the catalog holding it is admitted"),
        ),
    ]
}

fn open(governor: &CanonGovernor<MemoryCaseStore>) -> CaseId {
    governor
        .open(
            NAME,
            BTreeMap::from([("findings".to_owned(), "f1".to_owned())]),
            CaseInputs::none(),
        )
        .expect("the case opens")
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A `canon-evidence/1` `case_outcome` record about `findings` at `f1`, naming `child` when given.
fn outcome(id: &str, child: Option<&str>, result: &str) -> Value {
    let mut record = json!({
        "format": "canon-evidence/1",
        "id": id,
        "kind": "case_outcome",
        "result": result,
        "subject": "findings",
        "subject_revision": "f1",
    });
    if let Some(child) = child {
        record["child_case"] = json!(child);
    }
    record
}

/// Submits `record` through Commission's evidence port, citing an observation of its own that the
/// governor received first.
fn receive(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId, n: u64, record: &Value) {
    let observation = ObservationId(uuid(0x100 + n));
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

fn status(frontier: &Frontier<frontier_state::Issued>, action: &str) -> ActionStatus {
    frontier
        .data()
        .actions
        .iter()
        .find(|entry| entry.action == action)
        .unwrap_or_else(|| panic!("the frontier lists {action}: {:?}", frontier.data()))
        .status
}

fn resolved(frontier: &Frontier<frontier_state::Issued>) -> Truth {
    frontier
        .data()
        .claims
        .iter()
        .find(|claim| claim.claim == "findings.resolved")
        .unwrap_or_else(|| {
            panic!(
                "the frontier lists findings.resolved: {:?}",
                frontier.data()
            )
        })
        .value
}

/// The open evidence `action_facts` gives `open_change`.
fn open_evidence(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId) -> Vec<String> {
    governor
        .action_facts(case)
        .expect("the governor answers the case's action facts")
        .actions
        .into_iter()
        .find(|facts| facts.action == "open_change")
        .expect("the facts name open_change")
        .open_evidence
}

#[test]
fn a_protocol_with_case_open_and_children_registers_and_its_frontier_lists_the_opening_action() {
    for (admission, governor) in governors() {
        let case = open(&governor);
        let frontier = governor.frontier(&case).expect("the frontier");
        let opening = frontier
            .data()
            .actions
            .iter()
            .find(|entry| entry.action == "open_change")
            .unwrap_or_else(|| {
                panic!(
                    "{admission}: the frontier lists open_change: {:?}",
                    frontier.data()
                )
            });
        assert_eq!(
            (opening.status, opening.capability.as_deref()),
            (ActionStatus::ApprovalRequired, Some("case.open")),
            "{admission}: the opening action needs its capability, like any other action"
        );
        assert_eq!(
            status(&frontier, "close"),
            ActionStatus::Blocked,
            "{admission}"
        );
        assert_eq!(
            resolved(&frontier),
            Truth::Unknown,
            "{admission}: no child record"
        );
        assert_eq!(
            open_evidence(&governor, &case),
            ["case_outcome"],
            "{admission}: nothing counted yet"
        );
    }
}

#[test]
fn an_open_child_leaves_the_children_claim_unknown_and_its_record_counts_at_any_revision() {
    for (admission, governor) in governors() {
        let case = open(&governor);
        assert_eq!(
            open_evidence(&governor, &case),
            ["case_outcome"],
            "{admission}"
        );

        receive(&governor, &case, 1, &outcome("o1", Some("CHG-1"), "open"));
        let frontier = governor.frontier(&case).expect("the frontier");
        assert_eq!(
            resolved(&frontier),
            Truth::Unknown,
            "{admission}: a child still open"
        );
        assert_eq!(
            status(&frontier, "close"),
            ActionStatus::Blocked,
            "{admission}"
        );
        assert!(
            open_evidence(&governor, &case).is_empty(),
            "{admission}: the applying case_outcome record counts"
        );

        governor
            .update_revision(&case, "findings", "f2")
            .expect("the findings move");
        assert!(
            open_evidence(&governor, &case).is_empty(),
            "{admission}: the children form reads a case_outcome record at any revision, so it \
             still counts once its subject moved"
        );
        assert_eq!(
            resolved(&governor.frontier(&case).expect("the frontier")),
            Truth::Unknown,
            "{admission}: the open child is still read after the move"
        );
    }
}

#[test]
fn an_accepted_child_resolves_the_parent() {
    for (admission, governor) in governors() {
        let case = open(&governor);
        receive(
            &governor,
            &case,
            1,
            &outcome("o1", Some("CHG-1"), "accepted"),
        );
        let frontier = governor.frontier(&case).expect("the frontier");
        assert_eq!(resolved(&frontier), Truth::True, "{admission}");
        assert_eq!(
            status(&frontier, "close"),
            ActionStatus::Admissible,
            "{admission}"
        );
        assert!(
            matches!(
                governor.completion(&case).expect("completion"),
                CompletionDetermination::Complete(ref complete) if complete.outcome == "resolved"
            ),
            "{admission}: the case completes through resolved"
        );
    }
}

#[test]
fn a_case_outcome_record_naming_no_child_is_refused_by_canon_and_never_applies() {
    let without_child = outcome("o1", None, "open");

    let catalog = catalog();
    let model = &catalog.get(NAME).expect("audit@1").model;
    let ir = compile(model).expect("audit@1 compiles");
    let snapshot = json!({
        "format": "canon-case/1",
        "id": "case-1",
        "protocol": "audit",
        "artifacts": {"findings": {"revision": "f1"}},
        "revision": "r1",
    });
    let yaml = |value: &Value| serde_yaml_ng::to_value(value).expect("JSON is YAML");
    let refusal = evaluate_with(
        &ir,
        &case_from_value(&yaml(&snapshot)).expect("the snapshot"),
        &[evidence_from_value(&yaml(&without_child)).expect("the record reads")],
        Supplied::default(),
    )
    .expect_err("Canon refuses a case_outcome record naming no child");
    assert_eq!(refusal.code(), "missing-child-case", "{refusal}");

    let request = |records: &[Value]| EvaluationRequest {
        protocol: ProtocolName(NAME.into()),
        snapshot: CaseSnapshot(json::parse(&snapshot.to_string()).expect("JSON")),
        evidence: records
            .iter()
            .map(|record| EvidenceRecord(json::parse(&record.to_string()).expect("JSON")))
            .collect(),
        at: None,
        decisions: Vec::new(),
    };
    let without = evaluate(&catalog, &request(&[])).expect("decided with no record");
    let with = evaluate(&catalog, &request(std::slice::from_ref(&without_child)))
        .expect("a readable record Canon refuses for the case is set aside, as the governor does");
    assert_eq!(
        (&with.actions, &with.claims, &with.outcome, &with.canon),
        (
            &without.actions,
            &without.claims,
            &without.outcome,
            &without.canon
        ),
        "the decision is the one made without the record"
    );
    assert!(
        with.claims
            .iter()
            .any(|claim| claim.claim == "findings.resolved" && claim.value == ClaimValue::Unknown),
        "{with:?}"
    );

    for (admission, governor) in governors() {
        let case = open(&governor);
        receive(&governor, &case, 1, &without_child);
        assert_eq!(
            governor.evidence(&case).expect("evidence").len(),
            1,
            "{admission}: the record is held"
        );
        assert_eq!(
            open_evidence(&governor, &case),
            ["case_outcome"],
            "{admission}: a held record that does not apply counts for nothing"
        );
        assert_eq!(
            resolved(&governor.frontier(&case).expect("the frontier")),
            Truth::Unknown,
            "{admission}"
        );

        receive(
            &governor,
            &case,
            2,
            &outcome("o2", Some("CHG-1"), "accepted"),
        );
        assert_eq!(
            resolved(&governor.frontier(&case).expect("the frontier")),
            Truth::True,
            "{admission}: the record that did not apply stops no later evaluation"
        );
    }
}

/// `evaluate`'s decision over `records`, in that order, and the decision document the governor
/// issues for the same records received in the same order: the two must be equal, byte for byte
/// as Canon renders them. Returns the claim `findings.resolved` as `evaluate` gives it.
fn evaluate_equals_the_governor(label: &str, records: &[Value]) -> ClaimValue {
    let catalog = catalog();
    let snapshot = json!({
        "format": "canon-case/1",
        "id": "case-1",
        "protocol": "audit",
        "artifacts": {"findings": {"revision": "f1"}},
        "revision": "r1",
    });
    let decision = evaluate(
        &catalog,
        &EvaluationRequest {
            protocol: ProtocolName(NAME.into()),
            snapshot: CaseSnapshot(json::parse(&snapshot.to_string()).expect("JSON")),
            evidence: records
                .iter()
                .map(|record| EvidenceRecord(json::parse(&record.to_string()).expect("JSON")))
                .collect(),
            at: None,
            decisions: Vec::new(),
        },
    )
    .unwrap_or_else(|refusal| panic!("{label}: evaluate refused {refusal:?}"));
    for (admission, governor) in governors() {
        let case = open(&governor);
        assert_eq!(case.0, "case-1", "{label}, {admission}");
        for (n, record) in records.iter().enumerate() {
            receive(&governor, &case, n as u64 + 1, record);
        }
        let frontier = governor.frontier(&case).expect("the frontier");
        let document = governor
            .decision_document(&case, &frontier.data().frontier_id)
            .expect("the governor holds the case")
            .expect("the governor keeps the document it just issued");
        assert_eq!(
            decision.canon, document,
            "{label}, {admission}: evaluate decides what the governor decides for the same history"
        );
    }
    decision
        .claims
        .iter()
        .find(|claim| claim.claim == "findings.resolved")
        .expect("findings.resolved")
        .value
        .clone()
}

/// Two records for one child: whichever comes first applies and the later one is set aside, by
/// `evaluate` as by the governor (Canon refuses the two together, `duplicate-child-case`).
#[test]
fn of_two_records_for_one_child_the_first_applies_in_either_order() {
    let open_first = [
        outcome("o1", Some("CHG-1"), "open"),
        outcome("o2", Some("CHG-1"), "accepted"),
    ];
    assert_eq!(
        evaluate_equals_the_governor("open, then accepted", &open_first),
        ClaimValue::Unknown,
        "the open record came first"
    );
    let accepted_first = [
        outcome("o1", Some("CHG-1"), "accepted"),
        outcome("o2", Some("CHG-1"), "open"),
    ];
    assert_eq!(
        evaluate_equals_the_governor("accepted, then open", &accepted_first),
        ClaimValue::True,
        "the accepted record came first"
    );
}

/// Records for two children both apply, by `evaluate` as by the governor.
#[test]
fn records_for_two_children_both_apply() {
    let records = [
        outcome("o1", Some("CHG-1"), "accepted"),
        outcome("o2", Some("CHG-2"), "open"),
    ];
    assert_eq!(
        evaluate_equals_the_governor("two children", &records),
        ClaimValue::Unknown,
        "the second child is still open, so both records were read"
    );
    let reversed = [
        outcome("o1", Some("CHG-1"), "open"),
        outcome("o2", Some("CHG-2"), "declined"),
    ];
    assert_eq!(
        evaluate_equals_the_governor("two children, one open", &reversed),
        ClaimValue::Unknown
    );
    let ended = [
        outcome("o1", Some("CHG-1"), "accepted"),
        outcome("o2", Some("CHG-2"), "declined"),
    ];
    assert_eq!(
        evaluate_equals_the_governor("two children, both ended", &ended),
        ClaimValue::True,
        "both records apply: every child ended in the set"
    );
}

/// A record set aside on its own (no child named) and a second record for one child, in one
/// history: both are set aside and the decision is the governor's.
#[test]
fn a_record_set_aside_alone_and_one_set_aside_with_the_rest_are_both_set_aside() {
    let records = [
        outcome("o0", None, "accepted"),
        outcome("o1", Some("CHG-1"), "open"),
        outcome("o2", Some("CHG-1"), "accepted"),
        outcome("o3", Some("CHG-2"), "accepted"),
    ];
    assert_eq!(
        evaluate_equals_the_governor("mixed", &records),
        ClaimValue::Unknown,
        "CHG-1 is still open by its first record"
    );
}
