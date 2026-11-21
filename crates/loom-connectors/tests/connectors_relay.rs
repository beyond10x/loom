//! Acceptance for `story:connectors-relay-client`: `loom_connectors::relay::ConnectorsRelay` reads
//! and performs one governed write over the Connectors wire at a loopback relay URL, holding no
//! credential.
//!
//! The fake service binds `127.0.0.1:0` inside this process and reads its port back. It records
//! every request (method, path, headers, body and the raw bytes) and answers each as the test
//! scripts it, by method and path: `GET /v1/describe` ("at the describe") with a bare v1alpha1
//! Descriptor, a boundary body or a legacy Connectors error, and `POST /v1alpha2/invoke` ("at the
//! invoke", after a describe answered with the operation's descriptor) with what the test builds
//! from the request id it read. Every Connectors answer at the invoke is an `InvokeResponse` built
//! with `connectors-core` `v0.62.0` and written with the encoder of the binding the client reads it
//! under: `encode` for a read (read with `decode`), `encode_keyed` for a write (read with
//! `decode_keyed`). Only the answers whose subject is a body no encoder writes are built by hand:
//! the uncorrelated answer, a body with `source_audit`, the lost and unreadable answers, and the
//! boundary's own bodies (`crates/loom-boundary/src/lib.rs`, `refused`). No test makes a network
//! call: every address is loopback.

use std::fmt::Debug;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::iter::repeat_n;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

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
    ReadResult, SourceName, WriteApproval, WriteOperation, WriteResult,
};
use b10x_loom_connectors::cli::{CliError, WriteError, idempotency_key, json};
use b10x_loom_connectors::relay::{ConnectorsRelay, loopback_url};
use connectors_core::v1alpha2::{
    AttemptId, AttemptReference, AuditStatus, EffectKnowledge, Error as WireError, ErrorCode,
    InvokeResponse, MutationObservation, Refusal, ResponseStatus, SourceAudit, Version,
};
use serde_json::json;

/// The one adapter and connection pair behind the relay.
const ADAPTER: &str = "desk";
const CONNECTION: &str = "conn-desk";
/// Another pair's members.
const OTHER_ADAPTER: &str = "mail";
const OTHER_CONNECTION: &str = "conn-other";
/// The read operations the service describes.
const LIST: &str = "tickets.list";
const GET: &str = "tickets.get";
/// The write operation, described with the `mutation` profile.
const ROUTE: &str = "ticket.route";
/// The instance and revision the descriptor names.
const INSTANCE: &str = "desk-support";
const REVISION: &str = "rev-3";
/// The attempt the fake host reports.
const ATTEMPT: &str = "00000000-0000-4000-8000-0000000000a1";
/// The request id a replay names as its original.
const ORIGINAL: &str = "req-original-1";
const NOW: &str = "2026-11-21T09:00:00Z";
/// A fixture token the test writes; no request may carry it.
const TOKEN: &str = "fixture-relay-token-7d2e9b41";
/// Set in the child process of [`the_relay_client_sends_no_credential`].
const CHILD: &str = "LOOM_RELAY_CREDENTIAL_CHILD";

// ---------------------------------------------------------------------------------------------
// The fake service.

/// One request the fake service received.
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

/// How the fake service answers one request.
#[derive(Debug, Clone)]
enum Reply {
    /// The status and the body, then the connection closes.
    Answer(u16, String),
    /// The listener is closed first, then the status and body are written with
    /// `connection: close`: nothing listens on the port afterwards.
    Vanish(u16, String),
    /// The request is read and recorded, and the connection closes with no answer.
    Lost,
}

type Invoking = Box<dyn Fn(&Received) -> Reply + Send + Sync>;

/// A Connectors relay on a loopback port of this process.
struct Fake {
    addr: SocketAddr,
    received: Arc<Mutex<Vec<Received>>>,
    connections: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Fake {
    /// Answers every describe with `describe` and every invoke with what `invoke` gives for it.
    fn start(describe: Reply, invoke: impl Fn(&Received) -> Reply + Send + Sync + 'static) -> Self {
        let invoke: Invoking = Box::new(invoke);
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let addr = listener.local_addr().expect("the bound address");
        let received = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let received = Arc::clone(&received);
            let connections = Arc::clone(&connections);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                loop {
                    let Ok((stream, _)) = listener.accept() else {
                        continue;
                    };
                    if stop.load(Ordering::SeqCst) {
                        return;
                    }
                    connections.fetch_add(1, Ordering::SeqCst);
                    let Some((request, stream)) = read_request(stream) else {
                        continue;
                    };
                    received
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push(request.clone());
                    let reply = match (request.method.as_str(), request.path.as_str()) {
                        ("GET", "/v1/describe") => describe.clone(),
                        ("POST", "/v1alpha2/invoke") => invoke(&request),
                        _ => Reply::Answer(404, String::new()),
                    };
                    match reply {
                        Reply::Answer(status, body) => write_answer(stream, status, &body),
                        Reply::Vanish(status, body) => {
                            drop(listener);
                            write_answer(stream, status, &body);
                            return;
                        }
                        Reply::Lost => drop(stream),
                    }
                }
            })
        };
        Self {
            addr,
            received,
            connections,
            stop,
            thread: Some(thread),
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.addr.port())
    }

    fn port(&self) -> u16 {
        self.addr.port()
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

    /// The relay client for the pair over `sources`, at this fake's port.
    fn relay(&self, sources: Vec<DataSource>) -> ConnectorsRelay {
        ConnectorsRelay::new(&self.url(), adapter(), connection(), sources)
            .expect("a loopback relay URL")
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop so it sees the flag; after a vanish nothing listens.
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Reads one HTTP/1.1 request: its line, headers and `content-length` body.
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

fn write_answer(mut stream: TcpStream, status: u16, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
         connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.flush();
}

/// The request id the invoke body carries.
fn request_id(request: &Received) -> String {
    request.json()["request_id"]
        .as_str()
        .expect("the invoke carries a request id")
        .to_owned()
}

/// A URL of a loopback port nothing listens on: a port the kernel gave a listener that is then
/// closed, never bound again by this test.
fn unreachable_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let port = listener.local_addr().expect("the bound address").port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}

// ---------------------------------------------------------------------------------------------
// Bodies.

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

/// The bare v1alpha1 Descriptor `GET /v1/describe` answers, [`ROUTE`] described with
/// `route_profile`.
fn descriptor_with(route_profile: &str) -> String {
    json!({
        "version": "v1alpha1",
        "instance": INSTANCE,
        "adapter": "desk-adapter",
        "revision": REVISION,
        "operations": [
            operation(LIST, "read"),
            operation(GET, "read"),
            operation(ROUTE, route_profile)
        ],
        "configuration_schema": {"type": "object"}
    })
    .to_string()
}

fn descriptor() -> Reply {
    Reply::Answer(200, descriptor_with("mutation"))
}

/// The boundary's own answer: its fixed `{"error", "message"}` body.
fn boundary(status: u16, code: &str) -> Reply {
    let body = json!({"error": code, "message": "the boundary says so"});
    Reply::Answer(status, body.to_string())
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

/// An error answer after the audit anchor: its record complete.
fn failure(
    request_id: Option<&str>,
    code: ErrorCode,
    mutation: Option<MutationObservation>,
) -> InvokeResponse {
    InvokeResponse {
        version: Version::V1alpha2,
        request_id: request_id.map(str::to_owned),
        status: ResponseStatus::Error,
        result: None,
        error: Some(WireError::new(code, "the fake host says so")),
        audit_ref: Some("aud-2".to_owned()),
        audit_status: AuditStatus::Complete,
        source_audit: None,
        mutation,
    }
}

/// A refusal before the audit anchor: no record, no mutation.
fn before_anchor(request_id: Option<&str>, code: ErrorCode) -> InvokeResponse {
    InvokeResponse {
        audit_ref: None,
        audit_status: AuditStatus::Unavailable,
        ..failure(request_id, code, None)
    }
}

/// A read's answer, written as the unkeyed binding writes it.
fn read_answer(status: u16, response: &InvokeResponse) -> Reply {
    let bytes = response.encode().expect("the unkeyed binding writes it");
    Reply::Answer(status, String::from_utf8(bytes).expect("UTF-8"))
}

/// A write's answer, written as a host with standing grants writes it.
fn write_answer_keyed(status: u16, response: &InvokeResponse) -> Reply {
    let bytes = response.encode_keyed().expect("the keyed binding writes it");
    Reply::Answer(status, String::from_utf8(bytes).expect("UTF-8"))
}

/// A body built by hand, through no encoder.
fn by_hand(status: u16, response: &InvokeResponse) -> Reply {
    Reply::Answer(
        status,
        serde_json::to_string(response).expect("serializable"),
    )
}

fn read_success(request: &Received) -> Reply {
    read_answer(
        200,
        &success(&request_id(request), json!({"tickets": [1, 2]}), None),
    )
}

fn write_applied(request: &Received) -> Reply {
    let id = request_id(request);
    write_answer_keyed(
        200,
        &success(
            &id,
            json!({"routed": true, "queue": "billing"}),
            Some(observed(EffectKnowledge::Applied, &id)),
        ),
    )
}

// ---------------------------------------------------------------------------------------------
// The client's inputs.

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
        .join("connectors_relay/route.proof")
        .display()
        .to_string()
}

fn approval(path: impl Into<String>) -> WriteApproval {
    WriteApproval {
        proof_file: path.into(),
    }
}

fn read(relay: &ConnectorsRelay) -> Result<ReadResult, CliError> {
    relay.invoke_read(
        &adapter(),
        &connection(),
        &OperationId(LIST.to_owned()),
        &read_input(),
    )
}

fn write(relay: &ConnectorsRelay, request: &AdmittedRequest) -> Result<WriteResult, WriteError> {
    relay.invoke_write(request, &route(), &input(), &approval(proof()))
}

fn refused<T: Debug>(result: Result<T, CliError>) -> (String, String) {
    match result {
        Err(CliError::Refused(refusal)) => (refusal.code, refusal.message),
        other => panic!("expected CliError::Refused, got {other:?}"),
    }
}

fn failed<T: Debug>(result: Result<T, CliError>) -> String {
    match result {
        Err(CliError::Failed(message)) => message,
        other => panic!("expected CliError::Failed, got {other:?}"),
    }
}

fn write_refused(result: Result<WriteResult, WriteError>) -> (String, String) {
    match result {
        Err(WriteError::Refused(refusal)) => (refusal.code, refusal.message),
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

// ---------------------------------------------------------------------------------------------
// The admitted request, made by the runtime.

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// Ids from counters, and the one trusted time [`NOW`].
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

/// Keeps the request the runtime admitted and performs nothing.
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

/// The request the runtime admits for [`ROUTE`] with the arguments [`input`] holds.
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
// A write's answer, classified from the typed response.

/// At the invoke, a success with `mutation.classification: applied`, a top-level `audit_ref`
/// with `audit_status: complete`, no `source_audit`, and a `result`.
#[test]
fn a_relay_write_applied_carries_its_classification_audit_and_body() {
    let fake = Fake::start(descriptor(), write_applied);
    let request = admitted();
    let result = written(write(&fake.relay(vec![]), &request));
    let invocations = fake.invocations();
    assert_eq!(fake.describes(), 1);
    assert_eq!(invocations.len(), 1);
    assert_eq!(
        result,
        WriteResult {
            classification: MutationClassification::Applied,
            idempotency_key: idempotency_key(&request),
            request_id: Some(request_id(&invocations[0])),
            body: Some(json::parse(r#"{"routed":true,"queue":"billing"}"#).unwrap()),
            source_audit: None,
            audit_ref: Some("aud-1".to_owned()),
            code: None,
        }
    );
}

/// At the invoke, an error with `mutation.classification: not_attempted` and a base code, then
/// one with `unknown` and `outcome_unknown`.
#[test]
fn a_relay_write_whose_mutation_is_not_attempted_or_unknown_says_so() {
    let request = admitted();
    for (status, code, knowledge, classification, name) in [
        (
            503,
            ErrorCode::Unavailable,
            EffectKnowledge::NotAttempted,
            MutationClassification::NotAttempted,
            "unavailable",
        ),
        (
            502,
            ErrorCode::OutcomeUnknown,
            EffectKnowledge::Unknown,
            MutationClassification::Unknown,
            "outcome_unknown",
        ),
    ] {
        let fake = Fake::start(descriptor(), move |request| {
            let id = request_id(request);
            write_answer_keyed(
                status,
                &failure(Some(&id), code, Some(observed(knowledge, &id))),
            )
        });
        let result = written(write(&fake.relay(vec![]), &request));
        let invocations = fake.invocations();
        assert_eq!(invocations.len(), 1, "{name}: one invoke");
        assert_eq!(
            result,
            WriteResult {
                classification,
                idempotency_key: idempotency_key(&request),
                request_id: Some(request_id(&invocations[0])),
                body: None,
                source_audit: None,
                audit_ref: Some("aud-2".to_owned()),
                code: Some(name.to_owned()),
            },
            "{name}"
        );
    }
}

/// At the invoke, an applied success plus a `source_audit`, built by hand: neither encoder writes
/// it, and `decode_keyed` and `decode` refuse it at `v0.62.0`.
#[test]
fn an_answer_carrying_a_source_audit_is_an_upstream_protocol_answer() {
    let with_source_audit = |mut response: InvokeResponse| {
        response.source_audit = Some(SourceAudit {
            instance: INSTANCE.to_owned(),
            audit_ref: Some("src-1".to_owned()),
            audit_status: AuditStatus::Complete,
        });
        response
    };
    let probe = with_source_audit(success("probe", json!({}), None));
    assert!(probe.encode().is_err(), "the unkeyed encoder refuses it");
    assert!(probe.encode_keyed().is_err(), "the keyed encoder refuses it");

    let fake = Fake::start(descriptor(), move |request| {
        let id = request_id(request);
        by_hand(
            200,
            &with_source_audit(success(
                &id,
                json!({"routed": true}),
                Some(observed(EffectKnowledge::Applied, &id)),
            )),
        )
    });
    let request = admitted();
    let result = written(write(&fake.relay(vec![]), &request));
    assert_eq!(result.classification, MutationClassification::Unknown);
    assert_eq!(result.source_audit, None);
    assert_eq!(fake.invocations().len(), 1, "not retried");

    let fake = Fake::start(descriptor(), move |request| {
        by_hand(
            200,
            &with_source_audit(success(&request_id(request), json!({}), None)),
        )
    });
    starts(&failed(read(&fake.relay(vec![]))), "upstream-protocol:");
    assert_eq!(fake.invocations().len(), 1);
}

// ---------------------------------------------------------------------------------------------
// Reads.

/// One `GET /v1/describe`, one `POST /v1alpha2/invoke` with no `idempotency_key`; the
/// `ReadResult` carries the answer's `audit_ref`.
#[test]
fn a_relay_read_describes_then_invokes_once_unkeyed() {
    let fake = Fake::start(descriptor(), read_success);
    let result = read(&fake.relay(vec![])).expect("the read answers");
    let received = fake.received();
    assert_eq!(received.len(), 2);
    assert_eq!(
        (received[0].method.as_str(), received[0].path.as_str()),
        ("GET", "/v1/describe")
    );
    assert_eq!(
        (received[1].method.as_str(), received[1].path.as_str()),
        ("POST", "/v1alpha2/invoke")
    );
    let body = received[1].json();
    let members: Vec<&str> = body
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        sorted(members),
        ["input", "operation", "request_id", "revision", "version"]
    );
    assert_eq!(body["version"], "v1alpha2");
    assert_eq!(body["operation"], LIST);
    assert_eq!(body["revision"], REVISION);
    assert_eq!(body["input"], json!({"limit": 5}));
    assert_eq!(
        result,
        ReadResult {
            body: json::parse(r#"{"tickets":[1,2]}"#).unwrap(),
            audit_ref: Some("aud-1".to_owned()),
        }
    );
    // `read` of a source reaches the same operation the same way.
    let fake = Fake::start(descriptor(), read_success);
    let relay = fake.relay(vec![source()]);
    relay
        .read(&source(), ReadKind::Get, &read_input())
        .expect("the read answers");
    assert_eq!(fake.describes(), 1);
    assert_eq!(fake.invocations()[0].json()["operation"], GET);
}

fn sorted(mut members: Vec<&str>) -> Vec<&str> {
    members.sort_unstable();
    members
}

#[test]
fn a_read_of_a_kind_the_source_does_not_declare_sends_no_request() {
    let fake = Fake::start(descriptor(), read_success);
    let relay = fake.relay(vec![source()]);
    let (code, _) = refused(relay.read(&source(), ReadKind::Search, &read_input()));
    assert_eq!(code, "undeclared-kind");
    assert!(fake.received().is_empty());
    assert_eq!(fake.connections(), 0);
}

#[test]
fn a_relay_read_of_a_mutation_operation_sends_no_invoke() {
    let fake = Fake::start(descriptor(), read_success);
    let (code, _) = refused(fake.relay(vec![]).invoke_read(
        &adapter(),
        &connection(),
        &OperationId(ROUTE.to_owned()),
        &read_input(),
    ));
    assert_eq!(code, "write-operation");
    assert_eq!(fake.describes(), 1);
    assert!(fake.invocations().is_empty());
    // `describe` refuses it the same way.
    let (code, _) = refused(
        fake.relay(vec![])
            .describe(&adapter(), &OperationId(ROUTE.to_owned())),
    );
    assert_eq!(code, "write-operation");
}

/// At the invoke: a success carrying a `mutation`, an audit record `incomplete`, and one
/// `complete` with an empty `audit_ref`.
#[test]
fn a_read_answer_with_a_mutation_or_an_incomplete_audit_is_failed() {
    let answers: Vec<fn(&str) -> InvokeResponse> = vec![
        |id| {
            success(
                id,
                json!({}),
                Some(observed(EffectKnowledge::Applied, id)),
            )
        },
        |id| InvokeResponse {
            audit_status: AuditStatus::Incomplete,
            ..success(id, json!({}), None)
        },
        |id| InvokeResponse {
            audit_ref: Some(String::new()),
            ..success(id, json!({}), None)
        },
    ];
    for answer in answers {
        let fake = Fake::start(descriptor(), move |request| {
            read_answer(200, &answer(&request_id(request)))
        });
        failed(read(&fake.relay(vec![])));
        assert_eq!(fake.invocations().len(), 1);
    }
}

#[test]
fn a_relay_source_lists_what_the_describe_reports_for_each_kind() {
    let fake = Fake::start(descriptor(), read_success);
    let listed = fake.relay(vec![source()]).sources().expect("listed");
    assert_eq!(listed.len(), 1);
    let (listed_source, entities) = &listed[0];
    assert_eq!(listed_source, &source());
    let kinds: Vec<ReadKind> = entities.iter().map(|(kind, _)| *kind).collect();
    assert_eq!(kinds, [ReadKind::List, ReadKind::Get]);
    for (_, entity) in entities {
        assert_eq!(entity.revision, REVISION);
        assert_eq!(entity.input_schema, json::parse(r#"{"type":"object"}"#).unwrap());
        assert!(!entity.schema.is_empty());
    }
    assert!(fake.invocations().is_empty(), "listing invokes nothing");
}

// ---------------------------------------------------------------------------------------------
// The write's rules kept from the command line.

#[test]
fn a_relay_write_is_keyed_with_the_command_lines_key() {
    let fake = Fake::start(descriptor(), write_applied);
    let request = admitted();
    let result = written(write(&fake.relay(vec![]), &request));
    let invocations = fake.invocations();
    assert_eq!(invocations.len(), 1);
    assert_eq!(
        invocations[0].json()["idempotency_key"],
        json!(idempotency_key(&request))
    );
    assert_eq!(result.idempotency_key, idempotency_key(&request));
}

#[test]
fn a_relay_write_of_other_input_or_action_sends_nothing() {
    let fake = Fake::start(descriptor(), write_applied);
    let relay = fake.relay(vec![]);
    let request = admitted();
    let other = json::parse(r#"{"ticket":"48213","queue":"sales"}"#).unwrap();
    let (code, _) =
        write_refused(relay.invoke_write(&request, &route(), &other, &approval(proof())));
    assert_eq!(code, "input-not-admitted");
    let operation = WriteOperation {
        action: "ticket.close".to_owned(),
        ..route()
    };
    let (code, _) =
        write_refused(relay.invoke_write(&request, &operation, &input(), &approval(proof())));
    assert_eq!(code, "action-not-admitted");
    assert!(fake.received().is_empty());
    assert_eq!(fake.connections(), 0);
}

#[test]
fn a_relay_write_with_a_relative_proof_path_sends_nothing() {
    let fake = Fake::start(descriptor(), write_applied);
    let relay = fake.relay(vec![]);
    let request = admitted();
    for (path, expected) in [
        ("approvals/route.proof", "approval-not-absolute"),
        ("", "approval-missing"),
        ("/approvals/route\n.proof", "approval-not-a-path"),
    ] {
        let (code, _) =
            write_refused(relay.invoke_write(&request, &route(), &input(), &approval(path)));
        assert_eq!(code, expected, "{path:?}");
    }
    assert!(fake.received().is_empty());
    assert_eq!(fake.connections(), 0);
}

#[test]
fn a_relay_write_sends_no_approval_member() {
    let fake = Fake::start(descriptor(), write_applied);
    let request = admitted();
    written(write(&fake.relay(vec![]), &request));
    let invocations = fake.invocations();
    assert_eq!(invocations.len(), 1);
    let body = invocations[0].json();
    let members: Vec<&str> = body
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        sorted(members),
        [
            "idempotency_key",
            "input",
            "operation",
            "request_id",
            "revision",
            "version"
        ]
    );
    assert_eq!(body["operation"], ROUTE);
    assert_eq!(body["input"], json!({"ticket": "48213", "queue": "billing"}));
    let raw = String::from_utf8_lossy(&invocations[0].raw).into_owned();
    assert!(!raw.contains("approval"), "{raw}");
    assert!(!raw.contains(&proof()), "{raw}");
}

#[test]
fn a_relay_write_described_as_a_read_is_not_a_write() {
    let fake = Fake::start(Reply::Answer(200, descriptor_with("read")), write_applied);
    let (code, _) = write_refused(write(&fake.relay(vec![]), &admitted()));
    assert_eq!(code, "not-a-write");
    assert_eq!(fake.describes(), 1);
    assert!(fake.invocations().is_empty());
}

// ---------------------------------------------------------------------------------------------
// One instance per client.

#[test]
fn a_call_naming_another_instance_sends_nothing() {
    let fake = Fake::start(descriptor(), read_success);
    let request = admitted();
    let pair = |message: &str| {
        assert!(
            message.contains(ADAPTER) && message.contains(CONNECTION),
            "`{message}` names the pair the client was built for"
        );
    };

    let configured = fake.relay(vec![source(), source_of(ADAPTER, OTHER_CONNECTION)]);
    let (code, message) = refused(configured.sources());
    assert_eq!(code, "relay-other-instance");
    assert!(message.contains(OTHER_CONNECTION), "{message}");
    pair(&message);

    let narrowed = fake
        .relay(vec![source()])
        .over(vec![source_of(OTHER_ADAPTER, CONNECTION)]);
    let (code, message) = refused(narrowed.sources());
    assert_eq!(code, "relay-other-instance");
    assert!(message.contains(OTHER_ADAPTER), "{message}");
    pair(&message);

    let relay = fake.relay(vec![source()]);
    let (code, message) = refused(relay.invoke_read(
        &adapter(),
        &ConnectionId(OTHER_CONNECTION.to_owned()),
        &OperationId(LIST.to_owned()),
        &read_input(),
    ));
    assert_eq!(code, "relay-other-instance");
    assert!(message.contains(LIST), "{message}");
    pair(&message);

    let (code, message) = refused(relay.read(
        &source_of(OTHER_ADAPTER, CONNECTION),
        ReadKind::List,
        &read_input(),
    ));
    assert_eq!(code, "relay-other-instance");
    assert!(message.contains("tickets"), "{message}");
    pair(&message);

    let (code, message) = refused(relay.describe(
        &AdapterAlias(OTHER_ADAPTER.to_owned()),
        &OperationId(LIST.to_owned()),
    ));
    assert_eq!(code, "relay-other-instance");
    assert!(message.contains(LIST), "{message}");
    pair(&message);

    let operation = WriteOperation {
        connection: ConnectionId(OTHER_CONNECTION.to_owned()),
        ..route()
    };
    let (code, message) =
        write_refused(relay.invoke_write(&request, &operation, &input(), &approval(proof())));
    assert_eq!(code, "relay-other-instance");
    assert!(message.contains(ROUTE), "{message}");
    pair(&message);

    assert!(fake.received().is_empty());
    assert_eq!(fake.connections(), 0);
}

// ---------------------------------------------------------------------------------------------
// What every answer becomes.

/// No listener on the port: the read is `relay-unavailable:`, and so is the write, whose describe
/// is its first request.
#[test]
fn a_relay_that_cannot_be_reached_is_unavailable() {
    let url = unreachable_url();
    let relay = ConnectorsRelay::new(&url, adapter(), connection(), vec![]).unwrap();
    starts(&failed(read(&relay)), "relay-unavailable:");
    starts(&write_failed(write(&relay, &admitted())), "relay-unavailable:");
}

/// The fake closes its listener before it writes the describe's answer (`connection: close`),
/// so the invoke meets no listener and no kept-alive connection.
#[test]
fn a_write_whose_relay_goes_away_after_the_describe_is_refused_unavailable() {
    let vanish = || Reply::Vanish(200, descriptor_with("mutation"));
    let fake = Fake::start(vanish(), write_applied);
    let (code, _) = write_refused(write(&fake.relay(vec![]), &admitted()));
    assert_eq!(code, "relay-unavailable");
    assert_eq!(fake.describes(), 1);
    assert!(fake.invocations().is_empty());

    let fake = Fake::start(vanish(), read_success);
    starts(&failed(read(&fake.relay(vec![]))), "relay-unavailable:");
    assert_eq!(fake.describes(), 1);
    assert!(fake.invocations().is_empty());
}

#[test]
fn a_boundary_refusal_at_the_describe_is_relay_refused_and_no_invoke_is_sent() {
    let request = admitted();
    for (status, code) in [(404, "not_relayed"), (405, "method_refused")] {
        let fake = Fake::start(boundary(status, code), read_success);
        let (refusal, message) = refused(read(&fake.relay(vec![])));
        assert_eq!(refusal, "relay-refused", "{code}");
        assert!(message.contains(code), "{message}");
        assert_eq!(fake.received().len(), 1, "{code}: one request");

        let fake = Fake::start(boundary(status, code), write_applied);
        let (refusal, message) = write_refused(write(&fake.relay(vec![]), &request));
        assert_eq!(refusal, "relay-refused", "{code}");
        assert!(message.contains(code), "{message}");
        assert_eq!(fake.received().len(), 1, "{code}: one request");
    }
}

#[test]
fn a_boundary_refusal_at_the_invoke_is_relay_refused_and_nothing_is_resent() {
    let request = admitted();
    for (status, code) in [
        (403, "operation_refused"),
        (413, "body_too_large"),
        (400, "body_unreadable"),
    ] {
        let fake = Fake::start(descriptor(), move |_| boundary(status, code));
        let (refusal, message) = refused(read(&fake.relay(vec![])));
        assert_eq!(refusal, "relay-refused", "{code}");
        assert!(message.contains(code), "{message}");
        assert_eq!((fake.describes(), fake.invocations().len()), (1, 1), "{code}");

        let fake = Fake::start(descriptor(), move |_| boundary(status, code));
        let (refusal, message) = write_refused(write(&fake.relay(vec![]), &request));
        assert_eq!(refusal, "relay-refused", "{code}");
        assert!(message.contains(code), "{message}");
        assert_eq!((fake.describes(), fake.invocations().len()), (1, 1), "{code}");
    }
}

#[test]
fn a_boundary_unavailable_answer_at_the_describe_is_relay_unavailable() {
    let request = admitted();
    for (status, code) in [
        (503, "token_unavailable"),
        (502, "upstream_unreachable"),
        (504, "upstream_timeout"),
        (502, "upstream_failed"),
    ] {
        let fake = Fake::start(boundary(status, code), read_success);
        starts(&failed(read(&fake.relay(vec![]))), "relay-unavailable:");
        assert!(fake.invocations().is_empty(), "{code}");

        let fake = Fake::start(boundary(status, code), write_applied);
        starts(
            &write_failed(write(&fake.relay(vec![]), &request)),
            "relay-unavailable:",
        );
        assert!(fake.invocations().is_empty(), "{code}");
    }
}

#[test]
fn a_boundary_unavailable_answer_at_the_invoke_is_relay_unavailable() {
    let request = admitted();
    for (status, code) in [
        (503, "token_unavailable"),
        (502, "upstream_unreachable"),
        (504, "upstream_timeout"),
    ] {
        let fake = Fake::start(descriptor(), move |_| boundary(status, code));
        starts(&failed(read(&fake.relay(vec![]))), "relay-unavailable:");
        assert_eq!(fake.invocations().len(), 1, "{code}");

        let fake = Fake::start(descriptor(), move |_| boundary(status, code));
        let (refusal, _) = write_refused(write(&fake.relay(vec![]), &request));
        assert_eq!(refusal, "relay-unavailable", "{code}");
        assert_eq!(fake.invocations().len(), 1, "{code}");
    }
}

#[test]
fn an_upstream_failed_write_is_unknown_and_not_resent() {
    let fake = Fake::start(descriptor(), |_| boundary(502, "upstream_failed"));
    let request = admitted();
    let result = written(write(&fake.relay(vec![]), &request));
    assert_eq!(result.classification, MutationClassification::Unknown);
    assert_eq!(result.idempotency_key, idempotency_key(&request));
    assert_eq!(fake.invocations().len(), 1);

    let fake = Fake::start(descriptor(), |_| boundary(502, "upstream_failed"));
    starts(&failed(read(&fake.relay(vec![]))), "relay-unavailable:");
    assert_eq!(fake.invocations().len(), 1);
}

/// At the describe, a legacy Connectors error: 401 `unauthorized` from its token review.
#[test]
fn a_connectors_error_at_the_describe_is_refused_with_its_code() {
    let legacy = connectors_core::Error::new(
        connectors_core::ErrorCode::Unauthorized,
        "the token review did not admit the caller",
    );
    let body = serde_json::to_string(&legacy).unwrap();
    let fake = Fake::start(Reply::Answer(401, body.clone()), read_success);
    let (code, _) = refused(read(&fake.relay(vec![])));
    assert_eq!(code, "unauthorized");
    assert!(fake.invocations().is_empty());

    let fake = Fake::start(Reply::Answer(401, body), write_applied);
    let (code, _) = write_refused(write(&fake.relay(vec![]), &admitted()));
    assert_eq!(code, "unauthorized");
    assert!(fake.invocations().is_empty());
}

/// At the invoke, a valid v1alpha2 401 `unauthorized` envelope, request id null, no `mutation`.
#[test]
fn a_connectors_unauthorized_answer_at_the_invoke_is_refused_for_a_read_and_unknown_for_a_write()
{
    let fake = Fake::start(descriptor(), |_| {
        read_answer(401, &before_anchor(None, ErrorCode::Unauthorized))
    });
    let (code, _) = refused(read(&fake.relay(vec![])));
    assert_eq!(code, "unauthorized");
    assert_eq!(fake.invocations().len(), 1);

    let fake = Fake::start(descriptor(), |_| {
        write_answer_keyed(401, &before_anchor(None, ErrorCode::Unauthorized))
    });
    let result = written(write(&fake.relay(vec![]), &admitted()));
    assert_eq!(result.classification, MutationClassification::Unknown);
    assert_eq!(result.code.as_deref(), Some("unauthorized"));
    assert_eq!(result.request_id, None);
    assert_eq!(fake.invocations().len(), 1);
}

/// At the invoke, 403 `approval_required` and 409 `idempotency_conflict` are `Unknown` with their
/// code; a 4xx with `mutation.classification: refused` is `Refused`.
#[test]
fn a_connectors_error_with_a_refused_mutation_is_refused() {
    let request = admitted();
    for (status, code, name) in [
        (403, ErrorCode::ApprovalRequired, "approval_required"),
        (409, ErrorCode::IdempotencyConflict, "idempotency_conflict"),
    ] {
        let fake = Fake::start(descriptor(), move |request| {
            write_answer_keyed(status, &before_anchor(Some(&request_id(request)), code))
        });
        let result = written(write(&fake.relay(vec![]), &request));
        assert_eq!(result.classification, MutationClassification::Unknown);
        assert_eq!(result.code.as_deref(), Some(name));
        assert_eq!(fake.invocations().len(), 1);
    }

    let fake = Fake::start(descriptor(), |request| {
        let id = request_id(request);
        write_answer_keyed(
            403,
            &failure(
                Some(&id),
                ErrorCode::Forbidden,
                Some(observed(EffectKnowledge::Refused, &id)),
            ),
        )
    });
    let result = written(write(&fake.relay(vec![]), &request));
    assert_eq!(result.classification, MutationClassification::Refused);
    assert_eq!(result.code.as_deref(), Some("forbidden"));
    assert_eq!(result.audit_ref.as_deref(), Some("aud-2"));
    assert_eq!(fake.invocations().len(), 1);
}

#[test]
fn a_connectors_5xx_at_the_invoke_is_refused_for_a_read_and_unknown_for_a_write() {
    for (status, code, name) in [
        (500, ErrorCode::Internal, "internal"),
        (503, ErrorCode::Unavailable, "unavailable"),
        (404, ErrorCode::NotFound, "not_found"),
        (429, ErrorCode::RateLimited, "rate_limited"),
    ] {
        let fake = Fake::start(descriptor(), move |request| {
            read_answer(status, &failure(Some(&request_id(request)), code, None))
        });
        let (refusal, _) = refused(read(&fake.relay(vec![])));
        assert_eq!(refusal, name);
        assert_eq!(fake.invocations().len(), 1, "{name}: not retried");
    }

    let request = admitted();
    for (status, code, name) in [
        (500, ErrorCode::Internal, "internal"),
        (503, ErrorCode::Unavailable, "unavailable"),
    ] {
        let fake = Fake::start(descriptor(), move |request| {
            write_answer_keyed(status, &failure(Some(&request_id(request)), code, None))
        });
        let result = written(write(&fake.relay(vec![]), &request));
        assert_eq!(result.classification, MutationClassification::Unknown);
        assert_eq!(result.code.as_deref(), Some(name));
        assert_eq!(fake.invocations().len(), 1, "{name}: not retried");
    }
}

/// The fake closes after reading the invoke. This also stands for a timeout after the request
/// left, which has the same result in every column; a test of the timeout itself would wait out
/// the 20 s request bound.
#[test]
fn a_lost_relay_write_answer_is_unknown_and_not_resent() {
    let fake = Fake::start(descriptor(), |_| Reply::Lost);
    let request = admitted();
    let result = written(write(&fake.relay(vec![]), &request));
    assert_eq!(
        result,
        WriteResult {
            classification: MutationClassification::Unknown,
            idempotency_key: idempotency_key(&request),
            request_id: None,
            body: None,
            source_audit: None,
            audit_ref: None,
            code: None,
        }
    );
    assert_eq!(fake.invocations().len(), 1);

    let fake = Fake::start(descriptor(), |_| Reply::Lost);
    starts(&failed(read(&fake.relay(vec![]))), "upstream-protocol:");
    assert_eq!(fake.invocations().len(), 1);
}

#[test]
fn an_unreadable_describe_is_an_upstream_protocol_failure() {
    let unreadable = || Reply::Answer(200, r#"{"not": "a descriptor"}"#.to_owned());
    let fake = Fake::start(unreadable(), read_success);
    starts(&failed(read(&fake.relay(vec![]))), "upstream-protocol:");
    assert!(fake.invocations().is_empty());

    let fake = Fake::start(unreadable(), write_applied);
    starts(
        &write_failed(write(&fake.relay(vec![]), &admitted())),
        "upstream-protocol:",
    );
    assert!(fake.invocations().is_empty());
}

/// At the invoke, another request id echoed.
#[test]
fn an_uncorrelated_answer_is_not_a_connectors_answer() {
    let other = "another-request";
    let fake = Fake::start(descriptor(), move |_| {
        by_hand(200, &success(other, json!({}), None))
    });
    starts(&failed(read(&fake.relay(vec![]))), "upstream-protocol:");
    assert_eq!(fake.invocations().len(), 1);

    let fake = Fake::start(descriptor(), move |_| {
        by_hand(
            200,
            &success(
                other,
                json!({}),
                Some(observed(EffectKnowledge::Applied, other)),
            ),
        )
    });
    let result = written(write(&fake.relay(vec![]), &admitted()));
    assert_eq!(result.classification, MutationClassification::Unknown);
    assert_eq!(result.request_id, None);
    assert_eq!(fake.invocations().len(), 1);
}

/// At the invoke, a host without standing grants refuses the key: `unsupported`, request id null.
#[test]
fn a_keyed_write_refused_unsupported_names_the_standing_grant_and_is_not_resent_unkeyed() {
    let fake = Fake::start(descriptor(), |_| {
        let refusal = Refusal {
            request_id: None,
            error: WireError::new(
                ErrorCode::Unsupported,
                "invocation member is not supported on this binding",
            ),
        };
        write_answer_keyed(400, &refusal.into_response())
    });
    let request = admitted();
    let message = write_failed(write(&fake.relay(vec![]), &request));
    assert!(message.contains("standing_grants"), "{message}");
    assert!(message.contains("connectors.standing-grants/1"), "{message}");
    let invocations = fake.invocations();
    assert_eq!(invocations.len(), 1, "never resent");
    assert_eq!(
        invocations[0].json()["idempotency_key"],
        json!(idempotency_key(&request))
    );
}

/// At the invoke, a replay: `{classification: applied, replayed: true, attempt, original_request_id}`.
#[test]
fn a_replayed_write_is_applied_naming_the_original_attempt() {
    let fake = Fake::start(descriptor(), |request| {
        let replay = MutationObservation {
            replayed: true,
            ..observed(EffectKnowledge::Applied, ORIGINAL)
        };
        write_answer_keyed(
            200,
            &success(&request_id(request), json!({"routed": true}), Some(replay)),
        )
    });
    let result = written(write(&fake.relay(vec![]), &admitted()));
    assert_eq!(result.classification, MutationClassification::Applied);
    assert_eq!(result.request_id.as_deref(), Some(ORIGINAL));
    assert_eq!(fake.invocations().len(), 1);
}

// ---------------------------------------------------------------------------------------------
// No credential, loopback only, no process.

/// The headers a client could carry a credential in.
const CREDENTIAL_HEADERS: [&str; 4] = ["authorization", "proxy-authorization", "x-api-key", "cookie"];

/// A read and a write, each answered; no recorded request carries a credential header or the
/// fixture token.
fn exercise_without_credential() {
    let fake = Fake::start(descriptor(), |request| {
        if request.json().get("idempotency_key").is_some() {
            write_applied(request)
        } else {
            read_success(request)
        }
    });
    let relay = fake.relay(vec![source()]);
    read(&relay).expect("the read answers");
    let result = written(write(&relay, &admitted()));
    assert_eq!(result.classification, MutationClassification::Applied);
    let received = fake.received();
    assert_eq!(received.len(), 4, "two describes and two invokes");
    for request in &received {
        for header in CREDENTIAL_HEADERS {
            assert_eq!(
                request.header(header),
                None,
                "{} {} carries `{header}`",
                request.method,
                request.path
            );
        }
        let raw = String::from_utf8_lossy(&request.raw);
        assert!(!raw.contains(TOKEN), "{} {} carries the token", request.method, request.path);
    }
}

/// No recorded request carries `authorization`, `proxy-authorization`, `x-api-key` or `cookie`,
/// and a fixture token the test writes appears in no request: in this process, and in a child
/// whose environment names the token, its file and a proxy with it as credentials.
#[test]
fn the_relay_client_sends_no_credential() {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("connectors_relay");
    fs::create_dir_all(&directory).unwrap();
    let token_file = directory.join("token");
    fs::write(&token_file, TOKEN).unwrap();
    exercise_without_credential();
    if std::env::var_os(CHILD).is_some() {
        return;
    }
    // A proxy that would see a client that honoured the proxy variables.
    let proxy = Fake::start(descriptor(), |_| Reply::Lost);
    let proxied = format!("http://relay:{TOKEN}@127.0.0.1:{}", proxy.port());
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["the_relay_client_sends_no_credential", "--exact", "--test-threads=1"])
        .env(CHILD, "1")
        .env("CONNECTORS_TOKEN", TOKEN)
        .env("CONNECTORS_SERVICE_CREDENTIAL", TOKEN)
        .env("LOOM_CONNECTORS_TOKEN", TOKEN)
        .env("LOOM_BOUNDARY_CONNECTORS_TOKEN", TOKEN)
        .env("LOOM_BOUNDARY_TOKEN", TOKEN)
        .env("CONNECTORS_TOKEN_FILE", &token_file)
        .env("LOOM_CONNECTORS_TOKEN_FILE", &token_file)
        .env("HTTP_PROXY", &proxied)
        .env("http_proxy", &proxied)
        .env("ALL_PROXY", &proxied)
        .env("all_proxy", &proxied)
        .env_remove("NO_PROXY")
        .env_remove("no_proxy")
        .output()
        .expect("the test binary runs again");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "the child failed: {stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("1 passed"), "the child ran the test: {stdout}");
    assert_eq!(proxy.connections(), 0, "nothing went through the proxy");
}

#[test]
fn a_relay_url_that_is_not_loopback_is_refused_before_any_request() {
    let fake = Fake::start(descriptor(), read_success);
    let shapes = |port: u16| {
        [
            format!("http://10.0.0.1:{port}"),
            format!("https://127.0.0.1:{port}"),
            format!("http://u:p@127.0.0.1:{port}"),
            format!("http://127.0.0.1:{port}/x"),
            format!("http://127.0.0.1:{port}?q"),
            format!("http://127.0.0.1:{port}#f"),
        ]
    };
    for value in shapes(8402).into_iter().chain(shapes(fake.port())) {
        let by_rule = loopback_url(&value).expect_err(&value);
        let by_client =
            ConnectorsRelay::new(&value, adapter(), connection(), vec![source()]).expect_err(&value);
        assert_eq!(by_rule, by_client, "{value}");
        let said = by_rule.to_string();
        assert!(!said.contains(&value), "`{said}` repeats the value");
        assert!(!said.contains("10.0.0.1") && !said.contains("u:p"), "{said}");
    }
    let accepted = loopback_url("http://127.0.0.1:8402").expect("a loopback URL");
    assert_eq!(accepted.to_string(), "http://127.0.0.1:8402");
    assert_eq!(accepted.port(), 8402);
    ConnectorsRelay::new("http://127.0.0.1:8402", adapter(), connection(), vec![])
        .expect("a loopback URL");
    assert!(fake.received().is_empty());
    assert_eq!(fake.connections(), 0);
}

/// A source scan, as `no_invoke_ever_carries_an_approval_file` scans `src/cli.rs`: `src/relay.rs`
/// starts no program and reaches the command-line client only through the names listed.
#[test]
fn the_relay_module_starts_no_process() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/relay.rs");
    let source = fs::read_to_string(&path).unwrap();
    for forbidden in ["std::process", "tokio::process", "Command", "ConnectorsCli"] {
        assert!(
            !source.contains(forbidden),
            "src/relay.rs holds `{forbidden}`"
        );
    }
    let allowed = [
        "CliError",
        "WriteError",
        "idempotency_key",
        "WRITE_PROFILES",
        "SourceEntities",
        "Described",
        "text",
        "write::admits",
    ];
    let reached = cli_names(&source);
    assert!(!reached.is_empty(), "the scan saw the `cli::` paths");
    for name in &reached {
        assert!(
            allowed.contains(&name.as_str()),
            "src/relay.rs reaches `cli::{name}`, which is not one of {allowed:?}"
        );
    }
}

/// Every name `source` reaches through a `cli::` path, in a `use` group or inline. A group
/// nested in a group, or an alias, is read as a name outside the list.
fn cli_names(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for (at, _) in source.match_indices("cli::") {
        let segment_start = source[..at]
            .chars()
            .next_back()
            .is_none_or(|before| !(before.is_alphanumeric() || before == '_'));
        if !segment_start {
            continue;
        }
        let rest = &source[at + "cli::".len()..];
        if let Some(group) = rest.strip_prefix('{') {
            let end = group.find('}').expect("a closed `use` group");
            for item in group[..end].split(',') {
                let item: String = item.split_whitespace().collect();
                if !item.is_empty() {
                    names.push(item);
                }
            }
        } else {
            let path: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':' || *c == '{')
                .collect();
            names.push(path.trim_end_matches(':').to_owned());
        }
    }
    names
}
