//! Adversary pass 1 on `story:boundary-loopback-relays` (wave 2026-10-30-w1, unit u2).
//!
//! Each case drives the boundary from outside, as the hostile agent container on loopback would,
//! against a fake upstream on `127.0.0.1:0` that records what reaches it. The threat model: the
//! agent must not obtain the token, must not reach an undeclared operation or repository, and must
//! not make the boundary send its token anywhere but the configured upstream.
//!
//! A red case's assertion message starts with `DEFECT:` and names the defect. Every other case
//! pins a property the threat model rests on. Token files are fixtures under the tree's own
//! `CARGO_TARGET_TMPDIR`; nothing here reads a real credential or leaves loopback.

use std::collections::VecDeque;
use std::convert::Infallible;
use std::fs;
use std::net::SocketAddr;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use b10x_loom_boundary::{MAX_INVOKE_BYTES, Relay, Upstream, bind};
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt as _, Empty, Full};
use hyper::body::{Body, Bytes, Frame, Incoming};
use hyper::header::HeaderMap;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};

const TOKEN: &str = "adversary-fixture-token";
const AGENT_VALUE: &str = "agent-held-value";
const WAIT: Duration = Duration::from_secs(10);
const OPERATIONS: [&str; 2] = ["merge_request.get", "pipeline.get"];

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary-d243-u2")
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean scratch");
    }
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn token_file(dir: &Path, contents: &[u8]) -> PathBuf {
    let path = dir.join("token");
    let staged = dir.join("token.staged");
    fs::write(&staged, contents).expect("fixture token");
    fs::rename(&staged, &path).expect("place fixture token");
    path
}

#[derive(Debug, Clone)]
struct Received {
    method: String,
    target: String,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Received {
    fn values(&self, name: &str) -> Vec<String> {
        self.headers
            .get_all(name)
            .iter()
            .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned())
            .collect()
    }
}

type TestBody = BoxBody<Bytes, Infallible>;
type Respond = Arc<dyn Fn(&Received) -> Response<TestBody> + Send + Sync>;

fn full(bytes: impl Into<Bytes>) -> TestBody {
    Full::new(bytes.into()).boxed()
}

/// A hyper upstream that records every request and answers with `respond`.
struct FakeUpstream {
    addr: SocketAddr,
    received: Arc<Mutex<Vec<Received>>>,
}

impl FakeUpstream {
    async fn start(respond: Respond) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind upstream");
        let addr = listener.local_addr().expect("upstream address");
        let received = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&received);
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    continue;
                };
                let record = Arc::clone(&record);
                let respond = Arc::clone(&respond);
                tokio::spawn(async move {
                    let service = service_fn(move |request: Request<Incoming>| {
                        let record = Arc::clone(&record);
                        let respond = Arc::clone(&respond);
                        async move {
                            let (parts, body) = request.into_parts();
                            let body = body
                                .collect()
                                .await
                                .map(|collected| collected.to_bytes().to_vec())
                                .unwrap_or_default();
                            let received = Received {
                                method: parts.method.to_string(),
                                target: parts
                                    .uri
                                    .path_and_query()
                                    .map(|pq| pq.as_str().to_owned())
                                    .unwrap_or_default(),
                                headers: parts.headers,
                                body,
                            };
                            let response = respond(&received);
                            record.lock().expect("record").push(received);
                            Ok::<_, Infallible>(response)
                        }
                    });
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), service)
                        .await;
                });
            }
        });
        FakeUpstream { addr, received }
    }

    async fn answering() -> Self {
        Self::start(Arc::new(|_| {
            Response::builder()
                .status(StatusCode::OK)
                .body(full("upstream-answer"))
                .expect("response")
        }))
        .await
    }

    fn upstream(&self) -> Upstream {
        Upstream::parse(&format!("http://{}", self.addr)).expect("a plain http upstream")
    }

    fn received(&self) -> Vec<Received> {
        self.received.lock().expect("record").clone()
    }
}

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

    async fn try_send<B>(&self, request: Request<B>, wait: Duration) -> Option<Response<Incoming>>
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let stream = TcpStream::connect(self.addr).await.expect("connect");
        let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .expect("handshake");
        tokio::spawn(connection);
        tokio::time::timeout(wait, sender.send_request(request))
            .await
            .ok()
            .map(|answered| answered.expect("the boundary answers"))
    }

    async fn send<B>(&self, request: Request<B>) -> Response<Incoming>
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        self.try_send(request, WAIT)
            .await
            .expect("the boundary answers in time")
    }

    async fn post(&self, target: &str, body: impl Into<Bytes>) -> Response<Incoming> {
        self.send(
            Request::post(target)
                .header("host", "x")
                .header("content-type", "application/json")
                .body(Full::new(body.into()))
                .expect("request"),
        )
        .await
    }

    /// Writes `request` as raw bytes and returns whatever the boundary answered before it closed
    /// or five seconds passed.
    async fn raw(&self, request: &[u8]) -> Vec<u8> {
        let mut stream = TcpStream::connect(self.addr).await.expect("connect");
        let _ = stream.write_all(request).await;
        let mut answer = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut answer)).await;
        answer
    }
}

fn status_of(answer: &[u8]) -> Option<u16> {
    let text = String::from_utf8_lossy(answer);
    text.strip_prefix("HTTP/1.1 ")
        .and_then(|rest| rest.get(..3))
        .and_then(|status| status.parse().ok())
}

async fn body_bytes(response: Response<Incoming>) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .map(|collected| collected.to_bytes().to_vec())
        .unwrap_or_default()
}

fn get(target: &str) -> Request<Empty<Bytes>> {
    Request::get(target)
        .header("host", "x")
        .body(Empty::<Bytes>::new())
        .expect("request")
}

/// A request body sent as the given chunks, its length unknown, so it goes chunked.
struct Chunks(VecDeque<Bytes>);

impl Body for Chunks {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        Poll::Ready(
            self.get_mut()
                .0
                .pop_front()
                .map(|bytes| Ok(Frame::data(bytes))),
        )
    }
}

async fn model_boundary(name: &str, upstream: &FakeUpstream) -> Boundary {
    let dir = scratch(name);
    Boundary::start(Relay::model(
        upstream.upstream(),
        token_file(&dir, TOKEN.as_bytes()),
    ))
    .await
}

async fn connectors_boundary(name: &str, upstream: &FakeUpstream) -> Boundary {
    let dir = scratch(name);
    let relay = Relay::connectors(
        upstream.upstream(),
        token_file(&dir, TOKEN.as_bytes()),
        OPERATIONS,
    )
    .expect("declared operations");
    Boundary::start(relay).await
}

// ---------------------------------------------------------------------------------------------
// The token never reaches the agent
// ---------------------------------------------------------------------------------------------

/// The boundary holds the token value while it relays the response, and the story says the
/// token is never returned. An upstream that reflects the request's `authorization` into a
/// response header (a debug echo, an error page) hands it to the agent unless the boundary
/// drops it.
#[tokio::test]
async fn an_upstream_that_reflects_authorization_does_not_hand_the_token_to_the_agent() {
    let upstream = FakeUpstream::start(Arc::new(|received| {
        let mut response = Response::builder().status(StatusCode::OK);
        if let Some(value) = received.headers.get("authorization") {
            response = response.header("x-received-authorization", value.clone());
        }
        response.body(full("upstream-answer")).expect("response")
    }))
    .await;
    let boundary = model_boundary("reflecting-upstream", &upstream).await;

    let response = boundary.send(get("/v1/models")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let returned = format!("{:?}", response.headers());
    let received = upstream.received();
    assert_eq!(received.len(), 1, "the request reached the upstream");
    assert_eq!(
        received[0].values("authorization"),
        vec![format!("Bearer {TOKEN}")]
    );
    assert_eq!(
        boundary.logs.lock().expect("logs").clone(),
        vec!["model GET /v1/models 200"]
    );
    assert!(
        !returned.contains(TOKEN),
        "DEFECT: the boundary returns the upstream's response headers unfiltered (lib.rs send(), \
         only hop-by-hop headers are stripped), so an upstream that reflects Authorization hands \
         the projected token to the agent: {returned}"
    );
}

/// A redirect is the agent's to follow: the boundary returns it and never sends the token to
/// the `Location` it names.
#[tokio::test]
async fn a_redirect_from_the_upstream_reaches_the_agent_and_is_never_followed() {
    let elsewhere = FakeUpstream::answering().await;
    let location = format!("http://{}/v1/models", elsewhere.addr);
    let redirecting = FakeUpstream::start(Arc::new(move |_| {
        Response::builder()
            .status(StatusCode::TEMPORARY_REDIRECT)
            .header("location", location.clone())
            .body(full(""))
            .expect("response")
    }))
    .await;
    let boundary = model_boundary("redirect", &redirecting).await;

    for method in ["GET", "POST"] {
        let response = boundary
            .send(
                Request::builder()
                    .method(method)
                    .uri("/v1/messages")
                    .header("host", "x")
                    .body(Full::new(Bytes::from_static(b"{}")))
                    .expect("request"),
            )
            .await;
        assert_eq!(
            response.status(),
            StatusCode::TEMPORARY_REDIRECT,
            "{method}"
        );
        assert_eq!(
            response.headers()["location"],
            format!("http://{}/v1/models", elsewhere.addr).as_str()
        );
    }
    assert_eq!(redirecting.received().len(), 2);
    assert!(
        elsewhere.received().is_empty(),
        "the boundary followed a redirect and sent its token to another host"
    );
}

/// A raw upstream that answers `101 Switching Protocols` to whatever it is sent, then writes
/// bytes as if a tunnel were open.
async fn switching_upstream() -> (SocketAddr, Arc<Mutex<Vec<u8>>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind upstream");
    let addr = listener.local_addr().expect("upstream address");
    let heads = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&heads);
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                continue;
            };
            let record = Arc::clone(&record);
            tokio::spawn(async move {
                let mut head = Vec::new();
                let mut buffer = [0u8; 4096];
                while !head.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut buffer).await {
                        Ok(0) | Err(_) => break,
                        Ok(read) => head.extend_from_slice(&buffer[..read]),
                    }
                }
                record.lock().expect("heads").extend_from_slice(&head);
                let _ = stream
                    .write_all(
                        b"HTTP/1.1 101 Switching Protocols\r\nconnection: upgrade\r\nupgrade: websocket\r\n\r\n",
                    )
                    .await;
                tokio::time::sleep(Duration::from_millis(300)).await;
                let _ = stream.write_all(b"UPSTREAM-TUNNEL-BYTES").await;
                tokio::time::sleep(Duration::from_secs(2)).await;
            });
        }
    });
    (addr, heads)
}

#[tokio::test]
async fn an_upstream_answering_101_opens_no_tunnel_and_the_agent_gets_502() {
    let (addr, heads) = switching_upstream().await;
    let dir = scratch("upstream-101");
    let upstream = Upstream::parse(&format!("http://{addr}")).expect("upstream");
    let boundary =
        Boundary::start(Relay::model(upstream, token_file(&dir, TOKEN.as_bytes()))).await;

    let response = boundary
        .send(
            Request::get("/v1/realtime")
                .header("host", "x")
                .header("connection", "upgrade")
                .header("upgrade", "websocket")
                .body(Empty::<Bytes>::new())
                .expect("request"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert!(!response.headers().contains_key("upgrade"));
    let body = String::from_utf8_lossy(&body_bytes(response).await).into_owned();
    assert!(
        !body.contains("UPSTREAM-TUNNEL"),
        "upstream tunnel bytes reached the agent: {body}"
    );
    let head = String::from_utf8_lossy(&heads.lock().expect("heads")).to_ascii_lowercase();
    assert!(head.starts_with("get /v1/realtime http/1.1\r\n"), "{head}");
    assert!(
        !head.contains("upgrade"),
        "the agent's upgrade request reached the upstream: {head}"
    );
}

// ---------------------------------------------------------------------------------------------
// Framing and header smuggling
// ---------------------------------------------------------------------------------------------

/// `Transfer-Encoding` beside `Content-Length`, in both orders: the upstream receives the chunked
/// body as one framed request, never the agent's `content-length`, and nothing the agent put
/// after the terminating chunk is relayed as part of it.
#[tokio::test]
async fn transfer_encoding_beside_content_length_smuggles_nothing_upstream() {
    let upstream = FakeUpstream::answering().await;
    let boundary = model_boundary("te-cl", &upstream).await;
    let smuggled = format!(
        "GET /v1/smuggled HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {AGENT_VALUE}\r\n\r\n"
    );
    for framing in [
        "Content-Length: 4\r\nTransfer-Encoding: chunked\r\n",
        "Transfer-Encoding: chunked\r\nContent-Length: 4\r\n",
        "Transfer-Encoding: Chunked\r\nContent-Length: 60\r\n",
    ] {
        let request = format!(
            "POST /v1/messages HTTP/1.1\r\nHost: x\r\n{framing}\r\n5\r\nhello\r\n0\r\n\r\n{smuggled}"
        );
        let answer = boundary.raw(request.as_bytes()).await;
        assert_eq!(
            status_of(&answer),
            Some(200),
            "{framing:?}: {}",
            String::from_utf8_lossy(&answer)
        );
    }
    let received = upstream.received();
    let posts: Vec<&Received> = received.iter().filter(|r| r.method == "POST").collect();
    assert_eq!(posts.len(), 3, "{received:?}");
    for post in posts {
        assert_eq!(post.target, "/v1/messages");
        assert_eq!(post.body, b"hello", "the chunked body, decoded");
        assert!(
            !post.values("content-length").contains(&"4".to_owned())
                && !post.values("content-length").contains(&"60".to_owned()),
            "the agent's content-length reached the upstream: {post:?}"
        );
    }
    for request in &received {
        assert!(
            !format!("{:?}", request.headers).contains(AGENT_VALUE),
            "the agent's credential reached the upstream: {request:?}"
        );
        assert_eq!(
            request.values("authorization"),
            vec![format!("Bearer {TOKEN}")]
        );
    }
}

/// Request heads hyper must refuse: none reaches the upstream, each is answered 4xx.
#[tokio::test]
async fn malformed_framing_and_folded_headers_never_reach_the_upstream() {
    let upstream = FakeUpstream::answering().await;
    let boundary = model_boundary("malformed-heads", &upstream).await;
    let mut many_headers = String::from("GET /v1/models HTTP/1.1\r\nHost: x\r\n");
    for index in 0..150 {
        many_headers.push_str(&format!("x-filler-{index}: {index}\r\n"));
    }
    many_headers.push_str("Connection: close\r\n\r\n");
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "differing content-lengths",
            b"POST /v1/messages HTTP/1.1\r\nHost: x\r\nContent-Length: 1\r\nContent-Length: 2\r\nConnection: close\r\n\r\nab".to_vec(),
        ),
        (
            "an obs-folded authorization",
            format!("GET /v1/models HTTP/1.1\r\nHost: x\r\nX-Note: a\r\n Authorization: Bearer {AGENT_VALUE}\r\nConnection: close\r\n\r\n").into_bytes(),
        ),
        (
            "a space before the colon",
            format!("GET /v1/models HTTP/1.1\r\nHost: x\r\nAuthorization : Bearer {AGENT_VALUE}\r\nConnection: close\r\n\r\n").into_bytes(),
        ),
        (
            "a transfer-encoding that is not chunked",
            b"POST /v1/messages HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: gzip\r\nConnection: close\r\n\r\nab".to_vec(),
        ),
        (
            "transfer-encoding on HTTP/1.0",
            b"POST /v1/messages HTTP/1.0\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n0\r\n\r\n".to_vec(),
        ),
        (
            "a NUL in a header value",
            b"GET /v1/models HTTP/1.1\r\nHost: x\r\nX-Note: a\x00b\r\nConnection: close\r\n\r\n".to_vec(),
        ),
        (
            "a path that is not UTF-8",
            b"GET /v1/\xff\xfe HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n".to_vec(),
        ),
        ("150 headers", many_headers.into_bytes()),
    ];
    for (case, request) in cases {
        let answer = boundary.raw(&request).await;
        let status = status_of(&answer);
        assert!(
            matches!(status, Some(400..=499)),
            "{case}: {}",
            String::from_utf8_lossy(&answer)
        );
    }
    assert!(
        upstream.received().is_empty(),
        "a malformed head reached the upstream: {:?}",
        upstream.received()
    );
}

/// HTTP/2 with prior knowledge is not spoken: the preface is refused and nothing is relayed.
#[tokio::test]
async fn http2_prior_knowledge_reaches_nothing() {
    let upstream = FakeUpstream::answering().await;
    let boundary = model_boundary("h2-prior-knowledge", &upstream).await;
    // The preface, then an empty SETTINGS frame.
    let mut preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n".to_vec();
    preface.extend_from_slice(&[0, 0, 0, 4, 0, 0, 0, 0, 0]);
    let answer = boundary.raw(&preface).await;
    assert!(
        !matches!(status_of(&answer), Some(200..=399)),
        "{}",
        String::from_utf8_lossy(&answer)
    );
    assert!(upstream.received().is_empty());
}

/// The agent's `connection` header cannot name away the token or the upstream's `host`, and
/// `expect` is the agent's own framing, never relayed.
#[tokio::test]
async fn the_agent_cannot_unset_the_token_or_host_and_expect_is_not_relayed() {
    let upstream = FakeUpstream::answering().await;
    let boundary = model_boundary("connection-listed", &upstream).await;
    let response = boundary
        .send(
            Request::post("/v1/messages")
                .header("host", "agent-chosen.example")
                .header("connection", "authorization, host, x-keep")
                .header("x-keep", "1")
                .header("expect", "100-continue")
                .body(Full::new(Bytes::from_static(b"{\"model\":\"m\"}")))
                .expect("request"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received();
    assert_eq!(received.len(), 1);
    let request = &received[0];
    assert_eq!(
        request.values("authorization"),
        vec![format!("Bearer {TOKEN}")]
    );
    assert_eq!(request.values("host"), vec![upstream.addr.to_string()]);
    assert!(request.values("x-keep").is_empty());
    assert!(request.values("expect").is_empty(), "{request:?}");
    assert_eq!(request.body, b"{\"model\":\"m\"}");
}

// ---------------------------------------------------------------------------------------------
// Connectors invoke admission
// ---------------------------------------------------------------------------------------------

/// Bodies Connectors would read as another operation, or that are not one UTF-8 JSON object:
/// each refused 403 and none reaches the upstream.
#[tokio::test]
async fn an_escaped_second_operation_key_or_a_body_that_is_not_one_json_object_is_refused() {
    let upstream = FakeUpstream::answering().await;
    let boundary = connectors_boundary("invoke-json", &upstream).await;
    let mut bom = b"\xef\xbb\xbf".to_vec();
    bom.extend_from_slice(br#"{"operation":"merge_request.get"}"#);
    let bodies: Vec<(&str, Vec<u8>)> = vec![
        (
            "an escaped second key after",
            br#"{"operation":"merge_request.get","operation":"merge_request.merge"}"#.to_vec(),
        ),
        (
            "an escaped second key before",
            br#"{"operation":"merge_request.merge","operation":"merge_request.get"}"#.to_vec(),
        ),
        (
            "two escaped keys",
            br#"{"operation":"merge_request.get","operation":"merge_request.merge"}"#.to_vec(),
        ),
        ("a byte-order mark", bom),
        (
            "an escaped NUL in the operation",
            br#"{"operation":"merge_request.get\u0000"}"#.to_vec(),
        ),
        (
            "a trailing space in the operation",
            br#"{"operation":"merge_request.get "}"#.to_vec(),
        ),
        (
            "invalid UTF-8 in the operation",
            b"{\"operation\":\"merge_request.g\xc3t\"}".to_vec(),
        ),
        (
            "a NUL after the object",
            b"{\"operation\":\"merge_request.get\"}\x00".to_vec(),
        ),
        (
            "a trailing comma",
            br#"{"operation":"merge_request.get",}"#.to_vec(),
        ),
        (
            "single quotes",
            br#"{'operation':'merge_request.get'}"#.to_vec(),
        ),
        (
            "a comment",
            br#"/*x*/{"operation":"merge_request.get"}"#.to_vec(),
        ),
        (
            "a second object",
            br#"{"operation":"merge_request.get"}{"operation":"merge_request.merge"}"#.to_vec(),
        ),
    ];
    for (case, body) in bodies {
        let response = boundary.post("/v1alpha2/invoke", body).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{case}");
    }
    // `Expect: 100-continue` does not change what is admitted.
    let response = boundary
        .send(
            Request::post("/v1alpha2/invoke")
                .header("host", "x")
                .header("expect", "100-continue")
                .body(Full::new(Bytes::from_static(
                    br#"{"operation":"merge_request.merge"}"#,
                )))
                .expect("request"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN, "expect");
    assert!(
        upstream.received().is_empty(),
        "a refused invoke reached the upstream: {:?}",
        upstream.received()
    );
}

/// A body of exactly the limit sent chunked, in small frames, with no length declared: admitted
/// whole and forwarded byte for byte.
#[tokio::test]
async fn an_invoke_at_the_limit_sent_chunked_is_admitted_whole() {
    let upstream = FakeUpstream::answering().await;
    let boundary = connectors_boundary("invoke-chunked-limit", &upstream).await;
    let mut body = br#"{"operation":"pipeline.get","pad":""#.to_vec();
    body.resize(MAX_INVOKE_BYTES - 2, b'y');
    body.extend_from_slice(b"\"}");
    assert_eq!(body.len(), MAX_INVOKE_BYTES);
    let chunks: VecDeque<Bytes> = body.chunks(4093).map(Bytes::copy_from_slice).collect();
    let response = boundary
        .send(
            Request::post("/v1alpha2/invoke")
                .header("host", "x")
                .header("content-type", "application/json")
                .body(Chunks(chunks))
                .expect("request"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].body, body);
    assert_eq!(
        received[0].values("content-length"),
        vec![MAX_INVOKE_BYTES.to_string()]
    );
}

// ---------------------------------------------------------------------------------------------
// Git routes
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn git_paths_with_case_parameters_or_encodings_are_refused() {
    let upstream = FakeUpstream::answering().await;
    let dir = scratch("git-variants");
    let relay = Relay::git(
        upstream.upstream(),
        token_file(&dir, TOKEN.as_bytes()),
        ["acme/widgets"],
    )
    .expect("declared repositories");
    let boundary = Boundary::start(relay).await;
    for (method, target) in [
        ("GET", "/acme/widgets.GIT/info/refs?service=git-upload-pack"),
        ("GET", "/acme/widgets.Git/info/refs?service=git-upload-pack"),
        (
            "GET",
            "/acme/widgets.git;x=1/info/refs?service=git-upload-pack",
        ),
        (
            "GET",
            "/acme/widgets.git/info/refs;x?service=git-upload-pack",
        ),
        (
            "GET",
            "/acme/widgets.git/info/refs?service%3Dgit-upload-pack",
        ),
        (
            "GET",
            "/acme/widgets.git/info/refs?service=git-upload-pack%20",
        ),
        ("GET", "/acme/widgets.git/info/refs?service=Git-Upload-Pack"),
        (
            "GET",
            "/acme/widgets.git/info/refs?service=git-upload-pack;x",
        ),
        ("POST", "/acme/widgets.git/git-upload-pack/"),
        ("POST", "/acme/widgets.git/GIT-UPLOAD-PACK"),
        ("POST", "/acme/WIDGETS.git/git-upload-pack"),
        ("POST", "/acme/widgets.git%2f/git-upload-pack"),
        ("POST", "/acme/widgets.git/%67it-upload-pack"),
        ("POST", "/%61cme/widgets.git/git-upload-pack"),
        ("POST", "/acme/widgets.git\\git-upload-pack"),
        (
            "POST",
            "/acme/widgets.git/git-receive-pack/../git-upload-pack",
        ),
        (
            "HEAD",
            "/acme/widgets.git/info/refs?service=git-upload-pack",
        ),
    ] {
        let response = boundary
            .send(
                Request::builder()
                    .method(method)
                    .uri(target)
                    .header("host", "x")
                    .body(Empty::<Bytes>::new())
                    .expect("request"),
            )
            .await;
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "{method} {target}"
        );
    }
    assert!(
        upstream.received().is_empty(),
        "a git variant reached the upstream: {:?}",
        upstream.received()
    );
}

// ---------------------------------------------------------------------------------------------
// The token file
// ---------------------------------------------------------------------------------------------

/// A token with a line break or a NUL inside cannot add a header to the upstream request.
#[tokio::test]
async fn a_token_file_with_an_inner_line_break_injects_no_header() {
    let upstream = FakeUpstream::answering().await;
    let dir = scratch("token-line-break");
    let path = token_file(&dir, TOKEN.as_bytes());
    let boundary = Boundary::start(Relay::model(upstream.upstream(), path.clone())).await;
    for contents in [
        format!("{TOKEN}\r\nx-injected: 1"),
        format!("{TOKEN}\nx-injected: 1\n"),
        format!("{TOKEN}\0x"),
        format!("Bearer {TOKEN}"),
    ] {
        token_file(&dir, contents.as_bytes());
        let response = boundary.send(get("/v1/models")).await;
        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "{contents:?}"
        );
    }
    assert!(upstream.received().is_empty(), "{:?}", upstream.received());
    token_file(&dir, format!("{TOKEN}\n").as_bytes());
    assert_eq!(
        boundary.send(get("/v1/models")).await.status(),
        StatusCode::OK
    );
    let received = upstream.received();
    assert_eq!(received.len(), 1);
    assert!(received[0].values("x-injected").is_empty());
}

/// `O_NONBLOCK` on Linux (x86_64 and aarch64): opening a FIFO's write end this way does not
/// wait, and wakes a reader blocked in `open`.
const O_NONBLOCK: i32 = 0o4000;

/// `read_token` refuses what is not a regular file (a directory answers 503), but it checks only
/// after `File::open`, and opening a FIFO for reading waits for a writer. The request hangs and
/// holds a blocking thread instead of answering 503.
#[test]
fn a_fifo_in_place_of_the_token_file_answers_503_instead_of_hanging() {
    let dir = scratch("token-fifo");
    let path = dir.join("token");
    let made = std::process::Command::new("mkfifo")
        .arg(&path)
        .status()
        .expect("mkfifo runs");
    assert!(made.success(), "mkfifo");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");
    let (status, reached) = runtime.block_on(async {
        let upstream = FakeUpstream::answering().await;
        let boundary = Boundary::start(Relay::model(upstream.upstream(), path.clone())).await;
        let answered = boundary
            .try_send(get("/v1/models"), Duration::from_secs(5))
            .await
            .map(|response| response.status().as_u16());
        (answered, upstream.received().len())
    });
    // Wake the boundary's blocked open, so its blocking thread ends with the runtime.
    let _ = fs::OpenOptions::new()
        .write(true)
        .custom_flags(O_NONBLOCK)
        .open(&path);
    runtime.shutdown_timeout(Duration::from_millis(500));
    assert_eq!(reached, 0, "a request without a token reached the upstream");
    assert_eq!(
        status,
        Some(503),
        "DEFECT: a FIFO in the token's place blocks read_token in File::open before its is_file \
         check (lib.rs:490-491): the request hangs and holds a blocking thread instead of \
         answering 503"
    );
}
