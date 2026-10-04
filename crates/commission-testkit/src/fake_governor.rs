//! A scripted fake `Governor`. Extended by `story:observation-evidence-ports`.
//!
//! Each case the fake holds has a script: a list of [`Answer`]s, one per call. Every port call on
//! the case — current revision, frontier or completion — takes the next answer, and the last one
//! keeps answering once the script runs out. An answer either sets the case's revision,
//! determination and frontier contents for that call or makes the call fail with
//! `GovernorUnavailable`. A case with no script is unknown to the fake.
//!
//! The frontiers it issues carry the case id, the answer's revision and exactly the claims,
//! obligations and actions the answer carries: none, unless the answer was given some with
//! [`Answer::with_items`]. Every `Governor` call the fake receives is logged, failed calls included
//! ([`FakeGovernor::calls`]).
//!
//! The fake also implements the observation and evidence ports. It records what each receives, in
//! the order received and in two separate records: [`FakeGovernor::observations`] and
//! [`FakeGovernor::evidence`]. Neither port takes a scripted answer, and neither record ever
//! receives from the other port.
//!
//! Evidence for a case the fake holds no script for is refused with `UnknownCase`, as its
//! `Governor` calls are, and is not recorded. Evidence naming observation ids the fake never
//! received is still accepted: `GovernorError` has no variant to refuse it with, and adding one is
//! a change to `ess/`. Observations are recorded whatever their subject: an observation carries no
//! case id, and the specification declares no relation from it to a case.
//!
//! All of the fake's state sits behind one lock, and a call logs itself, takes its answer and, for
//! a frontier, its frontier id inside one critical section. Under concurrent calls the k-th logged
//! call is therefore the one that received the k-th answer.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, MutexGuard, PoisonError};

use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    CaseId, CompletionDetermination, CompletionDeterminationComplete, Frontier, FrontierAction,
    FrontierClaim, FrontierData, FrontierId, FrontierObligation, GovernorError, Unit,
    frontier_state,
};
use b10x_commission::model::responsibility::{
    EvidenceData, Observation, ObservationData, observation_state,
};
use b10x_commission::ports::evidence::{AttributedEvidence, EvidencePort, ObservationPort};
use b10x_commission::ports::governor::Governor;

/// What the fake governor answers to one call on a case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    reply: Result<Reply, GovernorError>,
}

/// An answer the governor gives: the case's revision, its determination and the contents of the
/// frontier issued for that revision.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Reply {
    revision: i64,
    determination: CompletionDetermination,
    claims: Vec<FrontierClaim>,
    obligations: Vec<FrontierObligation>,
    actions: Vec<FrontierAction>,
}

impl Answer {
    /// The case is at `revision` and open, and its frontier lists nothing.
    pub fn at(revision: i64) -> Self {
        Self {
            reply: Ok(Reply {
                revision,
                determination: CompletionDetermination::Open(Unit(true)),
                claims: Vec::new(),
                obligations: Vec::new(),
                actions: Vec::new(),
            }),
        }
    }

    /// The governor cannot answer this call.
    pub fn unavailable() -> Self {
        Self {
            reply: Err(GovernorError::GovernorUnavailable),
        }
    }

    /// The same answer, with the case complete with `outcome`. An unavailable answer stays
    /// unavailable.
    pub fn complete(self, outcome: impl Into<String>) -> Self {
        let outcome = outcome.into();
        Self {
            reply: self.reply.map(|reply| Reply {
                determination: CompletionDetermination::Complete(CompletionDeterminationComplete {
                    outcome,
                }),
                ..reply
            }),
        }
    }

    /// The same answer, with a frontier that holds exactly these claims, obligations and actions.
    /// An unavailable answer stays unavailable.
    pub fn with_items(
        self,
        claims: Vec<FrontierClaim>,
        obligations: Vec<FrontierObligation>,
        actions: Vec<FrontierAction>,
    ) -> Self {
        Self {
            reply: self.reply.map(|reply| Reply {
                claims,
                obligations,
                actions,
                ..reply
            }),
        }
    }
}

/// One call the fake governor received: the port method and the case it was asked about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GovernorCall {
    /// [`Governor::current_revision`].
    CurrentRevision(CaseId),
    /// [`Governor::frontier`].
    Frontier(CaseId),
    /// [`Governor::completion`].
    Completion(CaseId),
}

/// A governor whose answers are scripted per case and per call.
#[derive(Debug, Default)]
pub struct FakeGovernor {
    state: Mutex<State>,
}

/// Everything the fake holds, behind its one lock.
#[derive(Debug, Default)]
struct State {
    scripts: BTreeMap<String, VecDeque<Answer>>,
    issued: u64,
    calls: Vec<GovernorCall>,
    observations: Vec<ObservationData>,
    evidence: Vec<EvidenceData>,
}

impl State {
    /// Logs `call`, then answers it on `case`: the next answer in its script, or its last once the
    /// script has run out.
    fn answer(&mut self, call: GovernorCall, case: &CaseId) -> Result<Reply, GovernorError> {
        self.calls.push(call);
        let script = self
            .scripts
            .get_mut(&case.0)
            .ok_or(GovernorError::UnknownCase)?;
        let answer = if script.len() > 1 {
            script.pop_front()
        } else {
            script.front().cloned()
        };
        answer
            .map(|answer| answer.reply)
            .unwrap_or(Err(GovernorError::UnknownCase))
    }

    /// A frontier id no earlier frontier from this fake carries.
    fn next_frontier_id(&mut self) -> FrontierId {
        self.issued += 1;
        FrontierId(Uuid(format!(
            "00000000-0000-4000-8000-{:012x}",
            self.issued
        )))
    }
}

impl FakeGovernor {
    /// A fake that holds no case.
    pub fn new() -> Self {
        Self::default()
    }

    /// Holds `case` with `answers`, one per call, replacing any script it had.
    ///
    /// # Panics
    ///
    /// If `answers` is empty: a held case must have something to answer.
    pub fn script(&self, case: CaseId, answers: impl IntoIterator<Item = Answer>) {
        let answers: VecDeque<Answer> = answers.into_iter().collect();
        assert!(
            !answers.is_empty(),
            "the script for case `{}` has no answer",
            case.0
        );
        self.state().scripts.insert(case.0, answers);
    }

    /// Every observation the observation port received so far, in the order received.
    pub fn observations(&self) -> Vec<ObservationData> {
        self.state().observations.clone()
    }

    /// Every evidence record the evidence port received so far, in the order received.
    pub fn evidence(&self) -> Vec<EvidenceData> {
        self.state().evidence.clone()
    }

    /// Every `Governor` call received so far, in the order received, failed calls included.
    pub fn calls(&self) -> Vec<GovernorCall> {
        self.state().calls.clone()
    }

    /// The fake's state, locked for one call.
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Governor for FakeGovernor {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.state()
            .answer(GovernorCall::CurrentRevision(case.clone()), case)
            .map(|reply| reply.revision)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let (reply, frontier_id) = {
            let mut state = self.state();
            let reply = state.answer(GovernorCall::Frontier(case.clone()), case)?;
            (reply, state.next_frontier_id())
        };
        Ok(Frontier::new(FrontierData {
            frontier_id,
            case_id: case.clone(),
            case_revision: reply.revision,
            claims: reply.claims,
            obligations: reply.obligations,
            actions: reply.actions,
        }))
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.state()
            .answer(GovernorCall::Completion(case.clone()), case)
            .map(|reply| reply.determination)
    }
}

impl ObservationPort for FakeGovernor {
    fn observe(
        &self,
        observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        self.state().observations.push(observation.into_data());
        Ok(())
    }
}

impl EvidencePort for FakeGovernor {
    fn receive(&self, evidence: AttributedEvidence) -> Result<(), GovernorError> {
        let evidence = evidence.into_evidence().into_data();
        let mut state = self.state();
        if !state.scripts.contains_key(&evidence.case_id.0) {
            return Err(GovernorError::UnknownCase);
        }
        state.evidence.push(evidence);
        Ok(())
    }
}
