//! Adversary cases for `story:connector-read-performed`, pass 1.
//!
//! Each case drives `ConnectorEffects` over [`ConnectorsInvoker`] through Commission's own loop
//! against an in-process fake Connectors service, as `connectors_invoker.rs` does, with the action
//! bound to the read operation the service describes. The answers are ones that file does not try:
//!
//! - `a_read_naming_an_empty_audit_record_is_an_error`: a read's success whose `audit_ref` is the
//!   empty string. Compatibility § 5 of the Connectors service contract: "Record absence is never
//!   replaced by a plausible opaque string"; `ConnectorAuditRef` is documented as the public
//!   `audit_ref` of a record Connectors completed, and an empty text names none.
//! - `a_read_whose_failure_carries_a_recorded_attempt_is_an_error_not_a_refusal`: a read's error
//!   answer that carries a `mutation` (`refused` or `not_attempted`). Connectors records an attempt
//!   only for an `external_write` (compatibility § 2.1, `mutation`: "Absent for reads"), so the
//!   answer contradicts the describe the binding was checked against; the invoker already answers
//!   `Err` for a read's success that carries one, and the same contradiction on a failure must not
//!   be read as Connectors' own "nothing changed".
//! - `a_read_of_an_operation_the_service_does_not_describe_is_never_invoked`: the `Read` side of
//!   the undescribed-operation check, which the implementation leaves to the client.

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
    ConnectorCredentialRef, ConnectorEndpoint, ConnectorEndpointData, ConnectorEndpointUrl,
    ConnectorInstanceId, ConnectorOperationEffect, ConnectorOperationId, ExecutorOutcome,
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
use b10x_loom_connectors::{ConnectorsInvoker, CredentialError, CredentialResolver};
use serde_json::json;

const INSPECT: &str = "repository.inspect";
const INSTANCE: &str = "source-host";
const WRITE_OPERATION: &str = "contents.write";
const READ_OPERATION: &str = "contents.read";
const REVISION: &str = "rev-7";
const CREDENTIAL: &str = "source-host-token";
const SECRET: &str = "s3cret-bearer-read";
const ATTEMPT: &str = "00000000-0000-4000-8000-0000000000a1";
const NOW: &str = "2026-10-08T09:00:00Z";

// ---------------------------------------------------------------------------------------------
// The fake Connectors service.

#[derive(Debug, Clone)]
struct Received {
    path: String,
    body: Vec<u8>,
}

type Answering = Box<dyn Fn(&str) -> (u16, String) + Send + Sync>;

struct FakeService {
    addr: SocketAddr,
    received: Arc<Mutex<Vec<Received>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeService {
    fn start(answer: Answering) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let addr = listener.local_addr().expect("the bound address");
        let received = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let descriptor = json!({
            "version": "v1alpha1",
            "instance": INSTANCE,
            "adapter": "fake-source-host",
            "revision": REVISION,
            "operations": [{
                "id": WRITE_OPERATION,
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

    fn paths(&self) -> Vec<String> {
        self.received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|r| r.path.clone())
            .collect()
    }

    fn invocations(&self) -> usize {
        self.paths()
            .iter()
            .filter(|p| *p == "/v1alpha2/invoke")
            .count()
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
    let request = Received { path, body };
    received
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(request.clone());
    let (status, text) = match (method.as_str(), request.path.as_str()) {
        ("GET", "/v1/describe") => (200, descriptor.to_owned()),
        ("POST", "/v1alpha2/invoke") => {
            let parsed: serde_json::Value =
                serde_json::from_slice(&request.body).expect("the invocation body is JSON");
            let request_id = parsed["request_id"].as_str().unwrap_or_default().to_owned();
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

fn mutation(classification: &str, request_id: &str) -> serde_json::Value {
    json!({
        "classification": classification,
        "attempt": {"instance": INSTANCE, "id": ATTEMPT},
        "original_request_id": request_id,
        "replayed": false,
        "cause": null
    })
}

/// A read's success, no mutation, its audit record `audit_ref` `complete`.
fn read_success_with_audit(audit_ref: &'static str) -> Answering {
    Box::new(move |request_id| {
        let body = json!({
            "version": "v1alpha2",
            "request_id": request_id,
            "status": "success",
            "result": {"text": "fn main() {}"},
            "audit_ref": audit_ref,
            "audit_status": "complete"
        });
        (200, body.to_string())
    })
}

/// An error answer at HTTP `status` with `code`, carrying a mutation classified `classification`.
fn failed_with_attempt(status: u16, code: &'static str, classification: &'static str) -> Answering {
    Box::new(move |request_id| {
        let body = json!({
            "version": "v1alpha2",
            "request_id": request_id,
            "status": "error",
            "error": {"code": code, "message": "the fake service says so"},
            "audit_ref": "aud-1",
            "audit_status": "complete",
            "mutation": mutation(classification, request_id)
        });
        (status, body.to_string())
    })
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
        case_id: CaseId("case-inspect".to_owned()),
        principal: PrincipalId("principal-c".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn binding(
    commission: &Commission<commission_state::Assigned>,
    operation: &str,
) -> ActionBinding<action_binding_state::Declared> {
    let id = commission.data().commission_id.clone();
    ActionBinding::new(ActionBindingData {
        binding: ActionBindingKey {
            commission_id: id.clone(),
            action: INSPECT.to_owned(),
        },
        commission_id: id,
        instance_id: ConnectorInstanceId(INSTANCE.to_owned()),
        operation_id: ConnectorOperationId(operation.to_owned()),
        effect: ConnectorOperationEffect::Read,
    })
}

fn endpoint(url: &str) -> ConnectorEndpoint<connector_endpoint_state::Declared> {
    ConnectorEndpoint::new(ConnectorEndpointData {
        instance_id: ConnectorInstanceId(INSTANCE.to_owned()),
        url: ConnectorEndpointUrl(url.to_owned()),
        credential: ConnectorCredentialRef(CREDENTIAL.to_owned()),
        allow_plaintext: true,
    })
}

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

/// One loop with [`INSPECT`] bound as a `Read` to `operation` of the service at `url`.
fn run_read(url: &str, operation: &str) -> Result<LoopEnd, LoopError> {
    let commission = commission();
    let case = commission.data().case_id.clone();
    let open = |revision| {
        Answer::at(revision).with_items(
            Vec::new(),
            Vec::new(),
            vec![FrontierAction {
                action: INSPECT.to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            }],
        )
    };
    let governor = FakeGovernor::new();
    governor.script(
        case,
        repeat_n(open(3), 5).chain([open(4), open(4).complete("inspected")]),
    );
    let executor = ScriptedExecutor::new(vec![ExecutorOutcome::ProposedAction(
        ExecutorOutcomeProposedAction {
            action: INSPECT.to_owned(),
            arguments: ProposedActionArguments(Value::Object(vec![(
                "path".to_owned(),
                Value::Text("src/lib.rs".to_owned()),
            )])),
        },
    )]);
    let invoker = ConnectorsInvoker::new([endpoint(url)], Secrets).expect("one endpoint");
    let effects = ConnectorEffects::new(&commission, [binding(&commission, operation)], &invoker)
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
// The cases.

/// A read's `Performed` names the audit record Connectors completed. A success whose `audit_ref`
/// is the empty string names no record, so it is not performed: the answer is `Err`, after the one
/// invocation, never resent.
#[test]
fn a_read_naming_an_empty_audit_record_is_an_error() {
    let name = "empty audit_ref";
    let service = FakeService::start(read_success_with_audit(""));
    let message = effect_error(name, run_read(&service.url(), READ_OPERATION));
    assert!(!message.is_empty(), "{name}");
    assert_eq!(
        service.invocations(),
        1,
        "{name}: invoked once, never resent"
    );
}

/// Connectors records no attempt for a read. A read's error answer that carries one contradicts
/// the describe the binding was checked against, exactly as a read's success carrying one does
/// (which the invoker answers `Err`); it is not Connectors' own "nothing changed", so it is `Err`
/// and never `Refused`, after the one invocation.
#[test]
fn a_read_whose_failure_carries_a_recorded_attempt_is_an_error_not_a_refusal() {
    for (name, status, code, classification) in [
        ("read refused with an attempt", 403, "forbidden", "refused"),
        (
            "read not attempted with an attempt",
            503,
            "unavailable",
            "not_attempted",
        ),
    ] {
        let service = FakeService::start(failed_with_attempt(status, code, classification));
        let message = effect_error(name, run_read(&service.url(), READ_OPERATION));
        assert!(!message.is_empty(), "{name}");
        assert_eq!(
            service.invocations(),
            1,
            "{name}: invoked once, never resent"
        );
    }
}

/// A `Read` binding of an operation the service does not describe answers `Err` after the
/// describe alone; nothing is invoked.
#[test]
fn a_read_of_an_operation_the_service_does_not_describe_is_never_invoked() {
    let name = "undescribed read";
    let service = FakeService::start(read_success_with_audit("aud-1"));
    let message = effect_error(name, run_read(&service.url(), "contents.list"));
    assert!(message.contains("contents.list"), "{name}: {message}");
    assert_eq!(service.paths(), ["/v1/describe"], "{name}: never invoked");
}
