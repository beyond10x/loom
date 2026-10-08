//! Adversary cases for `story:connectors-invoker`, pass 1.
//!
//! Each case drives `ConnectorEffects` over [`ConnectorsInvoker`] through Commission's own loop
//! against an in-process fake Connectors service, as `connectors_invoker.rs` does, with answers that
//! file does not try: every error code of the first binding, a refusal before the audit anchor, a
//! success naming an attempt of another instance, an `applied` error with an attempt-store cause,
//! HTTP 404/503 without an envelope, a keep-alive service that drops the invocation, a credential
//! that must not leak, the sync/async bridge inside a Tokio runtime, and argument numbers that do
//! not fit a float.

use std::io::{BufRead, BufReader, Read, Write};
use std::iter::repeat_n;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread::JoinHandle;
use std::time::Duration;

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionBinding, ActionBindingData, ActionBindingKey, ActionRequestId, ActionStatus,
    AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData, CommissionId,
    ConnectorCredentialRef, ConnectorEndpoint, ConnectorEndpointData, ConnectorEndpointUrl,
    ConnectorInstanceId, ConnectorOperationId, EffectOutcome, ExecutorOutcome,
    ExecutorOutcomeProposedAction, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, action_binding_state, commission_state,
    connector_endpoint_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::connector::ConnectorEffects;
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, LoopError, LoopFailure, run_until_blocked,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_connectors::{ConnectorsInvoker, CredentialError, CredentialResolver, EndpointError};
use serde_json::json;

const EDIT: &str = "repository.edit";
const INSTANCE: &str = "source-host";
const OPERATION: &str = "contents.write";
const REVISION: &str = "rev-7";
const CREDENTIAL: &str = "source-host-token";
const SECRET: &str = "s3cret-bearer-ADVERSARY";
const ATTEMPT: &str = "00000000-0000-4000-8000-0000000000a1";
const NOW: &str = "2026-10-08T09:00:00Z";

// ---------------------------------------------------------------------------------------------
// The fake Connectors service.

#[derive(Debug, Clone)]
struct Received {
    path: String,
    body: Vec<u8>,
}

/// How the fake answers an invocation, given the request id it read. `None` drops the connection
/// without an answer.
type Answering = Box<dyn Fn(&str) -> Option<(u16, String)> + Send + Sync>;

struct FakeService {
    addr: SocketAddr,
    received: Arc<Mutex<Vec<Received>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeService {
    /// One connection per request (`connection: close`).
    fn start(instance: &str, answer: Answering) -> Self {
        Self::start_with(instance, answer, false)
    }

    /// `keep_alive`: every connection serves requests until the client closes it.
    fn start_with(instance: &str, answer: Answering, keep_alive: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let addr = listener.local_addr().expect("the bound address");
        let received = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let descriptor = json!({
            "version": "v1alpha1",
            "instance": instance,
            "adapter": "fake-source-host",
            "revision": REVISION,
            "operations": [{
                "id": OPERATION,
                "description": "write a file",
                "contract": "operations/v1alpha1",
                "profile": "mutation",
                "input_schema": {"type": "object"},
                "output_schema": {"type": "object"}
            }],
            "configuration_schema": {"type": "object"}
        })
        .to_string();
        let answer = Arc::new(answer);
        let thread = {
            let received = Arc::clone(&received);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let received = Arc::clone(&received);
                    let descriptor = descriptor.clone();
                    let answer = Arc::clone(&answer);
                    std::thread::spawn(move || {
                        serve(stream, &received, &descriptor, &answer, keep_alive);
                    });
                }
            })
        };
        Self {
            addr,
            received,
            stop,
            thread: Some(thread),
        }
    }

    fn url(&self) -> String {
        format!("http://{}/", self.addr)
    }

    fn received(&self) -> Vec<Received> {
        self.received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn invocations(&self) -> Vec<Received> {
        self.received()
            .into_iter()
            .filter(|r| r.path == "/v1alpha2/invoke")
            .collect()
    }
}

impl Drop for FakeService {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(
    stream: TcpStream,
    received: &Mutex<Vec<Received>>,
    descriptor: &str,
    answer: &Answering,
    keep_alive: bool,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or_default().to_owned();
        let path = parts.next().unwrap_or_default().to_owned();
        let mut length = 0usize;
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header).unwrap_or(0) == 0 {
                return;
            }
            let header = header.trim_end();
            if header.is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0; length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Received {
                path: path.clone(),
                body: body.clone(),
            });
        let reply = match (method.as_str(), path.as_str()) {
            ("GET", "/v1/describe") => Some((200, descriptor.to_owned())),
            ("POST", "/v1alpha2/invoke") => {
                let request: serde_json::Value =
                    serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
                answer(request["request_id"].as_str().unwrap_or_default())
            }
            _ => Some((404, String::new())),
        };
        let stream = reader.get_mut();
        let Some((status, text)) = reply else {
            let _ = stream.shutdown(Shutdown::Both);
            return;
        };
        let connection = if keep_alive { "keep-alive" } else { "close" };
        let _ = write!(
            stream,
            "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
             connection: {connection}\r\n\r\n{text}",
            text.len()
        );
        let _ = stream.flush();
        if !keep_alive {
            return;
        }
    }
}

/// An attempt reference naming `instance`.
fn mutation_of(
    classification: &str,
    request_id: &str,
    instance: &str,
    cause: serde_json::Value,
) -> serde_json::Value {
    json!({
        "classification": classification,
        "attempt": {"instance": instance, "id": ATTEMPT},
        "original_request_id": request_id,
        "replayed": false,
        "cause": cause
    })
}

/// A raw answer, the request id substituted for `$RID`.
fn raw(status: u16, template: serde_json::Value) -> Answering {
    let text = template.to_string();
    Box::new(move |request_id| Some((status, text.replace("$RID", request_id))))
}

/// An error answer with `code` at `status` and `mutation` (which may name `$RID`), or none.
fn error_answer(status: u16, code: &str, mutation: Option<serde_json::Value>) -> Answering {
    let mut body = json!({
        "version": "v1alpha2",
        "request_id": "$RID",
        "status": "error",
        "error": {"code": code, "message": "the fake service says so"},
        "audit_ref": "aud-1",
        "audit_status": "complete"
    });
    if let Some(mutation) = mutation {
        body["mutation"] = mutation;
    }
    raw(status, body)
}

/// The extended HTTP mapping of compatibility § 5, for the codes the first binding emits.
const FIRST_BINDING_CODES: [(&str, u16); 14] = [
    ("invalid_input", 400),
    ("unsupported", 400),
    ("unauthorized", 401),
    ("forbidden", 403),
    ("not_found", 404),
    ("stale_description", 409),
    ("stale_cursor", 409),
    ("capacity", 413),
    ("rate_limited", 429),
    ("internal", 500),
    ("upstream_protocol", 502),
    ("outcome_unknown", 502),
    ("unavailable", 503),
    ("timeout", 504),
];

// ---------------------------------------------------------------------------------------------
// The commission and its loop.

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x81)),
        agent_revision_id: AgentRevisionId(uuid(0x90)),
        case_id: CaseId("case-edit".to_owned()),
        principal: PrincipalId("principal-c".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn binding(
    commission: &Commission<commission_state::Assigned>,
    instance: &str,
) -> ActionBinding<action_binding_state::Declared> {
    let id = commission.data().commission_id.clone();
    ActionBinding::new(ActionBindingData {
        binding: ActionBindingKey {
            commission_id: id.clone(),
            action: EDIT.to_owned(),
        },
        commission_id: id,
        instance_id: ConnectorInstanceId(instance.to_owned()),
        operation_id: ConnectorOperationId(OPERATION.to_owned()),
    })
}

fn endpoint_with(
    instance: &str,
    url: &str,
    allow_plaintext: bool,
) -> ConnectorEndpoint<connector_endpoint_state::Declared> {
    ConnectorEndpoint::new(ConnectorEndpointData {
        instance_id: ConnectorInstanceId(instance.to_owned()),
        url: ConnectorEndpointUrl(url.to_owned()),
        credential: ConnectorCredentialRef(CREDENTIAL.to_owned()),
        allow_plaintext,
    })
}

fn endpoint(instance: &str, url: &str) -> ConnectorEndpoint<connector_endpoint_state::Declared> {
    endpoint_with(instance, url, true)
}

struct Secrets;

impl CredentialResolver for Secrets {
    fn resolve(&self, credential: &ConnectorCredentialRef) -> Result<String, CredentialError> {
        if credential.0 == CREDENTIAL {
            Ok(SECRET.to_owned())
        } else {
            Err(CredentialError {
                message: "no secret".to_owned(),
            })
        }
    }
}

fn arguments() -> Value {
    Value::Object(vec![(
        "path".to_owned(),
        Value::Text("src/lib.rs".to_owned()),
    )])
}

#[derive(Debug, Default)]
struct Context {
    requests: u64,
    observations: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x300 + self.requests))
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

fn run_with(
    invoker: &ConnectorsInvoker,
    instance: &str,
    arguments: Value,
) -> Result<LoopEnd, LoopError> {
    let commission = commission();
    let case = commission.data().case_id.clone();
    let open = |revision| {
        Answer::at(revision).with_items(
            Vec::new(),
            Vec::new(),
            vec![FrontierAction {
                action: EDIT.to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            }],
        )
    };
    let governor = FakeGovernor::new();
    governor.script(
        case,
        repeat_n(open(3), 5).chain([open(4), open(4).complete("edited")]),
    );
    let executor = ScriptedExecutor::new(vec![ExecutorOutcome::ProposedAction(
        ExecutorOutcomeProposedAction {
            action: EDIT.to_owned(),
            arguments: ProposedActionArguments(arguments),
        },
    )]);
    let effects = ConnectorEffects::new(&commission, [binding(&commission, instance)], invoker)
        .unwrap_or_else(|error| panic!("the binding is refused: {error}"));
    let mut issued = 0u64;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));
    run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission,
        &mut runs,
        &mut Context::default(),
    )
}

fn run(invoker: &ConnectorsInvoker) -> Result<LoopEnd, LoopError> {
    run_with(invoker, INSTANCE, arguments())
}

/// What one invocation against `answer` came to: `Ok(outcome)` or `Err(message)`, and how many
/// invocations the service received.
fn attempt(answer: Answering) -> (Result<EffectOutcome, String>, usize) {
    let service = FakeService::start(INSTANCE, answer);
    let invoker = ConnectorsInvoker::new([endpoint(INSTANCE, &service.url())], Secrets)
        .expect("one endpoint per instance");
    let outcome = match run(&invoker) {
        Ok(end) => {
            assert_eq!(end.effects.len(), 1, "{:?}", end.effects);
            Ok(end.effects[0].clone())
        }
        Err(LoopError {
            failure: LoopFailure::Effect(error),
            ..
        }) => Err(error.message),
        Err(other) => panic!("the loop failed outside the effect port: {other:?}"),
    };
    (outcome, service.invocations().len())
}

/// Asserts `answer` is an `Err`, never a refusal or a performance, after exactly one invocation.
fn assert_error(name: &str, answer: Answering) {
    let (outcome, invocations) = attempt(answer);
    assert!(
        outcome.is_err(),
        "{name}: expected Err (something may have changed), got {outcome:?}"
    );
    assert_eq!(invocations, 1, "{name}: invoked exactly once");
}

// ---------------------------------------------------------------------------------------------
// 1. Refused versus Err.

/// Every first-binding error code at its mapped status, with no `mutation`: Connectors gives no
/// proof that nothing changed, so none is `Refused`.
#[test]
fn every_error_code_without_a_mutation_is_an_error() {
    for (code, status) in FIRST_BINDING_CODES {
        assert_error(code, error_answer(status, code, None));
    }
}

/// `outcome_unknown` and `upstream_protocol` are failures to answer even when a `mutation` says
/// `refused`: § 5 admits the shape, and neither code is a definitive refusal.
#[test]
fn unknown_and_protocol_codes_are_errors_whatever_the_mutation_says() {
    for (code, status) in [("outcome_unknown", 502), ("upstream_protocol", 502)] {
        for classification in ["refused", "not_attempted", "applied"] {
            assert_error(
                &format!("{code} with {classification}"),
                error_answer(
                    status,
                    code,
                    Some(mutation_of(classification, "$RID", INSTANCE, json!(null))),
                ),
            );
        }
    }
    assert_error(
        "outcome_unknown with unknown",
        error_answer(
            502,
            "outcome_unknown",
            Some(mutation_of("unknown", "$RID", INSTANCE, json!(null))),
        ),
    );
}

/// An error that keeps a known `applied` effect, the cause at the attempt store or the response:
/// something changed, so it is never `Refused`.
#[test]
fn an_applied_error_with_a_cause_is_an_error() {
    for (code, status, stage) in [
        ("internal", 500, "attempt_store"),
        ("internal", 500, "response"),
        ("unavailable", 503, "observation"),
        ("capacity", 413, "response"),
    ] {
        assert_error(
            &format!("{code} applied at {stage}"),
            error_answer(
                status,
                code,
                Some(mutation_of(
                    "applied",
                    "$RID",
                    INSTANCE,
                    json!({"code": code, "stage": stage}),
                )),
            ),
        );
    }
}

/// A refusal before the audit anchor (compatibility § 2.1): `request_id: null`,
/// `audit_status: unavailable`, no `mutation`. No proof of non-dispatch reaches the invoker.
#[test]
fn a_refusal_before_the_audit_anchor_is_an_error() {
    assert_error(
        "anchor refused",
        raw(
            503,
            json!({
                "version": "v1alpha2",
                "request_id": null,
                "status": "error",
                "error": {"code": "unavailable", "message": "no state"},
                "audit_ref": null,
                "audit_status": "unavailable"
            }),
        ),
    );
}

/// HTTP 404 and 503 without a v1alpha2 envelope, and a body that is not JSON.
#[test]
fn statuses_without_an_envelope_are_errors() {
    assert_error("404 empty", Box::new(|_| Some((404, String::new()))));
    assert_error(
        "503 html",
        Box::new(|_| Some((503, "<html>down</html>".to_owned()))),
    );
    assert_error(
        "200 truncated",
        Box::new(|_| Some((200, "{\"version\":\"v1alpha2\"".to_owned()))),
    );
}

/// A refusal answered for another request: correlation fails, so it is not this invocation's
/// refusal.
#[test]
fn a_refusal_for_another_request_is_an_error() {
    assert_error(
        "refused, another request id",
        raw(
            403,
            json!({
                "version": "v1alpha2",
                "request_id": "another-request",
                "status": "error",
                "error": {"code": "forbidden", "message": "no"},
                "audit_ref": "aud-1",
                "audit_status": "complete",
                "mutation": mutation_of("refused", "another-request", INSTANCE, json!(null))
            }),
        ),
    );
}

/// `replayed: true` is refused by the first binding's reader; never `Performed`.
#[test]
fn a_replayed_success_is_an_error() {
    let mut mutation = mutation_of("applied", "$RID", INSTANCE, json!(null));
    mutation["replayed"] = json!(true);
    assert_error(
        "replayed",
        raw(
            200,
            json!({
                "version": "v1alpha2",
                "request_id": "$RID",
                "status": "success",
                "result": {},
                "audit_ref": "aud-1",
                "audit_status": "complete",
                "mutation": mutation
            }),
        ),
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Performed.

/// `applied` with both identity fields null (§ 5 admits it: "both null until an original attempt
/// identity is safely known"): no attempt, so `Err`.
#[test]
fn an_applied_success_with_no_attempt_identity_is_an_error() {
    assert_error(
        "applied, attempt null",
        raw(
            200,
            json!({
                "version": "v1alpha2",
                "request_id": "$RID",
                "status": "success",
                "result": {},
                "audit_ref": "aud-1",
                "audit_status": "complete",
                "mutation": {
                    "classification": "applied",
                    "attempt": null,
                    "original_request_id": null,
                    "replayed": false,
                    "cause": null
                }
            }),
        ),
    );
}

/// RED. A success whose attempt is another instance's record is not the attempt this invocation
/// produced on the bound instance (compatibility § 2.1: `attempt` is `{instance, id}` of the
/// record the serving host recorded). The describe check runs on a separate request, so only the
/// attempt's `instance` says which instance performed the write. Expected `Err`.
#[test]
fn a_success_naming_an_attempt_of_another_instance_is_an_error() {
    assert_error(
        "attempt of another instance",
        raw(
            200,
            json!({
                "version": "v1alpha2",
                "request_id": "$RID",
                "status": "success",
                "result": {},
                "audit_ref": "aud-1",
                "audit_status": "complete",
                "mutation": mutation_of("applied", "$RID", "another-instance", json!(null))
            }),
        ),
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Exactly once.

/// A keep-alive service that answers describe, then reads the invocation and drops the
/// connection without answering: the invoker answers `Err` and the service saw the invocation once.
#[test]
fn a_dropped_invocation_on_a_reused_connection_is_never_resent() {
    let service = FakeService::start_with(INSTANCE, Box::new(|_| None), true);
    let invoker = ConnectorsInvoker::new([endpoint(INSTANCE, &service.url())], Secrets)
        .expect("one endpoint per instance");
    let result = run(&invoker);
    assert!(
        matches!(
            result,
            Err(LoopError {
                failure: LoopFailure::Effect(_),
                ..
            })
        ),
        "a dropped invocation is a failure to answer: {result:?}"
    );
    let paths: Vec<String> = service.received().into_iter().map(|r| r.path).collect();
    assert_eq!(
        paths,
        ["/v1/describe", "/v1alpha2/invoke"],
        "described once, invoked once, never resent"
    );
}

// ---------------------------------------------------------------------------------------------
// 5. Plaintext and secrets.

/// An `http` endpoint without `allow_plaintext` is refused when the invoker is made, so it is
/// never called; the refusal names neither the URL's secret parts nor the credential.
#[test]
fn a_plaintext_endpoint_without_admission_is_never_called() {
    let service = FakeService::start(INSTANCE, raw(200, json!({})));
    let made = ConnectorsInvoker::new([endpoint_with(INSTANCE, &service.url(), false)], Secrets);
    assert!(
        matches!(made, Err(EndpointError::Refused { ref instance_id, .. }) if instance_id.0 == INSTANCE),
        "{:?}",
        made.as_ref().err()
    );
    assert!(service.received().is_empty(), "never called");

    let with_password = ConnectorsInvoker::new(
        [endpoint_with(
            INSTANCE,
            "https://user:pw-in-url@127.0.0.1:1/",
            false,
        )],
        Secrets,
    );
    let message = with_password
        .err()
        .map(|e| format!("{e} {e:?}"))
        .unwrap_or_default();
    assert!(!message.is_empty(), "a URL with credentials is refused");
    assert!(!message.contains("pw-in-url"), "{message}");
}

/// The resolved secret appears in no error message and in no `Debug` rendering of the invoker.
#[test]
fn the_resolved_secret_never_leaks() {
    let service = FakeService::start(INSTANCE, error_answer(401, "unauthorized", None));
    let invoker = ConnectorsInvoker::new([endpoint(INSTANCE, &service.url())], Secrets)
        .expect("one endpoint per instance");
    assert!(
        !format!("{invoker:?}").contains(SECRET),
        "Debug: {invoker:?}"
    );
    let rendered = format!("{:?}", run(&invoker));
    assert!(
        rendered.contains("Unauthorized") || rendered.contains("unauthorized"),
        "{rendered}"
    );
    assert!(!rendered.contains(SECRET), "{rendered}");
}

// ---------------------------------------------------------------------------------------------
// 6. The sync/async bridge.

/// Runs `body` on a thread and fails if it neither returns nor panics within 60 s.
fn within_deadline(name: &str, body: impl FnOnce() + Send + 'static) {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
        let _ = sender.send(outcome.is_ok());
    });
    match receiver.recv_timeout(Duration::from_secs(60)) {
        Ok(true) => {}
        Ok(false) => panic!("{name}: panicked"),
        Err(_) => panic!("{name}: deadlocked (no answer in 60 s)"),
    }
}

fn performed_inside(runtime: tokio::runtime::Runtime) {
    let service = FakeService::start(
        INSTANCE,
        raw(
            200,
            json!({
                "version": "v1alpha2",
                "request_id": "$RID",
                "status": "success",
                "result": {},
                "audit_ref": "aud-1",
                "audit_status": "complete",
                "mutation": mutation_of("applied", "$RID", INSTANCE, json!(null))
            }),
        ),
    );
    let url = service.url();
    runtime.block_on(async move {
        // Made, used twice and dropped inside the runtime.
        let invoker = ConnectorsInvoker::new([endpoint(INSTANCE, &url)], Secrets)
            .expect("one endpoint per instance");
        for _ in 0..2 {
            let end = run(&invoker).expect("the loop ends");
            assert!(
                matches!(end.effects.as_slice(), [EffectOutcome::Performed(_)]),
                "{:?}",
                end.effects
            );
        }
        drop(invoker);
    });
    assert_eq!(service.invocations().len(), 2);
}

#[test]
fn invoking_inside_a_current_thread_runtime_neither_panics_nor_deadlocks() {
    within_deadline("current-thread", || {
        performed_inside(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("a runtime"),
        );
    });
}

#[test]
fn invoking_inside_a_multi_thread_runtime_neither_panics_nor_deadlocks() {
    within_deadline("multi-thread", || {
        performed_inside(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("a runtime"),
        );
    });
}

// ---------------------------------------------------------------------------------------------
// The request's arguments.

/// RED. Commission's `Value::Number` keeps "the spelling it arrived in rather than parsed into a
/// float" (generated `json.rs`). An admitted argument beyond `u64` is sent to Connectors as a
/// rounded float: the operation receives another value than the one admitted. Expected: the
/// invocation's `input` carries the admitted integer exactly.
#[test]
fn an_integer_argument_beyond_u64_is_never_sent_altered() {
    const BIG: &str = "18446744073709551617";
    let service = FakeService::start(
        INSTANCE,
        raw(
            200,
            json!({
                "version": "v1alpha2",
                "request_id": "$RID",
                "status": "success",
                "result": {},
                "audit_ref": "aud-1",
                "audit_status": "complete",
                "mutation": mutation_of("applied", "$RID", INSTANCE, json!(null))
            }),
        ),
    );
    let invoker = ConnectorsInvoker::new([endpoint(INSTANCE, &service.url())], Secrets)
        .expect("one endpoint per instance");
    let arguments = Value::Object(vec![("id".to_owned(), Value::Number(BIG.to_owned()))]);
    let result = run_with(&invoker, INSTANCE, arguments);
    let invocations = service.invocations();
    let sent_unchanged = invocations
        .iter()
        .any(|r| String::from_utf8_lossy(&r.body).contains(&format!("\"id\":{BIG}")));
    let refused_before_call = invocations.is_empty() && format!("{result:?}").contains(BIG);
    assert!(
        sent_unchanged || (result.is_err() && refused_before_call),
        "the admitted argument {BIG} must be sent unchanged or refused before any call (loop: {result:?})"
    );
}
