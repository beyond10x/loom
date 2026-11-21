//! A client of the Connectors wire at a loopback relay URL (`story:connectors-relay-client`).
//!
//! [`ConnectorsRelay`] reads and performs one governed write for one Connectors instance, the one
//! adapter and connection pair behind a credential boundary's Connectors relay
//! (`crates/loom-boundary`, AGENTS.md § Credential boundary). It speaks exactly what that relay
//! admits: `GET /v1/describe`, the legacy bare Descriptor Connectors keeps for describe, and
//! `POST /v1alpha2/invoke` (connectors `contracts/service/compatibility.md` § 2.1). Bodies are
//! encoded and decoded with `connectors-core`'s own types at the pinned tag ([`InvokeRequest`],
//! [`KeyedInvokeRequest`], [`InvokeResponse::decode`] and [`InvokeResponse::decode_keyed`],
//! [`Descriptor`]).
//!
//! It holds no credential. It sends no `authorization` header, takes no token and reads no
//! variable or file for one: the boundary adds its own token to what it relays. Its HTTP client
//! follows no redirect, uses no proxy (the proxy variables are not read), keeps no idle connection,
//! so no request is ever sent again on a connection that went away, and bounds a connect at 5 s and
//! a whole request at 20 s, as `connectors-client`'s does. It runs on a Tokio runtime the client
//! owns, shared by every clone and by [`ConnectorsRelay::over`], and shut down in the background
//! when the last of them drops.
//!
//! The URL rule is [`loopback_url`]: `http://127.0.0.1:<port>` exactly, with no path, user
//! information, query or fragment; [`ConnectorsRelay::new`] calls it first. Every method refuses a
//! source or operation naming another adapter or connection than the client's, before any request,
//! with code `relay-other-instance`.
//!
//! A read is a describe and then one unkeyed invoke. A write is refused before any request when
//! its action or input is not the admitted request's, or its approval proof path is empty, holds a
//! control character or is not absolute (the rules of the command-line write, reusing
//! [`admits`]); it is refused `not-a-write` when the describe gives the operation another profile
//! than [`WRITE_PROFILES`]; it is then sent once, keyed with [`idempotency_key`] of the admitted
//! request, and never sent again. The approval proof is not sent: the binding refuses an
//! `approval` member, and the host's standing grant approves every keyed write.
//!
//! Every answer, per request:
//!
//! | Answer | A read | A write's describe | A write's invoke |
//! |---|---|---|---|
//! | no connection (nothing sent) | `Failed` `relay-unavailable:` | `Failed` `relay-unavailable:` | `Refused` `relay-unavailable` |
//! | the boundary refused it (400, 403, 404, 405, 413) | `Refused` `relay-refused` | `Refused` `relay-refused` | `Refused` `relay-refused` |
//! | the boundary's `token_unavailable`, `upstream_unreachable`, `upstream_timeout` | `Failed` `relay-unavailable:` | `Failed` `relay-unavailable:` | `Refused` `relay-unavailable` |
//! | the boundary's `upstream_failed` | `Failed` `relay-unavailable:` | `Failed` `relay-unavailable:` | `Unknown` |
//! | a valid Connectors error | `Refused` with its code | `Refused` with its code | as [`write_result`] reads it |
//! | success | the invoke, then a [`ReadResult`] | the invoke, or `not-a-write` | as [`write_result`] reads it |
//! | any other answer, lost, unreadable, late | `Failed` `upstream-protocol:` | `Failed` `upstream-protocol:` | `Unknown` |
//!
//! A boundary answer is a non-2xx whose body is exactly the boundary's `{"error", "message"}`
//! with one of its fixed codes at that code's status. A valid Connectors answer at the invoke is
//! read under `connectors-client`'s correlation rule: the request id echoed (or null only on an
//! error without `mutation`) and the HTTP status the one its error code maps to.
//! `unsupported` with a null request id at a write's invoke is a host without standing grants
//! refusing the key: `WriteError::Failed` naming the remedy, and the write is never sent
//! without its key.

use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::BuildHasher as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use b10x_loom_commission::ports::effect::AdmittedRequest;
use connectors_core::v1alpha2::{
    AuditStatus, EffectKnowledge, ErrorCode, InvokeRequest, InvokeResponse, KeyedInvokeRequest,
    ResponseStatus, VERSION,
};
use connectors_core::{Descriptor, RESPONSE_LIMIT, WIRE_VERSION};
use loom::datasource::{
    AdapterAlias, ConnectionId, DataSource, MutationClassification, OperationId, ReadKind,
    ReadRefusal, ReadResult, SourceEntity, SourceName, WriteApproval, WriteOperation, WriteRefusal,
    WriteResult,
};
use loom::json::{self, Value};
use sha2::{Digest as _, Sha256};

use crate::cli::write::admits;
use crate::cli::{CliError, SourceEntities, WRITE_PROFILES, WriteError, idempotency_key};

/// The bound on making a connection to the relay.
const CONNECT_BOUND: Duration = Duration::from_secs(5);
/// The bound on one whole request, its answer read included.
const REQUEST_BOUND: Duration = Duration::from_secs(20);
const DESCRIBE_PATH: &str = "/v1/describe";
const INVOKE_PATH: &str = "/v1alpha2/invoke";
const LOOPBACK_PREFIX: &str = "http://127.0.0.1:";

/// A relay URL [`loopback_url`] admits: `http://127.0.0.1:<port>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayUrl {
    port: u16,
}

impl RelayUrl {
    /// The loopback port.
    pub fn port(&self) -> u16 {
        self.port
    }

    fn at(&self, path: &str) -> String {
        format!("{LOOPBACK_PREFIX}{}{path}", self.port)
    }
}

impl fmt::Display for RelayUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{LOOPBACK_PREFIX}{}", self.port)
    }
}

/// A value [`loopback_url`] refuses. It names the rule, never the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayUrlRefused {
    _rule: (),
}

impl RelayUrlRefused {
    /// The rule a relay URL follows.
    pub const RULE: &'static str = "a Connectors relay URL is `http://127.0.0.1:<port>`, with no \
                                    path, user information, query or fragment";
}

impl fmt::Display for RelayUrlRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Self::RULE)
    }
}

impl std::error::Error for RelayUrlRefused {}

/// The relay URL `value` names: exactly `http://127.0.0.1:<port>`, the port a decimal from 1 to
/// 65535 without a leading zero, and nothing after it (no path, not even `/`, no query, no
/// fragment); no other scheme, host or user information.
///
/// # Errors
/// [`RelayUrlRefused`] for any other value.
pub fn loopback_url(value: &str) -> Result<RelayUrl, RelayUrlRefused> {
    let refused = RelayUrlRefused { _rule: () };
    let digits = value.strip_prefix(LOOPBACK_PREFIX).ok_or(refused.clone())?;
    if digits.is_empty()
        || digits.len() > 5
        || digits.starts_with('0')
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(refused);
    }
    let port = digits.parse::<u16>().map_err(|_| refused)?;
    Ok(RelayUrl { port })
}

/// The HTTP client and the runtime it runs on, shared by every clone of a [`ConnectorsRelay`].
struct Shared {
    runtime: Option<tokio::runtime::Runtime>,
    http: Result<reqwest::Client, String>,
}

impl Shared {
    /// The client a relay client runs on: connect bound [`CONNECT_BOUND`], request bound
    /// [`REQUEST_BOUND`].
    fn start() -> Self {
        Self::bounded(CONNECT_BOUND, REQUEST_BOUND)
    }

    fn bounded(connect: Duration, request: Duration) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("loom-connectors-relay")
            .enable_all()
            .build();
        let runtime = match runtime {
            Ok(runtime) => runtime,
            Err(error) => {
                return Self {
                    runtime: None,
                    http: Err(format!("the relay client's runtime did not start: {error}")),
                };
            }
        };
        let http = reqwest::Client::builder()
            .connect_timeout(connect)
            .timeout(request)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .pool_max_idle_per_host(0)
            .build()
            .map_err(|error| format!("the relay client's HTTP client did not start: {error}"));
        Self {
            runtime: Some(runtime),
            http,
        }
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        // Dropping a runtime blocks, which panics inside another runtime; shutting it down in the
        // background does not.
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// A client of the Connectors wire at a loopback relay URL, for one adapter and connection pair.
#[derive(Clone)]
pub struct ConnectorsRelay {
    url: RelayUrl,
    adapter: AdapterAlias,
    connection: ConnectionId,
    sources: Vec<DataSource>,
    shared: Arc<Shared>,
}

impl fmt::Debug for ConnectorsRelay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectorsRelay")
            .field("url", &self.url)
            .field("adapter", &self.adapter)
            .field("connection", &self.connection)
            .field("sources", &self.sources)
            .finish_non_exhaustive()
    }
}

impl ConnectorsRelay {
    /// A client of the relay at `url` for the one `adapter` and `connection` pair behind it, over
    /// `sources`.
    ///
    /// # Errors
    /// [`RelayUrlRefused`] for a `url` [`loopback_url`] refuses.
    pub fn new(
        url: &str,
        adapter: AdapterAlias,
        connection: ConnectionId,
        sources: Vec<DataSource>,
    ) -> Result<Self, RelayUrlRefused> {
        let url = loopback_url(url)?;
        Ok(Self {
            url,
            adapter,
            connection,
            sources,
            shared: Arc::new(Shared::start()),
        })
    }

    /// The same client (URL, pair, HTTP client and runtime) over other `sources`; a source of
    /// another pair stays refused.
    #[must_use]
    pub fn over(&self, sources: Vec<DataSource>) -> Self {
        Self {
            sources,
            ..self.clone()
        }
    }

    /// The configured sources, each with the [`SourceEntity`] the describe reports for every
    /// kind it declares, in the order list, search, get.
    ///
    /// # Errors
    /// [`CliError::Refused`] with code `relay-other-instance`, before any request, when a
    /// configured source names another pair; otherwise the first describe that fails or is
    /// refused, a write operation included.
    pub fn sources(&self) -> Result<Vec<(DataSource, SourceEntities)>, CliError> {
        for source in &self.sources {
            self.serves_source(source)?;
        }
        self.sources
            .iter()
            .map(|source| {
                let entities = [ReadKind::List, ReadKind::Search, ReadKind::Get]
                    .into_iter()
                    .filter_map(|kind| declared(source, kind).map(|operation| (kind, operation)))
                    .map(|(kind, operation)| {
                        self.describe_read(Some(&source.name), &source.adapter, operation)
                            .map(|described| (kind, described.entity))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((source.clone(), entities))
            })
            .collect()
    }

    /// What the describe reports for the read operation `operation` of `adapter`.
    ///
    /// # Errors
    /// [`CliError::Refused`] with code `relay-other-instance` for another adapter than the
    /// client's, before any request; with `write-operation` for an operation described with the
    /// `mutation` profile; otherwise as the table of the module says.
    pub fn describe(
        &self,
        adapter: &AdapterAlias,
        operation: &OperationId,
    ) -> Result<SourceEntity, CliError> {
        if *adapter != self.adapter {
            return Err(self.other_instance(
                None,
                adapter,
                Some(operation),
                format!("adapter `{}` operation `{}`", adapter.0, operation.0),
            ));
        }
        self.describe_read(None, adapter, operation)
            .map(|described| described.entity)
    }

    /// Reads `operation` of `adapter` on `connection`: one describe, then exactly one unkeyed
    /// invoke with `input`.
    ///
    /// # Errors
    /// [`CliError::Refused`] with code `relay-other-instance` for another pair than the client's,
    /// before any request; otherwise as [`ConnectorsRelay::describe`] and the table of the module.
    pub fn invoke_read(
        &self,
        adapter: &AdapterAlias,
        connection: &ConnectionId,
        operation: &OperationId,
        input: &Value,
    ) -> Result<ReadResult, CliError> {
        if *adapter != self.adapter || *connection != self.connection {
            return Err(self.other_instance(
                None,
                adapter,
                Some(operation),
                format!(
                    "adapter `{}` connection `{}` operation `{}`",
                    adapter.0, connection.0, operation.0
                ),
            ));
        }
        self.read_through(None, adapter, operation, input)
    }

    /// Reads `source` through the operation it declares for `kind`.
    ///
    /// # Errors
    /// [`CliError::Refused`] with code `relay-other-instance` for a source of another pair, and
    /// `undeclared-kind` when `source` declares no operation for `kind`, each before any request;
    /// otherwise as [`ConnectorsRelay::invoke_read`], every refusal naming the source.
    pub fn read(
        &self,
        source: &DataSource,
        kind: ReadKind,
        input: &Value,
    ) -> Result<ReadResult, CliError> {
        self.serves_source(source)?;
        let Some(operation) = declared(source, kind) else {
            return Err(CliError::Refused(ReadRefusal {
                source: Some(source.name.clone()),
                adapter: source.adapter.clone(),
                operation: None,
                code: "undeclared-kind".to_owned(),
                message: format!(
                    "source `{}` declares no {} operation",
                    source.name.0,
                    kind_name(kind)
                ),
            }));
        };
        self.read_through(Some(&source.name), &source.adapter, operation, input)
    }

    /// Writes `operation` for the admitted `request` with `input`: one describe, then exactly one
    /// keyed invoke, never sent again. The write is bound to the admitted request: its `action`
    /// must be the request's and `input` its admitted arguments. `approval` must name an absolute
    /// proof path; the proof is not sent.
    ///
    /// # Errors
    /// [`WriteError::Refused`], before any request, with code `relay-other-instance` for another
    /// pair than the client's, `action-not-admitted`, `input-not-admitted`, `approval-missing`,
    /// `approval-not-a-path` or `approval-not-absolute`; with `not-a-write` when the describe
    /// gives another profile than a write's, before the invoke; otherwise as the table of the
    /// module says.
    pub fn invoke_write(
        &self,
        request: &AdmittedRequest,
        operation: &WriteOperation,
        input: &Value,
        approval: &WriteApproval,
    ) -> Result<WriteResult, WriteError> {
        let WriteOperation {
            action,
            adapter,
            connection,
            operation,
        } = operation;
        if *adapter != self.adapter || *connection != self.connection {
            return Err(WriteError::from(self.other_instance(
                None,
                adapter,
                Some(operation),
                format!(
                    "the write of adapter `{}` connection `{}` operation `{}`",
                    adapter.0, connection.0, operation.0
                ),
            )));
        }
        let admitted = request.data();
        if *action != admitted.action {
            return Err(write_refused(
                adapter,
                operation,
                "action-not-admitted",
                format!(
                    "a write of action `{action}` was asked for a request admitted for `{}`",
                    admitted.action
                ),
            ));
        }
        if !admits(&admitted.arguments.0, input) {
            return Err(write_refused(
                adapter,
                operation,
                "input-not-admitted",
                format!(
                    "the input of a write of adapter `{}` operation `{}` is not the arguments the \
                     runtime admitted for `{action}`",
                    adapter.0, operation.0
                ),
            ));
        }
        check_proof(adapter, operation, &approval.proof_file)?;
        let key = idempotency_key(request);
        let subject = subject(None, adapter, operation);
        let described = self
            .described(operation)
            .map_err(|fault| describe_write_fault(fault, adapter, operation, &subject))?;
        if !WRITE_PROFILES.contains(&described.profile.as_str()) {
            return Err(write_refused(
                adapter,
                operation,
                "not-a-write",
                format!(
                    "{subject} is described with the `{}` profile, not a write's, and is never \
                     invoked as one",
                    described.profile
                ),
            ));
        }
        let keyed = KeyedInvokeRequest {
            version: VERSION.to_owned(),
            request_id: request_id(),
            operation: operation.0.clone(),
            revision: described.entity.revision.clone(),
            input: to_wire(input).map_err(|reason| {
                WriteError::Failed(format!("the input of {subject} cannot be sent: {reason}"))
            })?,
            idempotency_key: Some(key.clone()),
        };
        let body = keyed.encode().map_err(|violation| {
            WriteError::Failed(format!(
                "the input of {subject} cannot be sent: {violation}"
            ))
        })?;
        let exchange = self.exchange(Some(body));
        match invoke_answer(exchange, &keyed.request_id, Binding::Keyed) {
            Ok(response) => {
                if response.request_id.is_none()
                    && response
                        .error
                        .as_ref()
                        .is_some_and(|error| error.code == ErrorCode::Unsupported)
                {
                    return Err(WriteError::Failed(format!(
                        "the Connectors host refused the idempotency key of {subject} \
                         `unsupported`, with no request id; the write is not sent without it{}",
                        crate::standing_grant_remedy(&described.instance, &operation.0)
                    )));
                }
                Ok(write_result(&response, key))
            }
            Err(Fault::Unavailable(reason)) => Err(write_refused(
                adapter,
                operation,
                "relay-unavailable",
                format!(
                    "the Connectors relay could not take {subject}: {reason}; nothing was sent"
                ),
            )),
            Err(Fault::RelayRefused(code)) => Err(write_refused(
                adapter,
                operation,
                "relay-refused",
                relay_refused(&subject, code),
            )),
            // The request may have left: the write may have happened.
            Err(Fault::UpstreamFailed | Fault::Protocol(_)) => Ok(unknown(key)),
            Err(Fault::Connectors { code, .. }) => Ok(WriteResult {
                code: Some(code),
                ..unknown(key)
            }),
            Err(Fault::NotDescribed) => Ok(unknown(key)),
        }
    }

    fn serves_source(&self, source: &DataSource) -> Result<(), CliError> {
        if source.adapter == self.adapter && source.connection == self.connection {
            return Ok(());
        }
        Err(self.other_instance(
            Some(&source.name),
            &source.adapter,
            None,
            format!(
                "source `{}` of adapter `{}` connection `{}`",
                source.name.0, source.adapter.0, source.connection.0
            ),
        ))
    }

    /// `relay-other-instance`: `what` names another pair than the client's.
    fn other_instance(
        &self,
        source: Option<&SourceName>,
        adapter: &AdapterAlias,
        operation: Option<&OperationId>,
        what: String,
    ) -> CliError {
        CliError::Refused(ReadRefusal {
            source: source.cloned(),
            adapter: adapter.clone(),
            operation: operation.cloned(),
            code: "relay-other-instance".to_owned(),
            message: format!(
                "{what} is not served: this relay client serves adapter `{}` connection `{}` \
                 only, the one Connectors instance behind its relay",
                self.adapter.0, self.connection.0
            ),
        })
    }

    /// The describe of a read: refused `write-operation` for a write's profile.
    fn describe_read(
        &self,
        source: Option<&SourceName>,
        adapter: &AdapterAlias,
        operation: &OperationId,
    ) -> Result<Describing, CliError> {
        let subject = subject(source, adapter, operation);
        let described = self
            .described(operation)
            .map_err(|fault| read_fault(fault, source, adapter, operation, &subject))?;
        if WRITE_PROFILES.contains(&described.profile.as_str()) {
            return Err(CliError::Refused(ReadRefusal {
                source: source.cloned(),
                adapter: adapter.clone(),
                operation: Some(operation.clone()),
                code: "write-operation".to_owned(),
                message: format!(
                    "{subject} is described with the `{}` profile, a write, and is never invoked",
                    described.profile
                ),
            }));
        }
        Ok(described)
    }

    fn read_through(
        &self,
        source: Option<&SourceName>,
        adapter: &AdapterAlias,
        operation: &OperationId,
        input: &Value,
    ) -> Result<ReadResult, CliError> {
        let subject = subject(source, adapter, operation);
        let described = self.describe_read(source, adapter, operation)?;
        let request = InvokeRequest {
            version: VERSION.to_owned(),
            request_id: request_id(),
            operation: operation.0.clone(),
            revision: described.entity.revision,
            input: to_wire(input).map_err(|reason| {
                CliError::Failed(format!("the input of {subject} cannot be sent: {reason}"))
            })?,
        };
        let body = request.encode().map_err(|violation| {
            CliError::Failed(format!(
                "the input of {subject} cannot be sent: {violation}"
            ))
        })?;
        let exchange = self.exchange(Some(body));
        let response = invoke_answer(exchange, &request.request_id, Binding::Unkeyed)
            .map_err(|fault| read_fault(fault, source, adapter, operation, &subject))?;
        if let Some(error) = &response.error {
            let code = crate::code_name(error.code);
            if response.mutation.is_some() {
                return Err(CliError::Failed(format!(
                    "upstream-protocol: Connectors answered {subject}, a read, `{code}` and \
                     recorded an attempt"
                )));
            }
            return Err(CliError::Refused(ReadRefusal {
                source: source.cloned(),
                adapter: adapter.clone(),
                operation: Some(operation.clone()),
                code: code.clone(),
                message: format!("Connectors refused {subject} with `{code}`"),
            }));
        }
        if response.mutation.is_some() {
            return Err(CliError::Failed(format!(
                "upstream-protocol: Connectors answered {subject}, a read, and recorded an attempt"
            )));
        }
        let audit_ref = match (response.audit_status, &response.audit_ref) {
            (AuditStatus::Complete, Some(audit)) if !audit.is_empty() => audit.clone(),
            (AuditStatus::Complete, _) => {
                return Err(CliError::Failed(format!(
                    "upstream-protocol: Connectors answered {subject} and named no audit record"
                )));
            }
            (status, _) => {
                return Err(CliError::Failed(format!(
                    "upstream-protocol: Connectors answered {subject} and its audit record is \
                     {status:?}, not complete"
                )));
            }
        };
        let result = response.result.as_ref().ok_or_else(|| {
            CliError::Failed(format!(
                "upstream-protocol: Connectors answered {subject} with no result"
            ))
        })?;
        let body = from_wire(result).map_err(|reason| {
            CliError::Failed(format!(
                "upstream-protocol: the result of {subject} cannot be read: {reason}"
            ))
        })?;
        Ok(ReadResult {
            body,
            audit_ref: Some(audit_ref),
        })
    }

    /// `GET /v1/describe`, and what it says of `operation`.
    fn described(&self, operation: &OperationId) -> Result<Describing, Fault> {
        let descriptor = describe_answer(self.exchange(None))?;
        let Ok(selected) = descriptor.operation(&operation.0) else {
            return Err(Fault::NotDescribed);
        };
        // The schema identity the command line's describe answers for the same operation
        // (connectors `v0.62.0`, `crates/connectors-host/src/local/owner.rs`, `schema`).
        let schema = connectors_core::digest(&serde_json::json!({
            "instance": descriptor.instance,
            "revision": descriptor.revision,
            "operation": selected.id,
            "input": selected.input_schema,
            "output": selected.output_schema
        }));
        let input_schema = from_wire(&selected.input_schema).map_err(|reason| {
            Fault::Protocol(format!(
                "the described input schema cannot be read: {reason}"
            ))
        })?;
        Ok(Describing {
            entity: SourceEntity {
                operation: operation.clone(),
                schema,
                input_schema,
                revision: descriptor.revision.clone(),
            },
            profile: selected.profile.clone(),
            instance: descriptor.instance.clone(),
        })
    }

    /// One request to the relay: `GET /v1/describe`, or `POST /v1alpha2/invoke` with `body`.
    fn exchange(&self, body: Option<Vec<u8>>) -> Exchange {
        let http = match &self.shared.http {
            Ok(http) => http.clone(),
            Err(reason) => return Exchange::Unreached(reason.clone()),
        };
        let Some(runtime) = self.shared.runtime.as_ref() else {
            return Exchange::Unreached("the relay client's runtime is not running".to_owned());
        };
        let url = self.url.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        runtime.spawn(async move {
            let _ = sender.send(exchange(http, url, body).await);
        });
        receiver.recv().unwrap_or_else(|_| {
            Exchange::Lost("the request stopped before it was answered".to_owned())
        })
    }
}

/// What a describe says of one operation.
struct Describing {
    entity: SourceEntity,
    profile: String,
    instance: String,
}

/// What one request came to.
enum Exchange {
    /// No connection was made: nothing was sent.
    Unreached(String),
    /// A whole answer.
    Answered { status: u16, body: Vec<u8> },
    /// The request may have left, and no whole answer came back.
    Lost(String),
}

async fn exchange(http: reqwest::Client, url: RelayUrl, body: Option<Vec<u8>>) -> Exchange {
    let request = match body {
        None => http.get(url.at(DESCRIBE_PATH)),
        Some(body) => http
            .post(url.at(INVOKE_PATH))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body),
    };
    let mut response = match request.send().await {
        Ok(response) => response,
        Err(error) if error.is_connect() => {
            return Exchange::Unreached(if error.is_timeout() {
                format!("the relay at {url} accepted no connection within the connect bound")
            } else {
                format!("the relay at {url} cannot be connected to")
            });
        }
        Err(error) if error.is_timeout() => {
            return Exchange::Lost("no answer came within the request bound".to_owned());
        }
        Err(_) => return Exchange::Lost("the answer was lost".to_owned()),
    };
    let status = response.status().as_u16();
    if response
        .content_length()
        .is_some_and(|length| length > RESPONSE_LIMIT as u64)
    {
        return Exchange::Lost("the answer exceeds the response limit".to_owned());
    }
    let mut bytes = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if bytes.len() + chunk.len() > RESPONSE_LIMIT {
                    return Exchange::Lost("the answer exceeds the response limit".to_owned());
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(error) if error.is_timeout() => {
                return Exchange::Lost(
                    "the answer did not end within the request bound".to_owned(),
                );
            }
            Err(_) => return Exchange::Lost("the answer was cut off".to_owned()),
        }
    }
    Exchange::Answered {
        status,
        body: bytes,
    }
}

/// Why a request answered nothing a caller can use as it stands.
enum Fault {
    /// Nothing reached Connectors: no connection, or the boundary had no token or no upstream.
    Unavailable(String),
    /// The boundary refused the request itself, with this code; it was not relayed.
    RelayRefused(&'static str),
    /// The boundary's upstream did not answer: the request may have reached Connectors.
    UpstreamFailed,
    /// A valid Connectors error at the describe.
    Connectors { code: String },
    /// The describe does not describe the operation.
    NotDescribed,
    /// Any other answer, or none.
    Protocol(String),
}

/// How the boundary answered, when the answer is one of its own.
#[derive(Clone, Copy)]
enum BoundaryAnswer {
    Refused,
    Unavailable,
    UpstreamFailed,
}

/// The boundary's fixed codes and their statuses (`crates/loom-boundary/src/route.rs` and
/// `src/lib.rs`): the first seven are answered before anything is relayed, the next three before
/// anything reaches the upstream, and `upstream_failed` after the request may have left.
const BOUNDARY: [(&str, u16, BoundaryAnswer); 11] = [
    ("not_relayed", 404, BoundaryAnswer::Refused),
    ("path_refused", 400, BoundaryAnswer::Refused),
    ("target_refused", 400, BoundaryAnswer::Refused),
    ("method_refused", 405, BoundaryAnswer::Refused),
    ("operation_refused", 403, BoundaryAnswer::Refused),
    ("body_too_large", 413, BoundaryAnswer::Refused),
    ("body_unreadable", 400, BoundaryAnswer::Refused),
    ("token_unavailable", 503, BoundaryAnswer::Unavailable),
    ("upstream_unreachable", 502, BoundaryAnswer::Unavailable),
    ("upstream_timeout", 504, BoundaryAnswer::Unavailable),
    ("upstream_failed", 502, BoundaryAnswer::UpstreamFailed),
];

/// The boundary's answer `body` at `status` is: exactly `{"error", "message"}`, both text, the
/// error one of its codes at that code's status.
fn boundary(status: u16, body: &[u8]) -> Option<Fault> {
    let Ok(serde_json::Value::Object(members)) = connectors_core::read_json(body) else {
        return None;
    };
    let (Some(serde_json::Value::String(code)), Some(serde_json::Value::String(_))) =
        (members.get("error"), members.get("message"))
    else {
        return None;
    };
    if members.len() != 2 {
        return None;
    }
    let &(code, _, answer) = BOUNDARY
        .iter()
        .find(|(named, fixed, _)| named == code && *fixed == status)?;
    Some(match answer {
        BoundaryAnswer::Refused => Fault::RelayRefused(code),
        BoundaryAnswer::Unavailable => Fault::Unavailable(format!(
            "the boundary answered `{code}`, and nothing reached Connectors"
        )),
        BoundaryAnswer::UpstreamFailed => Fault::UpstreamFailed,
    })
}

/// The Descriptor a describe answered.
fn describe_answer(exchange: Exchange) -> Result<Descriptor, Fault> {
    let (status, body) = match exchange {
        Exchange::Unreached(reason) => return Err(Fault::Unavailable(reason)),
        Exchange::Lost(reason) => return Err(Fault::Protocol(reason)),
        Exchange::Answered { status, body } => (status, body),
    };
    if (200..300).contains(&status) {
        return match connectors_core::read_json::<Descriptor>(&body) {
            Ok(descriptor) if descriptor.version == WIRE_VERSION => Ok(descriptor),
            _ => Err(Fault::Protocol(
                "the describe answered no v1alpha1 Descriptor".to_owned(),
            )),
        };
    }
    if let Some(fault) = boundary(status, &body) {
        return Err(fault);
    }
    // A legacy Connectors error, at the status the legacy binding gives its code.
    match connectors_core::read_json::<connectors_core::Error>(&body) {
        Ok(error) if legacy_status(&error.code) == status => Err(Fault::Connectors {
            code: legacy_code_name(&error.code),
        }),
        _ => Err(Fault::Protocol(format!(
            "the describe answered HTTP {status} with no Connectors error"
        ))),
    }
}

/// Which reader an invoke's answer is read with.
#[derive(Clone, Copy)]
enum Binding {
    /// `decode`: a read, sent without a key.
    Unkeyed,
    /// `decode_keyed`: a write, sent with its key to a host with standing grants.
    Keyed,
}

/// The valid Connectors answer an invoke got, under `binding` and the correlation rule.
fn invoke_answer(
    exchange: Exchange,
    request_id: &str,
    binding: Binding,
) -> Result<InvokeResponse, Fault> {
    let (status, body) = match exchange {
        Exchange::Unreached(reason) => return Err(Fault::Unavailable(reason)),
        Exchange::Lost(reason) => return Err(Fault::Protocol(reason)),
        Exchange::Answered { status, body } => (status, body),
    };
    if let Some(fault) = boundary(status, &body) {
        return Err(fault);
    }
    let decoded = match binding {
        Binding::Unkeyed => InvokeResponse::decode(&body),
        Binding::Keyed => InvokeResponse::decode_keyed(&body),
    };
    let Ok(response) = decoded else {
        return Err(Fault::Protocol(format!(
            "the invoke answered HTTP {status} with no valid v1alpha2 response"
        )));
    };
    // `connectors-client`'s `read_v1alpha2`: the request's id echoed, or null only on an error
    // without `mutation`, and the status the error code maps to.
    let echoed = match &response.request_id {
        Some(echoed) => echoed == request_id,
        None => response.status == ResponseStatus::Error && response.mutation.is_none(),
    };
    let expected = response
        .error
        .as_ref()
        .map_or(200, |error| http_status(error.code));
    if !echoed || status != expected {
        return Err(Fault::Protocol(
            "the invoke's answer does not correlate with its request or its status".to_owned(),
        ));
    }
    Ok(response)
}

/// What a write's valid Connectors answer at the invoke reports: `classification` is the
/// `mutation`'s, `Unknown` without one; `request_id` the response's, or the original request's
/// for a replay; `body` the `result` on success; `audit_ref` the response's; `code` Connectors'
/// error code as it spells it on the wire. `source_audit` is always `None`: the binding refuses
/// one at `v0.62.0`, so a response carrying it never decodes.
fn write_result(response: &InvokeResponse, key: String) -> WriteResult {
    let classification = match response
        .mutation
        .as_ref()
        .map(|mutation| mutation.classification)
    {
        Some(EffectKnowledge::NotAttempted) => MutationClassification::NotAttempted,
        Some(EffectKnowledge::Refused) => MutationClassification::Refused,
        Some(EffectKnowledge::Applied) => MutationClassification::Applied,
        Some(EffectKnowledge::Unknown) | None => MutationClassification::Unknown,
    };
    let request_id = match &response.mutation {
        Some(mutation) if mutation.replayed => mutation.original_request_id.clone(),
        _ => response.request_id.clone(),
    };
    WriteResult {
        classification,
        idempotency_key: key,
        request_id,
        body: response
            .result
            .as_ref()
            .and_then(|result| from_wire(result).ok()),
        source_audit: None,
        audit_ref: response.audit_ref.clone(),
        code: response
            .error
            .as_ref()
            .map(|error| crate::code_name(error.code)),
    }
}

fn unknown(idempotency_key: String) -> WriteResult {
    WriteResult {
        classification: MutationClassification::Unknown,
        idempotency_key,
        request_id: None,
        body: None,
        source_audit: None,
        audit_ref: None,
        code: None,
    }
}

/// A read's fault, at its describe or its invoke.
fn read_fault(
    fault: Fault,
    source: Option<&SourceName>,
    adapter: &AdapterAlias,
    operation: &OperationId,
    subject: &str,
) -> CliError {
    let refusal = |code: String, message: String| {
        CliError::Refused(ReadRefusal {
            source: source.cloned(),
            adapter: adapter.clone(),
            operation: Some(operation.clone()),
            code,
            message,
        })
    };
    match fault {
        Fault::Unavailable(reason) => CliError::Failed(format!(
            "relay-unavailable: {subject} was not read: {reason}"
        )),
        Fault::UpstreamFailed => CliError::Failed(format!(
            "relay-unavailable: {subject} was not read: the boundary answered `upstream_failed`"
        )),
        Fault::RelayRefused(code) => {
            refusal("relay-refused".to_owned(), relay_refused(subject, code))
        }
        Fault::Connectors { code } => {
            let message = format!("Connectors refused {subject} with `{code}`");
            refusal(code, message)
        }
        Fault::NotDescribed => refusal(
            "not_found".to_owned(),
            format!("the relay's Connectors instance does not describe {subject}"),
        ),
        Fault::Protocol(reason) => {
            CliError::Failed(format!("upstream-protocol: {subject}: {reason}"))
        }
    }
}

/// A write's fault at its describe: nothing was invoked.
fn describe_write_fault(
    fault: Fault,
    adapter: &AdapterAlias,
    operation: &OperationId,
    subject: &str,
) -> WriteError {
    match fault {
        Fault::Unavailable(reason) => WriteError::Failed(format!(
            "relay-unavailable: {subject} was not described: {reason}"
        )),
        Fault::UpstreamFailed => WriteError::Failed(format!(
            "relay-unavailable: {subject} was not described: the boundary answered \
             `upstream_failed`"
        )),
        Fault::RelayRefused(code) => write_refused(
            adapter,
            operation,
            "relay-refused",
            relay_refused(subject, code),
        ),
        Fault::Connectors { code } => {
            let message = format!("Connectors refused the describe of {subject} with `{code}`");
            write_refused(adapter, operation, &code, message)
        }
        Fault::NotDescribed => write_refused(
            adapter,
            operation,
            "not_found",
            format!("the relay's Connectors instance does not describe {subject}"),
        ),
        Fault::Protocol(reason) => {
            WriteError::Failed(format!("upstream-protocol: {subject}: {reason}"))
        }
    }
}

fn relay_refused(subject: &str, code: &str) -> String {
    format!("the Connectors relay refused {subject} with `{code}`, and did not relay it")
}

fn write_refused(
    adapter: &AdapterAlias,
    operation: &OperationId,
    code: &str,
    message: String,
) -> WriteError {
    WriteError::Refused(WriteRefusal {
        adapter: adapter.clone(),
        operation: Some(operation.clone()),
        code: code.to_owned(),
        message,
    })
}

/// The approval proof path rules of the command-line write: not empty, no control character,
/// absolute. The path is checked and never read or sent.
fn check_proof(
    adapter: &AdapterAlias,
    operation: &OperationId,
    proof: &str,
) -> Result<(), WriteError> {
    if proof.is_empty() {
        return Err(write_refused(
            adapter,
            operation,
            "approval-missing",
            format!(
                "a write of adapter `{}` operation `{}` names no approval proof",
                adapter.0, operation.0
            ),
        ));
    }
    if proof.chars().any(char::is_control) {
        return Err(write_refused(
            adapter,
            operation,
            "approval-not-a-path",
            format!("the approval proof path {proof:?} holds a control character"),
        ));
    }
    if !std::path::Path::new(proof).is_absolute() {
        return Err(write_refused(
            adapter,
            operation,
            "approval-not-absolute",
            format!("the approval proof path `{proof}` is not absolute"),
        ));
    }
    Ok(())
}

/// The extended HTTP mapping a valid v1alpha2 answer follows (connectors `v0.62.0`,
/// `crates/connectors-client/src/lib.rs`, `http_status`; compatibility § 5).
fn http_status(code: ErrorCode) -> u16 {
    use ErrorCode::*;
    match code {
        InvalidInput | Unsupported => 400,
        Unauthorized => 401,
        Forbidden | ApprovalRequired | ApprovalRefused | NotGranted | RouteRefused
        | InsufficientScope => 403,
        NotFound => 404,
        StaleDescription | StaleCursor | ApprovalReplayed | IdempotencyConflict
        | StaleAuthority | ConnectionNotReady | SessionNotReady | OfferRejected | Conflict => 409,
        SessionLost | OfferExpired | LeaseExpired | Revoked => 410,
        Capacity => 413,
        RateLimited => 429,
        Internal => 500,
        UpstreamProtocol | OutcomeUnknown => 502,
        Unavailable | RouteUnavailable => 503,
        Timeout => 504,
    }
}

/// The legacy binding's HTTP mapping, which `GET /v1/describe` answers under (connectors
/// `v0.62.0`, `crates/connectors-host/src/server.rs`, `status`).
fn legacy_status(code: &connectors_core::ErrorCode) -> u16 {
    use connectors_core::ErrorCode::*;
    match code {
        InvalidInput | Conflict => 400,
        Unauthorized => 401,
        Forbidden => 403,
        NotFound => 404,
        StaleDescription | StaleCursor => 409,
        Unsupported => 501,
        Capacity | RateLimited => 429,
        Timeout => 504,
        Unavailable => 503,
        UpstreamProtocol => 502,
        Internal => 500,
    }
}

/// A legacy code as Connectors spells it on the wire (`unauthorized`).
fn legacy_code_name(code: &connectors_core::ErrorCode) -> String {
    match serde_json::to_value(code) {
        Ok(serde_json::Value::String(name)) => name,
        _ => format!("{code:?}"),
    }
}

/// A Loom JSON value as the wire's, crossing as JSON text.
fn to_wire(value: &Value) -> Result<serde_json::Value, String> {
    let mut text = String::new();
    json::push_value(&mut text, value);
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

/// The wire's JSON value as Loom's, crossing as JSON text.
fn from_wire(value: &serde_json::Value) -> Result<Value, String> {
    let text = serde_json::to_string(value).map_err(|error| error.to_string())?;
    json::parse(&text).map_err(|error| error.to_string())
}

/// A request id no other request of this process carries: `loom-relay-` and 32 hex digits of a
/// SHA-256 over a per-process counter, the time and two randomly keyed hashes.
fn request_id() -> String {
    static ISSUED: AtomicU64 = AtomicU64::new(0);
    let count = ISSUED.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let mut hasher = Sha256::new();
    hasher.update(b"loom.connectors.relay.request-id/1");
    hasher.update(count.to_be_bytes());
    hasher.update(nanos.to_be_bytes());
    hasher.update(RandomState::new().hash_one(count).to_be_bytes());
    hasher.update(RandomState::new().hash_one(nanos).to_be_bytes());
    let digest: String = hasher
        .finalize()
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("loom-relay-{digest}")
}

/// The operation `source` declares for `kind`.
fn declared(source: &DataSource, kind: ReadKind) -> Option<&OperationId> {
    match kind {
        ReadKind::List => source.list.as_ref(),
        ReadKind::Search => source.search.as_ref(),
        ReadKind::Get => source.get.as_ref(),
    }
}

fn kind_name(kind: ReadKind) -> &'static str {
    match kind {
        ReadKind::List => "list",
        ReadKind::Search => "search",
        ReadKind::Get => "get",
    }
}

/// What a message names: the source when the read named one, else the adapter, and the
/// operation.
fn subject(source: Option<&SourceName>, adapter: &AdapterAlias, operation: &OperationId) -> String {
    match source {
        Some(source) => format!("source `{}` operation `{}`", source.0, operation.0),
        None => format!("adapter `{}` operation `{}`", adapter.0, operation.0),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead as _, BufReader, Read as _, Write as _};
    use std::net::TcpListener;
    use std::sync::atomic::AtomicUsize;
    use std::time::Instant;

    use super::*;

    /// A relay on `127.0.0.1:0` that answers the describe and holds every invoke for `hold`
    /// without an answer, counting the invokes.
    fn holding(hold: Duration) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let url = format!(
            "{LOOPBACK_PREFIX}{}",
            listener.local_addr().expect("the bound address").port()
        );
        let invokes = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&invokes);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                let _ = reader.read_line(&mut line);
                let mut length = 0usize;
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).unwrap_or(0) == 0 || header.trim().is_empty() {
                        break;
                    }
                    if let Some((name, value)) = header.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        length = value.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0; length];
                let _ = reader.read_exact(&mut body);
                let mut stream = reader.into_inner();
                if line.starts_with("GET /v1/describe ") {
                    let descriptor = serde_json::json!({
                        "version": "v1alpha1", "instance": "held", "adapter": "held",
                        "revision": "rev-1", "configuration_schema": {},
                        "operations": [{"id": "items.list", "description": "", "contract": "c",
                            "profile": "read", "input_schema": {}, "output_schema": {}}]
                    })
                    .to_string();
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n\
                         {descriptor}",
                        descriptor.len()
                    );
                } else {
                    counted.fetch_add(1, Ordering::SeqCst);
                    std::thread::spawn(move || {
                        std::thread::sleep(hold);
                        drop(stream);
                    });
                }
            }
        });
        (url, invokes)
    }

    /// The request bound ends an invoke whose answer does not come: the read fails
    /// `upstream-protocol` at the bound, long before the relay would close, and is not resent.
    #[test]
    fn an_answer_past_the_request_bound_is_lost_at_the_bound() {
        let (url, invokes) = holding(Duration::from_secs(5));
        let relay = ConnectorsRelay {
            url: loopback_url(&url).expect("a loopback URL"),
            adapter: AdapterAlias("held".to_owned()),
            connection: ConnectionId("held".to_owned()),
            sources: Vec::new(),
            shared: Arc::new(Shared::bounded(CONNECT_BOUND, Duration::from_millis(300))),
        };
        let started = Instant::now();
        let answer = relay.invoke_read(
            &AdapterAlias("held".to_owned()),
            &ConnectionId("held".to_owned()),
            &OperationId("items.list".to_owned()),
            &Value::Object(Vec::new()),
        );
        let elapsed = started.elapsed();
        match answer {
            Err(CliError::Failed(message)) => assert!(
                message.starts_with("upstream-protocol:") && message.contains("request bound"),
                "{message}"
            ),
            other => panic!("expected an upstream-protocol failure, got {other:?}"),
        }
        assert!(
            elapsed < Duration::from_secs(3),
            "answered after {elapsed:?}"
        );
        assert_eq!(invokes.load(Ordering::SeqCst), 1, "sent once");
    }

    /// The bounds the relay client runs on are `connectors-client`'s.
    #[test]
    fn the_bounds_are_five_and_twenty_seconds() {
        assert_eq!(CONNECT_BOUND, Duration::from_secs(5));
        assert_eq!(REQUEST_BOUND, Duration::from_secs(20));
    }
}
