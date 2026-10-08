#![forbid(unsafe_code)]

//! Commission's [`ConnectorInvoker`] over a Connectors service (`story:connectors-invoker`).
//!
//! [`ConnectorsInvoker`] holds the host's [`ConnectorEndpoint`]s, one per Connector instance. For a
//! binding it resolves the binding's `instance_id` to its endpoint, resolves the endpoint's
//! credential reference through the host's [`CredentialResolver`], describes the service
//! (`GET /v1/describe`) and invokes the bound operation once on the v1alpha2 binding
//! (`POST /v1alpha2/invoke`, `connectors_client::Client::invoke_v1alpha2`) with the admitted
//! request's arguments. It never falls back to `/v1/invoke` and never resends.
//!
//! The binding's `effect` must be the one the service describes for the operation: `Write` for
//! the `mutation` profile (an `external_write`, for which Connectors records an attempt), `Read`
//! for every other profile (`story:connector-read-performed`). A binding the service describes
//! otherwise is `Err`, and the operation is not invoked.
//!
//! The answer is read as Connectors states it (compatibility § 5 of the Connectors service
//! contract):
//!
//! | Connectors answers | The invoker answers |
//! |---|---|
//! | for a `Write` binding, success with a `mutation` naming its attempt on the bound instance | `Performed`: the result as the report, the attempt's id |
//! | for a `Read` binding, success with no `mutation` and its audit record `complete`, with a non-empty `audit_ref` | `Performed`: the result as the report, the audit record's `audit_ref` |
//! | for a `Write` binding, error with a `mutation` classified `refused` or `not_attempted`, on a valid response | `Refused` |
//! | anything else: a write's success naming no attempt or another instance's, a read's answer with a `mutation` (success or error), a read's audit record not `complete` or with an empty `audit_ref`, an error with no `mutation`, `unknown`, `applied` without a delivered result, a protocol or transport failure | `Err` |
//!
//! An error with no `mutation` is no proof that the operation was not dispatched, so it is never
//! a refusal. An `instance_id` with no endpoint, a credential that does not resolve, a failed
//! describe, a service that describes another instance and a binding whose effect is not the one
//! described are `Err` before the operation is invoked, and so is an argument number that cannot be
//! sent without changing its value (one beyond the 64-bit integer range, or one whose shortest
//! float text differs from the admitted one).
//!
//! `ConnectorInvoker::invoke` is synchronous and the client is async: the invoker owns a Tokio
//! runtime and runs each invocation on it, so it can be called from any thread, including one
//! inside another runtime.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, mpsc};

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::responsibility::{
    ActionBindingData, ConnectorAttemptId, ConnectorAuditRef, ConnectorCredentialRef,
    ConnectorEndpoint, ConnectorEndpointData, ConnectorInstanceId, ConnectorOperationEffect,
    EffectOutcome, EffectOutcomePerformed, EffectOutcomeRefused, connector_endpoint_state,
};
use b10x_loom_commission::ports::connector::ConnectorInvoker;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError};
use connectors_client::{Client, Endpoint, Failure, Invoked};
use connectors_core::v1alpha2::{AuditStatus, EffectKnowledge, ErrorCode};

/// Resolves the credential an endpoint names. The host supplies it; the invoker reads no
/// credential itself and keeps none between invocations.
pub trait CredentialResolver: Send + Sync {
    /// The service credential `credential` names.
    fn resolve(&self, credential: &ConnectorCredentialRef) -> Result<String, CredentialError>;
}

/// Why a credential could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialError {
    /// What the resolver reported. It must not carry the secret.
    pub message: String,
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CredentialError {}

/// Why the endpoints cannot make a [`ConnectorsInvoker`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointError {
    /// A second endpoint for this instance.
    Duplicate(ConnectorInstanceId),
    /// The client refuses the endpoint's configuration: a URL that does not parse, a scheme other
    /// than `https` (or `http` when plaintext is admitted), credentials, a query or a fragment.
    Refused {
        /// The instance whose endpoint is refused.
        instance_id: ConnectorInstanceId,
        /// What the client reported.
        message: String,
    },
    /// The runtime the invoker runs invocations on could not be started.
    Runtime(String),
}

impl fmt::Display for EndpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(instance) => write!(f, "`{}` has two endpoints", instance.0),
            Self::Refused {
                instance_id,
                message,
            } => write!(
                f,
                "the endpoint of `{}` is refused: {message}",
                instance_id.0
            ),
            Self::Runtime(message) => write!(f, "the invoker's runtime did not start: {message}"),
        }
    }
}

impl std::error::Error for EndpointError {}

/// One declared endpoint and the client transport made from it.
struct Served {
    data: ConnectorEndpointData,
    transport: Endpoint,
}

/// A [`ConnectorInvoker`] over the host's Connectors endpoints, one per instance.
pub struct ConnectorsInvoker {
    endpoints: BTreeMap<String, Served>,
    resolver: Arc<dyn CredentialResolver>,
    runtime: Option<tokio::runtime::Runtime>,
}

impl fmt::Debug for ConnectorsInvoker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectorsInvoker")
            .field(
                "endpoints",
                &self.endpoints.values().map(|s| &s.data).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl ConnectorsInvoker {
    /// The invoker over `endpoints`, resolving their credentials through `resolver`. Refuses a
    /// second endpoint for one instance and an endpoint the client refuses.
    pub fn new(
        endpoints: impl IntoIterator<Item = ConnectorEndpoint<connector_endpoint_state::Declared>>,
        resolver: impl CredentialResolver + 'static,
    ) -> Result<Self, EndpointError> {
        let mut served = BTreeMap::new();
        for endpoint in endpoints {
            let data = endpoint.into_data();
            if served.contains_key(&data.instance_id.0) {
                return Err(EndpointError::Duplicate(data.instance_id));
            }
            let transport = Endpoint::new(&data.url.0, data.allow_plaintext).map_err(|error| {
                EndpointError::Refused {
                    instance_id: data.instance_id.clone(),
                    message: error.message,
                }
            })?;
            served.insert(data.instance_id.0.clone(), Served { data, transport });
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("loom-connectors")
            .enable_all()
            .build()
            .map_err(|error| EndpointError::Runtime(error.to_string()))?;
        Ok(Self {
            endpoints: served,
            resolver: Arc::new(resolver),
            runtime: Some(runtime),
        })
    }

    /// The endpoint declared for `instance`, if any.
    pub fn endpoint(&self, instance: &ConnectorInstanceId) -> Option<&ConnectorEndpointData> {
        self.endpoints.get(&instance.0).map(|served| &served.data)
    }

    /// Runs `future` to completion on the invoker's runtime and waits for it on this thread.
    fn block_on<T: Send + 'static>(
        &self,
        future: impl Future<Output = T> + Send + 'static,
    ) -> Result<T, EffectError> {
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| EffectError::new("the invoker's runtime is shut down"))?;
        let (sender, receiver) = mpsc::sync_channel(1);
        runtime.spawn(async move {
            let _ = sender.send(future.await);
        });
        receiver
            .recv()
            .map_err(|_| EffectError::new("the invocation stopped before it answered"))
    }
}

impl Drop for ConnectorsInvoker {
    fn drop(&mut self) {
        // Dropping a runtime blocks, which panics inside another runtime; shutting it down in the
        // background does not.
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// What one invocation returned, before it is read as an outcome.
enum Answered {
    Invoked(Invoked),
    Failed(Box<Failure>),
}

impl ConnectorInvoker for ConnectorsInvoker {
    fn invoke(
        &self,
        binding: &ActionBindingData,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let instance = &binding.instance_id.0;
        let operation = binding.operation_id.0.clone();
        let Some(served) = self.endpoints.get(instance) else {
            return Err(EffectError::new(format!(
                "no Connectors endpoint is declared for instance `{instance}`"
            )));
        };
        let token = self
            .resolver
            .resolve(&served.data.credential)
            .map_err(|error| {
                EffectError::new(format!(
                    "the credential `{}` of instance `{instance}` did not resolve: {error}",
                    served.data.credential.0
                ))
            })?;
        let client: Client = served.transport.with_token(token).map_err(|error| {
            EffectError::new(format!(
                "the credential of instance `{instance}` is refused: {}",
                error.message
            ))
        })?;
        let input = to_serde(&request.data().arguments.0)?;
        let expected = instance.clone();
        let target = operation.clone();
        let effect = binding.effect;
        let answered = self.block_on(async move {
            let descriptor = client.describe().await.map_err(|error| {
                format!(
                    "describing instance `{expected}` failed: {:?}: {}",
                    error.code, error.message
                )
            })?;
            if descriptor.instance != expected {
                return Err(format!(
                    "the endpoint of instance `{expected}` describes instance `{}`",
                    descriptor.instance
                ));
            }
            // An operation the service does not describe fails in the client, before any call.
            if let Ok(described) = descriptor.operation(&target) {
                let writes = described.profile == MUTATION_PROFILE;
                if writes != (effect == ConnectorOperationEffect::Write) {
                    return Err(format!(
                        "the operation `{target}` of instance `{expected}` is described with \
                         profile `{}`, and its binding declares it {effect:?}",
                        described.profile
                    ));
                }
            }
            Ok(
                match client.invoke_v1alpha2(&descriptor, &target, input).await {
                    Ok(invoked) => Answered::Invoked(invoked),
                    Err(failure) => Answered::Failed(failure),
                },
            )
        })?;
        match answered.map_err(EffectError::new)? {
            Answered::Invoked(invoked) => match binding.effect {
                ConnectorOperationEffect::Write => performed(instance, &operation, invoked),
                ConnectorOperationEffect::Read => read(instance, &operation, invoked),
            },
            Answered::Failed(failure) => refused(binding.effect, instance, &operation, &failure),
        }
    }
}

/// The profile of an operation Connectors records an attempt for: its host declaration includes
/// `external_write`, and a read profile cannot carry it (Connectors' operations contract § 2, the
/// effect declaration rule). The v1alpha1 descriptor carries no `effects`; this is the host's own
/// discriminator.
const MUTATION_PROFILE: &str = "mutation";

/// A read's success is `Performed` only when Connectors recorded no attempt for it and completed
/// its audit record; the outcome names that record.
fn read(instance: &str, operation: &str, invoked: Invoked) -> Result<EffectOutcome, EffectError> {
    if invoked.mutation.is_some() {
        return Err(EffectError::new(format!(
            "the Connector read `{operation}` of `{instance}` succeeded and recorded an attempt"
        )));
    }
    // An empty `audit_ref` names no record: absence is never an opaque reference.
    let audit = match (invoked.audit_status, invoked.audit_ref) {
        (AuditStatus::Complete, Some(audit)) if !audit.is_empty() => audit,
        (AuditStatus::Complete, _) => {
            return Err(EffectError::new(format!(
                "the Connector read `{operation}` of `{instance}` succeeded and named no audit record"
            )));
        }
        (status, _) => {
            return Err(EffectError::new(format!(
                "the Connector read `{operation}` of `{instance}` succeeded and its audit record is \
                 {status:?}, not complete"
            )));
        }
    };
    Ok(EffectOutcome::Performed(EffectOutcomePerformed {
        report: from_serde(&invoked.result),
        attempt: None,
        audit: Some(ConnectorAuditRef(audit)),
    }))
}

/// A write's success is `Performed` only when it names the attempt Connectors recorded.
fn performed(
    instance: &str,
    operation: &str,
    invoked: Invoked,
) -> Result<EffectOutcome, EffectError> {
    let attempt = invoked
        .mutation
        .filter(|mutation| mutation.classification == EffectKnowledge::Applied)
        .and_then(|mutation| mutation.attempt);
    let Some(attempt) = attempt else {
        return Err(EffectError::new(format!(
            "the Connector operation `{operation}` of `{instance}` succeeded and named no attempt"
        )));
    };
    if attempt.instance != instance {
        return Err(EffectError::new(format!(
            "the Connector operation `{operation}` of `{instance}` succeeded and named an attempt \
             of instance `{}`",
            attempt.instance
        )));
    }
    Ok(EffectOutcome::Performed(EffectOutcomePerformed {
        report: from_serde(&invoked.result),
        attempt: Some(ConnectorAttemptId(attempt.id.as_str().to_owned())),
        audit: None,
    }))
}

/// A failure is `Refused` only when a valid response says the recorded attempt of a write was
/// refused or not attempted; every other failure is an `Err`. Connectors records no attempt for a
/// read, so a read's failure is never `Refused`: one that carries a `mutation` contradicts the
/// describe its binding was checked against, as a read's success carrying one does.
fn refused(
    effect: ConnectorOperationEffect,
    instance: &str,
    operation: &str,
    failure: &Failure,
) -> Result<EffectOutcome, EffectError> {
    let answered = failure.audit_status.is_some()
        && !matches!(
            failure.error.code,
            ErrorCode::UpstreamProtocol | ErrorCode::OutcomeUnknown
        );
    let nothing_changed = failure.mutation.as_ref().is_some_and(|mutation| {
        matches!(
            mutation.classification,
            EffectKnowledge::Refused | EffectKnowledge::NotAttempted
        )
    });
    let mut said = format!(
        "the Connector operation `{operation}` of `{instance}` answered {:?}: {}",
        failure.error.code, failure.error.message
    );
    let read = effect == ConnectorOperationEffect::Read;
    if read && failure.mutation.is_some() {
        said.push_str(" (a read, and it recorded an attempt)");
    }
    if answered && nothing_changed && !read {
        Ok(EffectOutcome::Refused(EffectOutcomeRefused {
            reason: said,
        }))
    } else {
        Err(EffectError::new(said))
    }
}

/// A Commission JSON value as the client's.
fn to_serde(value: &Value) -> Result<serde_json::Value, EffectError> {
    Ok(match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(flag) => serde_json::Value::Bool(*flag),
        Value::Number(text) => {
            let number: serde_json::Number = text.parse().map_err(|_| {
                EffectError::new(format!("the argument number `{text}` is not JSON"))
            })?;
            let exact = decimal(text).is_some_and(|admitted| {
                decimal(&number.to_string()).is_some_and(|sent| sent == admitted)
            });
            if !exact {
                return Err(EffectError::new(format!(
                    "the argument number `{text}` cannot be sent without changing its value \
                     (it would be sent as `{number}`)"
                )));
            }
            serde_json::Value::Number(number)
        }
        Value::Text(text) => serde_json::Value::String(text.clone()),
        Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(to_serde).collect::<Result<_, _>>()?)
        }
        Value::Object(members) => {
            let mut object = serde_json::Map::new();
            for (name, member) in members {
                if object.insert(name.clone(), to_serde(member)?).is_some() {
                    return Err(EffectError::new(format!(
                        "the arguments name `{name}` twice"
                    )));
                }
            }
            serde_json::Value::Object(object)
        }
    })
}

/// The value a JSON number's text denotes, as (negative, significant digits, exponent): the
/// number is `digits × 10^exponent`, with no leading or trailing zero in `digits`, and zero is
/// `(false, "", 0)`. Two texts denote the same value exactly when these are equal. `None` for text
/// that is no JSON number.
fn decimal(text: &str) -> Option<(bool, String, i64)> {
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (mantissa, exponent) = match rest.find(['e', 'E']) {
        Some(at) => (&rest[..at], rest[at + 1..].parse::<i64>().ok()?),
        None => (rest, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if whole.is_empty()
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let digits = format!("{whole}{fraction}");
    let digits = digits.trim_start_matches('0');
    let trimmed = digits.trim_end_matches('0');
    if trimmed.is_empty() {
        return Some((false, String::new(), 0));
    }
    let shift = i64::try_from(digits.len() - trimmed.len()).ok()?;
    let places = i64::try_from(fraction.len()).ok()?;
    let exponent = exponent.checked_sub(places)?.checked_add(shift)?;
    Some((negative, trimmed.to_owned(), exponent))
}

/// The client's JSON value as Commission's.
fn from_serde(value: &serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(flag) => Value::Bool(*flag),
        serde_json::Value::Number(number) => Value::Number(number.to_string()),
        serde_json::Value::String(text) => Value::Text(text.clone()),
        serde_json::Value::Array(items) => Value::Array(items.iter().map(from_serde).collect()),
        serde_json::Value::Object(members) => Value::Object(
            members
                .iter()
                .map(|(name, member)| (name.clone(), from_serde(member)))
                .collect(),
        ),
    }
}
