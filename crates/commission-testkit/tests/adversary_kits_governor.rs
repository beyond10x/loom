//! Adversary cases for the governor conformance kit (`kits::governor`).
//!
//! Each case runs the kit against a broken governor: Commission's scripted fake behind a wrapper
//! with exactly one defect. The proof cases show a check catches a plain mutant; the hole cases are
//! defects the kit's own documentation says it catches, and does not.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, PoisonError};

use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionStatus, CaseId, CompletionDetermination, EvidenceData, EvidenceId, Frontier,
    FrontierAction, FrontierData, GovernorError, Observation, ObservationData, frontier_state,
    observation_state,
};
use b10x_commission::ports::evidence::{
    AttributedEvidence, EvidencePort, ObservationPort, submit_evidence,
};
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_commission_testkit::kits::governor::{self, Check, GovernorFixture};

fn at(revision: i64) -> Answer {
    Answer::at(revision).with_items(
        Vec::new(),
        Vec::new(),
        vec![FrontierAction {
            action: "kit.act".to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    )
}

/// The one defect a [`Mutant`] carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Defect {
    /// Every frontier is issued one revision ahead of the case's current revision.
    FrontierAhead,
    /// `completion` answers an unknown case with `GovernorUnavailable`.
    CompletionUnavailableForUnknown,
    /// Every observation is also recorded as evidence, at once.
    ObservationBecomesEvidence,
    /// Evidence arrives without the last observation id it named.
    DropsLastObservationId,
    /// `current_revision` follows the case; `frontier` keeps issuing the first frontier it issued.
    FrontierStuck,
    /// Observations received are turned into evidence at the next `Governor` call.
    LazyPromotion,
    /// From the second observation on, every observation received so far becomes evidence.
    SecondObservationPromotes,
    /// Evidence arrives naming every observation the governor holds, not the ids it was given.
    AttributesAllObservations,
    /// `completion` panics on a case the governor does not hold, as `.expect()` adapter code does.
    PanicsOnUnknown,
}

struct Mutant {
    inner: FakeGovernor,
    defect: Defect,
    stuck: Mutex<Option<FrontierData>>,
    pending: Mutex<Vec<ObservationData>>,
}

/// Evidence built from `observations`, as a governor that promotes observations would build it.
fn promoted(n: usize, observations: &[ObservationData]) -> EvidenceData {
    EvidenceData {
        evidence_id: EvidenceId(Uuid(format!("00000000-0000-4000-8000-{:012x}", 0xe00 + n))),
        case_id: CaseId(observations[0].subject.clone()),
        kind: "observation".to_owned(),
        subject_revision: 0,
        producer: "governor:mutant".to_owned(),
        observation_ids: observations
            .iter()
            .map(|observation| observation.observation_id.clone())
            .collect(),
        facts: Value::Null,
        provenance: Value::Null,
    }
}

impl Mutant {
    fn promote_pending(&self) {
        let pending: Vec<ObservationData> = self
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
            .collect();
        for (n, observation) in pending.iter().enumerate() {
            let _ = submit_evidence(
                &self.inner,
                "governor:mutant",
                promoted(n, std::slice::from_ref(observation)),
            );
        }
    }

    fn before_call(&self) {
        if self.defect == Defect::LazyPromotion {
            self.promote_pending();
        }
    }
}

impl Governor for Mutant {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.before_call();
        self.inner.current_revision(case)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        self.before_call();
        match self.defect {
            Defect::FrontierAhead => {
                let mut data = self.inner.frontier(case)?.into_data();
                data.case_revision += 1;
                Ok(Frontier::new(data))
            }
            Defect::FrontierStuck => {
                let mut stuck = self.stuck.lock().unwrap_or_else(PoisonError::into_inner);
                if let Some(data) = stuck.as_ref() {
                    return Ok(Frontier::new(data.clone()));
                }
                let data = self.inner.frontier(case)?.into_data();
                *stuck = Some(data.clone());
                Ok(Frontier::new(data))
            }
            _ => self.inner.frontier(case),
        }
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.before_call();
        match (self.defect, self.inner.completion(case)) {
            (Defect::CompletionUnavailableForUnknown, Err(GovernorError::UnknownCase)) => {
                Err(GovernorError::GovernorUnavailable)
            }
            (Defect::PanicsOnUnknown, Err(GovernorError::UnknownCase)) => {
                panic!("case `{}` is not held", case.0)
            }
            (_, answer) => answer,
        }
    }
}

impl ObservationPort for Mutant {
    fn observe(
        &self,
        observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        let data = observation.data().clone();
        self.inner.observe(observation)?;
        match self.defect {
            Defect::ObservationBecomesEvidence => {
                let _ = submit_evidence(
                    &self.inner,
                    "governor:mutant",
                    promoted(0, std::slice::from_ref(&data)),
                );
            }
            Defect::LazyPromotion => self
                .pending
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(data),
            Defect::SecondObservationPromotes => {
                let held = self.inner.observations();
                if held.len() >= 2 {
                    let _ = submit_evidence(&self.inner, "governor:mutant", promoted(1, &held));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl EvidencePort for Mutant {
    fn receive(&self, evidence: AttributedEvidence) -> Result<(), GovernorError> {
        let mut data = evidence.evidence().data().clone();
        match self.defect {
            Defect::DropsLastObservationId => {
                data.observation_ids.pop();
            }
            Defect::AttributesAllObservations => {
                data.observation_ids = self
                    .inner
                    .observations()
                    .into_iter()
                    .map(|observation| observation.observation_id)
                    .collect();
            }
            _ => return self.inner.receive(evidence),
        }
        let producer = data.producer.clone();
        submit_evidence(&self.inner, &producer, data).map_err(|_| GovernorError::UnknownCase)
    }
}

struct MutantFixture(Defect);

impl GovernorFixture for MutantFixture {
    type Governor = Mutant;

    fn hold(&self, case: &CaseId) -> Mutant {
        let inner = FakeGovernor::new();
        inner.script(case.clone(), [at(1)]);
        Mutant {
            inner,
            defect: self.0,
            stuck: Mutex::new(None),
            pending: Mutex::new(Vec::new()),
        }
    }

    fn advance(&self, governor: &Mutant, case: &CaseId) {
        let now = governor
            .inner
            .current_revision(case)
            .unwrap_or_else(|error| panic!("the fake holds the case it advances: {error:?}"));
        governor.inner.script(case.clone(), [at(now + 1)]);
    }

    fn observations(&self, governor: &Mutant) -> Vec<ObservationData> {
        governor.inner.observations()
    }

    fn evidence(&self, governor: &Mutant) -> Vec<EvidenceData> {
        governor.inner.evidence()
    }
}

// Proof cases: each check catches a plain mutant.

#[test]
fn adversary_kits_current_frontier_catches_a_frontier_ahead_of_the_revision() {
    let report = governor::run(&MutantFixture(Defect::FrontierAhead));
    assert!(
        report.failed().contains(&Check::CurrentFrontier),
        "{report}"
    );
}

#[test]
fn adversary_kits_unknown_case_catches_unavailable_for_an_unknown_case() {
    let report = governor::run(&MutantFixture(Defect::CompletionUnavailableForUnknown));
    assert_eq!(report.failed(), vec![Check::UnknownCase], "{report}");
}

#[test]
fn adversary_kits_observation_not_evidence_catches_eager_promotion() {
    let report = governor::run(&MutantFixture(Defect::ObservationBecomesEvidence));
    assert!(
        report.failed().contains(&Check::ObservationNotEvidence),
        "{report}"
    );
}

#[test]
fn adversary_kits_evidence_observation_ids_catches_a_dropped_id() {
    let report = governor::run(&MutantFixture(Defect::DropsLastObservationId));
    assert_eq!(
        report.failed(),
        vec![Check::EvidenceObservationIds],
        "{report}"
    );
}

// Hole cases.

/// `Check::CurrentFrontier` says the frontier is issued for the held case at its current revision.
/// It reads the frontier only before the case ever moves, and `SupersededRevision` never reads the
/// frontier after it moves, so a governor whose frontier stays at revision N while it reports N+1
/// passes every check.
#[test]
fn adversary_kits_current_frontier_catches_a_frontier_stuck_after_the_case_moves() {
    let report = governor::run(&MutantFixture(Defect::FrontierStuck));
    assert!(
        report.failed().contains(&Check::CurrentFrontier),
        "a governor that keeps issuing the frontier for revision N after reporting N+1 passed: \
         {report}"
    );
}

/// `Check::ObservationNotEvidence` reads the evidence record immediately after `observe` and before
/// any other call, so a governor that promotes observations to evidence on its next call passes.
#[test]
fn adversary_kits_observation_not_evidence_catches_promotion_on_the_next_call() {
    let report = governor::run(&MutantFixture(Defect::LazyPromotion));
    assert!(
        report.failed().contains(&Check::ObservationNotEvidence),
        "a governor that turns observations into evidence on its next call passed: {report}"
    );
}

/// `Check::ObservationNotEvidence` observes a single output; a governor that promotes once a second
/// arrives is reported under `evidence-observation-ids`, which names the wrong defect.
#[test]
fn adversary_kits_observation_not_evidence_names_promotion_from_the_second_observation() {
    let report = governor::run(&MutantFixture(Defect::SecondObservationPromotes));
    assert!(
        report.failed().contains(&Check::ObservationNotEvidence),
        "a governor that promotes the second observation to evidence is not named as \
         observation-not-evidence: {report}"
    );
}

/// `Check::EvidenceObservationIds` submits evidence naming exactly every observation the governor
/// received, so a governor that overwrites the ids with every observation it holds passes.
#[test]
fn adversary_kits_evidence_observation_ids_catches_ids_replaced_by_all_held() {
    let report = governor::run(&MutantFixture(Defect::AttributesAllObservations));
    assert!(
        report.failed().contains(&Check::EvidenceObservationIds),
        "a governor that replaces the submitted observation ids with every observation it holds \
         passed: {report}"
    );
}

/// The kit documents that it reports each check it ran, naming each one that failed. An adapter
/// that panics on an unknown case makes `run` panic, so no report and no check name come back.
#[test]
fn adversary_kits_governor_reports_a_panicking_check_as_failed() {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        governor::run(&MutantFixture(Defect::PanicsOnUnknown))
    }));
    match outcome {
        Ok(report) => assert_eq!(report.failed(), vec![Check::UnknownCase], "{report}"),
        Err(_) => panic!(
            "governor::run panicked on a governor that panics on an unknown case; \
             no report named unknown-case"
        ),
    }
}
