//! Acceptance for `story:canon-0-10-0-pin` and `story:canon-0-11-0-pin`: Loom decides with Canon
//! 0.11.0. Every decision document it hands out names `0.11.0`, lists an approval-required action
//! under `pending_decisions` and an admissible action's missing kinds under `missing_evidence`;
//! Canon reads `canon-authority/1` entries keyed by capability, scope, case revision and
//! principal; a protocol using a 0.10.0 member registers and is decided, its judgement marks
//! changing no frontier; a protocol with `imports` is refused, naming Canon's `MissingImport`. A
//! protocol using a 0.11.0 member registers and is decided; a decision counts only while its `when`
//! holds; a `descended` form is `unknown` in a case the governor holds, which gives no
//! `descends_from`.
//!
//! The protocols are written here in the forms of Canon's fixtures at tag `0.10.0`
//! (`fixtures/investigation/capability-scope.yaml`, `supplied-evidence/`,
//! `judgement-marking.yaml`, `child-outcome-subject.yaml`, `pending-decisions.yaml`,
//! `missing-evidence.yaml`) and at tag `0.11.0` (`fixtures/investigation/decision-condition.yaml`,
//! `revision-ordering.yaml`). Three catalogs are built at most once per test process (`OnceLock`,
//! a few milliseconds), the one of `pin-check@1` apart, so a test of the decision document fails on
//! another Canon for its own reason; no test makes a model, network or clock call.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use b10x_canon::eval::{Refusal, Supplied, case_from_value, evaluate_with};
use b10x_canon::ir::compile;
use b10x_canon::model::Decision;
use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, Frontier, Observation, ObservationData, ObservationId,
    frontier_state,
};
use b10x_loom_commission::ports::evidence::{ObservationPort as _, submit_evidence};
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::model::evaluation::{
    CaseSnapshot, EvaluationRequest, EvidenceRecord, ProtocolName,
};
use loom_governor::model::json;
use loom_governor::{CanonGovernor, CaseInputs, MemoryCaseStore, evaluate};
use loom_protocols::{ProtocolCatalog, ProtocolSource, SourceKind};
use serde_json::{Value, json};

/// A protocol with an approval-required action and an admissible one that may produce `review`.
const PIN: &str = "pin-check@1";
const PIN_YAML: &str = r#"
format: protocol/1
protocol: {id: pin.check, revision: 1}
artifacts: {change: {}}
evidence_kinds: {review: {}}
claims:
  change.reviewed:
    true_when: {evidence: {kind: review}}
actions:
  inspect:
    may_produce:
      - evidence: review
  merge:
    requires:
      - capability: repository.merge
outcomes:
  reviewed:
    requires: {claim: change.reviewed}
"#;

/// A protocol with every 0.10.0 member the story names: a scoped capability requirement, a
/// `supplied_by` kind, `author`, `revises`, `decided_by`, and a `children` form with `subject`
/// and `revision: current`.
const MEMBERS: &str = "pin-members@1";
const MEMBERS_YAML: &str = r#"
format: protocol/1
protocol: {id: pin.members, revision: 1}
artifacts: {spec: {}, wave: {}}
evidence_kinds:
  spec_source: {}
  spec_review: {}
  peer_review: {supplied_by: pin.review}
  case_outcome: {}
claims:
  spec.drafted:
    true_when: {evidence: {kind: spec_source}}
  peer.approved:
    true_when: {evidence: {kind: peer_review, result: approved}}
  wave.passed:
    true_when:
      children: {in: [passed], subject: wave, revision: current}
actions:
  draft_spec:
    author: judgement
    effect: write
    revises: [spec]
    may_produce:
      - evidence: spec_source
        decided_by: program
  review_spec:
    author: judgement
    precondition: {claim: spec.drafted}
    may_produce:
      - evidence: spec_review
        decided_by: judgement
  inspect:
    requires:
      - capability: metrics.read
        scope: {cluster: prod-eu, namespace: foo}
  stories.open:
    case_open: {protocol: story, subject: wave}
outcomes:
  wave.done:
    requires: {claim: wave.passed}
"#;

/// `MEMBERS_YAML` without `author`, `revises` and `decided_by`.
const UNMARKED: &str = "pin-unmarked@1";
const UNMARKED_YAML: &str = r#"
format: protocol/1
protocol: {id: pin.unmarked, revision: 1}
artifacts: {spec: {}, wave: {}}
evidence_kinds:
  spec_source: {}
  spec_review: {}
  peer_review: {supplied_by: pin.review}
  case_outcome: {}
claims:
  spec.drafted:
    true_when: {evidence: {kind: spec_source}}
  peer.approved:
    true_when: {evidence: {kind: peer_review, result: approved}}
  wave.passed:
    true_when:
      children: {in: [passed], subject: wave, revision: current}
actions:
  draft_spec:
    effect: write
    may_produce:
      - evidence: spec_source
  review_spec:
    precondition: {claim: spec.drafted}
    may_produce:
      - evidence: spec_review
  inspect:
    requires:
      - capability: metrics.read
        scope: {cluster: prod-eu, namespace: foo}
  stories.open:
    case_open: {protocol: story, subject: wave}
outcomes:
  wave.done:
    requires: {claim: wave.passed}
"#;

/// A protocol importing one Loom is not given.
const IMPORTS: &str = "pin-imports@1";
const IMPORTS_YAML: &str = r#"
format: protocol/1
protocol: {id: pin.imports, revision: 1}
imports:
  pin.review:
    revision: 1
    digest: sha256:1a2cc72944b479e53776adc362125e87607f587ce05b3d1cee4d95186ba0ab68
artifacts: {change: {}}
actions:
  inspect: {}
"#;

/// A protocol with every 0.11.0 member the story names: `revised_by` on an artifact,
/// `records_together` on an action, a claim over a `descended` form, and an outcome requiring a
/// decision with a `when`.
const ELEVEN: &str = "pin-eleven@1";
const ELEVEN_YAML: &str = r#"
format: protocol/1
protocol: {id: pin.eleven, revision: 1}
artifacts:
  change: {revised_by: pin.revise}
evidence_kinds:
  publication: {}
  test_result: {}
claims:
  refused:
    true_when: {evidence: {kind: publication, result: refused}}
  red.then.green:
    true_when:
      descended:
        first: {kind: test_result, result: failed}
        then: {kind: test_result, result: passed}
        subject: change
actions:
  submit:
    may_produce:
      - evidence: publication
  test:
    may_produce:
      - evidence: test_result
      - evidence: publication
    records_together:
      - kinds: [test_result, publication]
outcomes:
  withdrawn:
    requires: {decision: withdraw, when: {claim: refused}}
  done:
    requires: {claim: red.then.green}
"#;

fn source() -> ProtocolSource {
    ProtocolSource {
        kind: SourceKind::Memory,
        location: "canon-0-10-0-fixture".into(),
        revision: String::new(),
        path: String::new(),
    }
}

fn catalog_of(protocols: &[(&str, &str)]) -> ProtocolCatalog {
    let mut catalog = ProtocolCatalog::default();
    for (name, yaml) in protocols {
        catalog
            .add_yaml(name, yaml, source())
            .unwrap_or_else(|error| panic!("{name} is added to the catalog: {error}"));
    }
    catalog
}

/// The catalog holding `pin-check@1`, which uses no 0.10.0 member, so a test reading it fails on
/// another Canon for its own reason.
fn catalog() -> &'static ProtocolCatalog {
    static CATALOG: OnceLock<ProtocolCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| catalog_of(&[(PIN, PIN_YAML)]))
}

/// The catalog holding the protocols that use 0.10.0 members.
fn members_catalog() -> &'static ProtocolCatalog {
    static CATALOG: OnceLock<ProtocolCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| catalog_of(&[(MEMBERS, MEMBERS_YAML), (UNMARKED, UNMARKED_YAML)]))
}

/// The catalog holding the protocol that uses 0.11.0 members.
fn eleven_catalog() -> &'static ProtocolCatalog {
    static CATALOG: OnceLock<ProtocolCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| catalog_of(&[(ELEVEN, ELEVEN_YAML)]))
}

fn governor(name: &str, yaml: &str) -> CanonGovernor<MemoryCaseStore> {
    CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml(name, yaml)
        .unwrap_or_else(|error| panic!("{name} registers: {error:?}"))
}

fn open(governor: &CanonGovernor<MemoryCaseStore>, name: &str, artifacts: &[&str]) -> CaseId {
    governor
        .open(
            name,
            artifacts
                .iter()
                .map(|artifact| ((*artifact).to_owned(), "a1".to_owned()))
                .collect::<BTreeMap<_, _>>(),
            CaseInputs::none(),
        )
        .unwrap_or_else(|error| panic!("a case of {name} opens: {error:?}"))
}

/// `document` as `serde_json`, written exactly as it was read.
fn serde(document: &json::Value) -> Value {
    let mut out = String::new();
    json::push_value(&mut out, document);
    serde_json::from_str(&out).expect("the document is JSON")
}

fn snapshot(protocol: &str) -> Value {
    json!({
        "format": "canon-case/1",
        "id": "case-1",
        "protocol": protocol,
        "artifacts": {"change": {"revision": "c1"}},
        "revision": "r1",
    })
}

fn review(id: &str) -> Value {
    json!({
        "format": "canon-evidence/1",
        "id": id,
        "kind": "review",
        "subject": "change",
        "subject_revision": "c1",
    })
}

/// The `canon` member `evaluate` returns for a case of `pin-check@1` with `records`.
fn evaluated(records: &[Value]) -> Value {
    let request = EvaluationRequest {
        protocol: ProtocolName(PIN.into()),
        snapshot: CaseSnapshot(json::parse(&snapshot("pin.check").to_string()).expect("JSON")),
        evidence: records
            .iter()
            .map(|record| EvidenceRecord(json::parse(&record.to_string()).expect("JSON")))
            .collect(),
        at: None,
        decisions: Vec::new(),
    };
    let decision = evaluate(catalog(), &request)
        .unwrap_or_else(|refusal| panic!("evaluate decides: {refusal:?}"));
    serde(&decision.canon)
}

/// Canon's decision for a case of `pin-check@1` at `r1` with `authority` supplied, over the IR the
/// catalog entry compiles.
fn with_authority(authority: &str) -> Result<Decision, Refusal> {
    let ir = compile(&catalog().get(PIN).expect("pin-check@1").model).expect("it compiles");
    let yaml = |value: &Value| serde_yaml_ng::to_value(value).expect("JSON is YAML");
    evaluate_with(
        &ir,
        &case_from_value(&yaml(&snapshot("pin.check"))).expect("the snapshot"),
        &[],
        Supplied {
            authority: Some(authority),
            ..Supplied::default()
        },
    )
}

fn merge_status(authority: &str) -> String {
    let decision = with_authority(authority)
        .unwrap_or_else(|refusal| panic!("Canon reads the authority: {refusal}"));
    decision.actions.expect("an actions section")["merge"]["status"]
        .as_str()
        .expect("a status")
        .to_owned()
}

#[test]
fn the_decision_document_names_canon_0_11_0() {
    let canon = evaluated(&[]);
    assert_eq!(
        canon["explanation"]["computed_from"]["semantics"]["canon"],
        json!("0.11.0"),
        "{canon:#}"
    );
}

#[test]
fn an_approval_required_action_is_listed_under_pending_decisions() {
    let governor = governor(PIN, PIN_YAML);
    let case = open(&governor, PIN, &["change"]);
    let frontier = governor.frontier(&case).expect("the frontier");
    let document = governor
        .decision_document(&case, &frontier.data().frontier_id)
        .expect("the governor holds the case")
        .expect("the governor keeps the document it just issued");
    let document = serde(&document);
    assert_eq!(
        document["pending_decisions"]["authority"],
        json!([{
            "action": "merge",
            "capability": "repository.merge",
            "current": "none",
            "case_revision": "r1",
            "options": ["granted", "denied"],
        }]),
        "{document:#}"
    );
}

#[test]
fn an_admissible_action_lists_its_missing_evidence() {
    let before = evaluated(&[]);
    assert_eq!(
        before["missing_evidence"],
        json!({"inspect": ["review"]}),
        "{before:#}"
    );
    let after = evaluated(&[review("e1")]);
    assert_eq!(
        after["missing_evidence"],
        json!({"inspect": []}),
        "{after:#}"
    );
}

#[test]
fn canon_reads_authority_entries_by_principal_and_case_revision() {
    let granted =
        "- {capability: repository.merge, decision: granted, principal: lead, case_revision: r1}\n";
    assert_eq!(merge_status("[]"), "approval-required", "nothing granted");
    assert_eq!(merge_status(granted), "admissible", "granted at r1");

    let denied_by_another = format!(
        "{granted}- {{capability: repository.merge, decision: denied, principal: owner, case_revision: r1}}\n"
    );
    assert_eq!(
        merge_status(&denied_by_another),
        "blocked",
        "a denial by another principal is a second entry, and denied comes first"
    );

    let at_r2 =
        "- {capability: repository.merge, decision: granted, principal: lead, case_revision: r2}\n";
    assert_eq!(
        merge_status(at_r2),
        "approval-required",
        "an entry bound to r2 does not apply at r1"
    );

    let granted_by_another = format!(
        "{granted}- {{capability: repository.merge, decision: granted, principal: someone-else, case_revision: r1}}\n"
    );
    assert_eq!(
        merge_status(&granted_by_another),
        "admissible",
        "the principal is matched against nothing"
    );
    let another_alone = "- {capability: repository.merge, decision: granted, principal: someone-else, case_revision: r1}\n";
    assert_eq!(
        merge_status(another_alone),
        merge_status(granted),
        "another principal's entry decides as the first one's"
    );
}

#[test]
fn two_authority_entries_equal_in_their_four_keys_are_refused() {
    let twice = "\
- {capability: repository.merge, decision: granted, principal: lead, case_revision: r1}
- {capability: repository.merge, decision: denied, principal: lead, case_revision: r1}
";
    let refusal = with_authority(twice).expect_err("two entries equal in their four keys");
    assert_eq!(refusal.code(), "duplicate-identifier", "{refusal}");
}

fn frontier_of(name: &str, yaml: &str) -> (Frontier<frontier_state::Issued>, Value) {
    let governor = governor(name, yaml);
    let case = open(&governor, name, &["spec", "wave"]);
    let frontier = governor.frontier(&case).expect("the frontier");
    let document = governor
        .decision_document(&case, &frontier.data().frontier_id)
        .expect("the governor holds the case")
        .expect("the governor keeps the document it just issued");
    (frontier, serde(&document))
}

#[test]
fn a_protocol_with_0_10_0_members_registers_and_is_decided() {
    assert!(
        members_catalog().get(MEMBERS).is_some(),
        "ProtocolCatalog::add_yaml holds it"
    );
    let from_catalog = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(members_catalog())
        .expect("the catalog holding it is admitted");
    let case = open(&from_catalog, MEMBERS, &["spec", "wave"]);
    let catalog_frontier = from_catalog.frontier(&case).expect("the frontier");

    let (frontier, _) = frontier_of(MEMBERS, MEMBERS_YAML);
    for (admission, frontier) in [
        ("with_protocol_yaml", &frontier),
        ("with_catalog", &catalog_frontier),
    ] {
        let inspect = frontier
            .data()
            .actions
            .iter()
            .find(|entry| entry.action == "inspect")
            .unwrap_or_else(|| panic!("{admission}: the frontier lists inspect"));
        assert_eq!(
            (inspect.status, inspect.capability.as_deref()),
            (
                b10x_loom_commission::model::responsibility::ActionStatus::ApprovalRequired,
                Some("metrics.read")
            ),
            "{admission}"
        );
        let reasons: Vec<Value> = inspect
            .reasons
            .iter()
            .map(|reason| serde_json::from_str(reason).expect("a reason is JSON"))
            .collect();
        assert_eq!(
            reasons,
            [json!({
                "capability": "metrics.read",
                "decision": "none",
                "scope": {"cluster": "prod-eu", "namespace": "foo"},
            })],
            "{admission}: the reason names the scope"
        );
    }
}

#[test]
fn judgement_marks_change_no_frontier() {
    let (marked, marked_document) = frontier_of(MEMBERS, MEMBERS_YAML);
    let (unmarked, unmarked_document) = frontier_of(UNMARKED, UNMARKED_YAML);
    assert_eq!(marked.data().actions, unmarked.data().actions);
    assert_eq!(marked.data().claims, unmarked.data().claims);
    assert_eq!(
        marked_document["actions"], unmarked_document["actions"],
        "the actions section"
    );
    assert!(
        marked_document["actions"].is_object(),
        "{marked_document:#}"
    );
}

#[test]
fn a_protocol_with_imports_is_refused_naming_missing_import() {
    let mut catalog = ProtocolCatalog::default();
    let refusal = catalog
        .add_yaml(IMPORTS, IMPORTS_YAML, source())
        .expect_err("a protocol with imports is refused");
    assert!(
        refusal.contains("MissingImport") && refusal.contains("pin.review"),
        "{refusal}"
    );
    assert!(
        catalog.get(IMPORTS).is_none(),
        "the catalog does not hold it"
    );

    let refused = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml(IMPORTS, IMPORTS_YAML)
        .expect_err("the governor refuses it too");
    assert!(format!("{refused:?}").contains("pin.review"), "{refused:?}");
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A `canon-evidence/1` record of `kind` with `result` about `change` at `revision`, naming `item`.
fn change_record(id: &str, kind: &str, result: &str, item: &str, revision: &str) -> Value {
    json!({
        "format": "canon-evidence/1",
        "id": id,
        "kind": kind,
        "result": result,
        "item": item,
        "subject": "change",
        "subject_revision": revision,
    })
}

/// Hands the governor `record`, citing an observation of its own that the governor received first.
fn submit(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId, n: u64, record: &Value) {
    let observation = ObservationId(uuid(0x100 + n));
    governor
        .observe(Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: "host/fixture".to_owned(),
            subject: "change".to_owned(),
            observed_at: Timestamp("2026-11-22T10:00:00Z".to_owned()),
            payload: commission_json::Value::Null,
        }))
        .expect("the governor keeps the observation");
    submit_evidence(
        governor,
        "service:ci",
        EvidenceData {
            evidence_id: EvidenceId(uuid(n)),
            case_id: case.clone(),
            kind: record["kind"].as_str().expect("kind").to_owned(),
            subject_revision: governor.current_revision(case).expect("revision"),
            producer: String::new(),
            observation_ids: vec![observation],
            facts: commission_json::parse(&record.to_string()).expect("JSON"),
            provenance: commission_json::Value::Null,
        },
    )
    .expect("the governor takes the evidence");
}

/// The decision document behind the frontier the governor issues for `case` now.
fn document_of(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId) -> Value {
    let frontier = governor.frontier(case).expect("the frontier");
    let document = governor
        .decision_document(case, &frontier.data().frontier_id)
        .expect("the governor holds the case")
        .expect("the governor keeps the document it just issued");
    serde(&document)
}

/// The explicit decisions `document` lists as pending.
fn pending_decisions(document: &Value) -> Vec<Value> {
    document["pending_decisions"]["decisions"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn a_protocol_with_0_11_0_members_registers_and_is_decided() {
    assert!(
        eleven_catalog().get(ELEVEN).is_some(),
        "ProtocolCatalog::add_yaml holds it"
    );
    let from_catalog = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(eleven_catalog())
        .expect("the catalog holding it is admitted");
    let from_yaml = governor(ELEVEN, ELEVEN_YAML);
    for (admission, governor) in [
        ("with_catalog", &from_catalog),
        ("with_protocol_yaml", &from_yaml),
    ] {
        let case = open(governor, ELEVEN, &["change"]);
        let frontier = governor.frontier(&case).expect("the frontier");
        let actions: Vec<&str> = frontier
            .data()
            .actions
            .iter()
            .map(|entry| entry.action.as_str())
            .collect();
        assert_eq!(actions, ["submit", "test"], "{admission}");
        let document = document_of(governor, &case);
        assert_eq!(document["format"], json!("canon-decision/1"), "{admission}");
        assert_eq!(
            document["claims"]["red.then.green"],
            json!({"value": "unknown"}),
            "{admission}: {document:#}"
        );
        assert_eq!(
            document["outcomes"]["withdrawn"]["status"],
            json!("blocked"),
            "{admission}: {document:#}"
        );
    }
}

#[test]
fn a_decision_counts_only_while_its_when_holds() {
    let governor = governor(ELEVEN, ELEVEN_YAML);
    let case = open(&governor, ELEVEN, &["change"]);

    let before = document_of(&governor, &case);
    assert_eq!(
        before["outcomes"]["withdrawn"],
        json!({
            "reasons": [{"claim": "refused", "value": "unknown"}],
            "status": "blocked",
        }),
        "blocked by the claim of `when`: {before:#}"
    );
    assert_eq!(
        pending_decisions(&before),
        Vec::<Value>::new(),
        "no decision is pending while `when` is not true: {before:#}"
    );

    submit(
        &governor,
        &case,
        1,
        &change_record("pub-1", "publication", "refused", "p1", "a1"),
    );
    let after = document_of(&governor, &case);
    assert_eq!(
        after["claims"]["refused"],
        json!({"value": "true"}),
        "{after:#}"
    );
    assert_eq!(
        after["outcomes"]["withdrawn"],
        json!({
            "reasons": [{"decision": "withdraw", "present": false}],
            "status": "blocked",
        }),
        "blocked naming the decision: {after:#}"
    );
    let revision = format!(
        "r{}",
        governor.current_revision(&case).expect("the revision")
    );
    assert_eq!(
        pending_decisions(&after),
        [json!({
            "case_revision": revision,
            "decision": "withdraw",
            "options": ["taken"],
            "outcome": "withdrawn",
        })],
        "{after:#}"
    );
}

#[test]
fn a_descended_form_is_unknown_in_a_loom_case() {
    let governor = governor(ELEVEN, ELEVEN_YAML);
    let case = open(&governor, ELEVEN, &["change"]);
    let failed = change_record("t-failed", "test_result", "failed", "t1", "a1");
    let passed = change_record("t-passed", "test_result", "passed", "t1", "a1");
    submit(&governor, &case, 1, &failed);
    submit(&governor, &case, 2, &passed);
    let document = document_of(&governor, &case);
    assert_eq!(
        document["claims"]["red.then.green"],
        json!({"value": "unknown"}),
        "{document:#}"
    );
    assert_eq!(
        document["explanation"]["computed_from"]["case"]["artifacts"]["change"],
        json!({"revision": "a1"}),
        "the governor gives no descends_from: {document:#}"
    );

    // The same form decides once a snapshot gives the ancestry, which only a caller of `evaluate`
    // can: a failed run about `a0` and a passed one about `a1`, the current revision.
    let decided = |artifact: Value| {
        let snapshot = json!({
            "format": "canon-case/1",
            "id": "case-1",
            "protocol": "pin.eleven",
            "artifacts": {"change": artifact},
            "revision": "r1",
        });
        let request = EvaluationRequest {
            protocol: ProtocolName(ELEVEN.into()),
            snapshot: CaseSnapshot(json::parse(&snapshot.to_string()).expect("JSON")),
            evidence: [
                change_record("t-failed", "test_result", "failed", "t1", "a0"),
                passed.clone(),
            ]
            .iter()
            .map(|record| EvidenceRecord(json::parse(&record.to_string()).expect("JSON")))
            .collect(),
            at: None,
            decisions: Vec::new(),
        };
        let decision = evaluate(eleven_catalog(), &request)
            .unwrap_or_else(|refusal| panic!("evaluate decides: {refusal:?}"));
        serde(&decision.canon)["claims"]["red.then.green"]["value"].clone()
    };
    assert_eq!(
        decided(json!({"revision": "a1", "descends_from": ["a0"]})),
        json!("true")
    );
    assert_eq!(decided(json!({"revision": "a1"})), json!("unknown"));
}
