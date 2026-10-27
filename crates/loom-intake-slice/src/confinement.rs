//! Confined local tests, and confined children a host names. The embedded open/adopt/exec path is
//! ported from Harness. Substrate owns isolation; Loom declares a closed Rust/system-tool profile
//! for a test, the host's own environment and profile for a child ([`ChildCommand`]), and records
//! the driver's actual observation. Host dependency fetching is never implicit.

use std::collections::BTreeMap;
use std::fmt;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::os::unix::fs::MetadataExt as _;
#[cfg(test)]
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use crate::executor::TestCommand;
pub use intake_model::confinement::{
    AppliedAperture, AppliedConfinement, Backend, ChildExit, ChildOutput, ChildStream,
    ConfinementProfile, ConfinementRefusal, EnvironmentEntry, ReadOnlyFile, ToolchainRoot,
};
/// A child's confinement as one value, applied by [`ChildCommand::confined`], and the types its
/// fields name: each maps one to one onto the Substrate type the builder of the same meaning takes.
pub use intake_model::confinement::{
    BaselineVariable, ChildAperture, ChildEnvironment, ChildNetwork, ChildProfile, ConfinementSpec,
    WritableRoot,
};
use sha2::{Digest as _, Sha256};
/// Substrate's own declaration of the egress aperture a [`ChildCommand`] may reach: its name, the
/// host and port the host names, the address that host was resolved to and an optional byte
/// ceiling.
pub use substrate_host::EgressAperture;
use substrate_host::confinement::{ContractFamily, FACT_ORDER, FAMILY_UNSERVED};
use substrate_host::{DispatchOutcome, Driver as _, ExecObservation, HostConfig, HostDriver};
/// The most egress apertures one child reaches ([`ChildCommand::with_egress`]), Substrate's own
/// limit.
pub use substrate_wire::MAX_EGRESS_APERTURES;
/// The longest working directory a child is started in ([`ChildCommand::with_workdir`]), in bytes:
/// Substrate's own limit.
pub use substrate_wire::MAX_WORKDIR_BYTES;
use substrate_wire::{
    APERTURE_CA_BUNDLE_PATH, APERTURE_HOSTS_PATH, APERTURE_LOOPBACK_ADDRESS, ApertureMechanism,
    ApertureMode, AppliedNetwork, CapabilityFacts, CaptureMode, ConfinementRequest,
    EXEC_SCRATCH_MOUNT, EXECUTION_CAPSULE_MOUNT, ExecExit, ExecLimits, ExecRefusal,
    ExecSignalInput, ExecStartInput, ExecState, MAX_PATH_DEPTH, MAX_READ_ONLY_FILE_COMPONENT_BYTES,
    MAX_READ_ONLY_FILE_PATH_BYTES, MAX_READ_ONLY_FILES, MAX_READ_ONLY_ROOTS,
    MAX_WORKSPACE_WRITABLE_SUBTREES, PipeSessionStartInput, RESERVED_MOUNTS, ReadOnlyRoot,
    SessionMode, Signal, WorkspaceAccess, validate_read_only_files, validate_read_only_roots,
    validate_relative_path, validate_workspace_access,
};
/// Substrate's own wire types a host names a [`ChildCommand`] with.
pub use substrate_wire::{BaselineEnvironment, ExecEnvironment, NetworkMode, SandboxProfile};

mod child;
#[cfg(test)]
mod security_w20261023w1_u3;
#[cfg(test)]
mod several_apertures;
mod spec;
#[cfg(test)]
mod workdir;
pub use child::{ChildError, ChildFrame, ChildResult, ChildSignal, ConfinedChild};

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
    /// The standard output and standard error apart, as bytes. Substrate keeps each whole up to
    /// its output limit (1 MiB) and flags it cut past that; `output` holds only the last 8 KiB of
    /// each, as text. The unconfined runner keeps the last 8 KiB of each, flagged cut when the
    /// command wrote more.
    pub child_output: ChildOutput,
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
/// A runner and its clones share one Substrate driver per directory that holds their workspaces,
/// egress apertures and certificate bundle they name (Substrate fixes a driver's apertures and
/// bundle when it opens), opened at the first launch that needs it, so their launches in
/// workspaces of one directory run side by side; each launch releases its execution from the
/// driver once it ended. Opening a driver ends every confined run under its cgroup root (Substrate
/// reconciles them as orphans), so no driver is opened on a root, however its path is spelled,
/// while a launch of this process runs under it: a launch that needs a driver for another
/// directory, set of egress apertures or certificate bundle then is refused with
/// [`ConfinementRefusal::CapabilityUnserved`] instead.
#[derive(Debug, Clone, Default)]
pub struct SubstrateRunner {
    cgroup_root: Option<PathBuf>,
    drivers: Arc<Drivers>,
}

/// The drivers a runner and its clones opened, by the directory holding their workspaces, each
/// with the [`DriverEgress`] it was opened for.
#[derive(Default)]
struct Drivers(Mutex<BTreeMap<PathBuf, Vec<Arc<OpenDriver>>>>);
impl fmt::Debug for Drivers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let drivers = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        f.debug_list().entries(drivers.keys()).finish()
    }
}

/// The rest of a shared driver's key, beside the directory holding its workspaces: what Substrate
/// fixes when the driver opens. A launch shares a driver only with launches in workspaces of the
/// same directory whose `DriverEgress` is equal, every field compared:
///
/// - `apertures`: the egress apertures the driver declares (`HostConfig::egress_apertures`), each
///   whole declaration, in the order the child named them, or none; a set in another order is
///   another key, since the order decides each aperture's address in the sandbox;
/// - `ca_bundle`: the certificate bundle a run with those apertures trusts, as the launch read it
///   ([`CaBundle`]), compared as the path the host named and the bytes read from it, or none.
///   The driver hands Substrate a private copy of those bytes (`HostConfig::ca_bundle`).
///
/// So two children naming different bundles, one naming a bundle and one naming none, and a
/// bundle whose bytes changed since a driver opened for it, never share a driver, whatever else
/// they share. A test, and a child naming neither, use [`DriverEgress::NONE`].
#[derive(Debug, Clone, PartialEq, Eq)]
struct DriverEgress {
    apertures: Vec<EgressAperture>,
    ca_bundle: Option<CaBundle>,
}

impl DriverEgress {
    /// No egress aperture and no certificate bundle.
    const NONE: Self = Self {
        apertures: Vec::new(),
        ca_bundle: None,
    };
}

/// A certificate bundle as one launch read it ([`check_ca_bundle`]): the path the host named and
/// the bytes read from it once, through an open with its last component not followed. Those
/// bytes are what the child trusts: the driver opened for them hands Substrate a private copy of
/// them ([`CaBundle::write_private_copy`]), never the named path, so a link or another file put
/// at that path after the read is never read.
#[derive(Clone, PartialEq, Eq)]
struct CaBundle {
    named: PathBuf,
    bytes: Vec<u8>,
}

impl fmt::Debug for CaBundle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaBundle")
            .field("named", &self.named)
            .field("bytes", &self.bytes.len())
            .field("sha256", &format!("{:x}", Sha256::digest(&self.bytes)))
            .finish()
    }
}

impl CaBundle {
    /// Write the bytes read to `ca-bundle.pem` in `directory`, a driver's private state directory
    /// (created by this process for itself, owner-only, removed with the driver): a new file,
    /// never one already there or a link, read-only. Substrate snapshots this copy when a child
    /// starts, so every child of the driver trusts exactly these bytes.
    fn write_private_copy(&self, directory: &Path) -> Result<PathBuf, ConfinementError> {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let copy = directory.join("ca-bundle.pem");
        let failed = |e: std::io::Error| {
            refuse(
                ConfinementRefusal::BackendMissing,
                format!(
                    "the private copy of the CA bundle `{}` could not be written: {e}",
                    self.named.display()
                ),
            )
        };
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o400)
            .open(&copy)
            .map_err(failed)?;
        file.write_all(&self.bytes).map_err(failed)?;
        Ok(copy)
    }
}

/// The configuration of a driver for the workspaces in `parent`, opened for `egress`, before its
/// cgroup root is chosen: its capsules in `state`, its private state directory, the apertures it
/// declares, and for a certificate bundle the private copy written to `state`, never the path the
/// host named.
fn driver_config(
    parent: &Path,
    state: &Path,
    egress: &DriverEgress,
) -> Result<HostConfig, ConfinementError> {
    let mut config = HostConfig::minimum(parent);
    config.capsule_root = state.join("capsules");
    config.egress_apertures = egress.apertures.clone();
    config.ca_bundle = egress
        .ca_bundle
        .as_ref()
        .map(|bundle| bundle.write_private_copy(state))
        .transpose()?;
    Ok(config)
}

/// One opened driver, the cgroup root it reconciled, the egress it was opened for, and its private
/// state directory.
struct OpenDriver {
    driver: Arc<HostDriver>,
    /// The facts Substrate's `linux` adapter proved for this driver when it opened, in
    /// `FACT_ORDER`'s spelling ([`admitted_facts`]); every launch on it records them.
    adapter_facts: Vec<String>,
    cgroup_root: PathBuf,
    egress: DriverEgress,
    state: tempfile::TempDir,
}

impl OpenDriver {
    /// `launched`, settled on this driver, recording the adapter it ran under, Substrate's `linux`,
    /// and the facts that adapter proved when this driver opened.
    fn adapted(&self, mut launched: Launched) -> Launched {
        launched.execution.applied.adapter = Some("linux".to_owned());
        launched.execution.applied.facts = Some(self.adapter_facts.clone());
        launched
    }
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
        self.driver(&root, &DriverEgress::NONE)?;
        Ok(())
    }

    /// The driver for the workspace `root` opened for `egress`: the one this runner or a clone
    /// opened for its directory and an equal [`DriverEgress`], or a new one. The workspace is
    /// adopted on it either way.
    fn driver(
        &self,
        root: &Path,
        egress: &DriverEgress,
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
            .and_then(|opened| opened.iter().find(|open| open.egress == *egress))
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

    fn open(&self, parent: &Path, egress: &DriverEgress) -> Result<OpenDriver, ConfinementError> {
        let state = tempfile::Builder::new()
            .prefix("loom-substrate-")
            .tempdir()
            .map_err(|e| refuse(ConfinementRefusal::BackendMissing, e))?;
        let mut config = driver_config(parent, state.path(), egress)?;
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
        let machine = driver.machine();
        let adapter_facts = admitted_facts(&machine)?;
        let facts = machine.facts;
        if facts.exec_no_egress != Some(true) || facts.exec_workspace_scoped_write != Some(true) {
            return Err(refuse(
                ConfinementRefusal::CapabilityUnserved,
                "Substrate does not serve no-network execution with scoped workspace writes",
            ));
        }
        // Substrate publishes the declared apertures only once its mechanism worked in a
        // throwaway sandbox; a declared one it does not publish is never served.
        for aperture in &egress.apertures {
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
            adapter_facts,
            cgroup_root,
            egress: egress.clone(),
            state,
        })
    }

    /// Run a child process the host names, confined through Substrate with the guarantees a test
    /// has. A guarantee Substrate does not serve is a [`ConfinementError`] and nothing runs, never
    /// an unconfined fall-back. The child's environment is exactly [`ChildCommand::environment`],
    /// it writes only under `target` and the workspace paths the host named
    /// ([`ChildCommand::with_writable`]), and its network is none or exactly the egress apertures
    /// it names, at most [`MAX_EGRESS_APERTURES`] ([`ChildCommand::with_egress`]), where it trusts
    /// the certificate bundle the host names ([`ChildCommand::with_ca_bundle`]); an aperture
    /// declaration Substrate never makes or the child could not reach by name, a bundle the child
    /// cannot be given as named, or a writable path that names no directory of the workspace by
    /// itself, is refused before anything starts.
    /// Once the child has run, its result is refused as a [`ConfinementError`] when the
    /// confinement Substrate applied is not the one requested, a wider network or writable set
    /// included, or when Substrate reports no exit status unless the child's timeout or its
    /// aperture's byte ceiling ended it; otherwise it carries the applied confinement, listing the
    /// writable paths, and the driver's own record of it. Substrate's record names no certificate
    /// bundle, so the bundle is not checked after the child ran.
    ///
    /// The child's standard input is empty and closed, unless the host gave it bytes
    /// ([`ChildCommand::with_stdin`]): those it reads once, then end of file, through a Substrate
    /// raw-pipe session under the same checks; more than [`ChildCommand::MAX_INPUT_BYTES`] is
    /// refused before anything starts. Its result holds its standard output and standard error
    /// whole and apart ([`TestExecution::child_output`]) beside the tail of each in `output`.
    pub fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        check_program(&child.program)?;
        check_stdin(child.stdin.as_deref())?;
        check_network(child.network, &child.egress)?;
        // Read once, here: the child trusts these bytes, whatever the path holds later.
        let ca_bundle = check_ca_bundle(child)?;
        let writable = check_writable(&child.writable)?;
        let child = check_mounts(child, ca_bundle.as_ref())?;
        let child = child.as_ref();
        check_workdir(child)?;
        let root = validate_workspace(&child.directory, &writable)?;
        let egress = DriverEgress {
            apertures: child.egress.clone(),
            ca_bundle,
        };
        let open = self.driver(&root, &egress)?;
        let workspace = workspace_name(&root)?;
        let machine = open.driver.machine();
        check_driver_mounts(&open, child, &machine.facts)?;
        check_workdir_served(child, &machine.facts)?;
        let launched = match child.stdin.as_deref() {
            None => Self::launch(&open, workspace, &child.input(workspace, machine.snapshot))?,
            Some(stdin) => {
                check_pipe_sessions(&machine.facts)?;
                Self::launch_with_stdin(
                    &open,
                    workspace,
                    &child.session_input(workspace, machine.snapshot),
                    stdin,
                    child.timeout,
                )?
            }
        };
        child_result(launched)
    }

    /// Start a child the host names live, confined as [`SubstrateRunner::run_child`] confines
    /// one, every check before launch the same, and return it running: the host writes its
    /// standard input ([`ConfinedChild::write_stdin`], [`ConfinedChild::close_stdin`]), reads its
    /// standard output and standard error as frames of their own stream as they arrive
    /// ([`ConfinedChild::read`]), signals it ([`ConfinedChild::signal`]) and waits for its result
    /// ([`ConfinedChild::wait`]), which is checked once it ended as `run_child`'s is. It runs as a
    /// Substrate raw-pipe session; a driver that does not publish what one needs is refused
    /// [`ConfinementRefusal::CapabilityUnserved`] and nothing starts. Its timeout ends it as it
    /// ends any child, and a child dropped before it ended is killed. While it runs it counts as
    /// a launch under its cgroup root, so no driver opens there.
    ///
    /// A child named with standard input ([`ChildCommand::with_stdin`]) is refused
    /// [`ConfinementRefusal::ScopeInvalid`] before anything starts: a live child is written
    /// through `write_stdin`.
    pub fn spawn_child(&self, child: &ChildCommand) -> Result<ConfinedChild, ConfinementError> {
        check_program(&child.program)?;
        if child.stdin.is_some() {
            return Err(scope_error(
                "a live child is given standard input through ConfinedChild::write_stdin, never ChildCommand::with_stdin",
            ));
        }
        check_network(child.network, &child.egress)?;
        let ca_bundle = check_ca_bundle(child)?;
        let writable = check_writable(&child.writable)?;
        let child = check_mounts(child, ca_bundle.as_ref())?;
        let child = child.as_ref();
        check_workdir(child)?;
        let root = validate_workspace(&child.directory, &writable)?;
        let egress = DriverEgress {
            apertures: child.egress.clone(),
            ca_bundle,
        };
        let open = self.driver(&root, &egress)?;
        let machine = open.driver.machine();
        check_driver_mounts(&open, child, &machine.facts)?;
        check_workdir_served(child, &machine.facts)?;
        check_pipe_sessions(&machine.facts)?;
        let workspace = workspace_name(&root)?;
        let session = child.session_input(workspace, machine.snapshot);
        ConfinedChild::start(open, workspace, session, child.timeout)
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
        let prepared = Prepared::new(open, workspace, input)?;
        let driver = &open.driver;
        let id = prepared.id.as_str();
        // Declared before anything that can return, so every path releases the execution.
        let _release = Release {
            driver: driver.as_ref(),
            id,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| refuse(ConfinementRefusal::BackendMissing, e))?;
        dispatched(runtime.block_on(driver.start_exec(id, &prepared.root_identity, input)))?;
        let observed = runtime
            .block_on(driver.observe_exec(id))
            .map_err(observation_error)?;
        settle(
            input,
            &open.egress.apertures,
            prepared.digest,
            Ending::from(observed),
        )
        .map(|launched| open.adapted(launched))
    }

    /// Start `session` in the adopted workspace as a Substrate raw-pipe session, write `stdin` to
    /// it and close it, read its output until it closed, and settle its end as
    /// [`SubstrateRunner::launch`] does. Its output is taken from Substrate's terminal
    /// observation, which records both streams whatever was read live. A write the child ended
    /// first is no failure: it read what it read. Every path ends the child before it returns,
    /// killing it when it ran past its timeout and a margin, which leaves it with no exit status
    /// and so refused.
    fn launch_with_stdin(
        open: &OpenDriver,
        workspace: &str,
        session: &PipeSessionStartInput,
        stdin: &[u8],
        timeout: Duration,
    ) -> Result<Launched, ConfinementError> {
        let input = &session.exec;
        let prepared = Prepared::new(open, workspace, input)?;
        let driver = open.driver.as_ref();
        let id = prepared.id.as_str();
        let _release = Release { driver, id };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| refuse(ConfinementRefusal::BackendMissing, e))?;
        let deadline = drain_deadline(timeout)?;
        let observed = runtime.block_on(async {
            dispatched(
                driver
                    .start_pipe_session(id, &prepared.root_identity, session)
                    .await,
            )?;
            let write = async {
                let mut written = Ok(());
                for frame in stdin.chunks(FRAME_BYTES) {
                    written = driver.write_pipe_session(id, frame).await;
                    if written.is_err() {
                        break;
                    }
                }
                let closed = driver.close_pipe_session_input(id).await;
                written.and(closed)
            };
            let drain = async {
                loop {
                    let step = deadline
                        .saturating_duration_since(tokio::time::Instant::now())
                        .min(READ_STEP);
                    if step.is_zero() {
                        return Ok(());
                    }
                    match driver.read_pipe_session(id, step).await {
                        Ok(Some(_)) => {}
                        Ok(None) => return Ok(()),
                        Err(e) if e.code == substrate_wire::SESSION_READ_TIMEOUT => {}
                        Err(e) => return Err(e),
                    }
                }
            };
            let (written, drained) = tokio::join!(write, drain);
            let observed = match driver.observe_exec(id).await {
                Ok(observed) if is_terminal(observed.resource.state) => observed,
                // Still running past its deadline, or unobservable: it is ended here, and a child
                // ended so has no exit status.
                _ => driver
                    .signal(
                        id,
                        &ExecSignalInput {
                            signal: Signal::Kill,
                            grace_ms: 0,
                        },
                    )
                    .await
                    .map_err(observation_error)?,
            };
            if let Err(e) = drained {
                return Err(refuse(
                    ConfinementRefusal::CapabilityUnserved,
                    format!("Substrate pipe session output: {e:?}"),
                ));
            }
            match written {
                Err(e)
                    if e.code != substrate_wire::SESSION_WRITE_FAILED
                        || !is_terminal(observed.resource.state) =>
                {
                    Err(refuse(
                        ConfinementRefusal::CapabilityUnserved,
                        format!("Substrate pipe session input: {e:?}"),
                    ))
                }
                _ => Ok(observed),
            }
        })?;
        settle(
            input,
            &open.egress.apertures,
            prepared.digest,
            Ending::from(observed),
        )
        .map(|launched| open.adapted(launched))
    }
}

/// How long a child given standard input, or a live child being waited for, is read past its
/// timeout before it is killed: Substrate ends it at its timeout itself.
const DRAIN_MARGIN: Duration = Duration::from_secs(30);

/// When a child given `timeout` and still running is killed: its timeout and [`DRAIN_MARGIN`]
/// from now. A timeout past what the clock can hold is refused
/// [`ConfinementRefusal::ScopeInvalid`] before the child starts, never a panic.
fn drain_deadline(timeout: Duration) -> Result<tokio::time::Instant, ConfinementError> {
    timeout
        .checked_add(DRAIN_MARGIN)
        .and_then(|after| tokio::time::Instant::now().checked_add(after))
        .ok_or_else(|| {
            scope_error(format!(
                "a timeout of {timeout:?} is past what this host's clock can hold"
            ))
        })
}
/// The longest one read of a child's output waits, so a deadline is looked at again.
const READ_STEP: Duration = Duration::from_secs(1);
/// The largest frame of standard input Substrate takes in one write.
const FRAME_BYTES: usize = 64 * 1024;
/// The most frames of output Substrate queues for a pipe session before it waits for a read.
const QUEUED_FRAMES: u32 = 16;

/// Whether Substrate observed `state` as an end.
fn is_terminal(state: ExecState) -> bool {
    !matches!(state, ExecState::Accepted | ExecState::Running)
}

/// What a launch holds from before its start until its result is read: its count under the
/// cgroup root, the digest of the profile it asks for, the adopted workspace's identity and the
/// id of its execution.
struct Prepared {
    _running: Running,
    digest: String,
    root_identity: String,
    id: String,
}

impl Prepared {
    /// Prepare `input` on `open`, refusing a program Substrate's `/usr/bin/env` would read as a
    /// variable assignment or an option before anything starts.
    fn new(
        open: &OpenDriver,
        workspace: &str,
        input: &ExecStartInput,
    ) -> Result<Self, ConfinementError> {
        if let Some(program) = input.argv.first() {
            check_program(program)?;
        }
        let egress = open.egress.apertures.as_slice();
        let running = Running::enter(&open.cgroup_root);
        let profile = requested_profile(input, egress);
        let digest = profile_digest(
            &profile,
            &input.env.allow,
            egress,
            open.egress.ca_bundle.as_ref(),
            &input.read_only_files,
            input.workdir.as_deref(),
        );
        let root_identity = open
            .driver
            .workspace_root_identity(workspace)
            .map_err(|e| scope_error(format!("Substrate adopt: {e:?}")))?;
        let id = format!(
            "ex_loom_{}_{}",
            std::process::id(),
            NEXT_EXEC.fetch_add(1, Ordering::Relaxed)
        );
        Ok(Self {
            _running: running,
            digest,
            root_identity,
            id,
        })
    }
}

/// Refuse a start Substrate did not dispatch, or whose outcome it does not know.
fn dispatched(outcome: DispatchOutcome<ExecObservation>) -> Result<(), ConfinementError> {
    let detail = match outcome {
        DispatchOutcome::Observed(_) => return Ok(()),
        DispatchOutcome::NotDispatched(e) => format!("Substrate exec not dispatched: {e:?}"),
        DispatchOutcome::ContainedAbsent(e) => format!("Substrate exec contained absent: {e:?}"),
        DispatchOutcome::OutcomeUnknown(e) => {
            format!("Substrate exec outcome unknown; do not retry blindly: {e:?}")
        }
    };
    Err(refuse(ConfinementRefusal::CapabilityUnserved, detail))
}

fn observation_error(e: substrate_host::DriverError) -> ConfinementError {
    refuse(
        ConfinementRefusal::CapabilityUnserved,
        format!("Substrate observation: {e:?}"),
    )
}

/// How Substrate observed one execution end: its record of the applied confinement, its state,
/// exit and refusal, and its two streams as it kept them.
struct Ending {
    record: Option<substrate_wire::AppliedConfinement>,
    state: ExecState,
    exit: Option<ExecExit>,
    refusal: Option<ExecRefusal>,
    output: ChildOutput,
    output_complete: bool,
}

impl From<ExecObservation> for Ending {
    fn from(observed: ExecObservation) -> Self {
        Self {
            record: observed.resource.applied,
            state: observed.resource.state,
            exit: observed.resource.exit,
            refusal: observed.resource.refusal,
            output: ChildOutput {
                stdout: observed.stdout,
                stderr: observed.stderr,
                stdout_truncated: observed.stdout_truncated,
                stderr_truncated: observed.stderr_truncated,
            },
            output_complete: observed.output_complete,
        }
    }
}

/// Accept `ending`, an execution of `input`, only when the confinement Substrate applied is the
/// one `input` required, with the egress apertures the driver declares when `input` asks for them,
/// and project it: the applied confinement (with `digest`, the requested profile's), how it
/// ended, and its output, the tail of each stream as text and both whole. Every launch, a child
/// run to its end and a live child alike, is checked here once it ended.
fn settle(
    input: &ExecStartInput,
    egress: &[EgressAperture],
    digest: String,
    ending: Ending,
) -> Result<Launched, ConfinementError> {
    let record = ending.record.ok_or_else(|| {
        refuse(
            ConfinementRefusal::CapabilityUnserved,
            "Substrate returned no applied confinement",
        )
    })?;
    check_record(input, egress, &record)?;
    check_entered_workdir(input, &ending.output)?;
    let timed_out = ending
        .refusal
        .as_ref()
        .is_some_and(|r| r.code == "exec.timeout");
    let exit_code =
        if ending.refusal.is_some() || ending.state != ExecState::Exited || !ending.output_complete
        {
            None
        } else {
            ending.exit.as_ref().and_then(|e| e.code).map(i32::from)
        };
    let applied = AppliedConfinement {
        backend: Backend::Substrate,
        profile_digest: digest,
        capability_snapshot: record.capability_snapshot.clone(),
        cgroup: record.cgroup.clone(),
        filesystem: enum_name(&record.filesystem),
        network: Some(applied_apertures(&record.network)),
        profile: enum_name(&record.profile),
        writable_scopes: record
            .workspace_access
            .writable_subtrees()
            .unwrap_or_default()
            .to_vec(),
        read_only_roots: roots_projection(&record.read_only_roots),
        read_only_files: files_projection(&record.read_only_files),
        workdir: record.workdir.clone(),
        // Named by the driver that settled the launch (`OpenDriver::adapted`); absent until then.
        adapter: None,
        facts: None,
    };
    let described = format!(
        "state {}, refusal {}, output complete {}",
        enum_name(&ending.state),
        ending.refusal.as_ref().map_or("none", |r| r.code.as_str()),
        ending.output_complete
    );
    let streams = ending.output;
    let mut output = format!(
        "{}{}",
        String::from_utf8_lossy(tail(&streams.stdout)),
        String::from_utf8_lossy(tail(&streams.stderr))
    );
    if streams.stdout_truncated || streams.stderr_truncated {
        output.push_str("\n[Substrate output truncated]\n");
    }
    let refusal = ending.refusal.map(|refusal| {
        output.push_str(&format!("\n{}: {}\n", refusal.code, refusal.message));
        refusal.code
    });
    let exit = ChildExit {
        exit_code: exit_code.map(i64::from),
        signal: ending
            .exit
            .and_then(|e| e.signal)
            .map(|s| i64::from(s.number())),
        timed_out,
    };
    Ok(Launched {
        execution: TestExecution {
            exit_code,
            timed_out,
            output,
            applied,
            driver_record: Some(record),
            child_output: streams,
        },
        ending: described,
        refusal,
        exit,
    })
}

/// What one launch returned, how Substrate observed its end, the code of the refusal it ended
/// with, if any, and how the child ended.
struct Launched {
    execution: TestExecution,
    ending: String,
    refusal: Option<String>,
    exit: ChildExit,
}

/// Whether an aperture of `network` declares a byte ceiling.
fn declares_a_ceiling(network: &AppliedNetwork) -> bool {
    match network {
        AppliedNetwork::None => false,
        AppliedNetwork::Aperture(aperture) => aperture.max_bytes.is_some(),
        AppliedNetwork::Apertures(apertures) => apertures
            .iter()
            .any(|aperture| aperture.max_bytes.is_some()),
    }
}

/// The refusal Substrate ends a run with once an egress aperture's declared byte ceiling is
/// reached, as it ends one at its deadline with `exec.timeout`.
const BYTE_LIMIT: &str = "exec.aperture-byte-limit";

/// A child's result: refused when Substrate reports no exit status, unless the child's deadline or
/// the byte ceiling of an egress aperture it ran with ended it, since such a child was ended by
/// something other than itself or a bound the host declared. A child its byte ceiling ended has
/// no exit status and is not timed out; its output names [`BYTE_LIMIT`] and its driver record
/// counts the bytes that crossed.
fn child_result(launched: Launched) -> Result<TestExecution, ConfinementError> {
    let execution = launched.execution;
    // Only a ceiling the record shows applied: the record already equals the declaration.
    let ceiling_ended = launched.refusal.as_deref() == Some(BYTE_LIMIT)
        && execution
            .driver_record
            .as_ref()
            .is_some_and(|record| declares_a_ceiling(&record.network));
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

/// A live child's result, once it ended: a child the host signalled that the signal ended is a
/// result with no exit status, naming the signal; any other end is judged as a child run to its
/// end is ([`child_result`]). The driver's record is required, as [`settle`] already checked it.
fn live_result(launched: Launched, signalled: bool) -> Result<ChildResult, ConfinementError> {
    let exit = launched.exit.clone();
    let ended_by_host = signalled && launched.refusal.is_none() && exit.signal.is_some();
    let execution = if ended_by_host {
        launched.execution
    } else {
        child_result(launched)?
    };
    let driver_record = execution.driver_record.ok_or_else(|| {
        refuse(
            ConfinementRefusal::CapabilityUnserved,
            "Substrate returned no applied confinement",
        )
    })?;
    Ok(ChildResult {
        exit,
        output: execution.child_output,
        applied: execution.applied,
        driver_record,
    })
}

impl TestRunner for SubstrateRunner {
    fn backend(&self) -> Backend {
        Backend::Substrate
    }

    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        check_program(&command.program().to_string_lossy())?;
        // A test writes only under `target`: no host names it another writable path.
        let root = validate_workspace(root, &[])?;
        let open = self.driver(&root, &DriverEgress::NONE)?;
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
/// A child that must reach a host, such as a harness and its vendor's API, names it as a
/// Substrate [`EgressAperture`] it may reach ([`ChildCommand::with_egress`], once per host, at
/// most [`MAX_EGRESS_APERTURES`]): its network is then those apertures alone, every other
/// guarantee kept. A child reaching such a host over TLS also names the certificate bundle it
/// trusts there
/// ([`ChildCommand::with_ca_bundle`]); without one it cannot verify the host's certificate.
///
/// Beside `/usr`, `/bin`, `/lib`, `/lib64`, `/proc`, `/dev`, a private `/tmp` and its workspace,
/// a child sees only the host directories and files the host names, read-only, each at the mount
/// the host names ([`ChildCommand::with_read_only_root`], [`ChildCommand::with_read_only_file`])
/// or at its own host path ([`ChildCommand::at_host_path`]). [`ChildCommand::sandbox_path`] says
/// where a host path is inside. The child starts in `/workspace` unless the host names another
/// directory of the workspace or of a read-only root ([`ChildCommand::with_workdir`]). Loom requests
/// no writable path outside the workspace, though Substrate 0.10.0 offers writable roots: a directory
/// outside it, such as a linked worktree's common git directory, can be given read-only only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildCommand {
    program: String,
    args: Vec<String>,
    directory: PathBuf,
    writable: Vec<PathBuf>,
    environment: ExecEnvironment,
    profile: SandboxProfile,
    network: NetworkMode,
    egress: Vec<EgressAperture>,
    ca_bundle: Option<PathBuf>,
    timeout: Duration,
    stdin: Option<Vec<u8>>,
    read_only_roots: Vec<ReadOnlyMount>,
    read_only_files: Vec<ReadOnlyMount>,
    host_paths: Vec<PathBuf>,
    workdir: Option<PathBuf>,
}

/// A host directory or file a [`ChildCommand`] reads, and where the child sees it: its host path
/// and its mount, both as the host named them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOnlyMount {
    pub host_path: PathBuf,
    pub mount: PathBuf,
}

impl ChildCommand {
    /// How long a child may run unless [`ChildCommand::with_timeout`] says otherwise.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

    /// The most bytes a child takes on its standard input, given through
    /// [`ChildCommand::with_stdin`] or written to a live child through
    /// [`ConfinedChild::write_stdin`]: 16 MiB, the bound Substrate's daemon admits for a pipe
    /// session.
    pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;

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
            egress: Vec::new(),
            ca_bundle: None,
            timeout: Self::DEFAULT_TIMEOUT,
            stdin: None,
            read_only_roots: Vec::new(),
            read_only_files: Vec::new(),
            host_paths: Vec::new(),
            workdir: None,
        }
    }

    /// At most this many read-only roots a child is given, the roots
    /// [`ChildCommand::at_host_path`] names included: Substrate's bound.
    pub const MAX_READ_ONLY_ROOTS: usize = MAX_READ_ONLY_ROOTS as usize;

    /// At most this many read-only files a child is given, the files
    /// [`ChildCommand::at_host_path`] names included: Substrate's bound.
    pub const MAX_READ_ONLY_FILES: usize = MAX_READ_ONLY_FILES as usize;

    /// The same child also reading the host directory `host_path`, and everything under it, at
    /// `mount`, read-only. Bubblewrap binds the directory by its path when the child starts.
    ///
    /// Refused [`ConfinementRefusal::ScopeInvalid`] before anything starts or `target` is created,
    /// naming the path:
    ///
    /// - a host path or mount that is not UTF-8, not absolute, or not canonical (a `.`, `..` or
    ///   empty component, a trailing `/`, more than 64 components);
    /// - a host path that does not exist, is no directory, or is reached through a symbolic link
    ///   in any component;
    /// - a mount on, under or containing one Substrate owns: `/`, `/usr`, `/bin`, `/lib`, `/lib64`,
    ///   `/proc`, `/dev`, `/tmp`, `/workspace`, `/runtime`, `/scratch`, `/etc/hosts` and
    ///   `/etc/ssl/certs/ca-certificates.crt`, so no mount shadows the workspace or the
    ///   certificate bundle a child trusts;
    /// - a mount equal to, under or containing another root's or file's mount (`/opt/a` and
    ///   `/opt/ab` do not overlap);
    /// - more than [`ChildCommand::MAX_READ_ONLY_ROOTS`] roots;
    /// - a host path that is, holds or lies inside the workspace, or holds the certificate bundle
    ///   the child names ([`ChildCommand::with_ca_bundle`]) or the directory the driver's private
    ///   state is made in ([`std::env::temp_dir`]), and, once the driver opened, one that is,
    ///   holds or lies inside that state directory.
    ///
    /// The applied confinement must name exactly the roots and files requested, or the result is
    /// refused.
    #[must_use]
    pub fn with_read_only_root(
        mut self,
        host_path: impl Into<PathBuf>,
        mount: impl Into<PathBuf>,
    ) -> Self {
        self.read_only_roots.push(ReadOnlyMount {
            host_path: host_path.into(),
            mount: mount.into(),
        });
        self
    }

    /// The same child also reading the host file `host_path`, alone, at `mount`, read-only.
    /// Substrate opens the file without following a symbolic link in any component and binds what
    /// it opened, so nothing else of its directory is visible. Refused as
    /// [`ChildCommand::with_read_only_root`] refuses a root, with a regular file in place of a
    /// directory, a path bounded at 4000 bytes and each component at 255, and at most
    /// [`ChildCommand::MAX_READ_ONLY_FILES`] files; and refused
    /// [`ConfinementRefusal::CapabilityUnserved`] on a driver that does not publish
    /// `exec.read-only-files` (it proves its mechanism when it opens).
    #[must_use]
    pub fn with_read_only_file(
        mut self,
        host_path: impl Into<PathBuf>,
        mount: impl Into<PathBuf>,
    ) -> Self {
        self.read_only_files.push(ReadOnlyMount {
            host_path: host_path.into(),
            mount: mount.into(),
        });
        self
    }

    /// The same child reading `host_path` read-only at that same path, so a host path in its
    /// arguments or configuration resolves inside: a read-only root when it is a directory, a
    /// read-only file when it is a regular file, decided on disk at each launch. `host_path`
    /// must be canonical, as both are; anything else is refused as they refuse it, naming the
    /// path, and so is a host path that is neither a directory nor a regular file.
    #[must_use]
    pub fn at_host_path(mut self, host_path: impl Into<PathBuf>) -> Self {
        self.host_paths.push(host_path.into());
        self
    }

    /// The same child, also writing under `path`, a directory of the workspace named relative to
    /// it: a scratch copy the child edits, `.git` for a child that commits, or a command's state
    /// directory. Everything else in the workspace stays read-only, `.git` included unless the
    /// host names it; a child that may write `.git` may also plant hooks or configuration that a
    /// later git call on the host runs. Substrate makes only directories writable, so a file at
    /// the workspace root cannot be named. Naming `target`, or a path twice, changes nothing.
    ///
    /// Refused [`ConfinementRefusal::ScopeInvalid`] before anything starts or `target` is created,
    /// naming the path: a path
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

    /// The same child with exactly `environment` and no other variable, except `SSL_CERT_FILE`
    /// when it names a certificate bundle ([`ChildCommand::with_ca_bundle`]). Beside a bundle the
    /// environment may not name the variables `with_ca_bundle` lists, so a host does not override
    /// by accident the bundle it named; that is no guarantee about what the child trusts.
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

    /// The same child also reaching `aperture`, and nothing but the apertures named:
    /// [`NetworkMode::Aperture`], with the host, port and pinned address `aperture` declares. Each
    /// call adds one aperture, in order; a child reaches at most [`MAX_EGRESS_APERTURES`]. The child
    /// reaches each by its host's name, which Substrate maps to that aperture's forwarder inside
    /// the sandbox, at the named port: the first on `127.0.0.1`, the n-th on `127.0.0.n`. A
    /// declaration Substrate's daemon never makes, or one the child could not reach by, is refused
    /// before anything starts: a name it refuses, no port, a byte ceiling of zero, a host that is
    /// no host name or IPv4 literal, an IPv4 literal host other than `127.0.0.1` (named in the
    /// refusal; name such a host by its name), an IPv4 literal other than the pinned address, and
    /// a pinned address that is no IPv4 one or not at the named port. Beside those, a set is
    /// refused when it holds more than [`MAX_EGRESS_APERTURES`] apertures (naming the count), one
    /// name twice, a second or later aperture whose host is an IPv4 literal or `localhost`, which
    /// only the first one's forwarder answers (naming the host), or two apertures naming one host,
    /// compared without ASCII case and a trailing `.` (naming it). A child a byte ceiling ends is a
    /// result with no exit status, not timed out, whose output names `exec.aperture-byte-limit`. It
    /// gets no certificate bundle unless the host names one ([`ChildCommand::with_ca_bundle`]):
    /// without one, a child that verifies TLS certificates cannot verify the hosts'.
    #[must_use]
    pub fn with_egress(self, aperture: EgressAperture) -> Self {
        let mut egress = self.egress;
        egress.push(aperture);
        Self {
            network: NetworkMode::Aperture,
            egress,
            ..self
        }
    }

    /// The same child trusting the certificate bundle at `path`, a file of the host, where it
    /// reaches its egress aperture ([`ChildCommand::with_egress`]) over TLS. Each launch reads the
    /// file once, through an open with its last component not followed, and the child trusts the
    /// bytes read then: the runner opens the driver with a private read-only copy of them
    /// (Substrate's `HostConfig::ca_bundle`), never with the named path, so a link or another file
    /// put at that path after the read is never read. Substrate binds a snapshot of that copy, taken when
    /// the child starts, at `/etc/ssl/certs/ca-certificates.crt` and sets `SSL_CERT_FILE` to that
    /// path, the one variable the child gets beside its environment.
    ///
    /// The bundle is part of the shared driver's key, as the path and the bytes read: children
    /// naming different bundles, one naming a bundle and one naming none, and a bundle whose
    /// bytes changed since a driver opened for it, never share a driver. Like any other new
    /// driver, the one a changed bundle needs cannot open while a launch of this process runs
    /// under the same cgroup root, and the launch is refused
    /// [`ConfinementRefusal::CapabilityUnserved`]. Substrate's record of the applied confinement
    /// names no bundle, so the bundle is not checked after the child ran.
    ///
    /// The bundle is the trust store the child gets through `SSL_CERT_FILE`. Beside it, the
    /// environment may not name the variables listed below, so a host does not override by
    /// accident the bundle it named. That is no guarantee about what the child trusts:
    /// configuration under `HOME`, a program's own flags and variables not listed can still point
    /// a client elsewhere or turn verification off. What the child reaches is bounded by its
    /// aperture.
    ///
    /// Refused [`ConfinementRefusal::ScopeInvalid`] before anything starts or `target` is created,
    /// naming the path:
    ///
    /// - a bundle for a child with no egress aperture, which reaches no host to verify;
    /// - a path that is not UTF-8 or not absolute;
    /// - beside an environment naming, each named in the refusal, `SSL_CERT_FILE`,
    ///   `SSL_CERT_DIR`, `CURL_CA_BUNDLE`, `CURL_CA_PATH`, `CURL_HOME`, `REQUESTS_CA_BUNDLE`,
    ///   `NODE_EXTRA_CA_CERTS`, `GIT_SSL_CAINFO`, `GIT_SSL_CAPATH`, `GIT_SSL_NO_VERIFY`,
    ///   `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM` or `GIT_CONFIG_PARAMETERS`, or a
    ///   `GIT_CONFIG_KEY_<n>` naming Git's `http.sslCAInfo`, `http.sslCAPath` or `http.sslVerify`
    ///   (in any case, with or without a URL subsection); Git configuration naming other keys
    ///   is allowed;
    /// - a path that does not exist, is a symbolic link, is no regular file, is empty or holds
    ///   more than 1 MiB, checked on the path and again on the file the launch opens, as Substrate
    ///   refuses its own bundle (name the file a link leads to);
    /// - a file this process cannot read.
    #[must_use]
    pub fn with_ca_bundle(self, path: impl Into<PathBuf>) -> Self {
        Self {
            ca_bundle: Some(path.into()),
            ..self
        }
    }

    /// The same child, killed once it has run for `timeout`.
    #[must_use]
    pub fn with_timeout(self, timeout: Duration) -> Self {
        Self { timeout, ..self }
    }

    /// The same child given `bytes` on its standard input by [`SubstrateRunner::run_child`]: it
    /// reads them once, then end of file. Without them its standard input is empty and closed.
    /// More than [`ChildCommand::MAX_INPUT_BYTES`] is refused
    /// [`ConfinementRefusal::ScopeInvalid`] before anything starts, naming the limit. A live child
    /// ([`SubstrateRunner::spawn_child`]) is written through [`ConfinedChild::write_stdin`]
    /// instead, and one named with standard input is refused.
    #[must_use]
    pub fn with_stdin(self, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            stdin: Some(bytes.into()),
            ..self
        }
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

    /// The egress apertures the child may reach, in the order named; empty when it names none.
    pub fn egress(&self) -> &[EgressAperture] {
        &self.egress
    }

    /// The certificate bundle the child trusts at its egress aperture, as named, if it names one.
    pub fn ca_bundle(&self) -> Option<&Path> {
        self.ca_bundle.as_deref()
    }

    /// The maximum duration of one run of the child.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// The bytes the child is given on its standard input, if the host gave it any.
    pub fn stdin(&self) -> Option<&[u8]> {
        self.stdin.as_deref()
    }

    /// The host directories the child reads at the mounts the host named, as named.
    pub fn read_only_roots(&self) -> &[ReadOnlyMount] {
        &self.read_only_roots
    }

    /// The host files the child reads at the mounts the host named, as named.
    pub fn read_only_files(&self) -> &[ReadOnlyMount] {
        &self.read_only_files
    }

    /// The host paths the child reads at their own path ([`ChildCommand::at_host_path`]), as
    /// named.
    pub fn host_paths(&self) -> &[PathBuf] {
        &self.host_paths
    }

    /// The same child, started in `workdir` instead of `/workspace`: a directory as the child sees
    /// it, at or beneath `/workspace` or the mount of a read-only root it reads
    /// ([`ChildCommand::with_read_only_root`], or a directory given by
    /// [`ChildCommand::at_host_path`], whose mount is its host path). A host holding a host path
    /// names it through [`ChildCommand::sandbox_path`]. Substrate's wire and its record carry the
    /// sandbox path, so the child is named by it too. Naming it again replaces the first.
    ///
    /// Refused [`ConfinementRefusal::ScopeInvalid`] before anything starts or `target` is created,
    /// naming the directory:
    ///
    /// - a path that is not UTF-8, longer than [`MAX_WORKDIR_BYTES`] (4096), not absolute, or not
    ///   lexically normal (a `.`, `..` or empty component, a trailing `/`), or holding a control
    ///   character, which the refusal escapes;
    /// - one at or beneath a read-only file's mount;
    /// - one not at or beneath `/workspace` or a read-only root's mount, `/`, `/scratch`,
    ///   `/runtime` and `/tmp` included: Loom asks Substrate for no scratch directory, execution
    ///   capsule or writable root;
    /// - one the host's disk does not hold as a directory reached through no symbolic link,
    ///   below the workspace directory or the root's host path, read before the launch;
    ///   `/workspace/target` alone may be absent, since the launch creates it;
    /// - one this process cannot search, or below a directory it cannot search: bubblewrap could
    ///   not enter it, and a run whose output carries bubblewrap's `Can't chdir to` diagnostic for
    ///   it is refused the same way once it ended, so a program that never ran reports no exit.
    ///
    /// Refused [`ConfinementRefusal::CapabilityUnserved`] on a driver that does not publish
    /// `exec.workdir`. Substrate checks the directory again when it starts the child, beneath the
    /// mount it binds, so one removed or swapped for a link after this check is refused there.
    /// The applied confinement names the working directory Substrate recorded
    /// ([`AppliedConfinement::workdir`]), and a record naming another one, or none, is refused.
    #[must_use]
    pub fn with_workdir(mut self, workdir: impl Into<PathBuf>) -> Self {
        self.workdir = Some(workdir.into());
        self
    }

    /// The working directory the child starts in, as named ([`ChildCommand::with_workdir`]);
    /// `None` is `/workspace`.
    pub fn workdir(&self) -> Option<&Path> {
        self.workdir.as_deref()
    }

    /// Where the child sees the host path `path`: a path inside the workspace directory at
    /// `/workspace/<relative>`, one inside a read-only root at
    /// the same place under its mount, a read-only file at its mount, and a path given at its
    /// own host path, or inside such a directory, at itself. Anything else is `None`: a relative
    /// path, one with a `..` component, one that leads through a symbolic link, a path beneath a
    /// file, and a path outside the workspace and every mount, one sharing only a prefix of a
    /// root's text (`/a/bc` beside the root `/a/b`) included. A path not created yet maps as its
    /// directory does.
    ///
    /// The answer holds for the disk as it was read here: a link put on the path later is the
    /// launch's to refuse for a mount, and inside the workspace the child resolves its links
    /// through its own mounts.
    pub fn sandbox_path(&self, path: &Path) -> Option<PathBuf> {
        if !path.is_absolute()
            || path.to_str().is_none()
            || path
                .components()
                .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
            || !no_link_on(path)
        {
            return None;
        }
        // The workspace as a launch adopts it: absolute, through no link, canonical.
        if let Ok(workspace) = canonical_workspace(&self.directory)
            && let Some(inside) = beneath(path, &workspace)
        {
            return Some(under(Path::new(SANDBOX_WORKSPACE), inside));
        }
        for root in &self.read_only_roots {
            if is_canonical_text(&root.host_path)
                && is_canonical_text(&root.mount)
                && let Some(inside) = beneath(path, &root.host_path)
            {
                return Some(under(&root.mount, inside));
            }
        }
        for file in &self.read_only_files {
            if is_canonical_text(&file.host_path)
                && is_canonical_text(&file.mount)
                && path == file.host_path
            {
                return Some(file.mount.clone());
            }
        }
        for host in &self.host_paths {
            if !is_canonical_text(host) {
                continue;
            }
            if path == host {
                return Some(path.to_owned());
            }
            let directory = std::fs::symlink_metadata(host).is_ok_and(|meta| meta.is_dir());
            if directory && beneath(path, host).is_some() {
                return Some(path.to_owned());
            }
        }
        None
    }

    /// The child as a Substrate raw-pipe session: the same execution as [`ChildCommand::input`],
    /// not waited for at its start, recorded, with pipes, at most
    /// [`ChildCommand::MAX_INPUT_BYTES`] of input in frames of at most 64 KiB, and at most 16
    /// frames of output queued, Substrate's bounds.
    fn session_input(&self, workspace: &str, snapshot: String) -> PipeSessionStartInput {
        PipeSessionStartInput {
            capture: CaptureMode::Recorded,
            exec: ExecStartInput {
                wait: false,
                ..self.input(workspace, snapshot)
            },
            input_limit_bytes: Self::MAX_INPUT_BYTES as u64,
            frame_limit_bytes: FRAME_BYTES as u64,
            queued_frames: QUEUED_FRAMES,
            mode: SessionMode::Pipes,
            window: None,
        }
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
                // Names only: the destination is the driver's declaration, never the request's.
                // One aperture is asked for as one, as before; several as a list, in order.
                aperture: match self.egress.as_slice() {
                    [only] => Some(only.name.clone()),
                    _ => None,
                },
                apertures: match self.egress.as_slice() {
                    [] | [_] => Vec::new(),
                    several => several
                        .iter()
                        .map(|aperture| aperture.name.clone())
                        .collect(),
                },
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
            // In the order named; `check_mounts` resolved every host path to a root or a file
            // and refused a path that is not UTF-8 before a launch builds this.
            read_only_roots: self
                .read_only_roots
                .iter()
                .map(|root| ReadOnlyRoot {
                    host_path: root.host_path.to_string_lossy().into_owned(),
                    mount: root.mount.to_string_lossy().into_owned(),
                })
                .collect(),
            read_only_files: self
                .read_only_files
                .iter()
                .map(|file| substrate_wire::ReadOnlyFile {
                    host_path: file.host_path.to_string_lossy().into_owned(),
                    mount: file.mount.to_string_lossy().into_owned(),
                })
                .collect(),
            secret_slots: Vec::new(),
            capsule: None,
            lease_ttl_ms: None,
            // `check_workdir` refused a working directory that is not UTF-8 before a launch builds
            // this.
            workdir: self
                .workdir
                .as_deref()
                .map(|workdir| workdir.to_string_lossy().into_owned()),
            writable_roots: Vec::new(),
        }
    }
}

/// The workspace inside the sandbox: where Substrate mounts the workspace and starts the child.
const SANDBOX_WORKSPACE: &str = "/workspace";

/// `inside` under `mount`: `mount` itself for the empty path, never with a trailing `/`.
fn under(mount: &Path, inside: &Path) -> PathBuf {
    if inside.as_os_str().is_empty() {
        mount.to_owned()
    } else {
        mount.join(inside)
    }
}

/// `path` below `ancestor`, component by component, so `/a/bc` is not below `/a/b`: the empty
/// path for `ancestor` itself, `None` for a path outside it.
fn beneath<'a>(path: &'a Path, ancestor: &Path) -> Option<&'a Path> {
    path.strip_prefix(ancestor).ok()
}

/// Whether no existing prefix of the absolute `path` is a symbolic link: a component not there
/// (yet) ends the walk, and one that cannot be read is no answer.
fn no_link_on(path: &Path) -> bool {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => return false,
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return true,
            Err(_) => return false,
        }
    }
    true
}

/// Whether `path` is UTF-8, absolute and canonical as Substrate's wire requires of a mount and a
/// host path: no `.`, `..` or empty component, no trailing `/`, no NUL, between 1 and 64
/// components.
fn is_canonical_text(path: &Path) -> bool {
    path.to_str()
        .is_some_and(|text| canonical_text(text).is_ok())
}

/// Why the path `text` is not absolute and canonical as Substrate's wire requires, if it is not.
fn canonical_text(text: &str) -> Result<(), &'static str> {
    if !text.starts_with('/') {
        return Err("is not absolute; name it by an absolute path");
    }
    if text.contains('\0') {
        return Err("holds a NUL");
    }
    if text.contains("//") || (text.len() > 1 && text.ends_with('/')) {
        return Err(
            "is not canonical: it has an empty component or a trailing `/`; name it without them",
        );
    }
    let components: Vec<&str> = text.split('/').skip(1).filter(|c| !c.is_empty()).collect();
    if components.iter().any(|c| *c == "." || *c == "..") {
        return Err("is not canonical: it has a `.` or `..` component; name it without them");
    }
    if components.is_empty() {
        return Err("names the root `/`");
    }
    if components.len() > MAX_PATH_DEPTH {
        return Err("is not canonical: it has more than the 64 components Substrate admits");
    }
    Ok(())
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

/// Refuse more standard input than a pipe session admits ([`ChildCommand::MAX_INPUT_BYTES`]),
/// naming the limit, before anything starts.
fn check_stdin(stdin: Option<&[u8]>) -> Result<(), ConfinementError> {
    match stdin {
        Some(bytes) if bytes.len() > ChildCommand::MAX_INPUT_BYTES => Err(scope_error(format!(
            "standard input of {} bytes exceeds the {} bytes a Substrate pipe session admits",
            bytes.len(),
            ChildCommand::MAX_INPUT_BYTES
        ))),
        _ => Ok(()),
    }
}

/// Refuse a live child on a driver that does not publish what a confined pipe session needs:
/// every namespace, the process, memory and cpu limits of its cgroup, a cgroup kill, no egress,
/// an output limit, and the three signals the host may send. These are the facts Substrate's
/// daemon requires before it serves a pipe session, except explicit leases, which a host driver
/// does not hold.
fn check_pipe_sessions(facts: &CapabilityFacts) -> Result<(), ConfinementError> {
    let namespaces = facts.exec_namespaces.as_ref().is_some_and(|namespaces| {
        namespaces.user
            && namespaces.mount
            && namespaces.pid
            && namespaces.ipc
            && namespaces.uts
            && namespaces.network
    });
    let limits = facts
        .exec_cgroup_limits
        .as_ref()
        .is_some_and(|limits| limits.processes && limits.memory && limits.cpu);
    let signals = facts.exec_signals.as_ref().is_some_and(|signals| {
        [Signal::Int, Signal::Term, Signal::Kill]
            .iter()
            .all(|signal| signals.contains(signal))
    });
    if namespaces
        && limits
        && signals
        && facts.exec_cgroup_kill == Some(true)
        && facts.exec_no_egress == Some(true)
        && facts.exec_output_limit_bytes.is_some_and(|bytes| bytes > 0)
    {
        Ok(())
    } else {
        Err(refuse(
            ConfinementRefusal::CapabilityUnserved,
            "Substrate does not serve a confined pipe session on this machine: it needs every namespace, cgroup limits and kill, no egress, an output limit and the interrupt, terminate and kill signals",
        ))
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
/// are held here, for each aperture of a set.
///
/// A set of apertures is refused, too, when it holds more than [`MAX_EGRESS_APERTURES`] (naming
/// the count), one name twice, a second or later aperture whose host is an IPv4 literal or
/// `localhost` (naming the host: only the first aperture's forwarder listens on `127.0.0.1`, so
/// a later one is reached by its host name alone), or two apertures naming one host, compared
/// without ASCII case and a trailing `.` (naming it): Substrate maps each host name to one
/// forwarder in the child's `/etc/hosts`.
fn check_network(network: NetworkMode, egress: &[EgressAperture]) -> Result<(), ConfinementError> {
    if egress.is_empty() {
        return match network {
            NetworkMode::None => Ok(()),
            NetworkMode::Aperture => Err(scope_error(
                "network aperture names no host: name the egress aperture with ChildCommand::with_egress",
            )),
        };
    }
    let most = usize::try_from(MAX_EGRESS_APERTURES).unwrap_or(usize::MAX);
    if egress.len() > most {
        let named = egress
            .iter()
            .map(|aperture| format!("`{}` ({})", aperture.name, aperture.host))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(scope_error(format!(
            "{} egress apertures are named, {named}; a child reaches at most {MAX_EGRESS_APERTURES}",
            egress.len()
        )));
    }
    for (index, aperture) in egress.iter().enumerate() {
        if let Some(earlier) = egress[..index]
            .iter()
            .find(|earlier| earlier.name == aperture.name)
        {
            return Err(scope_error(format!(
                "egress aperture `{}` is named twice, for the hosts {} and {}: Substrate declares each aperture name once",
                aperture.name, earlier.host, aperture.host
            )));
        }
        check_aperture(network, aperture, index)?;
        let host = host_key(&aperture.host);
        if let Some(earlier) = egress[..index]
            .iter()
            .find(|earlier| host_key(&earlier.host) == host)
        {
            return Err(scope_error(format!(
                "egress apertures `{}` and `{}` both name the host {host}: Substrate maps a host name to one aperture",
                earlier.name, aperture.name
            )));
        }
    }
    Ok(())
}

/// A host as Substrate compares the hosts of a set of apertures: ASCII lowercase, with no trailing
/// `.`.
fn host_key(host: &str) -> String {
    host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase()
}

/// Refuse the aperture at `index` of a set unless Substrate's daemon declares it so and the child
/// can reach it: the rules of [`check_network`] for one aperture, and for a second or later one a
/// host that is no IPv4 literal and not `localhost`.
fn check_aperture(
    network: NetworkMode,
    aperture: &EgressAperture,
    index: usize,
) -> Result<(), ConfinementError> {
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
    if index > 0 {
        let host = aperture.host.strip_suffix('.').unwrap_or(&aperture.host);
        if numeric_host(host) || host.eq_ignore_ascii_case("localhost") {
            return Err(scope_error(format!(
                "egress aperture `{name}` names the host {}: only the first aperture listens on {APERTURE_LOOPBACK_ADDRESS}, so a later one is reached by its host name alone; name the host by its name",
                aperture.host
            )));
        }
    }
    check_host(name, &aperture.host, *pinned.ip())
}

/// Whether the C resolver reads `host` as an IPv4 address rather than a name: one to four
/// dot-separated labels (a trailing `.` ignored), each a number as `inet_aton` reads one, decimal,
/// `0x` or `0X` hexadecimal, or octal with a leading `0`. glibc's `getaddrinfo` parses such a host
/// numerically before it reads `/etc/hosts`, so `0x7f000001` or `0177.0.0.1` reaches `127.0.0.1`
/// whatever Substrate maps the name to there. A label holding digits an octal or decimal number
/// cannot hold counts as numeric too: such a host is refused either way.
fn numeric_host(host: &str) -> bool {
    let host = host.strip_suffix('.').unwrap_or(host);
    let labels = host.split('.').count();
    (1..=4).contains(&labels)
        && host.split('.').all(|label| {
            match label
                .strip_prefix("0x")
                .or_else(|| label.strip_prefix("0X"))
            {
                Some(hex) => hex.bytes().all(|byte| byte.is_ascii_hexdigit()),
                None => !label.is_empty() && label.bytes().all(|byte| byte.is_ascii_digit()),
            }
        })
}

/// Refuse an aperture's host unless it is a declaration Substrate's daemon makes for an address
/// pinned at `pinned` and one the child can reach by: a host name, dot-separated labels of ASCII
/// letters, digits and inner hyphens, at most 63 bytes each and 253 in all, the last not all
/// digits and not every one a number ([`numeric_host`]), or exactly the IPv4 literal `127.0.0.1`
/// pinned there. Anything else (whitespace, a second line, a port, a URL, an IPv6 literal) never
/// resolves to an IPv4 address there. Any other numeric host, in whatever spelling `inet_aton`
/// reads (`10.0.0.1`, `0x0a000001`, `0177.0.0.1`), is refused too: Substrate maps only a host name
/// to the forwarder it starts on [`APERTURE_LOOPBACK_ADDRESS`] inside the sandbox, so a child
/// dialling any other address would never reach the host; a literal also resolves to itself, so it
/// must be `pinned`.
fn check_host(name: &str, host: &str, pinned: Ipv4Addr) -> Result<(), ConfinementError> {
    if numeric_host(host) {
        let forwarder: Ipv4Addr = APERTURE_LOOPBACK_ADDRESS
            .parse()
            .expect("Substrate's forwarder address is an IPv4 literal");
        return if host != APERTURE_LOOPBACK_ADDRESS {
            Err(scope_error(format!(
                "egress aperture `{name}` names the host {host}: the resolver reads it as an IPv4 address, and a child reaches an aperture by its host name, or at {forwarder} spelled so, where Substrate's forwarder listens in the sandbox; name the host by its name"
            )))
        } else if forwarder != pinned {
            Err(scope_error(format!(
                "egress aperture `{name}` names the address {forwarder} but is pinned to {pinned}"
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

/// The environment variables through which a TLS client takes another trust store than the bundle
/// Substrate points `SSL_CERT_FILE` at, replaces it, adds to it or turns verification off, by the
/// tools that read them: OpenSSL (`SSL_CERT_FILE`, which Substrate itself sets to the snapshot at
/// [`APERTURE_CA_BUNDLE_PATH`], and `SSL_CERT_DIR`), curl (`CURL_CA_BUNDLE`, `CURL_CA_PATH`, and
/// `CURL_HOME`, where it reads a `.curlrc` that may name a `cacert`), Python's requests
/// (`REQUESTS_CA_BUNDLE`), Node (`NODE_EXTRA_CA_CERTS`) and Git (`GIT_SSL_CAINFO`,
/// `GIT_SSL_CAPATH`, `GIT_SSL_NO_VERIFY`, and `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM` and
/// `GIT_CONFIG_PARAMETERS`, through which it takes configuration that may set `http.sslCAInfo`).
/// A child naming a bundle is refused when its environment names any of them, or a
/// `GIT_CONFIG_KEY_<n>` naming one of [`GIT_TRUST_KEYS`] ([`check_ca_bundle`]).
///
/// The list keeps a host from overriding, by accident, the bundle it names through the
/// environment it names beside it. It is no guarantee about what the child trusts: configuration
/// under `HOME`, a program's own flags and variables not listed can still point a client
/// elsewhere or turn verification off; what the child reaches is bounded by its aperture.
const TRUST_STORE_VARIABLES: [&str; 13] = [
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "CURL_CA_BUNDLE",
    "CURL_CA_PATH",
    "CURL_HOME",
    "REQUESTS_CA_BUNDLE",
    "NODE_EXTRA_CA_CERTS",
    "GIT_SSL_CAINFO",
    "GIT_SSL_CAPATH",
    "GIT_SSL_NO_VERIFY",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_PARAMETERS",
];

/// The keys of Git's `http` section through which Git takes another trust store or turns
/// verification off. A `GIT_CONFIG_KEY_<n>` naming one, the section and the key in any case and
/// with or without a URL subsection (`http.<url>.sslCAInfo`), is refused beside a bundle
/// ([`names_git_trust_key`]); one naming any other key (`safe.directory`) is not.
const GIT_TRUST_KEYS: [&str; 3] = ["sslCAInfo", "sslCAPath", "sslVerify"];

/// Whether an environment variable `name` set to `value`, beside a certificate bundle, would
/// override the trust store the child gets: a name in [`TRUST_STORE_VARIABLES`], or a
/// `GIT_CONFIG_KEY_<n>` (`n` decimal digits, as Git reads them) whose value names one of
/// [`GIT_TRUST_KEYS`].
fn overrides_trust_store(name: &str, value: &str) -> bool {
    let git_config_key = name
        .strip_prefix("GIT_CONFIG_KEY_")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|byte| byte.is_ascii_digit()));
    TRUST_STORE_VARIABLES.contains(&name) || (git_config_key && names_git_trust_key(value))
}

/// Whether the Git configuration key `key` is one of [`GIT_TRUST_KEYS`] in the `http` section:
/// its first dot-separated part is `http` and its last one the key, both compared without case,
/// so `http.sslCAInfo`, `HTTP.SSLCAINFO` and `http.https://host.example/.sslCAInfo` all are.
fn names_git_trust_key(key: &str) -> bool {
    let key = key.trim();
    match (key.split_once('.'), key.rsplit_once('.')) {
        (Some((section, _)), Some((_, name))) => {
            section.eq_ignore_ascii_case("http")
                && GIT_TRUST_KEYS
                    .iter()
                    .any(|trust| name.eq_ignore_ascii_case(trust))
        }
        _ => false,
    }
}

/// The largest certificate bundle a child is given, 1 MiB: the ceiling Substrate holds its own
/// bundles to (`MAX_CA_BUNDLE_BYTES`, private to `b10x-substrate-sdk`'s transport and to
/// `substrate-daemon`'s hosted admission, so not importable here).
const MAX_CA_BUNDLE_BYTES: u64 = 1024 * 1024;

/// Refuse, before anything starts, a certificate bundle the child cannot be given as named
/// ([`ChildCommand::with_ca_bundle`]), each refusal naming the path, in this order, and otherwise
/// return the bundle as read: the bytes the child trusts.
///
/// 1. a bundle for a child with no egress aperture: Substrate gives a run the bundle only with
///    its aperture, and a child with none reaches no host the bundle could verify;
/// 2. a path that is not UTF-8, or not absolute, which names no one file whatever the working
///    directory;
/// 3. an environment naming any of [`TRUST_STORE_VARIABLES`] beside the bundle, or a
///    `GIT_CONFIG_KEY_<n>` naming one of [`GIT_TRUST_KEYS`] ([`overrides_trust_store`]), each
///    named in the refusal: it would override the trust store the child gets through
///    `SSL_CERT_FILE` (whose own value Substrate would replace unseen);
/// 4. the path itself, its last component not followed: one that does not exist, is a symbolic
///    link, is no regular file, is empty or holds more than [`MAX_CA_BUNDLE_BYTES`], as Substrate
///    refuses a bundle of its own; an empty one would trust nothing while the child is pointed
///    at it;
/// 5. the file, opened once with its last component not followed (`O_NOFOLLOW`) and without
///    waiting for a writer (`O_NONBLOCK`): the rules of step 4 again on that open file, so a link
///    put at the path since is refused as a link, then its bytes read through it, at most one past
///    the ceiling, refused when they cannot be read, are empty or pass the ceiling.
///
/// A child naming no bundle passes with `None`.
fn check_ca_bundle(child: &ChildCommand) -> Result<Option<CaBundle>, ConfinementError> {
    use std::io::Read as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let Some(path) = child.ca_bundle.as_deref() else {
        return Ok(None);
    };
    // The path as every refusal names it: verbatim in backticks, or escaped when not UTF-8.
    let named = path
        .to_str()
        .map_or_else(|| format!("{path:?}"), |text| format!("`{text}`"));
    let refused = |why: &str| scope_error(format!("the CA bundle {named} {why}"));
    if child.egress.is_empty() {
        return Err(refused(
            "is named for a child with no egress aperture, which reaches no host to verify; name the aperture with ChildCommand::with_egress",
        ));
    }
    if path.to_str().is_none() {
        return Err(refused("is not UTF-8"));
    }
    if !path.is_absolute() {
        return Err(refused(
            "is a relative path; name it by an absolute path, so it names one file whatever the working directory",
        ));
    }
    // In the environment's order (by name); a Git key is named with the key it sets.
    let variables: Vec<String> = child
        .environment
        .set
        .iter()
        .filter(|(name, value)| overrides_trust_store(name, value))
        .map(|(name, value)| {
            if TRUST_STORE_VARIABLES.contains(&name.as_str()) {
                name.clone()
            } else {
                format!("{name} ({value})")
            }
        })
        .collect();
    if !variables.is_empty() {
        return Err(refused(&format!(
            "comes with an environment naming {}, which would override the trust store the child gets through SSL_CERT_FILE, the bundle's snapshot at {APERTURE_CA_BUNDLE_PATH}; leave {} out of the environment",
            variables.join(", "),
            if variables.len() == 1 { "it" } else { "them" }
        )));
    }
    let link = "is a symbolic link, which Substrate's daemon refuses as a bundle; name the file it leads to";
    let empty = "is empty: it would trust nothing, and Substrate's daemon refuses an empty bundle";
    // Step 4's rules, held on the path and again on the file opened.
    let bundle_file = |metadata: &std::fs::Metadata| {
        if metadata.file_type().is_symlink() {
            Err(refused(link))
        } else if !metadata.is_file() {
            Err(refused(
                "is no regular file (a directory, socket, device or FIFO)",
            ))
        } else if metadata.len() == 0 {
            Err(refused(empty))
        } else if metadata.len() > MAX_CA_BUNDLE_BYTES {
            Err(refused(&format!(
                "holds {} bytes, more than the {MAX_CA_BUNDLE_BYTES} (1 MiB) Substrate admits for a certificate bundle",
                metadata.len()
            )))
        } else {
            Ok(())
        }
    };
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => bundle_file(&metadata)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(refused("does not exist"));
        }
        Err(e) => return Err(refused(&format!("cannot be inspected: {e}"))),
    }
    let unreadable = |e: std::io::Error| refused(&format!("cannot be read: {e}"));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| match e.raw_os_error() {
            Some(libc::ELOOP) => refused(link),
            _ if e.kind() == std::io::ErrorKind::NotFound => refused("does not exist"),
            _ => unreadable(e),
        })?;
    bundle_file(&file.metadata().map_err(unreadable)?)?;
    let mut bytes = Vec::new();
    file.take(MAX_CA_BUNDLE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(unreadable)?;
    if bytes.is_empty() {
        return Err(refused(empty));
    }
    if bytes.len() as u64 > MAX_CA_BUNDLE_BYTES {
        return Err(refused(&format!(
            "grew past the {MAX_CA_BUNDLE_BYTES} bytes (1 MiB) Substrate admits for a certificate bundle while it was read"
        )));
    }
    Ok(Some(CaBundle {
        named: path.to_owned(),
        bytes,
    }))
}

/// Whether a read-only mount is a directory or a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MountKind {
    Root,
    File,
}

impl MountKind {
    fn noun(self) -> &'static str {
        match self {
            Self::Root => "read-only root",
            Self::File => "read-only file",
        }
    }
}

/// One read-only mount as a launch checks it: what it is (`None` for a host path given at its own
/// path, until the disk says), its host path and its mount, both UTF-8.
struct Mount<'a> {
    kind: Option<MountKind>,
    host: &'a str,
    mount: &'a str,
}

impl Mount<'_> {
    /// The mount as every refusal names it: its kind and its host path.
    fn label(&self) -> String {
        let noun = self.kind.map_or("host path", MountKind::noun);
        format!("{noun} `{}`", self.host)
    }
}

/// The mounts Substrate owns, which no read-only root or file may equal, lie under or contain:
/// Substrate's reserved mounts, the execution capsule, the scratch directory, the hosts file and
/// the certificate bundle path of an egress aperture.
const OWNED_MOUNTS: [&str; 13] = [
    RESERVED_MOUNTS[0],
    RESERVED_MOUNTS[1],
    RESERVED_MOUNTS[2],
    RESERVED_MOUNTS[3],
    RESERVED_MOUNTS[4],
    RESERVED_MOUNTS[5],
    RESERVED_MOUNTS[6],
    RESERVED_MOUNTS[7],
    RESERVED_MOUNTS[8],
    EXECUTION_CAPSULE_MOUNT,
    EXEC_SCRATCH_MOUNT,
    APERTURE_HOSTS_PATH,
    APERTURE_CA_BUNDLE_PATH,
];

/// Whether the path text `path` lies below `ancestor`, component by component: `/a/bc` is not
/// below `/a/b`.
fn text_beneath(path: &str, ancestor: &str) -> bool {
    path.strip_prefix(ancestor)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// Whether the path texts `a` and `b` are the same path or one lies below the other.
fn text_overlaps(a: &str, b: &str) -> bool {
    a == b || text_beneath(a, b) || text_beneath(b, a)
}

/// The workspace `root` named by a launch, absolute, through no symbolic link, canonical.
fn canonical_workspace(root: &Path) -> Result<PathBuf, ConfinementError> {
    let absolute = if root.is_absolute() {
        root.to_owned()
    } else {
        std::env::current_dir().map_err(scope_error)?.join(root)
    };
    reject_links(&absolute)?;
    absolute.canonicalize().map_err(scope_error)
}

/// Refuse, before anything starts or `target` is created, a read-only root or file the child
/// cannot be given exactly as named ([`ChildCommand::with_read_only_root`],
/// [`ChildCommand::with_read_only_file`], [`ChildCommand::at_host_path`]), naming the path, and
/// return the child with each host path given at its own path resolved to a root or a file. A
/// child naming none passes as it is, the disk untouched. In order:
///
/// 1. each host path and mount as text: UTF-8, absolute, canonical, a file's within Substrate's
///    byte bounds; a mount on, under or containing one Substrate owns ([`OWNED_MOUNTS`]);
/// 2. each host path on disk, every component read without following it: none a symbolic link,
///    the path there, a directory for a root, a regular file for a file;
/// 3. at most 4 roots and 16 files, Substrate's bounds;
/// 4. no two mounts equal, and none under another;
/// 5. no host path that holds the certificate bundle the child names ([`check_ca_bundle`]), as
///    named or as it resolves, or the directory the driver's private state is made in;
/// 6. no host path that is, holds or lies inside the workspace;
/// 7. Substrate's own rules for the request's roots and files, which its driver applies again.
///
/// What remains between this check and the bind: Substrate's admission opens each root and file
/// once, refusing a link at any component, and bubblewrap binds the descriptor it opened
/// (`--ro-bind-fd`; Substrate's `crates/substrate-host/src/read_only_roots.rs` and
/// `read_only_files.rs`), so only a directory or regular file put in its place between this check
/// and that admission is bound instead; nothing swapped after admission is.
fn check_mounts<'a>(
    child: &'a ChildCommand,
    ca_bundle: Option<&CaBundle>,
) -> Result<std::borrow::Cow<'a, ChildCommand>, ConfinementError> {
    if child.read_only_roots.is_empty()
        && child.read_only_files.is_empty()
        && child.host_paths.is_empty()
    {
        return Ok(std::borrow::Cow::Borrowed(child));
    }
    let utf8 = |noun: &str, path: &'a Path| {
        path.to_str()
            .ok_or_else(|| scope_error(format!("the {noun} {path:?} is not UTF-8")))
    };
    let mut mounts: Vec<Mount<'a>> = Vec::new();
    for (kind, named) in child
        .read_only_roots
        .iter()
        .map(|root| (MountKind::Root, root))
        .chain(
            child
                .read_only_files
                .iter()
                .map(|file| (MountKind::File, file)),
        )
    {
        let host = utf8(kind.noun(), named.host_path.as_path())?;
        let mount = utf8(
            &format!("mount of the {} `{host}`", kind.noun()),
            named.mount.as_path(),
        )?;
        mounts.push(Mount {
            kind: Some(kind),
            host,
            mount,
        });
    }
    for host in &child.host_paths {
        let host = utf8("host path", host.as_path())?;
        mounts.push(Mount {
            kind: None,
            host,
            mount: host,
        });
    }
    // 1. Text. A mount Substrate owns is named as such before its text is judged, so `/` reads
    // as Substrate's.
    for mount in &mounts {
        let label = mount.label();
        canonical_text(mount.host).map_err(|why| scope_error(format!("the {label} {why}")))?;
        if let Some(owned) = OWNED_MOUNTS.iter().find(|owned| {
            mount.mount == **owned || (**owned != "/" && text_overlaps(mount.mount, owned))
        }) {
            return Err(scope_error(format!(
                "the mount `{}` of the {label} is on, under or contains `{owned}`, which Substrate owns; mount it elsewhere, such as under `/opt`",
                mount.mount
            )));
        }
        canonical_text(mount.mount).map_err(|why| {
            scope_error(format!("the mount `{}` of the {label} {why}", mount.mount))
        })?;
    }
    // 2. Disk.
    for mount in &mut mounts {
        let metadata = host_on_disk(&mount.label(), Path::new(mount.host))?;
        let kind = match mount.kind {
            Some(kind) => kind,
            None if metadata.is_dir() => MountKind::Root,
            None if metadata.is_file() => MountKind::File,
            None => {
                return Err(scope_error(format!(
                    "the {} is neither a directory nor a regular file (a socket, device or FIFO)",
                    mount.label()
                )));
            }
        };
        match kind {
            MountKind::Root if !metadata.is_dir() => {
                return Err(scope_error(format!(
                    "the {} is no directory (a file, socket, device or FIFO); name a file with ChildCommand::with_read_only_file",
                    mount.label()
                )));
            }
            MountKind::File if !metadata.is_file() => {
                return Err(scope_error(format!(
                    "the {} is no regular file (a directory, socket, device or FIFO); name a directory with ChildCommand::with_read_only_root",
                    mount.label()
                )));
            }
            _ => {}
        }
        mount.kind = Some(kind);
    }
    // 3. Bounds.
    for mount in mounts
        .iter()
        .filter(|mount| mount.kind == Some(MountKind::File))
    {
        for (what, text) in [("host path", mount.host), ("mount", mount.mount)] {
            if text.len() > MAX_READ_ONLY_FILE_PATH_BYTES
                || text
                    .split('/')
                    .any(|part| part.len() > MAX_READ_ONLY_FILE_COMPONENT_BYTES)
            {
                return Err(scope_error(format!(
                    "the {what} `{text}` of the {} passes Substrate's bounds for a read-only file: {MAX_READ_ONLY_FILE_PATH_BYTES} bytes, {MAX_READ_ONLY_FILE_COMPONENT_BYTES} per component",
                    mount.label()
                )));
            }
        }
    }
    for (kind, bound) in [
        (MountKind::Root, ChildCommand::MAX_READ_ONLY_ROOTS),
        (MountKind::File, ChildCommand::MAX_READ_ONLY_FILES),
    ] {
        if let Some(past) = mounts
            .iter()
            .filter(|mount| mount.kind == Some(kind))
            .nth(bound)
        {
            return Err(scope_error(format!(
                "the {} is one more than the {bound} {}s Substrate gives a child",
                past.label(),
                kind.noun()
            )));
        }
    }
    // 4. Mounts against each other.
    for (index, mount) in mounts.iter().enumerate() {
        for earlier in &mounts[..index] {
            if mount.mount == earlier.mount {
                return Err(scope_error(format!(
                    "the mount `{}` is named twice, for the {} and the {}",
                    mount.mount,
                    earlier.label(),
                    mount.label()
                )));
            }
            if text_overlaps(mount.mount, earlier.mount) {
                return Err(scope_error(format!(
                    "the mounts `{}` of the {} and `{}` of the {} overlap: one lies under the other",
                    earlier.mount,
                    earlier.label(),
                    mount.mount,
                    mount.label()
                )));
            }
        }
    }
    // 5. What a host path may not reach: the bundle the child trusts, and the driver's state.
    if let Some(bundle) = ca_bundle {
        let named = bundle.named.to_string_lossy().into_owned();
        let resolved = bundle
            .named
            .canonicalize()
            .ok()
            .and_then(|path| path.to_str().map(str::to_owned));
        for mount in &mounts {
            if [Some(&named), resolved.as_ref()]
                .into_iter()
                .flatten()
                .any(|bundle| bundle == mount.host || text_beneath(bundle, mount.host))
            {
                return Err(scope_error(format!(
                    "the {} is or holds the certificate bundle `{named}` the child names; the child gets the bundle at {APERTURE_CA_BUNDLE_PATH}",
                    mount.label()
                )));
            }
        }
    }
    if let Some(temp) = std::env::temp_dir()
        .canonicalize()
        .ok()
        .as_deref()
        .and_then(Path::to_str)
    {
        for mount in &mounts {
            if temp == mount.host || text_beneath(temp, mount.host) {
                return Err(scope_error(format!(
                    "the {} holds `{temp}`, where the Substrate driver keeps its private state; name a directory that does not hold it",
                    mount.label()
                )));
            }
        }
    }
    // 6. The workspace.
    let workspace = canonical_workspace(&child.directory)?;
    let workspace = workspace.to_string_lossy();
    for mount in &mounts {
        let label = mount.label();
        if mount.host == workspace {
            return Err(scope_error(format!(
                "the {label} is the workspace; the child sees the workspace at {SANDBOX_WORKSPACE} already"
            )));
        }
        if text_beneath(&workspace, mount.host) {
            return Err(scope_error(format!(
                "the {label} holds the workspace `{workspace}`; name a directory beside it"
            )));
        }
        if text_beneath(mount.host, &workspace) {
            return Err(scope_error(format!(
                "the {label} lies inside the workspace `{workspace}`; the child sees it under {SANDBOX_WORKSPACE} already"
            )));
        }
    }
    // 7. Substrate's own rules, which its driver applies again.
    let mut resolved = child.clone();
    resolved.host_paths.clear();
    for host in &child.host_paths {
        let host = host.to_str().expect("checked as UTF-8 above");
        let mount = ReadOnlyMount {
            host_path: PathBuf::from(host),
            mount: PathBuf::from(host),
        };
        let kind = mounts
            .iter()
            .find(|checked| checked.host == host && checked.mount == host)
            .and_then(|checked| checked.kind);
        match kind {
            Some(MountKind::File) => resolved.read_only_files.push(mount),
            _ => resolved.read_only_roots.push(mount),
        }
    }
    let input = resolved.input("workspace", String::new());
    validate_read_only_roots(&input.read_only_roots)
        .and_then(|()| validate_read_only_files(&input.read_only_files, &input.read_only_roots))
        .map_err(|e| {
            scope_error(format!(
                "the read-only roots and files are refused by Substrate: {e}"
            ))
        })?;
    Ok(std::borrow::Cow::Owned(resolved))
}

/// The metadata of the host path `path`, named `label` in a refusal, read without following any
/// of its components: refused when one is a symbolic link, when it does not exist, or when it
/// cannot be read.
fn host_on_disk(label: &str, path: &Path) -> Result<std::fs::Metadata, ConfinementError> {
    let mut current = PathBuf::new();
    let mut last = None;
    for component in path.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(scope_error(if current == path {
                    format!("the {label} is a symbolic link; name the path it leads to")
                } else {
                    format!(
                        "the {label} passes through the symbolic link `{}`; name the path it leads to",
                        current.display()
                    )
                }));
            }
            Ok(metadata) => last = Some(metadata),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(scope_error(format!("the {label} does not exist")));
            }
            Err(e) => return Err(scope_error(format!("the {label} cannot be read: {e}"))),
        }
    }
    last.ok_or_else(|| scope_error(format!("the {label} names nothing")))
}

/// Refuse, once the driver for a launch opened and before anything starts, what only the driver
/// decides: a read-only root or file that is, holds or lies inside the driver's private state
/// directory (refused [`ConfinementRefusal::ScopeInvalid`]), and read-only files on a driver that
/// does not publish `exec.read-only-files` (refused [`ConfinementRefusal::CapabilityUnserved`]).
fn check_driver_mounts(
    open: &OpenDriver,
    child: &ChildCommand,
    facts: &CapabilityFacts,
) -> Result<(), ConfinementError> {
    let state = open
        .state
        .path()
        .canonicalize()
        .unwrap_or_else(|_| open.state.path().to_owned());
    for (kind, named) in child
        .read_only_roots
        .iter()
        .map(|root| (MountKind::Root, root))
        .chain(
            child
                .read_only_files
                .iter()
                .map(|file| (MountKind::File, file)),
        )
    {
        let host = &named.host_path;
        if host == &state || state.starts_with(host) || host.starts_with(&state) {
            return Err(scope_error(format!(
                "the {} `{}` is, holds or lies inside the Substrate driver's private state `{}`",
                kind.noun(),
                host.display(),
                state.display()
            )));
        }
    }
    if !child.read_only_files.is_empty() && facts.exec_read_only_files.is_none() {
        return Err(refuse(
            ConfinementRefusal::CapabilityUnserved,
            "Substrate does not serve read-only files on this machine: its mechanism did not prove itself when the driver opened",
        ));
    }
    Ok(())
}

/// Refuse, before anything starts or `target` is created, a working directory the child cannot
/// start in ([`ChildCommand::with_workdir`]), naming it; `child` is the one [`check_mounts`]
/// returned, each host path given at its own path resolved to a root or a file. A child naming
/// none passes, the disk untouched. In order:
///
/// 1. the text: UTF-8, no control character (the refusal escapes it), at most
///    [`MAX_WORKDIR_BYTES`], absolute, no `.`, `..` or empty component, as Substrate's
///    `validate_workdir` requires;
/// 2. not at or beneath a read-only file's mount, which is no directory;
/// 3. at or beneath `/workspace` or a read-only root's mount, the only mounts Loom places that
///    hold directories: Loom asks for no scratch, capsule or writable root, so `/scratch`,
///    `/runtime` and every other path are refused here rather than by Substrate;
/// 4. on disk, below the workspace directory or the root's host path, every component read
///    without following it: a directory, through no symbolic link. `/workspace/target` alone may
///    be absent, since the launch creates it; and searchable by this process, with every
///    directory on the way to it ([`may_search`]), since bubblewrap cannot enter one that is not.
///
/// Substrate checks step 4 again when the child starts, beneath the descriptor it binds
/// (`exec.workdir-missing`), so a directory removed or swapped after this check is refused there.
/// A directory bubblewrap still cannot enter is refused once the run ended
/// ([`check_entered_workdir`]).
fn check_workdir(child: &ChildCommand) -> Result<(), ConfinementError> {
    let Some(named) = child.workdir.as_deref() else {
        return Ok(());
    };
    // 1. Text.
    let Some(text) = named.to_str() else {
        return Err(scope_error(format!(
            "the working directory {named:?} is not UTF-8; Substrate names a working directory as text"
        )));
    };
    if text.chars().any(char::is_control) {
        return Err(scope_error(format!(
            "the working directory {text:?} holds a control character"
        )));
    }
    if text.len() > MAX_WORKDIR_BYTES {
        let mut end = 64;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        return Err(scope_error(format!(
            "the working directory `{}…` is {} bytes, longer than the {MAX_WORKDIR_BYTES} bytes Substrate admits",
            &text[..end],
            text.len()
        )));
    }
    if !text.starts_with('/') {
        return Err(scope_error(format!(
            "the working directory `{text}` is not absolute; name it as the child sees it, such as `/workspace/{text}`"
        )));
    }
    if text != "/"
        && text[1..]
            .split('/')
            .any(|component| matches!(component, "" | "." | ".."))
    {
        return Err(scope_error(format!(
            "the working directory `{text}` is not lexically normal: it has a `.`, `..` or empty component or a trailing `/`; name it without them"
        )));
    }
    let at_or_beneath = |mount: &str| -> Option<String> {
        if text == mount {
            Some(String::new())
        } else {
            text.strip_prefix(mount)
                .and_then(|rest| rest.strip_prefix('/'))
                .map(str::to_owned)
        }
    };
    // 2. A read-only file.
    if let Some(file) = child
        .read_only_files
        .iter()
        .find(|file| at_or_beneath(&file.mount.to_string_lossy()).is_some())
    {
        return Err(scope_error(format!(
            "the working directory `{text}` lies at or beneath the read-only file mounted at `{}`; a child starts in a directory",
            file.mount.display()
        )));
    }
    // 3. The mounts that hold directories.
    let (base, rest) = if let Some(rest) = at_or_beneath(SANDBOX_WORKSPACE) {
        (canonical_workspace(&child.directory)?, rest)
    } else if let Some((root, rest)) = child
        .read_only_roots
        .iter()
        .find_map(|root| at_or_beneath(&root.mount.to_string_lossy()).map(|rest| (root, rest)))
    {
        (root.host_path.clone(), rest)
    } else {
        return Err(scope_error(format!(
            "the working directory `{text}` is not at or beneath /workspace or a read-only root the child reads; Loom places no other directory, such as /scratch or /runtime"
        )));
    };
    // 4. The disk.
    let in_workspace = at_or_beneath(SANDBOX_WORKSPACE).is_some();
    let searchable = |path: &Path| -> Result<(), ConfinementError> {
        if may_search(path) {
            Ok(())
        } else {
            Err(scope_error(format!(
                "the working directory `{text}` cannot be entered: `{}` or a directory on the way to it is not searchable",
                path.display()
            )))
        }
    };
    searchable(&base)?;
    let mut current = base;
    for component in rest.split('/').filter(|component| !component.is_empty()) {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(scope_error(format!(
                    "the working directory `{text}` passes through the symbolic link `{}`; name the directory it leads to",
                    current.display()
                )));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(scope_error(format!(
                    "the working directory `{text}` is no directory: `{}` is not one",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if in_workspace && rest == "target" {
                    return Ok(());
                }
                return Err(scope_error(format!(
                    "the working directory `{text}` does not exist: `{}` is absent",
                    current.display()
                )));
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::PermissionDenied
                    && current.parent().is_some_and(|parent| !may_search(parent)) =>
            {
                return searchable(current.parent().unwrap_or(&current));
            }
            Err(e) => {
                return Err(scope_error(format!(
                    "the working directory `{text}` cannot be read at `{}`: {e}",
                    current.display()
                )));
            }
        }
    }
    searchable(&current)
}

/// Whether this process may search `path`, a directory, and every directory on the way to it: a
/// child cannot start in a directory it cannot enter, and bubblewrap's `--chdir` fails there with
/// `Permission denied`. The probe is a `stat` of `<path>/.`, which resolves through `path` and so
/// needs search permission on it and on each directory above it, as `chdir` does.
fn may_search(path: &Path) -> bool {
    let mut probe = path.as_os_str().to_owned();
    probe.push("/.");
    std::fs::metadata(probe).is_ok()
}

/// Refuse the result of a run that named a working directory when its output carries
/// bubblewrap's diagnostic for that directory (`bwrap: Can't chdir to <workdir>: …`): bubblewrap
/// could not enter it, so the program never ran and the exit status is bubblewrap's, not the
/// child's. Substrate reads that failure as no refusal of its own, so the result is refused here
/// as an invalid scope, the refusal [`check_workdir`] gives before launch for the same directory.
fn check_entered_workdir(
    input: &ExecStartInput,
    output: &ChildOutput,
) -> Result<(), ConfinementError> {
    let Some(workdir) = input.workdir.as_deref() else {
        return Ok(());
    };
    let diagnostic = format!("bwrap: Can't chdir to {workdir}: ");
    let carries = [&output.stderr, &output.stdout].into_iter().any(|stream| {
        String::from_utf8_lossy(stream)
            .lines()
            .any(|line| line.starts_with(&diagnostic))
    });
    if carries {
        return Err(scope_error(format!(
            "the child could not enter its working directory {workdir:?}, so its program never ran: {}",
            String::from_utf8_lossy(tail(&output.stderr)).trim_end()
        )));
    }
    Ok(())
}

/// Refuse a working directory on a driver that does not publish `exec.workdir`
/// ([`ConfinementRefusal::CapabilityUnserved`]), before anything starts: Substrate proves the
/// mechanism when its driver opens, and a driver without it would refuse the start itself.
fn check_workdir_served(
    child: &ChildCommand,
    facts: &CapabilityFacts,
) -> Result<(), ConfinementError> {
    match child.workdir.as_deref() {
        Some(workdir) if facts.exec_workdir != Some(true) => Err(refuse(
            ConfinementRefusal::CapabilityUnserved,
            format!(
                "Substrate does not serve a working directory on this machine (`exec.workdir`), so the child cannot start in `{}`; start it in /workspace",
                workdir.display()
            ),
        )),
        _ => Ok(()),
    }
}

/// Accept Substrate's record of what it applied only when it is the confinement `input` required:
/// its workspace access (exactly the writable paths requested, so a record reporting a wider or
/// another writable set is refused), profile, read-only roots, read-only files and capability
/// snapshot, no secret slot, a cgroup, and exactly the requested network ([`network_applied`]),
/// the apertures `input` asked for by name being exactly `egress`, in order; exactly the working
/// directory requested, or none when none was ([`ChildCommand::with_workdir`]); no writable root
/// or execution capsule, which Loom never requests, and so the filesystem Substrate applies without a capsule. A test's launch and a child's are held to the
/// same record. The record is taken apart without `..`, so a member a later Substrate adds does
/// not compile until it is judged here.
fn check_record(
    input: &ExecStartInput,
    egress: &[EgressAperture],
    record: &substrate_wire::AppliedConfinement,
) -> Result<(), ConfinementError> {
    let substrate_wire::AppliedConfinement {
        capability_snapshot,
        cgroup,
        filesystem,
        network,
        profile,
        workspace_access,
        capsule,
        read_only_roots,
        read_only_files,
        secret_slots,
        workdir,
        writable_roots,
    } = record;
    if *workspace_access != input.workspace_access
        || !network_applied(input.sandbox.network, egress, network)
        || !apertures_requested(&input.sandbox, egress)
        || *profile != input.sandbox.profile
        || *read_only_roots != input.read_only_roots
        || *read_only_files != input.read_only_files
        || *workdir != input.workdir
        || !input.writable_roots.is_empty()
        || !writable_roots.is_empty()
        || input.capsule.is_some()
        || capsule.is_some()
        || *filesystem != substrate_wire::AppliedFilesystem::WorkspaceReadWriteSystemReadOnly
        || !secret_slots.is_empty()
        || *capability_snapshot != input.sandbox.capability_snapshot
        || cgroup.is_empty()
    {
        return Err(refuse(
            ConfinementRefusal::CapabilityUnserved,
            "Substrate's applied confinement does not match the required profile",
        ));
    }
    Ok(())
}

/// Whether the apertures `sandbox` asks Substrate for by name are exactly `egress`, in order: one
/// aperture by `aperture` alone, several by `apertures` alone, none by neither.
fn apertures_requested(sandbox: &ConfinementRequest, egress: &[EgressAperture]) -> bool {
    match egress {
        [] => sandbox.aperture.is_none() && sandbox.apertures.is_empty(),
        [only] => {
            sandbox.aperture.as_deref() == Some(only.name.as_str()) && sandbox.apertures.is_empty()
        }
        several => {
            sandbox.aperture.is_none()
                && sandbox.apertures.len() == several.len()
                && sandbox
                    .apertures
                    .iter()
                    .zip(several)
                    .all(|(requested, declared)| *requested == declared.name)
        }
    }
}

/// Whether the network Substrate applied is the one requested: no network for
/// [`NetworkMode::None`], and for [`NetworkMode::Aperture`] exactly the declared apertures. One
/// aperture is applied as one, with no listen address; several as a list of as many, in the order
/// declared, each listening where Substrate puts the aperture at its place
/// ([`aperture_listen`]). Each is judged by its name, pinned destination, mechanism and byte
/// ceiling. Any other network, wider or narrower, in another order or of another form, is not the
/// one requested. An aperture is taken apart without `..`, as [`check_record`] takes the record;
/// its byte counts are what crossed and judge nothing.
fn network_applied(
    requested: NetworkMode,
    egress: &[EgressAperture],
    applied: &AppliedNetwork,
) -> bool {
    match (requested, egress, applied) {
        (NetworkMode::None, [], AppliedNetwork::None) => true,
        (NetworkMode::Aperture, [declared], AppliedNetwork::Aperture(applied)) => {
            aperture_applied(declared, applied, None)
        }
        (NetworkMode::Aperture, declared, AppliedNetwork::Apertures(applied))
            if declared.len() >= 2 =>
        {
            declared.len() == applied.len()
                && declared
                    .iter()
                    .zip(applied)
                    .enumerate()
                    .all(
                        |(index, (declared, applied))| match aperture_listen(index, declared) {
                            Some(listen) => aperture_applied(declared, applied, Some(listen)),
                            None => false,
                        },
                    )
        }
        _ => false,
    }
}

/// Whether `applied` is `declared` as Substrate applies it, listening at `listen`.
fn aperture_applied(
    declared: &EgressAperture,
    applied: &substrate_wire::AppliedAperture,
    listen: Option<SocketAddrV4>,
) -> bool {
    let substrate_wire::AppliedAperture {
        mode,
        name,
        destination,
        mechanism,
        bytes: _,
        max_bytes,
        listen: applied_listen,
    } = applied;
    *mode == ApertureMode::Aperture
        && *name == declared.name
        && *destination == declared.pinned.to_string()
        && *mechanism == ApertureMechanism::LoopbackForwarder
        && *max_bytes == declared.max_bytes
        && *applied_listen == listen.map(|listen| listen.to_string())
}

/// Where the aperture at `index` of a list listens inside the sandbox: the n-th (from 1) on
/// `127.0.0.n` ([`APERTURE_LOOPBACK_ADDRESS`] for the first), at the port it names; none at or past
/// [`MAX_EGRESS_APERTURES`].
fn aperture_listen(index: usize, aperture: &EgressAperture) -> Option<SocketAddrV4> {
    if index >= usize::try_from(MAX_EGRESS_APERTURES).ok()? {
        return None;
    }
    let first: Ipv4Addr = APERTURE_LOOPBACK_ADDRESS.parse().ok()?;
    let [a, b, c, d] = first.octets();
    let d = d.checked_add(u8::try_from(index).ok()?)?;
    Some(SocketAddrV4::new(Ipv4Addr::new(a, b, c, d), aperture.port))
}

/// A network as the requested profile names it: `none`, or `aperture:<name>@<destination>` for
/// each aperture, joined with `,`.
fn network_name(apertures: &[EgressAperture]) -> String {
    if apertures.is_empty() {
        return "none".to_owned();
    }
    apertures
        .iter()
        .map(|aperture| format!("aperture:{}@{}", aperture.name, aperture.pinned))
        .collect::<Vec<_>>()
        .join(",")
}

/// The apertures Substrate applied, as the confinement projection lists them: none, the one
/// aperture with no listen address, or each of a list with the address it listens on.
fn applied_apertures(network: &AppliedNetwork) -> Vec<AppliedAperture> {
    let project = |applied: &substrate_wire::AppliedAperture| AppliedAperture {
        name: applied.name.clone(),
        destination: applied.destination.clone(),
        listen: applied.listen.clone(),
    };
    match network {
        AppliedNetwork::None => Vec::new(),
        AppliedNetwork::Aperture(applied) => vec![project(applied)],
        AppliedNetwork::Apertures(applied) => applied.iter().map(project).collect(),
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
    let root = canonical_workspace(root)?;
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
    // Every check of the named paths, and the scan of what the writable paths hold, runs before
    // `target` is created, so a refused path leaves the workspace as it was.
    for path in &named {
        check_writable_directory(&root, path)?;
    }
    let target = root.join(WRITABLE);
    reject_links(&target)?;
    // A `target` not created yet holds nothing to scan; one already there is scanned with the
    // named paths, so a hard link between them counts as internal.
    let scopes: Vec<PathBuf> = target
        .is_dir()
        .then(|| target.clone())
        .into_iter()
        .chain(named.iter().map(|path| root.join(path)))
        .collect();
    validate_writable_artifacts(&scopes)?;
    match std::fs::create_dir(&target) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && target.is_dir() => {}
        Err(e) => return Err(scope_error(format!("target/ must be a directory: {e}"))),
    }
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
        // a repository's rust-toolchain file from turning resolution into a download. The
        // workspace decides the toolchain: a rustup proxy exports RUSTUP_TOOLCHAIN to whatever
        // it starts, and that variable would override the workspace's rust-toolchain file.
        let rustc = host_program("rustup", workspace).map(|rustup| {
            Command::new(rustup)
                .args(["which", "rustc"])
                .env("RUSTUP_AUTO_INSTALL", "0")
                .env_remove("RUSTUP_TOOLCHAIN")
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
                apertures: Vec::new(),
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
            read_only_files: Vec::new(),
            secret_slots: Vec::new(),
            capsule: None,
            lease_ttl_ms: None,
            workdir: None,
            writable_roots: Vec::new(),
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

/// The families every launch needs from Substrate's `linux` adapter: a driver is admitted by the
/// families and facts it proved when it opened, never by looking at the operating system (ADR
/// 0109 decision 6). Missing `limits` is the delegated cgroup missing, as before; any other
/// family is a capability the machine does not serve. `limits` comes first: Substrate's probe
/// withholds the floor (`process_run`, `wait`, `output_read`) together with `limits` when the
/// cgroup is not delegated, and that driver must be refused `CgroupUndelegated`, the reason the
/// CLI re-executes itself in a delegated scope on.
const NEEDED_FAMILIES: [ContractFamily; 6] = [
    ContractFamily::Limits,
    ContractFamily::ProcessRun,
    ContractFamily::Wait,
    ContractFamily::OutputRead,
    ContractFamily::Workspace,
    ContractFamily::FilesystemView,
];

/// Admit a driver whose `machine` proves every family in [`NEEDED_FAMILIES`] through Substrate's
/// `linux` adapter, and answer the facts it proved, in `FACT_ORDER`; a family it lacks is refused
/// naming `confinement.family-unserved` and the family.
#[cfg(target_os = "linux")]
fn admitted_facts(
    machine: &substrate_wire::CapabilitySnapshot,
) -> Result<Vec<String>, ConfinementError> {
    let proved = substrate_host::confinement::linux::proof(machine);
    for family in NEEDED_FAMILIES {
        if !proved.serves(&family) {
            let (reason, hint) = if family == ContractFamily::Limits {
                (
                    ConfinementRefusal::CgroupUndelegated,
                    "; Substrate proves it only once bubblewrap runs with user namespaces and the cpu, memory and pids controllers are delegated",
                )
            } else {
                (ConfinementRefusal::CapabilityUnserved, "")
            };
            return Err(refuse(
                reason,
                format!(
                    "{FAMILY_UNSERVED}: Substrate's linux adapter does not serve `{}` on this machine{hint}",
                    enum_name(&family)
                ),
            ));
        }
    }
    Ok(FACT_ORDER
        .iter()
        .filter(|fact| proved.proves(fact))
        .map(enum_name)
        .collect())
}

/// Off Linux there is no `linux` adapter, so no driver is admitted (Substrate's own driver refuses
/// to open there first).
#[cfg(not(target_os = "linux"))]
fn admitted_facts(
    _machine: &substrate_wire::CapabilitySnapshot,
) -> Result<Vec<String>, ConfinementError> {
    Err(refuse(
        ConfinementRefusal::BackendMissing,
        "Substrate's linux adapter serves Linux only",
    ))
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
fn files_projection(files: &[substrate_wire::ReadOnlyFile]) -> Vec<ReadOnlyFile> {
    files
        .iter()
        .map(|f| ReadOnlyFile {
            host_path: f.host_path.clone(),
            mount: f.mount.clone(),
        })
        .collect()
}
fn requested_profile(input: &ExecStartInput, egress: &[EgressAperture]) -> ConfinementProfile {
    ConfinementProfile {
        backend: Backend::Substrate,
        network: match (input.sandbox.network, egress) {
            (NetworkMode::Aperture, [_, ..]) => network_name(egress),
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
    egress: &[EgressAperture],
    ca_bundle: Option<&CaBundle>,
    read_only_files: &[substrate_wire::ReadOnlyFile],
    workdir: Option<&str>,
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
    // The whole egress declaration, absent without one for the same reason: one aperture as the
    // object it has always been, so its digest is unchanged, and several as a list of them.
    let declaration = |aperture: &EgressAperture| {
        serde_json::json!({ "name":aperture.name, "host":aperture.host, "port":aperture.port,
            "pinned":aperture.pinned.to_string(), "max_bytes":aperture.max_bytes })
    };
    match egress {
        [] => {}
        [only] => json["egress"] = declaration(only),
        several => json["egress"] = several.iter().map(declaration).collect(),
    }
    // The certificate bundle: its path as the host named it (UTF-8, `check_ca_bundle`) and the
    // digest of the bytes the child trusts, absent without one for the same reason.
    if let Some(bundle) = ca_bundle {
        json["ca_bundle"] = serde_json::json!({ "path":bundle.named.to_string_lossy(),
            "sha256":format!("{:x}", Sha256::digest(&bundle.bytes)) });
    }
    // The read-only files the child reads, each host path and mount, absent without one for the
    // same reason: [`ConfinementProfile`] names roots only.
    if !read_only_files.is_empty() {
        json["read_only_files"] = read_only_files
            .iter()
            .map(|f| serde_json::json!({"host_path":f.host_path,"mount":f.mount}))
            .collect();
    }
    // The working directory the child starts in, absent without one for the same reason.
    if let Some(workdir) = workdir {
        json["workdir"] = workdir.into();
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

    /// story:substrate-0-12-pin: the operating-system check for `/usr/bin/bwrap` gave way to the
    /// families Substrate's `linux` adapter proves; a driver missing one is refused naming it.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_driver_missing_a_family_is_refused_naming_it() {
        let snapshot = |exec: serde_json::Value| -> substrate_wire::CapabilitySnapshot {
            let mut facts = serde_json::json!({
                "operation.ledger-subject-max-rows": 1,
                "operation.ledger-subject-max-bytes": 1,
                "operation.ledger-global-max-rows": 1,
                "operation.ledger-global-max-bytes": 1,
            });
            facts
                .as_object_mut()
                .unwrap()
                .extend(exec.as_object().unwrap().clone());
            serde_json::from_value(serde_json::json!({
                "snapshot": "cap_1",
                "driver": "host",
                "driver_version": "0",
                "config_generation": 1,
                "probed_at": "2026-10-27T00:00:00Z",
                "facts": facts,
            }))
            .unwrap()
        };
        let served = serde_json::json!({
            "exec.argv-only": true,
            "exec.namespaces": {"user": true, "mount": true, "pid": true, "ipc": true, "uts": true, "network": true},
            "exec.cgroup-limits": {"processes": true, "memory": true, "cpu": true},
            "exec.cgroup-kill": true,
            "exec.output-limit-bytes": 1_048_576,
            "workspace.guarded-io": true,
            "exec.read-only-roots": {"max_roots": 4},
            "exec.read-only-files": {"max_files": 16},
        });
        let facts = admitted_facts(&snapshot(served.clone())).expect("every family served");
        assert_eq!(
            facts,
            [
                "whole_tree_kill",
                "ipc_socket_allowlist",
                "ipc_signals_contained"
            ],
            "the proved facts, exactly, in FACT_ORDER's spelling"
        );
        // A driver whose cgroup is not delegated proves no execution family at all: it is refused
        // for `limits`, so the CLI re-executes in a delegated scope.
        let undelegated = admitted_facts(&snapshot(serde_json::json!({})))
            .expect_err("a driver serving nothing is refused");
        assert_eq!(
            undelegated.reason,
            ConfinementRefusal::CgroupUndelegated,
            "story:substrate-0-12-pin: {undelegated}"
        );
        assert!(undelegated.detail.contains("`limits`"), "{undelegated}");
        for (fact, family, reason) in [
            (
                "exec.cgroup-limits",
                "limits",
                ConfinementRefusal::CgroupUndelegated,
            ),
            (
                "exec.argv-only",
                "process_run",
                ConfinementRefusal::CapabilityUnserved,
            ),
            (
                "exec.output-limit-bytes",
                "output_read",
                ConfinementRefusal::CapabilityUnserved,
            ),
            (
                "workspace.guarded-io",
                "workspace",
                ConfinementRefusal::CapabilityUnserved,
            ),
            (
                "exec.read-only-roots",
                "filesystem_view",
                ConfinementRefusal::CapabilityUnserved,
            ),
        ] {
            let mut exec = served.clone();
            exec.as_object_mut().unwrap().remove(fact);
            let error =
                admitted_facts(&snapshot(exec)).expect_err("a driver missing a family is refused");
            assert_eq!(error.reason, reason, "{fact}: {error}");
            assert!(
                error.detail.contains(FAMILY_UNSERVED) && error.detail.contains(family),
                "{fact}: the refusal names the family: {error}"
            );
        }
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

    /// `cargo test` runs under a rustup proxy, which exports its own `RUSTUP_TOOLCHAIN` to every
    /// process it starts. The workspace's toolchain file still decides the compiler: a custom path
    /// it names is refused, never swapped for the toolchain the launching process ran under.
    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "confinement runs on Linux only")]
    fn the_workspace_toolchain_file_decides_whatever_the_launching_process_inherited() {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(workspace.join("custom/bin")).unwrap();
        std::fs::create_dir(workspace.join(WRITABLE)).unwrap();
        let rustc = workspace.join("custom/bin/rustc");
        std::fs::write(&rustc, "#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(&rustc, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(
            workspace.join("rust-toolchain.toml"),
            format!(
                "[toolchain]\npath = '{}'\n",
                workspace.join("custom").display()
            ),
        )
        .unwrap();
        if host_program("rustup", &workspace).is_none() {
            eprintln!("skipped: missing prerequisite: rustup on PATH");
            return;
        }
        let error = RustClosure::discover(&workspace)
            .err()
            .expect("a custom toolchain path the workspace names is refused");
        assert_eq!(
            error.reason,
            ConfinementRefusal::ScopeInvalid,
            "{}",
            error.detail
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
        let profile = requested_profile(&input, &[]);
        assert_eq!(profile.writable_scopes, expected);
        let bare = ChildCommand::new("true", [] as [&str; 0], "/work/space");
        let plain = requested_profile(&bare.input("space", "snapshot".to_owned()), &[]);
        assert_ne!(
            profile_digest(&profile, &[], &[], None, &[], None),
            profile_digest(&plain, &[], &[], None, &[], None)
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
    fn a_writable_path_refused_on_disk_leaves_the_workspace_as_it_was() {
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
        // What a named directory holds is scanned before `target` is created too.
        std::fs::create_dir(workspace.join("sockets")).unwrap();
        let _held = std::os::unix::net::UnixListener::bind(workspace.join("sockets/held")).unwrap();
        std::fs::create_dir(workspace.join("linked")).unwrap();
        std::fs::hard_link(workspace.join("file"), workspace.join("linked/alias")).unwrap();
        for (path, named) in [("sockets", "socket"), ("linked", "hard links outside")] {
            let writable = check_writable(&[PathBuf::from(path)]).unwrap();
            let error = validate_workspace(&workspace, &writable).unwrap_err();
            assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{path}");
            assert!(
                error
                    .detail
                    .contains(&workspace.join(path).display().to_string()),
                "{error}"
            );
            assert!(error.detail.contains(named), "{path}: {error}");
            assert!(
                !workspace.join("target").exists(),
                "{path}: a refused path leaves the workspace as it was"
            );
        }
        let writable = check_writable(&[PathBuf::from("src/lib")]).unwrap();
        validate_workspace(&workspace, &writable).expect("a plain directory is writable");
        assert!(
            workspace.join("target").is_dir(),
            "an accepted launch has `target`"
        );
    }

    #[test]
    fn a_record_reporting_other_writable_paths_than_requested_is_refused() {
        let input = ChildCommand::new("true", [] as [&str; 0], "/work/space")
            .with_writable("scratch")
            .input("space", "snapshot".to_owned());
        let exact = record_for(&input, AppliedNetwork::None);
        check_record(&input, &[], &exact).unwrap();
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
            let error = check_record(&input, &[], &record).unwrap_err();
            assert_eq!(
                error.reason,
                ConfinementRefusal::CapabilityUnserved,
                "{access:?}"
            );
        }
    }

    /// Substrate 0.10.0 can apply a working directory, writable roots and several apertures. Loom
    /// requests none of them, so a record reporting any of them, for a child run to its end or a
    /// live child, is not the confinement requested and is refused.
    #[test]
    fn a_record_reporting_a_workdir_writable_roots_or_apertures_is_refused() {
        let child = ChildCommand::new("true", [] as [&str; 0], "/work/space").with_egress(model());
        let run = child.input("space", "snapshot".to_owned());
        let live = child.session_input("space", "snapshot".to_owned()).exec;
        for input in [&run, &live] {
            assert_eq!(input.workdir, None);
            assert!(input.writable_roots.is_empty());
            assert!(input.sandbox.apertures.is_empty());
            let single = applied_aperture("model", PINNED, None);
            let exact = record_for(input, single.clone());
            check_record(input, &[model()], &exact).unwrap();
            let AppliedNetwork::Aperture(applied) = single else {
                unreachable!("an applied aperture");
            };
            let others = [
                substrate_wire::AppliedConfinement {
                    workdir: Some(SANDBOX_WORKSPACE.to_owned()),
                    ..exact.clone()
                },
                substrate_wire::AppliedConfinement {
                    workdir: Some("/workspace/src".to_owned()),
                    ..exact.clone()
                },
                substrate_wire::AppliedConfinement {
                    writable_roots: vec![substrate_wire::AppliedWritableRoot::Host {
                        name: "cache".to_owned(),
                        host_path: "/srv/cache".to_owned(),
                        mount: "/cache".to_owned(),
                    }],
                    ..exact.clone()
                },
                substrate_wire::AppliedConfinement {
                    writable_roots: vec![substrate_wire::AppliedWritableRoot::Git {
                        workspace: "ws_other".to_owned(),
                        host_path: "/srv/workspaces/other".to_owned(),
                        mount: "/checkout".to_owned(),
                    }],
                    ..exact.clone()
                },
                substrate_wire::AppliedConfinement {
                    network: AppliedNetwork::Apertures(vec![applied.clone()]),
                    ..exact.clone()
                },
                substrate_wire::AppliedConfinement {
                    network: AppliedNetwork::Apertures(vec![
                        applied.clone(),
                        substrate_wire::AppliedAperture {
                            name: "other".to_owned(),
                            listen: Some("127.0.0.2".to_owned()),
                            ..applied.clone()
                        },
                    ]),
                    ..exact.clone()
                },
            ];
            for record in others {
                let error = check_record(input, &[model()], &record).unwrap_err();
                assert_eq!(
                    error.reason,
                    ConfinementRefusal::CapabilityUnserved,
                    "{record:?}"
                );
            }
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
                apertures: Vec::new(),
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
            .open(fixture.path(), &DriverEgress::NONE)
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
            .open(fixture.path(), &DriverEgress::NONE)
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
    fn projection(network: Vec<AppliedAperture>) -> AppliedConfinement {
        AppliedConfinement {
            backend: Backend::Substrate,
            profile_digest: String::new(),
            capability_snapshot: String::new(),
            cgroup: "substrate-ex_loom_1_0".to_owned(),
            filesystem: "workspace-rw-system-ro".to_owned(),
            network: Some(network),
            profile: "workspace".to_owned(),
            writable_scopes: vec!["target".to_owned()],
            read_only_roots: Vec::new(),
            read_only_files: Vec::new(),
            workdir: None,
            adapter: None,
            facts: None,
        }
    }

    /// A child's streams, both empty.
    fn no_output() -> ChildOutput {
        ChildOutput {
            stdout: Vec::new(),
            stderr: Vec::new(),
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }

    #[test]
    fn a_child_result_with_no_exit_status_and_no_timeout_is_refused() {
        let launched = |exit_code, timed_out| Launched {
            execution: TestExecution {
                exit_code,
                timed_out,
                output: String::new(),
                applied: projection(Vec::new()),
                driver_record: None,
                child_output: no_output(),
            },
            ending: "state unknown, refusal none, output complete true".to_owned(),
            refusal: None,
            exit: ChildExit {
                exit_code: exit_code.map(i64::from),
                signal: None,
                timed_out,
            },
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
                applied: projection(vec![AppliedAperture {
                    name: "model".to_owned(),
                    destination: "10.0.0.1:443".to_owned(),
                    listen: None,
                }]),
                driver_record: Some(record_for(&input, network)),
                child_output: no_output(),
            },
            ending: format!(
                "state cancelled, refusal {}, output complete true",
                refusal.unwrap_or("none")
            ),
            refusal: refusal.map(str::to_owned),
            exit: ChildExit {
                exit_code: None,
                signal: None,
                timed_out: false,
            },
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
            listen: None,
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
            read_only_files: input.read_only_files.clone(),
            secret_slots: Vec::new(),
            workdir: None,
            writable_roots: Vec::new(),
        }
    }

    #[test]
    fn only_the_requested_network_is_accepted_as_applied() {
        let aperture = applied_aperture("model", PINNED, None);
        assert!(network_applied(
            NetworkMode::None,
            &[],
            &AppliedNetwork::None
        ));
        assert!(!network_applied(NetworkMode::None, &[], &aperture));
        assert!(network_applied(
            NetworkMode::Aperture,
            &[model()],
            &aperture
        ));
        assert!(!network_applied(
            NetworkMode::Aperture,
            &[model()],
            &AppliedNetwork::None
        ));
        assert!(!network_applied(NetworkMode::Aperture, &[], &aperture));
        for other in [
            applied_aperture("other", PINNED, None),
            applied_aperture("model", "10.0.0.2:443", None),
            applied_aperture("model", PINNED, Some(1024)),
        ] {
            assert!(
                !network_applied(NetworkMode::Aperture, &[model()], &other),
                "{other:?}"
            );
        }
        let bounded = EgressAperture {
            max_bytes: Some(1024),
            ..model()
        };
        assert!(
            !network_applied(
                NetworkMode::Aperture,
                std::slice::from_ref(&bounded),
                &aperture
            ),
            "a declared ceiling applied without it is a wider network"
        );
    }

    #[test]
    fn a_record_reporting_another_network_than_requested_is_refused() {
        let child = ChildCommand::new("true", [] as [&str; 0], "/work/space");
        // A test's request and a plain child's ask for no network alike.
        let none = child.input("space", "snapshot".to_owned());
        check_record(&none, &[], &record_for(&none, AppliedNetwork::None)).unwrap();
        let wider = record_for(&none, applied_aperture("model", PINNED, None));
        let error = check_record(&none, &[], &wider).unwrap_err();
        assert_eq!(error.reason, ConfinementRefusal::CapabilityUnserved);
        assert_eq!(
            error.detail,
            "Substrate's applied confinement does not match the required profile"
        );

        let input = child
            .with_egress(model())
            .input("space", "snapshot".to_owned());
        let exact = record_for(&input, applied_aperture("model", PINNED, None));
        check_record(&input, &[model()], &exact).unwrap();
        for other in [
            AppliedNetwork::None,
            applied_aperture("model", "10.0.0.2:443", None),
            applied_aperture("other", PINNED, None),
            applied_aperture("model", PINNED, Some(1024)),
        ] {
            let error =
                check_record(&input, &[model()], &record_for(&input, other.clone())).unwrap_err();
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
        assert!(check_record(&input, std::slice::from_ref(&other), &record).is_err());
    }

    /// How Substrate observed a live child of `input` end: its record names `network`, it ended
    /// in `state` with `exit` and `refusal`, and both its streams are whole.
    fn live_ending(
        input: &ExecStartInput,
        network: AppliedNetwork,
        state: ExecState,
        exit: ExecExit,
        refusal: Option<&str>,
    ) -> Ending {
        Ending {
            record: Some(record_for(input, network)),
            state,
            exit: Some(exit),
            refusal: refusal.map(|code| ExecRefusal {
                class: substrate_wire::ErrorClass::Failed,
                code: code.to_owned(),
                message: "ended".to_owned(),
            }),
            output: ChildOutput {
                stdout: b"out".to_vec(),
                stderr: b"err".to_vec(),
                stdout_truncated: false,
                stderr_truncated: false,
            },
            output_complete: true,
        }
    }

    const EXITED: ExecExit = ExecExit {
        code: Some(0),
        signal: None,
    };

    #[test]
    fn a_live_child_whose_record_names_another_network_is_refused() {
        let child = ChildCommand::new("cat", [] as [&str; 0], "/work/space");
        let session = child.session_input("space", "snapshot".to_owned());
        assert!(
            !session.exec.wait,
            "a live child is not waited for at its start"
        );
        assert_eq!(session.exec, {
            let mut run = child.input("space", "snapshot".to_owned());
            run.wait = false;
            run
        });
        let input = &session.exec;
        let ending = |network| live_ending(input, network, ExecState::Exited, EXITED, None);
        let settled = settle(
            input,
            &[],
            "digest".to_owned(),
            ending(AppliedNetwork::None),
        )
        .expect("the record is the one requested");
        let result = live_result(settled, false).expect("a child that exited is a result");
        assert_eq!(
            result.exit,
            ChildExit {
                exit_code: Some(0),
                signal: None,
                timed_out: false
            }
        );
        assert_eq!(result.output.stdout, b"out");
        assert_eq!(result.output.stderr, b"err");
        assert_eq!(result.applied.network, Some(Vec::new()));
        assert_eq!(result.applied.profile_digest, "digest");
        assert_eq!(
            result.driver_record,
            record_for(input, AppliedNetwork::None)
        );

        let error = settle(
            input,
            &[],
            "digest".to_owned(),
            ending(applied_aperture("model", PINNED, None)),
        )
        .err()
        .expect("a wider network than requested is refused");
        assert_eq!(error.reason, ConfinementRefusal::CapabilityUnserved);
        assert_eq!(
            error.detail,
            "Substrate's applied confinement does not match the required profile"
        );
        let mut unrecorded = ending(AppliedNetwork::None);
        unrecorded.record = None;
        let error = settle(input, &[], "digest".to_owned(), unrecorded)
            .err()
            .expect("an end with no record is refused");
        assert_eq!(error.reason, ConfinementRefusal::CapabilityUnserved);

        let session = child
            .with_egress(model())
            .session_input("space", "snapshot".to_owned());
        let input = &session.exec;
        let ending = |network| live_ending(input, network, ExecState::Exited, EXITED, None);
        settle(
            input,
            &[model()],
            "digest".to_owned(),
            ending(applied_aperture("model", PINNED, None)),
        )
        .expect("the aperture requested");
        for other in [
            AppliedNetwork::None,
            applied_aperture("other", PINNED, None),
            applied_aperture("model", "10.0.0.2:443", None),
        ] {
            let error = settle(
                input,
                &[model()],
                "digest".to_owned(),
                ending(other.clone()),
            )
            .err()
            .expect("another network than requested is refused");
            assert_eq!(
                error.reason,
                ConfinementRefusal::CapabilityUnserved,
                "{other:?}"
            );
        }
    }

    #[test]
    fn a_live_child_the_host_signalled_is_a_result_and_one_ended_otherwise_is_not() {
        let input = ChildCommand::new("sleep", ["600"], "/work/space")
            .session_input("space", "snapshot".to_owned())
            .exec;
        let ended = |state, exit, refusal| {
            settle(
                &input,
                &[],
                "digest".to_owned(),
                live_ending(&input, AppliedNetwork::None, state, exit, refusal),
            )
            .expect("the record is the one requested")
        };
        let killed = ExecExit {
            code: None,
            signal: Some(Signal::Kill),
        };
        let result = live_result(ended(ExecState::Cancelled, killed.clone(), None), true)
            .expect("a child the host killed is a result");
        assert_eq!(
            result.exit,
            ChildExit {
                exit_code: None,
                signal: Some(9),
                timed_out: false
            }
        );
        let error = live_result(ended(ExecState::Cancelled, killed.clone(), None), false)
            .expect_err("a child killed by nobody the host knows is refused");
        assert_eq!(error.reason, ConfinementRefusal::CapabilityUnserved);
        assert!(error.detail.contains("state cancelled"), "{error}");

        let timed_out = live_result(
            ended(ExecState::Cancelled, killed.clone(), Some("exec.timeout")),
            false,
        )
        .expect("a child its timeout ended is a result");
        assert!(timed_out.exit.timed_out);
        assert_eq!(timed_out.exit.exit_code, None);

        let error = live_result(
            ended(
                ExecState::Cancelled,
                killed,
                Some("session.output-backpressure"),
            ),
            true,
        )
        .expect_err("a child Substrate ended because its output was not read is refused");
        assert_eq!(error.reason, ConfinementRefusal::CapabilityUnserved);
        assert!(
            error.detail.contains("session.output-backpressure"),
            "{error}"
        );

        let interrupted = live_result(
            ended(
                ExecState::Exited,
                ExecExit {
                    code: Some(130),
                    signal: None,
                },
                None,
            ),
            true,
        )
        .expect("a child that exited after the host's signal is a result");
        assert_eq!(interrupted.exit.exit_code, Some(130));
    }

    #[test]
    fn a_driver_missing_a_pipe_session_fact_serves_no_live_child() {
        let facts = |exec: serde_json::Value| -> CapabilityFacts {
            let mut members = serde_json::json!({
                "operation.ledger-subject-max-rows": 1,
                "operation.ledger-subject-max-bytes": 1,
                "operation.ledger-global-max-rows": 1,
                "operation.ledger-global-max-bytes": 1,
            });
            members
                .as_object_mut()
                .unwrap()
                .extend(exec.as_object().unwrap().clone());
            serde_json::from_value(members).unwrap()
        };
        let served = serde_json::json!({
            "exec.namespaces": {"user": true, "mount": true, "pid": true, "ipc": true, "uts": true, "network": true},
            "exec.cgroup-limits": {"processes": true, "memory": true, "cpu": true},
            "exec.cgroup-kill": true,
            "exec.no-egress": true,
            "exec.output-limit-bytes": 1_048_576,
            "exec.signals": ["INT", "TERM", "KILL"],
        });
        check_pipe_sessions(&facts(served.clone())).expect("every fact served");
        for (fact, value) in [
            (
                "exec.namespaces",
                serde_json::json!({"user": true, "mount": true, "pid": true, "ipc": true, "uts": true, "network": false}),
            ),
            (
                "exec.cgroup-limits",
                serde_json::json!({"processes": true, "memory": false, "cpu": true}),
            ),
            ("exec.cgroup-kill", serde_json::json!(false)),
            ("exec.no-egress", serde_json::json!(false)),
            ("exec.output-limit-bytes", serde_json::json!(0)),
            ("exec.signals", serde_json::json!(["INT", "KILL"])),
        ] {
            for exec in [
                {
                    let mut exec = served.clone();
                    exec[fact] = value.clone();
                    exec
                },
                {
                    let mut exec = served.clone();
                    exec.as_object_mut().unwrap().remove(fact);
                    exec
                },
            ] {
                let error = check_pipe_sessions(&facts(exec))
                    .expect_err("a fact missing or not served refuses the live child");
                assert_eq!(
                    error.reason,
                    ConfinementRefusal::CapabilityUnserved,
                    "{fact}"
                );
            }
        }
    }

    #[test]
    fn a_live_childs_frame_names_its_stream() {
        for (stream, expected) in [
            (substrate_host::PipeStream::Stdout, ChildStream::Stdout),
            (substrate_host::PipeStream::Stderr, ChildStream::Stderr),
        ] {
            let frame = ChildFrame::from(substrate_host::PipeFrame {
                stream,
                bytes: b"bytes".to_vec(),
            });
            assert_eq!(
                frame,
                ChildFrame {
                    stream: expected,
                    bytes: b"bytes".to_vec()
                }
            );
        }
        assert_eq!(ChildSignal::Interrupt.wire(), Signal::Int);
        assert_eq!(ChildSignal::Terminate.wire(), Signal::Term);
        assert_eq!(ChildSignal::Kill.wire(), Signal::Kill);
    }

    #[test]
    fn a_record_binding_a_read_only_file_the_request_did_not_name_is_refused() {
        let child = ChildCommand::new("true", [] as [&str; 0], "/work/space");
        let input = child.input("space", "snapshot".to_owned());
        let mut record = record_for(&input, AppliedNetwork::None);
        check_record(&input, &[], &record).unwrap();
        record.read_only_files.push(substrate_wire::ReadOnlyFile {
            host_path: "/etc/hostname".to_owned(),
            mount: "/etc/hostname".to_owned(),
        });
        let error = check_record(&input, &[], &record).unwrap_err();
        assert_eq!(error.reason, ConfinementRefusal::CapabilityUnserved);
        assert_eq!(
            error.detail,
            "Substrate's applied confinement does not match the required profile"
        );
    }

    /// The story's case, on the check every launch shares: a child's read-only roots and files go
    /// to Substrate as named, a record naming exactly them is accepted and projected, and a record
    /// naming none, another, or one more is refused, for a child run to its end and a live child
    /// alike.
    #[test]
    fn a_record_naming_other_read_only_roots_is_refused() {
        let root = |host: &str, mount: &str| substrate_wire::ReadOnlyRoot {
            host_path: host.to_owned(),
            mount: mount.to_owned(),
        };
        let file = |host: &str, mount: &str| substrate_wire::ReadOnlyFile {
            host_path: host.to_owned(),
            mount: mount.to_owned(),
        };
        let child = ChildCommand::new("true", [] as [&str; 0], "/work/space")
            .with_read_only_root("/srv/tools", "/opt/tools")
            .with_read_only_file("/srv/policy.toml", "/etc/loom/policy.toml");
        let run = child.input("space", "snapshot".to_owned());
        let live = child.session_input("space", "snapshot".to_owned()).exec;
        for input in [&run, &live] {
            assert_eq!(input.read_only_roots, [root("/srv/tools", "/opt/tools")]);
            assert_eq!(
                input.read_only_files,
                [file("/srv/policy.toml", "/etc/loom/policy.toml")]
            );
            let exact = record_for(input, AppliedNetwork::None);
            check_record(input, &[], &exact).unwrap();
            let others = [
                (Vec::new(), exact.read_only_files.clone()),
                (
                    vec![root("/srv/other", "/opt/tools")],
                    exact.read_only_files.clone(),
                ),
                (
                    vec![root("/srv/tools", "/opt/other")],
                    exact.read_only_files.clone(),
                ),
                (
                    vec![
                        root("/srv/tools", "/opt/tools"),
                        root("/srv/more", "/opt/more"),
                    ],
                    exact.read_only_files.clone(),
                ),
                (exact.read_only_roots.clone(), Vec::new()),
                (
                    exact.read_only_roots.clone(),
                    vec![file("/srv/other.toml", "/etc/loom/policy.toml")],
                ),
                (
                    exact.read_only_roots.clone(),
                    vec![file("/srv/policy.toml", "/etc/loom/other.toml")],
                ),
            ];
            for (read_only_roots, read_only_files) in others {
                let record = substrate_wire::AppliedConfinement {
                    read_only_roots,
                    read_only_files,
                    ..exact.clone()
                };
                let error = check_record(input, &[], &record).unwrap_err();
                assert_eq!(error.reason, ConfinementRefusal::CapabilityUnserved);
            }
            let ending = live_ending(input, AppliedNetwork::None, ExecState::Exited, EXITED, None);
            let settled = settle(input, &[], "digest".to_owned(), ending).unwrap();
            let result = live_result(settled, false).unwrap();
            assert_eq!(
                result.applied.read_only_roots,
                [ToolchainRoot {
                    host_path: "/srv/tools".to_owned(),
                    mount: "/opt/tools".to_owned(),
                }]
            );
            assert_eq!(
                result.applied.read_only_files,
                [ReadOnlyFile {
                    host_path: "/srv/policy.toml".to_owned(),
                    mount: "/etc/loom/policy.toml".to_owned(),
                }]
            );
        }
    }

    #[test]
    fn an_egress_aperture_naming_no_host_or_one_substrate_cannot_serve_is_refused() {
        check_network(NetworkMode::None, &[]).unwrap();
        check_network(NetworkMode::Aperture, &[model()]).unwrap();
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
            let error = check_network(network, egress.as_slice()).unwrap_err();
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
            check_network(
                NetworkMode::Aperture,
                std::slice::from_ref(&with_host(host)),
            )
            .unwrap_or_else(|error| panic!("{host:?}: {error}"));
        }
        // 127.0.0.1, where Substrate's forwarder listens in the sandbox, is the one literal a
        // child reaches the host by, and only pinned there.
        let at_forwarder = EgressAperture {
            host: "127.0.0.1".to_owned(),
            pinned: "127.0.0.1:443".parse().unwrap(),
            ..model()
        };
        check_network(NetworkMode::Aperture, std::slice::from_ref(&at_forwarder)).unwrap();
        let error = check_network(
            NetworkMode::Aperture,
            std::slice::from_ref(&with_host("127.0.0.1")),
        )
        .unwrap_err();
        assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid);
        assert!(error.detail.contains("pinned to 10.0.0.1"), "{error}");
        for (host, pinned) in [
            ("10.0.0.1", "10.0.0.1:443"),
            ("10.0.0.2", "10.0.0.1:443"),
            ("127.0.0.2", "127.0.0.2:443"),
            ("10.0.1", "10.0.0.1:443"),
            ("1", "10.0.0.1:443"),
        ] {
            let aperture = EgressAperture {
                host: host.to_owned(),
                pinned: pinned.parse().unwrap(),
                ..model()
            };
            let error =
                check_network(NetworkMode::Aperture, std::slice::from_ref(&aperture)).unwrap_err();
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
            "::1",
            "[::1]",
            "#api.example.test",
            label_too_long.as_str(),
            name_too_long.as_str(),
        ] {
            let error = check_network(
                NetworkMode::Aperture,
                std::slice::from_ref(&with_host(host)),
            )
            .unwrap_err();
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
            let error =
                check_network(NetworkMode::Aperture, std::slice::from_ref(&aperture)).unwrap_err();
            assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{pinned}");
            assert!(error.detail.contains("not the port it names"), "{error}");
        }
    }

    #[test]
    fn a_child_with_egress_asks_for_its_aperture_by_name_and_its_profile_names_the_destination() {
        let child = ChildCommand::new("true", [] as [&str; 0], "/work/space").with_egress(model());
        assert_eq!(child.network(), NetworkMode::Aperture);
        assert_eq!(child.egress(), [model()]);
        let input = child.input("space", "snapshot".to_owned());
        assert_eq!(
            input.sandbox,
            ConfinementRequest {
                capability_snapshot: "snapshot".to_owned(),
                network: NetworkMode::Aperture,
                aperture: Some("model".to_owned()),
                apertures: Vec::new(),
                profile: SandboxProfile::Workspace,
                required: true,
            }
        );
        let profile = requested_profile(&input, &[model()]);
        assert_eq!(profile.network, "aperture:model@10.0.0.1:443");
        assert_eq!(
            applied_apertures(&applied_aperture("model", PINNED, None)),
            [AppliedAperture {
                name: "model".to_owned(),
                destination: PINNED.to_owned(),
                listen: None,
            }]
        );
        assert!(applied_apertures(&AppliedNetwork::None).is_empty());
        let digest = profile_digest(&profile, &[], &[model()], None, &[], None);
        assert_ne!(digest, profile_digest(&profile, &[], &[], None, &[], None));
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
            assert_ne!(
                digest,
                profile_digest(&profile, &[], std::slice::from_ref(&other), None, &[], None)
            );
        }
    }

    #[test]
    fn inherited_baseline_names_change_the_digest_and_their_absence_does_not() {
        let child = ChildCommand::new("true", [] as [&str; 0], "/work/space");
        let input = child.input("space", "snapshot".to_owned());
        let profile = requested_profile(&input, &[]);
        assert_eq!(profile.network, "none");
        assert_eq!(profile.writable_scopes, ["target"]);
        assert_ne!(
            profile_digest(&profile, &[], &[], None, &[], None),
            profile_digest(&profile, &[BaselineEnvironment::Path], &[], None, &[], None)
        );
        // With no inherited name the digest is the one a test's profile always had.
        let before = serde_json::json!({ "backend":"substrate", "network":"none", "writable_scopes":["target"],
            "read_only_roots":[], "environment":[],
            "timeout_ms":300_000, "memory_bytes":8_589_934_592_i64, "processes":2048, "cpu_millis":3_600_000 });
        assert_eq!(
            profile_digest(&profile, &[], &[], None, &[], None),
            format!("{:x}", Sha256::digest(before.to_string().as_bytes()))
        );
    }

    #[test]
    fn read_only_files_change_the_digest_and_their_absence_does_not() {
        let child = ChildCommand::new("true", [] as [&str; 0], "/work/space")
            .with_read_only_file("/srv/policy.toml", "/etc/loom/policy.toml");
        let input = child.input("space", "snapshot".to_owned());
        let profile = requested_profile(&input, &[]);
        let digest = profile_digest(&profile, &[], &[], None, &input.read_only_files, None);
        assert_ne!(digest, profile_digest(&profile, &[], &[], None, &[], None));
        for other in [
            ("/srv/other.toml", "/etc/loom/policy.toml"),
            ("/srv/policy.toml", "/etc/loom/other.toml"),
        ] {
            let other = [substrate_wire::ReadOnlyFile {
                host_path: other.0.to_owned(),
                mount: other.1.to_owned(),
            }];
            assert_ne!(
                digest,
                profile_digest(&profile, &[], &[], None, &other, None)
            );
        }
        // Roots are in the profile itself, so a root changes the digest with no file named.
        let rooted = ChildCommand::new("true", [] as [&str; 0], "/work/space")
            .with_read_only_root("/srv/tools", "/opt/tools")
            .input("space", "snapshot".to_owned());
        assert_ne!(
            profile_digest(&requested_profile(&rooted, &[]), &[], &[], None, &[], None),
            profile_digest(&profile, &[], &[], None, &[], None)
        );
    }

    /// A bundle as a launch would have read it from `named`.
    fn read_as(named: &str, bytes: &[u8]) -> Option<CaBundle> {
        Some(CaBundle {
            named: PathBuf::from(named),
            bytes: bytes.to_vec(),
        })
    }

    #[test]
    fn drivers_differ_by_the_bundle_path_and_the_bytes_read_from_it() {
        let key = |ca_bundle| DriverEgress {
            apertures: vec![model()],
            ca_bundle,
        };
        let first = key(read_as("/anchors/first.pem", b"one"));
        assert_eq!(
            first,
            key(read_as("/anchors/first.pem", b"one")),
            "one path and one content, one driver"
        );
        for other in [
            key(read_as("/anchors/second.pem", b"one")),
            key(read_as("/anchors/first.pem", b"two")),
            key(None),
            DriverEgress::NONE,
        ] {
            assert_ne!(first, other, "{other:?}");
        }
        // The bundle is the driver's, never the start request's.
        let egress = ChildCommand::new("true", [] as [&str; 0], "/work/space").with_egress(model());
        let named = egress.clone().with_ca_bundle("/anchors/first.pem");
        assert_eq!(named.ca_bundle(), Some(Path::new("/anchors/first.pem")));
        assert_eq!(
            named.input("space", "snapshot".to_owned()),
            egress.input("space", "snapshot".to_owned())
        );
    }

    #[test]
    fn the_driver_hands_substrate_a_private_read_only_copy_of_the_bytes_read() {
        use std::os::unix::fs::PermissionsExt as _;
        let fixture = tempfile::tempdir().unwrap();
        let named = fixture.path().join("bundle.pem");
        let elsewhere = fixture.path().join("elsewhere.pem");
        std::fs::write(&named, "anchor\n").unwrap();
        std::fs::write(&elsewhere, "link target\n").unwrap();
        let child = ChildCommand::new("true", [] as [&str; 0], "/work/space")
            .with_egress(model())
            .with_ca_bundle(&named);
        let bundle = check_ca_bundle(&child).unwrap();
        assert_eq!(bundle, read_as(named.to_str().unwrap(), b"anchor\n"));

        let state = tempfile::tempdir().unwrap();
        let egress = DriverEgress {
            apertures: vec![model()],
            ca_bundle: bundle,
        };
        let config = driver_config(fixture.path(), state.path(), &egress).unwrap();
        let copy = config.ca_bundle.expect("Substrate is handed a bundle");
        assert_ne!(copy, named, "never the named path");
        assert_eq!(copy.parent(), Some(state.path()), "in the private state");
        let metadata = std::fs::symlink_metadata(&copy).unwrap();
        assert!(metadata.is_file(), "a regular file, no link");
        assert_eq!(metadata.permissions().mode() & 0o777, 0o400);
        assert_eq!(std::fs::read(&copy).unwrap(), b"anchor\n");
        // The named path swapped for a link after the read changes nothing Substrate copies.
        std::fs::remove_file(&named).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &named).unwrap();
        assert_eq!(std::fs::read(&copy).unwrap(), b"anchor\n");
        let error = check_ca_bundle(&child).unwrap_err();
        assert!(error.detail.contains("symbolic link"), "{error}");
        // A copy is written once: a second one in the same state is refused, never overwritten.
        assert_eq!(
            driver_config(fixture.path(), state.path(), &egress)
                .unwrap_err()
                .reason,
            ConfinementRefusal::BackendMissing
        );
        let none = tempfile::tempdir().unwrap();
        let plain = driver_config(fixture.path(), none.path(), &DriverEgress::NONE).unwrap();
        assert!(plain.ca_bundle.is_none());
        assert!(!none.path().join("ca-bundle.pem").exists());
    }

    #[test]
    fn a_ca_bundle_changes_the_digest_and_its_absence_leaves_an_egress_childs_as_it_was() {
        let input = ChildCommand::new("true", [] as [&str; 0], "/work/space")
            .with_egress(model())
            .input("space", "snapshot".to_owned());
        let profile = requested_profile(&input, &[model()]);
        let digest = |bundle: Option<CaBundle>| {
            profile_digest(&profile, &[], &[model()], bundle.as_ref(), &[], None)
        };
        let bare = digest(None);
        let first = digest(read_as("/anchors/first.pem", b"one"));
        assert_ne!(first, bare);
        assert_ne!(first, digest(read_as("/anchors/second.pem", b"one")));
        assert_ne!(first, digest(read_as("/anchors/first.pem", b"two")));
        // With no bundle the digest is the one an egress child had before bundles existed.
        let mut before = serde_json::json!({ "backend":"substrate", "network":"aperture:model@10.0.0.1:443",
            "writable_scopes":["target"], "read_only_roots":[], "environment":[],
            "timeout_ms":300_000, "memory_bytes":8_589_934_592_i64, "processes":2048, "cpu_millis":3_600_000 });
        before["egress"] = serde_json::json!({ "name":"model", "host":"api.example.test", "port":443,
            "pinned":PINNED, "max_bytes":null });
        assert_eq!(
            bare,
            format!("{:x}", Sha256::digest(before.to_string().as_bytes()))
        );
    }

    #[test]
    fn every_trust_store_variable_beside_a_ca_bundle_is_refused_naming_it_and_documented() {
        let anchored = |set: BTreeMap<String, String>| {
            ChildCommand::new("true", [] as [&str; 0], "/work/space")
                .with_egress(model())
                .with_environment(ExecEnvironment {
                    allow: Vec::new(),
                    set,
                })
                .with_ca_bundle("/anchors/absent.pem")
        };
        // The public rustdoc of `with_ca_bundle` names every variable the constant holds.
        let source = include_str!("confinement.rs");
        let lines: Vec<&str> = source.lines().collect();
        let method = lines
            .iter()
            .position(|line| line.contains("pub fn with_ca_bundle("))
            .expect("with_ca_bundle is defined here");
        let doc: String = lines[..method]
            .iter()
            .rev()
            .take_while(|line| {
                let line = line.trim_start();
                line.starts_with("///") || line.starts_with("#[")
            })
            .copied()
            .collect();
        for variable in TRUST_STORE_VARIABLES {
            let child = anchored(BTreeMap::from([(variable.to_owned(), "/x".to_owned())]));
            let error = check_ca_bundle(&child).unwrap_err();
            assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{variable}");
            assert!(error.detail.contains(variable), "{variable}: {error}");
            assert!(
                doc.contains(&format!("`{variable}`")),
                "the with_ca_bundle rustdoc names {variable}"
            );
        }
        for key in GIT_TRUST_KEYS {
            assert!(
                doc.contains(&format!("`http.{key}`")),
                "the with_ca_bundle rustdoc names http.{key}"
            );
        }
        assert!(doc.contains("`GIT_CONFIG_KEY_<n>`"));
        let both = anchored(BTreeMap::from([
            ("CURL_CA_BUNDLE".to_owned(), "/x".to_owned()),
            ("SSL_CERT_DIR".to_owned(), "/x".to_owned()),
        ]));
        let error = check_ca_bundle(&both).unwrap_err();
        assert!(
            error
                .detail
                .contains("naming CURL_CA_BUNDLE, SSL_CERT_DIR, "),
            "{error}"
        );
        let git = anchored(BTreeMap::from([
            ("GIT_CONFIG_COUNT".to_owned(), "1".to_owned()),
            ("GIT_CONFIG_KEY_0".to_owned(), "http.sslVerify".to_owned()),
            ("GIT_CONFIG_VALUE_0".to_owned(), "false".to_owned()),
        ]));
        let error = check_ca_bundle(&git).unwrap_err();
        assert!(
            error
                .detail
                .contains("naming GIT_CONFIG_KEY_0 (http.sslVerify), "),
            "{error}"
        );
        // Another variable passes this step, so the absent path is what is refused.
        let other = anchored(BTreeMap::from([("FIRST".to_owned(), "one".to_owned())]));
        let error = check_ca_bundle(&other).unwrap_err();
        assert!(error.detail.contains("does not exist"), "{error}");
    }

    #[test]
    fn only_a_git_config_key_naming_a_trust_key_overrides_the_trust_store() {
        for key in [
            "http.sslCAInfo",
            "http.sslcapath",
            "HTTP.SSLVERIFY",
            " http.sslCAInfo ",
            "http.https://model.example.test/.sslCAInfo",
            "Http.https://model.example.test:443/a.b.SslVerify",
        ] {
            assert!(names_git_trust_key(key), "{key:?}");
            assert!(overrides_trust_store("GIT_CONFIG_KEY_0", key), "{key:?}");
            assert!(overrides_trust_store("GIT_CONFIG_KEY_12", key), "{key:?}");
        }
        for key in [
            "safe.directory",
            "http.sslBackend",
            "http.sslCAInfoExtra",
            "https.sslCAInfo",
            "core.sslCAInfo",
            "sslCAInfo",
            "http",
            "",
        ] {
            assert!(!names_git_trust_key(key), "{key:?}");
            assert!(!overrides_trust_store("GIT_CONFIG_KEY_0", key), "{key:?}");
        }
        // Only a key Git reads: GIT_CONFIG_KEY_ and decimal digits, never a value or the count.
        for name in [
            "GIT_CONFIG_KEY_",
            "GIT_CONFIG_KEY_X",
            "GIT_CONFIG_KEY_0A",
            "GIT_CONFIG_VALUE_0",
            "GIT_CONFIG_COUNT",
        ] {
            assert!(!overrides_trust_store(name, "http.sslCAInfo"), "{name}");
        }
        for name in TRUST_STORE_VARIABLES {
            assert!(overrides_trust_store(name, ""), "{name}");
        }
    }

    #[test]
    fn a_ca_bundle_path_that_is_not_utf8_is_refused() {
        use std::os::unix::ffi::OsStrExt as _;
        let child = ChildCommand::new("true", [] as [&str; 0], "/work/space")
            .with_egress(model())
            .with_ca_bundle(std::ffi::OsStr::from_bytes(b"/anchors/bundle\xff.pem"));
        let error = check_ca_bundle(&child).unwrap_err();
        assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid);
        assert!(error.detail.contains("not UTF-8"), "{error}");
        assert!(error.detail.contains("bundle\\xFF.pem"), "{error}");
        assert_eq!(
            check_ca_bundle(&ChildCommand::new("true", [] as [&str; 0], "/work/space")).unwrap(),
            None
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
