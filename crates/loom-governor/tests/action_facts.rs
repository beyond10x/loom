//! Acceptance for `story:deterministic-default-selector`, the governor half: `CanonGovernor`
//! answers a case's action facts from the protocol's IR and the case's current decision. A kind
//! leaves an action's open evidence once a record of it arrives at the subject's current revision,
//! and is back once the subject's revision moves, with the earlier record still held; a record
//! Canon finds expired at the trusted time counts for nothing either. An action whose effect the
//! protocol does not declare has none in its facts.

use std::collections::BTreeMap;
use std::sync::Mutex;

use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, GovernorError, Observation, ObservationData, ObservationId,
};
use b10x_loom_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_loom_executor::model::run::ActionFacts;
use b10x_loom_executor::{ActionFactsPort, CaseFacts};
use loom_governor::{CanonGovernor, CaseInputs, MemoryCaseStore};

const PROTOCOL: &str = r#"
format: protocol/1
protocol: {id: facts.delivery, revision: 1}
artifacts: {implementation: {}}
evidence_kinds:
  built: {}
  reviewed: {}
  logged: {}
  probed: {max_age: 60s}
claims:
  built:
    true_when:
      evidence: {kind: built, subject: implementation}
  reviewed:
    true_when:
      evidence: {kind: reviewed, result: pass, subject: implementation}
  probed:
    true_when:
      evidence: {kind: probed, subject: implementation}
actions:
  build:
    effect: write
    may_produce:
      - evidence: built
  review:
    effect: read
    may_produce:
      - evidence: reviewed
  log:
    may_produce:
      - evidence: logged
  probe:
    effect: read
    may_produce:
      - evidence: probed
  note: {}
outcomes:
  accepted:
    requires: {claim: reviewed}
"#;

const PRODUCER: &str = "trusted:test-runner";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn revisions() -> BTreeMap<String, String> {
    [("implementation".to_owned(), "r1".to_owned())].into()
}

fn facts(action: &str, effect: Option<&str>, may_produce: &[&str], open: &[&str]) -> ActionFacts {
    ActionFacts {
        action: action.to_owned(),
        effect: effect.map(str::to_owned),
        may_produce: may_produce.iter().map(|kind| (*kind).to_owned()).collect(),
        open_evidence: open.iter().map(|kind| (*kind).to_owned()).collect(),
    }
}

fn open_of<'f>(facts: &'f CaseFacts, action: &str) -> &'f [String] {
    &facts
        .actions
        .iter()
        .find(|entry| entry.action == action)
        .unwrap_or_else(|| panic!("the facts name {action}: {facts:?}"))
        .open_evidence
}

/// Submits one `canon-evidence/1` record of `kind` about `implementation` at `subject_revision`,
/// citing an observation of its own that the governor received first.
fn record<G>(governor: &G, case: &CaseId, n: u64, kind: &str, subject_revision: &str)
where
    G: b10x_loom_commission::ports::evidence::EvidencePort + ObservationPort,
{
    record_named(
        governor,
        case,
        n,
        &format!("{kind}-{n}"),
        kind,
        subject_revision,
    );
}

/// [`record`], with the `canon-evidence/1` id `canon_id`.
fn record_named<G>(
    governor: &G,
    case: &CaseId,
    n: u64,
    canon_id: &str,
    kind: &str,
    subject_revision: &str,
) where
    G: b10x_loom_commission::ports::evidence::EvidencePort + ObservationPort,
{
    let observation = ObservationId(uuid(0x100 + n));
    ObservationPort::observe(
        governor,
        Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: PRODUCER.into(),
            subject: "implementation".into(),
            observed_at: Timestamp("2026-10-06T10:00:00Z".into()),
            payload: json::Value::Null,
        }),
    )
    .unwrap();
    let facts = json::parse(&format!(
        r#"{{"format":"canon-evidence/1","id":"{canon_id}","kind":"{kind}","subject":"implementation","subject_revision":"{subject_revision}","result":"pass"}}"#
    ))
    .unwrap();
    submit_evidence(
        governor,
        PRODUCER,
        EvidenceData {
            evidence_id: EvidenceId(uuid(n)),
            case_id: case.clone(),
            kind: kind.to_owned(),
            subject_revision: 1,
            producer: String::new(),
            facts,
            provenance: json::Value::Null,
            observation_ids: vec![observation],
        },
    )
    .unwrap();
}

#[test]
fn a_counted_kind_leaves_open_evidence_and_returns_when_the_subject_moves() {
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml("facts-delivery@1", PROTOCOL)
        .unwrap();
    let case = governor
        .open("facts-delivery@1", revisions(), CaseInputs::none())
        .unwrap();

    let opened = governor.action_facts(&case).unwrap();
    assert_eq!(opened.case_revision, 1);
    assert_eq!(
        opened.actions,
        vec![
            facts("build", Some("write"), &["built"], &["built"]),
            facts("log", None, &["logged"], &["logged"]),
            facts("note", None, &[], &[]),
            facts("probe", Some("read"), &["probed"], &["probed"]),
            facts("review", Some("read"), &["reviewed"], &["reviewed"]),
        ],
        "every declared action in identifier order, its effect only where declared"
    );

    record(&governor, &case, 1, "built", "r1");
    record(&governor, &case, 2, "logged", "r1");
    let counted = governor.action_facts(&case).unwrap();
    assert!(open_of(&counted, "build").is_empty(), "{counted:?}");
    assert!(
        open_of(&counted, "log").is_empty(),
        "a kind no claim reads counts by its binding too: {counted:?}"
    );
    assert_eq!(open_of(&counted, "review"), ["reviewed"]);

    assert_eq!(
        governor
            .update_revision(&case, "implementation", "r2")
            .unwrap(),
        2
    );
    let moved = governor.action_facts(&case).unwrap();
    assert_eq!(moved.case_revision, 2);
    assert_eq!(open_of(&moved, "build"), ["built"]);
    assert_eq!(open_of(&moved, "log"), ["logged"]);
    assert_eq!(
        governor.evidence(&case).unwrap().len(),
        2,
        "the earlier records are still held"
    );

    record(&governor, &case, 3, "built", "r2");
    assert!(open_of(&governor.action_facts(&case).unwrap(), "build").is_empty());

    let through_port = ActionFactsPort::action_facts(&governor, &case).unwrap();
    assert_eq!(through_port, governor.action_facts(&case).unwrap());
}

#[test]
fn an_expired_record_counts_for_nothing() {
    let now = Mutex::new(Some("2026-10-06T10:00:30Z".to_owned()));
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml("facts-delivery@1", PROTOCOL)
        .unwrap()
        .with_evaluation_time(|_: &CaseId| Ok(now.lock().unwrap().clone()));
    let case = governor
        .open("facts-delivery@1", revisions(), CaseInputs::none())
        .unwrap();
    ObservationPort::observe(
        &governor,
        Observation::new(ObservationData {
            observation_id: ObservationId(uuid(0x0b)),
            source: PRODUCER.into(),
            subject: "implementation".into(),
            observed_at: Timestamp("2026-10-06T10:00:00Z".into()),
            payload: json::Value::Null,
        }),
    )
    .unwrap();
    let facts = json::parse(
        r#"{"format":"canon-evidence/1","id":"probed-1","kind":"probed","subject":"implementation","subject_revision":"r1","observed_at":"2026-10-06T10:00:00Z"}"#,
    )
    .unwrap();
    submit_evidence(
        &governor,
        PRODUCER,
        EvidenceData {
            evidence_id: EvidenceId(uuid(0x0c)),
            case_id: case.clone(),
            kind: "probed".into(),
            subject_revision: 1,
            producer: String::new(),
            facts,
            provenance: json::Value::Null,
            observation_ids: vec![ObservationId(uuid(0x0b))],
        },
    )
    .unwrap();
    assert!(open_of(&governor.action_facts(&case).unwrap(), "probe").is_empty());
    *now.lock().unwrap() = Some("2026-10-06T10:02:00Z".into());
    assert_eq!(
        open_of(&governor.action_facts(&case).unwrap(), "probe"),
        ["probed"]
    );
}

#[test]
fn an_unknown_case_has_no_facts() {
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml("facts-delivery@1", PROTOCOL)
        .unwrap();
    assert_eq!(
        governor.action_facts(&CaseId("CASE-NOT-HELD".to_owned())),
        Err(GovernorError::UnknownCase)
    );
    assert!(ActionFactsPort::action_facts(&governor, &CaseId("CASE-NOT-HELD".to_owned())).is_err());
}

#[test]
fn a_held_record_that_does_not_apply_counts_for_nothing() {
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml("facts-delivery@1", PROTOCOL)
        .unwrap();
    let case = governor
        .open("facts-delivery@1", revisions(), CaseInputs::none())
        .unwrap();
    record_named(&governor, &case, 1, "shared-id", "built", "r1");
    // A second record reusing the first one's `canon-evidence/1` id: Canon refuses the pair, so
    // it does not apply, and is kept only.
    record_named(&governor, &case, 2, "shared-id", "reviewed", "r1");
    assert_eq!(
        governor.evidence(&case).unwrap().len(),
        2,
        "both records are held"
    );
    let facts = governor.action_facts(&case).unwrap();
    assert!(open_of(&facts, "build").is_empty(), "{facts:?}");
    assert_eq!(
        open_of(&facts, "review"),
        ["reviewed"],
        "a record that does not apply counts for nothing: {facts:?}"
    );
}
