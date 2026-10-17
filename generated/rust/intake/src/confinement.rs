// generated from intake v1
// model digest 9a78674226df1f0b3a560f6466640d34e4a20f9119e94bacebf92038386e1008
// contract digest ca82ce105dc72f6a20d493e23751dedc843123e85fab295e340813c472b7828e
// do not edit: regenerate with `ess synthesize --layout crate`

//! confinement — `intake.confinement`.
//!
//! Requested and observed confinement of local test commands. No automatic fallback. A command bound to a protocol's action runs outside Substrate, so a run that would start one is refused CommandUnconfined unless the operator opts out of confinement for those commands explicitly.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// AppliedConfinement — `intake.confinement.AppliedConfinement`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedConfinement {
    /// `backend` — `intake.confinement.Backend`.
    pub backend: Backend,
    /// `profile_digest` — `String`.
    pub profile_digest: String,
    /// `capability_snapshot` — `String`.
    pub capability_snapshot: String,
    /// `cgroup` — `String`.
    pub cgroup: String,
    /// `filesystem` — `String`.
    pub filesystem: String,
    /// `network` — `String`.
    pub network: String,
    /// `profile` — `String`.
    pub profile: String,
    /// `writable_scopes` — `List<String>`.
    pub writable_scopes: Vec<String>,
    /// `read_only_roots` — `List<intake.confinement.ToolchainRoot>`.
    pub read_only_roots: Vec<ToolchainRoot>,
}

/// Backend — `intake.confinement.Backend`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// `Substrate`.
    Substrate,
    /// `None`.
    None,
}

/// ConfinementProfile — `intake.confinement.ConfinementProfile`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfinementProfile {
    /// `backend` — `intake.confinement.Backend`.
    pub backend: Backend,
    /// `network` — `String`.
    pub network: String,
    /// `writable_scopes` — `List<String>`.
    pub writable_scopes: Vec<String>,
    /// `read_only_roots` — `List<intake.confinement.ToolchainRoot>`.
    pub read_only_roots: Vec<ToolchainRoot>,
    /// `environment` — `List<intake.confinement.EnvironmentEntry>`.
    pub environment: Vec<EnvironmentEntry>,
    /// `timeout_ms` — `Integer`.
    pub timeout_ms: i64,
    /// `memory_bytes` — `Integer`.
    pub memory_bytes: i64,
    /// `processes` — `Integer`.
    pub processes: i64,
    /// `cpu_millis` — `Integer`.
    pub cpu_millis: i64,
}

/// ConfinementRefusal — `intake.confinement.ConfinementRefusal`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfinementRefusal {
    /// `BackendMissing`.
    BackendMissing,
    /// `CgroupUndelegated`.
    CgroupUndelegated,
    /// `ScopeInvalid`.
    ScopeInvalid,
    /// `WorkspaceNameInvalid`.
    WorkspaceNameInvalid,
    /// `CapabilityUnserved`.
    CapabilityUnserved,
    /// `CommandUnconfined`.
    CommandUnconfined,
}

/// EnvironmentEntry — `intake.confinement.EnvironmentEntry`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentEntry {
    /// `name` — `String`.
    pub name: String,
    /// `value` — `String`.
    pub value: String,
}

/// ToolchainRoot — `intake.confinement.ToolchainRoot`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainRoot {
    /// `host_path` — `String`.
    pub host_path: String,
    /// `mount` — `String`.
    pub mount: String,
}
