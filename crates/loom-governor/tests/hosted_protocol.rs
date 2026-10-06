//! Hosted protocols use the same canonical governor and fail closed at host boundaries.
use b10x_loom_commission::model::responsibility::{
    ActionStatus, CaseId, CompletionDetermination, EvidenceData, EvidenceId, GovernorError,
    ObservationData, ObservationId,
};
use b10x_loom_commission::model::{json, primitives::Uuid};
use b10x_loom_commission::ports::{evidence::submit_evidence, governor::Governor};
use loom_governor::{CanonGovernor, CaseState, FallibleCaseStore, MemoryCaseStore, OpenError};
use std::collections::BTreeMap;
use std::sync::Mutex;

const PROTOCOL: &str = r#"
format: protocol/1
protocol: {id: hosted.delivery, revision: 1}
artifacts: {implementation: {}}
evidence_kinds:
  verified: {max_age: 60s}
claims:
  verified:
    true_when:
      evidence: {kind: verified, result: pass, subject: implementation}
actions:
  inspect: {effect: read}
  publish:
    effect: write
    precondition: {claim: verified}
    requires: [{capability: repository.publish}]
outcomes:
  accepted:
    requires: {claim: verified}
"#;
fn model() -> b10x_canon::model::Protocol {
    b10x_canon::model::parse(PROTOCOL).unwrap()
}
fn revisions() -> BTreeMap<String, String> {
    [("implementation".into(), "r1".into())].into()
}

#[test]
fn hosted_protocol_projects_and_expires_evidence() {
    let now = Mutex::new(Ok(Some("2026-10-06T10:00:30Z".to_owned())));
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml("hosted-delivery@1", PROTOCOL)
        .unwrap()
        .with_evaluation_time(|_: &CaseId| now.lock().unwrap().clone());
    let case = governor.open("hosted-delivery@1", revisions()).unwrap();
    assert!(matches!(
        governor.completion(&case).unwrap(),
        CompletionDetermination::Open(_)
    ));
    let facts = json::parse(r#"{"format":"canon-evidence/1","id":"verified-1","kind":"verified","subject":"implementation","subject_revision":"r1","observed_at":"2026-10-06T10:00:00Z","result":"pass"}"#).unwrap();
    submit_evidence(
        &governor,
        "trusted:test-runner",
        EvidenceData {
            evidence_id: EvidenceId(Uuid("00000000-0000-0000-0000-000000000001".into())),
            case_id: case.clone(),
            kind: "verified".into(),
            subject_revision: 1,
            producer: String::new(),
            facts,
            provenance: json::Value::Null,
            observation_ids: vec![ObservationId(Uuid(
                "00000000-0000-0000-0000-000000000002".into(),
            ))],
        },
    )
    .unwrap();
    assert!(matches!(
        governor.completion(&case).unwrap(),
        CompletionDetermination::Complete(_)
    ));
    let frontier = governor.frontier(&case).unwrap().into_data();
    let publish = frontier
        .actions
        .iter()
        .find(|a| a.action == "publish")
        .unwrap();
    assert_eq!(publish.status, ActionStatus::ApprovalRequired);
    assert_eq!(publish.capability.as_deref(), Some("repository.publish"));
    *now.lock().unwrap() = Ok(Some("2026-10-06T10:02:00Z".into()));
    assert!(matches!(
        governor.completion(&case).unwrap(),
        CompletionDetermination::Open(_)
    ));
    *now.lock().unwrap() = Err(GovernorError::GovernorUnavailable);
    assert!(matches!(
        governor.frontier(&case),
        Err(GovernorError::GovernorUnavailable)
    ));
}

#[test]
fn registry_cannot_be_replaced_or_weakened() {
    assert!(
        CanonGovernor::new(MemoryCaseStore::default())
            .with_protocol_yaml("broken@1", "not a protocol")
            .is_err()
    );
    assert!(
        CanonGovernor::new(MemoryCaseStore::default())
            .with_protocol_yaml(
                "broken@1",
                &PROTOCOL.replace(
                    "precondition: {claim: verified}",
                    "precondition: {claim: undeclared}"
                )
            )
            .is_err()
    );
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol("hosted-delivery@1", &model())
        .unwrap();
    assert!(
        governor
            .with_protocol("hosted-delivery@1", &model())
            .is_err()
    );
    assert!(
        CanonGovernor::new(MemoryCaseStore::default())
            .with_protocol("software-change@1", &model())
            .is_err()
    );
    let two = b10x_canon::model::parse(&PROTOCOL.replace(
        "[{capability: repository.publish}]",
        "[{capability: repository.publish}, {capability: deployment.publish}]",
    ))
    .unwrap();
    assert!(
        CanonGovernor::new(MemoryCaseStore::default())
            .with_protocol("hosted-delivery@1", &two)
            .is_err()
    );
}

struct FailingStore {
    inserts: std::sync::Arc<Mutex<usize>>,
}
impl FallibleCaseStore for FailingStore {
    type Error = &'static str;
    fn insert(&self, _: CaseState) -> Result<bool, Self::Error> {
        *self.inserts.lock().unwrap() += 1;
        Err("disk full")
    }
    fn get(&self, _: &CaseId) -> Result<Option<CaseState>, Self::Error> {
        Err("unavailable")
    }
    fn update<T>(
        &self,
        _: &CaseId,
        _: impl FnOnce(&mut CaseState) -> T,
    ) -> Result<Option<T>, Self::Error> {
        Err("unavailable")
    }
    fn observe(&self, _: ObservationData) -> Result<(), Self::Error> {
        Err("unavailable")
    }
    fn observations(&self) -> Result<Vec<ObservationData>, Self::Error> {
        Err("unavailable")
    }
}
#[test]
fn storage_failures_are_unavailable_without_retry() {
    let inserts = std::sync::Arc::new(Mutex::new(0));
    let governor = CanonGovernor::new(FailingStore {
        inserts: inserts.clone(),
    })
    .with_protocol("hosted-delivery@1", &model())
    .unwrap();
    assert!(matches!(
        governor.open("hosted-delivery@1", revisions()),
        Err(OpenError::StoreUnavailable { .. })
    ));
    assert_eq!(*inserts.lock().unwrap(), 1);
    assert!(matches!(
        governor.current_revision(&CaseId("case-1".into())),
        Err(GovernorError::GovernorUnavailable)
    ));
    assert!(governor.try_observations().is_err());
}

struct FaultStore {
    memory: MemoryCaseStore,
    fail: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl FallibleCaseStore for FaultStore {
    type Error = &'static str;
    fn insert(&self, state: CaseState) -> Result<bool, Self::Error> {
        self.available()?;
        Ok(loom_governor::CaseStore::insert(&self.memory, state))
    }
    fn get(&self, case: &CaseId) -> Result<Option<CaseState>, Self::Error> {
        self.available()?;
        Ok(loom_governor::CaseStore::get(&self.memory, case))
    }
    fn update<T>(
        &self,
        case: &CaseId,
        change: impl FnOnce(&mut CaseState) -> T,
    ) -> Result<Option<T>, Self::Error> {
        self.available()?;
        Ok(loom_governor::CaseStore::update(&self.memory, case, change))
    }
    fn observe(&self, data: ObservationData) -> Result<(), Self::Error> {
        self.available()?;
        loom_governor::CaseStore::observe(&self.memory, data);
        Ok(())
    }
    fn observations(&self) -> Result<Vec<ObservationData>, Self::Error> {
        self.available()?;
        Ok(loom_governor::CaseStore::observations(&self.memory))
    }
}
impl FaultStore {
    fn available(&self) -> Result<(), &'static str> {
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            Err("disk unavailable")
        } else {
            Ok(())
        }
    }
}
#[test]
fn failed_writes_are_not_acknowledged_and_recovery_preserves_revision() {
    use b10x_loom_commission::model::responsibility::Observation;
    use b10x_loom_commission::ports::evidence::ObservationPort;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let fail = Arc::new(AtomicBool::new(false));
    let governor = CanonGovernor::new(FaultStore {
        memory: MemoryCaseStore::default(),
        fail: fail.clone(),
    })
    .with_protocol("hosted-delivery@1", &model())
    .unwrap();
    let case = governor.open("hosted-delivery@1", revisions()).unwrap();
    fail.store(true, Ordering::SeqCst);
    assert!(matches!(
        governor.update_revision(&case, "implementation", "r2"),
        Err(loom_governor::UpdateError::StoreUnavailable { .. })
    ));
    let observation = Observation::new(ObservationData {
        observation_id: ObservationId(Uuid("00000000-0000-0000-0000-000000000003".into())),
        source: "test".into(),
        subject: "implementation".into(),
        observed_at: b10x_loom_commission::model::primitives::Timestamp(
            "2026-10-06T10:00:00Z".into(),
        ),
        payload: json::Value::Null,
    });
    assert!(matches!(
        governor.observe(observation),
        Err(GovernorError::GovernorUnavailable)
    ));
    fail.store(false, Ordering::SeqCst);
    assert_eq!(governor.revisions(&case).unwrap()["implementation"], "r1");
    assert_eq!(governor.current_revision(&case).unwrap(), 1);
    assert!(governor.try_observations().unwrap().is_empty());
    assert_eq!(
        governor
            .update_revision(&case, "implementation", "r2")
            .unwrap(),
        2
    );
}
