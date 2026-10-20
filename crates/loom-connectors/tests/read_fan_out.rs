//! Acceptance for `story:dw-read-fanout`, through bindings: three reads an executor proposes
//! together, each bound to its own Connector read operation, are invoked through
//! `ConnectorEffects` over [`ConnectorsInvoker`] once each, on `POST /v1alpha2/invoke`, and each is
//! `Performed` naming the audit record the service completed for it. A fan-out listing an action
//! bound to a write operation invokes nothing.
//!
//! The fake service binds a loopback port inside this process, as in `connectors_invoker.rs`: it
//! answers `GET /v1/describe` with a v1alpha1 descriptor of the three read operations and the
//! write [`WRITE_OPERATION`] (profile `mutation`), and `POST /v1alpha2/invoke` with a read's
//! success whose `audit_ref` names the operation invoked. The runtime's own fan-out rules are held
//! by `crates/loom-commission-testkit/tests/read_fan_out.rs`.

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
    ConnectorAuditRef, ConnectorCredentialRef, ConnectorEndpoint, ConnectorEndpointData,
    ConnectorEndpointUrl, ConnectorInstanceId, ConnectorOperationEffect, ConnectorOperationId,
    EffectOutcome, ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorOutcomeProposedReads,
    FrontierAction, ObservationId, PrincipalId, ProposedActionArguments, RunId, RunOutcome,
    RunOutcomeIdleBound, action_binding_state, commission_state, connector_endpoint_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::connector::ConnectorEffects;
use b10x_loom_commission::ports::effect::EffectPort;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, LoopError, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_connectors::{ConnectorsInvoker, CredentialError, CredentialResolver};
use serde_json::json;

/// The Connector instance every action is bound to.
const INSTANCE: &str = "incident-sources";
/// The read actions, each bound to the read operation of the same name.
const READS: [&str; 3] = ["logs.read", "metrics.read", "deploys.read"];
/// An action bound to the write operation.
const WRITE: &str = "ticket.write";
/// The write operation the service describes.
const WRITE_OPERATION: &str = "ticket.write";
/// The credential reference the endpoint names, and the secret the resolver gives for it.
const CREDENTIAL: &str = "incident-sources-token";
const SECRET: &str = "s3cret-bearer";
/// The trusted time the loop's context gives.
const NOW: &str = "2026-10-15T09:00:00Z";

// ---------------------------------------------------------------------------------------------
// The fake Connectors service.

/// One request the fake service received.
#[derive(Debug, Clone)]
struct Received {
    path: String,
    body: Vec<u8>,
}

impl Received {
    fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).expect("the invocation body is JSON")
    }
}

/// A Connectors service on a loopback port of this process, answering every invocation with a
/// read's success that names the audit record `aud-<operation>`.
struct FakeService {
    addr: SocketAddr,
    received: Arc<Mutex<Vec<Received>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeService {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let addr = listener.local_addr().expect("the bound address");
        let received = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let read = |id: &str| {
            json!({
                "id": id,
                "description": "read an incident source",
                "contract": "operations/v1alpha1",
                "profile": "read",
                "input_schema": {"type": "object"},
                "output_schema": {"type": "object"}
            })
        };
        let mut operations: Vec<serde_json::Value> = READS.iter().map(|id| read(id)).collect();
        operations.push(json!({
            "id": WRITE_OPERATION,
            "description": "open a ticket",
            "contract": "operations/v1alpha1",
            "profile": "mutation",
            "input_schema": {"type": "object"},
            "output_schema": {"type": "object"}
        }));
        let descriptor = json!({
            "version": "v1alpha1",
            "instance": INSTANCE,
            "adapter": "fake-incident-sources",
            "revision": "rev-1",
            "operations": operations,
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
                    serve(stream, &received, &descriptor);
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

    fn invocations(&self) -> Vec<Received> {
        self.received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|r| r.path == "/v1alpha2/invoke")
            .cloned()
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
fn serve(stream: TcpStream, received: &Mutex<Vec<Received>>, descriptor: &str) {
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
            let invoked = request.json();
            let operation = invoked["operation"].as_str().unwrap_or_default();
            let answer = json!({
                "version": "v1alpha2",
                "request_id": invoked["request_id"],
                "status": "success",
                "result": {"read": operation},
                "audit_ref": format!("aud-{operation}"),
                "audit_status": "complete"
            });
            (200, answer.to_string())
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

// ---------------------------------------------------------------------------------------------
// The commission and its loop.

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x81)),
        agent_revision_id: AgentRevisionId(uuid(0x90)),
        case_id: CaseId("case-incident".to_owned()),
        principal: PrincipalId("principal-c".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// `action` bound to the operation of the same name on [`INSTANCE`], with `effect`.
fn binding(
    commission: &Commission<commission_state::Assigned>,
    action: &str,
    effect: ConnectorOperationEffect,
) -> ActionBinding<action_binding_state::Declared> {
    let id = commission.data().commission_id.clone();
    ActionBinding::new(ActionBindingData {
        binding: ActionBindingKey {
            commission_id: id.clone(),
            action: action.to_owned(),
        },
        commission_id: id,
        instance_id: ConnectorInstanceId(INSTANCE.to_owned()),
        operation_id: ConnectorOperationId(action.to_owned()),
        effect,
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

fn reads(actions: &[&str]) -> ExecutorOutcome {
    ExecutorOutcome::ProposedReads(ExecutorOutcomeProposedReads {
        reads: actions
            .iter()
            .map(|action| ExecutorOutcomeProposedAction {
                action: (*action).to_owned(),
                arguments: ProposedActionArguments(Value::Object(vec![(
                    "window".to_owned(),
                    Value::Text("1h".to_owned()),
                )])),
            })
            .collect(),
    })
}

/// One loop of the commission whose frontier admits every action of `bindings`; the executor
/// proposes `proposed` together (twice, for a run whose fan-out is refused), and the case
/// completes after `open` governor calls.
fn run(
    invoker: &ConnectorsInvoker,
    bindings: &[(&str, ConnectorOperationEffect)],
    proposed: &[&str],
    open: usize,
) -> Result<LoopEnd, LoopError> {
    let commission = commission();
    let case = commission.data().case_id.clone();
    let listing = Answer::at(1).with_items(
        Vec::new(),
        Vec::new(),
        bindings
            .iter()
            .map(|(action, _)| FrontierAction {
                action: (*action).to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            })
            .collect(),
    );
    let governor = FakeGovernor::new();
    governor.script(
        case,
        repeat_n(listing.clone(), open).chain([listing.complete("collected")]),
    );
    let executor = ScriptedExecutor::new([reads(proposed), reads(proposed)]);
    let effects = ConnectorEffects::new(
        &commission,
        bindings
            .iter()
            .map(|(action, effect)| binding(&commission, action, *effect)),
        invoker,
    )
    .unwrap_or_else(|error| panic!("the bindings are refused: {error}"));
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

// ---------------------------------------------------------------------------------------------
// The acceptance.

/// Three reads proposed together: three invocations, one per bound operation, and three
/// `Performed` outcomes in proposal order, each naming the audit record the service completed for
/// its own operation and no attempt.
#[test]
fn each_read_is_invoked_once_through_its_binding() {
    let service = FakeService::start();
    let invoker = ConnectorsInvoker::new([endpoint(&service.url())], Secrets)
        .expect("one endpoint per instance");
    let bindings: Vec<_> = READS
        .iter()
        .map(|action| (*action, ConnectorOperationEffect::Read))
        .collect();

    let end = run(&invoker, &bindings, &READS, 3 + 2 * READS.len()).expect("the loop ends");

    assert_eq!(
        end.outcome,
        RunOutcome::Completed(
            b10x_loom_commission::model::responsibility::RunOutcomeCompleted {
                outcome: "collected".to_owned()
            }
        )
    );
    assert_eq!(end.steps, 1, "a fan-out is one step");
    let mut operations: Vec<String> = service
        .invocations()
        .iter()
        .map(|invocation| {
            invocation.json()["operation"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    operations.sort();
    let mut expected: Vec<String> = READS.iter().map(|s| (*s).to_owned()).collect();
    expected.sort();
    assert_eq!(
        operations, expected,
        "one invocation per bound read operation"
    );
    assert_eq!(end.effects.len(), 3, "{:?}", end.effects);
    for (effect, action) in end.effects.iter().zip(READS) {
        let EffectOutcome::Performed(performed) = effect else {
            panic!("{action} was not performed: {effect:?}");
        };
        assert_eq!(
            performed.audit,
            Some(ConnectorAuditRef(format!("aud-{action}"))),
            "{action} names its own audit record"
        );
        assert_eq!(performed.attempt, None, "{action} is a read");
        assert_eq!(
            performed.report,
            Value::Object(vec![("read".to_owned(), Value::Text(action.to_owned()))]),
            "{action}"
        );
    }
}

/// `ConnectorEffects` over `ConnectorsInvoker` reads exactly the actions bound to a read; a
/// fan-out that lists one bound to a write is refused whole and invokes nothing.
#[test]
fn a_fan_out_listing_a_bound_write_invokes_nothing() {
    let service = FakeService::start();
    let invoker = ConnectorsInvoker::new([endpoint(&service.url())], Secrets)
        .expect("one endpoint per instance");
    let commission = commission();
    let effects = ConnectorEffects::new(
        &commission,
        [
            binding(&commission, READS[0], ConnectorOperationEffect::Read),
            binding(&commission, WRITE, ConnectorOperationEffect::Write),
        ],
        &invoker,
    )
    .expect("the bindings");
    assert!(effects.reads(READS[0]), "a read binding reads");
    assert!(!effects.reads(WRITE), "a write binding does not read");
    assert!(!effects.reads(READS[1]), "an unbound action does not read");

    let bindings = [
        (READS[0], ConnectorOperationEffect::Read),
        (WRITE, ConnectorOperationEffect::Write),
    ];
    let end = run(&invoker, &bindings, &[READS[0], WRITE], 64).expect("the loop ends");

    assert_eq!(service.invocations().len(), 0, "nothing invoked");
    assert!(end.requests.is_empty(), "{:?}", end.requests);
    assert_eq!(
        end.outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 2 })
    );
}
