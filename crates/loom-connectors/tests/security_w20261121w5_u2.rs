//! Security review, pass 1, of `story:connectors-relay-client` (wave 2026-11-21-w5, unit U2):
//! conformance checks of `loom_connectors::relay::ConnectorsRelay` against its own contract. The
//! URL rule admits `http://127.0.0.1:<port>` and nothing else; every request goes to that URL
//! only (no redirect, no proxy, nothing an answer says moves it); no credential is sent or read;
//! a call of another pair sends nothing; an answer that is not the boundary's or a valid
//! Connectors one never reads as "nothing was sent" for a write; the connect bound ends a connect
//! that never completes; the runtime outlives a dropped clone and a call inside another runtime.
//!
//! Every listener binds `127.0.0.1:0` in this process and keeps its socket; no other address is
//! reached, and no file but the test's own is read.

use std::io::{BufRead, BufReader, Read, Write};
use std::iter::repeat_n;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeProposedAction, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_connectors::cli::datasource::{
    AdapterAlias, ConnectionId, DataSource, MutationClassification, OperationId, ReadKind,
    SourceName, WriteApproval, WriteOperation, WriteResult,
};
use b10x_loom_connectors::cli::{CliError, WriteError, idempotency_key, json};
use b10x_loom_connectors::relay::{ConnectorsRelay, RelayUrlRefused, loopback_url};
use connectors_core::RESPONSE_LIMIT;
use connectors_core::v1alpha2::{
    AttemptId, AttemptReference, AuditStatus, EffectKnowledge, Error as WireError, ErrorCode,
    InvokeResponse, MutationObservation, ResponseStatus, Version,
};
use serde_json::json;

const ADAPTER: &str = "desk";
const CONNECTION: &str = "conn-desk";
const OTHER_ADAPTER: &str = "mail";
const OTHER_CONNECTION: &str = "conn-other";
const LIST: &str = "tickets.list";
const GET: &str = "tickets.get";
const ROUTE: &str = "ticket.route";
const INSTANCE: &str = "desk-support";
const REVISION: &str = "rev-3";
const ATTEMPT: &str = "00000000-0000-4000-8000-0000000000a1";
const NOW: &str = "2026-11-21T09:00:00Z";
/// A fixture token placed in the child's environment; no request may carry it.
const TOKEN: &str = "fixture-sec-token-5c81e0aa";
/// A fixture cookie the fake sets; no request may send it back.
const COOKIE: &str = "fixture-sec-cookie-93b2";
/// Set in the child process of [`proxy_and_credential_variables_change_nothing`].
const CHILD: &str = "LOOM_RELAY_SEC_W5_U2_CHILD";

// ---------------------------------------------------------------------------------------------
// A loopback fake that answers each request with the bytes a test scripts.

#[derive(Debug, Clone)]
struct Received {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    raw: Vec<u8>,
}

impl Received {
    fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).expect("the invoke body is JSON")
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(named, _)| named == name)
            .map(|(_, value)| value.as_str())
    }
}

/// What the fake does with one request.
enum Act {
    /// These bytes, then the connection closes.
    Raw(Vec<u8>),
    /// The request is read and the connection closes with no answer.
    Lost,
    /// A 200 with a chunked body that does not end (up to 256 MiB, or until the peer closes).
    Endless,
    /// A 200 whose `content-length` is one byte past `RESPONSE_LIMIT`.
    Oversized,
}

type Handler = Arc<dyn Fn(&Received) -> Act + Send + Sync>;

struct Fake {
    addr: SocketAddr,
    received: Arc<Mutex<Vec<Received>>>,
    connections: Arc<AtomicUsize>,
    written: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Fake {
    fn start(handler: impl Fn(&Received) -> Act + Send + Sync + 'static) -> Self {
        let handler: Handler = Arc::new(handler);
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let addr = listener.local_addr().expect("the bound address");
        let received = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::new(AtomicUsize::new(0));
        let written = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let received = Arc::clone(&received);
            let connections = Arc::clone(&connections);
            let written = Arc::clone(&written);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        return;
                    }
                    let Ok(stream) = stream else { continue };
                    connections.fetch_add(1, Ordering::SeqCst);
                    let handler = Arc::clone(&handler);
                    let received = Arc::clone(&received);
                    let written = Arc::clone(&written);
                    std::thread::spawn(move || {
                        let Some((request, stream)) = read_request(stream) else {
                            return;
                        };
                        received
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .push(request.clone());
                        perform(stream, handler(&request), &written);
                    });
                }
            })
        };
        Self {
            addr,
            received,
            connections,
            written,
            stop,
            thread: Some(thread),
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.addr.port())
    }

    fn received(&self) -> Vec<Received> {
        self.received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn describes(&self) -> usize {
        self.received()
            .iter()
            .filter(|r| r.method == "GET" && r.path == "/v1/describe")
            .count()
    }

    fn invocations(&self) -> Vec<Received> {
        self.received()
            .into_iter()
            .filter(|r| r.method == "POST" && r.path == "/v1alpha2/invoke")
            .collect()
    }

    fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    fn relay(&self, sources: Vec<DataSource>) -> ConnectorsRelay {
        ConnectorsRelay::new(&self.url(), adapter(), connection(), sources)
            .expect("a loopback relay URL")
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn read_request(stream: TcpStream) -> Option<(Received, TcpStream)> {
    let mut reader = BufReader::new(stream);
    let mut raw = Vec::new();
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
        return None;
    }
    raw.extend_from_slice(line.as_bytes());
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();
    let mut headers = Vec::new();
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            return None;
        }
        raw.extend_from_slice(header.as_bytes());
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim().to_owned();
            if name == "content-length" {
                length = value.parse().unwrap_or(0);
            }
            headers.push((name, value));
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    raw.extend_from_slice(&body);
    Some((
        Received {
            method,
            path,
            headers,
            body,
            raw,
        },
        reader.into_inner(),
    ))
}

fn perform(mut stream: TcpStream, act: Act, written: &AtomicUsize) {
    match act {
        Act::Raw(bytes) => {
            let _ = stream.write_all(&bytes);
            let _ = stream.flush();
        }
        Act::Lost => drop(stream),
        Act::Endless => {
            let head = "HTTP/1.1 200 X\r\ncontent-type: application/json\r\n\
                        transfer-encoding: chunked\r\nconnection: close\r\n\r\n";
            if stream.write_all(head.as_bytes()).is_err() {
                return;
            }
            let chunk = vec![b' '; 65536];
            for _ in 0..4096 {
                let sent = stream
                    .write_all(b"10000\r\n")
                    .and_then(|()| stream.write_all(&chunk))
                    .and_then(|()| stream.write_all(b"\r\n"));
                if sent.is_err() {
                    return;
                }
                written.fetch_add(chunk.len(), Ordering::SeqCst);
            }
        }
        Act::Oversized => {
            let head = format!(
                "HTTP/1.1 200 X\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
                 connection: close\r\n\r\n",
                RESPONSE_LIMIT + 1
            );
            if stream.write_all(head.as_bytes()).is_err() {
                return;
            }
            let chunk = vec![b' '; 65536];
            let mut left = RESPONSE_LIMIT + 1;
            while left > 0 {
                let part = left.min(chunk.len());
                if stream.write_all(&chunk[..part]).is_err() {
                    return;
                }
                written.fetch_add(part, Ordering::SeqCst);
                left -= part;
            }
        }
    }
}

/// One HTTP/1.1 answer with `connection: close`.
fn response(status: u16, extra: &[(&str, &str)], body: &str) -> Vec<u8> {
    let mut head = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
         connection: close\r\n",
        body.len()
    );
    for (name, value) in extra {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let mut bytes = head.into_bytes();
    bytes.extend_from_slice(body.as_bytes());
    bytes
}

fn http(status: u16, extra: &[(&str, &str)], body: &str) -> Act {
    Act::Raw(response(status, extra, body))
}

fn operation(id: &str, profile: &str) -> serde_json::Value {
    json!({
        "id": id,
        "description": id,
        "contract": "operations/v1alpha1",
        "profile": profile,
        "input_schema": {"type": "object"},
        "output_schema": {"type": "object"}
    })
}

fn descriptor() -> String {
    json!({
        "version": "v1alpha1",
        "instance": INSTANCE,
        "adapter": "desk-adapter",
        "revision": REVISION,
        "operations": [
            operation(LIST, "read"),
            operation(GET, "read"),
            operation(ROUTE, "mutation")
        ],
        "configuration_schema": {"type": "object"}
    })
    .to_string()
}

/// The describe answered with [`descriptor`], every invoke with what `invoke` gives for it.
fn serve(invoke: impl Fn(&Received) -> Act + Send + Sync + 'static) -> Fake {
    Fake::start(
        move |request| match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/v1/describe") => http(200, &[], &descriptor()),
            ("POST", "/v1alpha2/invoke") => invoke(request),
            _ => http(404, &[], ""),
        },
    )
}

fn request_id(request: &Received) -> String {
    request.json()["request_id"]
        .as_str()
        .expect("the invoke carries a request id")
        .to_owned()
}

fn observed(classification: EffectKnowledge, request_id: &str) -> MutationObservation {
    MutationObservation {
        classification,
        attempt: Some(AttemptReference {
            instance: INSTANCE.to_owned(),
            id: AttemptId::parse(ATTEMPT).expect("a UUID"),
        }),
        original_request_id: Some(request_id.to_owned()),
        replayed: false,
        cause: None,
    }
}

fn success(
    request_id: &str,
    result: serde_json::Value,
    mutation: Option<MutationObservation>,
) -> InvokeResponse {
    InvokeResponse {
        version: Version::V1alpha2,
        request_id: Some(request_id.to_owned()),
        status: ResponseStatus::Success,
        result: Some(result),
        error: None,
        audit_ref: Some("aud-1".to_owned()),
        audit_status: AuditStatus::Complete,
        source_audit: None,
        mutation,
    }
}

fn failure(request_id: Option<&str>, code: ErrorCode) -> InvokeResponse {
    InvokeResponse {
        version: Version::V1alpha2,
        request_id: request_id.map(str::to_owned),
        status: ResponseStatus::Error,
        result: None,
        error: Some(WireError::new(code, "the fake host says so")),
        audit_ref: None,
        audit_status: AuditStatus::Unavailable,
        source_audit: None,
        mutation: None,
    }
}

fn read_ok(request: &Received) -> Act {
    let bytes = success(&request_id(request), json!({"tickets": [1, 2]}), None)
        .encode()
        .expect("the unkeyed binding writes it");
    http(200, &[], &String::from_utf8(bytes).expect("UTF-8"))
}

fn write_ok(request: &Received) -> Act {
    let id = request_id(request);
    let bytes = success(
        &id,
        json!({"routed": true}),
        Some(observed(EffectKnowledge::Applied, &id)),
    )
    .encode_keyed()
    .expect("the keyed binding writes it");
    http(200, &[], &String::from_utf8(bytes).expect("UTF-8"))
}

/// A write answered as applied, a read as a success.
fn either(request: &Received) -> Act {
    if request.json().get("idempotency_key").is_some() {
        write_ok(request)
    } else {
        read_ok(request)
    }
}

// ---------------------------------------------------------------------------------------------
// The client's inputs and outcomes.

fn adapter() -> AdapterAlias {
    AdapterAlias(ADAPTER.to_owned())
}

fn connection() -> ConnectionId {
    ConnectionId(CONNECTION.to_owned())
}

fn source_of(adapter: &str, connection: &str) -> DataSource {
    DataSource {
        name: SourceName("tickets".to_owned()),
        adapter: AdapterAlias(adapter.to_owned()),
        connection: ConnectionId(connection.to_owned()),
        list: Some(OperationId(LIST.to_owned())),
        search: None,
        get: Some(OperationId(GET.to_owned())),
    }
}

fn source() -> DataSource {
    source_of(ADAPTER, CONNECTION)
}

fn read_input() -> json::Value {
    json::parse(r#"{"limit":5}"#).unwrap()
}

fn route() -> WriteOperation {
    WriteOperation {
        action: ROUTE.to_owned(),
        adapter: adapter(),
        connection: connection(),
        operation: OperationId(ROUTE.to_owned()),
    }
}

fn input() -> json::Value {
    json::parse(r#"{"ticket":"48213","queue":"billing"}"#).unwrap()
}

fn proof() -> String {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("security_w20261121w5_u2/route.proof")
        .display()
        .to_string()
}

fn approval() -> WriteApproval {
    WriteApproval {
        proof_file: proof(),
    }
}

fn read(
    relay: &ConnectorsRelay,
) -> Result<b10x_loom_connectors::cli::datasource::ReadResult, CliError> {
    relay.invoke_read(
        &adapter(),
        &connection(),
        &OperationId(LIST.to_owned()),
        &read_input(),
    )
}

fn write(relay: &ConnectorsRelay, request: &AdmittedRequest) -> Result<WriteResult, WriteError> {
    relay.invoke_write(request, &route(), &input(), &approval())
}

fn refused_code<T: std::fmt::Debug>(result: Result<T, CliError>) -> String {
    match result {
        Err(CliError::Refused(refusal)) => refusal.code,
        other => panic!("expected CliError::Refused, got {other:?}"),
    }
}

fn failed<T: std::fmt::Debug>(result: Result<T, CliError>) -> String {
    match result {
        Err(CliError::Failed(message)) => message,
        other => panic!("expected CliError::Failed, got {other:?}"),
    }
}

fn write_refused_code(result: Result<WriteResult, WriteError>) -> String {
    match result {
        Err(WriteError::Refused(refusal)) => refusal.code,
        other => panic!("expected WriteError::Refused, got {other:?}"),
    }
}

fn write_failed(result: Result<WriteResult, WriteError>) -> String {
    match result {
        Err(WriteError::Failed(message)) => message,
        other => panic!("expected WriteError::Failed, got {other:?}"),
    }
}

fn written(result: Result<WriteResult, WriteError>) -> WriteResult {
    result.unwrap_or_else(|error| panic!("expected a WriteResult, got {error:?}"))
}

fn starts(message: &str, prefix: &str) {
    assert!(
        message.starts_with(prefix),
        "`{message}` does not start with `{prefix}`"
    );
}

/// `loom-relay-` and 32 lowercase hex digits.
fn is_request_id(id: &str) -> bool {
    id.strip_prefix("loom-relay-").is_some_and(|hex| {
        hex.len() == 32
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

// ---------------------------------------------------------------------------------------------
// The admitted request, made by the runtime (as `tests/connectors_relay.rs` makes it).

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Context {
    requests: u64,
    observations: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x400 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(NOW.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

#[derive(Default)]
struct Keep(Mutex<Vec<AdmittedRequest>>);

impl EffectPort for Keep {
    fn performs(&self, action: &str) -> bool {
        action == ROUTE
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: commission_json::Value::Null,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

fn admitted() -> AdmittedRequest {
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x81)),
        agent_revision_id: AgentRevisionId(uuid(0x90)),
        case_id: CaseId("case-route".to_owned()),
        principal: PrincipalId("principal-desk".to_owned()),
        authority_context: AuthorityContext(commission_json::Value::Null),
    });
    let open = |revision| {
        Answer::at(revision).with_items(
            Vec::new(),
            Vec::new(),
            vec![FrontierAction {
                action: ROUTE.to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            }],
        )
    };
    let governor = FakeGovernor::new();
    governor.script(
        commission.data().case_id.clone(),
        repeat_n(open(3), 5).chain([open(4), open(4).complete("routed")]),
    );
    let executor = ScriptedExecutor::new(vec![ExecutorOutcome::ProposedAction(
        ExecutorOutcomeProposedAction {
            action: ROUTE.to_owned(),
            arguments: ProposedActionArguments(
                commission_json::parse(r#"{"ticket":"48213","queue":"billing"}"#).unwrap(),
            ),
        },
    )]);
    let keep = Keep::default();
    let mut issued = 0u64;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));
    let _ = run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &keep,
        &commission,
        &mut runs,
        &mut Context {
            requests: 0,
            observations: 0,
        },
    );
    let mut kept = keep.0.into_inner().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(kept.len(), 1, "the runtime admits the route once");
    kept.remove(0)
}

// ---------------------------------------------------------------------------------------------
// Invariant 1: the URL rule.

/// Every other spelling of a loopback address, every other scheme, user information, a path,
/// query or fragment, a port out of range or absent, white space, control characters and a
/// second URL after a separator: `loopback_url` and `new` refuse each, and the refusal is the
/// rule's text exactly, repeating no part of the value.
#[test]
fn every_other_spelling_of_a_relay_url_is_refused_naming_only_the_rule() {
    let refused = [
        "",
        "http://localhost:8402",
        "http://localhost.:8402",
        "http://127.0.0.1.:8402",
        "http://127.1:8402",
        "http://127.0.1:8402",
        "http://127.0.0.2:8402",
        "http://0.0.0.0:8402",
        "http://0:8402",
        "http://[::1]:8402",
        "http://[::ffff:127.0.0.1]:8402",
        "http://[::ffff:7f00:1]:8402",
        "http://[fe80::1%25lo]:8402",
        "http://[::1%lo]:8402",
        "http://2130706433:8402",
        "http://0x7f000001:8402",
        "http://0x7f.0.0.1:8402",
        "http://0177.0.0.1:8402",
        "http://127.000.000.001:8402",
        "http://u@127.0.0.1:8402",
        "http://u:p@127.0.0.1:8402",
        "http://:p@127.0.0.1:8402",
        "http://127.0.0.1:8402@10.0.0.1:80",
        "http://127.0.0.1:8402/",
        "http://127.0.0.1:8402/v1/describe",
        "http://127.0.0.1:8402?",
        "http://127.0.0.1:8402?q=1",
        "http://127.0.0.1:8402#",
        "http://127.0.0.1:8402#f",
        "http://127.0.0.1:8402%2F",
        "http://127.0.0.1:8402\\@10.0.0.1",
        "https://127.0.0.1:8402",
        "ftp://127.0.0.1:8402",
        "ws://127.0.0.1:8402",
        "unix://127.0.0.1:8402",
        "HTTP://127.0.0.1:8402",
        "Http://127.0.0.1:8402",
        "http:/127.0.0.1:8402",
        "http:\\\\127.0.0.1:8402",
        "//127.0.0.1:8402",
        "127.0.0.1:8402",
        "http://127.0.0.1",
        "http://127.0.0.1:",
        "http://127.0.0.1:0",
        "http://127.0.0.1:00",
        "http://127.0.0.1:08402",
        "http://127.0.0.1:65536",
        "http://127.0.0.1:99999",
        "http://127.0.0.1:123456",
        "http://127.0.0.1:+8402",
        "http://127.0.0.1:-1",
        "http://127.0.0.1:1e3",
        "http://127.0.0.1:8402:8403",
        "http://127.0.0.1:\u{ff18}\u{ff14}\u{ff10}\u{ff12}",
        "http://127.0.0.1:\u{668}\u{664}\u{660}\u{662}",
        " http://127.0.0.1:8402",
        "http://127.0.0.1:8402 ",
        "http://127.0.0.1:8402\n",
        "http://127.0.0.1:8402\r\n",
        "http://127.0.0.1:8402\t",
        "http://127.0.0.1:84\u{0}02",
        "http://127.0.0.1:8402\u{0}",
        "http://127.0.0.1:8402\u{7f}",
        "http://127.0.0.1:8402,http://10.0.0.1:80",
        "http://127.0.0.1:8402 http://10.0.0.1:80",
        "http://127.0.0.1:8402;http://10.0.0.1:80",
        "http://127.0.0.1:8402\nhttp://10.0.0.1:80",
    ];
    for value in refused {
        let by_rule = loopback_url(value);
        assert!(by_rule.is_err(), "loopback_url admits {value:?}");
        let by_client = ConnectorsRelay::new(value, adapter(), connection(), vec![]);
        assert!(by_client.is_err(), "ConnectorsRelay::new admits {value:?}");
        let refusal = by_rule.expect_err("refused");
        assert_eq!(refusal.to_string(), RelayUrlRefused::RULE, "{value:?}");
        assert_eq!(
            by_client.expect_err("refused").to_string(),
            RelayUrlRefused::RULE,
            "{value:?}"
        );
    }
    for (value, port) in [
        ("http://127.0.0.1:1", 1),
        ("http://127.0.0.1:65535", 65535),
        ("http://127.0.0.1:8402", 8402),
    ] {
        let url = loopback_url(value).unwrap_or_else(|_| panic!("{value} is refused"));
        assert_eq!(url.port(), port);
        assert_eq!(url.to_string(), value);
    }
}

// ---------------------------------------------------------------------------------------------
// Invariant 2: every request goes to the relay URL and nowhere else.

/// Every 3xx, with a `location` naming another loopback listener that answers correctly and with
/// none, at the describe and at the invoke, for a read and a write: never followed. A write's
/// invoke answered with a 3xx is `Unknown`, sent once.
#[test]
fn no_3xx_is_followed_with_or_without_a_location() {
    let elsewhere = serve(either);
    let request = admitted();
    for status in [300u16, 301, 302, 303, 304, 305, 306, 307, 308, 399] {
        for location in [
            Some(format!("{}/v1/describe", elsewhere.url())),
            Some(format!("{}/v1alpha2/invoke", elsewhere.url())),
            None,
        ] {
            let redirect = move || match &location {
                Some(location) => http(status, &[("location", location.as_str())], ""),
                None => http(status, &[], ""),
            };
            let at_describe = {
                let redirect = redirect.clone();
                move || {
                    let redirect = redirect.clone();
                    Fake::start(move |request| match request.path.as_str() {
                        "/v1/describe" => redirect(),
                        _ => either(request),
                    })
                }
            };
            let fake = at_describe();
            starts(&failed(read(&fake.relay(vec![]))), "upstream-protocol:");
            assert!(fake.invocations().is_empty(), "{status}");
            let fake = at_describe();
            starts(
                &write_failed(write(&fake.relay(vec![]), &request)),
                "upstream-protocol:",
            );
            assert!(fake.invocations().is_empty(), "{status}");

            let at_invoke = {
                let redirect = redirect.clone();
                move || {
                    let redirect = redirect.clone();
                    serve(move |_| redirect())
                }
            };
            let fake = at_invoke();
            starts(&failed(read(&fake.relay(vec![]))), "upstream-protocol:");
            assert_eq!(fake.invocations().len(), 1, "{status}");
            let fake = at_invoke();
            let result = written(write(&fake.relay(vec![]), &request));
            assert_eq!(
                result.classification,
                MutationClassification::Unknown,
                "{status}"
            );
            assert_eq!(fake.invocations().len(), 1, "{status}");
        }
    }
    assert_eq!(elsewhere.connections(), 0, "a redirect was followed");
}

/// A `set-cookie` in every answer is never sent back on a later request of the same client.
#[test]
fn a_set_cookie_is_never_sent_back() {
    let cookie = format!("relay_session={COOKIE}; Path=/");
    let fake = Fake::start(move |request| {
        let cookie = [("set-cookie", cookie.as_str())];
        match request.path.as_str() {
            "/v1/describe" => http(200, &cookie, &descriptor()),
            _ => match either(request) {
                Act::Raw(bytes) => {
                    let text = String::from_utf8(bytes).expect("UTF-8");
                    let (head, body) = text.split_once("\r\n\r\n").expect("a head");
                    Act::Raw(
                        format!("{head}\r\nset-cookie: {}\r\n\r\n{body}", cookie[0].1).into_bytes(),
                    )
                }
                other => other,
            },
        }
    });
    let relay = fake.relay(vec![source()]);
    let request = admitted();
    read(&relay).expect("the read answers");
    written(write(&relay, &request));
    read(&relay).expect("the read answers");
    let received = fake.received();
    assert_eq!(received.len(), 6, "three describes and three invokes");
    for request in &received {
        assert_eq!(
            request.header("cookie"),
            None,
            "{} {}",
            request.method,
            request.path
        );
        assert!(
            !String::from_utf8_lossy(&request.raw).contains(COOKIE),
            "{} {} sends the cookie back",
            request.method,
            request.path
        );
    }
}

/// In a child whose environment sets every proxy variable (`HTTP_PROXY`, `http_proxy`,
/// `HTTPS_PROXY`, `https_proxy`, `ALL_PROXY`, `all_proxy`, credentials included) to a second
/// loopback listener, a `NO_PROXY` that does not cover the relay, and credential-like variables
/// naming the fixture token: a read and a write answer from the relay, no request carries a
/// credential header or the token, and the proxy sees no connection.
#[test]
fn proxy_and_credential_variables_change_nothing() {
    let fake = serve(either);
    let relay = fake.relay(vec![source()]);
    read(&relay).expect("the read answers");
    assert_eq!(
        written(write(&relay, &admitted())).classification,
        MutationClassification::Applied
    );
    for request in fake.received() {
        for header in [
            "authorization",
            "proxy-authorization",
            "x-api-key",
            "cookie",
        ] {
            assert_eq!(
                request.header(header),
                None,
                "{} carries `{header}`",
                request.path
            );
        }
        assert!(
            !String::from_utf8_lossy(&request.raw).contains(TOKEN),
            "{} carries the token",
            request.path
        );
    }
    if std::env::var_os(CHILD).is_some() {
        return;
    }
    let proxy = Fake::start(|_| Act::Lost);
    let proxied = format!("http://relay:{TOKEN}@127.0.0.1:{}", proxy.addr.port());
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child
        .args([
            "proxy_and_credential_variables_change_nothing",
            "--exact",
            "--test-threads=1",
        ])
        .env(CHILD, "1")
        .env_remove("REQUEST_METHOD");
    for name in [
        "HTTP_PROXY",
        "http_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "all_proxy",
    ] {
        child.env(name, &proxied);
    }
    for name in ["NO_PROXY", "no_proxy"] {
        child.env(name, "example.invalid");
    }
    for name in [
        "CONNECTORS_TOKEN",
        "CONNECTORS_API_KEY",
        "CONNECTORS_SERVICE_CREDENTIAL",
        "LOOM_CONNECTORS_TOKEN",
        "LOOM_RELAY_TOKEN",
        "LOOM_BOUNDARY_TOKEN",
        "BOUNDARY_TOKEN",
        "BEARER_TOKEN",
        "AUTHORIZATION",
    ] {
        child.env(name, TOKEN);
    }
    let output = child.output().expect("the test binary runs again");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "the child failed: {stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("1 passed"),
        "the child ran the case: {stdout}"
    );
    assert_eq!(proxy.connections(), 0, "a request went through the proxy");
}

// ---------------------------------------------------------------------------------------------
// Invariant 4: one pair.

/// Each member that takes a pair, named with another adapter, another connection, or both: each
/// `relay-other-instance`, and the fake sees no connection at all.
#[test]
fn every_member_naming_another_adapter_or_connection_sends_nothing() {
    let fake = serve(either);
    let relay = fake.relay(vec![source()]);
    let request = admitted();
    let pairs = [
        (OTHER_ADAPTER, CONNECTION),
        (ADAPTER, OTHER_CONNECTION),
        (OTHER_ADAPTER, OTHER_CONNECTION),
    ];
    for (other_adapter, other_connection) in pairs {
        let named = format!("{other_adapter}/{other_connection}");
        let alias = AdapterAlias(other_adapter.to_owned());
        let id = ConnectionId(other_connection.to_owned());
        assert_eq!(
            refused_code(relay.invoke_read(
                &alias,
                &id,
                &OperationId(LIST.to_owned()),
                &read_input()
            )),
            "relay-other-instance",
            "invoke_read {named}"
        );
        assert_eq!(
            refused_code(relay.read(
                &source_of(other_adapter, other_connection),
                ReadKind::List,
                &read_input()
            )),
            "relay-other-instance",
            "read {named}"
        );
        assert_eq!(
            refused_code(
                fake.relay(vec![source_of(other_adapter, other_connection)])
                    .sources()
            ),
            "relay-other-instance",
            "sources {named}"
        );
        assert_eq!(
            refused_code(
                relay
                    .over(vec![source(), source_of(other_adapter, other_connection)])
                    .sources()
            ),
            "relay-other-instance",
            "sources over {named}"
        );
        let operation = WriteOperation {
            adapter: alias.clone(),
            connection: id.clone(),
            ..route()
        };
        assert_eq!(
            write_refused_code(relay.invoke_write(&request, &operation, &input(), &approval())),
            "relay-other-instance",
            "invoke_write {named}"
        );
        if other_adapter != ADAPTER {
            assert_eq!(
                refused_code(relay.describe(&alias, &OperationId(LIST.to_owned()))),
                "relay-other-instance",
                "describe {named}"
            );
        }
    }
    assert!(fake.received().is_empty());
    assert_eq!(fake.connections(), 0);
}

// ---------------------------------------------------------------------------------------------
// Invariant 5: writes.

/// A write of an operation of the pair the describe does not list: refused `not_found` after
/// the describe, and no invoke is sent.
#[test]
fn a_write_of_an_operation_the_describe_does_not_list_sends_no_invoke() {
    let fake = serve(either);
    let operation = WriteOperation {
        operation: OperationId("ticket.unlisted".to_owned()),
        ..route()
    };
    let code = write_refused_code(fake.relay(vec![]).invoke_write(
        &admitted(),
        &operation,
        &input(),
        &approval(),
    ));
    assert_eq!(code, "not_found");
    assert_eq!(fake.describes(), 1);
    assert!(fake.invocations().is_empty());
}

/// The same admitted write twice: both invokes carry the one key of the admitted request, each
/// its own request id; the request id is no idempotency key.
#[test]
fn the_same_admitted_write_twice_carries_one_key_and_two_request_ids() {
    let fake = serve(write_ok);
    let relay = fake.relay(vec![]);
    let request = admitted();
    written(write(&relay, &request));
    written(write(&relay, &request));
    let invocations = fake.invocations();
    assert_eq!(invocations.len(), 2);
    let key = json!(idempotency_key(&request));
    assert_eq!(invocations[0].json()["idempotency_key"], key);
    assert_eq!(invocations[1].json()["idempotency_key"], key);
    let first = request_id(&invocations[0]);
    let second = request_id(&invocations[1]);
    assert_ne!(first, second);
    assert!(
        is_request_id(&first) && is_request_id(&second),
        "{first} {second}"
    );
}

// ---------------------------------------------------------------------------------------------
// Invariant 6: answers.

/// How the fake answers one invoke.
type Answering = fn(&Received) -> Act;

/// Bodies at the invoke that are neither the boundary's own body at its code's status nor a valid
/// correlated Connectors answer.
fn not_classified_answers() -> Vec<(&'static str, Answering)> {
    let answers: [(&'static str, Answering); 13] = [
        ("a boundary body with a third member", |_: &Received| {
            http(
                503,
                &[],
                r#"{"error":"token_unavailable","message":"m","detail":"x"}"#,
            )
        }),
        ("a boundary code at another status", |_: &Received| {
            http(502, &[], r#"{"error":"token_unavailable","message":"m"}"#)
        }),
        (
            "a boundary body whose message is no text",
            |_: &Received| http(503, &[], r#"{"error":"token_unavailable","message":7}"#),
        ),
        ("a boundary body naming its error twice", |_: &Received| {
            http(
                403,
                &[],
                r#"{"error":"upstream_failed","message":"m","error":"operation_refused"}"#,
            )
        }),
        ("a boundary code in upper case", |_: &Received| {
            http(404, &[], r#"{"error":"NOT_RELAYED","message":"m"}"#)
        }),
        ("a boundary body in a 2xx", |_: &Received| {
            http(200, &[], r#"{"error":"token_unavailable","message":"m"}"#)
        }),
        ("a refusal body at 200", |request: &Received| {
            let body = serde_json::to_string(&failure(
                Some(&request_id(request)),
                ErrorCode::Unauthorized,
            ))
            .expect("serializable");
            http(200, &[], &body)
        }),
        (
            "a success body at a refusal status",
            |request: &Received| {
                let id = request_id(request);
                let body = serde_json::to_string(&success(
                    &id,
                    json!({}),
                    Some(observed(EffectKnowledge::Applied, &id)),
                ))
                .expect("serializable");
                http(401, &[], &body)
            },
        ),
        (
            "a success body with an unknown member",
            |request: &Received| {
                let id = request_id(request);
                let mut body = serde_json::to_value(success(
                    &id,
                    json!({}),
                    Some(observed(EffectKnowledge::Applied, &id)),
                ))
                .expect("serializable");
                body["extra"] = json!(1);
                http(200, &[], &body.to_string())
            },
        ),
        ("a truncated body", |request: &Received| {
            let id = request_id(request);
            let body = serde_json::to_string(&success(
                &id,
                json!({}),
                Some(observed(EffectKnowledge::Applied, &id)),
            ))
            .expect("serializable");
            let mut bytes = format!(
                "HTTP/1.1 200 X\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
                 connection: close\r\n\r\n",
                body.len()
            )
            .into_bytes();
            bytes.extend_from_slice(&body.as_bytes()[..body.len() / 2]);
            Act::Raw(bytes)
        }),
        (
            "a body past the response limit by its length",
            |_: &Received| Act::Oversized,
        ),
        (
            "a body of another content type that is no JSON",
            |_: &Received| {
                Act::Raw(
                    b"HTTP/1.1 200 X\r\ncontent-type: text/html\r\ncontent-length: 13\r\n\
                  connection: close\r\n\r\n<html></html>"
                        .to_vec(),
                )
            },
        ),
        ("no answer", |_: &Received| Act::Lost),
    ];
    answers.to_vec()
}

/// Each such answer at a write's invoke is `Unknown`, never `Refused` (which says nothing was
/// sent), with no request id and no code, sent once; at a read's invoke `upstream-protocol`.
#[test]
fn an_answer_that_is_neither_the_boundarys_nor_connectors_never_reads_as_not_sent() {
    let request = admitted();
    for (name, answer) in not_classified_answers() {
        let fake = serve(answer);
        let result = write(&fake.relay(vec![]), &request);
        assert!(
            matches!(&result, Ok(result) if result.classification == MutationClassification::Unknown
                && result.request_id.is_none()
                && result.code.is_none()),
            "{name}: {result:?}"
        );
        assert_eq!(fake.invocations().len(), 1, "{name}: sent once");

        let fake = serve(answer);
        let message = failed(read(&fake.relay(vec![])));
        assert!(
            message.starts_with("upstream-protocol:"),
            "{name}: {message}"
        );
        assert_eq!(fake.invocations().len(), 1, "{name}: sent once");
    }
}

/// A loopback peer that streams a body without end: the client stops at `RESPONSE_LIMIT` and
/// well inside the request bound; the write is `Unknown`, the read `upstream-protocol`.
#[test]
fn a_body_that_never_ends_is_cut_at_the_response_limit() {
    let fake = serve(|_| Act::Endless);
    let started = Instant::now();
    let message = failed(read(&fake.relay(vec![])));
    let elapsed = started.elapsed();
    starts(&message, "upstream-protocol:");
    assert!(message.contains("response limit"), "{message}");
    assert!(
        elapsed < Duration::from_secs(10),
        "answered after {elapsed:?}"
    );
    let sent = fake.written.load(Ordering::SeqCst);
    assert!(
        sent >= RESPONSE_LIMIT / 2,
        "the fake streamed only {sent} bytes"
    );
    assert!(
        sent < 64 * 1024 * 1024,
        "the client read {sent} bytes of a body past the {RESPONSE_LIMIT}-byte limit"
    );

    let fake = serve(|_| Act::Endless);
    let result = written(write(&fake.relay(vec![]), &admitted()));
    assert_eq!(result.classification, MutationClassification::Unknown);
    assert_eq!(fake.invocations().len(), 1);
}

// ---------------------------------------------------------------------------------------------
// Invariant 7: bounds and lifetime.

/// A blocking loopback listener with an accept queue of one, made through Tokio's socket so the
/// backlog can be set; nothing accepts from it unless the case does.
fn small_backlog_listener() -> TcpListener {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .expect("a runtime");
    let listener = {
        let _entered = runtime.enter();
        let socket = tokio::net::TcpSocket::new_v4().expect("a socket");
        socket
            .bind("127.0.0.1:0".parse().expect("an address"))
            .expect("bind a loopback port");
        socket
            .listen(1)
            .expect("listen")
            .into_std()
            .expect("a std listener")
    };
    listener.set_nonblocking(false).expect("blocking");
    listener
}

/// Connects to `listener` until its accept queue is full: the kernel then drops every further
/// SYN, so a connect neither completes nor fails until its own bound.
fn fill(listener: &TcpListener) -> Vec<TcpStream> {
    let addr = listener.local_addr().expect("the bound address");
    let mut fillers = Vec::new();
    for _ in 0..32 {
        match TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
            Ok(stream) => fillers.push(stream),
            Err(_) => return fillers,
        }
    }
    panic!("the accept queue never filled");
}

/// A relay whose accept queue is full: the read's describe ends at the 5 s connect bound, long
/// before the 20 s request bound, as `relay-unavailable:` (nothing was sent).
#[test]
fn a_connect_that_never_completes_ends_at_the_connect_bound() {
    let listener = small_backlog_listener();
    let fillers = fill(&listener);
    let url = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().expect("the bound address").port()
    );
    let relay = ConnectorsRelay::new(&url, adapter(), connection(), vec![]).expect("loopback");
    let started = Instant::now();
    let message = failed(read(&relay));
    let elapsed = started.elapsed();
    starts(&message, "relay-unavailable:");
    assert!(
        elapsed >= Duration::from_secs(4) && elapsed < Duration::from_secs(12),
        "answered after {elapsed:?}: {message}"
    );
    drop(fillers);
}

/// The describe answers, then the relay's accept queue is full: the write's invoke ends at the
/// connect bound as `Refused` `relay-unavailable`, since no connection was made and nothing sent.
#[test]
fn a_write_whose_invoke_cannot_connect_is_refused_unavailable_at_the_connect_bound() {
    let listener = small_backlog_listener();
    let url = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().expect("the bound address").port()
    );
    let (done, wait) = mpsc::channel::<()>();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("the describe connects");
        let (request, mut stream) = read_request(stream).expect("the describe");
        let fillers = fill(&listener);
        let _ = stream.write_all(&response(200, &[], &descriptor()));
        let _ = stream.flush();
        drop(stream);
        let _ = wait.recv();
        drop(fillers);
        request
    });
    let relay = ConnectorsRelay::new(&url, adapter(), connection(), vec![]).expect("loopback");
    let started = Instant::now();
    let result = write(&relay, &admitted());
    let elapsed = started.elapsed();
    done.send(()).expect("the fake waits");
    let described = server.join().expect("the fake did not panic");
    assert_eq!(described.path, "/v1/describe");
    assert_eq!(write_refused_code(result), "relay-unavailable");
    assert!(
        elapsed >= Duration::from_secs(4) && elapsed < Duration::from_secs(12),
        "answered after {elapsed:?}"
    );
}

/// A call on a clone, and on `over`, after the client it came from dropped still answers.
#[test]
fn a_clone_and_over_still_answer_after_the_original_drops() {
    let fake = serve(either);
    let relay = fake.relay(vec![source()]);
    let clone = relay.clone();
    let narrowed = relay.over(vec![source()]);
    drop(relay);
    read(&clone).expect("the clone answers");
    drop(clone);
    narrowed
        .read(&source(), ReadKind::Get, &read_input())
        .expect("over answers");
    written(write(&narrowed, &admitted()));
}

/// Built, called and dropped (the last clone) inside a current-thread runtime's `block_on` and
/// inside a task of a multi-thread runtime: each call answers and nothing panics.
#[test]
fn a_call_inside_another_tokio_runtime_answers_and_dropping_there_does_not_panic() {
    let fake = serve(either);
    let current = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let url = fake.url();
    let answered = current.block_on(async move {
        let relay =
            ConnectorsRelay::new(&url, adapter(), connection(), vec![source()]).expect("loopback");
        let first = read(&relay).is_ok();
        let clone = relay.clone();
        drop(relay);
        let second = clone.read(&source(), ReadKind::Get, &read_input()).is_ok();
        drop(clone);
        (first, second)
    });
    assert_eq!(answered, (true, true));

    let multi = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .build()
        .expect("a runtime");
    let url = fake.url();
    let task = multi.spawn(async move {
        let relay =
            ConnectorsRelay::new(&url, adapter(), connection(), vec![source()]).expect("loopback");
        let answered = read(&relay).is_ok();
        drop(relay);
        answered
    });
    assert!(multi.block_on(task).expect("the task did not panic"));
}

/// Forty reads from eight threads on clones of one client: each answers, and every invoke
/// carries its own request id of the documented shape.
#[test]
fn concurrent_calls_on_clones_answer_with_distinct_request_ids() {
    let fake = serve(read_ok);
    let relay = fake.relay(vec![]);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let relay = relay.clone();
            scope.spawn(move || {
                for _ in 0..5 {
                    read(&relay).expect("the read answers");
                }
            });
        }
    });
    let mut ids: Vec<String> = fake.invocations().iter().map(request_id).collect();
    assert_eq!(ids.len(), 40);
    assert!(ids.iter().all(|id| is_request_id(id)), "{ids:?}");
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 40, "two invokes shared a request id");
}
