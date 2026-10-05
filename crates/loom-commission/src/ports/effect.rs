//! The `EffectPort` port: where an admitted action request takes effect (Atlas ADR 0082).
//!
//! Commission's runtime hands the port a request only after revalidating it against the case's
//! current revision, its current frontier and, where the frontier names a capability, the
//! authority provider's allow. [`AdmittedRequest`] is the proof: only the runtime makes one, so an
//! effect port cannot be handed a request that skipped revalidation through this trait.
//!
//! The port answers with the generated [`EffectOutcome`]: what it did, or that it refused and
//! changed nothing. The runtime delivers that outcome to the governor as an observation, never as
//! evidence (Atlas ADR 0074). A port that cannot answer at all returns an [`EffectError`]; the
//! runtime then suspends the Run.
//!
//! The port is domain-neutral: how an action binds to an operation, and what confines it, is the
//! implementation's (`decision-blocker:action-operation-binding`,
//! `decision-blocker:connector-substrate-containment`).

use std::fmt;

use crate::model::responsibility::{
    ActionRequestData, Commission, EffectOutcome, commission_state,
};

/// Performs admitted action requests.
pub trait EffectPort {
    /// Whether this port performs `action` at all, whatever its arguments. The runtime runs no
    /// executor on a frontier that lists no action the port performs.
    fn performs(&self, action: &str) -> bool;

    /// Performs `request` for `commission`, or refuses it. An `Err` is a failure to answer, never a
    /// refusal.
    fn invoke(
        &self,
        commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError>;
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
