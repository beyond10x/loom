//! Adversary pass 2 on the governor conformance kit (`kits::governor`).
//!
//! Each case runs the kit against Commission's scripted fake behind a wrapper with exactly one
//! defect the story names as a governor property, and asserts that the kit fails it. The last case
//! runs the kit against a governor with no defect, whose fixture keeps the `advance` contract the
//! kit documents, and asserts that the kit passes it.

use std::sync::{Mutex, PoisonError};

use b10x_commission::action_request::{request, revalidate};
use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionRequestId, ActionStatus, CaseId, CompletionDetermination, EvidenceData, EvidenceId,
    ExecutorOutcomeProposedAction, Frontier, FrontierAction, GovernorError, Observation,
    ObservationData, ProposedActionArguments, RevalidateActionRequestOutcome, RunId,
    frontier_state, observation_state,
};
use b10x_commission::ports::evidence::{
    AttributedEvidence, EvidencePort, ObservationPort, submit_evidence,
};
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_commission_testkit::kits::governor::{self, Check, GovernorFixture};

const ACTION: &str = "kit.act";

fn action(status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: ACTION.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

fn at(revision: i64) -> Answer {
    Answer::at(revision).with_items(
        Vec::new(),
        Vec::new(),
        vec![action(ActionStatus::Admissible, None)],
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Defect {
    /// Observations received are turned into evidence when the governor determines completion.
    PromotesOnCompletion,
    /// Observations received are turned into evidence when the governor issues a frontier.
    PromotesOnFrontier,
    /// The revision is derived from the case's state, which toggles between two values: it reads
    /// 1, 2, 1, 2 as the case changes, so it repeats an earlier revision.
    RevisionRepeats,
    /// `completion` answers `UnknownCase` for every case, the held one included.
    CompletionNeverHeld,
    /// No defect; the fixture's `advance` moves to a revision whose frontier lists the action as
    /// needing a capability.
    None,
}

struct Mutant {
    inner: FakeGovernor,
    defect: Defect,
    promoted: Mutex<usize>,
}

impl Mutant {
    /// Records every observation received and not yet promoted as evidence.
    fn promote(&self) {
        let held = self.inner.observations();
        let mut promoted = self.promoted.lock().unwrap_or_else(PoisonError::into_inner);
        for observation in held.iter().skip(*promoted) {
            let evidence = EvidenceData {
                evidence_id: EvidenceId(Uuid(format!(
                    "00000000-0000-4000-8000-{:012x}",
                    0xa00 + *promoted
                ))),
                case_id: CaseId(observation.subject.clone()),
                kind: "observation".to_owned(),
                subject_revision: 0,
                producer: "governor:mutant".to_owned(),
                observation_ids: vec![observation.observation_id.clone()],
                facts: Value::Null,
                provenance: Value::Null,
            };
            let _ = submit_evidence(&self.inner, "governor:mutant", evidence);
            *promoted += 1;
        }
    }

    /// The revision this governor reports for the fake's revision `inner`.
    fn reported(&self, inner: i64) -> i64 {
        if self.defect == Defect::RevisionRepeats {
            1 + (inner - 1).rem_euclid(2)
        } else {
            inner
        }
    }
}

impl Governor for Mutant {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.inner
            .current_revision(case)
            .map(|revision| self.reported(revision))
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        if self.defect == Defect::PromotesOnFrontier {
            self.promote();
        }
        let mut data = self.inner.frontier(case)?.into_data();
        data.case_revision = self.reported(data.case_revision);
        Ok(Frontier::new(data))
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        match self.defect {
            Defect::PromotesOnCompletion => {
                self.promote();
                self.inner.completion(case)
            }
            Defect::CompletionNeverHeld => Err(GovernorError::UnknownCase),
            _ => self.inner.completion(case),
        }
    }
}

impl ObservationPort for Mutant {
    fn observe(
        &self,
        observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        self.inner.observe(observation)
    }
}

impl EvidencePort for Mutant {
    fn receive(&self, evidence: AttributedEvidence) -> Result<(), GovernorError> {
        self.inner.receive(evidence)
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
            promoted: Mutex::new(0),
        }
    }

    fn advance(&self, governor: &Mutant, case: &CaseId) {
        let now = governor
            .inner
            .current_revision(case)
            .unwrap_or_else(|error| panic!("the fake holds the case it advances: {error:?}"));
        let next = if self.0 == Defect::None {
            Answer::at(now + 1).with_items(
                Vec::new(),
                Vec::new(),
                vec![action(ActionStatus::ApprovalRequired, Some("kit.approve"))],
            )
        } else {
            at(now + 1)
        };
        governor.inner.script(case.clone(), [next]);
    }

    fn observations(&self, governor: &Mutant) -> Vec<ObservationData> {
        governor.inner.observations()
    }

    fn evidence(&self, governor: &Mutant) -> Vec<EvidenceData> {
        governor.inner.evidence()
    }
}

/// The story: the governor "receives an executor's output as an observation, never as evidence".
/// The check reads the evidence after one more `current_revision` call only, so a governor that
/// turns observations into evidence when it determines completion passes every check.
#[test]
fn adversary2_kits_observation_not_evidence_catches_promotion_at_completion() {
    let report = governor::run(&MutantFixture(Defect::PromotesOnCompletion));
    assert!(
        report.failed().contains(&Check::ObservationNotEvidence),
        "a governor that turns observations into evidence when asked for completion passed:\n\
         {report}"
    );
}

/// The same hole, through `frontier`: a governor that interprets observations into evidence when
/// it issues the next frontier passes every check.
#[test]
fn adversary2_kits_observation_not_evidence_catches_promotion_at_frontier() {
    let report = governor::run(&MutantFixture(Defect::PromotesOnFrontier));
    assert!(
        report.failed().contains(&Check::ObservationNotEvidence),
        "a governor that turns observations into evidence when it issues a frontier passed:\n\
         {report}"
    );
}

/// The story: the governor "reports a revision change so that Commission's revalidation refuses a
/// request made at the superseded revision". A governor whose revision repeats (1, 2, 1) lets a
/// request made at revision 1 through after two changes; the kit moves the case once, sees 1 then
/// 2, and passes it.
#[test]
fn adversary2_kits_superseded_revision_catches_a_revision_that_repeats() {
    // The defect is real: after two changes a request made at the first revision is admitted.
    let fixture = MutantFixture(Defect::RevisionRepeats);
    let case = CaseId("adversary2-case".to_owned());
    let mutant = fixture.hold(&case);
    let frontier = mutant
        .frontier(&case)
        .unwrap_or_else(|error| panic!("the mutant holds the case: {error:?}"));
    let made_at_first = request(
        ActionRequestId(Uuid("00000000-0000-4000-8000-0000000000f1".to_owned())),
        RunId(Uuid("00000000-0000-4000-8000-0000000000f2".to_owned())),
        &frontier,
        ExecutorOutcomeProposedAction {
            action: ACTION.to_owned(),
            arguments: ProposedActionArguments(Value::Null),
        },
    );
    fixture.advance(&mutant, &case);
    fixture.advance(&mutant, &case);
    assert_eq!(
        revalidate(&mutant, &made_at_first),
        Ok(RevalidateActionRequestOutcome::Admitted),
        "precondition: the mutant admits a request made two changes ago"
    );

    let report = governor::run(&fixture);
    assert!(
        report.failed().contains(&Check::SupersededRevision),
        "a governor whose revision repeats, admitting a request made two changes ago, passed:\n\
         {report}"
    );
}

/// `GovernorFixture::hold` promises the case is held open, and `Check::UnknownCase` asserts only
/// the error for a case not held. Nothing asks `completion` about the held case, so a governor
/// answering `UnknownCase` to `completion` for every case passes every check.
#[test]
fn adversary2_kits_catch_a_completion_that_never_holds_the_case() {
    let report = governor::run(&MutantFixture(Defect::CompletionNeverHeld));
    assert!(
        !report.failed().is_empty(),
        "a governor whose completion answers UnknownCase for the held case passed:\n{report}"
    );
}

/// `GovernorFixture::advance` asks only that the case move to a later revision. `current-frontier`
/// also requires the frontier after the move to admit an action outright, so a governor with no
/// defect, whose fixture moves the case to a revision where the action needs a capability, fails.
#[test]
fn adversary2_kits_current_frontier_holds_a_fixture_to_the_documented_advance_contract() {
    let report = governor::run(&MutantFixture(Defect::None));
    assert_eq!(
        report.failed(),
        Vec::<Check>::new(),
        "a governor with no defect failed, its fixture keeping the documented advance contract:\n\
         {report}"
    );
}
