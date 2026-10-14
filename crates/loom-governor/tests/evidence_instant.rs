//! A record's `observed_at` is applied only when it is the instant of the observation the record
//! cites, and that observation reached the governor (`story:regression-evidence-integrity`).
//! Otherwise the governor refuses the record, naming it, and keeps nothing of it.

use std::collections::BTreeMap;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, GovernorError, Observation, ObservationData, ObservationId,
};
use b10x_loom_commission::ports::evidence::{EvidenceError, ObservationPort, submit_evidence};
use loom_governor::{CanonGovernor, CaseInputs, EvidenceRefusal, MemoryCaseStore};

const PROTOCOL: &str = "software-change@1";
const PRODUCER: &str = "service:ci";
const OBSERVED: &str = "2026-10-14T09:00:05Z";

fn governor() -> (CanonGovernor<MemoryCaseStore>, CaseId) {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = CaseId("chg-instant".to_owned());
    governor
        .open_case(
            case.clone(),
            PROTOCOL,
            [
                "intent",
                "system_specification",
                "plan",
                "implementation",
                "release",
                "deployment",
            ]
            .into_iter()
            .map(|artifact| (artifact.to_owned(), "r1".to_owned()))
            .collect::<BTreeMap<_, _>>(),
            CaseInputs::try_from_pairs([
                ("risk", "standard"),
                ("change_kind", "bugfix"),
                ("affects_security_boundary", "false"),
            ])
            .expect("inputs"),
        )
        .expect("the case opens");
    (governor, case)
}

fn observe(governor: &CanonGovernor<MemoryCaseStore>, id: &str, at: &str) {
    governor
        .observe(Observation::new(ObservationData {
            observation_id: ObservationId(Uuid(id.to_owned())),
            source: "host/ci".to_owned(),
            subject: "implementation".to_owned(),
            observed_at: Timestamp(at.to_owned()),
            payload: Value::Object(Vec::new()),
        }))
        .expect("the observation is kept");
}

fn record(evidence: &str, observation: &str, observed_at: &str) -> EvidenceData {
    let text = |value: &str| Value::Text(value.to_owned());
    EvidenceData {
        evidence_id: EvidenceId(Uuid(evidence.to_owned())),
        case_id: CaseId("chg-instant".to_owned()),
        kind: "test_result".to_owned(),
        subject_revision: 1,
        producer: PRODUCER.to_owned(),
        observation_ids: vec![ObservationId(Uuid(observation.to_owned()))],
        facts: Value::Object(vec![
            ("format".to_owned(), text("canon-evidence/1")),
            ("id".to_owned(), text(&format!("test-result-{evidence}"))),
            ("kind".to_owned(), text("test_result")),
            ("subject".to_owned(), text("implementation")),
            ("subject_revision".to_owned(), text("r1")),
            ("result".to_owned(), text("fail")),
            ("observed_at".to_owned(), text(observed_at)),
        ]),
        provenance: Value::Null,
    }
}

#[test]
fn a_record_whose_observed_at_is_its_observations_instant_is_received() {
    let (governor, case) = governor();
    observe(&governor, "obs-1", OBSERVED);
    let data = record("ev-1", "obs-1", OBSERVED);
    assert_eq!(governor.check_evidence(&data), Ok(()));
    submit_evidence(&governor, PRODUCER, data).expect("the record is received");
    assert_eq!(governor.evidence(&case).expect("held").len(), 1);
}

#[test]
fn a_record_whose_observed_at_differs_from_its_observations_instant_is_refused_naming_it() {
    let (governor, case) = governor();
    observe(&governor, "obs-1", OBSERVED);
    let data = record("ev-early", "obs-1", "2026-10-14T08:00:00Z");
    let refusal = governor
        .check_evidence(&data)
        .expect_err("an instant other than the observation's is refused");
    assert_eq!(
        refusal,
        EvidenceRefusal::ObservedAtDiffers {
            evidence: "ev-early".to_owned(),
            observation: "obs-1".to_owned(),
            observed_at: "2026-10-14T08:00:00Z".to_owned(),
            observation_at: OBSERVED.to_owned(),
        }
    );
    assert!(
        refusal.to_string().contains("ev-early"),
        "the refusal names the record: {refusal}"
    );
    assert_eq!(
        submit_evidence(&governor, PRODUCER, data),
        Err(EvidenceError::Governor(GovernorError::GovernorUnavailable)),
        "the governor does not take it"
    );
    assert!(
        governor.evidence(&case).expect("held").is_empty(),
        "nothing of the refused record is kept"
    );
}

#[test]
fn a_record_citing_an_observation_the_governor_never_received_is_refused_naming_it() {
    let (governor, case) = governor();
    observe(&governor, "obs-1", OBSERVED);
    let data = record("ev-unseen", "obs-never", OBSERVED);
    let refusal = governor
        .check_evidence(&data)
        .expect_err("an observation the governor never received is refused");
    assert_eq!(
        refusal,
        EvidenceRefusal::ObservationNotReceived {
            evidence: "ev-unseen".to_owned(),
            observation: "obs-never".to_owned(),
        }
    );
    assert!(
        refusal.to_string().contains("ev-unseen"),
        "the refusal names the record: {refusal}"
    );
    assert_eq!(
        submit_evidence(&governor, PRODUCER, data),
        Err(EvidenceError::Governor(GovernorError::GovernorUnavailable)),
        "the governor does not take it"
    );
    assert!(
        governor.evidence(&case).expect("held").is_empty(),
        "nothing of the refused record is kept"
    );
}
