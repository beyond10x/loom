// generated from intake v1
// model digest 91e7b24c1442a41a622f4feaa84db250d7769d7784122bae93f2e0cda0c64c8e
// contract digest 99df8d6f24c0dac87f11d99d24d6f1ffae8788936b89c7b0d0c7979db6533769
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
    /// `workdir` — `Optional<String>`.
    pub workdir: Option<String>,
    /// `adapter` — `Optional<String>`.
    pub adapter: Option<String>,
    /// `facts` — `Optional<List<String>>`.
    pub facts: Option<Vec<String>>,
}

/// Backend — `intake.confinement.Backend`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// `Substrate`.
    Substrate,
    /// `None`.
    None,
}

/// BaselineVariable — `intake.confinement.BaselineVariable`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselineVariable {
    /// `Lang`.
    Lang,
    /// `LcAll`.
    LcAll,
    /// `Path`.
    Path,
    /// `Term`.
    Term,
    /// `Tz`.
    Tz,
}

/// ChildAperture — `intake.confinement.ChildAperture`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildAperture {
    /// `name` — `String`.
    pub name: String,
    /// `host` — `String`.
    pub host: String,
    /// `port` — `Integer`.
    pub port: i64,
    /// `pinned` — `String`.
    pub pinned: String,
    /// `max_bytes` — `Optional<Integer>`.
    pub max_bytes: Option<i64>,
}

/// ChildEnvironment — `intake.confinement.ChildEnvironment`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildEnvironment {
    /// `allow` — `List<intake.confinement.BaselineVariable>`.
    pub allow: Vec<BaselineVariable>,
    /// `set` — `List<intake.confinement.EnvironmentEntry>`.
    pub set: Vec<EnvironmentEntry>,
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

/// ChildNetwork — `intake.confinement.ChildNetwork`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildNetwork {
    /// `None`.
    None,
    /// `Aperture`.
    Aperture,
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

/// ChildProfile — `intake.confinement.ChildProfile`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildProfile {
    /// `Workspace`.
    Workspace,
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

/// ConfinementSpec — `intake.confinement.ConfinementSpec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfinementSpec {
    /// `read_only_roots` — `List<intake.confinement.ToolchainRoot>`.
    pub read_only_roots: Vec<ToolchainRoot>,
    /// `read_only_files` — `List<intake.confinement.ReadOnlyFile>`.
    pub read_only_files: Vec<ReadOnlyFile>,
    /// `host_paths` — `List<String>`.
    pub host_paths: Vec<String>,
    /// `writable` — `List<String>`.
    pub writable: Vec<String>,
    /// `writable_roots` — `List<intake.confinement.WritableRoot>`.
    pub writable_roots: Vec<WritableRoot>,
    /// `workdir` — `Optional<String>`.
    pub workdir: Option<String>,
    /// `environment` — `intake.confinement.ChildEnvironment`.
    pub environment: ChildEnvironment,
    /// `profile` — `intake.confinement.ChildProfile`.
    pub profile: ChildProfile,
    /// `network` — `intake.confinement.ChildNetwork`.
    pub network: ChildNetwork,
    /// `egress` — `List<intake.confinement.ChildAperture>`.
    pub egress: Vec<ChildAperture>,
    /// `ca_bundle` — `Optional<String>`.
    pub ca_bundle: Option<String>,
    /// `timeout_ms` — `Optional<Integer>`.
    pub timeout_ms: Option<i64>,
    /// `stdin` — `Optional<Bytes>`.
    pub stdin: Option<Vec<u8>>,
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

/// WritableRoot — `intake.confinement.WritableRoot`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritableRoot {
    /// `host_path` — `String`.
    pub host_path: String,
    /// `mount` — `String`.
    pub mount: String,
}
