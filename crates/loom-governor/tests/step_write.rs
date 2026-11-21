//! `CanonGovernor::record_step` (`story:bound-artifact-revisions`): a step's revision moves and its
//! records are written to the case in one store write, so a crash leaves the case either as it was
//! before the step or complete after it, and an artifact the step does not name keeps its
//! revision.
//!
//! The crash is injected at each step of the `FileCaseStore` write (`FileCaseStore::update_until`),
//! and the case is read back by a fresh store over the same directory, as after a restart. Scratch
//! goes under `CARGO_TARGET_TMPDIR`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ops::ControlFlow;
use std::path::PathBuf;

use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, GovernorError, Observation, ObservationData, ObservationId,
};
use b10x_loom_commission::ports::evidence::{
    AttributedEvidence, EvidencePort, ObservationPort as _, submit_evidence,
};
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::{
    CanonGovernor, CaseInputs, CaseState, FallibleCaseStore, FileCaseStore, FileStoreError,
    MemoryCaseStore, StepError, WriteStep,
};

const REGISTRATION: &str = "fixture@1";
const CASE: &str = "case-1";
const OBSERVED_AT: &str = "2026-11-21T12:00:00Z";

const PROTOCOL: &str = r"format: protocol/1
protocol:
  id: fixture
  revision: 1
artifacts:
  spec: {}
  impl: {}
evidence_kinds:
  gate: {}
actions:
  spec.check:
    effect: write
    may_produce:
      - evidence: gate
";

fn governor_over<S: FallibleCaseStore>(store: S) -> CanonGovernor<S> {
    let governor = CanonGovernor::new(store)
        .with_protocol_yaml(REGISTRATION, PROTOCOL)
        .expect("the fixture protocol registers");
    governor
        .open_case(
            case(),
            REGISTRATION,
            [
                ("spec".to_owned(), "s1".to_owned()),
                ("impl".to_owned(), "i1".to_owned()),
            ]
            .into(),
            CaseInputs::none(),
        )
        .expect("the case opens");
    governor
}

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// The evidence port a record passes through to be attributed by `submit_evidence`.
#[derive(Default)]
struct Held(RefCell<Vec<AttributedEvidence>>);

impl EvidencePort for Held {
    fn receive(&self, evidence: AttributedEvidence) -> Result<(), GovernorError> {
        self.0.borrow_mut().push(evidence);
        Ok(())
    }
}

/// Keeps the observation the record cites, and answers a `gate` record about `spec` at
/// `spec_revision` following `impl` at `impl_revision`, for the case at `case_revision`.
fn record<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    n: u64,
    case_revision: i64,
    spec_revision: &str,
    impl_revision: &str,
) -> AttributedEvidence {
    let observation = ObservationId(uuid(1000 + n));
    governor
        .observe(Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: "test".to_owned(),
            subject: "spec.check".to_owned(),
            observed_at: Timestamp(OBSERVED_AT.to_owned()),
            payload: json::Value::Null,
        }))
        .expect("the observation is kept");
    let facts = format!(
        r#"{{"format": "canon-evidence/1", "id": "gate-{n}", "kind": "gate", "subject": "spec", "subject_revision": "{spec_revision}", "observed_at": "{OBSERVED_AT}", "result": "valid", "upstream_revisions": {{"impl": "{impl_revision}"}}}}"#
    );
    let held = Held::default();
    submit_evidence(
        &held,
        "ess",
        EvidenceData {
            evidence_id: EvidenceId(uuid(n)),
            case_id: case(),
            kind: "gate".to_owned(),
            subject_revision: case_revision,
            producer: "ess".to_owned(),
            observation_ids: vec![observation],
            facts: json::parse(&facts).expect("the facts parse"),
            provenance: json::Value::Null,
        },
    )
    .expect("the record is attributed");
    held.0.into_inner().pop().unwrap()
}

fn moves(spec: &str) -> BTreeMap<String, String> {
    [("spec".to_owned(), spec.to_owned())].into()
}

fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("step-write")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_step_moves_its_revisions_and_keeps_its_records_in_one_write() {
    let governor = governor_over(MemoryCaseStore::default());
    let gate = record(&governor, 1, 2, "t0123456789abcdef", "i1");
    let written = governor
        .record_step(&case(), &moves("t0123456789abcdef"), vec![gate])
        .expect("the step is written");
    assert_eq!(written.case_revision, 2);
    assert_eq!(written.moves.len(), 1);
    assert_eq!(
        (
            written.moves[0].artifact.as_str(),
            written.moves[0].from.as_str(),
            written.moves[0].to.as_str()
        ),
        ("spec", "s1", "t0123456789abcdef")
    );
    assert_eq!(written.applies, vec![true]);
    assert_eq!(governor.current_revision(&case()).unwrap(), 2);
    assert_eq!(governor.evidence(&case()).unwrap().len(), 1);
}

#[test]
fn an_artifact_the_step_does_not_name_keeps_its_revision() {
    let governor = governor_over(MemoryCaseStore::default());
    governor
        .record_step(&case(), &moves("t0123456789abcdef"), Vec::new())
        .expect("the step is written");
    let revisions = governor.revisions(&case()).unwrap();
    assert_eq!(revisions["impl"], "i1");
    assert_eq!(revisions["spec"], "t0123456789abcdef");
}

#[test]
fn a_step_that_moves_nothing_and_records_nothing_leaves_the_case_revision() {
    let governor = governor_over(MemoryCaseStore::default());
    let written = governor
        .record_step(&case(), &moves("s1"), Vec::new())
        .expect("the step is written");
    assert!(written.moves.is_empty());
    assert_eq!(governor.current_revision(&case()).unwrap(), 1);
}

#[test]
fn a_record_not_about_the_moved_revisions_writes_nothing() {
    let governor = governor_over(MemoryCaseStore::default());
    // About the revision the step moves away from.
    let stale_subject = record(&governor, 1, 2, "s1", "i1");
    // For the case revision before the move.
    let stale_case = record(&governor, 2, 1, "t0123456789abcdef", "i1");
    // Following an upstream revision the case does not hold.
    let stale_upstream = record(&governor, 3, 2, "t0123456789abcdef", "i0");
    for stale in [stale_subject, stale_case, stale_upstream] {
        match governor.record_step(&case(), &moves("t0123456789abcdef"), vec![stale]) {
            Err(StepError::StaleRecord { .. }) => {}
            other => panic!("not refused as stale: {other:?}"),
        }
        assert_eq!(governor.revisions(&case()).unwrap()["spec"], "s1");
        assert_eq!(governor.current_revision(&case()).unwrap(), 1);
        assert!(governor.evidence(&case()).unwrap().is_empty());
    }
}

#[test]
fn an_undeclared_artifact_or_a_revision_that_is_no_identifier_writes_nothing() {
    let governor = governor_over(MemoryCaseStore::default());
    let mut undeclared = moves("t0123456789abcdef");
    undeclared.insert("other".to_owned(), "t1".to_owned());
    assert!(matches!(
        governor.record_step(&case(), &undeclared, Vec::new()),
        Err(StepError::UndeclaredArtifact { .. })
    ));
    assert!(matches!(
        governor.record_step(&case(), &moves("two words"), Vec::new()),
        Err(StepError::InvalidRevision { .. })
    ));
    assert_eq!(governor.revisions(&case()).unwrap()["spec"], "s1");
    assert_eq!(governor.current_revision(&case()).unwrap(), 1);
}

/// A `FileCaseStore` whose update ends at `at`, as a process killed there would end it.
struct CrashAt {
    store: FileCaseStore,
    at: WriteStep,
}

impl FallibleCaseStore for CrashAt {
    type Error = FileStoreError;

    fn insert(&self, state: CaseState) -> Result<bool, Self::Error> {
        self.store.insert(state)
    }

    fn get(&self, case: &CaseId) -> Result<Option<CaseState>, Self::Error> {
        self.store.get(case)
    }

    fn update<T>(
        &self,
        case: &CaseId,
        change: impl FnOnce(&mut CaseState) -> T,
    ) -> Result<Option<T>, Self::Error> {
        let at = self.at;
        let stop = &mut |step: WriteStep| {
            if step == at {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        };
        self.store
            .update_until(case, change, stop)?
            .ok_or_else(|| FileStoreError {
                path: self.store.dir().to_owned(),
                problem: format!("crashed at {at:?}"),
            })
    }

    fn observe(&self, observation: ObservationData) -> Result<(), Self::Error> {
        self.store.observe(observation)
    }

    fn observations(&self) -> Result<Vec<ObservationData>, Self::Error> {
        self.store.observations()
    }
}

#[test]
fn a_crash_at_the_store_write_of_a_step_leaves_neither_or_both() {
    for at in WriteStep::ALL {
        let dir = store_dir(&format!("{at:?}"));
        let governor = governor_over(CrashAt {
            store: FileCaseStore::open(&dir).unwrap(),
            at,
        });
        let gate = record(&governor, 1, 2, "t0123456789abcdef", "i1");
        let crashed = governor.record_step(&case(), &moves("t0123456789abcdef"), vec![gate]);
        assert!(
            matches!(crashed, Err(StepError::StoreUnavailable { .. })),
            "{at:?}: {crashed:?}"
        );

        // A restart: a fresh store over the directory, which removes what the crash left.
        let reopened = CanonGovernor::new(FileCaseStore::open(&dir).unwrap())
            .with_protocol_yaml(REGISTRATION, PROTOCOL)
            .unwrap();
        let revisions = reopened.revisions(&case()).expect("the case file opens");
        let evidence = reopened.evidence(&case()).unwrap();
        let revision = reopened.current_revision(&case()).unwrap();
        if at.renamed() {
            assert_eq!(revisions["spec"], "t0123456789abcdef", "{at:?}");
            assert_eq!((revision, evidence.len()), (2, 1), "{at:?}");
        } else {
            assert_eq!(revisions["spec"], "s1", "{at:?}");
            assert_eq!((revision, evidence.len()), (1, 0), "{at:?}");
        }
        assert_eq!(revisions["impl"], "i1", "{at:?}");
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.starts_with(".tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "{at:?}: {leftovers:?}");
    }
}

#[test]
fn a_step_written_through_a_file_store_reads_back_after_a_restart() {
    let dir = store_dir("whole");
    let governor = governor_over(FileCaseStore::open(&dir).unwrap());
    let gate = record(&governor, 1, 2, "t0123456789abcdef", "i1");
    governor
        .record_step(&case(), &moves("t0123456789abcdef"), vec![gate])
        .expect("the step is written");
    let reopened = CanonGovernor::new(FileCaseStore::open(&dir).unwrap())
        .with_protocol_yaml(REGISTRATION, PROTOCOL)
        .unwrap();
    assert_eq!(
        reopened.revisions(&case()).unwrap()["spec"],
        "t0123456789abcdef"
    );
    assert_eq!(reopened.evidence(&case()).unwrap().len(), 1);
}
