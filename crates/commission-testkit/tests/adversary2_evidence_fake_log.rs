//! Adversary, pass 2, `story:observation-evidence-ports`: the fake governor's two new records beside
//! its `Governor` call log.
//!
//! The fake's documentation says neither port "takes a scripted answer" and that `calls` logs
//! every `Governor` call. `story:local-runtime-loop` scripts the fake per governor call; a port that
//! consumed an answer or logged a call would shift every later revision in that script. The unit's
//! suite never reads `calls()` after a port call, so a fake whose `receive` answered through the
//! script stays green there. The second case runs the ports and the `Governor` methods from three
//! threads and checks every record against what each thread did, and that no record shrinks
//! between two reads taken during the run.

use std::sync::{Arc, Barrier};
use std::thread;

use b10x_commission::model::json::{self, Value};
use b10x_commission::model::primitives::{Timestamp, Uuid};
use b10x_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, Observation, ObservationData, ObservationId,
};
use b10x_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};

const CASE: &str = "case-logged";
const PER_THREAD: u64 = 5_000;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn parse(text: &str) -> Value {
    json::parse(text).unwrap_or_else(|error| panic!("parse {text}: {error}"))
}

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn observation(n: u64) -> ObservationData {
    ObservationData {
        observation_id: ObservationId(uuid(0xd000_0000 + n)),
        source: "connector:ci".to_owned(),
        subject: format!("{CASE}@1"),
        observed_at: Timestamp("2026-10-04T00:00:00Z".to_owned()),
        payload: parse(r#"{"conclusion": "success"}"#),
    }
}

fn record(n: u64) -> EvidenceData {
    EvidenceData {
        evidence_id: EvidenceId(uuid(0xe000_0000 + n)),
        case_id: case(),
        kind: "test_result".to_owned(),
        subject_revision: 1,
        producer: "claimed".to_owned(),
        observation_ids: vec![ObservationId(uuid(0xd000_0000 + n))],
        facts: parse(r#"{"tests.pass": true}"#),
        provenance: parse(r#"{"source": "ci"}"#),
    }
}

#[test]
fn the_ports_take_no_answer_and_log_no_call() {
    let governor = FakeGovernor::new();
    governor.script(case(), [Answer::at(1), Answer::at(2), Answer::at(3)]);

    assert_eq!(governor.current_revision(&case()), Ok(1));
    governor
        .observe(Observation::new(observation(1)))
        .unwrap_or_else(|error| panic!("observe: {error:?}"));
    submit_evidence(&governor, "service:ci", record(1))
        .unwrap_or_else(|error| panic!("submit: {error:?}"));
    let mut unknown = record(2);
    unknown.case_id = CaseId("case-unscripted".to_owned());
    assert!(submit_evidence(&governor, "service:ci", unknown).is_err());
    assert_eq!(
        governor.current_revision(&case()),
        Ok(2),
        "an observation and two evidence submissions between two calls took no scripted answer"
    );
    assert_eq!(
        governor.calls(),
        vec![
            GovernorCall::CurrentRevision(case()),
            GovernorCall::CurrentRevision(case()),
        ],
        "the call log holds the two Governor calls and nothing from either port"
    );
    assert_eq!(governor.observations(), vec![observation(1)]);
    assert_eq!(governor.evidence().len(), 1);
}

#[test]
fn concurrent_ports_and_calls_lose_nothing_and_reorder_nothing() {
    let governor = Arc::new(FakeGovernor::new());
    governor.script(case(), [Answer::at(7)]);
    let start = Arc::new(Barrier::new(4));

    let submitter = {
        let (governor, start) = (Arc::clone(&governor), Arc::clone(&start));
        thread::spawn(move || {
            start.wait();
            for n in 0..PER_THREAD {
                submit_evidence(&*governor, "service:ci", record(n))
                    .unwrap_or_else(|error| panic!("submit {n}: {error:?}"));
            }
        })
    };
    let observer = {
        let (governor, start) = (Arc::clone(&governor), Arc::clone(&start));
        thread::spawn(move || {
            start.wait();
            for n in 0..PER_THREAD {
                governor
                    .observe(Observation::new(observation(n)))
                    .unwrap_or_else(|error| panic!("observe {n}: {error:?}"));
            }
        })
    };
    let caller = {
        let (governor, start) = (Arc::clone(&governor), Arc::clone(&start));
        thread::spawn(move || {
            start.wait();
            for _ in 0..PER_THREAD {
                assert_eq!(governor.current_revision(&case()), Ok(7));
            }
        })
    };
    start.wait();
    let mut snapshots = Vec::new();
    for _ in 0..200 {
        snapshots.push((
            governor.evidence().len(),
            governor.observations().len(),
            governor.calls().len(),
        ));
        thread::yield_now();
    }
    for (name, handle) in [
        ("submitter", submitter),
        ("observer", observer),
        ("caller", caller),
    ] {
        handle
            .join()
            .unwrap_or_else(|_| panic!("the {name} thread panicked"));
    }

    let evidence = governor.evidence();
    let observations = governor.observations();
    let calls = governor.calls();
    assert_eq!(
        evidence
            .iter()
            .map(|e| e.evidence_id.clone())
            .collect::<Vec<_>>(),
        (0..PER_THREAD)
            .map(|n| record(n).evidence_id)
            .collect::<Vec<_>>(),
        "every submitted record is recorded once, in submission order"
    );
    assert!(evidence.iter().all(|e| e.producer == "service:ci"));
    assert_eq!(
        observations,
        (0..PER_THREAD).map(observation).collect::<Vec<_>>(),
        "every observation is recorded once, in delivery order"
    );
    assert_eq!(
        calls,
        vec![GovernorCall::CurrentRevision(case()); PER_THREAD as usize],
        "the call log holds exactly the Governor calls"
    );
    for window in snapshots.windows(2) {
        let ((e0, o0, c0), (e1, o1, c1)) = (window[0], window[1]);
        assert!(
            e0 <= e1 && o0 <= o1 && c0 <= c1,
            "a record shrank between reads: {window:?}"
        );
    }
}
