//! Adversary, pass 1, `story:observation-evidence-ports`: a governor that refuses evidence is
//! reported as refusing it.
//!
//! The fake governor never refuses evidence, so the acceptance suite cannot see whether
//! `submit_evidence` hands the governor's refusal back as `EvidenceError::Governor`. A mutant that
//! drops the port's result (`let _ = port.receive(..); Ok(())`) stays green there, and a refused
//! submission would read as admitted — the opposite of failing toward more explicit uncertainty
//! (`AGENTS.md` § Rules). These cases pin it with a test-local port.

use std::cell::RefCell;

use b10x_commission::model::json::{self, Value};
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, GovernorError, ObservationId,
};
use b10x_commission::ports::evidence::{
    AttributedEvidence, EvidenceError, EvidencePort, submit_evidence,
};

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn parse(text: &str) -> Value {
    json::parse(text).unwrap_or_else(|error| panic!("parse {text}: {error}"))
}

fn record(observations: Vec<ObservationId>) -> EvidenceData {
    EvidenceData {
        evidence_id: EvidenceId(uuid(0xa00)),
        case_id: CaseId("case-refused".to_owned()),
        kind: "test_result".to_owned(),
        subject_revision: 2,
        producer: "claimed".to_owned(),
        observation_ids: observations,
        facts: parse(r#"{"tests.pass": true}"#),
        provenance: parse(r#"{"source": "ci"}"#),
    }
}

/// A port that answers every delivery with `answer` and counts the deliveries it saw.
struct ScriptedPort {
    answer: Result<(), GovernorError>,
    received: RefCell<Vec<EvidenceData>>,
}

impl ScriptedPort {
    fn answering(answer: Result<(), GovernorError>) -> Self {
        Self {
            answer,
            received: RefCell::new(Vec::new()),
        }
    }
}

impl EvidencePort for ScriptedPort {
    fn receive(&self, evidence: AttributedEvidence) -> Result<(), GovernorError> {
        self.received
            .borrow_mut()
            .push(evidence.into_evidence().into_data());
        self.answer
    }
}

#[test]
fn a_governor_refusal_comes_back_as_the_governor_error() {
    for refusal in [
        GovernorError::GovernorUnavailable,
        GovernorError::UnknownCase,
    ] {
        let port = ScriptedPort::answering(Err(refusal));
        assert_eq!(
            submit_evidence(
                &port,
                "service:ci",
                record(vec![ObservationId(uuid(0xa01))])
            ),
            Err(EvidenceError::Governor(refusal)),
            "the governor refused with {refusal:?}; the submitter must be told so"
        );
        assert_eq!(port.received.borrow().len(), 1, "delivered exactly once");
    }
}

#[test]
fn an_accepted_record_is_delivered_once_and_a_refused_one_never() {
    let port = ScriptedPort::answering(Ok(()));
    assert_eq!(
        submit_evidence(
            &port,
            "service:ci",
            record(vec![ObservationId(uuid(0xa02))])
        ),
        Ok(())
    );
    assert_eq!(port.received.borrow().len(), 1);
    assert_eq!(port.received.borrow()[0].producer, "service:ci");

    assert_eq!(
        submit_evidence(&port, "service:ci", record(Vec::new())),
        Err(EvidenceError::NoObservation)
    );
    assert_eq!(
        port.received.borrow().len(),
        1,
        "a record naming no observation never reaches the port, whatever port it is"
    );
}
