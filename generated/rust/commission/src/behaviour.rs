// generated from commission v1
// model digest 1ba42c043f9934dcbbb38e6d540f230eda0871defa63756e7c3db7d01b3050c3
// contract digest 9f2ffaa60ef43fd8f3ad719e5f980e33a4fc329e9ba4c5a990bbcfa1579a3135
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

/// `commission.responsibility.ResumeRun`, generated: every outcome is one the specification fully determines.
impl<P> crate::responsibility::obligations::ResumeRunBehavior for Generated<P>
where
    P: RunStorage,
{
    fn resume_run(&mut self, input: crate::responsibility::ResumeRun) -> Result<crate::responsibility::ResumeRunOutcome, UnmetObligation> {
        let _ = &input;
        // `resumed`: the default.
        let Some(held) = RunStorage::get(&self.ports, &input.run_id) else {
            return Ok(crate::responsibility::ResumeRunOutcome::WrongStateUnknownInstance);
        };
        let _ = &held;
        let held_state = held.state;
        let moved = match held.refine() {
            crate::responsibility::AnyRun::Suspended(instance) => crate::responsibility::AnyRun::Running(instance.resume()),
            _ => return Ok(crate::responsibility::ResumeRunOutcome::WrongState { error: crate::responsibility::RunStateConflict { state: held_state } }),
        };
        let next = moved.snapshot();
        let answer = crate::responsibility::ResumeRunOutcome::Resumed { run_resumed: crate::responsibility::RunResumed { run_id: input.run_id.clone() } };
        RunStorage::put(&mut self.ports, next);
        return Ok(answer);
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
