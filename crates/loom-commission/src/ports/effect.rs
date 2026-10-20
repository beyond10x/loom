//! The `EffectPort` port: where an admitted action request takes effect (Atlas ADR 0082).
//!
//! Commission's runtime hands the port a request only after revalidating it against the case's
//! current revision, its current frontier and, where the frontier names a capability, the
//! authority provider's allow. [`AdmittedRequest`] is the proof: only the runtime makes one, so an
//! effect port cannot be handed a request that skipped revalidation through this trait.
//!
//! The port answers with the generated [`EffectOutcome`]: what it did, that it refused and
//! changed nothing, or that the work the request started has not finished (`Pending`,
//! `story:pending-effect`). The runtime delivers that outcome to the governor as an observation,
//! never as evidence (Atlas ADR 0074). A port that cannot answer at all returns an
//! [`EffectError`]; the runtime then suspends the Run. A request answered `Pending` is handed to
//! the port again at the run's next iteration, the same request under its own id, revalidated
//! first as before its first invocation, until the port answers otherwise or the case moves, the
//! frontier or the provider no longer admits it, or a budget ends the run (`RunOutcome::Waiting`).
//! Nothing waits between two invocations: how long one takes is the port's, and a port keys the
//! work it started by the request's id.
//!
//! The port is domain-neutral. [`crate::ports::connector::ConnectorEffects`] is the
//! Connector-backed implementation: it performs the actions a commission's bindings name, each
//! through its one Connector operation (`decision-blocker:action-operation-binding`); what
//! confines the operation is Connectors' (`decision-blocker:connector-substrate-containment`).
//! [`EffectPort::performs`] also decides what an executor sees: the runtime hands it the frontier
//! less every action the port does not perform and that needs no authority.
//!
//! A port may also opt into read fan-out (`story:dw-read-fanout`): [`EffectPort::reads`] says which
//! actions it only reads, and [`EffectPort::invoke_reads`] invokes several admitted reads
//! concurrently, each once, answering in the order given. Both default to no: a port that keeps
//! them is handed one request at a time. Invoking concurrently needs a port several threads can
//! share, so a `Sync` port opts in through [`invoke_concurrently`], which invokes each request on
//! its own scoped thread and joins them all.

use std::fmt;
use std::thread;

use crate::model::responsibility::{
    ActionRequestData, Commission, EffectOutcome, commission_state,
};

/// Performs admitted action requests.
pub trait EffectPort {
    /// Whether this port performs `action` at all, whatever its arguments. The runtime runs no
    /// executor on a frontier that lists no action the port performs, and hands an executor no
    /// action the port does not perform unless the action needs authority.
    fn performs(&self, action: &str) -> bool;

    /// Whether `action` only reads, so the runtime may invoke it concurrently with other reads
    /// proposed with it (`ExecutorOutcome::ProposedReads`). `false` by default: a port that does
    /// not opt in never has a fan-out admitted. A port that answers `true` for an action performs
    /// it and answers [`EffectPort::invoke_reads`].
    fn reads(&self, action: &str) -> bool {
        let _ = action;
        false
    }

    /// Invokes each of `requests`, every one an admitted read, once, concurrently, and answers
    /// what each returned in the order given. `None` by default: the port cannot invoke
    /// concurrently and nothing was invoked. A `Sync` port answers
    /// `Some(invoke_concurrently(self, commission, requests))`.
    fn invoke_reads(
        &self,
        commission: &Commission<commission_state::Assigned>,
        requests: &[AdmittedRequest],
    ) -> Option<Vec<Result<EffectOutcome, EffectError>>> {
        let _ = (commission, requests);
        None
    }

    /// Performs `request` for `commission`, refuses it, or answers that the work it started for it
    /// has not finished (`Pending`), and the runtime hands it the same request again later. An
    /// `Err` is a failure to answer, never a refusal.
    fn invoke(
        &self,
        commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError>;
}

/// Invokes each of `requests` through `port` once, each on its own scoped thread, and joins them
/// all before it answers, in the order given, what each returned. A read that panics answers an
/// [`EffectError`] once every read has joined; no read is retried.
pub fn invoke_concurrently<P: EffectPort + Sync + ?Sized>(
    port: &P,
    commission: &Commission<commission_state::Assigned>,
    requests: &[AdmittedRequest],
) -> Vec<Result<EffectOutcome, EffectError>> {
    thread::scope(|scope| {
        let running: Vec<_> = requests
            .iter()
            .map(|request| scope.spawn(move || port.invoke(commission, request)))
            .collect();
        running
            .into_iter()
            .zip(requests)
            .map(|(read, request)| {
                read.join().unwrap_or_else(|_| {
                    Err(EffectError::new(format!(
                        "the read of `{}` panicked",
                        request.data().action
                    )))
                })
            })
            .collect()
    })
}

/// An action request the runtime revalidated and admitted. Only the runtime makes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedRequest(ActionRequestData);

impl AdmittedRequest {
    /// The request as the runtime admitted it.
    pub(crate) fn new(request: ActionRequestData) -> Self {
        Self(request)
    }

    /// The admitted request.
    pub fn data(&self) -> &ActionRequestData {
        &self.0
    }
}

/// Why an effect port could not answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectError {
    /// What the port reported.
    pub message: String,
}

impl EffectError {
    /// A failure carrying the port's own message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for EffectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "effect failed: {}", self.message)
    }
}

impl std::error::Error for EffectError {}
