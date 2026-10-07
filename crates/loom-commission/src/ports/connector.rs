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
//! An admitted request is handed to the [`ConnectorInvoker`] port once, with its binding. The
//! invoker's `Performed` names the one Connector attempt the invocation produced
//! (`decision-blocker:invocation-attempt-record`, A): every `Performed` this port returns carries
//! an attempt, and one without is a failure to answer. Nothing here retries, since a retry is a new
//! action request, rechecked. Read and consequential actions take this one path
//! (`decision-blocker:read-action-effect-path`, A). Commission depends on no Connectors crate: the
//! invoker is a port the host fills.

use std::collections::BTreeMap;
use std::fmt;

use crate::model::responsibility::Commission;
use crate::model::responsibility::{
    ActionBinding, ActionBindingData, CommissionId, EffectOutcome, EffectOutcomeRefused,
    action_binding_state, commission_state,
};
use crate::ports::effect::{AdmittedRequest, EffectError, EffectPort};

/// Invokes one Connector operation.
pub trait ConnectorInvoker {
    /// Invokes the operation `binding` names, once, for `request`. `Performed` names the one
    /// attempt the invocation produced; `Refused` says the operation was not performed and nothing
    /// changed. An `Err` is a failure to answer, never a refusal.
    fn invoke(
        &self,
        binding: &ActionBindingData,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError>;
}

impl<T: ConnectorInvoker + ?Sized> ConnectorInvoker for &T {
    fn invoke(
        &self,
        binding: &ActionBindingData,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        (**self).invoke(binding, request)
    }
}

/// Why a commission's bindings cannot make a [`ConnectorEffects`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingError {
    /// A binding of another commission, by its key or its `commission_id`.
    OtherCommission(ActionBindingData),
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
                return Err(BindingError::OtherCommission(data));
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

    /// Hands `request` to the invoker once, with its action's binding. An unbound action is
    /// refused and nothing is invoked; a commission other than the one the bindings belong to, and
    /// an invoker's `Performed` that names no attempt, are failures to answer.
    fn invoke(
        &self,
        commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        if commission.data().commission_id != self.commission_id {
            return Err(EffectError::new(format!(
                "the bindings are commission {}'s, not commission {}'s",
                self.commission_id.0.0,
                commission.data().commission_id.0.0
            )));
        }
        let action = &request.data().action;
        let Some(binding) = self.bindings.get(action) else {
            return Ok(EffectOutcome::Refused(EffectOutcomeRefused {
                reason: format!("`{action}` has no binding"),
            }));
        };
        match self.invoker.invoke(binding, request)? {
            EffectOutcome::Performed(performed) if performed.attempt.is_none() => {
                Err(EffectError::new(format!(
                    "the Connector operation `{}` of `{}` performed `{action}` and named no attempt",
                    binding.operation_id.0, binding.instance_id.0
                )))
            }
            answered => Ok(answered),
        }
    }
}
