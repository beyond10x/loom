//! The credential boundary of a credential-free agent pod.
//!
//! A `LoomAgent` with `spec.boundary` runs a sidecar beside the agent container, in the same
//! network namespace but as another user. The agent holds no credential and calls the boundary on
//! loopback; the boundary holds only kubelet-projected service-account tokens, one file per
//! upstream, and relays each admitted request to an in-cluster upstream with
//! `Authorization: Bearer <token>`. `b10x-loom agent boundary` runs it.
//!
//! One listener per declared upstream, each on `127.0.0.1` only ([`bind`] refuses any other
//! address):
//!
//! - [`MODEL_PORT`] (8401), the model pass-through: any method on a path under `/v1/`, the
//!   response streamed back. A path with an empty, `.` or `..` segment, a backslash, or an
//!   encoded `.`, `/` or `\` is refused `400`.
//! - [`CONNECTORS_PORT`] (8402), the Connectors wire: `GET /v1/describe` and
//!   `POST /v1alpha2/invoke`, the invoke body buffered up to [`MAX_INVOKE_BYTES`] (`413` past it)
//!   and admitted only when it is one JSON object whose top level names a declared `operation`
//!   once, as a string, and repeats no key (`403` otherwise). The admitted body is forwarded byte
//!   for byte.
//! - [`GIT_PORT`] (8403), the git smart-HTTP relay: `GET /<owner>/<repo>.git/info/refs` with
//!   exactly `?service=git-upload-pack` or `?service=git-receive-pack`, and
//!   `POST /<owner>/<repo>.git/git-upload-pack` or `git-receive-pack`, for a declared
//!   `<owner>/<repo>` only.
//!
//! Anything else is refused locally (`404` for an undeclared route or repository, `405` for
//! CONNECT and TRACE, `400` for an absolute- or authority-form target) and never reaches the
//! upstream.
//!
//! Every relayed request reads the token file again ([`MAX_TOKEN_BYTES`] at most, ASCII
//! whitespace trimmed), so a kubelet rotation is picked up on the next request; an absent, empty
//! or malformed token answers `503` and sends nothing. The agent's credential headers
//! (`authorization`, `proxy-authorization`, `x-api-key`, `cookie`) and hop-by-hop headers are
//! dropped, `host` is the upstream's, and hop-by-hop headers are stripped from the response. The
//! git relay passes only `content-type`, `content-encoding`, `git-protocol` and `accept`.
//! Bodies stream both ways except the buffered invoke. The token is never logged and never
//! returned; each request logs one line: service, method, path, status.
//!
//! Upstreams are plain `http://` URLs ([`Upstream`]); TLS to the upstream is not implemented, so
//! an `https` upstream is refused.

#![forbid(unsafe_code)]

mod invoke;
mod route;
mod upstream;

pub use upstream::{Upstream, UpstreamError};

use std::collections::BTreeSet;
use std::convert::Infallible;
use std::fmt;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt as _, Full};
use hyper::body::{Body as _, Bytes, Incoming};
use hyper::header::{self, HeaderMap, HeaderValue};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode, Uri, Version};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::io::AsyncReadExt as _;
use tokio::net::{TcpListener, TcpStream};

use route::{Headers, Refusal};

/// The model pass-through's port.
pub const MODEL_PORT: u16 = 8401;
/// The Connectors wire's port.
pub const CONNECTORS_PORT: u16 = 8402;
/// The git smart-HTTP relay's port.
pub const GIT_PORT: u16 = 8403;
/// The most bytes a token file may hold.
pub const MAX_TOKEN_BYTES: u64 = 64 * 1024;
/// The largest invoke body the Connectors wire admits: Connectors' own request limit.
pub const MAX_INVOKE_BYTES: usize = 64 * 1024;
/// How long a relay waits for the upstream to accept a connection. A response, once it has
/// started, has no time limit: a model stream or a pack may run long.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the agent has to send a request's head.
const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(30);
/// After an invoke body passed the limit, how much more of it is read and discarded before the
/// `413`, so the client reads the answer instead of a reset.
const DRAIN_BYTES: usize = 1024 * 1024;
const DRAIN_TIME: Duration = Duration::from_secs(2);

/// The headers the agent may hold a credential in; none reaches an upstream.
const CREDENTIAL_HEADERS: [&str; 4] = [
    "authorization",
    "proxy-authorization",
    "x-api-key",
    "cookie",
];
/// Hop-by-hop headers (RFC 9110 § 7.6.1 and the legacy ones), dropped both ways.
const HOP_BY_HOP: [&str; 9] = [
    "connection",
    "keep-alive",
    "proxy-connection",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];
/// Headers the relay sets itself or that frame the agent's own connection.
const FRAMING: [&str; 3] = ["host", "content-length", "expect"];
/// The only request headers the git relay passes.
const GIT_HEADERS: [&str; 4] = ["content-type", "content-encoding", "git-protocol", "accept"];

/// The body of every response and every upstream request.
type RelayBody = BoxBody<Bytes, hyper::Error>;

/// Where each request's log line goes.
pub type Log = Arc<dyn Fn(&str) + Send + Sync>;

#[derive(Debug, Clone)]
pub(crate) enum Service {
    Model,
    Connectors { operations: BTreeSet<String> },
    Git { repositories: BTreeSet<String> },
}

/// Why a relay's declared operations or repositories were refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationError {
    /// The Connectors relay declares no operation.
    NoOperation,
    /// An operation id is empty, longer than 256 bytes, or holds a byte that is not visible
    /// ASCII (or is `"` or `\`).
    Operation,
    /// The git relay declares no repository.
    NoRepository,
    /// A repository is not `<owner>/<repo>`, each part `[A-Za-z0-9._-]+` and neither `.` nor
    /// `..`, the repository named without `.git`.
    Repository,
}

impl fmt::Display for DeclarationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DeclarationError::NoOperation => "no operation is declared",
            DeclarationError::Operation => {
                "an operation id must be 1 to 256 visible ASCII characters other than '\"' and '\\'"
            }
            DeclarationError::NoRepository => "no repository is declared",
            DeclarationError::Repository => {
                "a repository must be <owner>/<repo>, each part letters, digits, '.', '_' or '-' and neither '.' nor '..', named without .git"
            }
        })
    }
}

impl std::error::Error for DeclarationError {}

/// One relay: its service, its upstream and the file its token is read from.
#[derive(Debug, Clone)]
pub struct Relay {
    service: Service,
    upstream: Upstream,
    token: PathBuf,
}

impl Relay {
    /// The model pass-through.
    pub fn model(upstream: Upstream, token: impl Into<PathBuf>) -> Self {
        Relay {
            service: Service::Model,
            upstream,
            token: token.into(),
        }
    }

    /// The Connectors wire, admitting an invoke of the declared `operations` only.
    pub fn connectors<I, S>(
        upstream: Upstream,
        token: impl Into<PathBuf>,
        operations: I,
    ) -> Result<Self, DeclarationError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let operations: BTreeSet<String> = operations.into_iter().map(Into::into).collect();
        if operations.is_empty() {
            return Err(DeclarationError::NoOperation);
        }
        if !operations.iter().all(|operation| operation_id(operation)) {
            return Err(DeclarationError::Operation);
        }
        Ok(Relay {
            service: Service::Connectors { operations },
            upstream,
            token: token.into(),
        })
    }

    /// The git smart-HTTP relay for the declared `<owner>/<repo>` `repositories`.
    pub fn git<I, S>(
        upstream: Upstream,
        token: impl Into<PathBuf>,
        repositories: I,
    ) -> Result<Self, DeclarationError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let repositories: BTreeSet<String> = repositories.into_iter().map(Into::into).collect();
        if repositories.is_empty() {
            return Err(DeclarationError::NoRepository);
        }
        if !repositories
            .iter()
            .all(|repository| repository_name(repository))
        {
            return Err(DeclarationError::Repository);
        }
        Ok(Relay {
            service: Service::Git { repositories },
            upstream,
            token: token.into(),
        })
    }

    /// The service's name in each log line: `model`, `connectors` or `git`.
    pub fn service(&self) -> &'static str {
        match self.service {
            Service::Model => "model",
            Service::Connectors { .. } => "connectors",
            Service::Git { .. } => "git",
        }
    }

    /// The port the command line binds for this service.
    pub fn port(&self) -> u16 {
        match self.service {
            Service::Model => MODEL_PORT,
            Service::Connectors { .. } => CONNECTORS_PORT,
            Service::Git { .. } => GIT_PORT,
        }
    }
}

fn operation_id(operation: &str) -> bool {
    (1..=256).contains(&operation.len())
        && operation
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'"' && byte != b'\\')
}

fn repository_name(repository: &str) -> bool {
    let Some((owner, name)) = repository.split_once('/') else {
        return false;
    };
    route::repository_part(owner) && route::repository_part(name) && !name.ends_with(".git")
}

/// Why a relay could not listen.
#[derive(Debug)]
pub enum BindError {
    /// The address is not a loopback address; the boundary listens on loopback only.
    NotLoopback,
    /// The address could not be bound.
    Io(std::io::Error),
}

impl fmt::Display for BindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BindError::NotLoopback => {
                f.write_str("the boundary listens on a loopback address only")
            }
            BindError::Io(error) => write!(f, "cannot listen: {error}"),
        }
    }
}

impl std::error::Error for BindError {}

/// Binds `relay` on `address`, which must be a loopback address.
pub async fn bind(address: SocketAddr, relay: Relay) -> Result<Listener, BindError> {
    if !address.ip().is_loopback() {
        return Err(BindError::NotLoopback);
    }
    let listener = TcpListener::bind(address).await.map_err(BindError::Io)?;
    Ok(Listener {
        listener,
        relay: Arc::new(relay),
        log: Arc::new(|line: &str| eprintln!("b10x-loom boundary: {line}")),
    })
}

/// A bound relay.
pub struct Listener {
    listener: TcpListener,
    relay: Arc<Relay>,
    log: Log,
}

impl Listener {
    /// The address the relay is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.listener
            .local_addr()
            .expect("a bound listener has an address")
    }

    /// Sends each request's log line to `log` instead of standard error.
    pub fn with_log(mut self, log: Log) -> Self {
        self.log = log;
        self
    }

    /// Serves the relay until the future is dropped.
    pub async fn serve(self) {
        let Listener {
            listener,
            relay,
            log,
        } = self;
        loop {
            let stream = match listener.accept().await {
                Ok((stream, _)) => stream,
                Err(_) => {
                    // Out of descriptors, for instance: wait instead of spinning.
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            };
            let relay = Arc::clone(&relay);
            let log = Arc::clone(&log);
            tokio::spawn(async move {
                let service = service_fn(move |request: Request<Incoming>| {
                    let relay = Arc::clone(&relay);
                    let log = Arc::clone(&log);
                    async move { Ok::<_, Infallible>(handle(&relay, &log, request).await) }
                });
                let _ = hyper::server::conn::http1::Builder::new()
                    .timer(TokioTimer::new())
                    .header_read_timeout(HEADER_READ_TIMEOUT)
                    .serve_connection(TokioIo::new(stream), service)
                    .await;
            });
        }
    }
}

/// Exits `Ok` when every `127.0.0.1:<port>` of `ports` accepts a TCP connection within
/// `timeout`, else names the first port that does not.
pub fn ready(ports: &[u16], timeout: Duration) -> Result<(), u16> {
    for &port in ports {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        if std::net::TcpStream::connect_timeout(&address, timeout).is_err() {
            return Err(port);
        }
    }
    Ok(())
}

/// Relays one request and logs its line.
async fn handle(relay: &Relay, log: &Log, request: Request<Incoming>) -> Response<RelayBody> {
    let method = request.method().clone();
    let path = printable(request.uri().path());
    let response = relay_request(relay, request).await;
    log(&format!(
        "{} {} {} {}",
        relay.service(),
        printable(method.as_str()),
        path,
        response.status().as_u16()
    ));
    response
}

const TOKEN_UNAVAILABLE: Refusal = Refusal {
    status: StatusCode::SERVICE_UNAVAILABLE,
    error: "token_unavailable",
    message: "the boundary has no usable token for this upstream",
};
const TOO_LARGE: Refusal = Refusal {
    status: StatusCode::PAYLOAD_TOO_LARGE,
    error: "body_too_large",
    message: "the invoke body exceeds 65536 bytes",
};
const BODY_UNREADABLE: Refusal = Refusal {
    status: StatusCode::BAD_REQUEST,
    error: "body_unreadable",
    message: "the request body could not be read",
};
const UNREACHABLE: Refusal = Refusal {
    status: StatusCode::BAD_GATEWAY,
    error: "upstream_unreachable",
    message: "the upstream cannot be reached",
};
const CONNECT_TIMED_OUT: Refusal = Refusal {
    status: StatusCode::GATEWAY_TIMEOUT,
    error: "upstream_timeout",
    message: "the upstream did not accept a connection in time",
};
const UPSTREAM_FAILED: Refusal = Refusal {
    status: StatusCode::BAD_GATEWAY,
    error: "upstream_failed",
    message: "the upstream did not answer",
};

async fn relay_request(relay: &Relay, request: Request<Incoming>) -> Response<RelayBody> {
    let forward = match route::route(&relay.service, request.method(), request.uri()) {
        Ok(forward) => forward,
        Err(refusal) => return refused(refusal),
    };
    let (parts, body) = request.into_parts();
    let body = match (&relay.service, forward.invoke) {
        (Service::Connectors { operations }, true) => match admitted_invoke(body, operations).await
        {
            Ok(bytes) => full(bytes),
            Err(refusal) => return refused(refusal),
        },
        _ => body.boxed(),
    };
    let Some(token) = read_token(&relay.token).await else {
        return refused(TOKEN_UNAVAILABLE);
    };
    let Ok(uri) = format!("{}{}", relay.upstream.base_path(), forward.target).parse::<Uri>() else {
        return refused(BODY_UNREADABLE);
    };
    let mut upstream_request = Request::new(body);
    *upstream_request.method_mut() = parts.method;
    *upstream_request.uri_mut() = uri;
    *upstream_request.version_mut() = Version::HTTP_11;
    *upstream_request.headers_mut() =
        forwarded_headers(&parts.headers, forward.headers, &relay.upstream, token);
    send(&relay.upstream, upstream_request).await
}

/// Buffers an invoke body up to [`MAX_INVOKE_BYTES`] and admits it by its operation.
async fn admitted_invoke(
    mut body: Incoming,
    operations: &BTreeSet<String>,
) -> Result<Bytes, Refusal> {
    let mut buffer = Vec::new();
    let mut over = body.size_hint().lower() > MAX_INVOKE_BYTES as u64;
    while !over {
        match body.frame().await {
            None => break,
            Some(Err(_)) => return Err(BODY_UNREADABLE),
            Some(Ok(frame)) => {
                if let Ok(data) = frame.into_data() {
                    over = buffer.len() + data.len() > MAX_INVOKE_BYTES;
                    if !over {
                        buffer.extend_from_slice(&data);
                    }
                }
            }
        }
    }
    if over {
        drain(body).await;
        return Err(TOO_LARGE);
    }
    invoke::admit(&buffer, operations).map_err(|refusal| Refusal {
        status: StatusCode::FORBIDDEN,
        error: "operation_refused",
        message: refusal.message(),
    })?;
    Ok(Bytes::from(buffer))
}

/// Reads and discards the rest of a refused body, within a bound.
async fn drain(mut body: Incoming) {
    let _ = tokio::time::timeout(DRAIN_TIME, async {
        let mut drained = 0;
        while let Some(Ok(frame)) = body.frame().await {
            if let Ok(data) = frame.into_data() {
                drained += data.len();
                if drained > DRAIN_BYTES {
                    break;
                }
            }
        }
    })
    .await;
}

/// The `Authorization` value for the token in `path`, read now: at most [`MAX_TOKEN_BYTES`],
/// ASCII whitespace trimmed, visible ASCII only. `None` when there is no usable token.
async fn read_token(path: &Path) -> Option<HeaderValue> {
    let file = tokio::fs::File::open(path).await.ok()?;
    if !file.metadata().await.ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_TOKEN_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .ok()?;
    if bytes.len() as u64 > MAX_TOKEN_BYTES {
        return None;
    }
    let token = bytes.trim_ascii();
    if token.is_empty() || !token.iter().all(u8::is_ascii_graphic) {
        return None;
    }
    let mut value = Vec::with_capacity(7 + token.len());
    value.extend_from_slice(b"Bearer ");
    value.extend_from_slice(token);
    let mut header = HeaderValue::from_bytes(&value).ok()?;
    header.set_sensitive(true);
    Some(header)
}

/// The names a `connection` header lists, lowercased.
fn connection_listed(headers: &HeaderMap) -> Vec<String> {
    headers
        .get_all(header::CONNECTION)
        .iter()
        .flat_map(|value| {
            String::from_utf8_lossy(value.as_bytes())
                .split(',')
                .map(|name| name.trim().to_ascii_lowercase())
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The headers the upstream receives: the agent's that `policy` passes, then `host` and the
/// token.
fn forwarded_headers(
    incoming: &HeaderMap,
    policy: Headers,
    upstream: &Upstream,
    token: HeaderValue,
) -> HeaderMap {
    let listed = connection_listed(incoming);
    let mut out = HeaderMap::new();
    for (name, value) in incoming {
        let name_str = name.as_str();
        let passes = match policy {
            Headers::AllButDropped => {
                !CREDENTIAL_HEADERS.contains(&name_str)
                    && !HOP_BY_HOP.contains(&name_str)
                    && !FRAMING.contains(&name_str)
            }
            Headers::Git => GIT_HEADERS.contains(&name_str),
        };
        if passes && !listed.iter().any(|listed| listed == name_str) {
            out.append(name.clone(), value.clone());
        }
    }
    out.insert(header::HOST, upstream.host_header().clone());
    out.insert(header::AUTHORIZATION, token);
    out
}

/// Removes hop-by-hop headers, and those `connection` lists, from a response.
fn strip_hop_by_hop(headers: &mut HeaderMap) {
    let listed = connection_listed(headers);
    for name in HOP_BY_HOP {
        headers.remove(name);
    }
    for name in listed {
        headers.remove(name.as_str());
    }
}

/// Sends `request` to `upstream` on a new connection and streams its response back.
async fn send(upstream: &Upstream, request: Request<RelayBody>) -> Response<RelayBody> {
    let connecting = TcpStream::connect((upstream.connect_host(), upstream.port()));
    let stream = match tokio::time::timeout(CONNECT_TIMEOUT, connecting).await {
        Err(_) => return refused(CONNECT_TIMED_OUT),
        Ok(Err(_)) => return refused(UNREACHABLE),
        Ok(Ok(stream)) => stream,
    };
    let _ = stream.set_nodelay(true);
    let Ok((mut sender, connection)) =
        hyper::client::conn::http1::handshake::<_, RelayBody>(TokioIo::new(stream)).await
    else {
        return refused(UNREACHABLE);
    };
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let Ok(response) = sender.send_request(request).await else {
        return refused(UPSTREAM_FAILED);
    };
    let (mut parts, body) = response.into_parts();
    if parts.status.is_informational() {
        // No upgrade was asked for (`upgrade` never reaches the upstream).
        return refused(UPSTREAM_FAILED);
    }
    strip_hop_by_hop(&mut parts.headers);
    Response::from_parts(parts, body.boxed())
}

fn full(bytes: Bytes) -> RelayBody {
    Full::new(bytes).map_err(|never| match never {}).boxed()
}

/// A local answer: the refusal's status and a small JSON error of fixed text.
fn refused(refusal: Refusal) -> Response<RelayBody> {
    let body = serde_json::json!({"error": refusal.error, "message": refusal.message});
    let mut response = Response::new(full(Bytes::from(body.to_string())));
    *response.status_mut() = refusal.status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
}

/// `text` for a log line: visible ASCII kept, anything else escaped, at most 512 characters.
fn printable(text: &str) -> String {
    let mut out = String::new();
    for character in text.chars().take(512) {
        if character.is_ascii_graphic() {
            out.push(character);
        } else {
            out.push_str(&format!("\\u{{{:x}}}", u32::from(character)));
        }
    }
    if text.chars().count() > 512 {
        out.push_str("...");
    }
    out
}
