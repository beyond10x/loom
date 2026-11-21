//! The suspension port: where a commission's Suspension is kept, so a run suspended on an approval
//! outlives the process that suspended it (`story:approval-suspend-resume-slice`,
//! `decision-blocker:suspension-durable-record`, option B).
//!
//! Commission defines the port; the host implements it over durable storage, as it implements the
//! governor's case store. A commission holds at most one Suspension: the run it suspended, the
//! action request pending the approval and the reason. The record lives and dies with the
//! commission, never with the case: it is not kept in the governor's record of the case, nor by the
//! authority provider.
//!
//! The port answers the three commands the specification declares on a Suspension, each decided
//! and enacted as one atomic step over what the store holds:
//!
//! * `HoldSuspension` stores it `Pending`; a commission already holding one is refused
//!   (`already-held`) and keeps it. A host that already settled the request the Suspension would
//!   hold answers `settled`, with its reason
//!   ([`SuspensionSettled`](crate::model::responsibility::SuspensionSettled)), and stores
//!   nothing; the runtime then ends the run with nothing held. [`decide_hold`] never answers it:
//!   only the host knows what it settled, so a host wraps its store to answer it first.
//! * `TakeSuspension` moves it `Pending` → `Resuming` before a resume performs anything. Of two
//!   resumes, one takes it; the other finds it `Resuming` (`wrong-state`) or gone (`not-found`).
//! * `ReleaseSuspension` removes a `Resuming` Suspension once its request was performed; a
//!   `Pending` one is refused (`not-taken`).
//!
//! A Suspension held for a decision request (`SuspensionReason::Decision`,
//! `story:decision-suspension-reason`) holds the Run and no action request: its request, expected
//! case revision, action and arguments are absent, and a take and a release of it name no request.
//! They name its decision request instead: [`decide_take`] and [`decide_release`] refuse one that
//! names another, or none, and one that names a decision request on a Suspension held for an action
//! request (`other-decision`, [`SuspensionDecisionMismatch`] naming both), and keep it, so a resume
//! that read the hold for one decision never takes or releases a later hold for another.
//! [`decide_hold`] refuses a hold whose reason and those four members disagree (`Decision` with any
//! of them, any other reason without all of them) as `storage-failed`, and stores nothing.
//!
//! A take and a release name the action request the resume read and revalidated. A Suspension
//! held for another request is refused (`other-request`, [`SuspensionRequestMismatch`] naming
//! both) whatever its state, and kept as it is, so a resume that read one Suspension never takes
//! or releases a later one the commission holds for another request.
//!
//! A `Resuming` Suspension is never taken again: its request may have been performed or not, and
//! settling it is the host's. Each command answers `storage-failed` when the store cannot read or
//! write, and then changes nothing. [`decide_hold`], [`decide_take`] and [`decide_release`] decide
//! each command over what is held, so every store answers the same outcomes; a store enacts the
//! [`SuspensionWrite`] the decision names while holding whatever keeps a second store from
//! interleaving. [`SuspensionStore`] is the port over memory.
//!
//! The runtime writes through it only when it is given one
//! ([`crate::runtime::run_with_suspension`]) and takes and releases the record when it resumes
//! ([`crate::runtime::resume_after_grant`]). A caller of [`crate::runtime::run_until_blocked`]
//! passes no port, and its runs are never suspended on an approval.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::model::behaviour::SuspensionStorage;
use crate::model::obligation::UnmetObligation;
use crate::model::primitives::Uuid;
use crate::model::responsibility::obligations::{
    HoldSuspensionBehavior, ReleaseSuspensionBehavior, TakeSuspensionBehavior,
};
use crate::model::responsibility::{
    ActionRequestId, AnySuspension, CommissionId, HoldSuspension, HoldSuspensionOutcome,
    ReleaseSuspension, ReleaseSuspensionOutcome, Suspension, SuspensionAlreadyHeld, SuspensionData,
    SuspensionDecisionMismatch, SuspensionHeld, SuspensionKey, SuspensionNotFound,
    SuspensionReason, SuspensionReleased, SuspensionRequestMismatch, SuspensionSnapshot,
    SuspensionStateConflict, SuspensionStorageFailed, SuspensionTaken, TakeSuspension,
    TakeSuspensionOutcome,
};

/// Keeps the Suspension of each commission, answering the commands the specification declares on
/// it, each atomically.
pub trait SuspensionPort {
    /// `HoldSuspension`: stores the Suspension `Pending`, or refuses when its commission already
    /// holds one, keeping that one. A host that already settled the request answers `settled` and
    /// stores nothing.
    fn hold(&self, input: HoldSuspension) -> HoldSuspensionOutcome;

    /// `TakeSuspension`: moves the commission's Suspension from `Pending` to `Resuming`, or refuses
    /// when none is stored, it is held for another request than the input's, or it is already
    /// `Resuming`.
    fn take(&self, input: TakeSuspension) -> TakeSuspensionOutcome;

    /// `ReleaseSuspension`: removes the commission's `Resuming` Suspension, or refuses when none
    /// is stored, it is held for another request than the input's, or it is `Pending`.
    fn release(&self, input: ReleaseSuspension) -> ReleaseSuspensionOutcome;

    /// The Suspension of `commission`, in either state, or `None` when none is stored.
    ///
    /// # Errors
    ///
    /// [`SuspensionStoreError`] when the store cannot be read, or what it holds is not a
    /// Suspension of `commission`.
    fn get(
        &self,
        commission: &CommissionId,
    ) -> Result<Option<SuspensionSnapshot>, SuspensionStoreError>;
}

/// Why a suspension store could not read or write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspensionStoreError {
    /// What the store reported.
    pub message: String,
}

impl SuspensionStoreError {
    /// A failure carrying the store's own message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// The `storage-failed` refusal a command answers with this failure.
    pub fn refusal(&self) -> SuspensionStorageFailed {
        SuspensionStorageFailed {
            reason: self.to_string(),
        }
    }
}

impl fmt::Display for SuspensionStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "suspension store failed: {}", self.message)
    }
}

impl std::error::Error for SuspensionStoreError {}

/// What a decided command does to the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuspensionWrite {
    /// Store this Suspension under its identity, replacing what was held.
    Put(Box<SuspensionSnapshot>),
    /// Remove the Suspension under this identity.
    Delete(SuspensionKey),
}

/// `HoldSuspension` over `held`, the Suspension stored under the input's identity: `already-held`
/// when there is one, `storage-failed` when the input's reason and its four request members
/// disagree, otherwise `held`, with the `Pending` Suspension to store.
pub fn decide_hold(
    held: Option<&SuspensionSnapshot>,
    input: HoldSuspension,
) -> (HoldSuspensionOutcome, Option<SuspensionWrite>) {
    if held.is_some() {
        return (
            HoldSuspensionOutcome::AlreadyHeld {
                error: SuspensionAlreadyHeld {
                    suspension: input.suspension,
                },
            },
            None,
        );
    }
    // A Suspension held for a decision holds no action request, and every other holds one whole
    // (`story:decision-suspension-reason`): ess/22 cannot tie the members to the reason, so the
    // port refuses the combination it forbids before any store keeps it.
    let decision = matches!(input.reason, SuspensionReason::Decision(_));
    let members = [
        input.action_request_id.is_some(),
        input.expected_case_revision.is_some(),
        input.action.is_some(),
        input.arguments.is_some(),
    ];
    if members.contains(&decision) {
        return (
            HoldSuspensionOutcome::StorageFailed {
                error: SuspensionStorageFailed {
                    reason: if decision {
                        "a Suspension held for a decision holds no action request".to_owned()
                    } else {
                        "a Suspension not held for a decision holds its whole action request"
                            .to_owned()
                    },
                },
            },
            None,
        );
    }
    let event = SuspensionHeld {
        suspension: input.suspension.clone(),
        run_id: input.run_id.clone(),
        action_request_id: input.action_request_id.clone(),
    };
    let pending = Suspension::new(SuspensionData {
        suspension: input.suspension,
        commission_id: input.commission_id,
        run_id: input.run_id,
        run_case_revision: input.run_case_revision,
        action_request_id: input.action_request_id,
        case_id: input.case_id,
        expected_case_revision: input.expected_case_revision,
        action: input.action,
        arguments: input.arguments,
        reason: input.reason,
    });
    (
        HoldSuspensionOutcome::Held {
            suspension_held: event,
        },
        Some(SuspensionWrite::Put(Box::new(
            AnySuspension::Pending(pending).snapshot(),
        ))),
    )
}

/// The refusal `other-request` of a take or a release naming `requested` when `held` is held for
/// another request; `None` when it is held for `requested`. A Decision Suspension holds no request
/// and is named by none: an absent request matches only an absent one
/// (`story:decision-suspension-reason`).
fn other_request(
    held: &SuspensionSnapshot,
    suspension: &SuspensionKey,
    requested: &Option<ActionRequestId>,
) -> Option<SuspensionRequestMismatch> {
    (held.data.action_request_id != *requested).then(|| SuspensionRequestMismatch {
        suspension: suspension.clone(),
        held_action_request_id: held.data.action_request_id.clone(),
        action_request_id: requested.clone(),
    })
}

/// The refusal `other-decision` of a take or a release naming the decision request `named` when
/// `held` is held for another decision request, or for none (an action request); `None` when
/// both name the same one or neither names one (`story:decision-suspension-reason`).
fn other_decision(
    held: &SuspensionSnapshot,
    suspension: &SuspensionKey,
    named: &Option<String>,
) -> Option<SuspensionDecisionMismatch> {
    let held_decision = match &held.data.reason {
        SuspensionReason::Decision(hold) => Some(&hold.request_id),
        _ => None,
    };
    (held_decision != named.as_ref()).then(|| SuspensionDecisionMismatch {
        suspension: suspension.clone(),
        held_decision_request_id: held_decision.cloned(),
        decision_request_id: named.clone(),
    })
}

/// `TakeSuspension` over `held`: `not-found` when nothing is stored, `other-request` naming both
/// requests when it is held for another request than the input's, `wrong-state` naming the
/// request when it is already `Resuming`, `other-decision` naming both decision requests when it
/// is held for another decision request than the input's, otherwise `taken`, with the `Resuming`
/// Suspension to store.
pub fn decide_take(
    held: Option<SuspensionSnapshot>,
    input: TakeSuspension,
) -> (TakeSuspensionOutcome, Option<SuspensionWrite>) {
    let Some(held) = held else {
        return (
            TakeSuspensionOutcome::NotFound {
                error: SuspensionNotFound {
                    suspension: input.suspension,
                },
            },
            None,
        );
    };
    if let Some(error) = other_request(&held, &input.suspension, &input.action_request_id) {
        return (TakeSuspensionOutcome::OtherRequest { error }, None);
    }
    let decision = other_decision(&held, &input.suspension, &input.decision_request_id);
    match (held.refine(), decision) {
        (AnySuspension::Pending(_), Some(error)) => {
            (TakeSuspensionOutcome::OtherDecision { error }, None)
        }
        (AnySuspension::Pending(pending), None) => {
            let taken = pending.take();
            let event = SuspensionTaken {
                suspension: input.suspension,
                action_request_id: taken.data().action_request_id.clone(),
            };
            (
                TakeSuspensionOutcome::Taken {
                    suspension_taken: event,
                },
                Some(SuspensionWrite::Put(Box::new(
                    AnySuspension::Resuming(taken).snapshot(),
                ))),
            )
        }
        (resuming @ AnySuspension::Resuming(_), _) => {
            let state = resuming.state();
            let held = resuming.snapshot();
            (
                TakeSuspensionOutcome::WrongState {
                    error: SuspensionStateConflict {
                        state,
                        action_request_id: held.data.action_request_id,
                    },
                },
                None,
            )
        }
    }
}

/// `ReleaseSuspension` over `held`: `not-found` when nothing is stored, `other-request` naming
/// both requests when it is held for another request than the input's, `not-taken` naming the
/// request when it is `Pending`, `other-decision` naming both decision requests when it is held
/// for another decision request than the input's, otherwise `released`, removing it.
pub fn decide_release(
    held: Option<SuspensionSnapshot>,
    input: ReleaseSuspension,
) -> (ReleaseSuspensionOutcome, Option<SuspensionWrite>) {
    let Some(held) = held else {
        return (
            ReleaseSuspensionOutcome::NotFound {
                error: SuspensionNotFound {
                    suspension: input.suspension,
                },
            },
            None,
        );
    };
    if let Some(error) = other_request(&held, &input.suspension, &input.action_request_id) {
        return (ReleaseSuspensionOutcome::OtherRequest { error }, None);
    }
    let decision = other_decision(&held, &input.suspension, &input.decision_request_id);
    match (held.refine(), decision) {
        (pending @ AnySuspension::Pending(_), _) => {
            let state = pending.state();
            let held = pending.snapshot();
            (
                ReleaseSuspensionOutcome::NotTaken {
                    error: SuspensionStateConflict {
                        state,
                        action_request_id: held.data.action_request_id,
                    },
                },
                None,
            )
        }
        (AnySuspension::Resuming(_), Some(error)) => {
            (ReleaseSuspensionOutcome::OtherDecision { error }, None)
        }
        (AnySuspension::Resuming(_), None) => (
            ReleaseSuspensionOutcome::Released {
                suspension_released: SuspensionReleased {
                    suspension: input.suspension.clone(),
                },
            },
            Some(SuspensionWrite::Delete(input.suspension)),
        ),
    }
}

/// The suspension port over memory: one Suspension per commission id, every command answered by
/// the decisions above under one lock. It answers `storage-failed` only for a hold
/// [`decide_hold`] refuses.
#[derive(Debug, Default)]
pub struct SuspensionStore {
    held: Mutex<BTreeMap<Uuid, SuspensionSnapshot>>,
}

impl SuspensionStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<Uuid, SuspensionSnapshot>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn enact(held: &mut BTreeMap<Uuid, SuspensionSnapshot>, write: Option<SuspensionWrite>) {
        match write {
            Some(SuspensionWrite::Put(snapshot)) => {
                held.insert(snapshot.data.suspension.commission_id.0.clone(), *snapshot);
            }
            Some(SuspensionWrite::Delete(key)) => {
                held.remove(&key.commission_id.0);
            }
            None => {}
        }
    }
}

impl SuspensionPort for SuspensionStore {
    fn hold(&self, input: HoldSuspension) -> HoldSuspensionOutcome {
        let mut held = self.lock();
        let (outcome, write) = decide_hold(held.get(&input.suspension.commission_id.0), input);
        Self::enact(&mut held, write);
        outcome
    }

    fn take(&self, input: TakeSuspension) -> TakeSuspensionOutcome {
        let mut held = self.lock();
        let current = held.get(&input.suspension.commission_id.0).cloned();
        let (outcome, write) = decide_take(current, input);
        Self::enact(&mut held, write);
        outcome
    }

    fn release(&self, input: ReleaseSuspension) -> ReleaseSuspensionOutcome {
        let mut held = self.lock();
        let current = held.get(&input.suspension.commission_id.0).cloned();
        let (outcome, write) = decide_release(current, input);
        Self::enact(&mut held, write);
        outcome
    }

    fn get(
        &self,
        commission: &CommissionId,
    ) -> Result<Option<SuspensionSnapshot>, SuspensionStoreError> {
        Ok(self.lock().get(&commission.0).cloned())
    }
}

impl SuspensionStorage for SuspensionStore {
    fn get(&self, identity: &SuspensionKey) -> Option<SuspensionSnapshot> {
        self.lock().get(&identity.commission_id.0).cloned()
    }

    fn put(&mut self, snapshot: SuspensionSnapshot) {
        Self::enact(
            &mut self.lock(),
            Some(SuspensionWrite::Put(Box::new(snapshot))),
        );
    }

    fn delete(&mut self, identity: &SuspensionKey) {
        Self::enact(
            &mut self.lock(),
            Some(SuspensionWrite::Delete(identity.clone())),
        );
    }

    fn list(&self) -> Vec<SuspensionSnapshot> {
        self.lock().values().cloned().collect()
    }
}

/// `HoldSuspension` over memory, through the port.
impl HoldSuspensionBehavior for SuspensionStore {
    fn hold_suspension(
        &mut self,
        input: HoldSuspension,
    ) -> Result<HoldSuspensionOutcome, UnmetObligation> {
        Ok(SuspensionPort::hold(self, input))
    }
}

/// `TakeSuspension` over memory, through the port.
impl TakeSuspensionBehavior for SuspensionStore {
    fn take_suspension(
        &mut self,
        input: TakeSuspension,
    ) -> Result<TakeSuspensionOutcome, UnmetObligation> {
        Ok(SuspensionPort::take(self, input))
    }
}

/// `ReleaseSuspension` over memory, through the port.
impl ReleaseSuspensionBehavior for SuspensionStore {
    fn release_suspension(
        &mut self,
        input: ReleaseSuspension,
    ) -> Result<ReleaseSuspensionOutcome, UnmetObligation> {
        Ok(SuspensionPort::release(self, input))
    }
}
