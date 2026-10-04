//! Adversary, pass 2, `story:observation-evidence-ports`: which refusal a record wrong in more than
//! one way gets.
//!
//! `submit_evidence`'s documentation lists its refusals in one order: `NoProducer`, then
//! `NoObservation`, both "before anything reaches the port", then `Governor`. The unit's suite
//! tests each refusal with the other inputs valid, so swapping the two local checks, or asking the
//! port first, leaves it green. These cases pin the documented order: a record wrong in several
//! ways is refused with the earliest refusal in that list, and the port is never asked.

use std::cell::Cell;

use b10x_commission::model::json::{self, Value};
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, GovernorError, ObservationId,
};
use b10x_commission::ports::evidence::{
    AttributedEvidence, EvidenceError, EvidencePort, submit_evidence,
};
use b10x_commission_testkit::fake_governor::FakeGovernor;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn parse(text: &str) -> Value {
    json::parse(text).unwrap_or_else(|error| panic!("parse {text}: {error}"))
}

fn record(observations: Vec<ObservationId>) -> EvidenceData {
    EvidenceData {
        evidence_id: EvidenceId(uuid(0xc00)),
        case_id: CaseId("case-nobody-holds".to_owned()),
        kind: "test_result".to_owned(),
        subject_revision: 1,
        producer: "claimed".to_owned(),
        observation_ids: observations,
        facts: parse(r#"{"tests.pass": true}"#),
        provenance: parse(r#"{"source": "ci"}"#),
    }
}

/// A port that refuses everything and counts how often it was asked.
struct RefusingPort {
    asked: Cell<usize>,
}

impl EvidencePort for RefusingPort {
    fn receive(&self, _evidence: AttributedEvidence) -> Result<(), GovernorError> {
        self.asked.set(self.asked.get() + 1);
        Err(GovernorError::GovernorUnavailable)
    }
}

#[test]
fn no_producer_comes_before_no_observation_and_before_the_port() {
    let port = RefusingPort {
        asked: Cell::new(0),
    };
    assert_eq!(
        submit_evidence(&port, " ", record(Vec::new())),
        Err(EvidenceError::NoProducer),
        "blank producer and no observation: the first documented refusal wins"
    );
    assert_eq!(
        submit_evidence(&port, "", record(vec![ObservationId(uuid(0xc01))])),
        Err(EvidenceError::NoProducer),
        "blank producer on a record the port would refuse"
    );
    assert_eq!(
        submit_evidence(&port, "service:ci", record(Vec::new())),
        Err(EvidenceError::NoObservation),
        "no observation on a record the port would refuse"
    );
    assert_eq!(
        port.asked.get(),
        0,
        "a locally refused record never reaches the port"
    );
}

#[test]
fn on_the_fake_a_local_refusal_hides_the_unknown_case() {
    let governor = FakeGovernor::new();
    assert_eq!(
        submit_evidence(&governor, "\t", record(Vec::new())),
        Err(EvidenceError::NoProducer)
    );
    assert_eq!(
        submit_evidence(&governor, "service:ci", record(Vec::new())),
        Err(EvidenceError::NoObservation)
    );
    assert_eq!(
        submit_evidence(
            &governor,
            "service:ci",
            record(vec![ObservationId(uuid(0xc02))])
        ),
        Err(EvidenceError::Governor(GovernorError::UnknownCase)),
        "only a record that passes both local checks reaches the fake"
    );
    assert_eq!(governor.evidence(), Vec::<EvidenceData>::new());
}
