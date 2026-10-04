//! Adversary, pass 1, `story:observation-evidence-ports`: evidence is attributable.
//!
//! `submit_evidence` overwrites the producer with the one the trusted caller supplies. Evidence is
//! "typed and attributable" (`docs/contracts/evidence.md`, the story's Outcome), and
//! `AttributedEvidence` promises "its producer is the trusted caller's". A producer that names
//! nobody is not an attribution: the port must not admit a record attributed to the empty string.

use b10x_commission::model::json::{self, Value};
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{CaseId, EvidenceData, EvidenceId, ObservationId};
use b10x_commission::ports::evidence::submit_evidence;
use b10x_commission_testkit::fake_governor::FakeGovernor;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn parse(text: &str) -> Value {
    json::parse(text).unwrap_or_else(|error| panic!("parse {text}: {error}"))
}

fn record(claimed_producer: &str) -> EvidenceData {
    EvidenceData {
        evidence_id: EvidenceId(uuid(0x900)),
        case_id: CaseId("case-attributed".to_owned()),
        kind: "test_result".to_owned(),
        subject_revision: 1,
        producer: claimed_producer.to_owned(),
        observation_ids: vec![ObservationId(uuid(0x901))],
        facts: parse(r#"{"tests.pass": true}"#),
        provenance: parse(r#"{"source": "ci"}"#),
    }
}

#[test]
fn evidence_attributed_to_nobody_is_refused() {
    for trusted in ["", "   "] {
        let governor = FakeGovernor::new();
        let result = submit_evidence(&governor, trusted, record("service:ci"));
        assert!(
            result.is_err(),
            "evidence attributed by the trusted caller to {trusted:?} names no producer, yet it \
             was admitted: {result:?}; the governor recorded {:?}",
            governor.evidence()
        );
        assert_eq!(
            governor.evidence(),
            Vec::<EvidenceData>::new(),
            "a refused record never reaches the governor"
        );
    }
}
