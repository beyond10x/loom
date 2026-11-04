//! The A2A side of `b10x-loom agent serve` (`story:agent-a2a`): when the spec declares
//! `expose.a2a`, the listener is prepared before anything runs ([`prepare`]: its bearer from
//! [`BEARER_ENV`], its port bound) and answers beside the polling host until the host stops
//! ([`serve`]). Its turns run on an agent of their own, built from the same spec and models on the
//! listener's thread, so the host's agent is never shared between threads. A spec with
//! `expose.a2a` and no `channel` polls nothing ([`answers_only`]): the listener is the host's main
//! loop, and SIGTERM ends it as it ends the poll.
//!
//! The listener's turns are recorded in the state directory's `record.jsonl` under the lock the
//! host holds: [`serve`] answers once the host of this process holds the directory (the polling
//! host, or `agent serve` itself for a spec that polls nothing), and appends to the record it
//! holds rather than opening the directory a second time.

use std::net::{IpAddr, SocketAddr, TcpListener};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use b10x_loom_a2a::A2aServer;
use b10x_loom_agent::serve::a2a::{
    AdmitServeOutcome, Answering, admit_serve, bearer, listener, port, serve_input,
};
use b10x_loom_agent::{HttpTrust, SpecPlugin};
use b10x_loom_plugin::state::HeldRecord;
use llm_core::{BoxFuture, Cancel, Capabilities, Model, Provenance, StreamSink, TurnRequest};
use loom_sdk::agent::AgentSpec;

use crate::plugin::turn_models;

pub use b10x_loom_agent::serve::a2a::BEARER_ENV;

/// The address the listener binds by default: every interface, as a pod's port is reached.
pub const DEFAULT_ADDRESS: &str = "0.0.0.0";

/// Why `--once` is refused for a spec that only answers A2A.
pub const ONCE_WITHOUT_A_CHANNEL: &str = "--once runs one poll cycle, and the spec declares no `channel` to poll; serve it without --once";

/// Whether `spec` only answers A2A: `expose.a2a` and no `channel` (`loom.agent.AdmitServe`
/// `answers`), so the host polls nothing.
pub fn answers_only(spec: &AgentSpec) -> bool {
    admit_serve(&serve_input(spec)) == AdmitServeOutcome::Answers
}

/// A model shared by the host's agent and the listener's: every call goes to the one model.
pub struct SharedModel(pub Arc<dyn Model>);

impl Model for SharedModel {
    fn provenance(&self) -> &Provenance {
        self.0.provenance()
    }

    fn capabilities(&self) -> &Capabilities {
        self.0.capabilities()
    }

    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        sink: &'a mut dyn StreamSink,
        cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<llm_core::TurnOutcome, llm_core::Error>> {
        self.0.turn(request, sink, cancel)
    }
}

/// The listener `spec` declares, with its socket bound on `address`, or `None` when the spec
/// declares no `expose.a2a`. `bearer_value` is the content of [`BEARER_ENV`].
///
/// # Errors
/// The bearer is unset or empty, the exposure cannot be served, or the port cannot be bound; the
/// message never holds the bearer.
pub fn prepare(
    spec: &AgentSpec,
    address: IpAddr,
    bearer_value: Option<&str>,
) -> Result<Option<(A2aServer, TcpListener)>, String> {
    let Some(exposure) = spec.expose.as_ref().and_then(|expose| expose.a2a.as_ref()) else {
        return Ok(None);
    };
    let bearer = bearer(bearer_value).map_err(|error| error.to_string())?;
    let server = listener(exposure, bearer).map_err(|error| error.to_string())?;
    let at = SocketAddr::new(address, port(exposure).map_err(|error| error.to_string())?);
    let socket = TcpListener::bind(at)
        .map_err(|error| format!("the A2A listener cannot bind {at}: {error}"))?;
    Ok(Some((server, socket)))
}

/// What the listener's agent is built from.
pub struct Agent<'a> {
    /// The spec the host serves.
    pub spec: AgentSpec,
    /// The host's state directory.
    pub state: &'a Path,
    /// The classifier model, shared with the host's agent.
    pub classifier: Arc<dyn Model>,
    /// The turn model, shared with the host's agent.
    pub model: Arc<dyn Model>,
    /// The roots the listener's turns read `https` bindings with, as the host's agent does
    /// (`LOOM_HTTP_CA_FILE`); the listener itself is never given them.
    pub trust: HttpTrust,
}

/// How often [`serve`] looks whether the host holds the state directory yet.
const HOLD_POLL: Duration = Duration::from_millis(20);

/// The record of the state directory `state` once a host of this process holds it, or `None`
/// when `stop` is set first.
fn held_record(state: &Path, stop: &AtomicBool) -> Option<HeldRecord> {
    loop {
        if let Some(record) = HeldRecord::of(state) {
            return Some(record);
        }
        if stop.load(Ordering::SeqCst) {
            return None;
        }
        std::thread::sleep(HOLD_POLL);
    }
}

/// Answers on `socket` until `stop` is set, building the listener's agent from `agent` on this
/// thread, once the host holds the state directory; its turns go to the record the host holds.
/// When the listener fails, `host_stop` is set, so the host ends after its current item. A `stop`
/// set before the host holds the directory ends this without answering.
///
/// # Errors
/// The agent cannot be built or the listener failed.
pub fn serve(
    server: &A2aServer,
    socket: TcpListener,
    agent: Agent<'_>,
    stop: &AtomicBool,
    host_stop: &AtomicBool,
) -> Result<(), String> {
    let served = (|| {
        let plugin = SpecPlugin::new(agent.spec, agent.state)
            .map_err(|error| error.to_string())?
            .with_classifier(Box::new(SharedModel(agent.classifier)))
            .with_turn_models(turn_models(agent.model))
            .with_http_trust(agent.trust);
        let Some(record) = held_record(agent.state, stop) else {
            return Ok(());
        };
        let at = socket
            .local_addr()
            .map_err(|error| format!("the A2A listener has no address: {error}"))?;
        eprintln!("b10x-loom: A2A listening on {at}");
        server
            .serve(socket, &Answering::new(&plugin).with_record(record), stop)
            .map_err(|error| format!("the A2A listener failed: {error}"))
    })();
    if served.is_err() {
        host_stop.store(true, Ordering::SeqCst);
    }
    served
}
