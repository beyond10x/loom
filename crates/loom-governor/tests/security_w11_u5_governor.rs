//! Conformance checks for `story:regression-evidence-integrity` (wave 2026-10-14-w2, U5), the
//! governor half: a record's `observed_at` is taken only when it is the instant of every
//! observation the record cites, each received by this governor; otherwise the record is refused
//! and nothing of it is kept. The acceptance statement also refuses a record that cites an
//! observation the governor never received, whatever its facts carry.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, GovernorError, Observation, ObservationData, ObservationId,
};
use b10x_loom_commission::ports::evidence::{EvidenceError, ObservationPort, submit_evidence};
use loom_governor::{
    CanonGovernor, CaseInputs, EvidenceRefusal, FallibleCaseStore, FileCaseStore, MemoryCaseStore,
};

const PROTOCOL: &str = "software-change@1";
const PRODUCER: &str = "service:ci";
const CASE: &str = "chg-security-w11-u5";
const OBSERVED: &str = "2026-10-14T09:00:05Z";

fn open<S: FallibleCaseStore>(governor: &CanonGovernor<S>) -> CaseId {
    let case = CaseId(CASE.to_owned());
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
    case
}

fn memory() -> (CanonGovernor<MemoryCaseStore>, CaseId) {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = open(&governor);
    (governor, case)
}

fn observe<S: FallibleCaseStore>(governor: &CanonGovernor<S>, id: &str, at: &str) {
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

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

/// A `test_result` record's facts, with `observed_at` members as given (none, one or several).
fn facts(evidence: &str, observed_at: &[Value]) -> Value {
    let mut members = vec![
        ("format".to_owned(), text("canon-evidence/1")),
        ("id".to_owned(), text(&format!("test-result-{evidence}"))),
        ("kind".to_owned(), text("test_result")),
        ("subject".to_owned(), text("implementation")),
        ("subject_revision".to_owned(), text("r1")),
        ("result".to_owned(), text("fail")),
    ];
    for at in observed_at {
        members.push(("observed_at".to_owned(), at.clone()));
    }
    Value::Object(members)
}

fn record(evidence: &str, observations: &[&str], observed_at: &[Value]) -> EvidenceData {
    EvidenceData {
        evidence_id: EvidenceId(Uuid(evidence.to_owned())),
        case_id: CaseId(CASE.to_owned()),
        kind: "test_result".to_owned(),
        subject_revision: 1,
        producer: PRODUCER.to_owned(),
        observation_ids: observations
            .iter()
            .map(|id| ObservationId(Uuid((*id).to_owned())))
            .collect(),
        facts: facts(evidence, observed_at),
        provenance: Value::Null,
    }
}

fn refused<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &CaseId, data: EvidenceData) {
    assert_eq!(
        submit_evidence(governor, PRODUCER, data),
        Err(EvidenceError::Governor(GovernorError::GovernorUnavailable)),
        "the governor does not take the record"
    );
    assert!(
        governor.evidence(case).expect("held").is_empty(),
        "nothing of the refused record is kept"
    );
}

/// Acceptance, second clause, read as written: a record citing an observation the governor never
/// received is refused, naming the record. A record whose facts carry no `observed_at` cites one
/// all the same, and a Canon record applies without an instant.
#[test]
fn security_a_record_without_observed_at_citing_an_unreceived_observation_is_refused() {
    let (governor, case) = memory();
    observe(&governor, "obs-1", OBSERVED);
    let data = record("ev-no-instant", &["obs-never"], &[]);
    assert_eq!(
        governor.check_evidence(&data),
        Err(EvidenceRefusal::ObservationNotReceived {
            evidence: "ev-no-instant".to_owned(),
            observation: "obs-never".to_owned(),
        }),
        "a record citing an observation the governor never received is refused, naming it"
    );
    refused(&governor, &case, data);
}

/// One mismatch among several cited observations refuses the whole record.
#[test]
fn security_one_mismatching_observation_among_several_refuses_the_record() {
    let (governor, case) = memory();
    observe(&governor, "obs-a", OBSERVED);
    observe(&governor, "obs-b", OBSERVED);
    observe(&governor, "obs-c", "2026-10-14T09:00:06Z");
    let data = record("ev-many", &["obs-a", "obs-b", "obs-c"], &[text(OBSERVED)]);
    assert!(matches!(
        governor.check_evidence(&data),
        Err(EvidenceRefusal::ObservedAtDiffers { ref observation, .. }) if observation == "obs-c"
    ));
    refused(&governor, &case, data);

    let unseen = record("ev-many-unseen", &["obs-a", "obs-never"], &[text(OBSERVED)]);
    assert!(matches!(
        governor.check_evidence(&unseen),
        Err(EvidenceRefusal::ObservationNotReceived { ref observation, .. })
            if observation == "obs-never"
    ));
    refused(&governor, &case, unseen);
}

/// The same instant written another way is not the observation's text, and is refused: the
/// governor reads no clock and takes the host's text only as given.
#[test]
fn security_another_spelling_of_the_same_instant_is_refused() {
    for (n, spelling) in [
        "2026-10-14T09:00:05+00:00",
        "2026-10-14T09:00:05.000Z",
        "2026-10-14T09:00:05z",
        "2026-10-14T11:00:05+02:00",
        " 2026-10-14T09:00:05Z",
        "2026-10-14T09:00:05Z ",
        "2026-10-14 09:00:05Z",
    ]
    .into_iter()
    .enumerate()
    {
        let (governor, case) = memory();
        observe(&governor, "obs-1", OBSERVED);
        let data = record(&format!("ev-spelling-{n}"), &["obs-1"], &[text(spelling)]);
        assert!(
            matches!(
                governor.check_evidence(&data),
                Err(EvidenceRefusal::ObservedAtDiffers { .. })
            ),
            "`{spelling}` is not the observation's text"
        );
        refused(&governor, &case, data);
    }
}

/// An `observed_at` that is no text is refused, however its observation reads.
#[test]
fn security_an_observed_at_that_is_not_text_is_refused() {
    for (n, value) in [
        Value::Null,
        Value::Number("1791000000".to_owned()),
        Value::Array(vec![text(OBSERVED)]),
        Value::Object(vec![("at".to_owned(), text(OBSERVED))]),
    ]
    .into_iter()
    .enumerate()
    {
        let (governor, case) = memory();
        observe(&governor, "obs-1", OBSERVED);
        let data = record(&format!("ev-not-text-{n}"), &["obs-1"], &[value]);
        assert!(governor.check_evidence(&data).is_err(), "{data:?}");
        refused(&governor, &case, data);
    }
}

/// A record whose facts name `observed_at` twice, once with the observation's instant and once
/// with another, carries an instant other than the observation's: it is refused and not kept.
#[test]
fn security_a_second_observed_at_member_with_another_instant_is_refused() {
    let (governor, case) = memory();
    observe(&governor, "obs-1", OBSERVED);
    let data = record(
        "ev-twice",
        &["obs-1"],
        &[text(OBSERVED), text("2026-10-14T08:00:00Z")],
    );
    refused(&governor, &case, data);
}

/// An observation delivered after the record does not rescue it: the record was refused and
/// nothing of it is kept; the same record submitted again, after the observation, is taken.
#[test]
fn security_an_observation_delivered_after_the_record_does_not_rescue_it() {
    let (governor, case) = memory();
    let data = record("ev-early", &["obs-late"], &[text(OBSERVED)]);
    refused(&governor, &case, data.clone());
    observe(&governor, "obs-late", OBSERVED);
    assert!(
        governor.evidence(&case).expect("held").is_empty(),
        "the refused record does not reappear when its observation arrives"
    );
    submit_evidence(&governor, PRODUCER, data).expect("submitted again, it is taken");
    assert_eq!(governor.evidence(&case).expect("held").len(), 1);
}

fn store_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "security-w11-u5-{label}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create the store directory");
    dir
}

/// Observations kept by a file-backed governor are the ones a governor over the same directory
/// checks after a restart: a matching record is taken and a mismatching one refused.
#[test]
fn security_the_check_reads_the_observations_a_restarted_governor_holds() {
    let dir = store_dir("restart");
    {
        let governor = CanonGovernor::new(FileCaseStore::open(&dir).expect("the store opens"));
        let _ = open(&governor);
        observe(&governor, "obs-kept", OBSERVED);
    }
    let governor = CanonGovernor::new(FileCaseStore::open(&dir).expect("the store reopens"));
    let case = CaseId(CASE.to_owned());
    refused(
        &governor,
        &case,
        record(
            "ev-restart-off",
            &["obs-kept"],
            &[text("2026-10-14T09:00:04Z")],
        ),
    );
    refused(
        &governor,
        &case,
        record("ev-restart-unseen", &["obs-other"], &[text(OBSERVED)]),
    );
    submit_evidence(
        &governor,
        PRODUCER,
        record("ev-restart-on", &["obs-kept"], &[text(OBSERVED)]),
    )
    .expect("a record at the kept observation's instant is taken after a restart");
    assert_eq!(governor.evidence(&case).expect("held").len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}
