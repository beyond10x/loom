//! The boundary's loopback relays (`story:boundary-loopback-relays`).
//!
//! Each upstream is a fake: a hyper server on `127.0.0.1:0` inside the test that records every
//! request it receives (method, target, headers, body) and answers as the test says. The token
//! files are fixtures the test writes under its own `CARGO_TARGET_TMPDIR`; nothing here reads a
//! real credential or reaches beyond loopback.

use std::convert::Infallible;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use b10x_loom_boundary::{
    BindError, CONNECTORS_PORT, GIT_PORT, Listener, MAX_INVOKE_BYTES, MODEL_PORT, Relay, Upstream,
    UpstreamError, bind,
};
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt as _, Empty, Full};
use hyper::body::{Body, Bytes, Frame, Incoming};
use hyper::header::HeaderMap;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

const TOKEN_ONE: &str = "fixture-token-one";
const TOKEN_TWO: &str = "fixture-token-two";
const WAIT: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// A fresh directory for one test under the tree's own build directory.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("boundary-relays")
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean scratch");
    }
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

/// A fixture token file holding `contents`, written whole and renamed into place.
fn write_token(path: &Path, contents: &[u8]) {
    let staged = path.with_extension("staged");
    fs::write(&staged, contents).expect("write fixture token");
    fs::rename(&staged, path).expect("place fixture token");
}

fn token_file(dir: &Path, contents: &str) -> PathBuf {
    let path = dir.join("token");
    write_token(&path, contents.as_bytes());
    path
}

/// One request a fake upstream received.
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

fn full(bytes: impl Into<Bytes>) -> TestBody {
    Full::new(bytes.into()).boxed()
}

type Respond = Arc<dyn Fn(&Received) -> Response<TestBody> + Send + Sync>;

/// A fake upstream: records each request, then answers with `respond`.
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

    /// An upstream that answers every request `200` with `upstream-answer`.
    async fn answering() -> Self {
        Self::start(Arc::new(|_| {
            Response::builder()
                .status(StatusCode::OK)
                .body(full("upstream-answer"))
                .expect("response")
        }))
        .await
    }

    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn upstream(&self) -> Upstream {
        Upstream::parse(&self.url()).expect("a plain http upstream")
    }

    fn received(&self) -> Vec<Received> {
        self.received.lock().expect("record").clone()
    }
}

/// A running boundary listener and the log lines it wrote.
struct Boundary {
    addr: SocketAddr,
    logs: Arc<Mutex<Vec<String>>>,
}

impl Boundary {
    async fn start(relay: Relay) -> Self {
        let listener: Listener = bind(SocketAddr::from(([127, 0, 0, 1], 0)), relay)
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

    async fn send<B>(&self, request: Request<B>) -> Response<Incoming>
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let stream = TcpStream::connect(self.addr).await.expect("connect");
        let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .expect("handshake");
        tokio::spawn(connection);
        tokio::time::timeout(WAIT, sender.send_request(request))
            .await
            .expect("the boundary answers in time")
            .expect("the boundary answers")
    }

    async fn get(&self, target: &str) -> Response<Incoming> {
        self.send(
            Request::get(target)
                .header("host", self.addr.to_string())
                .body(Empty::<Bytes>::new())
                .expect("request"),
        )
        .await
    }

    async fn post(&self, target: &str, body: impl Into<Bytes>) -> Response<Incoming> {
        self.send(
            Request::post(target)
                .header("host", self.addr.to_string())
                .header("content-type", "application/json")
                .body(Full::new(body.into()))
                .expect("request"),
        )
        .await
    }

    /// Writes `request` as raw bytes and returns the status the boundary answered.
    async fn raw_status(&self, request: &str) -> u16 {
        let mut stream = TcpStream::connect(self.addr).await.expect("connect");
        stream
            .write_all(request.as_bytes())
            .await
            .expect("write request");
        let mut answer = Vec::new();
        let _ = tokio::time::timeout(WAIT, stream.read_to_end(&mut answer)).await;
        let text = String::from_utf8_lossy(&answer);
        let status = text
            .strip_prefix("HTTP/1.1 ")
            .and_then(|rest| rest.get(..3))
            .unwrap_or_else(|| panic!("no status line in {text:?} for {request:?}"));
        status.parse().expect("a numeric status")
    }
}

async fn body_bytes(response: Response<Incoming>) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .expect("the body")
        .to_bytes()
        .to_vec()
}

fn bearer(token: &str) -> Vec<String> {
    vec![format!("Bearer {token}")]
}

/// A body fed from a channel, so a test decides when each chunk is sent.
struct ChannelBody(mpsc::Receiver<Bytes>);

impl Body for ChannelBody {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        self.get_mut()
            .0
            .poll_recv(cx)
            .map(|chunk| chunk.map(|bytes| Ok(Frame::data(bytes))))
    }
}

const DECLARED_OPERATIONS: [&str; 2] = ["merge_request.get", "pipeline.get"];

async fn connectors_boundary(name: &str) -> (FakeUpstream, Boundary) {
    let dir = scratch(name);
    let upstream = FakeUpstream::answering().await;
    let relay = Relay::connectors(
        upstream.upstream(),
        token_file(&dir, TOKEN_ONE),
        DECLARED_OPERATIONS,
    )
    .expect("declared operations");
    let boundary = Boundary::start(relay).await;
    (upstream, boundary)
}

async fn git_boundary(name: &str) -> (FakeUpstream, Boundary) {
    let dir = scratch(name);
    let upstream = FakeUpstream::answering().await;
    let base = Upstream::parse(&format!("{}/git/", upstream.url())).expect("a base path");
    let relay = Relay::git(base, token_file(&dir, TOKEN_ONE), ["acme/widgets"])
        .expect("declared repositories");
    let boundary = Boundary::start(relay).await;
    (upstream, boundary)
}

async fn model_boundary(name: &str) -> (FakeUpstream, Boundary) {
    let dir = scratch(name);
    let upstream = FakeUpstream::answering().await;
    let relay = Relay::model(upstream.upstream(), token_file(&dir, TOKEN_ONE));
    let boundary = Boundary::start(relay).await;
    (upstream, boundary)
}

// ---------------------------------------------------------------------------------------------
// Model relay
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_model_relay_adds_the_projected_token_and_drops_the_agents_credentials() {
    let dir = scratch("model-credentials");
    let upstream = FakeUpstream::start(Arc::new(|_| {
        Response::builder()
            .status(StatusCode::OK)
            .header("connection", "x-upstream-hop")
            .header("x-upstream-hop", "1")
            .header("keep-alive", "timeout=5")
            .header("proxy-authenticate", "Basic realm=upstream")
            .header("x-kept-response", "yes")
            .body(full("upstream-answer"))
            .expect("response")
    }))
    .await;
    // The fake really sends the hop-by-hop headers the boundary must strip.
    let direct = Boundary {
        addr: upstream.addr,
        logs: Arc::default(),
    }
    .get("/v1/models")
    .await;
    assert_eq!(direct.headers()["x-upstream-hop"], "1");
    assert_eq!(direct.headers()["keep-alive"], "timeout=5");
    assert!(direct.headers().contains_key("proxy-authenticate"));
    upstream.received.lock().expect("record").clear();

    let relay = Relay::model(upstream.upstream(), token_file(&dir, TOKEN_ONE));
    assert_eq!(relay.service(), "model");
    assert_eq!(relay.port(), MODEL_PORT);
    let boundary = Boundary::start(relay).await;

    let response = boundary
        .send(
            Request::post("/v1/messages?beta=true")
                .header("host", "agent-chosen.example")
                .header("authorization", "Bearer agent-held-value")
                .header("proxy-authorization", "Basic agent-proxy-value")
                .header("x-api-key", "agent-api-key-value")
                .header("cookie", "session=agent-cookie-value")
                .header("connection", "x-agent-hop")
                .header("x-agent-hop", "1")
                .header("keep-alive", "300")
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .header("x-kept-request", "yes")
                .body(Full::new(Bytes::from_static(b"{\"model\":\"m\"}")))
                .expect("request"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let headers = response.headers().clone();
    let body = body_bytes(response).await;
    assert_eq!(body, b"upstream-answer");

    let received = upstream.received();
    assert_eq!(received.len(), 1, "one request reached the upstream");
    let request = &received[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/v1/messages?beta=true");
    assert_eq!(request.body, b"{\"model\":\"m\"}");
    assert_eq!(request.values("authorization"), bearer(TOKEN_ONE));
    assert_eq!(request.values("host"), vec![upstream.addr.to_string()]);
    for dropped in [
        "proxy-authorization",
        "x-api-key",
        "cookie",
        "x-agent-hop",
        "keep-alive",
    ] {
        assert!(
            request.values(dropped).is_empty(),
            "{dropped} reached the upstream"
        );
    }
    assert!(
        request
            .values("connection")
            .iter()
            .all(|value| !value.contains("x-agent-hop")),
        "the agent's connection header reached the upstream"
    );
    assert_eq!(request.values("anthropic-version"), vec!["2023-06-01"]);
    assert_eq!(request.values("x-kept-request"), vec!["yes"]);
    assert_eq!(request.values("content-type"), vec!["application/json"]);
    let forwarded = format!("{:?}", request.headers);
    for agent_value in [
        "agent-held-value",
        "agent-proxy-value",
        "agent-api-key-value",
        "agent-cookie-value",
    ] {
        assert!(
            !forwarded.contains(agent_value),
            "{agent_value} reached the upstream"
        );
    }

    for stripped in ["x-upstream-hop", "keep-alive", "proxy-authenticate"] {
        assert!(
            !headers.contains_key(stripped),
            "{stripped} reached the agent"
        );
    }
    assert_eq!(headers["x-kept-response"], "yes");
    let returned = format!("{headers:?}");
    assert!(!returned.contains(TOKEN_ONE), "the token reached the agent");

    assert_eq!(boundary.logs(), vec!["model POST /v1/messages 200"]);
}

#[tokio::test]
async fn the_model_relay_streams_the_response() {
    let dir = scratch("model-streams");
    let (chunks, receiver) = mpsc::channel::<Bytes>(4);
    let receiver = Arc::new(Mutex::new(Some(receiver)));
    let upstream = FakeUpstream::start(Arc::new(move |_| {
        let receiver = receiver
            .lock()
            .expect("receiver")
            .take()
            .expect("one streamed request");
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/event-stream")
            .body(ChannelBody(receiver).boxed())
            .expect("response")
    }))
    .await;
    let boundary = Boundary::start(Relay::model(
        upstream.upstream(),
        token_file(&dir, TOKEN_ONE),
    ))
    .await;

    chunks
        .send(Bytes::from_static(b"event: message_start\ndata: {}\n\n"))
        .await
        .expect("first chunk");
    let response = boundary.post("/v1/messages", "{\"stream\":true}").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let mut body = response.into_body();

    // The upstream has sent one chunk and holds the stream open: a relay that buffered the
    // response would deliver nothing until the end.
    let first = tokio::time::timeout(WAIT, body.frame())
        .await
        .expect("the first chunk arrives while the upstream still streams")
        .expect("a frame")
        .expect("data");
    assert_eq!(
        first.into_data().expect("data frame"),
        Bytes::from_static(b"event: message_start\ndata: {}\n\n")
    );

    chunks
        .send(Bytes::from_static(b"event: message_stop\ndata: {}\n\n"))
        .await
        .expect("second chunk");
    drop(chunks);
    let mut rest = Vec::new();
    while let Some(frame) = tokio::time::timeout(WAIT, body.frame())
        .await
        .expect("the stream ends")
    {
        if let Ok(data) = frame.expect("a frame").into_data() {
            rest.extend_from_slice(&data);
        }
    }
    assert_eq!(rest, b"event: message_stop\ndata: {}\n\n");
}

#[tokio::test]
async fn a_model_path_that_could_traverse_is_refused_and_never_reaches_the_upstream() {
    let (upstream, boundary) = model_boundary("model-traversal").await;
    for (target, status) in [
        ("/v1/../admin", 400),
        ("/v1/./messages", 400),
        ("/v1/messages/..", 400),
        ("/v1//messages", 400),
        ("/v1/", 400),
        ("/v1/%2e%2e/admin", 400),
        ("/v1/%2E/messages", 400),
        ("/v1/a%2fb", 400),
        ("/v1/a%2Fb", 400),
        ("/v1/a%5cb", 400),
        ("/v1/.%2e/admin", 400),
        ("/v1", 404),
        ("/v2/messages", 404),
        ("/admin", 404),
        ("/", 404),
    ] {
        let response = boundary.get(target).await;
        assert_eq!(response.status().as_u16(), status, "{target}");
    }
    // A raw backslash is not a valid request target; either hyper or the relay refuses it.
    let status = boundary
        .raw_status("GET /v1/a\\b HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .await;
    assert_eq!(status, 400, "a backslash in the path");
    assert!(
        upstream.received().is_empty(),
        "a refused path reached the upstream"
    );

    // The same relay forwards a clean path.
    let response = boundary.get("/v1/models?limit=2").await;
    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].target, "/v1/models?limit=2");
}

#[tokio::test]
async fn a_proxy_target_connect_or_trace_is_refused_and_never_reaches_the_upstream() {
    let (upstream, boundary) = model_boundary("model-proxy-forms").await;
    let absolute = format!(
        "GET http://{}/v1/models HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        upstream.addr, upstream.addr
    );
    assert_eq!(boundary.raw_status(&absolute).await, 400, "absolute-form");
    let connect = format!(
        "CONNECT {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        upstream.addr, upstream.addr
    );
    assert_eq!(boundary.raw_status(&connect).await, 405, "CONNECT");
    let authority = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        upstream.addr, upstream.addr
    );
    assert_eq!(boundary.raw_status(&authority).await, 400, "authority-form");
    assert_eq!(
        boundary
            .raw_status("TRACE /v1/models HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
            .await,
        405,
        "TRACE would echo the token"
    );
    let asterisk = boundary
        .raw_status("OPTIONS * HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .await;
    assert!((400..500).contains(&asterisk), "asterisk-form: {asterisk}");
    assert!(
        upstream.received().is_empty(),
        "a proxy-form request reached the upstream"
    );
}

#[tokio::test]
async fn an_unreachable_upstream_answers_502_and_logs_it() {
    let dir = scratch("model-unreachable");
    let closed = TcpListener::bind("127.0.0.1:0").await.expect("a port");
    let addr = closed.local_addr().expect("address");
    drop(closed);
    let upstream = Upstream::parse(&format!("http://{addr}")).expect("upstream");
    let boundary = Boundary::start(Relay::model(upstream, token_file(&dir, TOKEN_ONE))).await;
    let response = boundary.get("/v1/models").await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = String::from_utf8(body_bytes(response).await).expect("text");
    assert!(!body.contains(TOKEN_ONE));
    assert_eq!(boundary.logs(), vec!["model GET /v1/models 502"]);
}

// ---------------------------------------------------------------------------------------------
// Connectors relay
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn an_undeclared_operation_is_refused_and_never_reaches_the_upstream() {
    let (upstream, boundary) = connectors_boundary("connectors-undeclared").await;
    for body in [
        r#"{"operation":"merge_request.merge","input":{"project":"acme/widgets"}}"#,
        r#"{"input":{"project":"acme/widgets"}}"#,
        r#"{"input":{"operation":"merge_request.get"}}"#,
        r#"{"operation":1}"#,
        r#"{"operation":null}"#,
        r#"{"operation":["merge_request.get"]}"#,
        r#"[{"operation":"merge_request.get"}]"#,
        r#""merge_request.get""#,
        r#"{"operation":"merge_request.get"} {}"#,
        r#"{"operation":"MERGE_REQUEST.GET"}"#,
        "operation=merge_request.get",
        "",
    ] {
        let response = boundary.post("/v1alpha2/invoke", body).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{body}");
        assert_eq!(
            response.headers()["content-type"],
            "application/json",
            "{body}"
        );
        let answer = String::from_utf8(body_bytes(response).await).expect("text");
        let error: serde_json::Value = serde_json::from_str(&answer).expect("a JSON error");
        assert!(error.get("error").is_some(), "{answer}");
        for request_value in ["merge_request", "MERGE_REQUEST", "acme", "widgets"] {
            assert!(
                !answer.contains(request_value),
                "the refusal repeats {request_value}: {answer}"
            );
        }
    }
    assert!(
        upstream.received().is_empty(),
        "an undeclared operation reached the upstream"
    );
}

#[tokio::test]
async fn a_declared_operation_reaches_the_upstream_byte_for_byte() {
    let (upstream, boundary) = connectors_boundary("connectors-declared").await;
    let body: &[u8] = b"{ \"input\" : {\"project\": \"acme/widgets\",  \"iid\": 7, \"note\": \"caf\\u00e9\" },\n  \"operation\" : \"merge_request.get\" }";
    let response = boundary
        .send(
            Request::post("/v1alpha2/invoke")
                .header("host", "agent-chosen.example")
                .header("content-type", "application/json")
                .header("authorization", "Bearer agent-held-value")
                .body(Full::new(Bytes::from_static(body)))
                .expect("request"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_bytes(response).await, b"upstream-answer");

    let received = upstream.received();
    assert_eq!(received.len(), 1);
    let request = &received[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/v1alpha2/invoke");
    assert_eq!(request.body, body, "the body is forwarded byte for byte");
    assert_eq!(
        request.values("content-length"),
        vec![body.len().to_string()]
    );
    assert_eq!(request.values("authorization"), bearer(TOKEN_ONE));
    assert_eq!(request.values("host"), vec![upstream.addr.to_string()]);
    assert_eq!(request.values("content-type"), vec!["application/json"]);

    // An escaped key or value is the same JSON text Connectors decodes: admitted as declared.
    let escaped: &[u8] = b"{\"op\\u0065ration\":\"pipeline\\u002eget\"}";
    let response = boundary.post("/v1alpha2/invoke", escaped).await;
    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received();
    assert_eq!(received.len(), 2);
    assert_eq!(received[1].body, escaped);

    // A body at exactly the limit is admitted; one byte more is refused before it is read whole.
    let prefix = br#"{"operation":"merge_request.get","pad":""#;
    let mut at_limit = prefix.to_vec();
    at_limit.resize(MAX_INVOKE_BYTES - 2, b'x');
    at_limit.extend_from_slice(b"\"}");
    assert_eq!(at_limit.len(), MAX_INVOKE_BYTES);
    let response = boundary.post("/v1alpha2/invoke", at_limit.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received();
    assert_eq!(received.len(), 3);
    assert_eq!(received[2].body, at_limit);
}

#[tokio::test]
async fn an_invoke_body_over_the_limit_is_refused_and_never_reaches_the_upstream() {
    let (upstream, boundary) = connectors_boundary("connectors-limit").await;
    let prefix = br#"{"operation":"merge_request.get","pad":""#;
    let mut over = prefix.to_vec();
    over.resize(MAX_INVOKE_BYTES - 1, b'x');
    over.extend_from_slice(b"\"}");
    assert_eq!(over.len(), MAX_INVOKE_BYTES + 1);
    let response = boundary.post("/v1alpha2/invoke", over.clone()).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

    // The same body sent chunked, with no length declared.
    let (chunks, receiver) = mpsc::channel::<Bytes>(4);
    let sending = tokio::spawn(async move {
        for part in over.chunks(16 * 1024) {
            if chunks.send(Bytes::copy_from_slice(part)).await.is_err() {
                break;
            }
        }
    });
    let response = boundary
        .send(
            Request::post("/v1alpha2/invoke")
                .header("host", "x")
                .header("content-type", "application/json")
                .body(ChannelBody(receiver))
                .expect("request"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let _ = sending.await;
    assert!(
        upstream.received().is_empty(),
        "an oversized invoke reached the upstream"
    );
}

#[tokio::test]
async fn a_duplicated_operation_key_is_refused() {
    let (upstream, boundary) = connectors_boundary("connectors-duplicated").await;
    for body in [
        r#"{"operation":"merge_request.get","operation":"merge_request.merge"}"#,
        r#"{"operation":"merge_request.merge","operation":"merge_request.get"}"#,
        r#"{"operation":"merge_request.get","operation":"merge_request.get"}"#,
        r#"{"operation":"merge_request.get","operation":"merge_request.merge"}"#,
        r#"{"operation":"merge_request.get","input":{},"input":{"iid":7}}"#,
    ] {
        let response = boundary.post("/v1alpha2/invoke", body).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{body}");
        let answer = String::from_utf8(body_bytes(response).await).expect("text");
        assert!(!answer.contains("merge_request"), "{answer}");
    }
    assert!(
        upstream.received().is_empty(),
        "a duplicated key reached the upstream"
    );
}

#[tokio::test]
async fn only_describe_and_the_v1alpha2_invoke_reach_the_connectors_upstream() {
    let (upstream, boundary) = connectors_boundary("connectors-routes").await;
    let declared = r#"{"operation":"merge_request.get"}"#;
    for (method, target) in [
        ("POST", "/v1/invoke"),
        ("GET", "/v1alpha2/invoke"),
        ("PUT", "/v1alpha2/invoke"),
        ("POST", "/v1alpha2/invoke?operation=merge_request.get"),
        ("POST", "/v1alpha2/invoke/"),
        ("POST", "/v1/describe"),
        ("GET", "/v1/describe?x=1"),
        ("GET", "/v1/describe/"),
        ("GET", "/healthz"),
        ("GET", "/"),
    ] {
        let response = boundary
            .send(
                Request::builder()
                    .method(method)
                    .uri(target)
                    .header("host", "x")
                    .header("content-type", "application/json")
                    .body(Full::new(Bytes::from_static(declared.as_bytes())))
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
        "a refused route reached the upstream"
    );

    let response = boundary.get("/v1/describe").await;
    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].method, "GET");
    assert_eq!(received[0].target, "/v1/describe");
    assert_eq!(received[0].values("authorization"), bearer(TOKEN_ONE));
}

// ---------------------------------------------------------------------------------------------
// Git relay
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn an_undeclared_repository_is_refused_and_never_reaches_the_upstream() {
    let (upstream, boundary) = git_boundary("git-undeclared").await;
    for (method, target) in [
        ("GET", "/acme/gadgets.git/info/refs?service=git-upload-pack"),
        (
            "GET",
            "/other/widgets.git/info/refs?service=git-upload-pack",
        ),
        ("GET", "/ACME/widgets.git/info/refs?service=git-upload-pack"),
        ("POST", "/acme/gadgets.git/git-receive-pack"),
        ("POST", "/acme/gadgets.git/git-upload-pack"),
        ("GET", "/acme/widgets/info/refs?service=git-upload-pack"),
        ("POST", "/acme/widgets.git.git/git-upload-pack"),
        ("GET", "/acme/.git/info/refs?service=git-upload-pack"),
        ("GET", "/widgets.git/info/refs?service=git-upload-pack"),
        (
            "GET",
            "/x/acme/widgets.git/info/refs?service=git-upload-pack",
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
        "an undeclared repository reached the upstream"
    );

    let response = boundary
        .get("/acme/widgets.git/info/refs?service=git-upload-pack")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received();
    assert_eq!(received.len(), 1);
    assert_eq!(
        received[0].target,
        "/git/acme/widgets.git/info/refs?service=git-upload-pack"
    );
    assert_eq!(received[0].values("authorization"), bearer(TOKEN_ONE));
}

#[tokio::test]
async fn a_git_request_outside_the_smart_http_routes_is_refused_and_never_reaches_the_upstream() {
    let (upstream, boundary) = git_boundary("git-routes").await;
    for (method, target) in [
        ("GET", "/acme/widgets.git/info/refs"),
        ("GET", "/acme/widgets.git/info/refs?"),
        (
            "GET",
            "/acme/widgets.git/info/refs?service=git-upload-pack&x=1",
        ),
        (
            "GET",
            "/acme/widgets.git/info/refs?x=1&service=git-upload-pack",
        ),
        (
            "GET",
            "/acme/widgets.git/info/refs?service=git-upload-pack&service=git-upload-pack",
        ),
        (
            "GET",
            "/acme/widgets.git/info/refs?service=git-upload-pack&",
        ),
        (
            "GET",
            "/acme/widgets.git/info/refs?service=git-upload-archive",
        ),
        (
            "GET",
            "/acme/widgets.git/info/refs?service=git-upload-pack%00",
        ),
        ("GET", "/acme/widgets.git/info/refs?SERVICE=git-upload-pack"),
        (
            "POST",
            "/acme/widgets.git/info/refs?service=git-upload-pack",
        ),
        ("GET", "/acme/widgets.git/git-upload-pack"),
        (
            "POST",
            "/acme/widgets.git/git-upload-pack?service=git-upload-pack",
        ),
        ("POST", "/acme/widgets.git/git-receive-pack?x=1"),
        ("POST", "/acme/widgets.git/git-upload-archive"),
        ("GET", "/acme/widgets.git/HEAD"),
        ("GET", "/acme/widgets.git/objects/info/packs"),
        (
            "GET",
            "/acme/widgets.git/info/refs/?service=git-upload-pack",
        ),
        (
            "GET",
            "/acme/../widgets.git/info/refs?service=git-upload-pack",
        ),
        (
            "GET",
            "/../acme/widgets.git/info/refs?service=git-upload-pack",
        ),
        ("POST", "/acme/widgets.git/../widgets.git/git-upload-pack"),
        ("POST", "/acme/./widgets.git/git-upload-pack"),
        ("POST", "/acme%2fwidgets.git/git-upload-pack"),
        ("POST", "/acme/widgets%2egit/git-upload-pack"),
        ("POST", "//acme/widgets.git/git-upload-pack"),
        ("POST", "/acme//widgets.git/git-upload-pack"),
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
        "a request outside the routes reached the upstream"
    );
}

#[tokio::test]
async fn a_declared_repository_reaches_the_upstream_under_its_base_path() {
    let (upstream, boundary) = git_boundary("git-declared").await;
    let pack: &[u8] =
        b"0098\x00\x01\x02 refs/heads/agent/x\x00report-status\n0000PACK\x00\x00\x00\x02\xff";
    let response = boundary
        .send(
            Request::post("/acme/widgets.git/git-receive-pack")
                .header("host", "agent-chosen.example")
                .header("content-type", "application/x-git-receive-pack-request")
                .header("accept", "application/x-git-receive-pack-result")
                .header("git-protocol", "version=2")
                .header("content-encoding", "gzip")
                .header("user-agent", "git/2.47.0")
                .header("authorization", "Basic agent-held-value")
                .header("cookie", "agent-cookie-value")
                .header("x-other", "1")
                .body(Full::new(Bytes::from_static(pack)))
                .expect("request"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received();
    assert_eq!(received.len(), 1);
    let request = &received[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/git/acme/widgets.git/git-receive-pack");
    assert_eq!(request.body, pack);
    assert_eq!(request.values("authorization"), bearer(TOKEN_ONE));
    assert_eq!(request.values("host"), vec![upstream.addr.to_string()]);
    assert_eq!(
        request.values("content-type"),
        vec!["application/x-git-receive-pack-request"]
    );
    assert_eq!(
        request.values("accept"),
        vec!["application/x-git-receive-pack-result"]
    );
    assert_eq!(request.values("git-protocol"), vec!["version=2"]);
    assert_eq!(request.values("content-encoding"), vec!["gzip"]);
    for dropped in ["user-agent", "cookie", "x-other"] {
        assert!(
            request.values(dropped).is_empty(),
            "{dropped} reached the upstream"
        );
    }

    let response = boundary
        .post("/acme/widgets.git/git-upload-pack", "0000")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = boundary
        .get("/acme/widgets.git/info/refs?service=git-receive-pack")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let targets: Vec<String> = upstream.received().into_iter().map(|r| r.target).collect();
    assert_eq!(
        targets,
        vec![
            "/git/acme/widgets.git/git-receive-pack",
            "/git/acme/widgets.git/git-upload-pack",
            "/git/acme/widgets.git/info/refs?service=git-receive-pack",
        ]
    );
    assert_eq!(
        boundary.logs(),
        vec![
            "git POST /acme/widgets.git/git-receive-pack 200",
            "git POST /acme/widgets.git/git-upload-pack 200",
            "git GET /acme/widgets.git/info/refs 200",
        ]
    );
}

// ---------------------------------------------------------------------------------------------
// The token file
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn a_rotated_token_file_is_read_on_the_next_request() {
    // The kubelet's projected-volume layout: `token` -> `..data/token`, `..data` -> a dated
    // directory, swapped atomically on rotation.
    let dir = scratch("token-rotation");
    fs::create_dir(dir.join("..v1")).expect("first generation");
    fs::write(dir.join("..v1/token"), TOKEN_ONE).expect("first token");
    std::os::unix::fs::symlink("..v1", dir.join("..data")).expect("..data");
    std::os::unix::fs::symlink("..data/token", dir.join("token")).expect("token link");

    let upstream = FakeUpstream::answering().await;
    let boundary = Boundary::start(Relay::model(upstream.upstream(), dir.join("token"))).await;
    assert_eq!(boundary.get("/v1/models").await.status(), StatusCode::OK);

    fs::create_dir(dir.join("..v2")).expect("second generation");
    fs::write(dir.join("..v2/token"), format!("{TOKEN_TWO}\n")).expect("second token");
    std::os::unix::fs::symlink("..v2", dir.join("..data_tmp")).expect("..data_tmp");
    fs::rename(dir.join("..data_tmp"), dir.join("..data")).expect("swap ..data");
    assert_eq!(boundary.get("/v1/models").await.status(), StatusCode::OK);

    let received = upstream.received();
    assert_eq!(received.len(), 2);
    assert_eq!(received[0].values("authorization"), bearer(TOKEN_ONE));
    assert_eq!(received[1].values("authorization"), bearer(TOKEN_TWO));
}

#[tokio::test]
async fn a_missing_token_file_fails_the_request_and_sends_nothing() {
    let dir = scratch("token-missing");
    let path = dir.join("token");
    let upstream = FakeUpstream::answering().await;
    let boundary = Boundary::start(Relay::model(upstream.upstream(), path.clone())).await;

    let response = boundary.get("/v1/models").await;
    assert_eq!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "missing"
    );
    let answer = String::from_utf8(body_bytes(response).await).expect("text");
    assert!(!answer.contains(TOKEN_ONE));

    for (case, contents) in [
        ("empty", b"".to_vec()),
        ("whitespace only", b" \n\t\r\n".to_vec()),
        ("an inner space", b"fixture token".to_vec()),
        ("a control byte", b"fixture\x01token".to_vec()),
        ("a non-ASCII byte", b"fixture\xc3\xa9token".to_vec()),
        ("over 64 KiB", vec![b'a'; 64 * 1024 + 1]),
    ] {
        write_token(&path, &contents);
        let response = boundary.get("/v1/models").await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{case}");
    }
    fs::remove_file(&path).expect("remove fixture");
    fs::create_dir(&path).expect("a directory in the token's place");
    let response = boundary.get("/v1/models").await;
    assert_eq!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "a directory"
    );
    fs::remove_dir(&path).expect("remove directory");

    assert!(
        upstream.received().is_empty(),
        "a request without a token reached the upstream"
    );
    assert!(
        boundary
            .logs()
            .iter()
            .all(|line| line == "model GET /v1/models 503"),
        "{:?}",
        boundary.logs()
    );

    // With a token in place the same relay forwards, so the refusals above were the token's.
    write_token(&path, format!("  {TOKEN_ONE}\r\n").as_bytes());
    assert_eq!(boundary.get("/v1/models").await.status(), StatusCode::OK);
    let received = upstream.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].values("authorization"), bearer(TOKEN_ONE));
}

// ---------------------------------------------------------------------------------------------
// Listening and configuration
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_boundary_listens_on_loopback_only() {
    assert_eq!(MODEL_PORT, 8401);
    assert_eq!(CONNECTORS_PORT, 8402);
    assert_eq!(GIT_PORT, 8403);
    let dir = scratch("listen");
    let token = token_file(&dir, TOKEN_ONE);
    let upstream = Upstream::parse("http://upstream.invalid:8080").expect("upstream");
    for address in [
        "0.0.0.0:0",
        "[::]:0",
        "192.0.2.10:0",
        "10.0.0.1:8401",
        "[2001:db8::1]:0",
    ] {
        let address: SocketAddr = address.parse().expect("address");
        let refused = bind(address, Relay::model(upstream.clone(), token.clone())).await;
        assert!(
            matches!(refused, Err(BindError::NotLoopback)),
            "{address} was not refused: {:?}",
            refused.map(|listener| listener.local_addr())
        );
    }
    let listener = bind(
        SocketAddr::from(([127, 0, 0, 1], 0)),
        Relay::model(upstream.clone(), token.clone()),
    )
    .await
    .expect("loopback is accepted");
    let bound = listener.local_addr();
    assert!(bound.ip().is_loopback(), "{bound}");
    assert_ne!(bound.port(), 0);

    let connectors =
        Relay::connectors(upstream.clone(), token.clone(), ["pipeline.get"]).expect("connectors");
    assert_eq!(
        (connectors.service(), connectors.port()),
        ("connectors", 8402)
    );
    let git = Relay::git(upstream, token, ["acme/widgets"]).expect("git");
    assert_eq!((git.service(), git.port()), ("git", 8403));
}

#[test]
fn an_upstream_that_is_not_plain_http_is_refused_without_repeating_it() {
    for (url, error) in [
        ("https://gateway.example", UpstreamError::Https),
        ("HTTPS://gateway.example", UpstreamError::Https),
        ("ftp://gateway.example", UpstreamError::NotHttp),
        ("gateway.example:8080", UpstreamError::NotHttp),
        (
            "http://agent:fixture-userinfo-value@gateway.example",
            UpstreamError::UserInfo,
        ),
        (
            "http://fixture-userinfo-value@gateway.example",
            UpstreamError::UserInfo,
        ),
        (
            "http://gateway.example/v1?key=fixture-query-value",
            UpstreamError::Query,
        ),
        (
            "http://gateway.example/v1#fixture-fragment-value",
            UpstreamError::Fragment,
        ),
        ("http://", UpstreamError::Host),
        ("http:///v1", UpstreamError::Host),
        ("http://gateway.example:0", UpstreamError::Port),
        ("http://gateway.example:65536", UpstreamError::Port),
        ("http://gateway.example:x", UpstreamError::Port),
        ("http://gate way.example", UpstreamError::Host),
        ("http://gateway.example/a b", UpstreamError::Path),
        ("http://gateway.example/a/../b", UpstreamError::Path),
        ("http://gateway.example/a/%2e%2e/b", UpstreamError::Path),
    ] {
        let refused = Upstream::parse(url).expect_err(url);
        assert_eq!(refused, error, "{url}");
        let said = refused.to_string();
        assert!(!said.is_empty());
        for value in [
            "fixture-userinfo-value",
            "fixture-query-value",
            "fixture-fragment-value",
            "gateway.example",
        ] {
            assert!(!said.contains(value), "{url}: {said}");
        }
    }
    for url in [
        "http://gateway.example",
        "http://gateway.example/",
        "HTTP://gateway.example:8080/base/",
        "http://10.96.0.12:8080",
        "http://[fd00::12]:8080/git",
        "http://llm-gateway.agent-credentials.svc.cluster.local:8080",
    ] {
        assert!(Upstream::parse(url).is_ok(), "{url}");
    }
}

#[test]
fn a_declaration_out_of_shape_is_refused() {
    let upstream = Upstream::parse("http://upstream.invalid").expect("upstream");
    let token = PathBuf::from("token");
    assert!(Relay::connectors(upstream.clone(), token.clone(), Vec::<String>::new()).is_err());
    assert!(Relay::connectors(upstream.clone(), token.clone(), [""]).is_err());
    assert!(Relay::connectors(upstream.clone(), token.clone(), ["merge request"]).is_err());
    assert!(Relay::git(upstream.clone(), token.clone(), Vec::<String>::new()).is_err());
    for repository in [
        "acme",
        "acme/",
        "/widgets",
        "acme/widgets/extra",
        "../widgets",
        "acme/..",
        "acme/.",
        "acme/widgets.git",
        "acme/wid gets",
        "acme/wid%2fgets",
    ] {
        assert!(
            Relay::git(upstream.clone(), token.clone(), [repository]).is_err(),
            "{repository}"
        );
    }
    assert!(Relay::git(upstream, token, ["acme/widgets", "acme-2/w.i_d-g"]).is_ok());
}
