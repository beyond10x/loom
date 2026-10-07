//! A recording fake `ConnectorInvoker`.
//!
//! It records each call with the Connector operation it was asked to invoke and the admitted
//! request, in call order, and answers the `n`-th call with its scripted answer or, once the script
//! is used up, `Performed` naming attempt [`RecordingInvoker::attempt`]`(n)` with report
//! [`RecordingInvoker::report`] of the operation. It reaches no Connector.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::responsibility::{
    ActionBindingData, ActionRequestData, ConnectorAttemptId, ConnectorInstanceId,
    ConnectorOperationId, EffectOutcome, EffectOutcomePerformed,
};
use b10x_loom_commission::ports::connector::ConnectorInvoker;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError};

/// One call the invoker received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvokerCall {
    /// The Connector instance of the operation invoked.
    pub instance_id: ConnectorInstanceId,
    /// The operation invoked.
    pub operation_id: ConnectorOperationId,
    /// The admitted request it was invoked for.
    pub request: ActionRequestData,
}

/// A `ConnectorInvoker` that records each call and answers from its script.
#[derive(Debug, Default)]
pub struct RecordingInvoker {
    answers: Mutex<VecDeque<Result<EffectOutcome, EffectError>>>,
    calls: Mutex<Vec<InvokerCall>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl RecordingInvoker {
    /// An invoker that answers every call `Performed`.
    pub fn new() -> Self {
        Self::default()
    }

    /// The same invoker, answering its first calls with `answers`, in order.
    pub fn answering(
        self,
        answers: impl IntoIterator<Item = Result<EffectOutcome, EffectError>>,
    ) -> Self {
        Self {
            answers: Mutex::new(answers.into_iter().collect()),
            ..self
        }
    }

    /// Every call so far, in the order received.
    pub fn calls(&self) -> Vec<InvokerCall> {
        lock(&self.calls).clone()
    }

    /// The attempt the unscripted answer to the `n`-th call names, counting from 1.
    pub fn attempt(n: usize) -> ConnectorAttemptId {
        ConnectorAttemptId(format!("attempt-{n}"))
    }

    /// The report of the unscripted answer for the operation `instance`, `operation`.
    pub fn report(instance: &ConnectorInstanceId, operation: &ConnectorOperationId) -> Value {
        Value::Object(vec![
            ("instance_id".to_owned(), Value::Text(instance.0.clone())),
            ("operation_id".to_owned(), Value::Text(operation.0.clone())),
        ])
    }
}

impl ConnectorInvoker for RecordingInvoker {
    fn invoke(
        &self,
        binding: &ActionBindingData,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let n = {
            let mut calls = lock(&self.calls);
            calls.push(InvokerCall {
                instance_id: binding.instance_id.clone(),
                operation_id: binding.operation_id.clone(),
                request: request.data().clone(),
            });
            calls.len()
        };
        lock(&self.answers).pop_front().unwrap_or_else(|| {
            Ok(EffectOutcome::Performed(EffectOutcomePerformed {
                report: Self::report(&binding.instance_id, &binding.operation_id),
                attempt: Some(Self::attempt(n)),
            }))
        })
    }
}
