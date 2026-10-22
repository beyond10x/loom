//! A live confined child: one Substrate raw-pipe session the host writes, reads, signals and
//! waits for while it runs ([`SubstrateRunner::spawn_child`](super::SubstrateRunner::spawn_child)).

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use substrate_host::{
    Driver as _, DriverError, ExecObservation, HostDriver, PipeFrame, PipeStream,
};
use substrate_wire::{ExecSignalInput, ExecStartInput, PipeSessionStartInput, Signal};

use super::{
    AppliedConfinement, ChildExit, ChildOutput, ChildStream, ConfinementError, ConfinementRefusal,
    DRAIN_MARGIN, Ending, FRAME_BYTES, OpenDriver, Prepared, READ_STEP, Running, dispatched,
    is_terminal, live_result, observation_error, refuse, settle,
};

/// How long a child the host interrupts or terminates has to end before Substrate kills it.
const SIGNAL_GRACE: Duration = Duration::from_secs(5);

/// One frame of a live child's output: the bytes it wrote on one stream, as they arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildFrame {
    pub stream: ChildStream,
    pub bytes: Vec<u8>,
}

impl From<PipeFrame> for ChildFrame {
    fn from(frame: PipeFrame) -> Self {
        Self {
            stream: match frame.stream {
                PipeStream::Stdout => ChildStream::Stdout,
                PipeStream::Stderr => ChildStream::Stderr,
            },
            bytes: frame.bytes,
        }
    }
}

/// A signal the host sends a live child ([`ConfinedChild::signal`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildSignal {
    /// `SIGINT`; the child is killed if it has not ended 5 s later.
    Interrupt,
    /// `SIGTERM`; the child is killed if it has not ended 5 s later.
    Terminate,
    /// `SIGKILL`, at once.
    Kill,
}

impl ChildSignal {
    pub(super) fn wire(self) -> Signal {
        match self {
            Self::Interrupt => Signal::Int,
            Self::Terminate => Signal::Term,
            Self::Kill => Signal::Kill,
        }
    }
}

/// A live child's result ([`ConfinedChild::wait`]): how it ended, its two streams whole and apart
/// as Substrate kept them (each up to its output limit, flagged cut past it), the confinement
/// Substrate applied and the driver's own record of it, which was checked against the request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildResult {
    pub exit: ChildExit,
    pub output: ChildOutput,
    pub applied: AppliedConfinement,
    pub driver_record: substrate_wire::AppliedConfinement,
}

/// A live child's session failed one call: a read that waited its whole timeout with no frame,
/// or another refusal or failure Substrate named. Neither ends the child; its result, or its
/// refusal, still comes from [`ConfinedChild::wait`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildError {
    /// No frame arrived within the read's timeout.
    ReadTimedOut,
    /// Substrate refused or failed the call: its code (`session.input-limit`,
    /// `session.input-closed`, `session.write-failed`, …) and its message.
    Session { code: String, detail: String },
}

impl fmt::Display for ChildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadTimedOut => write!(f, "no output from the child within the read's timeout"),
            Self::Session { code, detail } => write!(f, "{code}: {detail}"),
        }
    }
}

impl std::error::Error for ChildError {}

impl From<DriverError> for ChildError {
    fn from(error: DriverError) -> Self {
        if error.code == substrate_wire::SESSION_READ_TIMEOUT {
            Self::ReadTimedOut
        } else {
            Self::Session {
                code: error.code.to_owned(),
                detail: error.message,
            }
        }
    }
}

/// A child running confined, live ([`SubstrateRunner::spawn_child`](super::SubstrateRunner::spawn_child)).
///
/// Its methods block on the runtime the child owns, which keeps Substrate's supervision of it
/// (its output, its timeout) running between calls; call them outside an async runtime, as
/// `run_child` is. Its output must be read: a child whose 16 queued frames are not read for a
/// second is ended by Substrate, and its result is refused naming
/// `session.output-backpressure`. Dropped before it ended, it is killed; either way its
/// execution is released from the driver and it stops counting as a launch under its cgroup
/// root.
pub struct ConfinedChild {
    open: Arc<OpenDriver>,
    id: String,
    input: ExecStartInput,
    digest: String,
    runtime: Option<tokio::runtime::Runtime>,
    signalled: bool,
    ended: bool,
    deadline: Instant,
    // Last: the count is released only once the child was killed and released.
    _running: Running,
}

impl fmt::Debug for ConfinedChild {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfinedChild")
            .field("id", &self.id)
            .field("signalled", &self.signalled)
            .field("ended", &self.ended)
            .finish_non_exhaustive()
    }
}

impl ConfinedChild {
    /// Start `session` in `workspace` on `open`. The child is held before it starts, so a start
    /// whose outcome Substrate does not know is still killed and released when it is refused.
    pub(super) fn start(
        open: Arc<OpenDriver>,
        workspace: &str,
        session: PipeSessionStartInput,
        timeout: Duration,
    ) -> Result<Self, ConfinementError> {
        let Prepared {
            _running,
            digest,
            root_identity,
            id,
        } = Prepared::new(&open, workspace, &session.exec)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|e| refuse(ConfinementRefusal::BackendMissing, e))?;
        let child = Self {
            open,
            id,
            input: session.exec.clone(),
            digest,
            runtime: Some(runtime),
            signalled: false,
            ended: false,
            deadline: Instant::now() + timeout + DRAIN_MARGIN,
            _running,
        };
        let started =
            child.block_on(|driver, id| driver.start_pipe_session(id, &root_identity, &session));
        dispatched(started)?;
        Ok(child)
    }

    /// Run `call` on the child's driver and id, on its runtime, to its end.
    fn block_on<'a, F: std::future::Future>(
        &'a self,
        call: impl FnOnce(&'a HostDriver, &'a str) -> F,
    ) -> F::Output {
        let runtime = self
            .runtime
            .as_ref()
            .expect("a child holds its runtime until it is dropped");
        runtime.block_on(call(&self.open.driver, &self.id))
    }

    /// Write `bytes` to the child's standard input, in frames of at most 64 KiB; nothing for no
    /// bytes. Past [`ChildCommand::MAX_INPUT_BYTES`](super::ChildCommand::MAX_INPUT_BYTES) in all
    /// Substrate refuses it (`session.input-limit`), once closed `session.input-closed`, and a
    /// child that no longer reads it `session.write-failed`.
    pub fn write_stdin(&mut self, bytes: &[u8]) -> Result<(), ChildError> {
        for frame in bytes.chunks(FRAME_BYTES) {
            self.block_on(|driver, id| driver.write_pipe_session(id, frame))?;
        }
        Ok(())
    }

    /// Close the child's standard input: it reads end of file once it read what was written.
    pub fn close_stdin(&mut self) -> Result<(), ChildError> {
        self.block_on(|driver, id| driver.close_pipe_session_input(id))?;
        Ok(())
    }

    /// The next frame of the child's output, waiting at most `timeout` for one
    /// ([`ChildError::ReadTimedOut`] when none came), or `None` once both its streams closed and
    /// it ended. A zero timeout is refused.
    pub fn read(&mut self, timeout: Duration) -> Result<Option<ChildFrame>, ChildError> {
        let frame = self.block_on(|driver, id| driver.read_pipe_session(id, timeout))?;
        Ok(frame.map(ChildFrame::from))
    }

    /// Send `signal` to every process of the child, and return once it ended: an interrupt or a
    /// termination it has not ended from 5 s later is followed by a kill. A child that already
    /// ended is left as it is. Its output stops being delivered: frames already queued can still
    /// be read, then `None`.
    pub fn signal(&mut self, signal: ChildSignal) -> Result<(), ChildError> {
        let sent = self.block_on(|driver, id| async move {
            if is_terminal(driver.observe_exec(id).await?.resource.state) {
                return Ok(false);
            }
            let input = ExecSignalInput {
                signal: signal.wire(),
                grace_ms: SIGNAL_GRACE.as_millis() as u64,
            };
            driver.signal(id, &input).await.map(|_| true)
        })?;
        self.signalled |= sent;
        Ok(())
    }

    /// Close the child's standard input, read what is left of its output, and return its result
    /// once it ended, checked as [`SubstrateRunner::run_child`](super::SubstrateRunner::run_child)
    /// checks a child run to its end: refused when the confinement Substrate applied is not the
    /// one requested, or when it reports no exit status unless the child's timeout ended it or a
    /// signal the host sent did. A child still running past its timeout and a margin is killed
    /// here, and so refused.
    pub fn wait(mut self) -> Result<ChildResult, ConfinementError> {
        let deadline = tokio::time::Instant::from_std(self.deadline);
        let observed = self.block_on(|driver, id| async move {
            // Already closed, or a child that ended, is no reason not to wait for it.
            let _ = driver.close_pipe_session_input(id).await;
            loop {
                let step = deadline
                    .saturating_duration_since(tokio::time::Instant::now())
                    .min(READ_STEP);
                if step.is_zero() {
                    break;
                }
                match driver.read_pipe_session(id, step).await {
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(e) if e.code == substrate_wire::SESSION_READ_TIMEOUT => {}
                    Err(e) => {
                        return Err(refuse(
                            ConfinementRefusal::CapabilityUnserved,
                            format!("Substrate pipe session output: {e:?}"),
                        ));
                    }
                }
            }
            terminal(driver, id).await
        })?;
        self.ended = is_terminal(observed.resource.state);
        let launched = settle(
            &self.input,
            self.open.egress.aperture.as_ref(),
            self.digest.clone(),
            Ending::from(observed),
        )?;
        live_result(launched, self.signalled)
    }
}

/// The child's observation once it ended: as observed when it already did, otherwise after a
/// kill, which leaves it with no exit status.
async fn terminal(driver: &HostDriver, id: &str) -> Result<ExecObservation, ConfinementError> {
    match driver.observe_exec(id).await {
        Ok(observed) if is_terminal(observed.resource.state) => Ok(observed),
        _ => driver
            .signal(
                id,
                &ExecSignalInput {
                    signal: Signal::Kill,
                    grace_ms: 0,
                },
            )
            .await
            .map_err(observation_error),
    }
}

impl Drop for ConfinedChild {
    fn drop(&mut self) {
        if !self.ended {
            let _ = self.block_on(terminal);
        }
        self.open.driver.discard_superseded_exec(&self.id);
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}
