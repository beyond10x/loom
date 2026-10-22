// generated from intake v1
// model digest 4ed9220e25a096d3ee91fdabe8d0f6b9cdd3b59b5a826a2dfa07924bac771607
// contract digest 0d10146dd4b2743e959d71f4eeb3cfb293bf2dba47c285732ceb93672da1b95c
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

/// Where `intake.routing.MergeHandoff` is stored — a port the implementor provides.
///
/// Keyed by the identity `action_request_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait MergeHandoffStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::routing::ActionRequestId) -> Option<crate::routing::MergeHandoffSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::routing::MergeHandoffSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::routing::ActionRequestId);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::routing::MergeHandoffSnapshot>;
}

/// Where `intake.routing.ResumeContext` is stored — a port the implementor provides.
///
/// Keyed by the identity `commission_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait ResumeContextStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::routing::CommissionId) -> Option<crate::routing::ResumeContextSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::routing::ResumeContextSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::routing::CommissionId);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::routing::ResumeContextSnapshot>;
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

impl<P: crate::routing::obligations::DropResumeContextBehavior> crate::routing::obligations::DropResumeContextBehavior for Generated<P> {
    fn drop_resume_context(&mut self, input: crate::routing::DropResumeContext) -> Result<crate::routing::DropResumeContextOutcome, UnmetObligation> {
        crate::routing::obligations::DropResumeContextBehavior::drop_resume_context(&mut self.ports, input)
    }
}

impl<P: crate::routing::obligations::KeepResumeContextBehavior> crate::routing::obligations::KeepResumeContextBehavior for Generated<P> {
    fn keep_resume_context(&mut self, input: crate::routing::KeepResumeContext) -> Result<crate::routing::KeepResumeContextOutcome, UnmetObligation> {
        crate::routing::obligations::KeepResumeContextBehavior::keep_resume_context(&mut self.ports, input)
    }
}

impl<P: crate::routing::obligations::RecordMergeHandoffBehavior> crate::routing::obligations::RecordMergeHandoffBehavior for Generated<P> {
    fn record_merge_handoff(&mut self, input: crate::routing::RecordMergeHandoff) -> Result<crate::routing::RecordMergeHandoffOutcome, UnmetObligation> {
        crate::routing::obligations::RecordMergeHandoffBehavior::record_merge_handoff(&mut self.ports, input)
    }
}

/// `intake.routing.MergeHandoffs`, generated: every row is one the specification fully determines from the stored `intake.routing.MergeHandoff`s.
impl<P> crate::routing::obligations::MergeHandoffsQuery for Generated<P>
where
    P: MergeHandoffStorage,
{
    fn merge_handoffs(&self) -> Result<Vec<crate::routing::MergeHandoffs>, UnmetObligation> {
        let admitted = MergeHandoffStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::routing::MergeHandoffs {
                action_request_id: held.data.action_request_id,
                commission_id: held.data.commission_id,
                workspace: held.data.workspace,
                head: held.data.head,
                state: held.state,
            })
            .collect())
    }
}

/// `intake.routing.ResumeContexts`, generated: every row is one the specification fully determines from the stored `intake.routing.ResumeContext`s.
impl<P> crate::routing::obligations::ResumeContextsQuery for Generated<P>
where
    P: ResumeContextStorage,
{
    fn resume_contexts(&self) -> Result<Vec<crate::routing::ResumeContexts>, UnmetObligation> {
        let admitted = ResumeContextStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::routing::ResumeContexts {
                commission_id: held.data.commission_id,
                case_id: held.data.case_id,
                agent_revision_id: held.data.agent_revision_id,
                intent: held.data.intent,
                workspace: held.data.workspace,
                test_command: held.data.test_command,
                test_paths: held.data.test_paths,
                regression_test_command: held.data.regression_test_command,
                max_steps: held.data.max_steps,
                confinement: held.data.confinement,
                cgroup_root: held.data.cgroup_root,
                model: held.data.model,
                catalog: held.data.catalog,
                config: held.data.config,
                state: held.state,
            })
            .collect())
    }
}
