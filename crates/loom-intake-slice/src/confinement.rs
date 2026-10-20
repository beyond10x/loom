//! Confined local tests, and confined children a host names. The embedded open/adopt/exec path is
//! ported from Harness. Substrate owns isolation; Loom declares a closed Rust/system-tool profile
//! for a test, the host's own environment and profile for a child ([`ChildCommand`]), and records
//! the driver's actual observation. Host dependency fetching is never implicit.

use std::collections::BTreeMap;
use std::fmt;
use std::net::{Ipv4Addr, SocketAddr};
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use crate::executor::TestCommand;
pub use intake_model::confinement::{
    AppliedConfinement, Backend, ConfinementProfile, ConfinementRefusal, EnvironmentEntry,
    ToolchainRoot,
};
use sha2::{Digest as _, Sha256};
/// Substrate's own declaration of the egress aperture a [`ChildCommand`] may reach: its name, the
/// host and port the host names, the address that host was resolved to and an optional byte
/// ceiling.
pub use substrate_host::EgressAperture;
use substrate_host::{DispatchOutcome, Driver as _, HostConfig, HostDriver};
use substrate_wire::{
    APERTURE_LOOPBACK_ADDRESS, ApertureMechanism, ApertureMode, AppliedNetwork, ConfinementRequest,
    ExecLimits, ExecStartInput, MAX_WORKSPACE_WRITABLE_SUBTREES, ReadOnlyRoot, WorkspaceAccess,
    validate_relative_path, validate_workspace_access,
};
/// Substrate's own wire types a host names a [`ChildCommand`] with.
pub use substrate_wire::{BaselineEnvironment, ExecEnvironment, NetworkMode, SandboxProfile};

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

/// The Substrate runner: tests ([`TestRunner`]) and children the host names
/// ([`SubstrateRunner::run_child`]) run through it confined.
///
/// A runner and its clones share one Substrate driver per directory that holds their workspaces
/// and egress aperture they name (Substrate fixes a driver's apertures when it opens), opened at
/// the first launch that needs it, so their launches in workspaces of one directory run side by
/// side; each launch releases its execution from the driver once it ended. Opening a driver ends
/// every confined run under its cgroup root (Substrate reconciles them as orphans), so no driver is
/// opened on a root, however its path is spelled, while a launch of this process runs under it: a
/// launch that needs a driver for another directory, or for another egress aperture, then is
/// refused with [`ConfinementRefusal::CapabilityUnserved`] instead.
#[derive(Debug, Clone, Default)]
pub struct SubstrateRunner {
    cgroup_root: Option<PathBuf>,
    drivers: Arc<Drivers>,
}

/// The drivers a runner and its clones opened, by the directory holding their workspaces, each
/// declaring the egress aperture it was opened for, if any.
#[derive(Default)]
struct Drivers(Mutex<BTreeMap<PathBuf, Vec<Arc<OpenDriver>>>>);
impl fmt::Debug for Drivers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let drivers = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        f.debug_list().entries(drivers.keys()).finish()
    }
}

/// One opened driver, the cgroup root it reconciled, the egress aperture it declares, and its
/// private state directory.
struct OpenDriver {
    driver: Arc<HostDriver>,
    cgroup_root: PathBuf,
    egress: Option<EgressAperture>,
    _state: tempfile::TempDir,
}

/// The confined launches running in this process, by cgroup root, over every runner.
static RUNNING: Mutex<BTreeMap<PathBuf, usize>> = Mutex::new(BTreeMap::new());

fn running() -> MutexGuard<'static, BTreeMap<PathBuf, usize>> {
    RUNNING.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One execution a shared driver tracks, released when dropped. A driver tracks a bounded number
/// of executions (`HostConfig::max_tracked_execs`) and keeps a finished one until it is
/// discarded, so a launch discards its own once its result is read or it ended in a refusal;
/// one still running is kept.
struct Release<'a> {
    driver: &'a HostDriver,
    id: &'a str,
}
impl Drop for Release<'_> {
    fn drop(&mut self) {
        self.driver.discard_superseded_exec(self.id);
    }
}

/// One launch running under a cgroup root, counted from before its start until it is dropped.
struct Running(PathBuf);
impl Running {
    fn enter(cgroup_root: &Path) -> Self {
        *running().entry(cgroup_root.to_owned()).or_default() += 1;
        Self(cgroup_root.to_owned())
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        let mut running = running();
        if let Some(count) = running.get_mut(&self.0) {
            *count -= 1;
            if *count == 0 {
                running.remove(&self.0);
            }
        }
    }
}

impl SubstrateRunner {
    pub fn new(cgroup_root: Option<PathBuf>) -> Self {
        Self {
            cgroup_root,
            drivers: Arc::default(),
        }
    }

    /// Probe the real backend before opening a model session. No test is launched.
    pub fn preflight(&self, root: &Path) -> Result<(), ConfinementError> {
        let root = validate_workspace(root, &[])?;
        self.driver(&root, None)?;
        Ok(())
    }

    /// The driver for the workspace `root` declaring `egress`: the one this runner or a clone
    /// opened for its directory and that aperture, or a new one. The workspace is adopted on it
    /// either way.
    fn driver(
        &self,
        root: &Path,
        egress: Option<&EgressAperture>,
    ) -> Result<Arc<OpenDriver>, ConfinementError> {
        let parent = root
            .parent()
            .ok_or_else(|| scope_error("workspace has no parent"))?;
        let mut drivers = self
            .drivers
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let opened = drivers
            .get(parent)
            .and_then(|opened| opened.iter().find(|open| open.egress.as_ref() == egress))
            .map(Arc::clone);
        let open = match opened {
            Some(open) => open,
            None => {
                let open = Arc::new(self.open(parent, egress)?);
                drivers
                    .entry(parent.to_owned())
                    .or_default()
                    .push(Arc::clone(&open));
                open
            }
        };
        drop(drivers);
        open.driver
            .workspace_root_identity(workspace_name(root)?)
            .map_err(|e| {
                refuse(
                    ConfinementRefusal::WorkspaceNameInvalid,
                    format!("Substrate adopt: {e:?}"),
                )
            })?;
        Ok(open)
    }

    fn open(
        &self,
        parent: &Path,
        egress: Option<&EgressAperture>,
    ) -> Result<OpenDriver, ConfinementError> {
        let state = tempfile::Builder::new()
            .prefix("loom-substrate-")
            .tempdir()
            .map_err(|e| refuse(ConfinementRefusal::BackendMissing, e))?;
        let mut config = HostConfig::minimum(parent);
        config.capsule_root = state.path().join("capsules");
        config.egress_apertures = egress.into_iter().cloned().collect();
        validate_backend(&config.bubblewrap)?;
        // One root has one name: the count of running launches is keyed by it, however the host
        // spelled the root.
        let cgroup_root = self
            .cgroup_root
            .clone()
            .or_else(discover_cgroup_root)
            .and_then(|path| path.canonicalize().ok())
            .filter(|path| {
                path.join("cgroup.controllers").is_file()
                    && std::fs::read_to_string(path.join("cgroup.procs"))
                        .is_ok_and(|s| s.trim().is_empty())
                    && std::fs::OpenOptions::new()
                        .write(true)
                        .open(path.join("cgroup.procs"))
                        .is_ok()
            });
        let Some(cgroup_root) = cgroup_root else {
            return Err(refuse(
                ConfinementRefusal::CgroupUndelegated,
                "Substrate requires an empty delegated cgroup v2 subtree containing this process in a child; use systemd-run --user -p Delegate=yes --scope or --cgroup-root",
            ));
        };
        config.cgroup_root = Some(cgroup_root.clone());
        // Opening kills every exec cgroup under the root; hold the count so no launch starts
        // there meanwhile, and open nothing over a launch already running there.
        let running = running();
        if running.get(&cgroup_root).is_some_and(|count| *count > 0) {
            return Err(refuse(
                ConfinementRefusal::CapabilityUnserved,
                format!(
                    "a confined launch of this process is running under {}; opening another Substrate driver there would end it",
                    cgroup_root.display()
                ),
            ));
        }
        let driver = HostDriver::open(config).map_err(|e| {
            refuse(
                ConfinementRefusal::BackendMissing,
                format!("Substrate open: {e:?}"),
            )
        })?;
        drop(running);
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
        // Substrate publishes the declared apertures only once its mechanism worked in a
        // throwaway sandbox; a declared one it does not publish is never served.
        if let Some(aperture) = egress {
            let Some(served) = facts.exec_egress_apertures.as_ref() else {
                return Err(refuse(
                    ConfinementRefusal::CapabilityUnserved,
                    format!(
                        "Substrate does not serve the egress aperture `{}` on this machine: its mechanism did not prove itself when the driver opened",
                        aperture.name
                    ),
                ));
            };
            if !served.iter().any(|fact| {
                fact.name == aperture.name
                    && fact.destination == aperture.pinned.to_string()
                    && fact.max_bytes == aperture.max_bytes
            }) {
                return Err(refuse(
                    ConfinementRefusal::CapabilityUnserved,
                    format!(
                        "Substrate published no egress aperture `{}` as it was declared",
                        aperture.name
                    ),
                ));
            }
        }
        Ok(OpenDriver {
            driver,
            cgroup_root,
            egress: egress.cloned(),
            _state: state,
        })
    }

    /// Run a child process the host names, confined through Substrate with the guarantees a test
    /// has. A guarantee Substrate does not serve is a [`ConfinementError`] and nothing runs, never
    /// an unconfined fall-back. The child's environment is exactly [`ChildCommand::environment`],
    /// it writes only under `target` and the workspace paths the host named
    /// ([`ChildCommand::with_writable`]), and its network is none or exactly the egress aperture
    /// it names ([`ChildCommand::with_egress`]); an aperture naming no host, or a writable path
    /// that names no directory of the workspace by itself, is refused before anything starts.
    /// Once the child has run, its result is refused as a [`ConfinementError`] when the
    /// confinement Substrate applied is not the one requested, a wider network or writable set
    /// included, or when Substrate reports no exit status unless the child's timeout or its
    /// aperture's byte ceiling ended it; otherwise it carries the applied confinement, listing the
    /// writable paths, and the driver's own record of it.
    pub fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        check_program(&child.program)?;
        check_network(child.network, child.egress.as_ref())?;
        let writable = check_writable(&child.writable)?;
        let root = validate_workspace(&child.directory, &writable)?;
        let open = self.driver(&root, child.egress.as_ref())?;
        let workspace = workspace_name(&root)?;
        let input = child.input(workspace, open.driver.machine().snapshot);
        child_result(Self::launch(&open, workspace, &input)?)
    }

    /// Start `input` in the adopted workspace, wait for it, and accept its result only when the
    /// confinement Substrate applied is the one `input` required, with the egress aperture the
    /// driver declares when `input` asks for one. A program Substrate's `/usr/bin/env` would read
    /// as a variable assignment is refused before anything starts.
    fn launch(
        open: &OpenDriver,
        workspace: &str,
        input: &ExecStartInput,
    ) -> Result<Launched, ConfinementError> {
        if let Some(program) = input.argv.first() {
            check_program(program)?;
        }
        let driver = &open.driver;
        let egress = open.egress.as_ref();
        let _running = Running::enter(&open.cgroup_root);
        let profile = requested_profile(input, egress);
        let digest = profile_digest(&profile, &input.env.allow, egress);
        let root_identity = driver
            .workspace_root_identity(workspace)
            .map_err(|e| scope_error(format!("Substrate adopt: {e:?}")))?;
        let id = format!(
            "ex_loom_{}_{}",
            std::process::id(),
            NEXT_EXEC.fetch_add(1, Ordering::Relaxed)
        );
        // Declared before anything that can return, so every path releases the execution.
        let _release = Release {
            driver: driver.as_ref(),
            id: &id,
        };
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
        check_record(input, egress, &record)?;
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
            network: applied_network_name(&record.network),
            profile: enum_name(&record.profile),
            writable_scopes: record
                .workspace_access
                .writable_subtrees()
                .unwrap_or_default()
                .to_vec(),
            read_only_roots: roots_projection(&record.read_only_roots),
        };
        let ending = format!(
            "state {}, refusal {}, output complete {}",
            enum_name(&observed.resource.state),
            observed
                .resource
                .refusal
                .as_ref()
                .map_or("none", |r| r.code.as_str()),
            observed.output_complete
        );
        let mut output = format!(
            "{}{}",
            String::from_utf8_lossy(tail(&observed.stdout)),
            String::from_utf8_lossy(tail(&observed.stderr))
        );
        if observed.stdout_truncated || observed.stderr_truncated {
            output.push_str("\n[Substrate output truncated]\n");
        }
        let refusal = observed.resource.refusal.map(|refusal| {
            output.push_str(&format!("\n{}: {}\n", refusal.code, refusal.message));
            refusal.code
        });
        Ok(Launched {
            execution: TestExecution {
                exit_code,
                timed_out,
                output,
                applied,
                driver_record: Some(record),
            },
            ending,
            refusal,
        })
    }
}

/// What one launch returned, how Substrate observed its end, and the code of the refusal it ended
/// with, if any.
struct Launched {
    execution: TestExecution,
    ending: String,
    refusal: Option<String>,
}

/// The refusal Substrate ends a run with once its egress aperture's declared byte ceiling is
/// reached, as it ends one at its deadline with `exec.timeout`.
const BYTE_LIMIT: &str = "exec.aperture-byte-limit";

/// A child's result: refused when Substrate reports no exit status, unless the child's deadline or
/// the byte ceiling of the egress aperture it ran with ended it, since such a child was ended by
/// something other than itself or a bound the host declared. A child its byte ceiling ended has
/// no exit status and is not timed out; its output names [`BYTE_LIMIT`] and its driver record
/// counts the bytes that crossed.
fn child_result(launched: Launched) -> Result<TestExecution, ConfinementError> {
    let execution = launched.execution;
    // Only a ceiling the record shows applied: the record already equals the declaration.
    let ceiling_ended = launched.refusal.as_deref() == Some(BYTE_LIMIT)
        && execution.driver_record.as_ref().is_some_and(|record| {
            matches!(&record.network, AppliedNetwork::Aperture(aperture) if aperture.max_bytes.is_some())
        });
    if execution.exit_code.is_none() && !execution.timed_out && !ceiling_ended {
        return Err(refuse(
            ConfinementRefusal::CapabilityUnserved,
            format!(
                "Substrate reports no exit status for the child, no timeout and no byte ceiling it declared ({})",
                launched.ending
            ),
        ));
    }
    Ok(execution)
}

impl TestRunner for SubstrateRunner {
    fn backend(&self) -> Backend {
        Backend::Substrate
    }

    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        check_program(&command.program().to_string_lossy())?;
        // A test writes only under `target`: no host names it another writable path.
        let root = validate_workspace(root, &[])?;
        let open = self.driver(&root, None)?;
        let closure = RustClosure::discover(&root)?;
        let snapshot = open.driver.machine();
        let workspace = workspace_name(&root)?;
        let input = closure.input(command, workspace, snapshot.snapshot)?;
        let mut execution = Self::launch(&open, workspace, &input)?.execution;
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
/// Its program and arguments are never a shell line, and a program containing `=` is refused,
/// since Substrate's `/usr/bin/env` would read it as an assignment. Its directory is adopted as Substrate's
/// workspace and is the child's working directory, `/workspace`; the child writes only under its
/// `target/`, as a test does, and under the workspace directories the host names
/// ([`ChildCommand::with_writable`]), `.git` only when named, and no toolchain or cache is
/// mounted. Its environment is exactly the one the host names as Substrate's
/// [`ExecEnvironment`], empty unless it names one. It asks
/// Substrate for [`SandboxProfile::Workspace`] with [`NetworkMode::None`] unless the host names
/// another, and Substrate is always required: a guarantee it does not serve refuses the launch.
///
/// A child that must reach one host, such as a harness and its vendor's API, names it as the
/// Substrate [`EgressAperture`] it may reach ([`ChildCommand::with_egress`]): its network is then
/// that aperture alone, every other guarantee kept. Substrate gives one run at most one aperture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildCommand {
    program: String,
    args: Vec<String>,
    directory: PathBuf,
    writable: Vec<PathBuf>,
    environment: ExecEnvironment,
    profile: SandboxProfile,
    network: NetworkMode,
    egress: Option<EgressAperture>,
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
            writable: Vec::new(),
            environment: ExecEnvironment {
                allow: Vec::new(),
                set: BTreeMap::new(),
            },
            profile: SandboxProfile::Workspace,
            network: NetworkMode::None,
            egress: None,
            timeout: Self::DEFAULT_TIMEOUT,
        }
    }

    /// The same child, also writing under `path`, a directory of the workspace named relative to
    /// it: a scratch copy the child edits, `.git` for a child that commits, or a command's state
    /// directory. Everything else in the workspace stays read-only, `.git` included unless the
    /// host names it; a child that may write `.git` may also plant hooks or configuration that a
    /// later git call on the host runs. Substrate makes only directories writable, so a file at
    /// the workspace root cannot be named. Naming `target`, or a path twice, changes nothing.
    ///
    /// Refused [`ConfinementRefusal::ScopeInvalid`] before anything starts, naming the path: a path
    /// that is absolute, climbs out with `..`, or is no plain relative path (`.`, an empty
    /// component, a trailing `/`); one through a symbolic link, absent, or no directory (a file,
    /// socket, device or FIFO); a directory holding a socket, device or FIFO, or a file also
    /// hard-linked outside the writable paths; a path inside `target` or inside another named
    /// path; and more than 63 named paths, which with `target` pass Substrate's 64.
    #[must_use]
    pub fn with_writable(mut self, path: impl Into<PathBuf>) -> Self {
        self.writable.push(path.into());
        self
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

    /// The same child asking Substrate for `network`. [`NetworkMode::Aperture`] needs the aperture
    /// [`ChildCommand::with_egress`] names; without one the launch is refused before anything
    /// starts.
    #[must_use]
    pub fn with_network(self, network: NetworkMode) -> Self {
        Self { network, ..self }
    }

    /// The same child reaching `aperture` and nothing else: [`NetworkMode::Aperture`], with the
    /// host, port and pinned address `aperture` declares. The child reaches it by the host's name,
    /// which Substrate maps to its forwarder on `127.0.0.1` inside the sandbox, at the named port.
    /// A declaration Substrate's daemon never makes, or one the child could not reach by, is
    /// refused before anything starts: a name it refuses, no port, a byte ceiling of zero, a host
    /// that is no host name or IPv4 literal, an IPv4 literal host other than `127.0.0.1` (named in
    /// the refusal; name such a host by its name), an IPv4 literal other than the pinned address,
    /// and a pinned address that is no IPv4 one or not at the named port. A child its byte ceiling
    /// ends is a result with no exit status, not timed out, whose output names
    /// `exec.aperture-byte-limit`. It gets no certificate bundle: a child that verifies TLS
    /// certificates cannot verify the host's.
    #[must_use]
    pub fn with_egress(self, aperture: EgressAperture) -> Self {
        Self {
            network: NetworkMode::Aperture,
            egress: Some(aperture),
            ..self
        }
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

    /// The workspace paths the host named writable beside `target`, as named.
    pub fn writable(&self) -> &[PathBuf] {
        &self.writable
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

    /// The egress aperture the child may reach, if it names one.
    pub fn egress(&self) -> Option<&EgressAperture> {
        self.egress.as_ref()
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
                // A name only: the destination is the driver's declaration, never the request's.
                aperture: self.egress.as_ref().map(|aperture| aperture.name.clone()),
                profile: self.profile,
                required: true,
            },
            limits: limits(self.timeout),
            wait: true,
            workspace_access: WorkspaceAccess::Scoped {
                writable_subtrees: writable_request(&self.writable),
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

/// Refuse a program Substrate's `/usr/bin/env` would read as one of its own operands instead of
/// the program to run: a `NAME=VALUE` assignment, which would set a variable nobody named, one
/// Substrate refuses by name included, and a word starting with `-`, an option there (a lone `-`
/// is `-i`, and the next word would run instead). The refusal names an assignment up to its `=`,
/// never the value.
fn check_program(program: &str) -> Result<(), ConfinementError> {
    match program.split_once('=') {
        Some((name, _)) => Err(scope_error(format!(
            "the program `{name}=…` contains '=': Substrate starts a program through /usr/bin/env, which would read it as a variable assignment"
        ))),
        None if program.starts_with('-') => Err(scope_error(format!(
            "the program `{program}` starts with '-': Substrate starts a program through /usr/bin/env, which would read it as an option"
        ))),
        None => Ok(()),
    }
}

/// The workspace paths a child asks Substrate to write, in the order Substrate requires (sorted,
/// each once): `target` and the paths the host named, as named.
fn writable_request(named: &[PathBuf]) -> Vec<String> {
    let mut paths: Vec<String> = std::iter::once(WRITABLE.to_owned())
        .chain(named.iter().map(|path| path.to_string_lossy().into_owned()))
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// Refuse, before anything is touched, a named writable path that cannot name a directory of the
/// workspace by itself: one that is not UTF-8, is absolute, climbs out with `..`, or is no plain
/// relative path Substrate admits; a path inside another writable path, `target` included; and
/// more writable paths than Substrate admits. Returns what the child asks to write
/// ([`writable_request`]); [`validate_workspace`] checks the named paths on disk.
fn check_writable(named: &[PathBuf]) -> Result<Vec<String>, ConfinementError> {
    for path in named {
        let Some(text) = path.to_str() else {
            return Err(scope_error(format!(
                "the writable path {path:?} is not UTF-8"
            )));
        };
        if path.is_absolute() {
            return Err(scope_error(format!(
                "the writable path `{text}` is absolute, so it may lie outside the workspace; name a writable path relative to the workspace"
            )));
        }
        if path.components().any(|part| part == Component::ParentDir) {
            return Err(scope_error(format!(
                "the writable path `{text}` climbs out with `..`; name a writable path inside the workspace"
            )));
        }
        validate_relative_path(text).map_err(|e| {
            scope_error(format!(
                "the writable path `{text}` is no plain workspace-relative path ({e}); name it without `.`, empty components or a trailing `/`"
            ))
        })?;
    }
    let paths = writable_request(named);
    // An ancestor sorts before every path beneath it, though not always right before it.
    for (index, path) in paths.iter().enumerate() {
        if let Some(inner) = paths[index + 1..].iter().find(|other| {
            other
                .strip_prefix(path.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
        }) {
            return Err(scope_error(format!(
                "the writable paths `{path}` and `{inner}` overlap; name only `{path}`"
            )));
        }
    }
    if paths.len() > MAX_WORKSPACE_WRITABLE_SUBTREES as usize {
        return Err(scope_error(format!(
            "{} writable paths with `target` pass the {MAX_WORKSPACE_WRITABLE_SUBTREES} Substrate admits",
            paths.len()
        )));
    }
    // Substrate's own rules for the request's shape, which the driver applies again.
    validate_workspace_access(&WorkspaceAccess::Scoped {
        writable_subtrees: paths.clone(),
    })
    .map_err(|e| scope_error(format!("the writable paths are refused by Substrate: {e}")))?;
    Ok(paths)
}

fn workspace_name(root: &Path) -> Result<&str, ConfinementError> {
    root.file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| scope_error("workspace name must be UTF-8"))
}

/// Refuse, before anything starts, a network the child cannot have exactly as named: an aperture
/// with no named host, a mode and an aperture that disagree, and an aperture declaration
/// Substrate's daemon never makes. The daemon declares `name=host:port/tcp` with a name it admits,
/// a port other than 0 and a byte ceiling other than zero, and pins the IPv4 address `host`
/// resolves to at that port; so a host that is no host name or IPv4 literal ([`check_host`]), an
/// IPv4 literal other than the pinned address, a pinned address that is no IPv4 one, or a pinned
/// port other than the named port is refused too, and so is an IPv4 literal host other than
/// `127.0.0.1`, which the child could not reach the host by. The embedded driver checks only a request's
/// shape, and writes the host verbatim into the child's `/etc/hosts`, so the declaration's rules
/// are held here.
fn check_network(
    network: NetworkMode,
    egress: Option<&EgressAperture>,
) -> Result<(), ConfinementError> {
    let Some(aperture) = egress else {
        return match network {
            NetworkMode::None => Ok(()),
            NetworkMode::Aperture => Err(scope_error(
                "network aperture names no host: name the egress aperture with ChildCommand::with_egress",
            )),
        };
    };
    let name = &aperture.name;
    if aperture.host.trim().is_empty() {
        return Err(scope_error(format!(
            "egress aperture `{name}` names no host"
        )));
    }
    substrate_wire::validate_aperture_request(network, Some(name))
        .map_err(|e| scope_error(format!("egress aperture `{name}`: {e}")))?;
    if aperture.port == 0 {
        return Err(scope_error(format!(
            "egress aperture `{name}` names no port"
        )));
    }
    if aperture.max_bytes == Some(0) {
        return Err(scope_error(format!(
            "egress aperture `{name}` declares a byte ceiling of zero"
        )));
    }
    let SocketAddr::V4(pinned) = aperture.pinned else {
        return Err(scope_error(format!(
            "egress aperture `{name}` is pinned to no IPv4 address; Substrate's forwarder dials IPv4 only"
        )));
    };
    if pinned.port() != aperture.port {
        return Err(scope_error(format!(
            "egress aperture `{name}` is pinned to port {}, not the port it names ({})",
            pinned.port(),
            aperture.port
        )));
    }
    check_host(name, &aperture.host, *pinned.ip())
}

/// Refuse an aperture's host unless it is a declaration Substrate's daemon makes for an address
/// pinned at `pinned` and one the child can reach by: a host name, dot-separated labels of ASCII
/// letters, digits and inner hyphens, at most 63 bytes each and 253 in all, the last not all
/// digits, or the IPv4 literal `127.0.0.1` pinned there. Anything else (whitespace, a second line,
/// a port, a URL, an IPv6 literal) never resolves to an IPv4 address there. Another IPv4 literal is
/// refused too: Substrate maps only a host name to the forwarder it starts on
/// [`APERTURE_LOOPBACK_ADDRESS`] inside the sandbox, so a child dialling any other literal would
/// never reach the host; a literal also resolves to itself, so it must be `pinned`.
fn check_host(name: &str, host: &str, pinned: Ipv4Addr) -> Result<(), ConfinementError> {
    if let Ok(literal) = host.parse::<Ipv4Addr>() {
        let forwarder: Ipv4Addr = APERTURE_LOOPBACK_ADDRESS
            .parse()
            .expect("Substrate's forwarder address is an IPv4 literal");
        return if literal != forwarder {
            Err(scope_error(format!(
                "egress aperture `{name}` names the host {literal}: a child reaches an aperture by its host name, or at {forwarder}, where Substrate's forwarder listens in the sandbox; name the host by its name"
            )))
        } else if literal != pinned {
            Err(scope_error(format!(
                "egress aperture `{name}` names the address {literal} but is pinned to {pinned}"
            )))
        } else {
            Ok(())
        };
    }
    let label = |label: &str| {
        (1..=63).contains(&label.len())
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    };
    let host_name = host.len() <= 253
        && host.split('.').all(label)
        && !host
            .rsplit('.')
            .next()
            .is_some_and(|last| last.bytes().all(|byte| byte.is_ascii_digit()));
    if host_name {
        Ok(())
    } else {
        Err(scope_error(format!(
            "egress aperture `{name}` names the host {host:?}, which is no host name or IPv4 literal"
        )))
    }
}

/// Accept Substrate's record of what it applied only when it is the confinement `input` required:
/// its workspace access (exactly the writable paths requested, so a record reporting a wider or
/// another writable set is refused), profile, read-only roots and capability snapshot, no secret
/// slot, a cgroup, and exactly the requested network ([`network_applied`]). A test's launch and a
/// child's are held to the same record.
fn check_record(
    input: &ExecStartInput,
    egress: Option<&EgressAperture>,
    record: &substrate_wire::AppliedConfinement,
) -> Result<(), ConfinementError> {
    if record.workspace_access != input.workspace_access
        || !network_applied(input.sandbox.network, egress, &record.network)
        || input.sandbox.aperture.as_deref() != egress.map(|aperture| aperture.name.as_str())
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
    Ok(())
}

/// Whether the network Substrate applied is the one requested: no network for
/// [`NetworkMode::None`], and for [`NetworkMode::Aperture`] exactly the declared aperture, by its
/// name, pinned destination, mechanism and byte ceiling. Any other network, wider or narrower, is
/// not the one requested.
fn network_applied(
    requested: NetworkMode,
    egress: Option<&EgressAperture>,
    applied: &AppliedNetwork,
) -> bool {
    match (requested, egress, applied) {
        (NetworkMode::None, None, AppliedNetwork::None) => true,
        (NetworkMode::Aperture, Some(declared), AppliedNetwork::Aperture(applied)) => {
            applied.mode == ApertureMode::Aperture
                && applied.name == declared.name
                && applied.destination == declared.pinned.to_string()
                && applied.mechanism == ApertureMechanism::LoopbackForwarder
                && applied.max_bytes == declared.max_bytes
        }
        _ => false,
    }
}

/// A network as the confinement projection names it: `none`, or `aperture:<name>@<destination>`.
fn network_name(aperture: Option<(&str, &str)>) -> String {
    aperture.map_or_else(
        || "none".to_owned(),
        |(name, destination)| format!("aperture:{name}@{destination}"),
    )
}

/// The network Substrate applied, named as [`network_name`] names it.
fn applied_network_name(network: &AppliedNetwork) -> String {
    match network {
        AppliedNetwork::None => network_name(None),
        AppliedNetwork::Aperture(applied) => {
            network_name(Some((&applied.name, &applied.destination)))
        }
    }
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

/// Validate the workspace `root` and what a launch in it may write: `target`, created here, and the
/// `writable` paths a host named ([`check_writable`]; empty for a test). Returns the canonical root.
fn validate_workspace(root: &Path, writable: &[String]) -> Result<PathBuf, ConfinementError> {
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
    let named: Vec<&String> = writable.iter().filter(|path| *path != WRITABLE).collect();
    // Before `target` is created, so a refused path leaves the workspace as it was.
    for path in &named {
        check_writable_directory(&root, path)?;
    }
    let target = root.join(WRITABLE);
    reject_links(&target)?;
    match std::fs::create_dir(&target) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && target.is_dir() => {}
        Err(e) => return Err(scope_error(format!("target/ must be a directory: {e}"))),
    }
    let scopes: Vec<PathBuf> = std::iter::once(target)
        .chain(named.iter().map(|path| root.join(path)))
        .collect();
    validate_writable_artifacts(&scopes)?;
    Ok(root)
}

/// Refuse a named writable path unless each of its components, from the workspace down, exists
/// and is a directory and no symbolic link: Substrate makes only directories writable, and a link
/// could lead the child's writes out of the workspace or onto a path the host did not name.
fn check_writable_directory(root: &Path, path: &str) -> Result<(), ConfinementError> {
    let mut current = root.to_owned();
    for component in path.split('/') {
        current.push(component);
        let metadata = match std::fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(scope_error(format!(
                    "the writable path `{path}` does not exist in the workspace"
                )));
            }
            Err(e) => return Err(scope_error(format!("the writable path `{path}`: {e}"))),
        };
        if metadata.file_type().is_symlink() {
            return Err(scope_error(format!(
                "the writable path `{path}` passes through the symbolic link {}",
                current.display()
            )));
        }
        if !metadata.is_dir() {
            return Err(scope_error(format!(
                "the writable path `{path}` is no directory (a file, socket, device or FIFO): Substrate makes only directories writable"
            )));
        }
    }
    Ok(())
}

/// Read-only mounts do not protect an inode also reachable through a writable
/// hardlink. Count every name under the writable `scopes` and refuse aliases outside them.
/// Internal hardlinks are normal in Cargo's incremental cache and remain valid.
/// Do not follow symlinks: the sandbox resolves those through its own mounts.
fn validate_writable_artifacts(scopes: &[PathBuf]) -> Result<(), ConfinementError> {
    let mut directories = scopes.to_vec();
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
                        return Err(scope_error(
                            "hard links in the writable paths changed during validation",
                        ));
                    }
                    *seen += 1;
                }
            } else if !metadata.file_type().is_symlink() {
                return Err(scope_error(format!(
                    "a writable path contains a socket, device or other non-artifact: {}",
                    path.display()
                )));
            }
        }
    }
    for (_, (expected, seen, path)) in links {
        if expected != seen {
            return Err(scope_error(format!(
                "a file in a writable path has hard links outside the writable paths: {}; remove the alias before retrying",
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
fn requested_profile(
    input: &ExecStartInput,
    egress: Option<&EgressAperture>,
) -> ConfinementProfile {
    ConfinementProfile {
        backend: Backend::Substrate,
        network: match (input.sandbox.network, egress) {
            (NetworkMode::Aperture, Some(aperture)) => {
                network_name(Some((&aperture.name, &aperture.pinned.to_string())))
            }
            (network, _) => enum_name(&network),
        },
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
fn profile_digest(
    profile: &ConfinementProfile,
    inherited: &[BaselineEnvironment],
    egress: Option<&EgressAperture>,
) -> String {
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
    // The whole egress declaration, absent without one for the same reason.
    if let Some(aperture) = egress {
        json["egress"] = serde_json::json!({ "name":aperture.name, "host":aperture.host, "port":aperture.port,
            "pinned":aperture.pinned.to_string(), "max_bytes":aperture.max_bytes });
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
        validate_workspace(&workspace, &[]).expect("internal Cargo-style hardlinks are safe");
        std::fs::hard_link(&file, workspace.join("source")).unwrap();
        assert_eq!(
            validate_workspace(&workspace, &[]).unwrap_err().reason,
            ConfinementRefusal::ScopeInvalid
        );
    }

    #[test]
    fn a_hard_link_between_writable_paths_is_internal_and_one_leaving_them_is_not() {
        let fixture = tempfile::tempdir().unwrap();
        let workspace = fixture.path().join("workspace");
        std::fs::create_dir_all(workspace.join("target")).unwrap();
        std::fs::create_dir(workspace.join("scratch")).unwrap();
        let file = workspace.join("target/artifact");
        std::fs::write(&file, "artifact").unwrap();
        std::fs::hard_link(&file, workspace.join("scratch/alias")).unwrap();
        let writable = check_writable(&[PathBuf::from("scratch")]).unwrap();
        validate_workspace(&workspace, &writable).expect("both names are writable");
        let error = validate_workspace(&workspace, &[]).unwrap_err();
        assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid);
        assert!(error.detail.contains("hard links outside"), "{error}");
    }

    #[test]
    fn a_child_asks_to_write_target_and_the_named_paths_in_substrates_order() {
        let child = ChildCommand::new("true", [] as [&str; 0], "/work/space")
            .with_writable("src")
            .with_writable(".git")
            .with_writable("a/b")
            .with_writable("a-b")
            .with_writable("target")
            .with_writable("src");
        let expected = [".git", "a-b", "a/b", "src", "target"];
        assert_eq!(check_writable(child.writable()).unwrap(), expected);
        let input = child.input("space", "snapshot".to_owned());
        assert_eq!(
            input.workspace_access.writable_subtrees(),
            Some(expected.map(str::to_owned).as_slice())
        );
        let profile = requested_profile(&input, None);
        assert_eq!(profile.writable_scopes, expected);
        let bare = ChildCommand::new("true", [] as [&str; 0], "/work/space");
        let plain = requested_profile(&bare.input("space", "snapshot".to_owned()), None);
        assert_ne!(
            profile_digest(&profile, &[], None),
            profile_digest(&plain, &[], None)
        );
        let named = 63;
        let most = (0..named).fold(bare.clone(), |child, n| {
            child.with_writable(format!("p{n:02}"))
        });
        assert_eq!(check_writable(most.writable()).unwrap().len(), 64);
        let error = check_writable(most.with_writable("p63").writable()).unwrap_err();
        assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid);
        assert!(error.detail.contains("65 writable paths"), "{error}");
    }

    #[test]
    fn a_writable_path_that_names_no_plain_workspace_path_is_refused_naming_it() {
        for (path, named) in [
            ("/work/space/src", "absolute"),
            ("..", "climbs out"),
            ("../src", "climbs out"),
            ("src/../..", "climbs out"),
            ("", "no plain workspace-relative path"),
            (".", "no plain workspace-relative path"),
            ("./src", "no plain workspace-relative path"),
            ("src/", "no plain workspace-relative path"),
            ("src//lib", "no plain workspace-relative path"),
            ("src/./lib", "no plain workspace-relative path"),
            ("src\\lib", "no plain workspace-relative path"),
            ("target/debug", "overlap"),
        ] {
            let error = check_writable(&[PathBuf::from(path)]).unwrap_err();
            assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{path:?}");
            assert!(
                error.detail.contains(&format!("`{path}`")),
                "{path:?}: {error}"
            );
            assert!(error.detail.contains(named), "{path:?}: {error}");
        }
        let error = check_writable(&[PathBuf::from("src"), PathBuf::from("src/lib")]).unwrap_err();
        assert!(
            error.detail.contains("`src` and `src/lib` overlap"),
            "{error}"
        );
        use std::os::unix::ffi::OsStrExt as _;
        let invalid = std::ffi::OsStr::from_bytes(b"src\xff");
        let error = check_writable(&[PathBuf::from(invalid)]).unwrap_err();
        assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid);
        assert!(error.detail.contains("not UTF-8"), "{error}");
        assert_eq!(check_writable(&[]).unwrap(), ["target"]);
    }

    #[test]
    fn a_writable_path_through_a_link_absent_or_no_directory_is_refused_on_disk() {
        let fixture = tempfile::tempdir().unwrap();
        let workspace = fixture.path().join("workspace");
        std::fs::create_dir_all(workspace.join("src/lib")).unwrap();
        std::os::unix::fs::symlink(fixture.path(), workspace.join("up")).unwrap();
        std::os::unix::fs::symlink("lib", workspace.join("src/alias")).unwrap();
        std::fs::write(workspace.join("file"), "file").unwrap();
        for (path, named) in [
            ("up", "symbolic link"),
            ("up/workspace", "symbolic link"),
            ("src/alias", "symbolic link"),
            ("absent", "does not exist"),
            ("src/absent", "does not exist"),
            ("file", "no directory"),
            ("file/below", "no directory"),
        ] {
            let writable = check_writable(&[PathBuf::from(path)]).unwrap();
            let error = validate_workspace(&workspace, &writable).unwrap_err();
            assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{path}");
            assert!(error.detail.contains(&format!("`{path}`")), "{error}");
            assert!(error.detail.contains(named), "{path}: {error}");
            assert!(
                !workspace.join("target").exists(),
                "{path}: a refused path leaves the workspace as it was"
            );
        }
        let writable = check_writable(&[PathBuf::from("src/lib")]).unwrap();
        validate_workspace(&workspace, &writable).expect("a plain directory is writable");
    }

    #[test]
    fn a_record_reporting_other_writable_paths_than_requested_is_refused() {
        let input = ChildCommand::new("true", [] as [&str; 0], "/work/space")
            .with_writable("scratch")
            .input("space", "snapshot".to_owned());
        let exact = record_for(&input, AppliedNetwork::None);
        check_record(&input, None, &exact).unwrap();
        let scoped = |paths: &[&str]| WorkspaceAccess::Scoped {
            writable_subtrees: paths.iter().map(|path| (*path).to_owned()).collect(),
        };
        for access in [
            WorkspaceAccess::ReadWrite,
            WorkspaceAccess::ReadOnly,
            scoped(&["target"]),
            scoped(&["scratch", "src", "target"]),
            scoped(&["scratch/inner", "target"]),
            scoped(&["target", "scratch"]),
        ] {
            let record = substrate_wire::AppliedConfinement {
                workspace_access: access.clone(),
                ..exact.clone()
            };
            let error = check_record(&input, None, &record).unwrap_err();
            assert_eq!(
                error.reason,
                ConfinementRefusal::CapabilityUnserved,
                "{access:?}"
            );
        }
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
        assert!(bare.writable().is_empty(), "only `target` is writable");
        assert_eq!(bare.timeout(), ChildCommand::DEFAULT_TIMEOUT);
    }

    #[test]
    fn a_root_named_another_way_is_counted_as_one_root() {
        let fixture = tempfile::tempdir().unwrap();
        let cgroup = fixture.path().join("cgroup");
        std::fs::create_dir(&cgroup).unwrap();
        std::fs::write(cgroup.join("cgroup.controllers"), "cpu memory pids\n").unwrap();
        std::fs::write(cgroup.join("cgroup.procs"), "").unwrap();
        let launch = Running::enter(&cgroup.canonicalize().unwrap());
        let spelled = cgroup.join("..").join("cgroup");
        let error = SubstrateRunner::new(Some(spelled))
            .open(fixture.path(), None)
            .err()
            .expect("nothing is opened");
        let expected = if Path::new("/usr/bin/bwrap").is_file() {
            ConfinementRefusal::CapabilityUnserved
        } else {
            ConfinementRefusal::BackendMissing
        };
        assert_eq!(error.reason, expected, "{error}");
        drop(launch);
    }

    #[test]
    fn no_driver_is_opened_over_a_launch_running_under_its_cgroup_root() {
        let fixture = tempfile::tempdir().unwrap();
        // Shaped as an empty delegated cgroup, so only the running launch stands in the way.
        let cgroup = fixture.path().join("cgroup");
        std::fs::create_dir(&cgroup).unwrap();
        std::fs::write(cgroup.join("cgroup.controllers"), "cpu memory pids\n").unwrap();
        std::fs::write(cgroup.join("cgroup.procs"), "").unwrap();
        let runner = SubstrateRunner::new(Some(cgroup.clone()));
        assert!(Arc::ptr_eq(&runner.drivers, &runner.clone().drivers));
        let launch = Running::enter(&cgroup);
        let error = runner
            .open(fixture.path(), None)
            .err()
            .expect("nothing is opened");
        let expected = if Path::new("/usr/bin/bwrap").is_file() {
            ConfinementRefusal::CapabilityUnserved
        } else {
            ConfinementRefusal::BackendMissing
        };
        assert_eq!(error.reason, expected, "{error}");
        drop(launch);
        assert!(
            !running().contains_key(&cgroup),
            "a finished launch is no longer counted"
        );
    }

    /// An applied-confinement projection; the child-result rule does not read it.
    fn projection(network: &str) -> AppliedConfinement {
        AppliedConfinement {
            backend: Backend::Substrate,
            profile_digest: String::new(),
            capability_snapshot: String::new(),
            cgroup: "substrate-ex_loom_1_0".to_owned(),
            filesystem: "workspace-rw-system-ro".to_owned(),
            network: network.to_owned(),
            profile: "workspace".to_owned(),
            writable_scopes: vec!["target".to_owned()],
            read_only_roots: Vec::new(),
        }
    }

    #[test]
    fn a_child_result_with_no_exit_status_and_no_timeout_is_refused() {
        let launched = |exit_code, timed_out| Launched {
            execution: TestExecution {
                exit_code,
                timed_out,
                output: String::new(),
                applied: projection("none"),
                driver_record: None,
            },
            ending: "state unknown, refusal none, output complete true".to_owned(),
            refusal: None,
        };
        let error = child_result(launched(None, false)).unwrap_err();
        assert_eq!(error.reason, ConfinementRefusal::CapabilityUnserved);
        assert!(error.detail.contains("state unknown"), "{error}");
        assert!(child_result(launched(None, true)).unwrap().timed_out);
        assert_eq!(
            child_result(launched(Some(3), false)).unwrap().exit_code,
            Some(3)
        );
    }

    #[test]
    fn a_child_its_declared_byte_ceiling_ended_is_a_result_and_no_other_end_is() {
        let bounded = EgressAperture {
            max_bytes: Some(1024),
            ..model()
        };
        let input = ChildCommand::new("true", [] as [&str; 0], "/work/space")
            .with_egress(bounded)
            .input("space", "snapshot".to_owned());
        let ended = |network: AppliedNetwork, refusal: Option<&str>| Launched {
            execution: TestExecution {
                exit_code: None,
                timed_out: false,
                output: format!(
                    "\n{BYTE_LIMIT}: The declared egress aperture byte ceiling was reached.\n"
                ),
                applied: projection("aperture:model@10.0.0.1:443"),
                driver_record: Some(record_for(&input, network)),
            },
            ending: format!(
                "state cancelled, refusal {}, output complete true",
                refusal.unwrap_or("none")
            ),
            refusal: refusal.map(str::to_owned),
        };
        let result = child_result(ended(
            applied_aperture("model", PINNED, Some(1024)),
            Some(BYTE_LIMIT),
        ))
        .expect("a child its byte ceiling ended is a result");
        assert_eq!((result.exit_code, result.timed_out), (None, false));
        assert!(result.output.contains(BYTE_LIMIT), "{}", result.output);
        let record = result.driver_record.expect("the driver's record");
        assert_eq!(
            record.network,
            applied_aperture("model", PINNED, Some(1024)),
            "the bytes that crossed are kept"
        );
        for (network, refusal) in [
            (
                applied_aperture("model", PINNED, Some(1024)),
                Some("exec.cancelled"),
            ),
            (applied_aperture("model", PINNED, Some(1024)), None),
            (applied_aperture("model", PINNED, None), Some(BYTE_LIMIT)),
            (AppliedNetwork::None, Some(BYTE_LIMIT)),
        ] {
            let error = child_result(ended(network.clone(), refusal)).unwrap_err();
            assert_eq!(
                error.reason,
                ConfinementRefusal::CapabilityUnserved,
                "{network:?} {refusal:?}"
            );
        }
    }

    #[test]
    fn a_program_that_reads_as_an_assignment_is_refused_naming_it_without_its_value() {
        let error = check_program("GH_TOKEN=synthetic-value").unwrap_err();
        assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid);
        assert!(error.detail.contains("`GH_TOKEN=…`"), "{error}");
        assert!(!error.detail.contains("synthetic-value"), "{error}");
        check_program("/usr/bin/env").unwrap();
    }

    #[test]
    fn a_program_env_reads_as_an_option_is_refused() {
        for program in ["-", "-i", "--", "-u"] {
            let error = check_program(program).unwrap_err();
            assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{program}");
            assert!(error.detail.contains(&format!("`{program}`")), "{error}");
        }
        let error = check_program("-X=synthetic-value").unwrap_err();
        assert!(!error.detail.contains("synthetic-value"), "{error}");
        check_program("printf").unwrap();
        check_program("./a-b").unwrap();
    }

    /// Where the aperture `model` of these tests is pinned; nothing here connects to it.
    const PINNED: &str = "10.0.0.1:443";

    fn model() -> EgressAperture {
        EgressAperture {
            name: "model".to_owned(),
            host: "api.example.test".to_owned(),
            port: 443,
            pinned: PINNED.parse().unwrap(),
            max_bytes: None,
        }
    }

    fn applied_aperture(name: &str, destination: &str, max_bytes: Option<u64>) -> AppliedNetwork {
        AppliedNetwork::Aperture(substrate_wire::AppliedAperture {
            mode: ApertureMode::Aperture,
            name: name.to_owned(),
            destination: destination.to_owned(),
            mechanism: ApertureMechanism::LoopbackForwarder,
            bytes: substrate_wire::ApertureBytes {
                to_destination: 7,
                from_destination: 11,
            },
            max_bytes,
        })
    }

    /// A record of exactly what `input` asked for, with `network` as the applied network.
    fn record_for(
        input: &ExecStartInput,
        network: AppliedNetwork,
    ) -> substrate_wire::AppliedConfinement {
        substrate_wire::AppliedConfinement {
            capability_snapshot: input.sandbox.capability_snapshot.clone(),
            cgroup: "substrate-ex_loom_1_0".to_owned(),
            filesystem: substrate_wire::AppliedFilesystem::WorkspaceReadWriteSystemReadOnly,
            network,
            profile: input.sandbox.profile,
            workspace_access: input.workspace_access.clone(),
            capsule: None,
            read_only_roots: input.read_only_roots.clone(),
            secret_slots: Vec::new(),
        }
    }

    #[test]
    fn only_the_requested_network_is_accepted_as_applied() {
        let aperture = applied_aperture("model", PINNED, None);
        assert!(network_applied(
            NetworkMode::None,
            None,
            &AppliedNetwork::None
        ));
        assert!(!network_applied(NetworkMode::None, None, &aperture));
        assert!(network_applied(
            NetworkMode::Aperture,
            Some(&model()),
            &aperture
        ));
        assert!(!network_applied(
            NetworkMode::Aperture,
            Some(&model()),
            &AppliedNetwork::None
        ));
        assert!(!network_applied(NetworkMode::Aperture, None, &aperture));
        for other in [
            applied_aperture("other", PINNED, None),
            applied_aperture("model", "10.0.0.2:443", None),
            applied_aperture("model", PINNED, Some(1024)),
        ] {
            assert!(
                !network_applied(NetworkMode::Aperture, Some(&model()), &other),
                "{other:?}"
            );
        }
        let bounded = EgressAperture {
            max_bytes: Some(1024),
            ..model()
        };
        assert!(
            !network_applied(NetworkMode::Aperture, Some(&bounded), &aperture),
            "a declared ceiling applied without it is a wider network"
        );
    }

    #[test]
    fn a_record_reporting_another_network_than_requested_is_refused() {
        let child = ChildCommand::new("true", [] as [&str; 0], "/work/space");
        // A test's request and a plain child's ask for no network alike.
        let none = child.input("space", "snapshot".to_owned());
        check_record(&none, None, &record_for(&none, AppliedNetwork::None)).unwrap();
        let wider = record_for(&none, applied_aperture("model", PINNED, None));
        let error = check_record(&none, None, &wider).unwrap_err();
        assert_eq!(error.reason, ConfinementRefusal::CapabilityUnserved);
        assert_eq!(
            error.detail,
            "Substrate's applied confinement does not match the required profile"
        );

        let input = child
            .with_egress(model())
            .input("space", "snapshot".to_owned());
        let exact = record_for(&input, applied_aperture("model", PINNED, None));
        check_record(&input, Some(&model()), &exact).unwrap();
        for other in [
            AppliedNetwork::None,
            applied_aperture("model", "10.0.0.2:443", None),
            applied_aperture("other", PINNED, None),
            applied_aperture("model", PINNED, Some(1024)),
        ] {
            let error = check_record(&input, Some(&model()), &record_for(&input, other.clone()))
                .unwrap_err();
            assert_eq!(
                error.reason,
                ConfinementRefusal::CapabilityUnserved,
                "{other:?}"
            );
        }
        // A driver declaring another aperture than the request names is no match either.
        let other = EgressAperture {
            name: "other".to_owned(),
            ..model()
        };
        let record = record_for(&input, applied_aperture("other", PINNED, None));
        assert!(check_record(&input, Some(&other), &record).is_err());
    }

    #[test]
    fn an_egress_aperture_naming_no_host_or_one_substrate_cannot_serve_is_refused() {
        check_network(NetworkMode::None, None).unwrap();
        check_network(NetworkMode::Aperture, Some(&model())).unwrap();
        let refused = [
            (NetworkMode::Aperture, None, "names no host"),
            (
                NetworkMode::Aperture,
                Some(EgressAperture {
                    host: String::new(),
                    ..model()
                }),
                "names no host",
            ),
            (
                NetworkMode::Aperture,
                Some(EgressAperture {
                    host: "\t".to_owned(),
                    ..model()
                }),
                "names no host",
            ),
            (NetworkMode::None, Some(model()), "disagree"),
            (
                NetworkMode::Aperture,
                Some(EgressAperture {
                    name: "api.example.test:443".to_owned(),
                    ..model()
                }),
                "destination",
            ),
            (
                NetworkMode::Aperture,
                Some(EgressAperture {
                    name: "Model".to_owned(),
                    ..model()
                }),
                "legal aperture name",
            ),
            (
                NetworkMode::Aperture,
                Some(EgressAperture { port: 0, ..model() }),
                "no port",
            ),
            (
                NetworkMode::Aperture,
                Some(EgressAperture {
                    max_bytes: Some(0),
                    ..model()
                }),
                "ceiling of zero",
            ),
            (
                NetworkMode::Aperture,
                Some(EgressAperture {
                    pinned: "[::1]:443".parse().unwrap(),
                    ..model()
                }),
                "IPv4",
            ),
        ];
        for (network, egress, named) in refused {
            let error = check_network(network, egress.as_ref()).unwrap_err();
            assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
            assert!(error.detail.contains(named), "{named}: {error}");
        }
    }

    #[test]
    fn an_aperture_host_or_pin_the_daemon_never_declares_is_refused() {
        let with_host = |host: &str| EgressAperture {
            host: host.to_owned(),
            ..model()
        };
        let longest_label = format!("{}.test", "a".repeat(63));
        let label_too_long = format!("{}.test", "a".repeat(64));
        let name_too_long = format!("{0}.{0}.{0}.{0}.test", "a".repeat(63));
        for host in [
            "api.example.test",
            "localhost",
            "a-b.c1.example",
            longest_label.as_str(),
        ] {
            check_network(NetworkMode::Aperture, Some(&with_host(host)))
                .unwrap_or_else(|error| panic!("{host:?}: {error}"));
        }
        // 127.0.0.1, where Substrate's forwarder listens in the sandbox, is the one literal a
        // child reaches the host by, and only pinned there.
        let at_forwarder = EgressAperture {
            host: "127.0.0.1".to_owned(),
            pinned: "127.0.0.1:443".parse().unwrap(),
            ..model()
        };
        check_network(NetworkMode::Aperture, Some(&at_forwarder)).unwrap();
        let error =
            check_network(NetworkMode::Aperture, Some(&with_host("127.0.0.1"))).unwrap_err();
        assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid);
        assert!(error.detail.contains("pinned to 10.0.0.1"), "{error}");
        for (host, pinned) in [
            ("10.0.0.1", "10.0.0.1:443"),
            ("10.0.0.2", "10.0.0.1:443"),
            ("127.0.0.2", "127.0.0.2:443"),
        ] {
            let aperture = EgressAperture {
                host: host.to_owned(),
                pinned: pinned.parse().unwrap(),
                ..model()
            };
            let error = check_network(NetworkMode::Aperture, Some(&aperture)).unwrap_err();
            assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{host}");
            assert!(
                error.detail.contains(&format!("names the host {host}:")),
                "{host}: {error}"
            );
        }
        for host in [
            "api.example.test\n127.0.0.1\tother.example.test",
            "api example test",
            " api.example.test",
            "api.example.test:443",
            "https://api.example.test/",
            "api.example.test.",
            ".api.example.test",
            "api..example.test",
            "-api.example.test",
            "api-.example.test",
            "api_model.example.test",
            "api.example.tést",
            "10.0.1",
            "1",
            "::1",
            "[::1]",
            "#api.example.test",
            label_too_long.as_str(),
            name_too_long.as_str(),
        ] {
            let error = check_network(NetworkMode::Aperture, Some(&with_host(host))).unwrap_err();
            assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{host:?}");
            assert!(
                error.detail.contains("no host name or IPv4 literal"),
                "{host:?}: {error}"
            );
        }
        for pinned in ["10.0.0.1:0", "10.0.0.1:8443"] {
            let aperture = EgressAperture {
                pinned: pinned.parse().unwrap(),
                ..model()
            };
            let error = check_network(NetworkMode::Aperture, Some(&aperture)).unwrap_err();
            assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{pinned}");
            assert!(error.detail.contains("not the port it names"), "{error}");
        }
    }

    #[test]
    fn a_child_with_egress_asks_for_its_aperture_by_name_and_its_profile_names_the_destination() {
        let child = ChildCommand::new("true", [] as [&str; 0], "/work/space").with_egress(model());
        assert_eq!(child.network(), NetworkMode::Aperture);
        assert_eq!(child.egress(), Some(&model()));
        let input = child.input("space", "snapshot".to_owned());
        assert_eq!(
            input.sandbox,
            ConfinementRequest {
                capability_snapshot: "snapshot".to_owned(),
                network: NetworkMode::Aperture,
                aperture: Some("model".to_owned()),
                profile: SandboxProfile::Workspace,
                required: true,
            }
        );
        let profile = requested_profile(&input, Some(&model()));
        assert_eq!(profile.network, "aperture:model@10.0.0.1:443");
        assert_eq!(
            applied_network_name(&applied_aperture("model", PINNED, None)),
            profile.network
        );
        assert_eq!(applied_network_name(&AppliedNetwork::None), "none");
        let digest = profile_digest(&profile, &[], Some(&model()));
        assert_ne!(digest, profile_digest(&profile, &[], None));
        for other in [
            EgressAperture {
                host: "other.example.test".to_owned(),
                ..model()
            },
            EgressAperture {
                max_bytes: Some(1024),
                ..model()
            },
        ] {
            assert_ne!(digest, profile_digest(&profile, &[], Some(&other)));
        }
    }

    #[test]
    fn inherited_baseline_names_change_the_digest_and_their_absence_does_not() {
        let child = ChildCommand::new("true", [] as [&str; 0], "/work/space");
        let input = child.input("space", "snapshot".to_owned());
        let profile = requested_profile(&input, None);
        assert_eq!(profile.network, "none");
        assert_eq!(profile.writable_scopes, ["target"]);
        assert_ne!(
            profile_digest(&profile, &[], None),
            profile_digest(&profile, &[BaselineEnvironment::Path], None)
        );
        // With no inherited name the digest is the one a test's profile always had.
        let before = serde_json::json!({ "backend":"substrate", "network":"none", "writable_scopes":["target"],
            "read_only_roots":[], "environment":[],
            "timeout_ms":300_000, "memory_bytes":8_589_934_592_i64, "processes":2048, "cpu_millis":3_600_000 });
        assert_eq!(
            profile_digest(&profile, &[], None),
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
            validate_workspace(&workspace, &[]).unwrap_err().reason,
            ConfinementRefusal::ScopeInvalid
        );
    }
}
