// generated from loom v1
// model digest bd0680f8ac3534518a439d54a62dfd2a90bf5c733881ebeb4328aed4f2b64019
// contract digest 686d3e50c2ac373e8881a3960e68cf840d8e87f71ccb441f5efdc3580e3c3fa1
// do not edit: regenerate with `ess synthesize --layout crate`

//! Resource — `loom.resource`.
//!
//! The `LoomAgent` custom resource (group `loom.beyond10x.dev`, version `v1alpha1`): its spec, the `loom.agent/1` document and what the pod needs beside it, and its lifecycle as the controller reconciles it: applied into Pending, Running or Refused by the spec check, Degraded and back by the child Deployment's availability, Pending again on a new generation, and gone on delete.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// AgentVariant — `loom.resource.AgentVariant`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentVariant {
    /// `AgentServe`.
    AgentServe,
    /// `Command`.
    Command,
}

/// CheckResult — `loom.resource.CheckResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    /// `exit_code` — `Integer`.
    pub exit_code: i64,
    /// `message` — `String`.
    pub message: String,
}

/// CommandSpec — `loom.resource.CommandSpec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    /// `entrypoint` — `Optional<List<String>>`.
    pub entrypoint: Option<Vec<String>>,
    /// `args` — `Optional<List<String>>`.
    pub args: Option<Vec<String>>,
}

/// Condition — `loom.resource.Condition`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    /// `type` — `loom.resource.ConditionType`.
    pub r#type: ConditionType,
    /// `status` — `loom.resource.ConditionStatus`.
    pub status: ConditionStatus,
    /// `reason` — `String`.
    pub reason: String,
    /// `message` — `String`.
    pub message: String,
}

/// ConditionStatus — `loom.resource.ConditionStatus`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionStatus {
    /// `True`.
    True,
    /// `False`.
    False,
    /// `Unknown`.
    Unknown,
}

/// ConditionType — `loom.resource.ConditionType`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionType {
    /// `Checked`.
    Checked,
    /// `Available`.
    Available,
}

/// DeploymentAvailability — `loom.resource.DeploymentAvailability`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentAvailability {
    /// `Available`.
    Available,
    /// `Unavailable`.
    Unavailable,
}

/// DeploymentStatus — `loom.resource.DeploymentStatus`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentStatus {
    /// `availability` — `loom.resource.DeploymentAvailability`.
    pub availability: DeploymentAvailability,
}

/// ImageRef — `loom.resource.ImageRef`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRef {
    /// `repository` — `String`.
    pub repository: String,
    /// `tag` — `String`.
    pub tag: String,
}

/// The states of `loom.resource.LoomAgent`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `LoomAgent<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoomAgentState {
    /// `Degraded`.
    Degraded,
    /// `Pending`.
    Pending,
    /// `Refused`.
    Refused,
    /// `Running`.
    Running,
}

/// LoomAgentSpec — `loom.resource.LoomAgentSpec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentSpec {
    /// `agent` — `loom.agent.AgentSpec`.
    pub agent: crate::agent::AgentSpec,
    /// `routes` — `Json`.
    pub routes: crate::json::Value,
    /// `connectors_config` — `Optional<String>`.
    pub connectors_config: Option<String>,
    /// `secret_ref` — `Optional<loom.resource.SecretRef>`.
    pub secret_ref: Option<SecretRef>,
    /// `secret_files` — `Optional<loom.resource.SecretFiles>`.
    pub secret_files: Option<SecretFiles>,
    /// `image` — `Optional<loom.resource.ImageRef>`.
    pub image: Option<ImageRef>,
    /// `command` — `Optional<loom.resource.CommandSpec>`.
    pub command: Option<CommandSpec>,
    /// `workspace` — `Optional<loom.resource.Workspace>`.
    pub workspace: Option<Workspace>,
}

/// ResourceName — `loom.resource.ResourceName`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceName(pub String);

/// SecretFiles — `loom.resource.SecretFiles`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretFiles {
    /// `name` — `String`.
    pub name: String,
    /// `mount_path` — `String`.
    pub mount_path: String,
}

/// SecretRef — `loom.resource.SecretRef`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRef {
    /// `name` — `String`.
    pub name: String,
}

/// Workspace — `loom.resource.Workspace`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    /// `mount_path` — `String`.
    pub mount_path: String,
    /// `claim_name` — `Optional<String>`.
    pub claim_name: Option<String>,
}

/// What LoomAgent — `loom.resource.LoomAgent` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`LoomAgent<S>`], and at a boundary by [`LoomAgentSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentData {
    /// The identity: `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `observed_generation` — `Integer`.
    pub observed_generation: i64,
    /// `check` — `Optional<loom.resource.CheckResult>`.
    pub check: Option<CheckResult>,
    /// `deployment` — `Optional<loom.resource.DeploymentStatus>`.
    pub deployment: Option<DeploymentStatus>,
    /// `variant` — `Optional<loom.resource.AgentVariant>`.
    pub variant: Option<AgentVariant>,
    /// `conditions` — `Optional<List<loom.resource.Condition>>`.
    pub conditions: Option<Vec<Condition>>,
}

/// The states of `loom.resource.LoomAgent`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](loom_agent_state::Marker), so [`LoomAgent<S>`](LoomAgent) can only ever rest in a real state.
pub mod loom_agent_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Degraded {}
        impl Sealed for super::Pending {}
        impl Sealed for super::Refused {}
        impl Sealed for super::Running {}
    }

    /// A declared state of `LoomAgent`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::LoomAgentState;
    }

    /// `Degraded`.
    pub struct Degraded;

    impl Marker for Degraded {
        const STATE: super::LoomAgentState = super::LoomAgentState::Degraded;
    }

    /// `Pending`. Where a new instance starts.
    pub struct Pending;

    impl Marker for Pending {
        const STATE: super::LoomAgentState = super::LoomAgentState::Pending;
    }

    /// `Refused`.
    pub struct Refused;

    impl Marker for Refused {
        const STATE: super::LoomAgentState = super::LoomAgentState::Refused;
    }

    /// `Running`.
    pub struct Running;

    impl Marker for Running {
        const STATE: super::LoomAgentState = super::LoomAgentState::Running;
    }
}

/// LoomAgent — `loom.resource.LoomAgent` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Pending`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`LoomAgentSnapshot`]
/// and [`LoomAgentSnapshot::refine`].
pub struct LoomAgent<S: loom_agent_state::Marker> {
    data: LoomAgentData,
    state: core::marker::PhantomData<S>,
}

impl<S: loom_agent_state::Marker> LoomAgent<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> LoomAgentState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &LoomAgentData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> LoomAgentData {
        self.data
    }
}

impl LoomAgent<loom_agent_state::Pending> {
    /// A new instance, resting in `Pending` — the only state the lifecycle starts one in.
    pub fn new(data: LoomAgentData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

impl LoomAgent<loom_agent_state::Degraded> {
    /// `recover` — `Degraded` → `Running`. Taken by the `recovered` outcome of `loom.resource.ObserveDeployment`.
    pub fn recover(self) -> LoomAgent<loom_agent_state::Running> {
        LoomAgent {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `observe-degraded` — `Degraded` → `Degraded`. Taken by the `still-degraded` outcome of `loom.resource.ObserveDeployment`.
    pub fn observe_degraded(self) -> LoomAgent<loom_agent_state::Degraded> {
        LoomAgent {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `respecify` — `Degraded` → `Pending`. Taken by the `respecified` outcome of `loom.resource.ApplyLoomAgent`.
    pub fn respecify(self) -> LoomAgent<loom_agent_state::Pending> {
        LoomAgent {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }
}

impl LoomAgent<loom_agent_state::Pending> {
    /// `run` — `Pending` → `Running`. Taken by the `running` outcome of `loom.resource.RecordCheck`.
    pub fn run(self) -> LoomAgent<loom_agent_state::Running> {
        LoomAgent {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `refuse` — `Pending` → `Refused`. Taken by the `refused` outcome of `loom.resource.RecordCheck`.
    pub fn refuse(self) -> LoomAgent<loom_agent_state::Refused> {
        LoomAgent {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `respecify` — `Pending` → `Pending`. Taken by the `respecified` outcome of `loom.resource.ApplyLoomAgent`.
    pub fn respecify(self) -> LoomAgent<loom_agent_state::Pending> {
        LoomAgent {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }
}

impl LoomAgent<loom_agent_state::Refused> {
    /// `respecify` — `Refused` → `Pending`. Taken by the `respecified` outcome of `loom.resource.ApplyLoomAgent`.
    pub fn respecify(self) -> LoomAgent<loom_agent_state::Pending> {
        LoomAgent {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }
}

impl LoomAgent<loom_agent_state::Running> {
    /// `degrade` — `Running` → `Degraded`. Taken by the `degraded` outcome of `loom.resource.ObserveDeployment`.
    pub fn degrade(self) -> LoomAgent<loom_agent_state::Degraded> {
        LoomAgent {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `observe-running` — `Running` → `Running`. Taken by the `still-running` outcome of `loom.resource.ObserveDeployment`.
    pub fn observe_running(self) -> LoomAgent<loom_agent_state::Running> {
        LoomAgent {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `respecify` — `Running` → `Pending`. Taken by the `respecified` outcome of `loom.resource.ApplyLoomAgent`.
    pub fn respecify(self) -> LoomAgent<loom_agent_state::Pending> {
        LoomAgent {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }
}

/// `loom.resource.LoomAgent` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`LoomAgentSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: LoomAgentState,
    /// What it holds.
    pub data: LoomAgentData,
}

/// An `LoomAgent` in whichever declared state it was found.
pub enum AnyLoomAgent {
    /// Resting in `Degraded`.
    Degraded(LoomAgent<loom_agent_state::Degraded>),
    /// Resting in `Pending`.
    Pending(LoomAgent<loom_agent_state::Pending>),
    /// Resting in `Refused`.
    Refused(LoomAgent<loom_agent_state::Refused>),
    /// Resting in `Running`.
    Running(LoomAgent<loom_agent_state::Running>),
}

impl LoomAgentSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `LoomAgentState` cannot spell one.
    pub fn refine(self) -> AnyLoomAgent {
        match self.state {
            LoomAgentState::Degraded => AnyLoomAgent::Degraded(LoomAgent {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            LoomAgentState::Pending => AnyLoomAgent::Pending(LoomAgent {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            LoomAgentState::Refused => AnyLoomAgent::Refused(LoomAgent {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            LoomAgentState::Running => AnyLoomAgent::Running(LoomAgent {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyLoomAgent {
    /// The state, as the runtime value.
    pub fn state(&self) -> LoomAgentState {
        match self {
            Self::Degraded(_) => LoomAgentState::Degraded,
            Self::Pending(_) => LoomAgentState::Pending,
            Self::Refused(_) => LoomAgentState::Refused,
            Self::Running(_) => LoomAgentState::Running,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> LoomAgentSnapshot {
        match self {
            Self::Degraded(instance) => LoomAgentSnapshot {
                state: LoomAgentState::Degraded,
                data: instance.into_data(),
            },
            Self::Pending(instance) => LoomAgentSnapshot {
                state: LoomAgentState::Pending,
                data: instance.into_data(),
            },
            Self::Refused(instance) => LoomAgentSnapshot {
                state: LoomAgentState::Refused,
                data: instance.into_data(),
            },
            Self::Running(instance) => LoomAgentSnapshot {
                state: LoomAgentState::Running,
                data: instance.into_data(),
            },
        }
    }
}

/// ApplyLoomAgent — the input of `loom.resource.ApplyLoomAgent`.
///
/// Everything it can result in is [`ApplyLoomAgentOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyLoomAgent {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `generation` — `Integer`.
    pub generation: i64,
}

/// Everything `loom.resource.ApplyLoomAgent` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyLoomAgentOutcome {
    /// `created` — for an identity no record carries.
    Created {
        /// The `loom.resource.LoomAgentApplied` this outcome publishes.
        loom_agent_applied: LoomAgentApplied,
    },
    /// `stale` — when the existing subject's stored fields satisfy `observed_generation > input.generation`.
    Stale {
        /// Why it was refused: `loom.resource.StaleGeneration`.
        error: StaleGeneration,
    },
    /// `unchanged` — when the existing subject's stored fields satisfy `observed_generation == input.generation`.
    Unchanged,
    /// `respecified` — otherwise.
    Respecified {
        /// The `loom.resource.LoomAgentApplied` this outcome publishes.
        loom_agent_applied: LoomAgentApplied,
    },
}

/// DeleteLoomAgent — the input of `loom.resource.DeleteLoomAgent`.
///
/// Everything it can result in is [`DeleteLoomAgentOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteLoomAgent {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
}

/// Everything `loom.resource.DeleteLoomAgent` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteLoomAgentOutcome {
    /// `deleted` — otherwise.
    Deleted {
        /// The `loom.resource.LoomAgentDeleted` this outcome publishes.
        loom_agent_deleted: LoomAgentDeleted,
    },
    /// `unknown` — for an identity no record carries.
    Unknown {
        /// Why it was refused: `loom.resource.LoomAgentNotFound`.
        error: LoomAgentNotFound,
    },
}

/// ObserveDeployment — the input of `loom.resource.ObserveDeployment`.
///
/// Everything it can result in is [`ObserveDeploymentOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserveDeployment {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `availability` — `loom.resource.DeploymentAvailability`.
    pub availability: DeploymentAvailability,
}

/// Everything `loom.resource.ObserveDeployment` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveDeploymentOutcome {
    /// `unknown` — for an identity no record carries.
    Unknown {
        /// Why it was refused: `loom.resource.LoomAgentNotFound`.
        error: LoomAgentNotFound,
    },
    /// `degraded` — when the existing subject is in Running and `availability == Unavailable`.
    Degraded {
        /// The `loom.resource.LoomAgentDegraded` this outcome publishes.
        loom_agent_degraded: LoomAgentDegraded,
    },
    /// `still-running` — when the existing subject is in Running and `availability == Available`.
    StillRunning {
        /// The `loom.resource.LoomAgentObserved` this outcome publishes.
        loom_agent_observed: LoomAgentObserved,
    },
    /// `recovered` — when the existing subject is in Degraded and `availability == Available`.
    Recovered {
        /// The `loom.resource.LoomAgentRecovered` this outcome publishes.
        loom_agent_recovered: LoomAgentRecovered,
    },
    /// `still-degraded` — when the existing subject is in Degraded and `availability == Unavailable`.
    StillDegraded {
        /// The `loom.resource.LoomAgentObserved` this outcome publishes.
        loom_agent_observed: LoomAgentObserved,
    },
    /// `no-deployment` — otherwise.
    NoDeployment {
        /// Why it was refused: `loom.resource.LoomAgentStateConflict`.
        error: LoomAgentStateConflict,
    },
}

/// RecordCheck — the input of `loom.resource.RecordCheck`.
///
/// Everything it can result in is [`RecordCheckOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCheck {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `generation` — `Integer`.
    pub generation: i64,
    /// `exit_code` — `Integer`.
    pub exit_code: i64,
    /// `message` — `String`.
    pub message: String,
    /// `variant` — `loom.resource.AgentVariant`.
    pub variant: AgentVariant,
}

/// Everything `loom.resource.RecordCheck` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordCheckOutcome {
    /// `unknown` — for an identity no record carries.
    Unknown {
        /// Why it was refused: `loom.resource.LoomAgentNotFound`.
        error: LoomAgentNotFound,
    },
    /// `stale` — when the existing subject's stored fields satisfy `observed_generation != input.generation`.
    Stale {
        /// Why it was refused: `loom.resource.StaleGeneration`.
        error: StaleGeneration,
    },
    /// `running` — when `exit_code == 0`.
    Running {
        /// The `loom.resource.LoomAgentRunning` this outcome publishes.
        loom_agent_running: LoomAgentRunning,
    },
    /// `refused` — otherwise.
    Refused {
        /// The `loom.resource.LoomAgentRefused` this outcome publishes.
        loom_agent_refused: LoomAgentRefused,
    },
    /// `out-of-order` — from a state no declared move starts in.
    OutOfOrder {
        /// Why it was refused: `loom.resource.LoomAgentStateConflict`.
        error: LoomAgentStateConflict,
    },
}

/// LoomAgentApplied — the event `loom.resource.LoomAgentApplied`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentApplied {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `generation` — `Integer`.
    pub generation: i64,
}

/// LoomAgentDegraded — the event `loom.resource.LoomAgentDegraded`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentDegraded {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
}

/// LoomAgentDeleted — the event `loom.resource.LoomAgentDeleted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentDeleted {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
}

/// LoomAgentObserved — the event `loom.resource.LoomAgentObserved`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentObserved {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `availability` — `loom.resource.DeploymentAvailability`.
    pub availability: DeploymentAvailability,
}

/// LoomAgentRecovered — the event `loom.resource.LoomAgentRecovered`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentRecovered {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
}

/// LoomAgentRefused — the event `loom.resource.LoomAgentRefused`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentRefused {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `generation` — `Integer`.
    pub generation: i64,
    /// `message` — `String`.
    pub message: String,
}

/// LoomAgentRunning — the event `loom.resource.LoomAgentRunning`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentRunning {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `generation` — `Integer`.
    pub generation: i64,
}

/// The declared error `loom.resource.LoomAgentNotFound`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentNotFound {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
}

/// The declared error `loom.resource.LoomAgentStateConflict`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentStateConflict {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
}

/// The declared error `loom.resource.StaleGeneration`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleGeneration {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `generation` — `Integer`.
    pub generation: i64,
    /// `observed_generation` — `Integer`.
    pub observed_generation: i64,
}

/// LoomAgents — one row of the view `loom.resource.LoomAgents`.
///
/// Projects `loom.resource.LoomAgent` at `read_your_writes` consistency.
/// The specification fully determines every row, so its query is generated over the storage port —
/// see the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgents {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `observed_generation` — `Integer`.
    pub observed_generation: i64,
    /// `check` — `Optional<loom.resource.CheckResult>`.
    pub check: Option<CheckResult>,
    /// `deployment` — `Optional<loom.resource.DeploymentStatus>`.
    pub deployment: Option<DeploymentStatus>,
    /// `variant` — `Optional<loom.resource.AgentVariant>`.
    pub variant: Option<AgentVariant>,
    /// `conditions` — `Optional<List<loom.resource.Condition>>`.
    pub conditions: Option<Vec<Condition>>,
    /// `state` — `loom.resource.LoomAgent.State`.
    pub state: LoomAgentState,
}

/// What this bounded context owes its implementor, and the seams of what is generated.
///
/// One trait per obligation in the synthesis plan, each carrying the plan's own contract, and one
/// per generated behaviour, which [`Generated`](crate::behaviour::Generated) implements.
pub mod obligations {
    /// The behaviour `loom.resource.ApplyLoomAgent` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait ApplyLoomAgentBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.resource.ApplyLoomAgent`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn apply_loom_agent(&mut self, input: super::ApplyLoomAgent) -> Result<super::ApplyLoomAgentOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.resource.DeleteLoomAgent` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait DeleteLoomAgentBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.resource.DeleteLoomAgent`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn delete_loom_agent(&mut self, input: super::DeleteLoomAgent) -> Result<super::DeleteLoomAgentOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.resource.ObserveDeployment` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait ObserveDeploymentBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.resource.ObserveDeployment`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn observe_deployment(&mut self, input: super::ObserveDeployment) -> Result<super::ObserveDeploymentOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.resource.RecordCheck` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait RecordCheckBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.resource.RecordCheck`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn record_check(&mut self, input: super::RecordCheck) -> Result<super::RecordCheckOutcome, crate::obligation::UnmetObligation>;
    }

    /// The query `loom.resource.LoomAgents` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage port. Implement it yourself to replace that query.
    pub trait LoomAgentsQuery {
        /// Serves `loom.resource.LoomAgents` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of a row whose declared type cannot hold its value.
        fn loom_agents(&self) -> Result<Vec<super::LoomAgents>, crate::obligation::UnmetObligation>;
    }

}
