//! Run outcomes and suspension.
//!
//! [`derive`] decides how a run ends, or that it does not end yet, from the governor's
//! determination, the current frontier, the executor's outcome and any authority verdict. Only the
//! governor completes a case: an executor's `CompletedLocalReasoning` never does
//! (`docs/contracts/commission-executor.md`).
//!
//! [`RunStore`] holds runs for the `StartRun`, `SuspendRun` and `ResumeRun` commands through
//! [`crate::model::behaviour::Generated`]: it implements the `StartRun` and `SuspendRun` obligations
//! itself, and the generated `ResumeRun` behaviour reads and writes it as storage. Resume continues
//! the same run: its id and case revision are kept, and no new run is created. The store is in
//! memory and never answers `storage-failed`; a durable store implements the two obligations over
//! its own storage and answers `storage-failed` when a write fails. Keeping a suspended run across
//! a process restart is not done here.

use std::collections::BTreeMap;
use std::fmt;

use crate::admission::admit;
use crate::model::behaviour::RunStorage;
use crate::model::obligation::UnmetObligation;
use crate::model::primitives::Uuid;
use crate::model::responsibility::obligations::{StartRunBehavior, SuspendRunBehavior};
use crate::model::responsibility::{
    Admission, AnyRun, AuthorityVerdict, CompletionDetermination, ExecutorOutcome, Frontier, Run,
    RunData, RunId, RunOutcome, RunOutcomeCompleted, RunOutcomeNeedsAuthority,
    RunOutcomeNeedsExternalEvidence, RunOutcomeNeedsHumanJudgment, RunOutcomeSuspended,
    RunSnapshot, RunStarted, RunStateConflict, RunSuspended, StartRun, StartRunOutcome, SuspendRun,
    SuspendRunOutcome, Unit, frontier_state,
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
/// 6. otherwise — no useful action, `CompletedLocalReasoning`, `CaseMoved`, or a proposal that is
///    refused, denied or not decided — the frontier decides: continue if it admits some action,
///    needs external evidence carrying its open obligations if it has any, else no admissible
///    action. For `CaseMoved` the runtime passes the frontier the case was reloaded at, not the
///    one the executor was handed.
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
        ExecutorOutcome::NoUsefulAction(_)
        | ExecutorOutcome::CompletedLocalReasoning(_)
        | ExecutorOutcome::CaseMoved(_) => {}
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

/// The runs, in memory, with the source of new run ids. It implements the `StartRun` and
/// `SuspendRun` obligations itself, and is the storage port of the generated `ResumeRun`
/// behaviour and `RunStates` query.
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

impl RunStore {
    /// The next id from the store's source.
    ///
    /// # Panics
    ///
    /// If the source returns the id of a stored run: starting a run under it would replace that
    /// run.
    fn next_run_id(&mut self) -> RunId {
        let id = (self.ids)();
        assert!(
            !self.runs.contains_key(&id.0),
            "the run id source repeated the id of a stored run: {}",
            id.0.0
        );
        id
    }
}

/// `StartRun` over memory, which always holds the new run: it never answers `storage-failed`.
impl StartRunBehavior for RunStore {
    /// Starts a `Running` run under the next id from the store's source.
    ///
    /// # Panics
    ///
    /// If the source returns the id of a stored run.
    fn start_run(&mut self, input: StartRun) -> Result<StartRunOutcome, UnmetObligation> {
        let run_id = self.next_run_id();
        let data = RunData {
            run_id: run_id.clone(),
            commission_id: input.commission_id.clone(),
            case_revision: input.case_revision,
        };
        RunStorage::put(self, AnyRun::Running(Run::new(data)).snapshot());
        Ok(StartRunOutcome::Started {
            run_started: RunStarted {
                run_id,
                commission_id: input.commission_id,
                case_revision: input.case_revision,
            },
        })
    }
}

/// `SuspendRun` over memory, which always records the suspension: it never answers
/// `storage-failed`.
impl SuspendRunBehavior for RunStore {
    fn suspend_run(&mut self, input: SuspendRun) -> Result<SuspendRunOutcome, UnmetObligation> {
        let Some(held) = RunStorage::get(self, &input.run_id) else {
            return Ok(SuspendRunOutcome::WrongStateUnknownInstance);
        };
        let state = held.state;
        let AnyRun::Running(running) = held.refine() else {
            return Ok(SuspendRunOutcome::WrongState {
                error: RunStateConflict { state },
            });
        };
        RunStorage::put(self, AnyRun::Suspended(running.suspend()).snapshot());
        Ok(SuspendRunOutcome::Suspended {
            run_suspended: RunSuspended {
                run_id: input.run_id,
                reason: input.reason,
            },
        })
    }
}
