// generated from commission v1
// model digest f53571e132b2f71840edfe761bc401e64fb69f09c02e5231f9f99c89c76020a5
// contract digest d8e6b1ede68b8946fc3b58215b93b34033de96d39a3f84cf8669f4b6e8966278
// do not edit: regenerate with `ess synthesize --layout crate`

//! What the specification fully determines, generated: the behaviour of every command the plan
//! lists as generated, written against ports the implementor supplies.
//!
//! Storage is a port: one trait per entity, get, put and delete of a snapshot by identity. ess
//! preserves the trait; generated network entries supply an ephemeral store. `Context` carries the caller's attributes,
//! every identity and value the model says the implementation assigns, and the answer to each
//! `external:` branch. [`Generated`] implements every generated `…Behavior` trait over those ports
//! and forwards every behaviour and query the plan still owes to them, so it is a complete bundle
//! for every component port. To replace one generated behaviour, write a bundle of your own that
//! implements that trait and delegates the rest to a `Generated`.
//!
//! An `Err` from a generated behaviour is the typed refusal naming the command: the model declares
//! no outcome for the request (a guard is undecidable over it, or no declared branch answers it),
//! or — as `entity invariant` — the declared outcome would leave an entity breaking an invariant.

use crate::obligation::UnmetObligation;

/// Where `commission.responsibility.Run` is stored — a port the implementor provides.
///
/// Keyed by the identity `run_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait RunStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::responsibility::RunId) -> Option<crate::responsibility::RunSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::responsibility::RunSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::responsibility::RunId);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::responsibility::RunSnapshot>;
}

/// Where `commission.responsibility.Suspension` is stored — a port the implementor provides.
///
/// Keyed by the identity `suspension`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait SuspensionStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::responsibility::SuspensionKey) -> Option<crate::responsibility::SuspensionSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::responsibility::SuspensionSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::responsibility::SuspensionKey);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::responsibility::SuspensionSnapshot>;
}

/// Every generated behaviour of this workspace, over the ports `P` supplies.
///
/// `P` implements the storage trait of each entity a generated behaviour reads or writes, and
/// every `…Behavior` and `…Query` trait the plan still owes; `Generated<P>` forwards those to it.
pub struct Generated<P> {
    /// The storage ports, and every behaviour or query still owed.
    pub ports: P,
}

impl<P> Generated<P> {
    /// The generated behaviours, over `ports`.
    pub fn new(ports: P) -> Self {
        Self { ports }
    }
}

impl<P: crate::responsibility::obligations::HoldSuspensionBehavior> crate::responsibility::obligations::HoldSuspensionBehavior for Generated<P> {
    fn hold_suspension(&mut self, input: crate::responsibility::HoldSuspension) -> Result<crate::responsibility::HoldSuspensionOutcome, UnmetObligation> {
        crate::responsibility::obligations::HoldSuspensionBehavior::hold_suspension(&mut self.ports, input)
    }
}

impl<P: crate::responsibility::obligations::ReleaseSuspensionBehavior> crate::responsibility::obligations::ReleaseSuspensionBehavior for Generated<P> {
    fn release_suspension(&mut self, input: crate::responsibility::ReleaseSuspension) -> Result<crate::responsibility::ReleaseSuspensionOutcome, UnmetObligation> {
        crate::responsibility::obligations::ReleaseSuspensionBehavior::release_suspension(&mut self.ports, input)
    }
}

impl<P: crate::responsibility::obligations::ResumeRunBehavior> crate::responsibility::obligations::ResumeRunBehavior for Generated<P> {
    fn resume_run(&mut self, input: crate::responsibility::ResumeRun) -> Result<crate::responsibility::ResumeRunOutcome, UnmetObligation> {
        crate::responsibility::obligations::ResumeRunBehavior::resume_run(&mut self.ports, input)
    }
}

impl<P: crate::responsibility::obligations::RevalidateActionRequestBehavior> crate::responsibility::obligations::RevalidateActionRequestBehavior for Generated<P> {
    fn revalidate_action_request(&mut self, input: crate::responsibility::RevalidateActionRequest) -> Result<crate::responsibility::RevalidateActionRequestOutcome, UnmetObligation> {
        crate::responsibility::obligations::RevalidateActionRequestBehavior::revalidate_action_request(&mut self.ports, input)
    }
}

impl<P: crate::responsibility::obligations::StartRunBehavior> crate::responsibility::obligations::StartRunBehavior for Generated<P> {
    fn start_run(&mut self, input: crate::responsibility::StartRun) -> Result<crate::responsibility::StartRunOutcome, UnmetObligation> {
        crate::responsibility::obligations::StartRunBehavior::start_run(&mut self.ports, input)
    }
}

impl<P: crate::responsibility::obligations::SuspendRunBehavior> crate::responsibility::obligations::SuspendRunBehavior for Generated<P> {
    fn suspend_run(&mut self, input: crate::responsibility::SuspendRun) -> Result<crate::responsibility::SuspendRunOutcome, UnmetObligation> {
        crate::responsibility::obligations::SuspendRunBehavior::suspend_run(&mut self.ports, input)
    }
}

impl<P: crate::responsibility::obligations::TakeSuspensionBehavior> crate::responsibility::obligations::TakeSuspensionBehavior for Generated<P> {
    fn take_suspension(&mut self, input: crate::responsibility::TakeSuspension) -> Result<crate::responsibility::TakeSuspensionOutcome, UnmetObligation> {
        crate::responsibility::obligations::TakeSuspensionBehavior::take_suspension(&mut self.ports, input)
    }
}

/// `commission.responsibility.RunStates`, generated: every row is one the specification fully determines from the stored `commission.responsibility.Run`s.
impl<P> crate::responsibility::obligations::RunStatesQuery for Generated<P>
where
    P: RunStorage,
{
    fn run_states(&self) -> Result<Vec<crate::responsibility::RunStates>, UnmetObligation> {
        let admitted = RunStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::responsibility::RunStates {
                run_id: held.data.run_id,
                commission_id: held.data.commission_id,
                case_revision: held.data.case_revision,
                state: held.state,
            })
            .collect())
    }
}

/// `commission.responsibility.SuspensionStates`, generated: every row is one the specification fully determines from the stored `commission.responsibility.Suspension`s.
impl<P> crate::responsibility::obligations::SuspensionStatesQuery for Generated<P>
where
    P: SuspensionStorage,
{
    fn suspension_states(&self) -> Result<Vec<crate::responsibility::SuspensionStates>, UnmetObligation> {
        let admitted = SuspensionStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::responsibility::SuspensionStates {
                suspension: held.data.suspension,
                commission_id: held.data.commission_id,
                run_id: held.data.run_id,
                action_request_id: held.data.action_request_id,
                state: held.state,
            })
            .collect())
    }
}
