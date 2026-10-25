#![forbid(unsafe_code)]

//! The `LoomAgent` Kubernetes custom resource, as Loom's specification declares it: the
//! `loom.resource` domain (`ess/domains/resource.yaml`).
//!
//! * [`LoomAgentRecord`] is the controller's record of the resources it reconciles: an in-memory
//!   store behind the generated `LoomAgentStorage` port, over which the generated behaviour of
//!   `loom.resource` answers `ApplyLoomAgent`, `RecordCheck`, `ObserveDeployment` and
//!   `DeleteLoomAgent`, and the generated query answers the view `LoomAgents`. It decides no
//!   outcome itself: each answer is the generated one, and it keeps, in order, the event each
//!   outcome published.
//! * [`crd`] projects the `CustomResourceDefinition` from the JSON Schema documents
//!   `ess generate --kind schema` writes; `loom-xtask crd-generate` writes it to
//!   `deploy/crd/loomagents.loom.beyond10x.dev.yaml`.
//! * [`model`] is the generated `loom.resource` module, so a caller names one path for the types.
//!
//! The record holds the generation the controller applied, the spec check's result and what it
//! last read of the child Deployment, never the spec: the spec stays in the cluster and the
//! controller re-reads it on every reconcile. No command sets the row's `conditions` (ESS 0.69.0
//! admits no list under `sets:`); `b10x-loom-operator` computes them from the row when it
//! publishes the status.

pub mod crd;

pub use loom::obligation::UnmetObligation;
pub use loom::resource as model;

use loom::behaviour::{Generated, LoomAgentStorage};
use loom::resource::obligations::{
    ApplyLoomAgentBehavior, DeleteLoomAgentBehavior, LoomAgentsQuery, ObserveDeploymentBehavior,
    RecordCheckBehavior,
};
use model::{
    ApplyLoomAgent, ApplyLoomAgentOutcome, DeleteLoomAgent, DeleteLoomAgentOutcome,
    LoomAgentApplied, LoomAgentDegraded, LoomAgentDeleted, LoomAgentObserved, LoomAgentRecovered,
    LoomAgentRefused, LoomAgentRunning, LoomAgentSnapshot, LoomAgents, ObserveDeployment,
    ObserveDeploymentOutcome, RecordCheck, RecordCheckOutcome, ResourceName,
};

/// An event a `loom.resource` outcome published, as the record keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceEvent {
    /// `loom.resource.LoomAgentApplied`, by `ApplyLoomAgent`'s `created` or `respecified`.
    Applied(LoomAgentApplied),
    /// `loom.resource.LoomAgentRunning`, by `RecordCheck`'s `running`.
    Running(LoomAgentRunning),
    /// `loom.resource.LoomAgentRefused`, by `RecordCheck`'s `refused`.
    Refused(LoomAgentRefused),
    /// `loom.resource.LoomAgentDegraded`, by `ObserveDeployment`'s `degraded`.
    Degraded(LoomAgentDegraded),
    /// `loom.resource.LoomAgentRecovered`, by `ObserveDeployment`'s `recovered`.
    Recovered(LoomAgentRecovered),
    /// `loom.resource.LoomAgentObserved`, by `ObserveDeployment`'s `still-running` or
    /// `still-degraded`.
    Observed(LoomAgentObserved),
    /// `loom.resource.LoomAgentDeleted`, by `DeleteLoomAgent`'s `deleted`.
    Deleted(LoomAgentDeleted),
}

/// The resources a controller holds, in the order it first applied them, and the events their
/// outcomes published.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoomAgentRecord {
    agents: Vec<LoomAgentSnapshot>,
    events: Vec<ResourceEvent>,
}

impl LoomAgentStorage for LoomAgentRecord {
    fn get(&self, identity: &ResourceName) -> Option<LoomAgentSnapshot> {
        self.agents
            .iter()
            .find(|held| &held.data.name == identity)
            .cloned()
    }

    fn put(&mut self, snapshot: LoomAgentSnapshot) {
        match self
            .agents
            .iter_mut()
            .find(|held| held.data.name == snapshot.data.name)
        {
            Some(held) => *held = snapshot,
            None => self.agents.push(snapshot),
        }
    }

    fn delete(&mut self, identity: &ResourceName) {
        self.agents.retain(|held| &held.data.name != identity);
    }

    fn list(&self) -> Vec<LoomAgentSnapshot> {
        self.agents.clone()
    }
}

impl LoomAgentRecord {
    /// An empty record.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs one generated behaviour over this record as its storage port.
    fn generated<T>(&mut self, behaviour: impl FnOnce(&mut Generated<Self>) -> T) -> T {
        let mut generated = Generated::new(std::mem::take(self));
        let answer = behaviour(&mut generated);
        *self = generated.ports;
        answer
    }

    /// `loom.resource.ApplyLoomAgent`, by its generated behaviour.
    ///
    /// # Errors
    ///
    /// The generated behaviour's refusal of a request the specification declares no outcome for.
    pub fn apply_loom_agent(
        &mut self,
        input: ApplyLoomAgent,
    ) -> Result<ApplyLoomAgentOutcome, UnmetObligation> {
        let outcome = self.generated(|generated| generated.apply_loom_agent(input))?;
        match &outcome {
            ApplyLoomAgentOutcome::Created { loom_agent_applied }
            | ApplyLoomAgentOutcome::Respecified { loom_agent_applied } => self
                .events
                .push(ResourceEvent::Applied(loom_agent_applied.clone())),
            ApplyLoomAgentOutcome::Stale { .. } | ApplyLoomAgentOutcome::Unchanged => {}
        }
        Ok(outcome)
    }

    /// `loom.resource.RecordCheck`, by its generated behaviour.
    ///
    /// # Errors
    ///
    /// The generated behaviour's refusal of a request the specification declares no outcome for.
    pub fn record_check(
        &mut self,
        input: RecordCheck,
    ) -> Result<RecordCheckOutcome, UnmetObligation> {
        let outcome = self.generated(|generated| generated.record_check(input))?;
        match &outcome {
            RecordCheckOutcome::Running { loom_agent_running } => self
                .events
                .push(ResourceEvent::Running(loom_agent_running.clone())),
            RecordCheckOutcome::Refused { loom_agent_refused } => self
                .events
                .push(ResourceEvent::Refused(loom_agent_refused.clone())),
            RecordCheckOutcome::Unknown { .. }
            | RecordCheckOutcome::Stale { .. }
            | RecordCheckOutcome::OutOfOrder { .. } => {}
        }
        Ok(outcome)
    }

    /// `loom.resource.ObserveDeployment`, by its generated behaviour.
    ///
    /// # Errors
    ///
    /// The generated behaviour's refusal of a request the specification declares no outcome for.
    pub fn observe_deployment(
        &mut self,
        input: ObserveDeployment,
    ) -> Result<ObserveDeploymentOutcome, UnmetObligation> {
        let outcome = self.generated(|generated| generated.observe_deployment(input))?;
        match &outcome {
            ObserveDeploymentOutcome::Degraded {
                loom_agent_degraded,
            } => self
                .events
                .push(ResourceEvent::Degraded(loom_agent_degraded.clone())),
            ObserveDeploymentOutcome::Recovered {
                loom_agent_recovered,
            } => self
                .events
                .push(ResourceEvent::Recovered(loom_agent_recovered.clone())),
            ObserveDeploymentOutcome::StillRunning {
                loom_agent_observed,
            }
            | ObserveDeploymentOutcome::StillDegraded {
                loom_agent_observed,
            } => self
                .events
                .push(ResourceEvent::Observed(loom_agent_observed.clone())),
            ObserveDeploymentOutcome::Unknown { .. }
            | ObserveDeploymentOutcome::NoDeployment { .. } => {}
        }
        Ok(outcome)
    }

    /// `loom.resource.DeleteLoomAgent`, by its generated behaviour.
    ///
    /// # Errors
    ///
    /// The generated behaviour's refusal of a request the specification declares no outcome for.
    pub fn delete_loom_agent(
        &mut self,
        input: DeleteLoomAgent,
    ) -> Result<DeleteLoomAgentOutcome, UnmetObligation> {
        let outcome = self.generated(|generated| generated.delete_loom_agent(input))?;
        match &outcome {
            DeleteLoomAgentOutcome::Deleted { loom_agent_deleted } => self
                .events
                .push(ResourceEvent::Deleted(loom_agent_deleted.clone())),
            DeleteLoomAgentOutcome::Unknown { .. } => {}
        }
        Ok(outcome)
    }

    /// The view `loom.resource.LoomAgents`, by its generated query: one row per resource held.
    ///
    /// # Errors
    ///
    /// The generated query's refusal of a row its declared type cannot hold.
    pub fn loom_agents(&self) -> Result<Vec<LoomAgents>, UnmetObligation> {
        Generated::new(Self {
            agents: self.agents.clone(),
            events: Vec::new(),
        })
        .loom_agents()
    }

    /// The resource held under `name`, with its state.
    #[must_use]
    pub fn agent(&self, name: &ResourceName) -> Option<LoomAgentSnapshot> {
        LoomAgentStorage::get(self, name)
    }

    /// Every event the record's outcomes published, in order.
    #[must_use]
    pub fn events(&self) -> &[ResourceEvent] {
        &self.events
    }
}
