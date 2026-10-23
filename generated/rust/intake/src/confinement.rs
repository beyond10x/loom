// generated from intake v1
// model digest f836d6f3248bbf56362084e46b16df9b4352fcdfa0941299cd8dda192f1dc47c
// contract digest 0890560a4a02cf92eeee366965dd042fd7116dfad2f879b638eb8d98795f55c5
// do not edit: regenerate with `ess synthesize --layout crate`

//! confinement — `intake.confinement`.
//!
//! Requested and observed confinement of local test commands. No automatic fallback. A command bound to a protocol's action runs outside Substrate, so a run that would start one is refused CommandUnconfined unless the operator opts out of confinement for those commands explicitly.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// AppliedAperture — `intake.confinement.AppliedAperture`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedAperture {
    /// `name` — `String`.
    pub name: String,
    /// `destination` — `String`.
    pub destination: String,
    /// `listen` — `Optional<String>`.
    pub listen: Option<String>,
}

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
    /// `network` — `Optional<List<intake.confinement.AppliedAperture>>`.
    pub network: Option<Vec<AppliedAperture>>,
    /// `profile` — `String`.
    pub profile: String,
    /// `writable_scopes` — `List<String>`.
    pub writable_scopes: Vec<String>,
    /// `read_only_roots` — `List<intake.confinement.ToolchainRoot>`.
    pub read_only_roots: Vec<ToolchainRoot>,
    /// `read_only_files` — `List<intake.confinement.ReadOnlyFile>`.
    pub read_only_files: Vec<ReadOnlyFile>,
}

/// Backend — `intake.confinement.Backend`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// `Substrate`.
    Substrate,
    /// `None`.
    None,
}

/// ChildExit — `intake.confinement.ChildExit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildExit {
    /// `exit_code` — `Optional<Integer>`.
    pub exit_code: Option<i64>,
    /// `signal` — `Optional<Integer>`.
    pub signal: Option<i64>,
    /// `timed_out` — `Boolean`.
    pub timed_out: bool,
}

/// ChildOutput — `intake.confinement.ChildOutput`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildOutput {
    /// `stdout` — `Bytes`.
    pub stdout: Vec<u8>,
    /// `stderr` — `Bytes`.
    pub stderr: Vec<u8>,
    /// `stdout_truncated` — `Boolean`.
    pub stdout_truncated: bool,
    /// `stderr_truncated` — `Boolean`.
    pub stderr_truncated: bool,
}

/// ChildStream — `intake.confinement.ChildStream`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildStream {
    /// `Stdout`.
    Stdout,
    /// `Stderr`.
    Stderr,
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

/// ReadOnlyFile — `intake.confinement.ReadOnlyFile`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOnlyFile {
    /// `host_path` — `String`.
    pub host_path: String,
    /// `mount` — `String`.
    pub mount: String,
}

/// ToolchainRoot — `intake.confinement.ToolchainRoot`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainRoot {
    /// `host_path` — `String`.
    pub host_path: String,
    /// `mount` — `String`.
    pub mount: String,
}
