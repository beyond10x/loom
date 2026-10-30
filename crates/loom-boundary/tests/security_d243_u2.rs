//! Security review of `story:boundary-loopback-relays` (wave 2026-10-30-w1, unit u2): integrity
//! checks of the boundary's own contract that `tests/relays.rs` does not measure.
//!
//! Each upstream here is a raw TCP recorder on `127.0.0.1:0`: it keeps the exact bytes the
//! boundary wrote, so a check can say what reached the upstream on the wire (trailers, repeated
//! header lines), not only what a parser kept. Every agent request is written as raw bytes too.
//! The token files are fixtures this file writes under its own `CARGO_TARGET_TMPDIR`; nothing
//! reads a real credential or reaches beyond loopback.

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use b10x_loom_boundary::{Relay, Upstream, bind};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};

const MODEL_TOKEN: &str = "fixture-security-token-model";
const CONNECTORS_TOKEN: &str = "fixture-security-token-connectors";
const GIT_TOKEN: &str = "fixture-security-token-git";
const WAIT: Duration = Duration::from_secs(10);
/// How long a recorder waits for more bytes once a request head is complete.
const SETTLE: Duration = Duration::from_millis(300);
const OK_ANSWER: &str = "HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok";

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("security-d243-u2")
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean scratch");
    }
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn token_file(dir: &Path, file: &str, token: &str) -> PathBuf {
    let path = dir.join(file);
    fs::write(&path, token).expect("write fixture token");
    path
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn occurrences(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

/// The values of every `name:` line in the head of a raw HTTP message, the name matched without
/// regard to case.
fn header_lines(raw: &str, name: &str) -> Vec<String> {
    let head = raw.split("\r\n\r\n").next().unwrap_or("");
    head.split("\r\n")
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .filter(|(line_name, _)| line_name.trim().eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim().to_owned())
        .collect()
}

fn status(raw: &str) -> u16 {
    raw.strip_prefix("HTTP/1.1 ")
        .and_then(|rest| rest.get(..3))
        .unwrap_or_else(|| panic!("no status line in {raw:?}"))
        .parse()
        .expect("a numeric status")
}

/// An upstream that records the raw bytes of each request and answers each with `answer`.
struct RawUpstream {
    addr: SocketAddr,
    received: Arc<Mutex<Vec<String>>>,
}

impl RawUpstream {
    async fn start(answer: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind upstream");
        let addr = listener.local_addr().expect("upstream address");
        let received = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&received);
        let answer = Arc::new(answer);
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    continue;
                };
                let record = Arc::clone(&record);
                let answer = Arc::clone(&answer);
                tokio::spawn(async move {
                    let bytes = read_request(&mut stream).await;
                    record
                        .lock()
                        .expect("record")
                        .push(String::from_utf8_lossy(&bytes).into_owned());
                    let _ = stream.write_all(answer.as_bytes()).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        RawUpstream { addr, received }
    }

    fn received(&self) -> Vec<String> {
        self.received.lock().expect("record").clone()
    }
}

/// Reads a request: its head, then whatever follows until the sender pauses for [`SETTLE`].
async fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let idle = if contains(&bytes, b"\r\n\r\n") {
            SETTLE
        } else {
            WAIT
        };
        match tokio::time::timeout(idle, stream.read(&mut chunk)).await {
            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => return bytes,
            Ok(Ok(read)) => bytes.extend_from_slice(&chunk[..read]),
        }
    }
}

/// A running boundary listener and the log lines it wrote.
struct Boundary {
    addr: SocketAddr,
    logs: Arc<Mutex<Vec<String>>>,
}

impl Boundary {
    async fn start(relay: Relay) -> Self {
        let listener = bind(SocketAddr::from(([127, 0, 0, 1], 0)), relay)
            .await
            .expect("the boundary binds loopback");
        let addr = listener.local_addr();
        let logs = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&logs);
        let listener = listener.with_log(Arc::new(move |line: &str| {
            sink.lock().expect("logs").push(line.to_owned());
        }));
        tokio::spawn(listener.serve());
        Boundary { addr, logs }
    }

    fn logs(&self) -> Vec<String> {
        self.logs.lock().expect("logs").clone()
    }

    /// Writes `request` as the agent would and reads the whole answer.
    async fn raw(&self, request: &[u8]) -> String {
        let mut stream = TcpStream::connect(self.addr).await.expect("connect");
        stream.write_all(request).await.expect("write request");
        let mut answer = Vec::new();
        tokio::time::timeout(WAIT, stream.read_to_end(&mut answer))
            .await
            .expect("the boundary answers in time")
            .expect("read the answer");
        String::from_utf8_lossy(&answer).into_owned()
    }
}

/// `body` as one chunk, then the last chunk carrying `trailers` (each line ended by CRLF).
fn chunked(body: &[u8], trailers: &str) -> Vec<u8> {
    let mut out = format!("{:x}\r\n", body.len()).into_bytes();
    out.extend_from_slice(body);
    out.extend_from_slice(b"\r\n0\r\n");
    out.extend_from_slice(trailers.as_bytes());
    out.extend_from_slice(b"\r\n");
    out
}

/// A relay's recorder upstream and the boundary listener in front of it.
type Relayed = (RawUpstream, Boundary);

/// One relay of each kind, each on its own recorder with its own fixture token.
struct Relays {
    model: Relayed,
    connectors: Relayed,
    git: Relayed,
}

async fn relays(name: &str, answer: impl Fn() -> String) -> Relays {
    let dir = scratch(name);
    let model_upstream = RawUpstream::start(answer()).await;
    let connectors_upstream = RawUpstream::start(answer()).await;
    let git_upstream = RawUpstream::start(answer()).await;
    let model = Boundary::start(Relay::model(
        Upstream::parse(&format!("http://{}", model_upstream.addr)).expect("model upstream"),
        token_file(&dir, "model-token", MODEL_TOKEN),
    ))
    .await;
    let connectors = Boundary::start(
        Relay::connectors(
            Upstream::parse(&format!("http://{}", connectors_upstream.addr))
                .expect("connectors upstream"),
            token_file(&dir, "connectors-token", CONNECTORS_TOKEN),
            ["pipeline.get"],
        )
        .expect("declared operation"),
    )
    .await;
    let git = Boundary::start(
        Relay::git(
            Upstream::parse(&format!("http://{}/git", git_upstream.addr)).expect("git upstream"),
            token_file(&dir, "git-token", GIT_TOKEN),
            ["acme/widgets"],
        )
        .expect("declared repository"),
    )
    .await;
    Relays {
        model: (model_upstream, model),
        connectors: (connectors_upstream, connectors),
        git: (git_upstream, git),
    }
}

const INVOKE: &str = r#"{"operation":"pipeline.get","input":{}}"#;

fn assert_no_token_in_logs(boundary: &Boundary) {
    for line in boundary.logs() {
        for token in [MODEL_TOKEN, CONNECTORS_TOKEN, GIT_TOKEN] {
            assert!(!line.contains(token), "the log holds a token: {line}");
        }
    }
}

/// The boundary relays an upstream's redirect to the agent as it is: it never follows it, so
/// no request (and no token) reaches the redirect's target, and the answer holds no token.
#[tokio::test]
async fn a_redirect_from_the_upstream_reaches_the_agent_unfollowed_and_without_the_token() {
    let elsewhere = RawUpstream::start(OK_ANSWER.to_owned()).await;
    let location = format!("http://{}/v1/messages", elsewhere.addr);
    let redirect = {
        let location = location.clone();
        move || {
            format!(
                "HTTP/1.1 307 Temporary Redirect\r\nlocation: {location}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            )
        }
    };
    let relays = relays("redirect", redirect).await;
    let requests: [(&str, &Relayed, String); 3] = [
        (
            "model",
            &relays.model,
            "POST /v1/messages HTTP/1.1\r\nhost: x\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}".to_owned(),
        ),
        (
            "connectors",
            &relays.connectors,
            format!(
                "POST /v1alpha2/invoke HTTP/1.1\r\nhost: x\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{INVOKE}",
                INVOKE.len()
            ),
        ),
        (
            "git",
            &relays.git,
            "GET /acme/widgets.git/info/refs?service=git-upload-pack HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n".to_owned(),
        ),
    ];
    for (service, (upstream, boundary), request) in requests {
        let answer = boundary.raw(request.as_bytes()).await;
        assert_eq!(status(&answer), 307, "{service}: {answer}");
        assert_eq!(
            header_lines(&answer, "location"),
            vec![location.clone()],
            "{service}"
        );
        for token in [MODEL_TOKEN, CONNECTORS_TOKEN, GIT_TOKEN] {
            assert!(
                !answer.contains(token),
                "{service}: the answer holds a token"
            );
        }
        assert_eq!(upstream.received().len(), 1, "{service}");
        assert_no_token_in_logs(boundary);
    }
    tokio::time::sleep(SETTLE * 2).await;
    assert!(
        elsewhere.received().is_empty(),
        "the boundary followed a redirect: {:?}",
        elsewhere.received()
    );
}

/// Trailer fields the agent sends after a chunked body are not request headers the credential
/// filter sees; none of them, credential-named or not, reaches the upstream.
#[tokio::test]
async fn agent_request_trailers_never_reach_the_upstream() {
    let relays = relays("trailers", || OK_ANSWER.to_owned()).await;
    let trailers = "x-api-key: agent-trailer-key\r\ncookie: agent-trailer-cookie\r\nproxy-authorization: Basic agent-trailer-proxy\r\nx-agent-trailer: agent-trailer-value\r\n";
    let head = |line: &str| {
        format!(
            "{line} HTTP/1.1\r\nhost: x\r\ncontent-type: application/json\r\ntransfer-encoding: chunked\r\ntrailer: x-api-key, cookie, proxy-authorization, x-agent-trailer\r\nte: trailers\r\nconnection: close\r\n\r\n"
        )
        .into_bytes()
    };
    // The recorder keeps trailers when they are sent: written to it directly, they are there.
    // hyper's client would send every field a request's `trailer` header names except a short
    // list (`authorization`, `host`, `content-*`, ...), so `x-api-key`, `cookie` and
    // `proxy-authorization` are held back only because the boundary drops `trailer`.
    let direct = RawUpstream::start(OK_ANSWER.to_owned()).await;
    let mut request = head("POST /v1/messages");
    request.extend_from_slice(&chunked(b"{}", trailers));
    let answer = Boundary {
        addr: direct.addr,
        logs: Arc::default(),
    }
    .raw(&request)
    .await;
    assert_eq!(status(&answer), 200);
    assert!(direct.received()[0].contains("x-api-key: agent-trailer-key"));

    let cases: [(&str, &Relayed, &str, &[u8], &str); 3] = [
        (
            "model",
            &relays.model,
            "POST /v1/messages",
            b"{\"model\":\"m\"}",
            MODEL_TOKEN,
        ),
        (
            "connectors",
            &relays.connectors,
            "POST /v1alpha2/invoke",
            INVOKE.as_bytes(),
            CONNECTORS_TOKEN,
        ),
        (
            "git",
            &relays.git,
            "POST /acme/widgets.git/git-upload-pack",
            b"0000",
            GIT_TOKEN,
        ),
    ];
    for (service, (upstream, boundary), line, body, token) in cases {
        let mut request = head(line);
        request.extend_from_slice(&chunked(body, trailers));
        let answer = boundary.raw(&request).await;
        assert_eq!(status(&answer), 200, "{service}: {answer}");
        let received = upstream.received();
        assert_eq!(received.len(), 1, "{service}");
        let raw = &received[0];
        assert!(
            contains(raw.as_bytes(), body),
            "{service}: the body did not arrive: {raw:?}"
        );
        assert!(
            !raw.contains("agent-trailer"),
            "{service}: an agent trailer reached the upstream: {raw:?}"
        );
        assert!(
            header_lines(raw, "trailer").is_empty(),
            "{service}: the agent's trailer declaration reached the upstream: {raw:?}"
        );
        assert_eq!(occurrences(raw, token), 1, "{service}: {raw:?}");
        assert_no_token_in_logs(boundary);
    }
}

/// A `connection` header naming `authorization` or `host` drops the agent's own lines only: the
/// upstream still receives exactly one `authorization` (the token) and exactly one `host` (its
/// own), and nothing the agent put in either.
#[tokio::test]
async fn the_agents_connection_header_cannot_remove_the_token_or_the_upstream_host() {
    let relays = relays("connection-listed", || OK_ANSWER.to_owned()).await;
    let cases: [(&str, &Relayed, &str, &str, String); 3] = [
        (
            "model",
            &relays.model,
            "GET /v1/models",
            MODEL_TOKEN,
            relays.model.0.addr.to_string(),
        ),
        (
            "connectors",
            &relays.connectors,
            "GET /v1/describe",
            CONNECTORS_TOKEN,
            relays.connectors.0.addr.to_string(),
        ),
        (
            "git",
            &relays.git,
            "GET /acme/widgets.git/info/refs?service=git-upload-pack",
            GIT_TOKEN,
            relays.git.0.addr.to_string(),
        ),
    ];
    for (service, (upstream, boundary), line, token, host) in cases {
        let request = format!(
            "{line} HTTP/1.1\r\nhost: agent-chosen.example\r\nauthorization: Bearer agent-held-value\r\nconnection: authorization, host, close\r\n\r\n"
        );
        let answer = boundary.raw(request.as_bytes()).await;
        assert_eq!(status(&answer), 200, "{service}: {answer}");
        let received = upstream.received();
        assert_eq!(received.len(), 1, "{service}");
        let raw = &received[0];
        assert_eq!(
            header_lines(raw, "authorization"),
            vec![format!("Bearer {token}")],
            "{service}: {raw:?}"
        );
        assert_eq!(header_lines(raw, "host"), vec![host], "{service}: {raw:?}");
        for agent_value in ["agent-held-value", "agent-chosen.example"] {
            assert!(
                !raw.contains(agent_value),
                "{service}: {agent_value} reached the upstream: {raw:?}"
            );
        }
        assert!(
            !answer.contains(token),
            "{service}: the answer holds the token"
        );
    }
}

/// Each relay sends its own token, once, to its own upstream and to no other: the model token
/// never reaches Connectors or git, and so on.
#[tokio::test]
async fn each_relay_sends_only_its_own_token_to_only_its_own_upstream() {
    let relays = relays("token-per-relay", || OK_ANSWER.to_owned()).await;
    let model = relays
        .model
        .1
        .raw(b"POST /v1/messages HTTP/1.1\r\nhost: x\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}")
        .await;
    let connectors = relays
        .connectors
        .1
        .raw(
            format!(
                "POST /v1alpha2/invoke HTTP/1.1\r\nhost: x\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{INVOKE}",
                INVOKE.len()
            )
            .as_bytes(),
        )
        .await;
    let git = relays
        .git
        .1
        .raw(b"POST /acme/widgets.git/git-receive-pack HTTP/1.1\r\nhost: x\r\ncontent-length: 4\r\nconnection: close\r\n\r\n0000")
        .await;
    for (service, answer) in [
        ("model", &model),
        ("connectors", &connectors),
        ("git", &git),
    ] {
        assert_eq!(status(answer), 200, "{service}: {answer}");
    }
    for (service, upstream, own) in [
        ("model", &relays.model.0, MODEL_TOKEN),
        ("connectors", &relays.connectors.0, CONNECTORS_TOKEN),
        ("git", &relays.git.0, GIT_TOKEN),
    ] {
        let received = upstream.received();
        assert_eq!(received.len(), 1, "{service}");
        let raw = &received[0];
        for token in [MODEL_TOKEN, CONNECTORS_TOKEN, GIT_TOKEN] {
            let expected = usize::from(token == own);
            assert_eq!(
                occurrences(raw, token),
                expected,
                "{service} upstream, token {token}: {raw:?}"
            );
        }
    }
    for boundary in [&relays.model.1, &relays.connectors.1, &relays.git.1] {
        assert_no_token_in_logs(boundary);
    }
}

/// `TRACK`, the TRACE twin the route refuses beside TRACE (it would echo the request and the
/// token with it), answers 405 and never reaches the upstream.
#[tokio::test]
async fn track_is_refused_like_trace_and_never_reaches_the_upstream() {
    let relays = relays("track", || OK_ANSWER.to_owned()).await;
    for (service, (upstream, boundary), target) in [
        ("model", &relays.model, "/v1/models"),
        ("connectors", &relays.connectors, "/v1/describe"),
        (
            "git",
            &relays.git,
            "/acme/widgets.git/info/refs?service=git-upload-pack",
        ),
    ] {
        for method in ["TRACK", "TRACE"] {
            let answer = boundary
                .raw(
                    format!("{method} {target} HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n")
                        .as_bytes(),
                )
                .await;
            assert_eq!(status(&answer), 405, "{service} {method}: {answer}");
        }
        tokio::time::sleep(SETTLE).await;
        assert!(
            upstream.received().is_empty(),
            "{service}: TRACK or TRACE reached the upstream"
        );
    }
}
