//! Acceptance for `story:adapter-conformance-suites`: the governor and authority conformance kits,
//! run from a crate other than `b10x-commission` the way an adapter crate runs them.
//!
//! Each kit takes a fixture the adapter writes and runs every one of its checks, reporting each
//! check it ran and naming each one that failed. Here the fixtures are:
//!
//! 1. Commission's scripted fake governor, which the governor kit passes;
//! 2. Commission's static fake authority provider, which the authority kit passes;
//! 3. a broken governor: it wraps the fake and, once the fake has moved the case from revision N to
//!    N+1, keeps answering revision N and the frontier for N. The governor kit fails it, on its
//!    superseded-revision check and on no other;
//! 4. a broken provider: when its backing call fails it allows. The authority kit fails it, on its
//!    backing-failure check and on no other.
//!
//! The broken fixtures are mutants of the fakes, one defect each, so the kits are shown to fail as
//! well as to pass.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use b10x_commission::model::responsibility::{
    ActionStatus, AuthorityVerdict, CaseId, CommissionData, CompletionDetermination, EvidenceData,
    Frontier, FrontierAction, FrontierData, GovernorError, Observation, ObservationData, Unit,
    frontier_state, observation_state,
};
use b10x_commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use b10x_commission::ports::evidence::{AttributedEvidence, EvidencePort, ObservationPort};
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_commission_testkit::kits::authority::{self, AuthorityFixture, Backing};
use b10x_commission_testkit::kits::governor::{self, GovernorFixture};

/// The one action the fake governor's frontier admits, at every revision.
const ACTION: &str = "kit.act";

/// The fake governor's answer for a case at `revision`: open, with a frontier admitting [`ACTION`].
fn at(revision: i64) -> Answer {
    Answer::at(revision).with_items(
        Vec::new(),
        Vec::new(),
        vec![FrontierAction {
            action: ACTION.to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    )
}

/// The governor fixture over Commission's scripted fake.
struct FakeGovernorFixture;

impl GovernorFixture for FakeGovernorFixture {
    type Governor = FakeGovernor;

    fn hold(&self, case: &CaseId) -> FakeGovernor {
        let governor = FakeGovernor::new();
        governor.script(case.clone(), [at(1)]);
        governor
    }

    fn advance(&self, governor: &FakeGovernor, case: &CaseId) {
        let now = governor
            .current_revision(case)
            .unwrap_or_else(|error| panic!("the fake holds the case it advances: {error:?}"));
        governor.script(case.clone(), [at(now + 1)]);
    }

    fn observations(&self, governor: &FakeGovernor) -> Vec<ObservationData> {
        governor.observations()
    }

    fn evidence(&self, governor: &FakeGovernor) -> Vec<EvidenceData> {
        governor.evidence()
    }
}

/// A broken governor: it answers each case's revision and frontier from the first frontier the
/// wrapped fake issued for it, however far the fake has moved the case since.
struct PinnedGovernor {
    inner: FakeGovernor,
    pinned: Mutex<BTreeMap<String, FrontierData>>,
}

impl PinnedGovernor {
    /// The first frontier the fake issued for `case`, asked for now if there is none yet.
    fn pinned(&self, case: &CaseId) -> Result<FrontierData, GovernorError> {
        let mut pinned = self.pinned.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(frontier) = pinned.get(&case.0) {
            return Ok(frontier.clone());
        }
        let frontier = self.inner.frontier(case)?.data().clone();
        pinned.insert(case.0.clone(), frontier.clone());
        Ok(frontier)
    }
}

impl Governor for PinnedGovernor {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.pinned(case).map(|frontier| frontier.case_revision)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        self.pinned(case).map(Frontier::new)
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.inner.completion(case)
    }
}

impl ObservationPort for PinnedGovernor {
    fn observe(
        &self,
        observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        self.inner.observe(observation)
    }
}

impl EvidencePort for PinnedGovernor {
    fn receive(&self, evidence: AttributedEvidence) -> Result<(), GovernorError> {
        self.inner.receive(evidence)
    }
}

/// The governor fixture over the broken governor: the fake fixture's script, behind the pin.
struct PinnedGovernorFixture;

impl GovernorFixture for PinnedGovernorFixture {
    type Governor = PinnedGovernor;

    fn hold(&self, case: &CaseId) -> PinnedGovernor {
        PinnedGovernor {
            inner: FakeGovernorFixture.hold(case),
            pinned: Mutex::new(BTreeMap::new()),
        }
    }

    fn advance(&self, governor: &PinnedGovernor, case: &CaseId) {
        FakeGovernorFixture.advance(&governor.inner, case);
    }

    fn observations(&self, governor: &PinnedGovernor) -> Vec<ObservationData> {
        governor.inner.observations()
    }

    fn evidence(&self, governor: &PinnedGovernor) -> Vec<EvidenceData> {
        governor.inner.evidence()
    }
}

/// The authority fixture over Commission's static fake: the backing's table becomes the fake's.
struct FakeAuthorityFixture;

// Adversary pass 2, finding 6: the fake must see the live backing, so its table is rebuilt at each decision.
struct LiveStaticAuthorityProvider {
    backing: Backing,
}

impl AuthorityProvider for LiveStaticAuthorityProvider {
    fn decide(
        &self,
        commission: &CommissionData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        self.backing
            .entries()
            .into_iter()
            .fold(
                StaticAuthorityProvider::new(),
                |provider, (capability, answer)| match answer {
                    Ok(verdict) => provider.answer(capability, verdict),
                    Err(error) => provider.fail(capability, error.message),
                },
            )
            .decide(commission, capability)
    }
}

impl AuthorityFixture for FakeAuthorityFixture {
    type Provider = LiveStaticAuthorityProvider;

    fn provider(&self, backing: Backing) -> LiveStaticAuthorityProvider {
        LiveStaticAuthorityProvider { backing }
    }
}

/// A broken provider: it asks its backing service, and allows when that call fails.
struct AllowsOnFailure {
    backing: Backing,
}

impl AuthorityProvider for AllowsOnFailure {
    fn decide(
        &self,
        _commission: &CommissionData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        Ok(self
            .backing
            .call(capability)
            .unwrap_or(AuthorityVerdict::Allow(Unit(true))))
    }
}

/// The authority fixture over the broken provider.
struct AllowsOnFailureFixture;

impl AuthorityFixture for AllowsOnFailureFixture {
    type Provider = AllowsOnFailure;

    fn provider(&self, backing: Backing) -> AllowsOnFailure {
        AllowsOnFailure { backing }
    }
}

#[test]
fn kits_hold_fakes_and_catch_broken_ones() {
    // 1. The governor kit passes against the fake governor, every check run.
    let report = governor::run(&FakeGovernorFixture);
    assert_eq!(
        report.ran(),
        governor::Check::ALL.to_vec(),
        "the governor kit runs every check: {report}"
    );
    assert_eq!(
        report.failed(),
        Vec::<governor::Check>::new(),
        "the governor kit passes against the fake governor: {report}"
    );

    // 2. The authority kit passes against the fake provider, every check run.
    let report = authority::run(&FakeAuthorityFixture);
    assert_eq!(
        report.ran(),
        authority::Check::ALL.to_vec(),
        "the authority kit runs every check: {report}"
    );
    assert_eq!(
        report.failed(),
        Vec::<authority::Check>::new(),
        "the authority kit passes against the fake provider: {report}"
    );

    // 3. The governor kit fails the governor that keeps answering the superseded revision, on its
    //    superseded-revision check alone, and names it.
    let report = governor::run(&PinnedGovernorFixture);
    assert_eq!(
        report.ran(),
        governor::Check::ALL.to_vec(),
        "the governor kit runs every check: {report}"
    );
    assert_eq!(
        report.failed(),
        vec![governor::Check::SupersededRevision],
        "the governor kit fails a governor pinned to revision N on superseded-revision alone: \
         {report}"
    );
    assert_eq!(
        governor::Check::SupersededRevision.name(),
        "superseded-revision"
    );
    assert!(
        report.to_string().contains("superseded-revision"),
        "the report names the failing check: {report}"
    );

    // 4. The authority kit fails the provider that allows when its backing call fails, on its
    //    backing-failure check alone, and names it.
    let report = authority::run(&AllowsOnFailureFixture);
    assert_eq!(
        report.ran(),
        authority::Check::ALL.to_vec(),
        "the authority kit runs every check: {report}"
    );
    assert_eq!(
        report.failed(),
        vec![authority::Check::BackingFailure],
        "the authority kit fails a provider that allows on backing failure, on backing-failure \
         alone: {report}"
    );
    assert_eq!(authority::Check::BackingFailure.name(), "backing-failure");
    assert!(
        report.to_string().contains("backing-failure"),
        "the report names the failing check: {report}"
    );
}
