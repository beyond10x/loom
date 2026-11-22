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
    AppliedConfinement, Backend, Cancel, ChildCommand, ChildExit, ChildOutput, ChildStream,
    ConfinementAdapter, ConfinementError, ConfinementRefusal, EngineConfinement, NetworkMode,
    QUEUED_FRAMES, READ_STEP, SANDBOX_WORKSPACE, SubstrateRunner, TestExecution, TestRunner,
    cancelled_before_launch, canonical_workspace, check_program, check_stdin, check_workdir,
    check_writable, enum_name, escape_reason, is_set, killed_by_cancel, local_label, refuse,
    scope_error, tail,
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

    /// This runner with `bounds` for its `local` runs in place of [`KillBounds::HOST`]. A host
    /// keeps [`KillBounds::HOST`]; a test sets short ones so that it does not wait 5 s for a
    /// grace or a settle to run out. Under `linux` the driver holds its own grace
    /// ([`SIGNAL_GRACE`]) and the runner is returned unchanged.
    #[must_use]
    pub fn with_kill_bounds(self, bounds: KillBounds) -> Self {
        match self.route {
            Route::Linux(_) => self,
            Route::Local(local) => Self {
                route: Route::Local(Arc::new(LocalRoute {
                    adapter: Arc::clone(&local.adapter),
                    reason: local.reason.clone(),
                    facts: local.facts.clone(),
                    bounds,
                })),
            },
        }
    }

    /// The bounds this runner's `local` runs are given to end; [`KillBounds::HOST`] under
    /// `linux`.
    pub fn kill_bounds(&self) -> KillBounds {
        match &self.route {
            Route::Linux(_) => KillBounds::HOST,
            Route::Local(local) => local.bounds,
        }
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

    fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        EngineRunner::run_child(self, child)
    }

    fn preflight_child(&self, root: &Path) -> Result<(), ConfinementError> {
        self.preflight(root)
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

/// The `local` route: the adapter the constructor opened, the operator's reason, the facts the
/// adapter proved, in `FACT_ORDER`'s spelling, and how long a run is given to end.
pub(super) struct LocalRoute {
    adapter: Arc<dyn Adapter>,
    reason: String,
    facts: Vec<String>,
    bounds: KillBounds,
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
            bounds: KillBounds::HOST,
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
        if is_set(child.cancel.as_ref()) {
            return Err(cancelled_before_launch());
        }
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
            child.cancel.as_ref(),
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
            None,
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
    /// that returns once it started has ended it. A write still pending then is dropped. A
    /// `cancel` set while it runs sends the same `Kill` within [`CANCEL_POLL`], and the launch is
    /// refused [`ConfinementRefusal::Cancelled`].
    fn launch(
        &self,
        admitted: &AdmittedProfile,
        command: &substrate_host::confinement::Command,
        stdin: Option<&[u8]>,
        timeout: Duration,
        cancel: Option<&Cancel>,
    ) -> Result<Ended, ConfinementError> {
        let deadline = deadline(timeout)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| refuse(ConfinementRefusal::BackendMissing, e))?;
        let started = std::time::Instant::now();
        let by_cancel = AtomicBool::new(false);
        let ended = runtime.block_on(async {
            let run = self
                .adapter
                .start(admitted, command)
                .await
                .map_err(contract_error)?;
            let killed = AtomicBool::new(false);
            let ended = match stdin {
                None => {
                    self.finish(&run, deadline, &killed, cancel, &by_cancel)
                        .await
                }
                Some(bytes) => {
                    let write = self.write_all(&run, bytes);
                    let finish = self.finish(&run, deadline, &killed, cancel, &by_cancel);
                    tokio::pin!(write, finish);
                    // A write that ended first, failed or not, is answered once the run ended.
                    // A write still pending when the run ended (or the deadline's `Kill` was
                    // waited for) is dropped, which closes the run's standard input: a process
                    // outside the group that holds the pipe and reads nothing never holds the
                    // run past its deadline.
                    tokio::select! {
                        biased;
                        written = &mut write => written.and(finish.await),
                        ended = &mut finish => ended,
                    }
                }
            };
            let (observed, timed_out) = match ended {
                Ok(ended) => ended,
                Err(error) => {
                    // One `Kill` per run: the deadline's, already sent and waited for, is not
                    // sent again.
                    if !killed.load(Ordering::SeqCst) {
                        let _ = self.kill(&run).await;
                    }
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
        });
        if by_cancel.load(Ordering::SeqCst) {
            let mut refused = killed_by_cancel(started.elapsed());
            if let Err(error) = ended {
                // The kill was sent; what kept the run from ending after it is told, not hidden.
                refused.detail = format!("{}; {error}", refused.detail);
            }
            return Err(refused);
        }
        ended
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

    /// The run's end, and whether Loom ended it at `deadline`. `killed` is set once the deadline's
    /// or the cancel's `Kill` is sent, whether or not the run then ended; `by_cancel` once
    /// `cancel`, polled every [`CANCEL_POLL`], sent it. A run that ended first keeps its end.
    async fn finish(
        &self,
        run: &RunId,
        deadline: tokio::time::Instant,
        killed: &AtomicBool,
        cancel: Option<&Cancel>,
        by_cancel: &AtomicBool,
    ) -> Result<(RunObservation, bool), ConfinementError> {
        let cancelled = async {
            match cancel {
                Some(cancel) => {
                    while !cancel.is_cancelled() {
                        tokio::time::sleep(CANCEL_POLL).await;
                    }
                }
                None => std::future::pending::<()>().await,
            }
        };
        tokio::select! {
            biased;
            observed = self.adapter.wait(run) => Ok((observed.map_err(contract_error)?, false)),
            () = tokio::time::sleep_until(deadline) => {
                killed.store(true, Ordering::SeqCst);
                Ok((self.kill(run).await?, true))
            }
            () = cancelled => {
                killed.store(true, Ordering::SeqCst);
                by_cancel.store(true, Ordering::SeqCst);
                Ok((self.kill(run).await?, false))
            }
        }
    }

    /// `Signal::Kill` to the run's process group, then its end.
    async fn kill(&self, run: &RunId) -> Result<RunObservation, ConfinementError> {
        kill(&*self.adapter, run, self.bounds.kill_settle).await
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
pub const KILL_SETTLE: Duration = Duration::from_secs(5);

/// How often a `local` run reads its cancel handle ([`ChildCommand::with_cancel`]).
const CANCEL_POLL: Duration = Duration::from_millis(100);

/// How long a `local` run is given to end: after the host's interrupt or terminate before it is
/// killed (`signal_grace`), and after the kill before it is refused (`kill_settle`). A host's
/// runner keeps [`KillBounds::HOST`] ([`SIGNAL_GRACE`], [`KILL_SETTLE`]); a test sets shorter
/// ones ([`EngineRunner::with_kill_bounds`]) so that it does not wait 5 s for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KillBounds {
    /// From the host's interrupt or terminate to its kill ([`SIGNAL_GRACE`]).
    pub signal_grace: Duration,
    /// From a kill to the refusal of a run still not ended ([`KILL_SETTLE`]).
    pub kill_settle: Duration,
}

impl KillBounds {
    /// A host's bounds: [`SIGNAL_GRACE`] and [`KILL_SETTLE`].
    pub const HOST: KillBounds = KillBounds {
        signal_grace: SIGNAL_GRACE,
        kill_settle: KILL_SETTLE,
    };
}

impl Default for KillBounds {
    fn default() -> Self {
        Self::HOST
    }
}

/// `Signal::Kill` to `run`'s process group on `adapter`, then its end, waited for at most
/// `settle` ([`KILL_SETTLE`] on a host): a run still not ended then is refused
/// [`ConfinementRefusal::CapabilityUnserved`], since a process outside its group holds its output.
async fn kill(
    adapter: &dyn Adapter,
    run: &RunId,
    settle: Duration,
) -> Result<RunObservation, ConfinementError> {
    adapter
        .signal(run, Signal::Kill)
        .await
        .map_err(contract_error)?;
    match tokio::time::timeout(settle, adapter.wait(run)).await {
        Ok(observed) => observed.map_err(contract_error),
        Err(_) => Err(refuse(
            ConfinementRefusal::CapabilityUnserved,
            format!(
                "Loom killed the child's process group through Substrate's local adapter, yet it had not ended {settle:?} later: a process that left the group still holds its output"
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
            let bounds = self.route.bounds;
            adapter.signal(run, wire).await?;
            if wire != Signal::Kill
                && tokio::time::timeout(bounds.signal_grace, adapter.wait(run))
                    .await
                    .is_err()
            {
                adapter.signal(run, Signal::Kill).await?;
            }
            // A run a process outside its group keeps open is left to `wait`, which refuses it.
            match tokio::time::timeout(bounds.kill_settle, adapter.wait(run)).await {
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
        let settle = self.route.bounds.kill_settle;
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
                    kill(&*adapter, &run, settle).await?
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
                let settle = self.route.bounds.kill_settle;
                let (done, killed) = std::sync::mpsc::channel();
                runtime.spawn(async move {
                    let _ = kill(&*adapter, &run, settle).await;
                    let _ = done.send(());
                });
                let _ = killed.recv();
            }
            runtime.shutdown_background();
        }
    }
}

#[cfg(test)]
mod tests {
    //! `LocalRoute::launch` over a counting adapter: Substrate's own `local` adapter starts the
    //! run, the fake answers `wait` and counts each `Signal::Kill`.

    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::AtomicUsize;
    use std::time::Instant;

    use substrate_host::confinement::{
        AdapterFacts, Command, ConfinementError as Contract, ConfinementProfile, RenderedCommand,
    };

    use super::*;

    type Answer<'a, T> = Pin<Box<dyn Future<Output = Result<T, Contract>> + Send + 'a>>;

    /// What the fake's `wait` answers.
    #[derive(Clone, Copy)]
    enum Wait {
        /// Never: a run a process outside its group holds open, before and after `Kill`.
        Never,
        /// At once, with a contract error.
        Refused,
        /// As `local` answers it.
        Real,
        /// As `local` answers it, the run's state and exit status lost.
        Lost,
        /// Not before a `Kill` was sent, then as `local` answers it.
        UntilKill,
    }

    /// What else the fake does that `local` would not.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Fault {
        None,
        /// Every write to standard input stays pending: a process outside the group holds the
        /// pipe and reads nothing.
        StdinStalls,
        /// Sending a `Kill` fails, after it was counted.
        KillRefused,
    }

    struct Counting {
        inner: Arc<dyn Adapter>,
        wait: Wait,
        fault: Fault,
        kills: AtomicUsize,
    }

    impl Counting {
        fn refusal(&self, detail: &str) -> Contract {
            Contract::AdapterUnserved {
                adapter: self.inner.facts().adapter.clone(),
                detail: detail.to_owned(),
            }
        }
    }

    impl Adapter for Counting {
        fn facts(&self) -> &AdapterFacts {
            self.inner.facts()
        }

        fn check(&self, profile: &ConfinementProfile) -> Result<(), Contract> {
            self.inner.check(profile)
        }

        fn render(
            &self,
            admitted: &AdmittedProfile,
            command: &Command,
        ) -> Result<RenderedCommand, Contract> {
            self.inner.render(admitted, command)
        }

        fn start<'life0, 'life1, 'life2, 'async_trait>(
            &'life0 self,
            admitted: &'life1 AdmittedProfile,
            command: &'life2 Command,
        ) -> Answer<'async_trait, RunId>
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            'life2: 'async_trait,
            Self: 'async_trait,
        {
            self.inner.start(admitted, command)
        }

        fn observe<'life0, 'life1, 'async_trait>(
            &'life0 self,
            run: &'life1 RunId,
        ) -> Answer<'async_trait, RunObservation>
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            self.inner.observe(run)
        }

        fn output<'life0, 'life1, 'async_trait>(
            &'life0 self,
            run: &'life1 RunId,
        ) -> Answer<'async_trait, RunOutput>
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            self.inner.output(run)
        }

        fn wait<'life0, 'life1, 'async_trait>(
            &'life0 self,
            run: &'life1 RunId,
        ) -> Answer<'async_trait, RunObservation>
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            match self.wait {
                Wait::Never => Box::pin(std::future::pending()),
                Wait::Refused => {
                    let refusal = self.refusal("the counting fake refuses every wait");
                    Box::pin(async move { Err(refusal) })
                }
                Wait::UntilKill => Box::pin(async move {
                    while self.kills.load(Ordering::SeqCst) == 0 {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                    self.inner.wait(run).await
                }),
                Wait::Real => self.inner.wait(run),
                Wait::Lost => {
                    let waited = self.inner.wait(run);
                    Box::pin(async move {
                        waited.await.map(|observed| RunObservation {
                            state: RunState::Unknown,
                            exit: None,
                            ..observed
                        })
                    })
                }
            }
        }

        fn signal<'life0, 'life1, 'async_trait>(
            &'life0 self,
            run: &'life1 RunId,
            signal: Signal,
        ) -> Answer<'async_trait, ()>
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            if signal == Signal::Kill {
                self.kills.fetch_add(1, Ordering::SeqCst);
                if self.fault == Fault::KillRefused {
                    let refusal = self.refusal("the counting fake could not send the Kill");
                    return Box::pin(async move { Err(refusal) });
                }
            }
            self.inner.signal(run, signal)
        }

        fn write_stdin<'life0, 'life1, 'life2, 'async_trait>(
            &'life0 self,
            run: &'life1 RunId,
            bytes: &'life2 [u8],
        ) -> Answer<'async_trait, ()>
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            'life2: 'async_trait,
            Self: 'async_trait,
        {
            match self.fault {
                Fault::StdinStalls => Box::pin(std::future::pending()),
                _ => self.inner.write_stdin(run, bytes),
            }
        }

        fn close_stdin<'life0, 'life1, 'async_trait>(
            &'life0 self,
            run: &'life1 RunId,
        ) -> Answer<'async_trait, ()>
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            self.inner.close_stdin(run)
        }
    }

    const REASON: &str = "the engine's own unit test of one kill";

    /// A route over a counting fake of Substrate's `local` adapter, the fake, and a workspace.
    fn route(wait: Wait) -> (LocalRoute, Arc<Counting>, tempfile::TempDir) {
        route_with(wait, Fault::None)
    }

    /// [`route`], the fake also doing `fault`.
    fn route_with(wait: Wait, fault: Fault) -> (LocalRoute, Arc<Counting>, tempfile::TempDir) {
        let runner = EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(REASON.to_owned()),
            cgroup_root: None,
        })
        .expect("Substrate's local adapter opens on this host");
        let Route::Local(local) = runner.route else {
            panic!("EngineRunner::open of local takes the local route");
        };
        let fake = Arc::new(Counting {
            inner: Arc::clone(&local.adapter),
            wait,
            fault,
            kills: AtomicUsize::new(0),
        });
        let adapter: Arc<dyn Adapter> = fake.clone();
        let route = LocalRoute::new(adapter, REASON.to_owned()).expect("the fake proves local");
        let workspace = tempfile::tempdir().expect("a workspace");
        (route, fake, workspace)
    }

    /// `launch` of `/usr/bin/true` on `route`, with `stdin` written to it when given (its profile
    /// admits none), its error and how long it took.
    fn launch(
        route: &LocalRoute,
        workspace: &Path,
        stdin: Option<&[u8]>,
        timeout: Duration,
    ) -> (ConfinementError, Duration) {
        let (ended, took) = launched(route, workspace, stdin, timeout);
        match ended {
            Ok(ended) => panic!("the launch ended: {}", ended.described()),
            Err(error) => (error, took),
        }
    }

    /// `launch` as [`launch`] runs it, its result whatever it is.
    fn launched(
        route: &LocalRoute,
        workspace: &Path,
        stdin: Option<&[u8]>,
        timeout: Duration,
    ) -> (Result<Ended, ConfinementError>, Duration) {
        let workspace = canonical_workspace(workspace).expect("a canonical workspace");
        let admitted = route
            .admit(workspace, None, StdinMode::Null, OutputDelivery::Recorded)
            .expect("local admits the profile");
        let command = Command {
            argv: vec!["/usr/bin/true".to_owned()],
            env: BTreeMap::new(),
        };
        let started = Instant::now();
        let ended = route.launch(&admitted, &command, stdin, timeout, None);
        (ended, started.elapsed())
    }

    #[test]
    fn a_run_whose_status_was_lost_is_refused_never_timed_out_and_sent_no_kill() {
        let (route, fake, workspace) = route(Wait::Lost);
        let (ended, took) = launched(&route, workspace.path(), None, Duration::from_secs(60));
        let ended = ended.unwrap_or_else(|error| panic!("the run ended: {error}"));
        assert!(!ended.timed_out, "a lost status is no timeout");
        let error = route
            .execution(ended)
            .expect_err("a run with no exit status and no timeout is refused");
        assert_eq!(
            error.reason,
            ConfinementRefusal::CapabilityUnserved,
            "{error}"
        );
        assert!(
            error
                .detail
                .contains("reports no exit status for the child"),
            "{error}"
        );
        assert!(!error.detail.contains("timed out"), "{error}");
        assert_eq!(
            fake.kills.load(Ordering::SeqCst),
            0,
            "an ended run is never killed"
        );
        assert!(
            took < KILL_SETTLE,
            "nothing waited for the deadline: {took:?}"
        );
    }

    /// With a settle of 1 s in place of a host's [`KILL_SETTLE`] (5 s), which the refusal names.
    #[test]
    fn a_run_past_its_deadline_is_sent_one_kill_and_waited_for_one_settle() {
        let (mut route, fake, workspace) = route(Wait::Never);
        assert_eq!(
            route.bounds,
            KillBounds::HOST,
            "a route keeps a host's bounds"
        );
        let settle = Duration::from_secs(1);
        route.bounds.kill_settle = settle;
        let (error, took) = launch(&route, workspace.path(), None, Duration::from_millis(10));
        assert_eq!(
            error.reason,
            ConfinementRefusal::CapabilityUnserved,
            "{error}"
        );
        assert!(
            error.detail.contains("had not ended 1s later"),
            "the refusal names the settle that ran out: {error}"
        );
        assert_eq!(
            fake.kills.load(Ordering::SeqCst),
            1,
            "the deadline sends one Kill, and the launch's error path sends none after it"
        );
        assert!(
            took >= settle && took < 2 * settle,
            "the launch waits one settle ({settle:?}) after its Kill, not two: {took:?}"
        );
    }

    /// A host's runner keeps the crate's bounds, 5 s of grace and 5 s of settle; a runner given
    /// others keeps its adapter, reason and facts.
    #[test]
    fn a_hosts_runner_keeps_the_crates_kill_bounds() {
        assert_eq!(
            KillBounds::HOST,
            KillBounds {
                signal_grace: Duration::from_secs(5),
                kill_settle: Duration::from_secs(5),
            }
        );
        assert_eq!(KillBounds::default(), KillBounds::HOST);
        let runner = EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(REASON.to_owned()),
            cgroup_root: None,
        })
        .expect("Substrate's local adapter opens on this host");
        assert_eq!(runner.kill_bounds(), KillBounds::HOST);
        let short = KillBounds {
            signal_grace: Duration::from_millis(500),
            kill_settle: Duration::from_secs(1),
        };
        let shortened = runner.clone().with_kill_bounds(short);
        assert_eq!(shortened.kill_bounds(), short);
        assert_eq!(
            runner.kill_bounds(),
            KillBounds::HOST,
            "the original keeps its own"
        );
        let (Route::Local(before), Route::Local(after)) = (&runner.route, &shortened.route) else {
            panic!("both take the local route");
        };
        assert!(Arc::ptr_eq(&before.adapter, &after.adapter));
        assert_eq!(
            (&before.reason, &before.facts),
            (&after.reason, &after.facts)
        );
        let linux = EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Linux,
            local_reason: None,
            cgroup_root: None,
        })
        .expect("linux opens nothing");
        assert_eq!(
            linux.with_kill_bounds(short).kill_bounds(),
            KillBounds::HOST
        );
    }

    #[test]
    fn a_wait_that_fails_before_the_deadline_is_answered_with_one_kill() {
        let (route, fake, workspace) = route(Wait::Refused);
        let (error, took) = launch(&route, workspace.path(), None, Duration::from_secs(60));
        assert_eq!(
            error.reason,
            ConfinementRefusal::CapabilityUnserved,
            "{error}"
        );
        assert!(
            error
                .detail
                .contains("the counting fake refuses every wait"),
            "the launch returns the wait's own error: {error}"
        );
        assert_eq!(
            fake.kills.load(Ordering::SeqCst),
            1,
            "a failed wait is no timeout: the launch's error path sends its one Kill"
        );
        assert!(
            took < KILL_SETTLE,
            "nothing waited for the deadline: {took:?}"
        );
    }

    #[test]
    fn a_write_to_standard_input_that_fails_is_answered_with_one_kill() {
        let (route, fake, workspace) = route(Wait::Real);
        let (error, took) = launch(
            &route,
            workspace.path(),
            Some(b"arguments"),
            Duration::from_secs(60),
        );
        assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
        assert!(
            error.detail.contains(substrate_wire::SESSION_NOT_PIPE),
            "the launch returns the write's own error: {error}"
        );
        assert_eq!(
            fake.kills.load(Ordering::SeqCst),
            1,
            "a failed write is no timeout: the launch's error path sends its one Kill"
        );
        assert!(
            took < KILL_SETTLE,
            "nothing waited for the deadline: {took:?}"
        );
    }

    /// A write to standard input that a process outside the group stalls does not hold the run
    /// past its deadline: once the deadline's `Kill` ended the run, the pending write is dropped.
    #[test]
    fn a_stdin_write_still_pending_when_the_deadline_ended_the_run_is_dropped() {
        let (route, fake, workspace) = route_with(Wait::UntilKill, Fault::StdinStalls);
        let (sender, receiver) = std::sync::mpsc::channel();
        // On a thread of its own, so a launch the write holds fails this case instead of hanging
        // it; the thread is left behind then.
        std::thread::spawn(move || {
            let (ended, took) = launched(
                &route,
                workspace.path(),
                Some(b"arguments"),
                Duration::from_millis(10),
            );
            let _ = sender.send((ended.map(|ended| ended.timed_out), took));
        });
        let (timed_out, took) = receiver.recv_timeout(KILL_SETTLE).unwrap_or_else(|_| {
            panic!(
                "the launch had not returned {KILL_SETTLE:?} after its 10 ms deadline: the \
                 pending write to standard input held it"
            )
        });
        assert_eq!(timed_out, Ok(true), "the deadline ended the run");
        assert_eq!(
            fake.kills.load(Ordering::SeqCst),
            1,
            "the deadline sends one Kill"
        );
        assert!(took < KILL_SETTLE, "the launch took {took:?}");
    }

    /// A `Kill` that could not be sent ends the launch as a refusal of its runner, never as a run
    /// Loom killed at its deadline (the step reads "was killed" only for that), and is not sent
    /// a second time.
    #[test]
    fn a_kill_that_could_not_be_sent_is_a_refusal_never_a_kill_and_is_not_sent_again() {
        let (route, fake, workspace) = route_with(Wait::Never, Fault::KillRefused);
        let (error, took) = launch(&route, workspace.path(), None, Duration::from_millis(10));
        assert_eq!(
            error.reason,
            ConfinementRefusal::CapabilityUnserved,
            "{error}"
        );
        assert!(
            error
                .detail
                .contains("the counting fake could not send the Kill"),
            "the launch returns the send's own error: {error}"
        );
        assert!(!error.to_string().contains("killed"), "{error}");
        assert_eq!(
            fake.kills.load(Ordering::SeqCst),
            1,
            "the failed Kill is not sent again by the launch's error path"
        );
        assert!(
            took < KILL_SETTLE,
            "no settle is waited for a Kill never sent: {took:?}"
        );
    }
}
