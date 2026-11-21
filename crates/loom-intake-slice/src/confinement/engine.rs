//! Loom's confinement named once, as engine configuration ([`EngineConfinement`]), and the one
//! runner built from it ([`EngineRunner::open`]).
//!
//! Two launch routes exist side by side, a stated intermediate state
//! (`story:confinement-opens-from-engine-config`): `local` runs on Substrate's confinement
//! contract, through the `local` adapter the constructor opens; `linux` stays on the host-driver
//! route [`SubstrateRunner`] has always taken, until `story:child-launches-through-confinement-adapters`
//! moves it onto the contract.
//!
//! Under `local` a child runs unconfined, declared insecure: as the operator, with the host's file
//! system and network, and the operator's reason on every record. What `local` cannot serve is
//! refused before launch, never dropped: egress apertures and their bundle (`network_egress`),
//! read-only roots and files (`filesystem_view`). `local` serves no limits, so Loom holds every
//! launch's timeout itself and at the deadline kills the run's process group through the adapter.
//! Each launch starts, reads and waits inside one runtime it owns, because `local` spawns a run's
//! drain and reaper tasks on the caller's runtime; the adapter is shared by every launch of the
//! runner, a run is not.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use substrate_host::confinement::local::{LOCAL_POLICY_ENV, LocalRequest};
use substrate_host::confinement::{
    AdmittedProfile, ConfinementAdapter as Adapter, FACT_ORDER, Fact, LOCAL_REFUSED,
    OutputDelivery, OutputEvent, PipeStream, RunExit, RunId, RunObservation, RunOutput, RunState,
    STDIN_CHUNK_BYTES, StdinMode,
};
use substrate_wire::{BaselineEnvironment, Signal};

use super::child::{ChildError, ChildFrame, ChildResult, ChildSignal, ConfinedChild, SIGNAL_GRACE};
use super::{
    AppliedConfinement, Backend, ChildCommand, ChildExit, ChildOutput, ChildStream,
    ConfinementAdapter, ConfinementError, ConfinementRefusal, EngineConfinement, NetworkMode,
    QUEUED_FRAMES, READ_STEP, SANDBOX_WORKSPACE, SubstrateRunner, TestExecution, TestRunner,
    canonical_workspace, check_program, check_stdin, check_workdir, check_writable, enum_name,
    escape_reason, local_label, refuse, scope_error, tail,
};
use crate::executor::TestCommand;

/// The runner a host builds from its [`EngineConfinement`], once: a [`TestRunner`] whose
/// [`EngineRunner::run_child`] (standard input included) and [`EngineRunner::spawn_child`] run
/// under the adapter the configuration names. A request never names an adapter.
///
/// Under `linux` it wraps [`SubstrateRunner`] unchanged: building it opens nothing, so it ends no
/// run under the cgroup root, and each launch opens its driver as before. Under `local` it holds
/// the `local` adapter it opened, shared by every launch and every clone.
#[derive(Clone)]
pub struct EngineRunner {
    route: Route,
}

#[derive(Clone)]
enum Route {
    Linux(SubstrateRunner),
    Local(Arc<LocalRoute>),
}

impl fmt::Debug for EngineRunner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.route {
            Route::Linux(runner) => f.debug_tuple("EngineRunner::Linux").field(runner).finish(),
            Route::Local(local) => f
                .debug_struct("EngineRunner::Local")
                .field("local_reason", &local.reason)
                .field("facts", &local.facts)
                .finish_non_exhaustive(),
        }
    }
}

impl EngineRunner {
    /// The runner `confinement` names. Under `linux`, [`SubstrateRunner`] on its `cgroup_root`,
    /// opening nothing. Under `local`, Substrate's `local` adapter, opened through Substrate's
    /// confinement contract with the operator's reason.
    ///
    /// Refused before any child starts:
    ///
    /// - [`ConfinementRefusal::LocalReasonMissing`]: `local` with no reason, or one Substrate reads
    ///   as blank (white space, format characters, symbols alone: `ConfinementProfile::reason`);
    /// - [`ConfinementRefusal::CapabilityUnserved`]: a host whose policy forbids `local`
    ///   (`SUBSTRATE_CONFINEMENT_LOCAL` set to anything but `permit`, Substrate's
    ///   `confinement.local-refused`), read from the opened adapter's `local_permitted` fact;
    /// - [`ConfinementRefusal::ScopeInvalid`]: a `cgroup_root` beside `local`, which has none, and
    ///   a `local_reason` beside `linux`, which takes none.
    pub fn open(confinement: &EngineConfinement) -> Result<Self, ConfinementError> {
        let route = match confinement.adapter {
            ConfinementAdapter::Linux => {
                if confinement.local_reason.is_some() {
                    return Err(scope_error(
                        "a local_reason is the reason for the local adapter; linux takes none",
                    ));
                }
                Route::Linux(SubstrateRunner::new(
                    confinement.cgroup_root.as_ref().map(PathBuf::from),
                ))
            }
            ConfinementAdapter::Local => {
                if confinement.cgroup_root.is_some() {
                    return Err(scope_error(
                        "a cgroup_root names the delegated root of the linux adapter; local has none",
                    ));
                }
                let reason = local_reason(confinement.local_reason.as_deref())?;
                // Named here alone: no other function in the crates' sources names an adapter
                // request (`tests/engine_confinement.rs` fails on one).
                use substrate_host::confinement::AdapterRequest;
                let adapter = substrate_host::confinement::open(AdapterRequest::Local(
                    LocalRequest::default(),
                ))
                .map_err(contract_error)?;
                Route::Local(Arc::new(LocalRoute::new(adapter, reason)?))
            }
        };
        Ok(Self { route })
    }

    /// Check the workspace `root` before a model session opens. Under `linux`, the driver opens
    /// as [`SubstrateRunner::preflight`] opens it; under `local`, the workspace is a directory
    /// reached through no symbolic link. No child starts.
    pub fn preflight(&self, root: &Path) -> Result<(), ConfinementError> {
        match &self.route {
            Route::Linux(runner) => runner.preflight(root),
            Route::Local(_) => canonical_workspace(root).map(drop),
        }
    }

    /// Run `child` to its end under the configured adapter: as [`SubstrateRunner::run_child`]
    /// does under `linux`; under `local` unconfined, declared insecure, its result recording
    /// adapter `local`, the operator's reason and the facts `local` proved, with no driver record.
    pub fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        match &self.route {
            Route::Linux(runner) => runner.run_child(child),
            Route::Local(local) => local.run_child(child),
        }
    }

    /// Start `child` live under the configured adapter, as [`SubstrateRunner::spawn_child`] does
    /// under `linux`; under `local` its result carries no driver record
    /// ([`ChildResult::driver_record`] is `None`).
    pub fn spawn_child(&self, child: &ChildCommand) -> Result<ConfinedChild, ConfinementError> {
        match &self.route {
            Route::Linux(runner) => runner.spawn_child(child),
            Route::Local(local) => local.spawn_child(child),
        }
    }

    /// The path a child of this runner opens for the host path `path`: under `linux`,
    /// [`ChildCommand::sandbox_path`]; under `local`, where paths are host paths, the canonical
    /// host path itself wherever `sandbox_path` maps one, and `None` where it maps none.
    pub fn child_path(&self, child: &ChildCommand, path: &Path) -> Option<PathBuf> {
        let mapped = child.sandbox_path(path)?;
        match &self.route {
            Route::Linux(_) => Some(mapped),
            Route::Local(_) => Some(path.canonicalize().unwrap_or_else(|_| path.to_owned())),
        }
    }
}

impl TestRunner for EngineRunner {
    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        match &self.route {
            Route::Linux(runner) => runner.run(command, root),
            Route::Local(local) => local.run(command, root),
        }
    }

    /// `Substrate` under `linux`; `None` under `local`, which applies no confinement.
    fn backend(&self) -> Backend {
        match &self.route {
            Route::Linux(runner) => runner.backend(),
            Route::Local(_) => Backend::None,
        }
    }

    /// `substrate` under `linux`; under `local`, [`local_label`] of the reason.
    fn label(&self) -> String {
        match &self.route {
            Route::Linux(runner) => runner.label(),
            Route::Local(local) => local_label(&local.reason),
        }
    }

    fn local_reason(&self) -> Option<&str> {
        match &self.route {
            Route::Linux(_) => None,
            Route::Local(local) => Some(&local.reason),
        }
    }
}

/// The operator's reason, refused [`ConfinementRefusal::LocalReasonMissing`] when absent or
/// blank by Substrate's own rule.
fn local_reason(reason: Option<&str>) -> Result<String, ConfinementError> {
    let profile = substrate_host::confinement::ConfinementProfile {
        local_reason: reason.map(str::to_owned),
        ..Default::default()
    };
    profile.reason().map(str::to_owned).ok_or_else(|| {
        refuse(
            ConfinementRefusal::LocalReasonMissing,
            format!(
                "{}: the local adapter runs a child unconfined, declared insecure, and needs the operator's reason",
                substrate_host::confinement::LOCAL_REASON_MISSING
            ),
        )
    })
}

/// A refusal of Substrate's confinement contract or of the `local` adapter, as Loom's: a reason
/// missing is [`ConfinementRefusal::LocalReasonMissing`], a host refusal of what was asked
/// (`exec.environment-refused`, `exec.workdir-missing`, …) [`ConfinementRefusal::ScopeInvalid`],
/// anything else [`ConfinementRefusal::CapabilityUnserved`]; the detail names Substrate's code.
fn contract_error(error: substrate_host::confinement::ConfinementError) -> ConfinementError {
    use substrate_host::DriverErrorClass;
    use substrate_host::confinement::ConfinementError as Contract;
    let reason = match &error {
        Contract::LocalReasonMissing { .. } => ConfinementRefusal::LocalReasonMissing,
        Contract::Driver(driver) if driver.class == DriverErrorClass::Refused => {
            ConfinementRefusal::ScopeInvalid
        }
        _ => ConfinementRefusal::CapabilityUnserved,
    };
    refuse(
        reason,
        format!("Substrate's local adapter: {}: {error}", error.code()),
    )
}

/// A refusal of a live child's call, as [`ChildError`].
fn child_error(error: substrate_host::confinement::ConfinementError) -> ChildError {
    if error.code() == substrate_wire::SESSION_READ_TIMEOUT {
        ChildError::ReadTimedOut
    } else {
        ChildError::Session {
            code: error.code().to_owned(),
            detail: error.to_string(),
        }
    }
}

/// The `local` route: the adapter the constructor opened, the operator's reason and the facts
/// the adapter proved, in `FACT_ORDER`'s spelling.
pub(super) struct LocalRoute {
    adapter: Arc<dyn Adapter>,
    reason: String,
    facts: Vec<String>,
}

impl LocalRoute {
    /// The route over `adapter`, refused [`ConfinementRefusal::CapabilityUnserved`] when the
    /// host's policy forbids `local`: the adapter then did not prove `local_permitted`.
    fn new(adapter: Arc<dyn Adapter>, reason: String) -> Result<Self, ConfinementError> {
        let proof = adapter.facts();
        if !proof.proves(&Fact::LocalPermitted) {
            return Err(refuse(
                ConfinementRefusal::CapabilityUnserved,
                format!(
                    "{LOCAL_REFUSED}: the host's policy ({LOCAL_POLICY_ENV}) forbids Substrate's local adapter"
                ),
            ));
        }
        let facts = FACT_ORDER
            .iter()
            .filter(|fact| proof.proves(fact))
            .map(enum_name)
            .collect();
        Ok(Self {
            adapter,
            reason,
            facts,
        })
    }

    /// What every `local` launch records: no confinement applied (`Backend::None`, filesystem
    /// `unconfined`, no network record), adapter `local`, the facts it proved and the reason.
    fn applied(&self) -> AppliedConfinement {
        AppliedConfinement {
            backend: Backend::None,
            profile_digest: String::new(),
            capability_snapshot: String::new(),
            cgroup: String::new(),
            filesystem: "unconfined".to_owned(),
            network: None,
            profile: "none".to_owned(),
            writable_scopes: Vec::new(),
            read_only_roots: Vec::new(),
            read_only_files: Vec::new(),
            workdir: None,
            adapter: Some("local".to_owned()),
            facts: Some(self.facts.clone()),
            // Recorded as it is printed, so no reader of the record splits or hides a line.
            local_reason: Some(escape_reason(&self.reason)),
        }
    }

    /// `profile` admitted on the adapter, with the operator's reason, the workspace opened
    /// read-write (`local` refuses a scoped one), no network request and no bounds.
    fn admit(
        &self,
        workspace: PathBuf,
        workdir: Option<String>,
        stdin: StdinMode,
        output_delivery: OutputDelivery,
    ) -> Result<AdmittedProfile, ConfinementError> {
        let profile = substrate_host::confinement::ConfinementProfile {
            local_reason: Some(self.reason.clone()),
            workspace: Some(workspace),
            workdir,
            stdin,
            output_delivery,
            ..Default::default()
        };
        self.adapter.admit(&profile).map_err(contract_error)
    }

    /// Every check a `local` child gets before launch: the checks of a `linux` child that hold
    /// for a host process, and a refusal of what `local` cannot serve. Returns the canonical
    /// workspace and the working directory relative to it.
    fn check(child: &ChildCommand) -> Result<(PathBuf, Option<String>), ConfinementError> {
        check_program(&child.program)?;
        check_stdin(child.stdin.as_deref())?;
        if !child.egress.is_empty()
            || child.network == NetworkMode::Aperture
            || child.ca_bundle.is_some()
        {
            return Err(refuse(
                ConfinementRefusal::CapabilityUnserved,
                "egress apertures and their certificate bundle (`with_egress`, `with_ca_bundle`) need the contract family `network_egress`, which Substrate's local adapter does not serve: it runs the child on the host's network",
            ));
        }
        if !child.read_only_roots.is_empty()
            || !child.read_only_files.is_empty()
            || !child.host_paths.is_empty()
        {
            return Err(refuse(
                ConfinementRefusal::CapabilityUnserved,
                "read-only roots and files (`with_read_only_root`, `with_read_only_file`, `at_host_path`) need the contract family `filesystem_view`, which Substrate's local adapter does not serve: the child sees the host's file system as the operator",
            ));
        }
        check_writable(&child.writable)?;
        check_workdir(child)?;
        let workspace = canonical_workspace(&child.directory)?;
        Ok((workspace, relative_workdir(child.workdir.as_deref())))
    }

    /// `child`'s program, arguments and whole environment: each baseline variable it keeps at the
    /// value Substrate gives it, then the variables it sets.
    fn command(child: &ChildCommand) -> substrate_host::confinement::Command {
        let mut env: BTreeMap<String, String> = child
            .environment
            .allow
            .iter()
            .map(|name| (name.as_str().to_owned(), baseline_value(*name).to_owned()))
            .collect();
        env.extend(child.environment.set.clone());
        substrate_host::confinement::Command {
            argv: std::iter::once(&child.program)
                .chain(&child.args)
                .cloned()
                .collect(),
            env,
        }
    }

    fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        let (workspace, workdir) = Self::check(child)?;
        let stdin = match child.stdin {
            Some(_) => StdinMode::Piped {
                limit_bytes: ChildCommand::MAX_INPUT_BYTES as u64,
            },
            None => StdinMode::Null,
        };
        let admitted = self.admit(workspace, workdir, stdin, OutputDelivery::Recorded)?;
        let ended = self.launch(
            &admitted,
            &Self::command(child),
            child.stdin.as_deref(),
            child.timeout,
        )?;
        self.execution(ended)
    }

    fn spawn_child(
        self: &Arc<Self>,
        child: &ChildCommand,
    ) -> Result<ConfinedChild, ConfinementError> {
        if child.stdin.is_some() {
            return Err(scope_error(
                "a live child is given standard input through ConfinedChild::write_stdin, never ChildCommand::with_stdin",
            ));
        }
        let (workspace, workdir) = Self::check(child)?;
        let admitted = self.admit(
            workspace,
            workdir,
            StdinMode::Piped {
                limit_bytes: ChildCommand::MAX_INPUT_BYTES as u64,
            },
            OutputDelivery::Live {
                queued_chunks: QUEUED_FRAMES,
            },
        )?;
        LocalChild::start(
            Arc::clone(self),
            &admitted,
            &Self::command(child),
            child.timeout,
        )
        .map(ConfinedChild::local)
    }

    /// A test command run as the unconfined runner runs it: in the workspace directory with the
    /// host's toolchain, no read-only root and no private cargo home, its environment the host's
    /// `PATH`, `HOME`, `RUSTUP_HOME` and `CARGO_HOME` (each where set), the workspace's `target`
    /// as `CARGO_TARGET_DIR`, and today's `CARGO_NET_OFFLINE`, `CARGO_BUILD_JOBS` and
    /// `RUSTUP_AUTO_INSTALL`. Nothing else is inherited.
    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        check_program(&command.program().to_string_lossy())?;
        let workspace = canonical_workspace(root)?;
        let argv = std::iter::once(command.program())
            .chain(command.args().iter().map(|arg| arg.as_os_str()))
            .map(|word| {
                word.to_str().map(str::to_owned).ok_or_else(|| {
                    scope_error("the local adapter's command arguments must be UTF-8")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let env = test_environment(&workspace)?;
        let admitted = self.admit(workspace, None, StdinMode::Null, OutputDelivery::Recorded)?;
        let ended = self.launch(
            &admitted,
            &substrate_host::confinement::Command { argv, env },
            None,
            command.timeout(),
        )?;
        let mut execution = self.execution(ended)?;
        if execution.exit_code != Some(0)
            && (execution.output.contains("offline")
                || execution.output.contains("no matching package"))
        {
            execution.output.push_str("\nDependencies are offline. Run `cargo fetch` explicitly on the host before retrying Loom; Loom never fetches dependencies.\n");
        }
        Ok(execution)
    }

    /// Start `command` under `admitted`, write `stdin` to it and close it when given, and wait
    /// for its end, all inside one runtime this call owns. At `timeout` the run is sent
    /// `Signal::Kill`, which `local` sends to its whole process group, and waited for; every path
    /// that returns once it started has ended it.
    fn launch(
        &self,
        admitted: &AdmittedProfile,
        command: &substrate_host::confinement::Command,
        stdin: Option<&[u8]>,
        timeout: Duration,
    ) -> Result<Ended, ConfinementError> {
        let deadline = deadline(timeout)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| refuse(ConfinementRefusal::BackendMissing, e))?;
        runtime.block_on(async {
            let run = self
                .adapter
                .start(admitted, command)
                .await
                .map_err(contract_error)?;
            let ended = match stdin {
                None => self.finish(&run, deadline).await,
                Some(bytes) => {
                    let (written, ended) =
                        tokio::join!(self.write_all(&run, bytes), self.finish(&run, deadline));
                    written.and(ended)
                }
            };
            let (observed, timed_out) = match ended {
                Ok(ended) => ended,
                Err(error) => {
                    let _ = self.kill(&run).await;
                    return Err(error);
                }
            };
            let output = self.adapter.output(&run).await.map_err(contract_error)?;
            Ok(Ended {
                observed,
                timed_out,
                output,
                timeout,
            })
        })
    }

    /// Write `bytes` to the run's standard input, then close it. A child that stopped reading or
    /// ended is no failure: it read what it read.
    async fn write_all(&self, run: &RunId, bytes: &[u8]) -> Result<(), ConfinementError> {
        for chunk in bytes.chunks(STDIN_CHUNK_BYTES) {
            match self.adapter.write_stdin(run, chunk).await {
                Ok(()) => {}
                Err(error)
                    if matches!(
                        error.code(),
                        substrate_wire::SESSION_WRITE_FAILED | substrate_wire::SESSION_INPUT_CLOSED
                    ) =>
                {
                    break;
                }
                Err(error) => return Err(contract_error(error)),
            }
        }
        self.adapter.close_stdin(run).await.map_err(contract_error)
    }

    /// The run's end, and whether Loom ended it at `deadline`.
    async fn finish(
        &self,
        run: &RunId,
        deadline: tokio::time::Instant,
    ) -> Result<(RunObservation, bool), ConfinementError> {
        match tokio::time::timeout_at(deadline, self.adapter.wait(run)).await {
            Ok(observed) => Ok((observed.map_err(contract_error)?, false)),
            Err(_) => Ok((self.kill(run).await?, true)),
        }
    }

    /// `Signal::Kill` to the run's process group, then its end.
    async fn kill(&self, run: &RunId) -> Result<RunObservation, ConfinementError> {
        kill(&*self.adapter, run).await
    }

    /// A run's result: refused when `local` reports no exit status and Loom did not end it at its
    /// deadline, as a confined child's is.
    fn execution(&self, ended: Ended) -> Result<TestExecution, ConfinementError> {
        let exit = ended.exit();
        if exit.exit_code.is_none() && !exit.timed_out {
            return Err(refuse(
                ConfinementRefusal::CapabilityUnserved,
                format!(
                    "Substrate's local adapter reports no exit status for the child and no timeout ended it ({})",
                    ended.described()
                ),
            ));
        }
        let output = ended.output();
        let mut text = format!(
            "{}{}",
            String::from_utf8_lossy(tail(&output.stdout)),
            String::from_utf8_lossy(tail(&output.stderr))
        );
        if output.stdout_truncated || output.stderr_truncated {
            text.push_str("\n[Substrate output truncated]\n");
        }
        if exit.timed_out {
            text.push_str(&ended.timeout_line());
        }
        Ok(TestExecution {
            exit_code: exit.exit_code.and_then(|code| i32::try_from(code).ok()),
            timed_out: exit.timed_out,
            output: text,
            applied: self.applied(),
            driver_record: None,
            child_output: output,
        })
    }
}

/// How one `local` run ended: its last observation, whether Loom ended it at its deadline, its
/// output as `local` kept it, and the timeout it ran under.
struct Ended {
    observed: RunObservation,
    timed_out: bool,
    output: RunOutput,
    timeout: Duration,
}

impl Ended {
    fn exit(&self) -> ChildExit {
        let exit = self.observed.exit.unwrap_or(RunExit {
            code: None,
            signal: None,
        });
        ChildExit {
            exit_code: if self.timed_out || self.observed.state != RunState::Ended {
                None
            } else {
                exit.code.map(i64::from)
            },
            signal: exit.signal.map(i64::from),
            timed_out: self.timed_out,
        }
    }

    fn output(&self) -> ChildOutput {
        ChildOutput {
            stdout: self.output.stdout.clone(),
            stderr: self.output.stderr.clone(),
            stdout_truncated: self.output.stdout_truncated,
            stderr_truncated: self.output.stderr_truncated,
        }
    }

    fn described(&self) -> String {
        let exit = self.observed.exit;
        format!(
            "state {:?}, exit code {:?}, signal {:?}",
            self.observed.state,
            exit.and_then(|exit| exit.code),
            exit.and_then(|exit| exit.signal)
        )
    }

    fn timeout_line(&self) -> String {
        format!(
            "\nexec.timeout: the child ran past its timeout of {:?}; Loom killed its process group\n",
            self.timeout
        )
    }
}

/// How long a run killed with its process group is waited for. `local` ends a run once its leader
/// is reaped, every holder of its output closed it and its group is empty (or
/// `LOCAL_GROUP_SETTLE`, 1 s, passed); a process that left the group and still holds the output
/// would keep it running, and Loom does not wait on it past this bound.
const KILL_SETTLE: Duration = Duration::from_secs(5);

/// `Signal::Kill` to `run`'s process group on `adapter`, then its end, waited for at most
/// [`KILL_SETTLE`]: a run still not ended then is refused
/// [`ConfinementRefusal::CapabilityUnserved`], since a process outside its group holds its output.
async fn kill(adapter: &dyn Adapter, run: &RunId) -> Result<RunObservation, ConfinementError> {
    adapter
        .signal(run, Signal::Kill)
        .await
        .map_err(contract_error)?;
    match tokio::time::timeout(KILL_SETTLE, adapter.wait(run)).await {
        Ok(observed) => observed.map_err(contract_error),
        Err(_) => Err(refuse(
            ConfinementRefusal::CapabilityUnserved,
            format!(
                "Loom killed the child's process group through Substrate's local adapter, yet it had not ended {KILL_SETTLE:?} later: a process that left the group still holds its output"
            ),
        )),
    }
}

/// When a launch given `timeout` is killed: `timeout` from now, refused
/// [`ConfinementRefusal::ScopeInvalid`] before anything starts when the clock cannot hold it.
fn deadline(timeout: Duration) -> Result<tokio::time::Instant, ConfinementError> {
    tokio::time::Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| {
            scope_error(format!(
                "a timeout of {timeout:?} is past what this host's clock can hold"
            ))
        })
}

/// The working directory `local` starts a child in, relative to the workspace: `/workspace` is
/// none, `/workspace/<relative>` is `<relative>`. `check_workdir` refused every other working
/// directory a `local` child can name (a read-only root's mount needs a root, which is refused).
fn relative_workdir(workdir: Option<&Path>) -> Option<String> {
    let inside = workdir?.strip_prefix(SANDBOX_WORKSPACE).ok()?;
    (!inside.as_os_str().is_empty()).then(|| inside.to_string_lossy().into_owned())
}

/// The value Substrate gives a baseline variable a child keeps, the same under every adapter.
fn baseline_value(name: BaselineEnvironment) -> &'static str {
    match name {
        BaselineEnvironment::Lang | BaselineEnvironment::LcAll => "C.UTF-8",
        BaselineEnvironment::Path => "/usr/bin:/bin",
        BaselineEnvironment::Term => "dumb",
        BaselineEnvironment::Tz => "UTC",
    }
}

/// A `local` test's whole environment, with host values where the `linux` route sets sandbox
/// paths. A host variable that is not UTF-8 is refused naming it, never dropped; Substrate holds
/// the rest to its closed shape at start (`exec.environment-refused`), never cutting a value.
fn test_environment(workspace: &Path) -> Result<BTreeMap<String, String>, ConfinementError> {
    let mut env = BTreeMap::new();
    for name in ["PATH", "HOME", "RUSTUP_HOME", "CARGO_HOME"] {
        if let Some(value) = std::env::var_os(name) {
            let value = value.into_string().map_err(|_| {
                scope_error(format!(
                    "the host's {name} is not UTF-8, which Substrate's local adapter cannot pass"
                ))
            })?;
            env.insert(name.to_owned(), value);
        }
    }
    let target = workspace.join(super::WRITABLE);
    let target = target
        .to_str()
        .ok_or_else(|| scope_error("the workspace's target directory is not UTF-8"))?;
    env.insert("CARGO_TARGET_DIR".to_owned(), target.to_owned());
    env.insert("CARGO_NET_OFFLINE".to_owned(), "true".to_owned());
    env.insert("CARGO_BUILD_JOBS".to_owned(), "2".to_owned());
    env.insert("RUSTUP_AUTO_INSTALL".to_owned(), "0".to_owned());
    Ok(env)
}

/// A live `local` child ([`EngineRunner::spawn_child`]): one run of the shared adapter, on a
/// runtime the child owns, so `local`'s drain and reaper tasks and Loom's deadline keep running
/// between calls. Dropped before it ended, it is killed with its process group.
pub(super) struct LocalChild {
    route: Arc<LocalRoute>,
    run: RunId,
    deadline: tokio::time::Instant,
    timed_out: Arc<AtomicBool>,
    signalled: bool,
    ended: bool,
    runtime: Option<tokio::runtime::Runtime>,
}

impl fmt::Debug for LocalChild {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalChild")
            .field("run", &self.run.as_str())
            .field("signalled", &self.signalled)
            .field("ended", &self.ended)
            .finish_non_exhaustive()
    }
}

impl LocalChild {
    /// Start `command` under `admitted` on a runtime of its own, and the task that kills it with
    /// its group at `timeout` if it still runs then.
    fn start(
        route: Arc<LocalRoute>,
        admitted: &AdmittedProfile,
        command: &substrate_host::confinement::Command,
        timeout: Duration,
    ) -> Result<Self, ConfinementError> {
        let deadline = deadline(timeout)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|e| refuse(ConfinementRefusal::BackendMissing, e))?;
        let run = runtime
            .block_on(route.adapter.start(admitted, command))
            .map_err(contract_error)?;
        let timed_out = Arc::new(AtomicBool::new(false));
        let watched = (Arc::clone(&route), run.clone(), Arc::clone(&timed_out));
        runtime.spawn(async move {
            let (route, run, timed_out) = watched;
            tokio::time::sleep_until(deadline).await;
            if route
                .adapter
                .observe(&run)
                .await
                .is_ok_and(|observed| observed.state == RunState::Running)
            {
                timed_out.store(true, Ordering::SeqCst);
                let _ = route.adapter.signal(&run, Signal::Kill).await;
            }
        });
        Ok(Self {
            route,
            run,
            deadline,
            timed_out,
            signalled: false,
            ended: false,
            runtime: Some(runtime),
        })
    }

    fn block_on<F: std::future::Future>(&self, call: F) -> F::Output {
        self.runtime
            .as_ref()
            .expect("a child holds its runtime until it is dropped")
            .block_on(call)
    }

    pub(super) fn write_stdin(&mut self, bytes: &[u8]) -> Result<(), ChildError> {
        for chunk in bytes.chunks(STDIN_CHUNK_BYTES) {
            self.block_on(self.route.adapter.write_stdin(&self.run, chunk))
                .map_err(child_error)?;
        }
        Ok(())
    }

    pub(super) fn close_stdin(&mut self) -> Result<(), ChildError> {
        self.block_on(self.route.adapter.close_stdin(&self.run))
            .map_err(child_error)
    }

    pub(super) fn read(&mut self, timeout: Duration) -> Result<Option<ChildFrame>, ChildError> {
        loop {
            let event = self
                .block_on(self.route.adapter.read_output(&self.run, timeout))
                .map_err(child_error)?;
            match event {
                None => return Ok(None),
                Some(OutputEvent::Chunk { stream, bytes, .. }) => {
                    return Ok(Some(ChildFrame {
                        stream: match stream {
                            PipeStream::Stdout => ChildStream::Stdout,
                            PipeStream::Stderr => ChildStream::Stderr,
                        },
                        bytes,
                    }));
                }
                Some(OutputEvent::Stalled) => {
                    return Err(ChildError::Session {
                        code: "session.output-backpressure".to_owned(),
                        detail: "the child's output waited unread past the stall deadline, and Substrate's local adapter ended it".to_owned(),
                    });
                }
                // A stream that reached its bound delivers nothing more; read on.
                Some(_) => {}
            }
        }
    }

    pub(super) fn signal(&mut self, signal: ChildSignal) -> Result<(), ChildError> {
        let adapter = &self.route.adapter;
        let run = &self.run;
        let sent = self.block_on(async {
            if adapter.observe(run).await?.state != RunState::Running {
                return Ok(false);
            }
            let wire = signal.wire();
            adapter.signal(run, wire).await?;
            if wire != Signal::Kill
                && tokio::time::timeout(SIGNAL_GRACE, adapter.wait(run))
                    .await
                    .is_err()
            {
                adapter.signal(run, Signal::Kill).await?;
            }
            // A run a process outside its group keeps open is left to `wait`, which refuses it.
            match tokio::time::timeout(KILL_SETTLE, adapter.wait(run)).await {
                Ok(observed) => observed.map(|_| true),
                Err(_) => Ok(true),
            }
        });
        self.signalled |= sent.map_err(child_error)?;
        Ok(())
    }

    pub(super) fn wait(mut self) -> Result<ChildResult, ConfinementError> {
        let adapter = Arc::clone(&self.route.adapter);
        let run = self.run.clone();
        let deadline = self.deadline;
        let timed_out = Arc::clone(&self.timed_out);
        let (observed, output) = self.block_on(async {
            // Already closed, or a child that ended, is no reason not to wait for it.
            let _ = adapter.close_stdin(&run).await;
            loop {
                let step = deadline
                    .saturating_duration_since(tokio::time::Instant::now())
                    .min(READ_STEP);
                if step.is_zero() {
                    break;
                }
                match adapter.read_output(&run, step).await {
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(error) if error.code() == substrate_wire::SESSION_READ_TIMEOUT => {}
                    Err(error) => return Err(contract_error(error)),
                }
            }
            let observed = match tokio::time::timeout_at(deadline, adapter.wait(&run)).await {
                Ok(observed) => observed.map_err(contract_error)?,
                Err(_) => {
                    timed_out.store(true, Ordering::SeqCst);
                    kill(&*adapter, &run).await?
                }
            };
            let output = adapter.output(&run).await.map_err(contract_error)?;
            Ok::<_, ConfinementError>((observed, output))
        })?;
        self.ended = observed.state != RunState::Running;
        let ended = Ended {
            observed,
            timed_out: self.timed_out.load(Ordering::SeqCst),
            output,
            timeout: Duration::ZERO,
        };
        let exit = ended.exit();
        let ended_by_host = self.signalled && exit.signal.is_some();
        if exit.exit_code.is_none() && !exit.timed_out && !ended_by_host {
            return Err(refuse(
                ConfinementRefusal::CapabilityUnserved,
                format!(
                    "Substrate's local adapter reports no exit status for the live child, no timeout and no signal the host sent ({})",
                    ended.described()
                ),
            ));
        }
        Ok(ChildResult {
            exit,
            output: ended.output(),
            applied: self.route.applied(),
            driver_record: None,
        })
    }
}

impl Drop for LocalChild {
    fn drop(&mut self) {
        // A drop runs wherever its host drops the child, inside an async runtime too, where no
        // runtime may be blocked on: the kill runs as a task on the child's own runtime, and this
        // thread waits for it on a plain channel.
        if let Some(runtime) = self.runtime.take() {
            if !self.ended {
                let (adapter, run) = (Arc::clone(&self.route.adapter), self.run.clone());
                let (done, killed) = std::sync::mpsc::channel();
                runtime.spawn(async move {
                    let _ = kill(&*adapter, &run).await;
                    let _ = done.send(());
                });
                let _ = killed.recv();
            }
            runtime.shutdown_background();
        }
    }
}
