// generated from commission v1
// model digest 8baad8a2a232f1823d8fce586ddd35c901af8923715a1eb1fbc3f62817da8e4f
// contract digest c27daebab1de8a70c1c4985e2a48176db7d5700784cedf99b11902b83a5dea7a
// do not edit: regenerate with `ess synthesize --layout crate`

//! What the specification fully determines, generated: the behaviour of every command the plan
//! lists as generated, written against ports the implementor supplies.
//!
//! Storage is a port: one trait per entity, get, put and delete of a snapshot by identity. ess
//! generates the trait and never a store. `Context` is the other port: the caller's attributes,
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
/// Keyed by the identity `run_id`. ess generates this trait and never an implementation of it.
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

/// What the specification leaves to the implementor's context — a port the implementor provides.
///
/// The caller's attributes, the values the model says the implementation assigns, and the answer
/// to each `external:` branch.
pub trait Context {
    /// A new `commission.responsibility.RunId`, which the model says the implementation assigns — a created identity, a
    /// `{generated: true}` value, or an event field the model leaves undetermined.
    fn generate_commission_responsibility_run_id(&mut self) -> crate::responsibility::RunId;
}

/// Every generated behaviour of this workspace, over the ports `P` supplies.
///
/// `P` implements the storage trait of each entity a generated behaviour reads or writes,
/// `Context` where one asks it anything, and every `…Behavior` and `…Query` trait the plan still
/// owes; `Generated<P>` forwards those to it.
pub struct Generated<P> {
    /// The storage and context ports, and every behaviour or query still owed.
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
        RunStorage::put(&mut self.ports, next);
        return Ok(crate::responsibility::ResumeRunOutcome::Resumed { run_resumed: crate::responsibility::RunResumed { run_id: input.run_id.clone() } });
    }
}

impl<P: crate::responsibility::obligations::RevalidateActionRequestBehavior> crate::responsibility::obligations::RevalidateActionRequestBehavior for Generated<P> {
    fn revalidate_action_request(&mut self, input: crate::responsibility::RevalidateActionRequest) -> Result<crate::responsibility::RevalidateActionRequestOutcome, UnmetObligation> {
        crate::responsibility::obligations::RevalidateActionRequestBehavior::revalidate_action_request(&mut self.ports, input)
    }
}

/// `commission.responsibility.StartRun`, generated: every outcome is one the specification fully determines.
impl<P> crate::responsibility::obligations::StartRunBehavior for Generated<P>
where
    P: Context + RunStorage,
{
    fn start_run(&mut self, input: crate::responsibility::StartRun) -> Result<crate::responsibility::StartRunOutcome, UnmetObligation> {
        let _ = &input;
        // `started`: the default.
        let identity: crate::responsibility::RunId = self.ports.generate_commission_responsibility_run_id();
        let data = crate::responsibility::RunData {
            run_id: identity.clone(),
            commission_id: input.commission_id.clone(),
            case_revision: input.case_revision.clone(),
        };
        RunStorage::put(&mut self.ports, crate::responsibility::AnyRun::Running(crate::responsibility::Run::new(data)).snapshot());
        return Ok(crate::responsibility::StartRunOutcome::Started { run_started: crate::responsibility::RunStarted { run_id: identity.clone(), commission_id: input.commission_id.clone(), case_revision: input.case_revision.clone() } });
    }
}

/// `commission.responsibility.SuspendRun`, generated: every outcome is one the specification fully determines.
impl<P> crate::responsibility::obligations::SuspendRunBehavior for Generated<P>
where
    P: RunStorage,
{
    fn suspend_run(&mut self, input: crate::responsibility::SuspendRun) -> Result<crate::responsibility::SuspendRunOutcome, UnmetObligation> {
        let _ = &input;
        // `suspended`: the default.
        let Some(held) = RunStorage::get(&self.ports, &input.run_id) else {
            return Ok(crate::responsibility::SuspendRunOutcome::WrongStateUnknownInstance);
        };
        let _ = &held;
        let held_state = held.state;
        let moved = match held.refine() {
            crate::responsibility::AnyRun::Running(instance) => crate::responsibility::AnyRun::Suspended(instance.suspend()),
            _ => return Ok(crate::responsibility::SuspendRunOutcome::WrongState { error: crate::responsibility::RunStateConflict { state: held_state } }),
        };
        let next = moved.snapshot();
        RunStorage::put(&mut self.ports, next);
        return Ok(crate::responsibility::SuspendRunOutcome::Suspended { run_suspended: crate::responsibility::RunSuspended { run_id: input.run_id.clone(), reason: input.reason.clone() } });
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
