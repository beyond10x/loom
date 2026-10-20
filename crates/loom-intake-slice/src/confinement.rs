//! Confined local tests, and confined children a host names. The embedded open/adopt/exec path is
//! ported from Harness. Substrate owns isolation; Loom declares a closed Rust/system-tool profile
//! for a test, the host's own environment and profile for a child ([`ChildCommand`]), and records
//! the driver's actual observation. Host dependency fetching is never implicit.

use std::collections::BTreeMap;
use std::fmt;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::executor::TestCommand;
pub use intake_model::confinement::{
    AppliedConfinement, Backend, ConfinementProfile, ConfinementRefusal, EnvironmentEntry,
    ToolchainRoot,
};
use sha2::{Digest as _, Sha256};
use substrate_host::{DispatchOutcome, Driver as _, HostConfig, HostDriver};
/// Substrate's own wire types a host names a [`ChildCommand`] with.
pub use substrate_wire::{BaselineEnvironment, ExecEnvironment, NetworkMode, SandboxProfile};
use substrate_wire::{
    ConfinementRequest, ExecLimits, ExecStartInput, ReadOnlyRoot, WorkspaceAccess,
};

static NEXT_EXEC: AtomicU64 = AtomicU64::new(0);
const WRITABLE: &str = "target";
const RUST_MOUNT: &str = "/toolchain/rust";
const CACHE_MOUNT: &str = "/toolchain/cargo-cache";
// Four roots are admitted by the released driver: compiler, registry and two Git caches.
const CACHE_DIRS: [&str; 3] = ["registry", "git/db", "git/checkouts"];

/// The trusted test execution port. Implementations supply observed isolation, not promises.
pub trait TestRunner: Send + Sync {
    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError>;
    fn backend(&self) -> Backend;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestExecution {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub output: String,
    pub applied: AppliedConfinement,
    pub driver_record: Option<substrate_wire::AppliedConfinement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfinementError {
    pub reason: ConfinementRefusal,
    pub detail: String,
}
impl fmt::Display for ConfinementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.reason, self.detail)
    }
}
impl std::error::Error for ConfinementError {}
fn refuse(reason: ConfinementRefusal, detail: impl fmt::Display) -> ConfinementError {
    ConfinementError {
        reason,
        detail: detail.to_string(),
    }
}
fn scope_error(detail: impl fmt::Display) -> ConfinementError {
    refuse(ConfinementRefusal::ScopeInvalid, detail)
}

#[derive(Debug, Clone, Default)]
pub struct SubstrateRunner {
    cgroup_root: Option<PathBuf>,
}
impl SubstrateRunner {
    pub fn new(cgroup_root: Option<PathBuf>) -> Self {
        Self { cgroup_root }
    }

    /// Probe the real backend before opening a model session. No test is launched.
    pub fn preflight(&self, root: &Path) -> Result<(), ConfinementError> {
        let root = validate_workspace(root)?;
        let (_state, _driver) = self.open(&root)?;
        Ok(())
    }

    fn open(&self, root: &Path) -> Result<(tempfile::TempDir, Arc<HostDriver>), ConfinementError> {
        let state = tempfile::Builder::new()
            .prefix("loom-substrate-")
            .tempdir()
            .map_err(|e| refuse(ConfinementRefusal::BackendMissing, e))?;
        let mut config = HostConfig::minimum(
            root.parent()
                .ok_or_else(|| scope_error("workspace has no parent"))?,
        );
        config.capsule_root = state.path().join("capsules");
        validate_backend(&config.bubblewrap)?;
        config.cgroup_root = self.cgroup_root.clone().or_else(discover_cgroup_root);
        let delegated = config.cgroup_root.as_ref().is_some_and(|path| {
            path.join("cgroup.controllers").is_file()
                && std::fs::read_to_string(path.join("cgroup.procs"))
                    .is_ok_and(|s| s.trim().is_empty())
                && std::fs::OpenOptions::new()
                    .write(true)
                    .open(path.join("cgroup.procs"))
                    .is_ok()
        });
        if !delegated {
            return Err(refuse(
                ConfinementRefusal::CgroupUndelegated,
                "Substrate requires an empty delegated cgroup v2 subtree containing this process in a child; use systemd-run --user -p Delegate=yes --scope or --cgroup-root",
            ));
        }
        let driver = HostDriver::open(config).map_err(|e| {
            refuse(
                ConfinementRefusal::BackendMissing,
                format!("Substrate open: {e:?}"),
            )
        })?;
        let facts = driver.machine().facts;
        if facts.exec_cgroup_limits.is_none() {
            return Err(refuse(
                ConfinementRefusal::CgroupUndelegated,
                "Substrate did not establish cgroup execution capability; check delegated cpu, memory and pids controllers and user namespaces",
            ));
        }
        if facts.exec_no_egress != Some(true) || facts.exec_workspace_scoped_write != Some(true) {
            return Err(refuse(
                ConfinementRefusal::CapabilityUnserved,
                "Substrate does not serve no-network execution with scoped workspace writes",
            ));
        }
        let name = root
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| scope_error("workspace name must be UTF-8"))?;
        driver.workspace_root_identity(name).map_err(|e| {
            refuse(
                ConfinementRefusal::WorkspaceNameInvalid,
                format!("Substrate adopt: {e:?}"),
            )
        })?;
        Ok((state, driver))
    }

    /// Run a child process the host names, confined through Substrate with the guarantees a test
    /// has: a guarantee Substrate does not serve, or an applied confinement other than the one
    /// requested, is a [`ConfinementError`] and nothing runs unconfined; the child's environment
    /// is exactly [`ChildCommand::environment`]; and the result carries the applied confinement
    /// and the driver's own record of it.
    pub fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        let root = validate_workspace(&child.directory)?;
        let (_state, driver) = self.open(&root)?;
        let workspace = workspace_name(&root)?;
        let input = child.input(workspace, driver.machine().snapshot);
        Self::launch(&driver, workspace, &input)
    }

    /// Start `input` in the adopted workspace, wait for it, and accept its result only when the
    /// confinement Substrate applied is the one `input` required.
    fn launch(
        driver: &HostDriver,
        workspace: &str,
        input: &ExecStartInput,
    ) -> Result<TestExecution, ConfinementError> {
        let profile = requested_profile(input);
        let digest = profile_digest(&profile, &input.env.allow);
        let root_identity = driver
            .workspace_root_identity(workspace)
            .map_err(|e| scope_error(format!("Substrate adopt: {e:?}")))?;
        let id = format!(
            "ex_loom_{}_{}",
            std::process::id(),
            NEXT_EXEC.fetch_add(1, Ordering::Relaxed)
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| refuse(ConfinementRefusal::BackendMissing, e))?;
        match runtime.block_on(driver.start_exec(&id, &root_identity, input)) {
            DispatchOutcome::Observed(_) => {}
            DispatchOutcome::NotDispatched(e) => {
                return Err(refuse(
                    ConfinementRefusal::CapabilityUnserved,
                    format!("Substrate exec not dispatched: {e:?}"),
                ));
            }
            DispatchOutcome::ContainedAbsent(e) => {
                return Err(refuse(
                    ConfinementRefusal::CapabilityUnserved,
                    format!("Substrate exec contained absent: {e:?}"),
                ));
            }
            DispatchOutcome::OutcomeUnknown(e) => {
                return Err(refuse(
                    ConfinementRefusal::CapabilityUnserved,
                    format!("Substrate exec outcome unknown; do not retry blindly: {e:?}"),
                ));
            }
        }
        let observed = runtime.block_on(driver.observe_exec(&id)).map_err(|e| {
            refuse(
                ConfinementRefusal::CapabilityUnserved,
                format!("Substrate observation: {e:?}"),
            )
        })?;
        let record = observed.resource.applied.ok_or_else(|| {
            refuse(
                ConfinementRefusal::CapabilityUnserved,
                "Substrate returned no applied confinement",
            )
        })?;
        if record.workspace_access != input.workspace_access
            || !network_applied(input.sandbox.network, &record.network)
            || record.profile != input.sandbox.profile
            || record.read_only_roots != input.read_only_roots
            || !record.secret_slots.is_empty()
            || record.capability_snapshot != input.sandbox.capability_snapshot
            || record.cgroup.is_empty()
        {
            return Err(refuse(
                ConfinementRefusal::CapabilityUnserved,
                "Substrate's applied confinement does not match the required profile",
            ));
        }
        let timed_out = observed
            .resource
            .refusal
            .as_ref()
            .is_some_and(|r| r.code == "exec.timeout");
        let exit_code = if observed.resource.refusal.is_some()
            || observed.resource.state != substrate_wire::ExecState::Exited
            || !observed.output_complete
        {
            None
        } else {
            observed
                .resource
                .exit
                .as_ref()
                .and_then(|e| e.code)
                .map(i32::from)
        };
        let applied = AppliedConfinement {
            backend: Backend::Substrate,
            profile_digest: digest,
            capability_snapshot: record.capability_snapshot.clone(),
            cgroup: record.cgroup.clone(),
            filesystem: enum_name(&record.filesystem),
            network: enum_name(&record.network),
            profile: enum_name(&record.profile),
            writable_scopes: record
                .workspace_access
                .writable_subtrees()
                .unwrap_or_default()
                .to_vec(),
            read_only_roots: roots_projection(&record.read_only_roots),
        };
        let mut output = format!(
            "{}{}",
            String::from_utf8_lossy(tail(&observed.stdout)),
            String::from_utf8_lossy(tail(&observed.stderr))
        );
        if observed.stdout_truncated || observed.stderr_truncated {
            output.push_str("\n[Substrate output truncated]\n");
        }
        if let Some(refusal) = observed.resource.refusal {
            output.push_str(&format!("\n{}: {}\n", refusal.code, refusal.message));
        }
        Ok(TestExecution {
            exit_code,
            timed_out,
            output,
            applied,
            driver_record: Some(record),
        })
    }
}

impl TestRunner for SubstrateRunner {
    fn backend(&self) -> Backend {
        Backend::Substrate
    }

    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        let root = validate_workspace(root)?;
        let (_state, driver) = self.open(&root)?;
        let closure = RustClosure::discover(&root)?;
        let snapshot = driver.machine();
        let workspace = workspace_name(&root)?;
        let input = closure.input(command, workspace, snapshot.snapshot)?;
        let mut execution = Self::launch(&driver, workspace, &input)?;
        if execution.exit_code != Some(0)
            && (execution.output.contains("offline")
                || execution.output.contains("no matching package"))
        {
            execution.output.push_str("\nDependencies are offline. Run `cargo fetch` explicitly on the host before retrying Loom; Loom never fetches dependencies.\n");
        }
        Ok(execution)
    }
}

/// A child process the host names, run confined by [`SubstrateRunner::run_child`] with the
/// guarantees a test has.
///
/// Its program and arguments are never a shell line. Its directory is adopted as Substrate's
/// workspace and is the child's working directory, `/workspace`; the child writes only under its
/// `target/`, as a test does, and no toolchain or cache is mounted. Its environment is exactly the
/// one the host names as Substrate's [`ExecEnvironment`], empty unless it names one. It asks
/// Substrate for [`SandboxProfile::Workspace`] with [`NetworkMode::None`] unless the host names
/// another, and Substrate is always required: a guarantee it does not serve refuses the launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildCommand {
    program: String,
    args: Vec<String>,
    directory: PathBuf,
    environment: ExecEnvironment,
    profile: SandboxProfile,
    network: NetworkMode,
    timeout: Duration,
}

impl ChildCommand {
    /// How long a child may run unless [`ChildCommand::with_timeout`] says otherwise.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

    /// `program` with `args` in `directory`, with an empty environment, under the workspace
    /// profile with no network.
    pub fn new<A: Into<String>>(
        program: impl Into<String>,
        args: impl IntoIterator<Item = A>,
        directory: impl Into<PathBuf>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            directory: directory.into(),
            environment: ExecEnvironment {
                allow: Vec::new(),
                set: BTreeMap::new(),
            },
            profile: SandboxProfile::Workspace,
            network: NetworkMode::None,
            timeout: Self::DEFAULT_TIMEOUT,
        }
    }

    /// The same child with exactly `environment` and no other variable.
    #[must_use]
    pub fn with_environment(self, environment: ExecEnvironment) -> Self {
        Self {
            environment,
            ..self
        }
    }

    /// The same child under `profile`.
    #[must_use]
    pub fn with_profile(self, profile: SandboxProfile) -> Self {
        Self { profile, ..self }
    }

    /// The same child asking Substrate for `network`.
    #[must_use]
    pub fn with_network(self, network: NetworkMode) -> Self {
        Self { network, ..self }
    }

    /// The same child, killed once it has run for `timeout`.
    #[must_use]
    pub fn with_timeout(self, timeout: Duration) -> Self {
        Self { timeout, ..self }
    }

    /// The program the host named.
    pub fn program(&self) -> &str {
        &self.program
    }

    /// The host's arguments, without shell expansion.
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// The directory the child runs in.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// The child's whole environment.
    pub fn environment(&self) -> &ExecEnvironment {
        &self.environment
    }

    /// The sandbox profile the child asks for.
    pub fn profile(&self) -> SandboxProfile {
        self.profile
    }

    /// The network the child asks for.
    pub fn network(&self) -> NetworkMode {
        self.network
    }

    /// The maximum duration of one run of the child.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    fn input(&self, workspace: &str, snapshot: String) -> ExecStartInput {
        ExecStartInput {
            workspace: workspace.to_owned(),
            argv: std::iter::once(&self.program)
                .chain(&self.args)
                .cloned()
                .collect(),
            env: self.environment.clone(),
            sandbox: ConfinementRequest {
                capability_snapshot: snapshot,
                network: self.network,
                aperture: None,
                profile: self.profile,
                required: true,
            },
            limits: limits(self.timeout),
            wait: true,
            workspace_access: WorkspaceAccess::Scoped {
                writable_subtrees: vec![WRITABLE.to_owned()],
            },
            scratch: None,
            measurements: Default::default(),
            read_only_roots: Vec::new(),
            secret_slots: Vec::new(),
            capsule: None,
            lease_ttl_ms: None,
        }
    }
}

fn workspace_name(root: &Path) -> Result<&str, ConfinementError> {
    root.file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| scope_error("workspace name must be UTF-8"))
}

/// Whether the network Substrate applied is the one requested. Only no network is accepted: an
/// applied aperture is not verified here, so a request for one is refused rather than run
/// unverified.
fn network_applied(requested: NetworkMode, applied: &substrate_wire::AppliedNetwork) -> bool {
    matches!(
        (requested, applied),
        (NetworkMode::None, substrate_wire::AppliedNetwork::None)
    )
}

/// The resource bounds every confined launch runs under.
fn limits(timeout: Duration) -> ExecLimits {
    ExecLimits {
        timeout_ms: u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX),
        output_bytes: 1_048_576,
        processes: 2048,
        memory_bytes: 8_589_934_592,
        cpu_millis: 3_600_000,
    }
}
fn tail(bytes: &[u8]) -> &[u8] {
    &bytes[bytes.len().saturating_sub(8192)..]
}
fn enum_name(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .expect("wire enum serializes")
        .as_str()
        .expect("closed enum is a string")
        .to_owned()
}

fn validate_workspace(root: &Path) -> Result<PathBuf, ConfinementError> {
    let absolute = if root.is_absolute() {
        root.to_owned()
    } else {
        std::env::current_dir().map_err(scope_error)?.join(root)
    };
    reject_links(&absolute)?;
    let root = absolute.canonicalize().map_err(scope_error)?;
    let name = root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if name.is_empty()
        || name.starts_with('-')
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(refuse(
            ConfinementRefusal::WorkspaceNameInvalid,
            "Substrate workspace names require ASCII letters, digits, underscore or hyphen and cannot start with hyphen",
        ));
    }
    let target = root.join(WRITABLE);
    reject_links(&target)?;
    match std::fs::create_dir(&target) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && target.is_dir() => {}
        Err(e) => return Err(scope_error(format!("target/ must be a directory: {e}"))),
    }
    validate_writable_artifacts(&target)?;
    Ok(root)
}

/// Read-only mounts do not protect an inode also reachable through a writable
/// hardlink. Count every in-scope name and refuse aliases outside the scope.
/// Internal hardlinks are normal in Cargo's incremental cache and remain valid.
/// Do not follow symlinks: the sandbox resolves those through its own mounts.
fn validate_writable_artifacts(target: &Path) -> Result<(), ConfinementError> {
    let mut directories = vec![target.to_owned()];
    let mut links = BTreeMap::<(u64, u64), (u64, u64, PathBuf)>::new();
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).map_err(scope_error)? {
            let path = entry.map_err(scope_error)?.path();
            let metadata = std::fs::symlink_metadata(&path).map_err(scope_error)?;
            if metadata.is_dir() {
                directories.push(path);
            } else if metadata.is_file() {
                if metadata.nlink() > 1 {
                    let (expected, seen, _) = links
                        .entry((metadata.dev(), metadata.ino()))
                        .or_insert((metadata.nlink(), 0, path));
                    if *expected != metadata.nlink() {
                        return Err(scope_error("target/ hardlinks changed during validation"));
                    }
                    *seen += 1;
                }
            } else if !metadata.file_type().is_symlink() {
                return Err(scope_error(format!(
                    "target/ contains a socket, device or other non-artifact: {}",
                    path.display()
                )));
            }
        }
    }
    for (_, (expected, seen, path)) in links {
        if expected != seen {
            return Err(scope_error(format!(
                "target/ file has hardlinks outside its writable scope: {}; remove the alias before retrying",
                path.display()
            )));
        }
    }
    Ok(())
}
fn reject_links(path: &Path) -> Result<(), ConfinementError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(scope_error("parent traversal is not a writable scope"));
        }
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(scope_error(format!(
                    "symlinked scope or root: {}",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(scope_error(e)),
        }
    }
    Ok(())
}

struct RustClosure {
    roots: Vec<ReadOnlyRoot>,
    environment: BTreeMap<String, String>,
    _cargo_home: tempfile::TempDir,
}
impl RustClosure {
    fn discover(workspace: &Path) -> Result<Self, ConfinementError> {
        // The host rustup only resolves an installed compiler. AUTO_INSTALL=0 prevents
        // a repository's rust-toolchain file from turning resolution into a download.
        let rustc = host_program("rustup", workspace).map(|rustup| {
            Command::new(rustup)
                .args(["which", "rustc"])
                .env("RUSTUP_AUTO_INSTALL", "0")
                .current_dir(workspace)
                .output()
        });
        let mut roots = Vec::new();
        let mut path = "/usr/bin:/bin".to_owned();
        if let Some(Ok(output)) = rustc
            && output.status.success()
        {
            let binary = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
            let toolchain = binary
                .parent()
                .and_then(Path::parent)
                .ok_or_else(|| scope_error("rustup returned an invalid compiler path"))?;
            let installed = std::env::var_os("RUSTUP_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".rustup")))
                .ok_or_else(|| scope_error("cannot locate installed Rust toolchains"))?
                .join("toolchains");
            validate_toolchain(toolchain, &installed)?;
            roots.push(ReadOnlyRoot {
                host_path: toolchain
                    .canonicalize()
                    .map_err(scope_error)?
                    .to_string_lossy()
                    .into_owned(),
                mount: RUST_MOUNT.to_owned(),
            });
            path = format!("{RUST_MOUNT}/bin:{path}");
        }
        let cargo_home = tempfile::Builder::new()
            .prefix("loom-cargo-")
            .tempdir_in(workspace.join(WRITABLE))
            .map_err(scope_error)?;
        let home = std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".cargo")));
        if let Some(home) = home {
            for relative in CACHE_DIRS {
                let source = home.join(relative);
                if !source.exists() {
                    continue;
                }
                reject_links(&source)?;
                if relative == "registry" {
                    validate_registry(&source)?;
                }
                if !source.is_dir() {
                    return Err(scope_error(format!(
                        "dependency cache is not a directory: {}",
                        source.display()
                    )));
                }
                let mount = format!("{CACHE_MOUNT}/{relative}");
                roots.push(ReadOnlyRoot {
                    host_path: source
                        .canonicalize()
                        .map_err(scope_error)?
                        .to_string_lossy()
                        .into_owned(),
                    mount: mount.clone(),
                });
                let destination = cargo_home.path().join(relative);
                std::fs::create_dir_all(destination.parent().expect("cache parent"))
                    .map_err(scope_error)?;
                std::os::unix::fs::symlink(mount, destination).map_err(scope_error)?;
            }
        }
        let home_name = cargo_home
            .path()
            .file_name()
            .expect("tempdir name")
            .to_string_lossy();
        let environment = BTreeMap::from([
            ("PATH".to_owned(), path),
            ("HOME".to_owned(), "/tmp".to_owned()),
            (
                "CARGO_HOME".to_owned(),
                format!("/workspace/target/{home_name}"),
            ),
            (
                "CARGO_TARGET_DIR".to_owned(),
                "/workspace/target".to_owned(),
            ),
            ("CARGO_NET_OFFLINE".to_owned(), "true".to_owned()),
            ("CARGO_BUILD_JOBS".to_owned(), "2".to_owned()),
            ("RUSTUP_AUTO_INSTALL".to_owned(), "0".to_owned()),
        ]);
        Ok(Self {
            roots,
            environment,
            _cargo_home: cargo_home,
        })
    }
    fn input(
        &self,
        command: &TestCommand,
        workspace: &str,
        snapshot: String,
    ) -> Result<ExecStartInput, ConfinementError> {
        let argv = std::iter::once(command.program())
            .chain(command.args().iter().map(|s| s.as_os_str()))
            .map(|s| {
                s.to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| scope_error("Substrate command arguments must be UTF-8"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ExecStartInput {
            workspace: workspace.to_owned(),
            argv,
            env: ExecEnvironment {
                allow: Vec::new(),
                set: self.environment.clone(),
            },
            sandbox: ConfinementRequest {
                capability_snapshot: snapshot,
                network: NetworkMode::None,
                aperture: None,
                profile: SandboxProfile::Workspace,
                required: true,
            },
            limits: limits(command.timeout()),
            wait: true,
            workspace_access: WorkspaceAccess::Scoped {
                writable_subtrees: vec![WRITABLE.to_owned()],
            },
            scratch: None,
            measurements: Default::default(),
            read_only_roots: self.roots.clone(),
            secret_slots: Vec::new(),
            capsule: None,
            lease_ttl_ms: None,
        })
    }
}
fn host_program(name: &str, workspace: &Path) -> Option<PathBuf> {
    // A model-edited executable in a relative PATH entry must never run on the host.
    std::env::split_paths(&std::env::var_os("PATH")?)
        .filter(|p| p.is_absolute())
        .filter_map(|p| p.join(name).canonicalize().ok())
        .find(|p| p.is_file() && !p.starts_with(workspace))
}

fn validate_backend(path: &Path) -> Result<(), ConfinementError> {
    if !std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0) {
        return Err(refuse(
            ConfinementRefusal::BackendMissing,
            "Substrate requires executable /usr/bin/bwrap",
        ));
    }
    Ok(())
}

fn validate_toolchain(toolchain: &Path, installed: &Path) -> Result<(), ConfinementError> {
    reject_links(toolchain)?;
    reject_links(installed)?;
    if toolchain.parent() != Some(installed) {
        return Err(scope_error(
            "Rust compiler must belong to an installed RUSTUP_HOME/toolchains directory; custom external toolchain paths are not admitted",
        ));
    }
    Ok(())
}

fn validate_registry(source: &Path) -> Result<(), ConfinementError> {
    for entry in std::fs::read_dir(source).map_err(scope_error)? {
        let entry = entry.map_err(scope_error)?;
        let name = entry.file_name();
        if !["index", "src", "cache", "CACHEDIR.TAG"]
            .iter()
            .any(|allowed| name == *allowed)
        {
            return Err(scope_error(
                "Cargo registry contains a non-cache entry; only index, src, cache and CACHEDIR.TAG may be mounted",
            ));
        }
        reject_links(&entry.path())?;
    }
    Ok(())
}

fn roots_projection(roots: &[ReadOnlyRoot]) -> Vec<ToolchainRoot> {
    roots
        .iter()
        .map(|r| ToolchainRoot {
            host_path: r.host_path.clone(),
            mount: r.mount.clone(),
        })
        .collect()
}
fn requested_profile(input: &ExecStartInput) -> ConfinementProfile {
    ConfinementProfile {
        backend: Backend::Substrate,
        network: enum_name(&input.sandbox.network),
        writable_scopes: input
            .workspace_access
            .writable_subtrees()
            .unwrap_or_default()
            .to_vec(),
        read_only_roots: roots_projection(&input.read_only_roots),
        environment: input
            .env
            .set
            .iter()
            .map(|(name, value)| EnvironmentEntry {
                name: name.clone(),
                value: value.clone(),
            })
            .collect(),
        timeout_ms: i64::try_from(input.limits.timeout_ms).unwrap_or(i64::MAX),
        memory_bytes: input.limits.memory_bytes as i64,
        processes: i64::from(input.limits.processes),
        cpu_millis: input.limits.cpu_millis as i64,
    }
}
fn profile_digest(profile: &ConfinementProfile, inherited: &[BaselineEnvironment]) -> String {
    // Generated ESS model types deliberately have no serialization dependency.
    let mut json = serde_json::json!({ "backend":"substrate", "network":profile.network, "writable_scopes":profile.writable_scopes,
        "read_only_roots":profile.read_only_roots.iter().map(|r| serde_json::json!({"host_path":r.host_path,"mount":r.mount})).collect::<Vec<_>>(),
        "environment":profile.environment.iter().map(|e| serde_json::json!({"name":e.name,"value":e.value})).collect::<Vec<_>>(),
        "timeout_ms":profile.timeout_ms,"memory_bytes":profile.memory_bytes,"processes":profile.processes,"cpu_millis":profile.cpu_millis });
    // Baseline variables Substrate supplies by name; absent when none is named, so a test's
    // digest is what it was before children existed.
    if !inherited.is_empty() {
        json["inherited"] = inherited.iter().map(|name| name.as_str()).collect();
    }
    format!("{:x}", Sha256::digest(json.to_string().as_bytes()))
}

fn current_cgroup() -> Option<PathBuf> {
    std::fs::read_to_string("/proc/self/cgroup")
        .ok()?
        .lines()
        .find_map(|s| s.strip_prefix("0::"))
        .map(|s| Path::new("/sys/fs/cgroup").join(s.trim_start_matches('/')))
}
/// Find an already delegated, empty ancestor; discovery never moves the process.
pub fn discover_cgroup_root() -> Option<PathBuf> {
    let current = current_cgroup()?;
    current
        .ancestors()
        .take_while(|p| p.starts_with("/sys/fs/cgroup"))
        .find(|p| {
            std::fs::read_to_string(p.join("cgroup.procs")).is_ok_and(|s| s.trim().is_empty())
                && std::fs::OpenOptions::new()
                    .write(true)
                    .open(p.join("cgroup.procs"))
                    .is_ok()
        })
        .map(Path::to_owned)
}
/// Called only by the CLI child after entering its own delegated systemd scope.
/// Move the caller to a leaf so the scope can enforce subtree resource controllers.
pub fn prepare_delegated_cgroup() -> Result<PathBuf, ConfinementError> {
    let fail = |e| refuse(ConfinementRefusal::CgroupUndelegated, e);
    let root = current_cgroup().ok_or_else(|| fail("no unified cgroup membership".to_owned()))?;
    let leaf = root.join(format!("loom-controller-{}", std::process::id()));
    std::fs::create_dir(&leaf).map_err(|e| fail(e.to_string()))?;
    std::fs::write(leaf.join("cgroup.procs"), std::process::id().to_string())
        .map_err(|e| fail(e.to_string()))?;
    std::fs::write(root.join("cgroup.subtree_control"), "+cpu +memory +pids")
        .map_err(|e| fail(e.to_string()))?;
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_nonexecutable_backend_is_named() {
        let root = tempfile::tempdir().unwrap();
        let backend = root.path().join("bwrap");
        assert_eq!(
            validate_backend(&backend).unwrap_err().reason,
            ConfinementRefusal::BackendMissing
        );
        std::fs::write(&backend, "not executable").unwrap();
        std::fs::set_permissions(&backend, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            validate_backend(&backend).unwrap_err().reason,
            ConfinementRefusal::BackendMissing
        );
    }

    #[test]
    fn external_or_symlinked_custom_toolchain_is_not_mounted() {
        let root = tempfile::tempdir().unwrap();
        let installed = root.path().join("toolchains");
        let stable = installed.join("stable");
        std::fs::create_dir_all(&stable).unwrap();
        validate_toolchain(&stable, &installed).unwrap();
        assert_eq!(
            validate_toolchain(root.path(), &installed)
                .unwrap_err()
                .reason,
            ConfinementRefusal::ScopeInvalid
        );
        let custom = installed.join("custom");
        std::os::unix::fs::symlink(root.path(), &custom).unwrap();
        assert_eq!(
            validate_toolchain(&custom, &installed).unwrap_err().reason,
            ConfinementRefusal::ScopeInvalid
        );
    }

    #[test]
    fn registry_credentials_and_symlinked_cache_are_not_admitted() {
        let root = tempfile::tempdir().unwrap();
        for name in ["index", "src", "cache"] {
            std::fs::create_dir(root.path().join(name)).unwrap();
        }
        validate_registry(root.path()).unwrap();
        std::fs::write(root.path().join("credentials.toml"), "secret").unwrap();
        assert_eq!(
            validate_registry(root.path()).unwrap_err().reason,
            ConfinementRefusal::ScopeInvalid
        );
        std::fs::remove_file(root.path().join("credentials.toml")).unwrap();
        std::fs::remove_dir(root.path().join("index")).unwrap();
        std::os::unix::fs::symlink(root.path().join("src"), root.path().join("index")).unwrap();
        assert_eq!(
            validate_registry(root.path()).unwrap_err().reason,
            ConfinementRefusal::ScopeInvalid
        );
    }

    #[test]
    fn writable_aliases_must_stay_wholly_inside_target() {
        let fixture = tempfile::tempdir().unwrap();
        let workspace = fixture.path().join("workspace");
        let target = workspace.join("target");
        std::fs::create_dir_all(target.join("nested")).unwrap();
        let file = target.join("artifact");
        std::fs::write(&file, "artifact").unwrap();
        std::fs::hard_link(&file, target.join("nested/internal")).unwrap();
        validate_workspace(&workspace).expect("internal Cargo-style hardlinks are safe");
        std::fs::hard_link(&file, workspace.join("source")).unwrap();
        assert_eq!(
            validate_workspace(&workspace).unwrap_err().reason,
            ConfinementRefusal::ScopeInvalid
        );
    }

    #[test]
    fn a_child_request_is_exactly_what_the_host_named_and_requires_substrate() {
        let environment = ExecEnvironment {
            allow: vec![BaselineEnvironment::Lang],
            set: BTreeMap::from([("FIRST".to_owned(), "one".to_owned())]),
        };
        let child = ChildCommand::new("printf", ["%s", "one two"], "/work/space")
            .with_environment(environment.clone())
            .with_timeout(Duration::from_secs(7));
        let input = child.input("space", "snapshot".to_owned());
        assert_eq!(input.workspace, "space");
        assert_eq!(input.argv, ["printf", "%s", "one two"]);
        assert_eq!(input.env, environment);
        assert_eq!(
            input.sandbox,
            ConfinementRequest {
                capability_snapshot: "snapshot".to_owned(),
                network: NetworkMode::None,
                aperture: None,
                profile: SandboxProfile::Workspace,
                required: true,
            }
        );
        assert_eq!(
            input.workspace_access,
            WorkspaceAccess::Scoped {
                writable_subtrees: vec!["target".to_owned()]
            }
        );
        assert!(input.read_only_roots.is_empty());
        assert!(input.secret_slots.is_empty());
        assert!(input.capsule.is_none());
        assert_eq!(input.limits, limits(Duration::from_secs(7)));
        let bare = ChildCommand::new("true", [] as [&str; 0], "/work/space");
        assert!(bare.environment().allow.is_empty() && bare.environment().set.is_empty());
        assert_eq!(bare.timeout(), ChildCommand::DEFAULT_TIMEOUT);
    }

    #[test]
    fn only_no_network_is_accepted_as_applied() {
        use substrate_wire::{
            ApertureBytes, ApertureMechanism, ApertureMode, AppliedAperture, AppliedNetwork,
        };
        let aperture = AppliedNetwork::Aperture(AppliedAperture {
            mode: ApertureMode::Aperture,
            name: "model".to_owned(),
            destination: "10.0.0.1:443".to_owned(),
            mechanism: ApertureMechanism::LoopbackForwarder,
            bytes: ApertureBytes::default(),
            max_bytes: None,
        });
        assert!(network_applied(NetworkMode::None, &AppliedNetwork::None));
        assert!(!network_applied(NetworkMode::None, &aperture));
        assert!(!network_applied(NetworkMode::Aperture, &aperture));
        assert!(!network_applied(
            NetworkMode::Aperture,
            &AppliedNetwork::None
        ));
    }

    #[test]
    fn inherited_baseline_names_change_the_digest_and_their_absence_does_not() {
        let child = ChildCommand::new("true", [] as [&str; 0], "/work/space");
        let input = child.input("space", "snapshot".to_owned());
        let profile = requested_profile(&input);
        assert_eq!(profile.network, "none");
        assert_eq!(profile.writable_scopes, ["target"]);
        assert_ne!(
            profile_digest(&profile, &[]),
            profile_digest(&profile, &[BaselineEnvironment::Path])
        );
        // With no inherited name the digest is the one a test's profile always had.
        let before = serde_json::json!({ "backend":"substrate", "network":"none", "writable_scopes":["target"],
            "read_only_roots":[], "environment":[],
            "timeout_ms":300_000, "memory_bytes":8_589_934_592_i64, "processes":2048, "cpu_millis":3_600_000 });
        assert_eq!(
            profile_digest(&profile, &[]),
            format!("{:x}", Sha256::digest(before.to_string().as_bytes()))
        );
    }

    #[test]
    fn writable_host_socket_is_not_an_artifact() {
        let fixture = tempfile::tempdir().unwrap();
        let workspace = fixture.path().join("workspace");
        std::fs::create_dir_all(workspace.join("target")).unwrap();
        let _socket =
            std::os::unix::net::UnixListener::bind(workspace.join("target/host.sock")).unwrap();
        assert_eq!(
            validate_workspace(&workspace).unwrap_err().reason,
            ConfinementRefusal::ScopeInvalid
        );
    }
}
