//! Acceptance for `story:observation-evidence-ports`: observations and evidence reach the governor
//! through two separate ports in `ports::evidence`, and nothing in Commission turns one into the
//! other.
//!
//! - The observation port takes a raw report — here an executor's output — and delivers it as an
//!   `Observation`. It never becomes evidence (Atlas ADR 0074).
//! - The evidence port takes typed, attributable evidence. Its producer is the one the trusted
//!   caller supplies beside the payload, never one the payload claims.
//! - An `EvidenceAdapter` interprets observations into evidence; each record it makes names the
//!   observations it read, and the evidence port refuses a record that names none.
//!
//! It runs against the fake governor, which implements both ports and records what each receives,
//! and the scripted fake executor.

use b10x_commission::model::json::{self, Value};
use b10x_commission::model::primitives::{Timestamp, Uuid};
use b10x_commission::model::responsibility::{
    AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData, CommissionId,
    EvidenceData, EvidenceId, ExecutorOutcome, ExecutorOutcomeProposedAction, GovernorError,
    Observation, ObservationData, ObservationId, PrincipalId, ProposedActionArguments,
    commission_state,
};
use b10x_commission::ports::evidence::{
    EvidenceAdapter, EvidenceError, ObservationPort, submit_evidence,
};
use b10x_commission::ports::executor::AgentExecutor;
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};

const CASE: &str = "case-observed";
const REVISION: i64 = 3;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn parse(text: &str) -> Value {
    json::parse(text).unwrap_or_else(|error| panic!("parse {text}: {error}"))
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// An observation of `subject` carrying `payload`, as some integration reported it.
fn observation(n: u64, source: &str, payload: Value) -> ObservationData {
    ObservationData {
        observation_id: ObservationId(uuid(0x100 + n)),
        source: source.to_owned(),
        subject: format!("{CASE}@{REVISION}"),
        observed_at: Timestamp(format!("2026-10-04T00:00:0{n}Z")),
        payload,
    }
}

/// An evidence record for the case at `REVISION`, naming `observations`.
fn evidence(n: u64, producer: &str, observations: Vec<ObservationId>) -> EvidenceData {
    EvidenceData {
        evidence_id: EvidenceId(uuid(0x200 + n)),
        case_id: CaseId(CASE.to_owned()),
        kind: "test_result".to_owned(),
        subject_revision: REVISION,
        producer: producer.to_owned(),
        observation_ids: observations,
        facts: parse(r#"{"tests.pass": true}"#),
        provenance: parse(r#"{"source": "ci"}"#),
    }
}

/// A test-local adapter: every observation whose payload reports `"conclusion": "success"` is one
/// fact, and together they make one `test_result` record naming each observation it read.
struct ConclusionAdapter;

impl EvidenceAdapter for ConclusionAdapter {
    fn interpret(&self, observations: &[ObservationData]) -> Vec<EvidenceData> {
        let read: Vec<ObservationId> = observations
            .iter()
            .filter(|observation| {
                observation.payload.member("conclusion") == Some(&Value::Text("success".to_owned()))
            })
            .map(|observation| observation.observation_id.clone())
            .collect();
        if read.is_empty() {
            return Vec::new();
        }
        vec![evidence(3, "adapter:conclusion", read)]
    }
}

#[test]
fn observation_and_evidence_stay_apart() {
    let governor = FakeGovernor::new();
    governor.script(CaseId(CASE.to_owned()), [Answer::at(REVISION)]);
    let frontier = governor
        .frontier(&CaseId(CASE.to_owned()))
        .unwrap_or_else(|error| panic!("the scripted case issues a frontier: {error:?}"));

    // 1. An executor output that says tests passed reaches the governor as one observation, and
    //    as no evidence.
    let said = parse(r#"{"tests": "passed"}"#);
    let executor = ScriptedExecutor::new([ExecutorOutcome::ProposedAction(
        ExecutorOutcomeProposedAction {
            action: "report_result".to_owned(),
            arguments: ProposedActionArguments(said.clone()),
        },
    )]);
    let ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        arguments: ProposedActionArguments(output),
        ..
    }) = executor.run(&commission(), &frontier)
    else {
        panic!("the scripted executor returns its one ProposedAction");
    };
    let reported = observation(1, "executor:scripted", output);
    governor
        .observe(Observation::new(reported.clone()))
        .unwrap_or_else(|error| panic!("the observation port takes the output: {error:?}"));
    assert_eq!(
        governor.observations(),
        vec![reported.clone()],
        "the observation port delivers exactly the one observation it was given"
    );
    assert_eq!(
        governor.observations()[0].payload,
        said,
        "the executor's output is the observation's payload, unchanged"
    );
    assert_eq!(
        governor.evidence(),
        Vec::<EvidenceData>::new(),
        "an executor output that says tests passed is an observation, never evidence"
    );

    // 2. The payload claims producer P2; the trusted caller supplies P1. It arrives carrying P1.
    let mut claimed = evidence(2, "P2", vec![reported.observation_id.clone()]);
    claimed.facts = parse(r#"{"tests.pass": true, "producer": "P2"}"#);
    claimed.provenance = parse(r#"{"source": "ci", "producer": "P2"}"#);
    submit_evidence(&governor, "P1", claimed.clone()).unwrap_or_else(|error| {
        panic!("the evidence port takes a record naming an observation: {error:?}")
    });
    let arrived = governor.evidence();
    assert_eq!(arrived.len(), 1, "one submitted record arrives once");
    assert_eq!(
        arrived[0].producer, "P1",
        "the producer is the trusted caller's, never the payload's"
    );
    assert_eq!(
        arrived[0],
        EvidenceData {
            producer: "P1".to_owned(),
            ..claimed
        },
        "apart from its producer the record arrives as submitted"
    );

    // 3. An adapter interprets two recorded observations into one record that names both.
    let ci = observation(2, "connector:ci", parse(r#"{"conclusion": "success"}"#));
    let rerun = observation(3, "connector:ci", parse(r#"{"conclusion": "success"}"#));
    for reported in [&ci, &rerun] {
        governor
            .observe(Observation::new(reported.clone()))
            .unwrap_or_else(|error| panic!("the observation port takes a report: {error:?}"));
    }
    let recorded: Vec<ObservationData> = governor
        .observations()
        .into_iter()
        .filter(|observation| observation.source == "connector:ci")
        .collect();
    assert_eq!(recorded, vec![ci.clone(), rerun.clone()]);
    let interpreted = ConclusionAdapter.interpret(&recorded);
    assert_eq!(interpreted.len(), 1, "two observations, one record");
    for record in interpreted {
        submit_evidence(&governor, "service:ci", record)
            .unwrap_or_else(|error| panic!("the adapter's record is accepted: {error:?}"));
    }
    let arrived = governor.evidence();
    assert_eq!(arrived.len(), 2);
    assert_eq!(
        arrived[1].observation_ids,
        vec![ci.observation_id.clone(), rerun.observation_id.clone()],
        "the record names both observations it interprets"
    );
    assert_eq!(arrived[1].producer, "service:ci");

    // 4. A record that names no observation is refused with the port's typed error, and the
    //    governor records nothing for it.
    let observations_before = governor.observations();
    let evidence_before = governor.evidence();
    assert_eq!(
        submit_evidence(&governor, "P1", evidence(4, "P1", Vec::new())),
        Err(EvidenceError::NoObservation),
        "evidence names one or more observations"
    );
    assert_eq!(
        governor.evidence(),
        evidence_before,
        "nothing recorded for the refusal"
    );
    assert_eq!(governor.observations(), observations_before);
}

/// The fake governor refuses evidence for a case it holds no script for, as it refuses its
/// `Governor` calls, and `submit_evidence` hands the refusal back.
#[test]
fn evidence_for_a_case_the_governor_does_not_hold_is_refused() {
    let governor = FakeGovernor::new();
    governor.script(CaseId("case-other".to_owned()), [Answer::at(REVISION)]);
    let reported = observation(1, "connector:ci", parse(r#"{"conclusion": "success"}"#));
    assert_eq!(
        submit_evidence(
            &governor,
            "service:ci",
            evidence(5, "service:ci", vec![reported.observation_id])
        ),
        Err(EvidenceError::Governor(GovernorError::UnknownCase)),
        "the fake does not hold `{CASE}`"
    );
    assert_eq!(
        governor.evidence(),
        Vec::<EvidenceData>::new(),
        "nothing recorded for an unknown case"
    );
}

/// A trusted producer that is empty or only whitespace names nobody: `submit_evidence` refuses it by
/// name, even on a case the governor holds, and nothing reaches the governor.
#[test]
fn evidence_attributed_to_nobody_is_refused_as_no_producer() {
    let governor = FakeGovernor::new();
    governor.script(CaseId(CASE.to_owned()), [Answer::at(REVISION)]);
    let reported = observation(1, "connector:ci", parse(r#"{"conclusion": "success"}"#));
    for trusted in ["", " ", "\t\n", "\u{3000}"] {
        assert_eq!(
            submit_evidence(
                &governor,
                trusted,
                evidence(6, "service:ci", vec![reported.observation_id.clone()])
            ),
            Err(EvidenceError::NoProducer),
            "the trusted producer {trusted:?} names nobody"
        );
    }
    assert_eq!(
        governor.evidence(),
        Vec::<EvidenceData>::new(),
        "nothing recorded for a record attributed to nobody"
    );
    assert_eq!(
        submit_evidence(
            &governor,
            "service:ci",
            evidence(6, "service:ci", vec![reported.observation_id])
        ),
        Ok(()),
        "the same record with a producer is accepted on this case"
    );
}

/// The first and last character of each Unicode 16.0 `Cf` range (read from the Unicode Character
/// Database, 170 characters in 21 ranges).
const FORMAT_RANGE_ENDS: [char; 42] = [
    '\u{00AD}',
    '\u{00AD}',
    '\u{0600}',
    '\u{0605}',
    '\u{061C}',
    '\u{061C}',
    '\u{06DD}',
    '\u{06DD}',
    '\u{070F}',
    '\u{070F}',
    '\u{0890}',
    '\u{0891}',
    '\u{08E2}',
    '\u{08E2}',
    '\u{180E}',
    '\u{180E}',
    '\u{200B}',
    '\u{200F}',
    '\u{202A}',
    '\u{202E}',
    '\u{2060}',
    '\u{2064}',
    '\u{2066}',
    '\u{206F}',
    '\u{FEFF}',
    '\u{FEFF}',
    '\u{FFF9}',
    '\u{FFFB}',
    '\u{110BD}',
    '\u{110BD}',
    '\u{110CD}',
    '\u{110CD}',
    '\u{13430}',
    '\u{1343F}',
    '\u{1BCA0}',
    '\u{1BCA3}',
    '\u{1D173}',
    '\u{1D17A}',
    '\u{E0001}',
    '\u{E0001}',
    '\u{E0020}',
    '\u{E007F}',
];

/// The producer rule, case by case: non-empty, no leading or trailing whitespace, no control or
/// format character anywhere. A refused producer is never trimmed into an accepted one, and an
/// accepted producer arrives exactly as named.
#[test]
fn the_trusted_producer_arrives_exactly_as_named_or_is_refused() {
    let governor = FakeGovernor::new();
    governor.script(CaseId(CASE.to_owned()), [Answer::at(REVISION)]);
    let reported = observation(1, "connector:ci", parse(r#"{"conclusion": "success"}"#));
    let submit = |trusted: &str| {
        submit_evidence(
            &governor,
            trusted,
            evidence(7, "claimed", vec![reported.observation_id.clone()]),
        )
    };

    let mut refused: Vec<String> = ["P1\n", " P1", "P1 ", "\tP1\r\n", "P1\u{3000}", "P\u{0}1"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    refused.extend(FORMAT_RANGE_ENDS.iter().map(|c| format!("P{c}1")));
    for trusted in &refused {
        assert_eq!(
            submit(trusted),
            Err(EvidenceError::NoProducer),
            "the trusted producer {trusted:?} is padded or carries a control or format character"
        );
    }
    assert_eq!(
        governor.evidence(),
        Vec::<EvidenceData>::new(),
        "no refused producer reached the governor, trimmed or not"
    );

    let accepted = [
        "P1",
        "service:ci",
        "P 1",
        "P\u{00AC}1",
        "P\u{00AE}1",
        "P\u{0606}1",
        "P\u{2010}1",
        "P\u{1D172}1",
    ];
    for trusted in accepted {
        assert_eq!(submit(trusted), Ok(()), "{trusted:?} names someone");
    }
    let arrived: Vec<String> = governor
        .evidence()
        .into_iter()
        .map(|evidence| evidence.producer)
        .collect();
    assert_eq!(arrived, accepted, "each producer arrives exactly as named");
}
