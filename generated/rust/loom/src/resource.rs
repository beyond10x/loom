// generated from loom v1
// model digest a3dc4b640ce58eeb6c4d05b773be261d40478c50a85ee0b68f321ee7ebd08f08
// contract digest 52e82ecbc30fb6b42ad15e65bf8306548c049a91b152461c8290a62e4f490edb
// do not edit: regenerate with `ess synthesize --layout crate`

//! Resource — `loom.resource`.
//!
//! The `LoomAgent` custom resource (group `loom.beyond10x.dev`, version `v1alpha1`): its spec, the `loom.agent/1` document and what the pod needs beside it, and its lifecycle as the controller reconciles it: applied into Pending, Running or Refused by the spec check, Degraded and back by the child Deployment's availability, Completed or Failed when the child Job's run ends, with the program's exit code and the time it ended, or, on a schedule, Running with each finished run's end recorded in turn, Pending again on a new generation, and gone on delete.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// AdmissionRefusalReason — `loom.resource.AdmissionRefusalReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionRefusalReason {
    /// `Malformed`.
    Malformed,
    /// `SecretLiteral`.
    SecretLiteral,
    /// `UnknownFormat`.
    UnknownFormat,
    /// `BlankText`.
    BlankText,
    /// `NotAVariableName`.
    NotAVariableName,
    /// `PortOutOfRange`.
    PortOutOfRange,
    /// `NotAPlainPath`.
    NotAPlainPath,
    /// `NotAnInstant`.
    NotAnInstant,
    /// `UnknownProtocol`.
    UnknownProtocol,
    /// `UnknownAction`.
    UnknownAction,
    /// `UnboundSource`.
    UnboundSource,
    /// `HttpMethod`.
    HttpMethod,
    /// `SendWithoutWrites`.
    SendWithoutWrites,
    /// `EmptyChannelAllow`.
    EmptyChannelAllow,
    /// `NoModelRoute`.
    NoModelRoute,
    /// `TooLarge`.
    TooLarge,
    /// `BoundaryWithSecret`.
    BoundaryWithSecret,
    /// `BoundaryUpstreamInvalid`.
    BoundaryUpstreamInvalid,
    /// `BoundaryUpstreamNotAllowed`.
    BoundaryUpstreamNotAllowed,
    /// `BoundaryEgressHost`.
    BoundaryEgressHost,
    /// `BoundaryEgressNamespace`.
    BoundaryEgressNamespace,
    /// `SecretNotAllowed`.
    SecretNotAllowed,
    /// `NamespaceMismatch`.
    NamespaceMismatch,
    /// `EgressHostNotAllowed`.
    EgressHostNotAllowed,
    /// `EnvNameConflict`.
    EnvNameConflict,
    /// `ConfigFileInvalid`.
    ConfigFileInvalid,
    /// `ConfigFileConflict`.
    ConfigFileConflict,
    /// `MountPathInvalid`.
    MountPathInvalid,
    /// `ClaimNotAllowed`.
    ClaimNotAllowed,
    /// `ProfileInvalid`.
    ProfileInvalid,
    /// `ProfileNotAllowed`.
    ProfileNotAllowed,
    /// `ResourcesRequired`.
    ResourcesRequired,
}

/// AgentVariant — `loom.resource.AgentVariant`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentVariant {
    /// `AgentServe`.
    AgentServe,
    /// `Command`.
    Command,
}

/// Boundary — `loom.resource.Boundary`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boundary {
    /// `model` — `Optional<loom.resource.BoundaryUpstream>`.
    pub model: Option<BoundaryUpstream>,
    /// `connectors` — `Optional<loom.resource.ConnectorsBoundary>`.
    pub connectors: Option<ConnectorsBoundary>,
    /// `git` — `Optional<loom.resource.GitBoundary>`.
    pub git: Option<GitBoundary>,
}

/// BoundaryUpstream — `loom.resource.BoundaryUpstream`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryUpstream {
    /// `upstream` — `String`.
    pub upstream: String,
    /// `audience` — `String`.
    pub audience: String,
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
    /// `Completed`.
    Completed,
    /// `LastRunSucceeded`.
    LastRunSucceeded,
}

/// ConfigFile — `loom.resource.ConfigFile`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigFile {
    /// `name` — `String`.
    pub name: String,
    /// `content` — `String`.
    pub content: String,
}

/// ConfigMapKeyRef — `loom.resource.ConfigMapKeyRef`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigMapKeyRef {
    /// `name` — `String`.
    pub name: String,
    /// `key` — `String`.
    pub key: String,
}

/// ConfinementProfile — `loom.resource.ConfinementProfile`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfinementProfile {
    /// `type` — `loom.resource.ConfinementProfileKind`.
    pub r#type: ConfinementProfileKind,
    /// `localhost_profile` — `Optional<String>`.
    pub localhost_profile: Option<String>,
}

/// ConfinementProfileKind — `loom.resource.ConfinementProfileKind`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfinementProfileKind {
    /// `RuntimeDefault`.
    RuntimeDefault,
    /// `Localhost`.
    Localhost,
    /// `Unconfined`.
    Unconfined,
}

/// ConnectorsBoundary — `loom.resource.ConnectorsBoundary`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorsBoundary {
    /// `upstream` — `String`.
    pub upstream: String,
    /// `audience` — `String`.
    pub audience: String,
    /// `operations` — `List<String>`.
    pub operations: Vec<String>,
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

/// EnvVar — `loom.resource.EnvVar`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvVar {
    /// `name` — `String`.
    pub name: String,
    /// `value` — `String`.
    pub value: String,
}

/// GitBoundary — `loom.resource.GitBoundary`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBoundary {
    /// `upstream` — `String`.
    pub upstream: String,
    /// `audience` — `String`.
    pub audience: String,
    /// `repositories` — `List<String>`.
    pub repositories: Vec<String>,
}

/// ImageRef — `loom.resource.ImageRef`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRef {
    /// `repository` — `String`.
    pub repository: String,
    /// `tag` — `String`.
    pub tag: String,
}

/// LastRun — `loom.resource.LastRun`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastRun {
    /// `phase` — `loom.resource.RunPhase`.
    pub phase: RunPhase,
    /// `exit_code` — `Optional<Integer>`.
    pub exit_code: Option<i64>,
    /// `finished_at` — `Optional<Timestamp>`.
    pub finished_at: Option<crate::primitives::Timestamp>,
    /// `reason` — `Optional<String>`.
    pub reason: Option<String>,
}

/// The states of `loom.resource.LoomAgent`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `LoomAgent<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoomAgentState {
    /// `Completed`.
    Completed,
    /// `Degraded`.
    Degraded,
    /// `Failed`.
    Failed,
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
    /// `agent` — `Optional<loom.agent.AgentSpec>`.
    pub agent: Option<crate::agent::AgentSpec>,
    /// `routes` — `Json`.
    pub routes: crate::json::Value,
    /// `connectors_config` — `Optional<String>`.
    pub connectors_config: Option<String>,
    /// `secret_ref` — `Optional<loom.resource.SecretRef>`.
    pub secret_ref: Option<SecretRef>,
    /// `secret_files` — `Optional<loom.resource.SecretFiles>`.
    pub secret_files: Option<SecretFiles>,
    /// `env` — `Optional<List<loom.resource.EnvVar>>`.
    pub env: Option<Vec<EnvVar>>,
    /// `image` — `Optional<loom.resource.ImageRef>`.
    pub image: Option<ImageRef>,
    /// `command` — `Optional<loom.resource.CommandSpec>`.
    pub command: Option<CommandSpec>,
    /// `workspace` — `Optional<loom.resource.Workspace>`.
    pub workspace: Option<Workspace>,
    /// `config_files` — `Optional<List<loom.resource.ConfigFile>>`.
    pub config_files: Option<Vec<ConfigFile>>,
    /// `seccomp_profile` — `Optional<loom.resource.ConfinementProfile>`.
    pub seccomp_profile: Option<ConfinementProfile>,
    /// `app_armor_profile` — `Optional<loom.resource.ConfinementProfile>`.
    pub app_armor_profile: Option<ConfinementProfile>,
    /// `boundary` — `Optional<loom.resource.Boundary>`.
    pub boundary: Option<Boundary>,
    /// `schedule` — `Optional<loom.resource.ScheduleSpec>`.
    pub schedule: Option<ScheduleSpec>,
}

/// ResourceName — `loom.resource.ResourceName`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceName(pub String);

/// RunEnd — `loom.resource.RunEnd`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEnd {
    /// `exit_code` — `Optional<Integer>`.
    pub exit_code: Option<i64>,
    /// `finished_at` — `Optional<Timestamp>`.
    pub finished_at: Option<crate::primitives::Timestamp>,
    /// `reason` — `Optional<String>`.
    pub reason: Option<String>,
}

/// RunPhase — `loom.resource.RunPhase`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunPhase {
    /// `Active`.
    Active,
    /// `Succeeded`.
    Succeeded,
    /// `Failed`.
    Failed,
}

/// ScheduleInput — `loom.resource.ScheduleInput`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleInput {
    /// `text` — `Optional<String>`.
    pub text: Option<String>,
    /// `config_map_key_ref` — `Optional<loom.resource.ConfigMapKeyRef>`.
    pub config_map_key_ref: Option<ConfigMapKeyRef>,
}

/// ScheduleSpec — `loom.resource.ScheduleSpec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleSpec {
    /// `cron` — `String`.
    pub cron: String,
    /// `time_zone` — `Optional<String>`.
    pub time_zone: Option<String>,
    /// `suspend` — `Optional<Boolean>`.
    pub suspend: Option<bool>,
    /// `starting_deadline_seconds` — `Optional<Integer>`.
    pub starting_deadline_seconds: Option<i64>,
    /// `run_deadline_seconds` — `Optional<Integer>`.
    pub run_deadline_seconds: Option<i64>,
    /// `input` — `Optional<loom.resource.ScheduleInput>`.
    pub input: Option<ScheduleInput>,
}

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

/// StageDestination — `loom.resource.StageDestination`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageDestination {
    /// `Absent`.
    Absent,
    /// `Directory`.
    Directory,
    /// `Link`.
    Link,
    /// `Other`.
    Other,
}

/// StageRefusalReason — `loom.resource.StageRefusalReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageRefusalReason {
    /// `DestinationIsALink`.
    DestinationIsALink,
    /// `DestinationNotADirectory`.
    DestinationNotADirectory,
    /// `NameNotUtf8`.
    NameNotUtf8,
    /// `KeyOutsideVolume`.
    KeyOutsideVolume,
    /// `StaleDirectory`.
    StaleDirectory,
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
    /// `run` — `Optional<loom.resource.RunEnd>`.
    pub run: Option<RunEnd>,
    /// `last_run` — `Optional<loom.resource.LastRun>`.
    pub last_run: Option<LastRun>,
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
        impl Sealed for super::Completed {}
        impl Sealed for super::Degraded {}
        impl Sealed for super::Failed {}
        impl Sealed for super::Pending {}
        impl Sealed for super::Refused {}
        impl Sealed for super::Running {}
    }

    /// A declared state of `LoomAgent`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::LoomAgentState;
    }

    /// `Completed`.
    pub struct Completed;

    impl Marker for Completed {
        const STATE: super::LoomAgentState = super::LoomAgentState::Completed;
    }

    /// `Degraded`.
    pub struct Degraded;

    impl Marker for Degraded {
        const STATE: super::LoomAgentState = super::LoomAgentState::Degraded;
    }

    /// `Failed`.
    pub struct Failed;

    impl Marker for Failed {
        const STATE: super::LoomAgentState = super::LoomAgentState::Failed;
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

impl LoomAgent<loom_agent_state::Completed> {
    /// `respecify` — `Completed` → `Pending`. Taken by the `respecified` outcome of `loom.resource.ApplyLoomAgent`.
    pub fn respecify(self) -> LoomAgent<loom_agent_state::Pending> {
        LoomAgent {
            data: self.data,
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

impl LoomAgent<loom_agent_state::Failed> {
    /// `respecify` — `Failed` → `Pending`. Taken by the `respecified` outcome of `loom.resource.ApplyLoomAgent`.
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

    /// `observe-running` — `Running` → `Running`. Taken by the `still-running` outcome of `loom.resource.ObserveDeployment`, the `still-active` outcome of `loom.resource.ObserveRun`, the `last-run-succeeded` outcome of `loom.resource.ObserveScheduledRun`, the `last-run-failed` outcome of `loom.resource.ObserveScheduledRun`.
    pub fn observe_running(self) -> LoomAgent<loom_agent_state::Running> {
        LoomAgent {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `complete` — `Running` → `Completed`. Taken by the `completed` outcome of `loom.resource.ObserveRun`.
    pub fn complete(self) -> LoomAgent<loom_agent_state::Completed> {
        LoomAgent {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `fail` — `Running` → `Failed`. Taken by the `failed` outcome of `loom.resource.ObserveRun`.
    pub fn fail(self) -> LoomAgent<loom_agent_state::Failed> {
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
    /// Resting in `Completed`.
    Completed(LoomAgent<loom_agent_state::Completed>),
    /// Resting in `Degraded`.
    Degraded(LoomAgent<loom_agent_state::Degraded>),
    /// Resting in `Failed`.
    Failed(LoomAgent<loom_agent_state::Failed>),
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
            LoomAgentState::Completed => AnyLoomAgent::Completed(LoomAgent {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            LoomAgentState::Degraded => AnyLoomAgent::Degraded(LoomAgent {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            LoomAgentState::Failed => AnyLoomAgent::Failed(LoomAgent {
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
            Self::Completed(_) => LoomAgentState::Completed,
            Self::Degraded(_) => LoomAgentState::Degraded,
            Self::Failed(_) => LoomAgentState::Failed,
            Self::Pending(_) => LoomAgentState::Pending,
            Self::Refused(_) => LoomAgentState::Refused,
            Self::Running(_) => LoomAgentState::Running,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> LoomAgentSnapshot {
        match self {
            Self::Completed(instance) => LoomAgentSnapshot {
                state: LoomAgentState::Completed,
                data: instance.into_data(),
            },
            Self::Degraded(instance) => LoomAgentSnapshot {
                state: LoomAgentState::Degraded,
                data: instance.into_data(),
            },
            Self::Failed(instance) => LoomAgentSnapshot {
                state: LoomAgentState::Failed,
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

/// AdmitLoomAgent — the input of `loom.resource.AdmitLoomAgent`.
///
/// Everything it can result in is [`AdmitLoomAgentOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmitLoomAgent {
    /// `path` — `String`.
    pub path: String,
}

/// Everything `loom.resource.AdmitLoomAgent` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmitLoomAgentOutcome {
    /// `malformed` — externally decided (a member of the resource does not decode, or breaks a rule no other reason names).
    Malformed {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `secret-literal` — externally decided (a member of the resource holds a credential).
    SecretLiteral {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `unknown-format` — externally decided (spec.agent's format is not loom.agent/1).
    UnknownFormat {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `blank-text` — externally decided (a text member of spec.agent is empty or only whitespace).
    BlankText {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `not-a-variable-name` — externally decided (a bearer_env of spec.agent, or a spec.env name, is not an environment variable name).
    NotAVariableName {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `port-out-of-range` — externally decided (a port of spec.agent is outside 1 to 65535).
    PortOutOfRange {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `not-a-plain-path` — externally decided (an allow_paths entry of an http binding of spec.agent is not a plain path).
    NotAPlainPath {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `not-an-instant` — externally decided (a Timestamp member of spec.agent is not an RFC 3339 instant).
    NotAnInstant {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `unknown-protocol` — externally decided (the inner loop of an AgentServe resource's spec.agent names a protocol the plugin catalog does not hold).
    UnknownProtocol {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `unknown-action` — externally decided (a state of an AgentServe resource's spec.agent names an action its protocol does not declare).
    UnknownAction {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `unbound-source` — externally decided (a state, the outer loop or the channel of an AgentServe resource's spec.agent names a source no binding declares).
    UnboundSource {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `http-method` — externally decided (an http binding of an AgentServe resource's spec.agent has a method other than GET).
    HttpMethod {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `send-without-writes` — externally decided (an AgentServe resource's spec.agent has reply.send among its capabilities while writes are none or undeclared).
    SendWithoutWrites {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `empty-channel-allow` — externally decided (the channel of an AgentServe resource's spec.agent allows no channel id).
    EmptyChannelAllow {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `no-model-route` — externally decided (spec.routes is empty and the pod would reach no model).
    NoModelRoute {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `too-large` — externally decided (a member is beyond what the credential walk reads, or the ConfigMap beyond 1 MiB).
    TooLarge {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `boundary-with-secret` — externally decided (spec.boundary stands beside spec.secretRef or spec.secretFiles).
    BoundaryWithSecret {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `boundary-upstream-invalid` — externally decided (a boundary upstream is no plain http URL of a cluster service).
    BoundaryUpstreamInvalid {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `boundary-upstream-not-allowed` — externally decided (no allowedBoundaryUpstreams entry gives a boundary upstream's kind, URL and audience exactly).
    BoundaryUpstreamNotAllowed {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `boundary-egress-host` — externally decided (beside spec.boundary, an egress entry of spec.agent names a host).
    BoundaryEgressHost {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `boundary-egress-namespace` — externally decided (beside spec.boundary, an egress entry of spec.agent names a namespace).
    BoundaryEgressNamespace {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `secret-not-allowed` — externally decided (spec.secretRef or spec.secretFiles names a Secret outside allowedSecretNames).
    SecretNotAllowed {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `namespace-mismatch` — externally decided (spec.agent's deploy.kubernetes.namespace is not the resource's namespace).
    NamespaceMismatch {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `egress-host-not-allowed` — externally decided (an egress entry of spec.agent names no destination, or a host outside allowedEgressHosts).
    EgressHostNotAllowed {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `env-name-conflict` — externally decided (a spec.env name is one the controller sets, or one an earlier entry gives).
    EnvNameConflict {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `config-file-invalid` — externally decided (a spec.configFiles name is no plain file name).
    ConfigFileInvalid {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `config-file-conflict` — externally decided (a spec.configFiles name is one an earlier entry gives, or one the ConfigMap already holds).
    ConfigFileConflict {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `mount-path-invalid` — externally decided (a workspace or Secret mount path is not absolute and plain, or overlaps a mount the controller makes or the other one).
    MountPathInvalid {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `claim-not-allowed` — externally decided (spec.workspace names a claim outside allowedClaimNames).
    ClaimNotAllowed {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `profile-invalid` — externally decided (a seccomp or AppArmor profile is out of form).
    ProfileInvalid {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `profile-not-allowed` — externally decided (a seccomp or AppArmor profile is Unconfined or Localhost without the configuration allowing it).
    ProfileNotAllowed {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `resources-required` — externally decided (a Command resource declares no size and the configuration gives no defaultCommandResources).
    ResourcesRequired {
        /// Why it was refused: `loom.resource.AdmissionRefused`.
        error: AdmissionRefused,
    },
    /// `admitted` — otherwise.
    Admitted,
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

/// ObserveRun — the input of `loom.resource.ObserveRun`.
///
/// Everything it can result in is [`ObserveRunOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserveRun {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `phase` — `loom.resource.RunPhase`.
    pub phase: RunPhase,
    /// `exit_code` — `Optional<Integer>`.
    pub exit_code: Option<i64>,
    /// `finished_at` — `Optional<Timestamp>`.
    pub finished_at: Option<crate::primitives::Timestamp>,
    /// `reason` — `Optional<String>`.
    pub reason: Option<String>,
}

/// Everything `loom.resource.ObserveRun` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveRunOutcome {
    /// `unknown` — for an identity no record carries.
    Unknown {
        /// Why it was refused: `loom.resource.LoomAgentNotFound`.
        error: LoomAgentNotFound,
    },
    /// `still-active` — when the existing subject is in Running and `phase == Active`.
    StillActive {
        /// The `loom.resource.LoomAgentRunObserved` this outcome publishes.
        loom_agent_run_observed: LoomAgentRunObserved,
    },
    /// `completed` — when the existing subject is in Running and `phase == Succeeded`.
    Completed {
        /// The `loom.resource.LoomAgentCompleted` this outcome publishes.
        loom_agent_completed: LoomAgentCompleted,
    },
    /// `failed` — when the existing subject is in Running and `phase == Failed`.
    Failed {
        /// The `loom.resource.LoomAgentFailed` this outcome publishes.
        loom_agent_failed: LoomAgentFailed,
    },
    /// `finished` — when the existing subject is in Completed or Failed.
    Finished,
    /// `no-run` — otherwise.
    NoRun {
        /// Why it was refused: `loom.resource.LoomAgentStateConflict`.
        error: LoomAgentStateConflict,
    },
}

/// ObserveScheduledRun — the input of `loom.resource.ObserveScheduledRun`.
///
/// Everything it can result in is [`ObserveScheduledRunOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserveScheduledRun {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `phase` — `loom.resource.RunPhase`.
    pub phase: RunPhase,
    /// `exit_code` — `Optional<Integer>`.
    pub exit_code: Option<i64>,
    /// `finished_at` — `Optional<Timestamp>`.
    pub finished_at: Option<crate::primitives::Timestamp>,
    /// `reason` — `Optional<String>`.
    pub reason: Option<String>,
}

/// Everything `loom.resource.ObserveScheduledRun` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveScheduledRunOutcome {
    /// `unknown` — for an identity no record carries.
    Unknown {
        /// Why it was refused: `loom.resource.LoomAgentNotFound`.
        error: LoomAgentNotFound,
    },
    /// `last-run-succeeded` — when the existing subject is in Running and `phase == Succeeded`.
    LastRunSucceeded {
        /// The `loom.resource.LoomAgentLastRunObserved` this outcome publishes.
        loom_agent_last_run_observed: LoomAgentLastRunObserved,
    },
    /// `last-run-failed` — when the existing subject is in Running and `phase == Failed`.
    LastRunFailed {
        /// The `loom.resource.LoomAgentLastRunObserved` this outcome publishes.
        loom_agent_last_run_observed: LoomAgentLastRunObserved,
    },
    /// `run-active` — when the existing subject is in Running and `phase == Active`.
    RunActive,
    /// `not-running` — otherwise.
    NotRunning {
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

/// StageFiles — the input of `loom.resource.StageFiles`.
///
/// Everything it can result in is [`StageFilesOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageFiles {
    /// `destination` — `loom.resource.StageDestination`.
    pub destination: StageDestination,
    /// `names_utf8` — `Boolean`.
    pub names_utf8: bool,
    /// `keys_inside` — `Boolean`.
    pub keys_inside: bool,
    /// `stale_directory` — `Boolean`.
    pub stale_directory: bool,
}

/// Actual typed response of `loom.resource.StageFiles`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageFilesResponse {
    /// `directory_mode` — `Integer`.
    pub directory_mode: i64,
    /// `file_mode` — `Integer`.
    pub file_mode: i64,
}

/// Everything `loom.resource.StageFiles` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageFilesOutcome {
    /// `destination-is-a-link` — when `destination == Link`.
    DestinationIsALink {
        /// Why it was refused: `loom.resource.StageRefused`.
        error: StageRefused,
    },
    /// `destination-not-a-directory` — when `destination == Other`.
    DestinationNotADirectory {
        /// Why it was refused: `loom.resource.StageRefused`.
        error: StageRefused,
    },
    /// `name-not-utf8` — when `names_utf8 == false`.
    NameNotUtf8 {
        /// Why it was refused: `loom.resource.StageRefused`.
        error: StageRefused,
    },
    /// `key-outside-volume` — when `keys_inside == false`.
    KeyOutsideVolume {
        /// Why it was refused: `loom.resource.StageRefused`.
        error: StageRefused,
    },
    /// `stale-directory` — when `stale_directory == true`.
    StaleDirectory {
        /// Why it was refused: `loom.resource.StageRefused`.
        error: StageRefused,
    },
    /// `staged` — otherwise.
    Staged {
        /// Actual response returned by this branch.
        response: StageFilesResponse,
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

/// LoomAgentCompleted — the event `loom.resource.LoomAgentCompleted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentCompleted {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `generation` — `Integer`.
    pub generation: i64,
    /// `exit_code` — `Optional<Integer>`.
    pub exit_code: Option<i64>,
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

/// LoomAgentFailed — the event `loom.resource.LoomAgentFailed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentFailed {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `generation` — `Integer`.
    pub generation: i64,
    /// `exit_code` — `Optional<Integer>`.
    pub exit_code: Option<i64>,
}

/// LoomAgentLastRunObserved — the event `loom.resource.LoomAgentLastRunObserved`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentLastRunObserved {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `generation` — `Integer`.
    pub generation: i64,
    /// `phase` — `loom.resource.RunPhase`.
    pub phase: RunPhase,
    /// `exit_code` — `Optional<Integer>`.
    pub exit_code: Option<i64>,
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

/// LoomAgentRunObserved — the event `loom.resource.LoomAgentRunObserved`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentRunObserved {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `generation` — `Integer`.
    pub generation: i64,
}

/// LoomAgentRunning — the event `loom.resource.LoomAgentRunning`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomAgentRunning {
    /// `name` — `loom.resource.ResourceName`.
    pub name: ResourceName,
    /// `generation` — `Integer`.
    pub generation: i64,
}

/// The declared error `loom.resource.AdmissionRefused`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionRefused {
    /// `reason` — `loom.resource.AdmissionRefusalReason`.
    pub reason: AdmissionRefusalReason,
    /// `member` — `String`.
    pub member: String,
    /// `detail` — `String`.
    pub detail: String,
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

/// The declared error `loom.resource.StageRefused`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageRefused {
    /// `reason` — `loom.resource.StageRefusalReason`.
    pub reason: StageRefusalReason,
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
    /// `run` — `Optional<loom.resource.RunEnd>`.
    pub run: Option<RunEnd>,
    /// `last_run` — `Optional<loom.resource.LastRun>`.
    pub last_run: Option<LastRun>,
    /// `conditions` — `Optional<List<loom.resource.Condition>>`.
    pub conditions: Option<Vec<Condition>>,
    /// `state` — `loom.resource.LoomAgent.State`.
    pub state: LoomAgentState,
}

/// What this bounded context owes its implementor, and the seams of what is generated.
///
/// One trait per obligation in the synthesis plan, each carrying the plan's own contract, and one
/// per generated behaviour, which [`Generated`](crate::behaviour::Generated) implements.
/// [`Unimplemented`](obligations::Unimplemented) satisfies every owed trait by refusing in the type system.
pub mod obligations {
    /// The behaviour `loom.resource.AdmitLoomAgent` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by the fields of error `loom.resource.AdmissionRefused`, which the specification gives no source, in `malformed`.
    ///
    /// Contract: given `loom.resource.AdmitLoomAgent` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `malformed` externally decided (a member of the resource does not decode, or breaks a rule no other reason names), error `loom.resource.AdmissionRefused`; `secret-literal` externally decided (a member of the resource holds a credential), error `loom.resource.AdmissionRefused`; `unknown-format` externally decided (spec.agent's format is not loom.agent/1), error `loom.resource.AdmissionRefused`; `blank-text` externally decided (a text member of spec.agent is empty or only whitespace), error `loom.resource.AdmissionRefused`; `not-a-variable-name` externally decided (a bearer_env of spec.agent, or a spec.env name, is not an environment variable name), error `loom.resource.AdmissionRefused`; `port-out-of-range` externally decided (a port of spec.agent is outside 1 to 65535), error `loom.resource.AdmissionRefused`; `not-a-plain-path` externally decided (an allow_paths entry of an http binding of spec.agent is not a plain path), error `loom.resource.AdmissionRefused`; `not-an-instant` externally decided (a Timestamp member of spec.agent is not an RFC 3339 instant), error `loom.resource.AdmissionRefused`; `unknown-protocol` externally decided (the inner loop of an AgentServe resource's spec.agent names a protocol the plugin catalog does not hold), error `loom.resource.AdmissionRefused`; `unknown-action` externally decided (a state of an AgentServe resource's spec.agent names an action its protocol does not declare), error `loom.resource.AdmissionRefused`; `unbound-source` externally decided (a state, the outer loop or the channel of an AgentServe resource's spec.agent names a source no binding declares), error `loom.resource.AdmissionRefused`; `http-method` externally decided (an http binding of an AgentServe resource's spec.agent has a method other than GET), error `loom.resource.AdmissionRefused`; `send-without-writes` externally decided (an AgentServe resource's spec.agent has reply.send among its capabilities while writes are none or undeclared), error `loom.resource.AdmissionRefused`; `empty-channel-allow` externally decided (the channel of an AgentServe resource's spec.agent allows no channel id), error `loom.resource.AdmissionRefused`; `no-model-route` externally decided (spec.routes is empty and the pod would reach no model), error `loom.resource.AdmissionRefused`; `too-large` externally decided (a member is beyond what the credential walk reads, or the ConfigMap beyond 1 MiB), error `loom.resource.AdmissionRefused`; `boundary-with-secret` externally decided (spec.boundary stands beside spec.secretRef or spec.secretFiles), error `loom.resource.AdmissionRefused`; `boundary-upstream-invalid` externally decided (a boundary upstream is no plain http URL of a cluster service), error `loom.resource.AdmissionRefused`; `boundary-upstream-not-allowed` externally decided (no allowedBoundaryUpstreams entry gives a boundary upstream's kind, URL and audience exactly), error `loom.resource.AdmissionRefused`; `boundary-egress-host` externally decided (beside spec.boundary, an egress entry of spec.agent names a host), error `loom.resource.AdmissionRefused`; `boundary-egress-namespace` externally decided (beside spec.boundary, an egress entry of spec.agent names a namespace), error `loom.resource.AdmissionRefused`; `secret-not-allowed` externally decided (spec.secretRef or spec.secretFiles names a Secret outside allowedSecretNames), error `loom.resource.AdmissionRefused`; `namespace-mismatch` externally decided (spec.agent's deploy.kubernetes.namespace is not the resource's namespace), error `loom.resource.AdmissionRefused`; `egress-host-not-allowed` externally decided (an egress entry of spec.agent names no destination, or a host outside allowedEgressHosts), error `loom.resource.AdmissionRefused`; `env-name-conflict` externally decided (a spec.env name is one the controller sets, or one an earlier entry gives), error `loom.resource.AdmissionRefused`; `config-file-invalid` externally decided (a spec.configFiles name is no plain file name), error `loom.resource.AdmissionRefused`; `config-file-conflict` externally decided (a spec.configFiles name is one an earlier entry gives, or one the ConfigMap already holds), error `loom.resource.AdmissionRefused`; `mount-path-invalid` externally decided (a workspace or Secret mount path is not absolute and plain, or overlaps a mount the controller makes or the other one), error `loom.resource.AdmissionRefused`; `claim-not-allowed` externally decided (spec.workspace names a claim outside allowedClaimNames), error `loom.resource.AdmissionRefused`; `profile-invalid` externally decided (a seccomp or AppArmor profile is out of form), error `loom.resource.AdmissionRefused`; `profile-not-allowed` externally decided (a seccomp or AppArmor profile is Unconfined or Localhost without the configuration allowing it), error `loom.resource.AdmissionRefused`; `resources-required` externally decided (a Command resource declares no size and the configuration gives no defaultCommandResources), error `loom.resource.AdmissionRefused`; `admitted` otherwise.
    pub trait AdmitLoomAgentBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.resource.AdmitLoomAgent`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn admit_loom_agent(&mut self, input: super::AdmitLoomAgent) -> Result<super::AdmitLoomAgentOutcome, crate::obligation::UnmetObligation>;
    }

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

    /// The behaviour `loom.resource.ObserveRun` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait ObserveRunBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.resource.ObserveRun`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn observe_run(&mut self, input: super::ObserveRun) -> Result<super::ObserveRunOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.resource.ObserveScheduledRun` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait ObserveScheduledRunBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.resource.ObserveScheduledRun`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn observe_scheduled_run(&mut self, input: super::ObserveScheduledRun) -> Result<super::ObserveScheduledRunOutcome, crate::obligation::UnmetObligation>;
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

    /// The behaviour `loom.resource.StageFiles` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by a typed response (`response:`).
    ///
    /// Contract: given `loom.resource.StageFiles` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `destination-is-a-link` when `destination == Link`, error `loom.resource.StageRefused`; `destination-not-a-directory` when `destination == Other`, error `loom.resource.StageRefused`; `name-not-utf8` when `names_utf8 == false`, error `loom.resource.StageRefused`; `key-outside-volume` when `keys_inside == false`, error `loom.resource.StageRefused`; `stale-directory` when `stale_directory == true`, error `loom.resource.StageRefused`; `staged` otherwise.
    pub trait StageFilesBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.resource.StageFiles`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn stage_files(&mut self, input: super::StageFiles) -> Result<super::StageFilesOutcome, crate::obligation::UnmetObligation>;
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

    /// Every obligation of this bounded context, refused in the type system.
    ///
    /// Each method returns the typed refusal naming what is owed — never a panic, never a guessed
    /// value — so a workspace built on this stub compiles and reports its own gaps.
    pub struct Unimplemented;

    impl AdmitLoomAgentBehavior for Unimplemented {
        fn admit_loom_agent(&mut self, _input: super::AdmitLoomAgent) -> Result<super::AdmitLoomAgentOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "loom.resource.AdmitLoomAgent" })
        }
    }

    impl StageFilesBehavior for Unimplemented {
        fn stage_files(&mut self, _input: super::StageFiles) -> Result<super::StageFilesOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "loom.resource.StageFiles" })
        }
    }
}
