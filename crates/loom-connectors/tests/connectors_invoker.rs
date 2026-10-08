//! Acceptance for `story:connectors-invoker`: `ConnectorEffects` over [`ConnectorsInvoker`] and a
//! commission binding one action to a Connector operation performs an admitted request of that
//! action exactly once against an in-process fake Connectors service and returns `Performed`
//! naming the attempt the service reported.
//!
//! The fake service binds a loopback port inside this process. It answers `GET /v1/describe` with
//! a bare v1alpha1 descriptor (the write [`OPERATION`], profile `mutation`, and the read
//! [`READ_OPERATION`]) and `POST /v1alpha2/invoke` with the scripted v1alpha2 response, and
//! records every request it receives. The commission's loop is Commission's own
//! `run_until_blocked` over the testkit's scripted governor and executor; only the runtime makes an
//! admitted request.
//!
//! - `admitted_request_is_performed_once_and_names_the_attempt`: the acceptance.
//! - `not_performed_and_nothing_changed_is_refused`: Connectors reports `refused` or
//!   `not_attempted` for the attempt it recorded; the outcome is `Refused`.
//! - `every_other_failure_is_an_error_never_a_refusal`: transport, protocol and unmapped errors,
//!   an unknown outcome, a success that names no attempt and an unresolvable credential.
//! - `an_instance_resolves_to_exactly_one_endpoint`: an unknown `instance_id` answers `Err` before
//!   any call; a second endpoint for one instance is refused; a service describing another
//!   instance is never invoked.
//! - `an_admitted_read_is_performed_with_its_result_and_names_its_audit_record`: the acceptance of
//!   `story:connector-read-performed`; `a_read_without_a_complete_audit_record_is_an_error`,
//!   `a_write_without_a_recorded_attempt_is_still_an_error` and
//!   `a_binding_the_service_describes_otherwise_is_never_invoked` bound it.

use std::io::{BufRead, BufReader, Read, Write};
use std::iter::repeat_n;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionBinding, ActionBindingData, ActionBindingKey, ActionRequestId, ActionStatus,
    AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData, CommissionId,
    ConnectorAttemptId, ConnectorAuditRef, ConnectorCredentialRef, ConnectorEndpoint,
    ConnectorEndpointData, ConnectorEndpointUrl, ConnectorInstanceId, ConnectorOperationEffect,
    ConnectorOperationId, EffectOutcome, ExecutorOutcome, ExecutorOutcomeProposedAction,
    FrontierAction, ObservationId, PrincipalId, ProposedActionArguments, RunId,
    action_binding_state, commission_state, connector_endpoint_state,
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

/// The action the commission binds.
const EDIT: &str = "repository.edit";
/// The Connector instance the action is bound to.
const INSTANCE: &str = "source-host";
/// The operation the action is bound to.
const OPERATION: &str = "contents.write";
/// The read operation the service also describes.
const READ_OPERATION: &str = "contents.read";
/// The descriptor revision the fake service describes.
const REVISION: &str = "rev-7";
/// The credential reference the endpoint names, and the secret the resolver gives for it.
const CREDENTIAL: &str = "source-host-token";
const SECRET: &str = "s3cret-bearer";
/// The attempt the fake service reports.
const ATTEMPT: &str = "00000000-0000-4000-8000-0000000000a1";
/// The trusted time the loop's context gives.
const NOW: &str = "2026-10-08T09:00:00Z";

// ---------------------------------------------------------------------------------------------
// The fake Connectors service.

/// One request the fake service received.
#[derive(Debug, Clone)]
struct Received {
    method: String,
    path: String,
    authorization: Option<String>,
    body: Vec<u8>,
}

impl Received {
    fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).expect("the invocation body is JSON")
    }
}

/// How the fake service answers an invocation, given the request id it read from the body.
type Answering = Box<dyn Fn(&str) -> (u16, String) + Send + Sync>;

/// A Connectors service on a loopback port of this process.
struct FakeService {
    addr: SocketAddr,
    received: Arc<Mutex<Vec<Received>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeService {
    /// A service describing `instance` and answering every invocation with `answer`.
    fn start(instance: &str, answer: Answering) -> Self {
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
            }, {
                "id": READ_OPERATION,
                "description": "read a file",
                "contract": "operations/v1alpha1",
                "profile": "read",
                "input_schema": {"type": "object"},
                "output_schema": {"type": "object"}
            }],
            "configuration_schema": {"type": "object"}
        })
        .to_string();
        let thread = {
            let received = Arc::clone(&received);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    serve(stream, &received, &descriptor, &answer);
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
        // Wake the accept loop so it sees the flag.
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Reads one HTTP/1.1 request from `stream`, records it and answers it, then closes.
fn serve(stream: TcpStream, received: &Mutex<Vec<Received>>, descriptor: &str, answer: &Answering) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();
    let mut length = 0usize;
    let mut authorization = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 {
            return;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let value = value.trim();
            match name.to_ascii_lowercase().as_str() {
                "content-length" => length = value.parse().unwrap_or(0),
                "authorization" => authorization = Some(value.to_owned()),
                _ => {}
            }
        }
    }
    let mut body = vec![0; length];
    if reader.read_exact(&mut body).is_err() {
        return;
    }
    let request = Received {
        method,
        path,
        authorization,
        body,
    };
    received
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(request.clone());
    let (status, text) = match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/v1/describe") => (200, descriptor.to_owned()),
        ("POST", "/v1alpha2/invoke") => {
            let request_id = request.json()["request_id"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            answer(&request_id)
        }
        _ => (404, String::new()),
    };
    let mut stream = reader.into_inner();
    let _ = write!(
        stream,
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
         connection: close\r\n\r\n{text}",
        text.len()
    );
    let _ = stream.flush();
}

/// The mutation member of a response, for the attempt [`ATTEMPT`] of `request_id`.
fn mutation(classification: &str, request_id: &str) -> serde_json::Value {
    json!({
        "classification": classification,
        "attempt": {"instance": INSTANCE, "id": ATTEMPT},
        "original_request_id": request_id,
        "replayed": false,
        "cause": null
    })
}

/// A write that succeeded: `applied`, naming its attempt, with `result`.
fn applied(result: serde_json::Value) -> Answering {
    Box::new(move |request_id| {
        let body = json!({
            "version": "v1alpha2",
            "request_id": request_id,
            "status": "success",
            "result": result,
            "audit_ref": "aud-1",
            "audit_status": "complete",
            "mutation": mutation("applied", request_id)
        });
        (200, body.to_string())
    })
}

/// An error answer with `code` at HTTP `status`, carrying `classification`'s mutation, or none.
fn failed(status: u16, code: &'static str, classification: Option<&'static str>) -> Answering {
    Box::new(move |request_id| {
        let mut body = json!({
            "version": "v1alpha2",
            "request_id": request_id,
            "status": "error",
            "error": {"code": code, "message": "the fake service says so"},
            "audit_ref": "aud-1",
            "audit_status": "complete"
        });
        if let Some(classification) = classification {
            body["mutation"] = mutation(classification, request_id);
        }
        (status, body.to_string())
    })
}

/// A read's success: no mutation, so no attempt.
fn read_success() -> Answering {
    Box::new(|request_id| {
        let body = json!({
            "version": "v1alpha2",
            "request_id": request_id,
            "status": "success",
            "result": {"ok": true},
            "audit_ref": "aud-1",
            "audit_status": "complete"
        });
        (200, body.to_string())
    })
}

/// A success answered for another request.
fn uncorrelated() -> Answering {
    Box::new(|_| {
        let other = "another-request";
        let body = json!({
            "version": "v1alpha2",
            "request_id": other,
            "status": "success",
            "result": {},
            "audit_ref": "aud-1",
            "audit_status": "complete",
            "mutation": mutation("applied", other)
        });
        (200, body.to_string())
    })
}

/// Bytes that are no v1alpha2 envelope.
fn garbage() -> Answering {
    Box::new(|_| (200, "{\"not\": \"an envelope\"".to_owned()))
}

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
    operation: &str,
    effect: ConnectorOperationEffect,
) -> ActionBinding<action_binding_state::Declared> {
    let id = commission.data().commission_id.clone();
    ActionBinding::new(ActionBindingData {
        binding: ActionBindingKey {
            commission_id: id.clone(),
            action: EDIT.to_owned(),
        },
        commission_id: id,
        instance_id: ConnectorInstanceId(instance.to_owned()),
        operation_id: ConnectorOperationId(operation.to_owned()),
        effect,
    })
}

fn endpoint(instance: &str, url: &str) -> ConnectorEndpoint<connector_endpoint_state::Declared> {
    ConnectorEndpoint::new(ConnectorEndpointData {
        instance_id: ConnectorInstanceId(instance.to_owned()),
        url: ConnectorEndpointUrl(url.to_owned()),
        credential: ConnectorCredentialRef(CREDENTIAL.to_owned()),
        allow_plaintext: true,
    })
}

/// Resolves [`CREDENTIAL`] to [`SECRET`], and nothing else.
struct Secrets;

impl CredentialResolver for Secrets {
    fn resolve(&self, credential: &ConnectorCredentialRef) -> Result<String, CredentialError> {
        if credential.0 == CREDENTIAL {
            Ok(SECRET.to_owned())
        } else {
            Err(CredentialError {
                message: format!("no secret under `{}`", credential.0),
            })
        }
    }
}

/// Resolves nothing.
struct NoSecrets;

impl CredentialResolver for NoSecrets {
    fn resolve(&self, credential: &ConnectorCredentialRef) -> Result<String, CredentialError> {
        Err(CredentialError {
            message: format!("no secret under `{}`", credential.0),
        })
    }
}

/// The arguments the executor proposes.
fn arguments() -> Value {
    Value::Object(vec![
        ("path".to_owned(), Value::Text("src/lib.rs".to_owned())),
        ("lines".to_owned(), Value::Number("3".to_owned())),
    ])
}

/// New ids from counters and the one trusted time [`NOW`]; no step budget.
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

/// [`run_bound`] with [`EDIT`] bound to the write [`OPERATION`] of `instance`.
fn run(invoker: &ConnectorsInvoker, instance: &str) -> Result<LoopEnd, LoopError> {
    run_bound(
        invoker,
        instance,
        OPERATION,
        ConnectorOperationEffect::Write,
    )
}

/// One loop of the commission on a frontier admitting [`EDIT`], bound to `operation` of
/// `instance` with `effect`: the executor proposes it once, the runtime admits it and hands it to
/// `ConnectorEffects` over `invoker`; after the invocation the case completes.
fn run_bound(
    invoker: &ConnectorsInvoker,
    instance: &str,
    operation: &str,
    effect: ConnectorOperationEffect,
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
            arguments: ProposedActionArguments(arguments()),
        },
    )]);
    let effects = ConnectorEffects::new(
        &commission,
        [binding(&commission, instance, operation, effect)],
        invoker,
    )
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

/// The effect error a loop stopped with.
fn effect_error(name: &str, result: Result<LoopEnd, LoopError>) -> String {
    match result {
        Err(LoopError {
            failure: LoopFailure::Effect(error),
            ..
        }) => error.message,
        other => panic!("{name}: expected the effect port to fail, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// The acceptance and the checks.

#[test]
fn admitted_request_is_performed_once_and_names_the_attempt() {
    let service = FakeService::start(
        INSTANCE,
        applied(json!({"written": "src/lib.rs", "bytes": 42})),
    );
    let invoker = ConnectorsInvoker::new([endpoint(INSTANCE, &service.url())], Secrets)
        .expect("one endpoint per instance");
    let end = run(&invoker, INSTANCE).expect("the loop ends");

    assert_eq!(end.admitted.len(), 1, "{:?}", end.admitted);
    assert_eq!(end.effects.len(), 1, "{:?}", end.effects);
    let EffectOutcome::Performed(performed) = &end.effects[0] else {
        panic!("not performed: {:?}", end.effects);
    };
    assert_eq!(
        performed.attempt,
        Some(ConnectorAttemptId(ATTEMPT.to_owned())),
        "the attempt the service reported"
    );
    assert_eq!(
        performed.report,
        Value::Object(vec![
            ("bytes".to_owned(), Value::Number("42".to_owned())),
            ("written".to_owned(), Value::Text("src/lib.rs".to_owned())),
        ]),
        "the report is the operation's result, its members in the order the client reads them \
         (by name)"
    );

    let invocations = service.invocations();
    assert_eq!(
        invocations.len(),
        1,
        "invoked exactly once: {invocations:?}"
    );
    let invocation = &invocations[0];
    assert_eq!(invocation.method, "POST");
    assert_eq!(
        invocation.authorization.as_deref(),
        Some(format!("Bearer {SECRET}").as_str()),
        "the resolved credential is the bearer"
    );
    let body = invocation.json();
    assert_eq!(body["version"], "v1alpha2");
    assert_eq!(body["operation"], OPERATION);
    assert_eq!(body["revision"], REVISION, "the described revision");
    assert_eq!(
        body["input"],
        json!({"path": "src/lib.rs", "lines": 3}),
        "the admitted request's arguments"
    );
    let paths: Vec<String> = service.received().into_iter().map(|r| r.path).collect();
    assert_eq!(
        paths,
        ["/v1/describe", "/v1alpha2/invoke"],
        "described once, then invoked once; never /v1/invoke"
    );
}

#[test]
fn not_performed_and_nothing_changed_is_refused() {
    for (name, status, code, classification) in [
        ("provider refusal", 403, "forbidden", "refused"),
        ("not attempted", 503, "unavailable", "not_attempted"),
    ] {
        let service = FakeService::start(INSTANCE, failed(status, code, Some(classification)));
        let invoker = ConnectorsInvoker::new([endpoint(INSTANCE, &service.url())], Secrets)
            .expect("one endpoint per instance");
        let end =
            run(&invoker, INSTANCE).unwrap_or_else(|e| panic!("{name}: the loop failed: {e}"));
        assert_eq!(end.effects.len(), 1, "{name}: {:?}", end.effects);
        let EffectOutcome::Refused(refused) = &end.effects[0] else {
            panic!("{name}: not refused: {:?}", end.effects);
        };
        assert!(
            refused.reason.contains(code) || refused.reason.contains("fake service"),
            "{name}: the reason names Connectors' answer: {}",
            refused.reason
        );
        assert_eq!(service.invocations().len(), 1, "{name}: invoked once");
    }
}

#[test]
fn every_other_failure_is_an_error_never_a_refusal() {
    let answers: Vec<(&str, Answering)> = vec![
        (
            "an error without a recorded attempt",
            failed(404, "not_found", None),
        ),
        (
            "a forbidden answer without a recorded attempt",
            failed(403, "forbidden", None),
        ),
        (
            "an unknown outcome",
            failed(502, "outcome_unknown", Some("unknown")),
        ),
        (
            "applied, but the result was not delivered",
            failed(502, "upstream_protocol", Some("applied")),
        ),
        ("a success naming no attempt", read_success()),
        ("a response to another request", uncorrelated()),
        (
            "a refusal at a status outside the mapping",
            failed(200, "forbidden", Some("refused")),
        ),
        ("bytes that are no envelope", garbage()),
    ];
    for (name, answer) in answers {
        let service = FakeService::start(INSTANCE, answer);
        let invoker = ConnectorsInvoker::new([endpoint(INSTANCE, &service.url())], Secrets)
            .expect("one endpoint per instance");
        let message = effect_error(name, run(&invoker, INSTANCE));
        assert!(!message.is_empty(), "{name}");
        assert_eq!(
            service.invocations().len(),
            1,
            "{name}: invoked once, never resent"
        );
    }

    // Transport: nothing listens on the endpoint.
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        listener.local_addr().expect("the bound address")
    };
    let invoker =
        ConnectorsInvoker::new([endpoint(INSTANCE, &format!("http://{closed}/"))], Secrets)
            .expect("one endpoint per instance");
    let message = effect_error("transport", run(&invoker, INSTANCE));
    assert!(!message.is_empty(), "transport");

    // The credential cannot be resolved: nothing is called.
    let service = FakeService::start(INSTANCE, applied(json!({})));
    let invoker = ConnectorsInvoker::new([endpoint(INSTANCE, &service.url())], NoSecrets)
        .expect("one endpoint per instance");
    let message = effect_error("unresolved credential", run(&invoker, INSTANCE));
    assert!(
        message.contains(CREDENTIAL),
        "unresolved credential: {message}"
    );
    assert!(
        service.received().is_empty(),
        "unresolved credential: no call"
    );
}

#[test]
fn an_instance_resolves_to_exactly_one_endpoint() {
    // An instance no endpoint serves: `Err` before any call.
    let service = FakeService::start(INSTANCE, applied(json!({})));
    let invoker = ConnectorsInvoker::new([endpoint(INSTANCE, &service.url())], Secrets)
        .expect("one endpoint per instance");
    let message = effect_error("unknown instance", run(&invoker, "elsewhere"));
    assert!(message.contains("elsewhere"), "unknown instance: {message}");
    assert!(
        service.received().is_empty(),
        "unknown instance: no call at all"
    );

    // Two endpoints for one instance are refused.
    let refused = ConnectorsInvoker::new(
        [
            endpoint(INSTANCE, &service.url()),
            endpoint(INSTANCE, "http://127.0.0.1:9/"),
        ],
        Secrets,
    );
    assert!(
        matches!(refused, Err(EndpointError::Duplicate(ref id)) if id.0 == INSTANCE),
        "a second endpoint for one instance: {:?}",
        refused.err()
    );

    // An endpoint that describes another instance is never invoked.
    let other = FakeService::start("another-instance", applied(json!({})));
    let invoker = ConnectorsInvoker::new([endpoint(INSTANCE, &other.url())], Secrets)
        .expect("one endpoint per instance");
    let message = effect_error("another instance described", run(&invoker, INSTANCE));
    assert!(
        message.contains("another-instance"),
        "another instance: {message}"
    );
    assert!(
        other.invocations().is_empty(),
        "another instance: never invoked"
    );
}

/// A read's success with `result`, its audit record `audit_ref` in `audit_status`, and the
/// mutation `mutation` names, or none.
fn read_answer(
    result: serde_json::Value,
    audit_ref: Option<&'static str>,
    audit_status: &'static str,
    mutation_classification: Option<&'static str>,
) -> Answering {
    Box::new(move |request_id| {
        let mut body = json!({
            "version": "v1alpha2",
            "request_id": request_id,
            "status": "success",
            "result": result,
            "audit_ref": audit_ref,
            "audit_status": audit_status
        });
        if let Some(classification) = mutation_classification {
            body["mutation"] = mutation(classification, request_id);
        }
        (200, body.to_string())
    })
}

/// `story:connector-read-performed`: an admitted action bound to a read operation answers
/// `Performed` with the operation's result, naming the audit record the service completed for the
/// invocation and no attempt.
#[test]
fn an_admitted_read_is_performed_with_its_result_and_names_its_audit_record() {
    let service = FakeService::start(
        INSTANCE,
        read_answer(
            json!({"path": "src/lib.rs", "text": "fn main() {}"}),
            Some("aud-7"),
            "complete",
            None,
        ),
    );
    let invoker = ConnectorsInvoker::new([endpoint(INSTANCE, &service.url())], Secrets)
        .expect("one endpoint per instance");
    let end = run_bound(
        &invoker,
        INSTANCE,
        READ_OPERATION,
        ConnectorOperationEffect::Read,
    )
    .expect("the loop ends");

    assert_eq!(end.admitted.len(), 1, "{:?}", end.admitted);
    assert_eq!(
        end.effects,
        vec![EffectOutcome::Performed(
            b10x_loom_commission::model::responsibility::EffectOutcomePerformed {
                report: Value::Object(vec![
                    ("path".to_owned(), Value::Text("src/lib.rs".to_owned())),
                    ("text".to_owned(), Value::Text("fn main() {}".to_owned())),
                ]),
                attempt: None,
                audit: Some(ConnectorAuditRef("aud-7".to_owned())),
            }
        )],
        "the operation's result, its audit record and no attempt"
    );
    let invocations = service.invocations();
    assert_eq!(
        invocations.len(),
        1,
        "invoked exactly once: {invocations:?}"
    );
    assert_eq!(invocations[0].json()["operation"], READ_OPERATION);
}

/// A read is `Performed` only when the service completed its audit record and recorded no attempt;
/// every other answer to a read is `Err`, never a refusal, after the one invocation.
#[test]
fn a_read_without_a_complete_audit_record_is_an_error() {
    let answers: Vec<(&str, Answering)> = vec![
        (
            "an incomplete audit record",
            read_answer(json!({}), Some("aud-7"), "incomplete", None),
        ),
        (
            "an unavailable audit record",
            read_answer(json!({}), None, "unavailable", None),
        ),
        (
            "a recorded attempt",
            read_answer(json!({}), Some("aud-7"), "complete", Some("applied")),
        ),
        (
            "an error without a recorded attempt",
            failed(404, "not_found", None),
        ),
    ];
    for (name, answer) in answers {
        let service = FakeService::start(INSTANCE, answer);
        let invoker = ConnectorsInvoker::new([endpoint(INSTANCE, &service.url())], Secrets)
            .expect("one endpoint per instance");
        let message = effect_error(
            name,
            run_bound(
                &invoker,
                INSTANCE,
                READ_OPERATION,
                ConnectorOperationEffect::Read,
            ),
        );
        assert!(!message.is_empty(), "{name}");
        assert_eq!(
            service.invocations().len(),
            1,
            "{name}: invoked once, never resent"
        );
    }
}

/// A consequential action still answers `Err` when the service records no attempt: a write
/// operation's success that names none is not performed, whatever audit record it carries.
#[test]
fn a_write_without_a_recorded_attempt_is_still_an_error() {
    let service = FakeService::start(
        INSTANCE,
        read_answer(json!({"ok": true}), Some("aud-7"), "complete", None),
    );
    let invoker = ConnectorsInvoker::new([endpoint(INSTANCE, &service.url())], Secrets)
        .expect("one endpoint per instance");
    let message = effect_error("write without an attempt", run(&invoker, INSTANCE));
    assert!(
        message.contains("no attempt"),
        "write without an attempt: {message}"
    );
    assert_eq!(service.invocations().len(), 1, "invoked once");
}

/// A binding whose effect is not the one the service describes for its operation (a write is the
/// mutation profile) is never invoked: a read binding of a write operation, a write binding of a
/// read operation, and a binding of an operation the service does not describe answer `Err` after
/// the describe alone.
#[test]
fn a_binding_the_service_describes_otherwise_is_never_invoked() {
    for (name, operation, effect, names) in [
        (
            "read binding of a write operation",
            OPERATION,
            ConnectorOperationEffect::Read,
            "mutation",
        ),
        (
            "write binding of a read operation",
            READ_OPERATION,
            ConnectorOperationEffect::Write,
            "read",
        ),
        (
            "an operation the service does not describe",
            "contents.delete",
            ConnectorOperationEffect::Write,
            "contents.delete",
        ),
    ] {
        let service = FakeService::start(INSTANCE, applied(json!({})));
        let invoker = ConnectorsInvoker::new([endpoint(INSTANCE, &service.url())], Secrets)
            .expect("one endpoint per instance");
        let message = effect_error(name, run_bound(&invoker, INSTANCE, operation, effect));
        assert!(message.contains(names), "{name}: {message}");
        let paths: Vec<String> = service.received().into_iter().map(|r| r.path).collect();
        assert_eq!(paths, ["/v1/describe"], "{name}: described, never invoked");
    }
}
