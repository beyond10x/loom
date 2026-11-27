//! The loopback dashboard listener for packaged assets, scoped reads and operator actions.
use super::{Scope, digest, invalid, snapshot};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;

const HTML: &[u8] = include_bytes!("../../../../ui/dist/index.html");
const OPENAPI: &[u8] = include_bytes!("../../../../website/static/api/openapi/dashboard.json");
const MAX_HEAD: usize = 8192;
const MAX_CLIENTS: usize = 12;

/// A protected, regular file only, including after a configuration replacement.
pub(super) fn private_text(path: &Path) -> io::Result<String> {
    private_text_bounded(path, 65536)
}
pub(super) fn private_text_bounded(path: &Path, limit: u64) -> io::Result<String> {
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let meta = f.metadata()?;
    // SAFETY: geteuid reads the process's effective uid and takes no arguments.
    if !meta.is_file()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o077 != 0
        || meta.len() > limit
    {
        return Err(invalid(
            "dashboard viewer configuration must be a private regular file",
        ));
    }
    let mut text = String::new();
    f.take(limit + 1).read_to_string(&mut text)?;
    if text.len() as u64 > limit {
        return Err(invalid("dashboard viewer configuration is too large"));
    }
    Ok(text)
}

struct Slot(Arc<AtomicUsize>);
impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}

pub(super) fn accept(socket: &TcpListener, scope: &Arc<Scope>, clients: &Arc<AtomicUsize>) {
    while clients.load(Ordering::Acquire) < MAX_CLIENTS {
        let stream = match socket.accept() {
            Ok((s, _)) => s,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) => {
                eprintln!("b10x-loom: dashboard accept: {error}");
                break;
            }
        };
        clients.fetch_add(1, Ordering::Release);
        let slot = Slot(clients.clone());
        let scope = scope.clone();
        let _ = thread::Builder::new()
            .name("loom-observer-read".into())
            .spawn(move || {
                let _slot = slot;
                let _ = handle(&scope, stream);
            });
    }
}

pub(super) fn respond(
    w: &mut TcpStream,
    status: &str,
    kind: &str,
    body: &[u8],
    extra: &str,
) -> io::Result<()> {
    write!(
        w,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: private, no-store\r\nVary: Authorization\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'\r\n{extra}\r\n",
        body.len()
    )?;
    w.write_all(body)
}
pub(super) fn json_reply(w: &mut TcpStream, status: &str, value: &Value) -> io::Result<()> {
    respond(
        w,
        status,
        "application/json",
        &serde_json::to_vec(value)?,
        "",
    )
}

pub(super) fn error(w: &mut TcpStream, status: &str, code: &str) -> io::Result<()> {
    let headers = match code {
        "unauthorized" => "WWW-Authenticate: Bearer\r\n",
        "method_not_allowed" => "Allow: GET\r\n",
        _ => "",
    };
    respond(
        w,
        status,
        "application/json",
        format!("{{\"error\":\"{code}\"}}").as_bytes(),
        headers,
    )
}
enum Viewer {
    Operator,
    Scoped(Vec<String>),
}
fn authorize(scope: &Scope, authorization: &str) -> io::Result<Option<Viewer>> {
    let Some((scheme, token)) = authorization.split_once(' ') else {
        return Ok(None);
    };
    if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
        return Ok(None);
    }
    let hash = digest(token.as_bytes());
    if bool::from(hash.as_bytes().ct_eq(scope.token_hash.as_bytes())) {
        return Ok(Some(Viewer::Operator));
    }
    let list = match private_text(&scope.root.join("viewers.json")) {
        Ok(list) => list,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let scopes: BTreeMap<String, Vec<String>> = serde_json::from_str(&list)?;
    Ok(scopes.into_iter().find_map(|(key, agents)| {
        bool::from(hash.as_bytes().ct_eq(key.as_bytes())).then_some(Viewer::Scoped(agents))
    }))
}
// The browser sends a same-origin JSON fetch with an explicit bearer. Reject ambiguous
// framing and bound the entire read by one deadline, including time already spent on the head.
fn request_body(
    stream: &mut TcpStream,
    headers: &[httparse::Header<'_>],
    prefix: &[u8],
    deadline: Instant,
) -> io::Result<Option<Vec<u8>>> {
    let mut lengths = headers
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("Content-Length"));
    let length = lengths
        .next()
        .and_then(|h| std::str::from_utf8(h.value).ok())
        .and_then(|n| n.parse::<usize>().ok());
    let Some(length) = length.filter(|n| (1..=131072).contains(n)) else {
        return Ok(None);
    };
    if lengths.next().is_some()
        || prefix.len() > length
        || headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("Transfer-Encoding"))
    {
        return Ok(None);
    }
    let is_json = headers.iter().any(|h| {
        h.name.eq_ignore_ascii_case("Content-Type")
            && h.value
                .split(|b| *b == b';')
                .next()
                .is_some_and(|v| v.eq_ignore_ascii_case(b"application/json"))
    });
    if !is_json {
        return Ok(None);
    }
    let mut body = vec![0; length];
    body[..prefix.len()].copy_from_slice(prefix);
    let mut offset = prefix.len();
    while offset < length {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(None);
        }
        stream.set_read_timeout(Some(remaining))?;
        let got = stream.read(&mut body[offset..])?;
        if got == 0 {
            return Ok(None);
        }
        offset += got;
    }
    Ok(Some(body))
}
fn handle(scope: &Scope, mut stream: TcpStream) -> io::Result<()> {
    stream.set_write_timeout(Some(Duration::from_secs(4)))?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut buf = [0u8; MAX_HEAD];
    let mut n = 0;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() || n == MAX_HEAD {
            return error(&mut stream, "400 Bad Request", "invalid_request");
        }
        stream.set_read_timeout(Some(remaining))?;
        let read = stream.read(&mut buf[n..])?;
        if read == 0 {
            return Ok(());
        }
        n += read;
        if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let mut headers = [httparse::EMPTY_HEADER; 40];
    let mut request = httparse::Request::new(&mut headers);
    let Ok(httparse::Status::Complete(end)) = request.parse(&buf[..n]) else {
        return error(&mut stream, "400 Bad Request", "invalid_request");
    };
    let path = request.path.unwrap_or("");
    let (route, query) = path.split_once('?').unwrap_or((path, ""));
    let dispatch_id = route
        .strip_prefix("/api/agents/")
        .and_then(|s| s.strip_suffix("/dispatch"));
    let write = route == "/api/launch" || dispatch_id.is_some();
    let method = if write { "POST" } else { "GET" };
    if request.method != Some(method) {
        return respond(
            &mut stream,
            "405 Method Not Allowed",
            "application/json",
            br#"{"error":"method_not_allowed"}"#,
            &format!("Allow: {method}\r\n"),
        );
    }
    match route {
        "/" | "/index.html" => {
            return respond(&mut stream, "200 OK", "text/html; charset=utf-8", HTML, "");
        }
        "/api/openapi.json" => {
            return respond(&mut stream, "200 OK", "application/json", OPENAPI, "");
        }
        _ => {}
    }
    let detail = route
        .strip_prefix("/api/agents/")
        .and_then(|part| part.rsplit_once('/'))
        .filter(|(_, action)| matches!(*action, "spec" | "routing"));
    if !write && route != "/api/launch-templates" && route != "/api/snapshot" && detail.is_none() {
        return error(&mut stream, "404 Not Found", "not_found");
    }
    let mut auth = request
        .headers
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("Authorization"));
    let value = auth
        .next()
        .and_then(|h| std::str::from_utf8(h.value).ok())
        .unwrap_or("");
    if auth.next().is_some() {
        return error(&mut stream, "400 Bad Request", "invalid_request");
    }
    let viewer = match authorize(scope, value) {
        Ok(Some(v)) => v,
        Ok(None) => return error(&mut stream, "401 Unauthorized", "unauthorized"),
        Err(_) => {
            return error(
                &mut stream,
                "503 Service Unavailable",
                "viewer_configuration_unavailable",
            );
        }
    };
    if write || route == "/api/launch-templates" {
        if !matches!(viewer, Viewer::Operator) {
            return error(&mut stream, "403 Forbidden", "forbidden");
        }
        if !query.is_empty() {
            return error(&mut stream, "400 Bad Request", "invalid_request");
        }
        if !write {
            return super::launch::list(scope, &mut stream);
        }
        let Some(bytes) = request_body(&mut stream, request.headers, &buf[end..n], deadline)?
        else {
            return error(&mut stream, "400 Bad Request", "invalid_request");
        };
        if let Some(id) = dispatch_id {
            let mut credentials = request
                .headers
                .iter()
                .filter(|h| h.name.eq_ignore_ascii_case("X-Loom-Agent-Bearer"));
            let credential = credentials
                .next()
                .and_then(|h| std::str::from_utf8(h.value).ok())
                .unwrap_or("");
            if credentials.next().is_some() {
                return error(&mut stream, "400 Bad Request", "invalid_request");
            }
            return super::dispatch::handle(scope, id, credential, &bytes, &mut stream);
        }
        return super::launch::handle(scope, &bytes, &mut stream);
    }
    if let Some((id, action)) = detail {
        if !matches!(viewer, Viewer::Operator) {
            return error(&mut stream, "403 Forbidden", "forbidden");
        }
        if !query.is_empty() {
            return error(&mut stream, "400 Bad Request", "invalid_request");
        }
        if action == "routing" {
            return match registration(scope, id) {
                Ok(Some(row)) if row.routing.is_some() => respond(
                    &mut stream,
                    "200 OK",
                    "application/json",
                    &serde_json::to_vec(&row.routing)?,
                    "",
                ),
                Ok(_) => error(&mut stream, "404 Not Found", "not_found"),
                Err(_) => error(
                    &mut stream,
                    "503 Service Unavailable",
                    "routing_unavailable",
                ),
            };
        }
        return match agent_spec(scope, id) {
            Ok(Some((revision, source))) => respond(
                &mut stream,
                "200 OK",
                "application/yaml; charset=utf-8",
                source.as_bytes(),
                &format!("ETag: \"{revision}\"\r\n"),
            ),
            Ok(None) => error(&mut stream, "404 Not Found", "not_found"),
            Err(_) => error(&mut stream, "503 Service Unavailable", "spec_unavailable"),
        };
    }
    if matches!(&viewer, Viewer::Scoped(ids) if ids.is_empty()) {
        return error(&mut stream, "403 Forbidden", "forbidden");
    }
    let Some(filter) = snapshot::Filter::parse(query) else {
        return error(&mut stream, "400 Bad Request", "invalid_request");
    };
    let view = match &viewer {
        Viewer::Operator => None,
        Viewer::Scoped(agents) => Some(agents.as_slice()),
    };
    match snapshot::read(scope, view, &filter) {
        Ok(Some((cursor, body))) => render(&mut stream, request.headers, &cursor, body),
        Ok(None) => error(&mut stream, "404 Not Found", "not_found"),
        Err(_) => error(
            &mut stream,
            "503 Service Unavailable",
            "snapshot_unavailable",
        ),
    }
}
#[derive(serde::Deserialize)]
pub(super) struct Registration {
    pub(super) online: bool,
    pub(super) instance: String,
    pub(super) a2a_protocol: Option<String>,
    id: String,
    spec_revision: Option<String>,
    routing: Option<super::routing::Routing>,
}

pub(super) fn registration(scope: &Scope, id: &str) -> io::Result<Option<Registration>> {
    if !super::store::is_digest(id) {
        return Ok(None);
    }
    let manifest = scope.root.join("agents").join(format!("{id}.json"));
    let metadata = match private_text_bounded(&manifest, 64 * 1024 * 1024) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let row: Registration = serde_json::from_str(&metadata)?;
    if row.id != id {
        return Err(invalid("dashboard registration mismatch"));
    }
    Ok(Some(row))
}

/// Look up only the registered state's saved revision in the private observation directory.
fn agent_spec(scope: &Scope, id: &str) -> io::Result<Option<(String, String)>> {
    let directory = scope.root.join("agents");
    let gate = super::store::snapshot_lock(&directory)?;
    gate.lock()?;
    let Some(row) = registration(scope, id)? else {
        return Ok(None);
    };
    let manifest = directory.join(format!("{id}.json"));
    let Some(revision) = row.spec_revision else {
        return Ok(None);
    };
    if !super::store::is_digest(&revision) {
        return Ok(None);
    }
    let source = match private_text_bounded(
        &manifest.with_extension(format!("{revision}.yaml")),
        2 * 1024 * 1024,
    ) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    drop(gate);
    if digest(source.as_bytes()) != revision {
        return Err(invalid("dashboard spec digest mismatch"));
    }
    Ok(Some((revision, source)))
}

fn render(
    socket: &mut TcpStream,
    headers: &[httparse::Header<'_>],
    cursor: &str,
    body: Value,
) -> io::Result<()> {
    let etag = format!("\"{cursor}\"");
    let extra = format!("ETag: {etag}\r\n");
    let matches = headers
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("If-None-Match"))
        .filter_map(|h| std::str::from_utf8(h.value).ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .any(|tag| tag == "*" || tag.strip_prefix("W/").unwrap_or(tag) == etag);
    if matches {
        return respond(socket, "304 Not Modified", "application/json", &[], &extra);
    }
    respond(
        socket,
        "200 OK",
        "application/json",
        &serde_json::to_vec(&body)?,
        &extra,
    )
}
