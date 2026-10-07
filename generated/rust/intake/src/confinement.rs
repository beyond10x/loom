// generated from intake v1
// model digest f7fab3ba3958266082725d66c0d412f3b6ea6a13a27a13510350f0b62657a8e3
// contract digest de8b096fd7c892c2a32f23173c969f86f7576e033c100ea6b2e2e7537e8531e1
// do not edit: regenerate with `ess synthesize --layout crate`

//! confinement — `intake.confinement`.
//!
//! Requested and observed confinement of local test commands. No automatic fallback.
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
