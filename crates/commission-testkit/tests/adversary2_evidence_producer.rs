//! Adversary, pass 2, `story:observation-evidence-ports`: the producer an admitted record carries is
//! the id the trusted caller named.
//!
//! `submit_evidence` refuses a producer whose `trim()` is empty (`EvidenceError::NoProducer`), so it
//! treats surrounding whitespace as naming nothing. It then stores the producer untrimmed. A trusted
//! id read with a trailing newline is admitted as `"P1\n"`, which compares unequal to `"P1"` in
//! every later attribution check. The two halves disagree about whether whitespace is part of the
//! id. Either refusing the padded value or storing the trimmed one makes the first case green.
//!
//! A producer made only of characters that are invisible but not Unicode `White_Space` — a zero
//! width space, a byte-order mark, NUL — names nobody just as `" "` does, and is admitted.

use b10x_commission::model::json::{self, Value};
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{CaseId, EvidenceData, EvidenceId, ObservationId};
use b10x_commission::ports::evidence::{EvidenceError, submit_evidence};
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};

const CASE: &str = "case-producer";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn parse(text: &str) -> Value {
    json::parse(text).unwrap_or_else(|error| panic!("parse {text}: {error}"))
}

fn record() -> EvidenceData {
    EvidenceData {
        evidence_id: EvidenceId(uuid(0xb00)),
        case_id: CaseId(CASE.to_owned()),
        kind: "test_result".to_owned(),
        subject_revision: 1,
        producer: "claimed-by-payload".to_owned(),
        observation_ids: vec![ObservationId(uuid(0xb01))],
        facts: parse(r#"{"tests.pass": true}"#),
        provenance: parse(r#"{"source": "ci"}"#),
    }
}

fn governor() -> FakeGovernor {
    let governor = FakeGovernor::new();
    governor.script(CaseId(CASE.to_owned()), [Answer::at(1)]);
    governor
}

#[test]
fn a_padded_trusted_producer_is_refused_or_arrives_as_the_id_it_names() {
    for trusted in ["P1\n", " P1", "P1 ", "\tP1\r\n"] {
        let governor = governor();
        let result = submit_evidence(&governor, trusted, record());
        if result.is_ok() {
            let arrived: Vec<String> = governor
                .evidence()
                .into_iter()
                .map(|evidence| evidence.producer)
                .collect();
            assert_eq!(
                arrived,
                vec![trusted.trim().to_owned()],
                "the trusted producer {trusted:?} passed the blank check as {:?}, so whitespace \
                 is not part of the id, yet the record carries it",
                trusted.trim()
            );
        } else {
            assert_eq!(result, Err(EvidenceError::NoProducer));
        }
    }
}

#[test]
fn an_invisible_trusted_producer_names_nobody() {
    for trusted in ["\u{200B}", "\u{FEFF}", "\u{0}", "\u{200B}\u{2060}"] {
        let governor = governor();
        assert_eq!(
            submit_evidence(&governor, trusted, record()),
            Err(EvidenceError::NoProducer),
            "the trusted producer {trusted:?} renders as nothing and names nobody; the governor \
             recorded {:?}",
            governor.evidence()
        );
    }
}
