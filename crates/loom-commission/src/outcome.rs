//! Run outcomes and suspension.
//!
//! [`derive`] decides how a run ends, or that it does not end yet, from the governor's
//! determination, the current frontier, the executor's outcome and any authority verdict. Only the
//! governor completes a case: an executor's `CompletedLocalReasoning` never does
//! (`docs/contracts/commission-executor.md`).
//!
//! [`RunStore`] holds runs for the generated behaviours of the `StartRun`, `SuspendRun` and
//! `ResumeRun` commands ([`crate::model::behaviour::Generated`]). Resume continues the same run: its
//! id and case revision are kept, and no new run is created. The store is in memory; keeping a
//! suspended run across a process restart is not done here.

use std::collections::BTreeMap;
use std::fmt;

use crate::admission::admit;
use crate::model::behaviour::{Context, RunStorage};
use crate::model::primitives::Uuid;
use crate::model::responsibility::{
    Admission, AuthorityVerdict, CompletionDetermination, ExecutorOutcome, Frontier, RunId,
    RunOutcome, RunOutcomeCompleted, RunOutcomeNeedsAuthority, RunOutcomeNeedsExternalEvidence,
    RunOutcomeNeedsHumanJudgment, RunOutcomeSuspended, RunSnapshot, Unit, frontier_state,
};

/// What [`derive`] decides for a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Derived {
    /// The run does not end.
    Continue,
    /// The run ends with this outcome.
    Ended(RunOutcome),
}

/// An authority verdict and the capability it was obtained for.
///
/// [`derive`] counts the verdict only when `capability` is the one the frontier names for the
/// proposed action, so a verdict obtained for another capability cannot let the action continue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityVerdict<'a> {
    /// The capability the provider was asked about.
    pub capability: &'a str,
    /// What it answered.
    pub verdict: &'a AuthorityVerdict,
}

/// Derives a run's outcome, in this order:
///
/// 1. the governor holds the case complete: completed, carrying the governor's outcome, whatever
///    the executor returned;
/// 2. the executor returned `Suspended`: suspended, carrying its reason;
/// 3. the executor returned `NeedsHumanJudgment`: needs human judgment, carrying its request;
/// 4. the executor proposed an action the frontier admits: continue;
/// 5. the executor proposed an action the frontier marks as needing a capability, and `authority`
///    is a verdict for exactly that capability: on `ApprovalRequired`, needs authority, carrying
///    its request; on `Allow`, continue. A verdict for any other capability counts as none;
/// 6. otherwise — no useful action, `CompletedLocalReasoning`, or a proposal that is refused,
///    denied or not decided — the frontier decides: continue if it admits some action, needs
///    external evidence carrying its open obligations if it has any, else no admissible action.
///
/// A frontier admits an action when [`admit`] answers `Admissible` for it; an action that needs
/// authority is not admitted without it.
pub fn derive<S: frontier_state::Marker>(
    determination: &CompletionDetermination,
    frontier: &Frontier<S>,
    executor: &ExecutorOutcome,
    authority: Option<CapabilityVerdict<'_>>,
) -> Derived {
    if let CompletionDetermination::Complete(complete) = determination {
        return Derived::Ended(RunOutcome::Completed(RunOutcomeCompleted {
            outcome: complete.outcome.clone(),
        }));
    }
    match executor {
        ExecutorOutcome::Suspended(suspended) => {
            return Derived::Ended(RunOutcome::Suspended(RunOutcomeSuspended {
                reason: suspended.reason.clone(),
            }));
        }
        ExecutorOutcome::NeedsHumanJudgment(needs) => {
            return Derived::Ended(RunOutcome::NeedsHumanJudgment(
                RunOutcomeNeedsHumanJudgment {
                    request: needs.request.clone(),
                },
            ));
        }
        ExecutorOutcome::ProposedAction(proposed) => match admit(frontier, &proposed.action) {
            Admission::Admissible(_) => return Derived::Continue,
            Admission::NeedsAuthority(needs) => match authority
                .filter(|answer| answer.capability == needs.capability)
                .map(|answer| answer.verdict)
            {
                Some(AuthorityVerdict::ApprovalRequired(required)) => {
                    return Derived::Ended(RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
                        request: required.request.clone(),
                    }));
                }
                Some(AuthorityVerdict::Allow(Unit(true))) => return Derived::Continue,
                _ => {}
            },
            Admission::Refused(_) => {}
        },
        ExecutorOutcome::NoUsefulAction(_) | ExecutorOutcome::CompletedLocalReasoning(_) => {}
    }
    from_frontier(frontier)
}

/// Rule 6 of [`derive`]: what the frontier alone decides.
fn from_frontier<S: frontier_state::Marker>(frontier: &Frontier<S>) -> Derived {
    let data = frontier.data();
    if data
        .actions
        .iter()
        .any(|action| matches!(admit(frontier, &action.action), Admission::Admissible(_)))
    {
        return Derived::Continue;
    }
    let requirements: Vec<String> = data
        .obligations
        .iter()
        .filter(|obligation| obligation.open)
        .map(|obligation| obligation.obligation.clone())
        .collect();
    if requirements.is_empty() {
        Derived::Ended(RunOutcome::NoAdmissibleAction(Unit(true)))
    } else {
        Derived::Ended(RunOutcome::NeedsExternalEvidence(
            RunOutcomeNeedsExternalEvidence { requirements },
        ))
    }
}

/// The runs, in memory, with the source of new run ids: the storage and context ports of the
/// generated run commands.
pub struct RunStore {
    runs: BTreeMap<Uuid, RunSnapshot>,
    ids: Box<dyn FnMut() -> RunId + Send>,
}

impl RunStore {
    /// An empty store whose new runs take their ids from `ids`.
    pub fn new(ids: impl FnMut() -> RunId + Send + 'static) -> Self {
        Self {
            runs: BTreeMap::new(),
            ids: Box::new(ids),
        }
    }
}

impl fmt::Debug for RunStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunStore")
            .field("runs", &self.runs)
            .finish_non_exhaustive()
    }
}

impl RunStorage for RunStore {
    fn get(&self, identity: &RunId) -> Option<RunSnapshot> {
        self.runs.get(&identity.0).cloned()
    }

    fn put(&mut self, snapshot: RunSnapshot) {
        self.runs.insert(snapshot.data.run_id.0.clone(), snapshot);
    }

    fn delete(&mut self, identity: &RunId) {
        self.runs.remove(&identity.0);
    }

    fn list(&self) -> Vec<RunSnapshot> {
        self.runs.values().cloned().collect()
    }
}

impl Context for RunStore {
    /// The next id from the store's source.
    ///
    /// # Panics
    ///
    /// If the source returns the id of a stored run: starting a run under it would replace that
    /// run.
    fn generate_commission_responsibility_run_id(&mut self) -> RunId {
        let id = (self.ids)();
        assert!(
            !self.runs.contains_key(&id.0),
            "the run id source repeated the id of a stored run: {}",
            id.0.0
        );
        id
    }
}
