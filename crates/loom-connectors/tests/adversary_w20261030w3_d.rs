//! Adversary pass on `story:connectors-invoker-keyed-writes` (wave 2026-10-30-w3, unit D).
//!
//! `ConnectorsInvoker` sends every write keyed (`invoke_v1alpha2_keyed`) with
//! `cli::idempotency_key` of the admitted request, and the Connectors pins moved to `v0.62.0`,
//! whose `connectors-core` turns on `serde_json`'s `arbitrary_precision` for every crate built
//! with it. These cases drive the invoker against a fake Connectors service on a loopback port
//! that keeps a key ledger as a host with standing grants does (compatibility § 2.1, Keyed writes
//! under a standing grant), and read what the invoker sent with Connectors' own request decoder.

use std::collections::BTreeMap;
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
    ConnectorAttemptId, ConnectorCredentialRef, ConnectorEndpoint, ConnectorEndpointData,
    ConnectorEndpointUrl, ConnectorInstanceId, ConnectorOperationEffect, ConnectorOperationId,
    EffectOutcome, ExecutorOutcome, ExecutorOutcomeProposedAction, FrontierAction, ObservationId,
    PrincipalId, ProposedActionArguments, RunId, action_binding_state, commission_state,
    connector_endpoint_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::connector::{ConnectorEffects, ConnectorInvoker};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError};
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, LoopError, LoopFailure, run_until_blocked,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_connectors::{ConnectorsInvoker, CredentialError, CredentialResolver};
use serde_json::json;

const EDIT: &str = "repository.edit";
const INSTANCE: &str = "source-host";
const OPERATION: &str = "contents.write";
const REVISION: &str = "rev-7";
const CREDENTIAL: &str = "source-host-token";
const SECRET: &str = "s3cret-bearer-ADV-D";
const NOW: &str = "2026-10-30T09:00:00Z";
/// The text of the argument every run proposes; an error must not carry it.
const PATH_ARGUMENT: &str = "src/private-path-ADV-D.rs";

// ---------------------------------------------------------------------------------------------
// A fake Connectors service whose answer sees the whole invocation body.

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

type Answering = Box<dyn Fn(&serde_json::Value) -> (u16, String) + Send + Sync>;

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
        ("POST", "/v1alpha2/invoke") => answer(&request.json()),
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

/// A host with standing grants, as compatibility § 2.1 describes it: a key not seen before
/// records a new attempt and applies the write; the same key with the same operation and input
/// is a replay of the original attempt (`replayed: true`, `original_request_id` the original's);
/// the same key with other input is `idempotency_conflict`. Every grant covers every write.
fn keyed_host() -> Answering {
    #[derive(Clone)]
    struct Original {
        operation: serde_json::Value,
        input: serde_json::Value,
        request_id: String,
        attempt: String,
    }
    let ledger: Mutex<BTreeMap<String, Original>> = Mutex::new(BTreeMap::new());
    Box::new(move |body| {
        let request_id = body["request_id"].as_str().unwrap_or_default().to_owned();
        let Some(key) = body["idempotency_key"].as_str() else {
            return error_answer(&request_id, 400, "invalid_input", None);
        };
        let mut ledger = ledger.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(original) = ledger.get(key).cloned() {
            if original.operation != body["operation"] || original.input != body["input"] {
                return error_answer(&request_id, 409, "idempotency_conflict", None);
            }
            let reply = json!({
                "version": "v1alpha2",
                "request_id": request_id,
                "status": "success",
                "result": {"written": true},
                "audit_ref": "aud-replay",
                "audit_status": "complete",
                "mutation": {
                    "classification": "applied",
                    "attempt": {"instance": INSTANCE, "id": original.attempt},
                    "original_request_id": original.request_id,
                    "replayed": true,
                    "cause": null
                }
            });
            return (200, reply.to_string());
        }
        let attempt = format!("00000000-0000-4000-8000-0000000a{:04x}", ledger.len() + 1);
        ledger.insert(
            key.to_owned(),
            Original {
                operation: body["operation"].clone(),
                input: body["input"].clone(),
                request_id: request_id.clone(),
                attempt: attempt.clone(),
            },
        );
        let reply = json!({
            "version": "v1alpha2",
            "request_id": request_id,
            "status": "success",
            "result": {"written": true},
            "audit_ref": "aud-new",
            "audit_status": "complete",
            "mutation": {
                "classification": "applied",
                "attempt": {"instance": INSTANCE, "id": attempt},
                "original_request_id": request_id,
                "replayed": false,
                "cause": null
            }
        });
        (200, reply.to_string())
    })
}

/// An error envelope for `request_id` with `code` at HTTP `status`, with `mutation` if given.
fn error_answer(
    request_id: &str,
    status: u16,
    code: &str,
    mutation: Option<serde_json::Value>,
) -> (u16, String) {
    let mut body = json!({
        "version": "v1alpha2",
        "request_id": request_id,
        "status": "error",
        "error": {"code": code, "message": "the fake service says so"},
        "audit_ref": null,
        "audit_status": "unavailable"
    });
    if let Some(mutation) = mutation {
        body["mutation"] = mutation;
        body["audit_ref"] = json!("aud-1");
        body["audit_status"] = json!("complete");
    }
    (status, body.to_string())
}

// ---------------------------------------------------------------------------------------------
// The commission and its loop.

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(id: u64) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(id)),
        agent_revision_id: AgentRevisionId(uuid(0x90)),
        case_id: CaseId("case-edit".to_owned()),
        principal: PrincipalId("principal-c".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn binding(
    commission: &Commission<commission_state::Assigned>,
) -> ActionBinding<action_binding_state::Declared> {
    let id = commission.data().commission_id.clone();
    ActionBinding::new(ActionBindingData {
        binding: ActionBindingKey {
            commission_id: id.clone(),
            action: EDIT.to_owned(),
        },
        commission_id: id,
        instance_id: ConnectorInstanceId(INSTANCE.to_owned()),
        operation_id: ConnectorOperationId(OPERATION.to_owned()),
        effect: ConnectorOperationEffect::Write,
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
                message: "no secret".to_owned(),
            })
        }
    }
}

fn path_arguments() -> Value {
    Value::Object(vec![(
        "path".to_owned(),
        Value::Text(PATH_ARGUMENT.to_owned()),
    )])
}

/// Ids from counters starting at the same place in every run, as every `LoopContext` in this
/// repository's tests and `loom-agent`'s `Ids` (a process-local counter) do.
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

/// One loop of commission `commission_id` admitting [`EDIT`] once with `arguments`, bound to the
/// write [`OPERATION`].
fn run(
    invoker: impl ConnectorInvoker,
    commission_id: u64,
    arguments: Value,
) -> Result<LoopEnd, LoopError> {
    let commission = commission(commission_id);
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
    let effects = ConnectorEffects::new(&commission, [binding(&commission)], invoker)
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

fn attempt_of(name: &str, end: &LoopEnd) -> ConnectorAttemptId {
    match &end.effects[..] {
        [EffectOutcome::Performed(performed)] => performed
            .attempt
            .clone()
            .unwrap_or_else(|| panic!("{name}: performed without an attempt")),
        other => panic!("{name}: not one Performed: {other:?}"),
    }
}

/// Hands every request to the invoker `times` times, as a host retrying one admitted request
/// after a lost answer does, and answers the last outcome.
struct Retrying<'a> {
    invoker: &'a ConnectorsInvoker,
    times: usize,
    outcomes: Mutex<Vec<EffectOutcome>>,
}

impl ConnectorInvoker for Retrying<'_> {
    fn invoke(
        &self,
        binding: &ActionBindingData,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let mut last = None;
        for _ in 0..self.times {
            let outcome = self.invoker.invoke(binding, request)?;
            self.outcomes
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(outcome.clone());
            last = Some(outcome);
        }
        Ok(last.expect("invoked at least once"))
    }
}

// ---------------------------------------------------------------------------------------------
// Key derivation.

/// RED. Two admitted requests of two commissions must never share a key: a keyed host replays
/// the second as the first (same operation, same input) and the invoker reports `Performed`
/// naming the first commission's attempt, though nothing was written for the second.
/// `cli::idempotency_key` is `loom-action-request-<action_request_id>` alone, and
/// `LoopContext::action_request_id` promises only "a new id for an action request", which a
/// counter per run or per process (every test context here, `loom-agent`'s `Ids`) gives again.
#[test]
fn two_commissions_admitting_the_same_request_id_never_share_a_key() {
    let service = FakeService::start(keyed_host());
    let invoker = ConnectorsInvoker::new([endpoint(&service.url())], Secrets)
        .expect("one endpoint per instance");
    let first = run(&invoker, 0x81, path_arguments()).expect("the first loop ends");
    let second = run(&invoker, 0x82, path_arguments()).expect("the second loop ends");

    let keys: Vec<serde_json::Value> = service
        .invocations()
        .iter()
        .map(|r| r.json()["idempotency_key"].clone())
        .collect();
    assert_eq!(keys.len(), 2, "one invocation per commission");
    assert_ne!(
        keys[0], keys[1],
        "two commissions' admitted requests share the idempotency key {}",
        keys[0]
    );
    assert_ne!(
        attempt_of("first", &first),
        attempt_of("second", &second),
        "the second commission's write is reported Performed with the first commission's attempt"
    );
}

/// A host retrying one admitted request keeps its key, and the keyed host's replay is
/// `Performed` naming the original attempt both times.
#[test]
fn a_retry_of_one_admitted_request_keeps_its_key_and_names_the_original_attempt() {
    let service = FakeService::start(keyed_host());
    let invoker = ConnectorsInvoker::new([endpoint(&service.url())], Secrets)
        .expect("one endpoint per instance");
    let retrying = Retrying {
        invoker: &invoker,
        times: 2,
        outcomes: Mutex::new(Vec::new()),
    };
    let end = run(&retrying, 0x81, path_arguments()).expect("the loop ends");
    let invocations = service.invocations();
    assert_eq!(invocations.len(), 2, "the host invoked twice");
    let (one, two) = (invocations[0].json(), invocations[1].json());
    assert!(one["idempotency_key"].is_string(), "keyed: {one}");
    assert_eq!(
        one["idempotency_key"], two["idempotency_key"],
        "a retry keeps the key"
    );
    assert_ne!(
        one["request_id"], two["request_id"],
        "each send is its own request"
    );
    let outcomes = retrying
        .outcomes
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let attempts: Vec<_> = outcomes
        .iter()
        .map(|outcome| match outcome {
            EffectOutcome::Performed(performed) => performed.attempt.clone(),
            other => panic!("not performed: {other:?}"),
        })
        .collect();
    assert_eq!(
        attempts[0], attempts[1],
        "the replay names the original attempt"
    );
    assert_eq!(
        attempt_of("retry", &end),
        attempts[0].clone().expect("an attempt")
    );
}

// ---------------------------------------------------------------------------------------------
// `unsupported`, and what the error names.

/// RED (constructed). Only the refusal of the key itself (`request_id: null`, compatibility
/// § 2.1, "Refused §4 members") names a missing standing grant. An `unsupported` answered in a
/// valid envelope for this request, such as `unsupported request version`, is not that refusal,
/// and the remedy the invoker attaches (configure `standing_grants`) would not help.
#[test]
fn an_unsupported_answer_for_this_request_is_not_labelled_a_missing_standing_grant() {
    let service = FakeService::start(Box::new(|body| {
        let request_id = body["request_id"].as_str().unwrap_or_default().to_owned();
        let reply = json!({
            "version": "v1alpha2",
            "request_id": request_id,
            "status": "error",
            "error": {"code": "unsupported", "message": "unsupported request version"},
            "audit_ref": null,
            "audit_status": "unavailable"
        });
        (400, reply.to_string())
    }));
    let invoker = ConnectorsInvoker::new([endpoint(&service.url())], Secrets)
        .expect("one endpoint per instance");
    let message = effect_error("version", run(&invoker, 0x81, path_arguments()));
    assert!(message.contains("unsupported"), "names the code: {message}");
    assert!(
        !message.contains("standing_grants"),
        "an unsupported version is labelled a missing standing grant: {message}"
    );
    assert_eq!(service.invocations().len(), 1, "invoked once, never resent");
}

/// A 404 without an envelope (the service does not serve `/v1alpha2/invoke`) is `unsupported`
/// from the client, and is not labelled a missing standing grant; nothing is resent.
#[test]
fn a_404_without_an_envelope_is_not_labelled_a_missing_standing_grant() {
    let service = FakeService::start(Box::new(|_| (404, String::new())));
    let invoker = ConnectorsInvoker::new([endpoint(&service.url())], Secrets)
        .expect("one endpoint per instance");
    let message = effect_error("404", run(&invoker, 0x81, path_arguments()));
    assert!(message.contains("unsupported"), "names the code: {message}");
    assert!(
        !message.contains("standing_grants"),
        "a 404 is labelled a missing standing grant: {message}"
    );
    let invocations = service.invocations();
    assert_eq!(invocations.len(), 1, "invoked once, never resent");
    assert!(
        invocations[0].json()["idempotency_key"].is_string(),
        "sent keyed"
    );
}

/// The standing-grant error and the keyed refusals carry neither the credential nor the
/// admitted arguments.
#[test]
fn keyed_refusals_carry_neither_the_credential_nor_the_arguments() {
    let answers: Vec<(&str, Answering)> = vec![
        (
            "no standing grants",
            Box::new(|_| {
                let reply = json!({
                    "version": "v1alpha2",
                    "request_id": null,
                    "status": "error",
                    "error": {
                        "code": "unsupported",
                        "message": "invocation member is not supported on this binding"
                    },
                    "audit_ref": null,
                    "audit_status": "unavailable"
                });
                (400, reply.to_string())
            }),
        ),
        (
            "approval_required",
            Box::new(|body| {
                error_answer(
                    body["request_id"].as_str().unwrap_or_default(),
                    403,
                    "approval_required",
                    None,
                )
            }),
        ),
        (
            "idempotency_conflict",
            Box::new(|body| {
                error_answer(
                    body["request_id"].as_str().unwrap_or_default(),
                    409,
                    "idempotency_conflict",
                    None,
                )
            }),
        ),
    ];
    for (name, answer) in answers {
        let service = FakeService::start(answer);
        let invoker = ConnectorsInvoker::new([endpoint(&service.url())], Secrets)
            .expect("one endpoint per instance");
        let message = effect_error(name, run(&invoker, 0x81, path_arguments()));
        for secret in [SECRET, PATH_ARGUMENT] {
            assert!(
                !message.contains(secret),
                "{name}: the error carries {secret}: {message}"
            );
        }
        assert_eq!(service.invocations().len(), 1, "{name}: invoked once");
    }
}

/// RED (constructed). `approval_required` and `idempotency_conflict` are `Err`, never `Refused`
/// (the story's acceptance). The invoker reads them as every other error, so one carrying a
/// `not_attempted` mutation is `Refused`, and the run goes on as if the agent's action had been
/// turned down.
#[test]
fn approval_required_and_idempotency_conflict_are_never_refused() {
    for (code, status) in [("approval_required", 403), ("idempotency_conflict", 409)] {
        let service = FakeService::start(Box::new(move |body| {
            let request_id = body["request_id"].as_str().unwrap_or_default();
            error_answer(
                request_id,
                status,
                code,
                Some(json!({
                    "classification": "not_attempted",
                    "attempt": {"instance": INSTANCE, "id": "00000000-0000-4000-8000-0000000000c1"},
                    "original_request_id": request_id,
                    "replayed": false,
                    "cause": null
                })),
            )
        }));
        let invoker = ConnectorsInvoker::new([endpoint(&service.url())], Secrets)
            .expect("one endpoint per instance");
        let result = run(&invoker, 0x81, path_arguments());
        assert!(
            !matches!(&result, Ok(end) if matches!(end.effects[..], [EffectOutcome::Refused(_)])),
            "{code}: answered Refused: {result:?}"
        );
    }
}

/// A replay whose original failed answers the original's outcome: refused stays `Refused`,
/// `outcome_unknown` stays `Err`, and a replay naming another instance's attempt is `Err`.
#[test]
fn a_replay_of_a_failed_original_or_of_another_instance_is_never_performed() {
    let replayed = |classification: &str, instance: &str| {
        json!({
            "classification": classification,
            "attempt": {"instance": instance, "id": "00000000-0000-4000-8000-0000000000d1"},
            "original_request_id": "the-original-request",
            "replayed": true,
            "cause": null
        })
    };
    let refused = replayed("refused", INSTANCE);
    let unknown = replayed("unknown", INSTANCE);
    let elsewhere = replayed("applied", "another-instance");
    let cases: Vec<(&str, Answering)> = vec![
        (
            "refused original",
            Box::new(move |body| {
                error_answer(
                    body["request_id"].as_str().unwrap_or_default(),
                    403,
                    "forbidden",
                    Some(refused.clone()),
                )
            }),
        ),
        (
            "unknown original",
            Box::new(move |body| {
                error_answer(
                    body["request_id"].as_str().unwrap_or_default(),
                    502,
                    "outcome_unknown",
                    Some(unknown.clone()),
                )
            }),
        ),
        (
            "another instance",
            Box::new(move |body| {
                let reply = json!({
                    "version": "v1alpha2",
                    "request_id": body["request_id"],
                    "status": "success",
                    "result": {},
                    "audit_ref": "aud-1",
                    "audit_status": "complete",
                    "mutation": elsewhere.clone()
                });
                (200, reply.to_string())
            }),
        ),
    ];
    for (name, answer) in cases {
        let service = FakeService::start(answer);
        let invoker = ConnectorsInvoker::new([endpoint(&service.url())], Secrets)
            .expect("one endpoint per instance");
        let result = run(&invoker, 0x81, path_arguments());
        match (name, &result) {
            ("refused original", Ok(end)) => assert!(
                matches!(end.effects[..], [EffectOutcome::Refused(_)]),
                "{name}: {result:?}"
            ),
            ("refused original", _) => panic!("{name}: {result:?}"),
            (_, Err(_)) => {}
            (_, Ok(end)) => assert!(
                !matches!(end.effects[..], [EffectOutcome::Performed(_)]),
                "{name}: performed: {result:?}"
            ),
        }
        assert_eq!(service.invocations().len(), 1, "{name}: invoked once");
    }
}

// ---------------------------------------------------------------------------------------------
// `arbitrary_precision`.

/// `rate_limited` with `retry_after_seconds` decodes with `arbitrary_precision` on and is `Err`
/// naming the code.
#[test]
fn rate_limited_with_retry_after_names_the_code() {
    let service = FakeService::start(Box::new(|body| {
        let reply = json!({
            "version": "v1alpha2",
            "request_id": body["request_id"],
            "status": "error",
            "error": {"code": "rate_limited", "message": "slow down", "retry_after_seconds": 30},
            "audit_ref": null,
            "audit_status": "unavailable"
        });
        (429, reply.to_string())
    }));
    let invoker = ConnectorsInvoker::new([endpoint(&service.url())], Secrets)
        .expect("one endpoint per instance");
    let message = effect_error("rate_limited", run(&invoker, 0x81, path_arguments()));
    assert!(
        message.contains("`rate_limited`"),
        "names the code: {message}"
    );
}

/// RED. The invoker refuses an argument number it "cannot send without changing its value"
/// (`to_serde`). It decides that by parsing the text as a `serde_json::Number`, which under
/// `arbitrary_precision` (on since the `v0.62.0` pin) keeps every spelling exactly, so the check
/// always passes. The Connectors host reads the request with `connectors_core::read_json`, which
/// reads "an integer outside i64/u64 or any fraction/exponent [as] the nearest f64": the
/// operation receives another number than the one admitted. Expected: what the host decodes is
/// the admitted number, or the invoker refuses before any call (as it did at the base).
#[test]
fn an_argument_the_connectors_host_reads_as_another_number_is_never_sent() {
    for admitted in [
        "18446744073709551617",
        "123456789012345678901234567890",
        "0.10000000000000000000000001",
    ] {
        let service = FakeService::start(keyed_host());
        let invoker = ConnectorsInvoker::new([endpoint(&service.url())], Secrets)
            .expect("one endpoint per instance");
        let arguments = Value::Object(vec![("id".to_owned(), Value::Number(admitted.to_owned()))]);
        let result = run(&invoker, 0x81, arguments);
        let invocations = service.invocations();
        if invocations.is_empty() {
            assert!(
                result.is_err(),
                "{admitted}: not sent, and not an error: {result:?}"
            );
            continue;
        }
        let decoded = connectors_core::v1alpha2::KeyedInvokeRequest::decode(&invocations[0].body)
            .unwrap_or_else(|refusal| {
                panic!("{admitted}: the host refuses the request: {refusal:?}")
            });
        let received = decoded.input["id"].to_string();
        assert_eq!(
            received,
            admitted,
            "{admitted}: the Connectors host reads the admitted argument as {received} (sent bytes: {})",
            String::from_utf8_lossy(&invocations[0].body)
        );
    }
}
