//! The Connector-backed effect port: [`ConnectorEffects`] performs an admitted action request
//! through the action's binding to one Connector operation (`story:effect-invocation`, Atlas ADR
//! 0082).
//!
//! The host's composition declares a commission's bindings, the generated
//! [`ActionBinding`]s: at most one Connector operation (`instance_id`, `operation_id`) per action
//! id of the commission (`decision-blocker:action-operation-binding`, B). [`ConnectorEffects`]
//! performs exactly the actions they bind, and nothing else; which Connection serves an operation
//! is Connectors' choice.
//!
//! An admitted request is handed to the [`ConnectorInvoker`] port once, with its binding. What the
//! invoker's `Performed` names depends on the binding's effect (`story:connector-read-performed`):
//! for a `Write` binding, the one Connector attempt the invocation produced
//! (`decision-blocker:invocation-attempt-record`, A) and no audit record; for a `Read` binding,
//! which Connectors records no attempt for, the execution audit record it completed and no attempt.
//! Every `Performed` this port returns names exactly the one its binding requires, and any other is
//! a failure to answer. Nothing here retries, since a retry is a new action request, rechecked.
//! Read and consequential actions take this one path (`decision-blocker:read-action-effect-path`,
//! A). Commission depends on no Connectors crate: the invoker is a port the host fills.
//!
//! Through an invoker that can be shared across threads ([`ConnectorInvoker::concurrent`]),
//! [`ConnectorEffects`] reads exactly the actions whose binding declares the effect `Read`, and
//! invokes the reads of one fan-out concurrently, each once, through its own binding
//! (`story:dw-read-fanout`); a fan-out holding any other request invokes nothing. Through any
//! other invoker it reads nothing.

use std::collections::BTreeMap;
use std::fmt;

use crate::model::responsibility::Commission;
use crate::model::responsibility::{
    ActionBinding, ActionBindingData, CommissionId, ConnectorOperationEffect, EffectOutcome,
    EffectOutcomeRefused, action_binding_state, commission_state,
};
use crate::ports::effect::{AdmittedRequest, EffectError, EffectPort, invoke_concurrently};

/// Invokes one Connector operation.
pub trait ConnectorInvoker {
    /// Invokes the operation `binding` names, once, for `request`. `Performed` names what the
    /// binding's effect requires: for `Write`, the one attempt the invocation produced; for `Read`,
    /// the audit record Connectors completed for it. `Refused` says the operation was not
    /// performed and nothing changed. An `Err` is a failure to answer, never a refusal.
    fn invoke(
        &self,
        binding: &ActionBindingData,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError>;

    /// The invoker as several threads may share it, so [`ConnectorEffects`] may invoke the reads
    /// of one fan-out concurrently (`story:dw-read-fanout`). `None` by default: through an invoker
    /// that does not opt in, [`ConnectorEffects`] reads nothing and never fans out.
    fn concurrent(&self) -> Option<&(dyn ConnectorInvoker + Sync)> {
        None
    }
}

impl<T: ConnectorInvoker + ?Sized> ConnectorInvoker for &T {
    fn invoke(
        &self,
        binding: &ActionBindingData,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        (**self).invoke(binding, request)
    }

    fn concurrent(&self) -> Option<&(dyn ConnectorInvoker + Sync)> {
        (**self).concurrent()
    }
}

/// Why a commission's bindings cannot make a [`ConnectorEffects`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingError {
    /// A binding of another commission, by its key or its `commission_id`.
    OtherCommission(Box<ActionBindingData>),
    /// A second binding for this action id.
    Duplicate(String),
}

impl fmt::Display for BindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OtherCommission(binding) => write!(
                f,
                "the binding of `{}` belongs to commission {} (key {}), not this one",
                binding.binding.action,
                binding.commission_id.0.0,
                binding.binding.commission_id.0.0
            ),
            Self::Duplicate(action) => write!(f, "`{action}` is bound twice"),
        }
    }
}

impl std::error::Error for BindingError {}

/// The [`EffectPort`] of one commission's bindings, invoking through `I`.
#[derive(Debug)]
pub struct ConnectorEffects<I> {
    commission_id: CommissionId,
    bindings: BTreeMap<String, ActionBindingData>,
    invoker: I,
}

impl<I> ConnectorEffects<I> {
    /// The port of `commission`'s `bindings`, invoking through `invoker`. Refuses a binding of
    /// another commission and a second binding for one action id.
    pub fn new(
        commission: &Commission<commission_state::Assigned>,
        bindings: impl IntoIterator<Item = ActionBinding<action_binding_state::Declared>>,
        invoker: I,
    ) -> Result<Self, BindingError> {
        let commission_id = commission.data().commission_id.clone();
        let mut bound = BTreeMap::new();
        for binding in bindings {
            let data = binding.into_data();
            if data.commission_id != commission_id || data.binding.commission_id != commission_id {
                return Err(BindingError::OtherCommission(Box::new(data)));
            }
            let action = data.binding.action.clone();
            if bound.insert(action.clone(), data).is_some() {
                return Err(BindingError::Duplicate(action));
            }
        }
        Ok(Self {
            commission_id,
            bindings: bound,
            invoker,
        })
    }

    /// The binding of `action`, if the commission binds it.
    pub fn binding(&self, action: &str) -> Option<&ActionBindingData> {
        self.bindings.get(action)
    }
}

impl<I: ConnectorInvoker> EffectPort for ConnectorEffects<I> {
    /// Whether the commission binds `action`.
    fn performs(&self, action: &str) -> bool {
        self.bindings.contains_key(action)
    }

    /// Whether `action`'s binding declares the effect `Read` and the invoker can be shared for a
    /// fan-out ([`ConnectorInvoker::concurrent`]).
    fn reads(&self, action: &str) -> bool {
        self.invoker.concurrent().is_some()
            && self
                .bindings
                .get(action)
                .is_some_and(|binding| binding.effect == ConnectorOperationEffect::Read)
    }

    /// Invokes each of `requests` as [`EffectPort::invoke`] does, concurrently, through the
    /// invoker shared ([`ConnectorInvoker::concurrent`]); `None`, invoking nothing, when the
    /// invoker cannot be shared. The concurrent path is for `Read` bindings only, whoever answered
    /// [`EffectPort::reads`]: when any request's action has no binding or a binding whose effect is
    /// not `Read`, nothing is invoked and every request answers an [`EffectError`] naming it.
    fn invoke_reads(
        &self,
        commission: &Commission<commission_state::Assigned>,
        requests: &[AdmittedRequest],
    ) -> Option<Vec<Result<EffectOutcome, EffectError>>> {
        if let Some(request) = requests.iter().find(|request| {
            self.bindings
                .get(&request.data().action)
                .is_none_or(|binding| binding.effect != ConnectorOperationEffect::Read)
        }) {
            let error = EffectError::new(format!(
                "`{}` is not bound to a Read operation, so the fan-out invoked nothing",
                request.data().action
            ));
            return Some(requests.iter().map(|_| Err(error.clone())).collect());
        }
        let shared = Shared {
            commission_id: &self.commission_id,
            bindings: &self.bindings,
            invoker: self.invoker.concurrent()?,
        };
        Some(invoke_concurrently(&shared, commission, requests))
    }

    /// Hands `request` to the invoker once, with its action's binding. An unbound action is
    /// refused and nothing is invoked; a commission other than the one the bindings belong to, and
    /// an invoker's `Performed` that does not name exactly what its binding's effect requires (an
    /// attempt for `Write`, an audit record for `Read`), are failures to answer.
    fn invoke(
        &self,
        commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        perform(
            &self.commission_id,
            &self.bindings,
            &self.invoker,
            commission,
            request,
        )
    }
}

/// The bindings of a [`ConnectorEffects`] over its invoker shared, for a fan-out's threads.
struct Shared<'a> {
    commission_id: &'a CommissionId,
    bindings: &'a BTreeMap<String, ActionBindingData>,
    invoker: &'a (dyn ConnectorInvoker + Sync),
}

impl EffectPort for Shared<'_> {
    fn performs(&self, action: &str) -> bool {
        self.bindings.contains_key(action)
    }

    fn invoke(
        &self,
        commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        perform(
            self.commission_id,
            self.bindings,
            self.invoker,
            commission,
            request,
        )
    }
}

/// [`ConnectorEffects`]'s invocation of `request` through `invoker`, as its
/// [`EffectPort::invoke`] documents.
fn perform<I: ConnectorInvoker + ?Sized>(
    commission_id: &CommissionId,
    bindings: &BTreeMap<String, ActionBindingData>,
    invoker: &I,
    commission: &Commission<commission_state::Assigned>,
    request: &AdmittedRequest,
) -> Result<EffectOutcome, EffectError> {
    if commission.data().commission_id != *commission_id {
        return Err(EffectError::new(format!(
            "the bindings are commission {}'s, not commission {}'s",
            commission_id.0.0,
            commission.data().commission_id.0.0
        )));
    }
    let action = &request.data().action;
    let Some(binding) = bindings.get(action) else {
        return Ok(EffectOutcome::Refused(EffectOutcomeRefused {
            reason: format!("`{action}` has no binding"),
            ends_run: false,
        }));
    };
    match invoker.invoke(binding, request)? {
        EffectOutcome::Performed(performed) => {
            let named = match (binding.effect, &performed.attempt, &performed.audit) {
                (ConnectorOperationEffect::Write, Some(_), None)
                | (ConnectorOperationEffect::Read, None, Some(_)) => None,
                (ConnectorOperationEffect::Write, None, _) => Some("named no attempt"),
                (ConnectorOperationEffect::Write, Some(_), Some(_)) => {
                    Some("named an audit record beside its attempt")
                }
                (ConnectorOperationEffect::Read, None, None) => Some("named no audit record"),
                (ConnectorOperationEffect::Read, Some(_), _) => {
                    Some("is a read and named an attempt")
                }
            };
            match named {
                None => Ok(EffectOutcome::Performed(performed)),
                Some(named) => Err(EffectError::new(format!(
                    "the Connector operation `{}` of `{}` performed `{action}` and {named}",
                    binding.operation_id.0, binding.instance_id.0
                ))),
            }
        }
        refused => Ok(refused),
    }
}
