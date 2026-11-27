//! The loopback observation listener. Only packaged assets and a scoped read are routed.
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
fn private_text_bounded(path: &Path, limit: u64) -> io::Result<String> {
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

fn respond(
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
fn error(w: &mut TcpStream, status: &str, code: &str) -> io::Result<()> {
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
    if !matches!(request.parse(&buf[..n]), Ok(httparse::Status::Complete(_))) {
        return error(&mut stream, "400 Bad Request", "invalid_request");
    }
    if request.method != Some("GET") {
        return error(&mut stream, "405 Method Not Allowed", "method_not_allowed");
    }
    let path = request.path.unwrap_or("");
    let (route, query) = path.split_once('?').unwrap_or((path, ""));
    match route {
        "/" | "/index.html" => {
            return respond(&mut stream, "200 OK", "text/html; charset=utf-8", HTML, "");
        }
        "/api/openapi.json" => {
            return respond(&mut stream, "200 OK", "application/json", OPENAPI, "");
        }
        _ => {}
    }
    let spec_id = route
        .strip_prefix("/api/agents/")
        .and_then(|part| part.strip_suffix("/spec"));
    if route != "/api/snapshot" && spec_id.is_none() {
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
    if let Some(id) = spec_id {
        if !matches!(viewer, Viewer::Operator) {
            return error(&mut stream, "403 Forbidden", "forbidden");
        }
        if !query.is_empty() {
            return error(&mut stream, "400 Bad Request", "invalid_request");
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
/// Look up only the registered state's saved revision in the private observation directory.
/// The validation and the response use the same immutable bytes even during a host restart.
fn agent_spec(scope: &Scope, id: &str) -> io::Result<Option<(String, String)>> {
    if !super::store::is_digest(id) {
        return Ok(None);
    }
    let directory = scope.root.join("agents");
    let gate = super::store::snapshot_lock(&directory)?;
    gate.lock()?;
    let manifest = directory.join(format!("{id}.json"));
    let metadata = match private_text_bounded(&manifest, 64 * 1024 * 1024) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    #[derive(serde::Deserialize)]
    struct RegisteredSpec {
        id: String,
        spec_revision: Option<String>,
    }
    let row: RegisteredSpec = serde_json::from_str(&metadata)?;
    if row.id != id {
        return Err(invalid("dashboard registration mismatch"));
    }
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
