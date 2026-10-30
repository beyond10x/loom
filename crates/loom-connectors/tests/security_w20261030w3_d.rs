//! Security review of `story:connectors-invoker-keyed-writes` (loom wave 2026-10-30-w3, unit D).
//!
//! Integrity conformance checks of `ConnectorsInvoker`'s keyed writes against the story's own
//! contract, each against a fake Connectors service on a loopback port, driven through the
//! runtime so every request is one the runtime admitted:
//!
//! - `approval_required` and `idempotency_conflict` are `Err` naming the code and never
//!   `Refused`, whatever `mutation` a valid envelope carries with them;
//! - a replay naming another instance's attempt is never `Performed`;
//! - two admitted requests of two runs are sent under two keys, so a host with standing grants
//!   never answers one with the other's replay;
//! - refusal text carries neither the credential nor an input value;
//! - an admitted number is sent with its admitted spelling, whatever `serde_json` feature the
//!   build unified.

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
use b10x_loom_connectors::cli::idempotency_key;
use b10x_loom_connectors::{ConnectorsInvoker, CredentialError, CredentialResolver};
use serde_json::json;

const EDIT: &str = "repository.edit";
const INSTANCE: &str = "source-host";
const OTHER_INSTANCE: &str = "other-host";
const OPERATION: &str = "contents.write";
const REVISION: &str = "rev-7";
const CREDENTIAL: &str = "source-host-token";
const SECRET: &str = "s3cret-bearer-SECURITY-D";
/// An input value no refusal may repeat.
const PAYLOAD: &str = "PAYLOAD-must-not-leak-4711";
const NOW: &str = "2026-10-30T09:00:00Z";

// ---------------------------------------------------------------------------------------------
// The fake Connectors service.

/// How the fake answers an invocation, given its decoded body.
type Answering = Box<dyn Fn(&serde_json::Value) -> (u16, String) + Send + Sync>;

struct FakeService {
    addr: SocketAddr,
    bodies: Arc<Mutex<Vec<Vec<u8>>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeService {
    fn start(answer: Answering) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let addr = listener.local_addr().expect("the bound address");
        let bodies = Arc::new(Mutex::new(Vec::new()));
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
            let bodies = Arc::clone(&bodies);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    serve(stream, &bodies, &descriptor, &answer);
                }
            })
        };
        Self {
            addr,
            bodies,
            stop,
            thread: Some(thread),
        }
    }

    fn url(&self) -> String {
        format!("http://{}/", self.addr)
    }

    /// The raw bodies of every invocation received.
    fn invocations(&self) -> Vec<Vec<u8>> {
        self.bodies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
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

fn serve(stream: TcpStream, bodies: &Mutex<Vec<Vec<u8>>>, descriptor: &str, answer: &Answering) {
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
    let (status, text) = match (method.as_str(), path.as_str()) {
        ("GET", "/v1/describe") => (200, descriptor.to_owned()),
        ("POST", "/v1alpha2/invoke") => {
            bodies
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(body.clone());
            let decoded: serde_json::Value =
                serde_json::from_slice(&body).expect("the invocation body is JSON");
            answer(&decoded)
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

fn request_id(body: &serde_json::Value) -> String {
    body["request_id"].as_str().unwrap_or_default().to_owned()
}

/// A write that succeeded, naming attempt `attempt` of `instance`.
fn applied_as(instance: &'static str, attempt: &'static str, replayed: bool) -> Answering {
    Box::new(move |body| {
        let id = request_id(body);
        let original = if replayed {
            "the-original-request".to_owned()
        } else {
            id.clone()
        };
        let text = json!({
            "version": "v1alpha2",
            "request_id": id,
            "status": "success",
            "result": {"ok": true},
            "audit_ref": "aud-1",
            "audit_status": "complete",
            "mutation": {
                "classification": "applied",
                "attempt": {"instance": instance, "id": attempt},
                "original_request_id": original,
                "replayed": replayed,
                "cause": null
            }
        });
        (200, text.to_string())
    })
}

/// A keyed host's refusal `code` at HTTP `status`, carrying a `mutation` that states nothing
/// changed (`not_attempted`, no attempt): a valid envelope under the keyed rules.
fn keyed_refusal_stating_nothing_changed(status: u16, code: &'static str) -> Answering {
    Box::new(move |body| {
        let text = json!({
            "version": "v1alpha2",
            "request_id": request_id(body),
            "status": "error",
            "error": {"code": code, "message": "the fake service says so"},
            "audit_ref": null,
            "audit_status": "unavailable",
            "mutation": {
                "classification": "not_attempted",
                "attempt": null,
                "original_request_id": null,
                "replayed": false,
                "cause": null
            }
        });
        (status, text.to_string())
    })
}

/// A keyed host's refusal `code` at HTTP `status` with no `mutation`, request id echoed.
fn keyed_refusal(status: u16, code: &'static str) -> Answering {
    Box::new(move |body| {
        let text = json!({
            "version": "v1alpha2",
            "request_id": request_id(body),
            "status": "error",
            "error": {"code": code, "message": "the fake service says so"},
            "audit_ref": null,
            "audit_status": "unavailable"
        });
        (status, text.to_string())
    })
}

/// A host without standing grants: the key is refused before the envelope is read.
fn key_refused() -> Answering {
    Box::new(|_| {
        let text = json!({
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
        (400, text.to_string())
    })
}

/// A host with standing grants that keeps its keys as Connectors does (compatibility § 2.1): the
/// first use of a key performs the write and records an attempt; the same key with the same
/// input again is a replay of that attempt with no second write; with other input it is
/// `idempotency_conflict`. `performed` counts the writes it made.
fn keyed_host(performed: Arc<Mutex<Vec<String>>>) -> Answering {
    let ledger: Mutex<Vec<(String, serde_json::Value, String, String)>> = Mutex::new(Vec::new());
    Box::new(move |body| {
        let id = request_id(body);
        let key = body["idempotency_key"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let input = body["input"].clone();
        let mut ledger = ledger.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, stored, attempt, original)) = ledger.iter().find(|entry| entry.0 == key) {
            if *stored != input {
                return keyed_refusal(409, "idempotency_conflict")(body);
            }
            let text = json!({
                "version": "v1alpha2",
                "request_id": id,
                "status": "success",
                "result": {"ok": true},
                "audit_ref": "aud-replay",
                "audit_status": "complete",
                "mutation": {
                    "classification": "applied",
                    "attempt": {"instance": INSTANCE, "id": attempt},
                    "original_request_id": original,
                    "replayed": true,
                    "cause": null
                }
            });
            return (200, text.to_string());
        }
        let mut writes = performed.lock().unwrap_or_else(PoisonError::into_inner);
        let attempt = format!("00000000-0000-4000-8000-{:012x}", 0xa00 + writes.len() + 1);
        writes.push(attempt.clone());
        ledger.push((key, input, attempt.clone(), id.clone()));
        let text = json!({
            "version": "v1alpha2",
            "request_id": id,
            "status": "success",
            "result": {"ok": true},
            "audit_ref": "aud-write",
            "audit_status": "complete",
            "mutation": {
                "classification": "applied",
                "attempt": {"instance": INSTANCE, "id": attempt},
                "original_request_id": id,
                "replayed": false,
                "cause": null
            }
        });
        (200, text.to_string())
    })
}

// ---------------------------------------------------------------------------------------------
// The commission and its loop.

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// The commission `n`, with its own id and case.
fn commission(n: u64) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x80 + n)),
        agent_revision_id: AgentRevisionId(uuid(0x90)),
        case_id: CaseId(format!("case-edit-{n}")),
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
                message: format!("no secret under `{}`", credential.0),
            })
        }
    }
}

fn arguments() -> Value {
    Value::Object(vec![
        ("path".to_owned(), Value::Text("src/lib.rs".to_owned())),
        ("contents".to_owned(), Value::Text(PAYLOAD.to_owned())),
    ])
}

/// Ids from counters that start again for every run, as a host's ids "unique within one resume"
/// do (`crates/loom-agent/src/serve.rs`, `Ids`).
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

/// Keeps every request the runtime handed the invoker.
struct Recording<'a> {
    invoker: &'a ConnectorsInvoker,
    admitted: Mutex<Vec<AdmittedRequest>>,
}

impl<'a> Recording<'a> {
    fn new(invoker: &'a ConnectorsInvoker) -> Self {
        Self {
            invoker,
            admitted: Mutex::new(Vec::new()),
        }
    }

    fn admitted(&self) -> Vec<AdmittedRequest> {
        self.admitted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl ConnectorInvoker for Recording<'_> {
    fn invoke(
        &self,
        binding: &ActionBindingData,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.admitted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        self.invoker.invoke(binding, request)
    }
}

/// One run of commission `n` (its runs numbered from `0x100 + 0x10 * n`) proposing [`EDIT`] with
/// `arguments` once, through `invoker`.
fn run(invoker: impl ConnectorInvoker, n: u64, arguments: Value) -> Result<LoopEnd, LoopError> {
    let commission = commission(n);
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
    let base = 0x100 + 0x10 * n;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(base + issued))
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

fn invoker_for(service: &FakeService) -> ConnectorsInvoker {
    ConnectorsInvoker::new([endpoint(&service.url())], Secrets).expect("one endpoint")
}

/// The effect error a loop stopped with, or a panic naming what it ended with instead.
fn effect_error(name: &str, result: Result<LoopEnd, LoopError>) -> String {
    match result {
        Err(LoopError {
            failure: LoopFailure::Effect(error),
            ..
        }) => error.message,
        Ok(end) => panic!(
            "{name}: expected the effect port to fail, the loop ended with effects {:?}",
            end.effects
        ),
        Err(other) => panic!("{name}: expected the effect port to fail, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// The checks.

/// Story acceptance: `approval_required` and `idempotency_conflict` are `Err` naming the code,
/// "never `Refused`". A keyed host's envelope that carries a `mutation` stating nothing changed
/// with either code is valid under `decode_keyed` (§ 5 rules admit an error with a
/// `not_attempted` mutation); the invoker must still answer `Err`, after one request.
#[test]
fn approval_required_and_conflict_stating_nothing_changed_are_still_errors() {
    let mut wrong = Vec::new();
    for (code, status) in [("approval_required", 403), ("idempotency_conflict", 409)] {
        let service = FakeService::start(keyed_refusal_stating_nothing_changed(status, code));
        let invoker = invoker_for(&service);
        match run(&invoker, 1, arguments()) {
            Err(LoopError {
                failure: LoopFailure::Effect(error),
                ..
            }) => assert!(
                error.message.contains(code),
                "{code}: the error names it: {}",
                error.message
            ),
            other => wrong.push(format!("{code}: {other:?}")),
        }
        assert_eq!(
            service.invocations().len(),
            1,
            "{code}: one request, never resent"
        );
    }
    assert!(
        wrong.is_empty(),
        "answered other than Err naming the code:\n{}",
        wrong.join("\n")
    );
}

/// A replay (`replayed: true`) naming another instance's attempt is never `Performed`, after one
/// request.
#[test]
fn a_replay_naming_another_instances_attempt_is_an_error() {
    let service = FakeService::start(applied_as(
        OTHER_INSTANCE,
        "00000000-0000-4000-8000-0000000000b9",
        true,
    ));
    let invoker = invoker_for(&service);
    let message = effect_error("foreign replay", run(&invoker, 1, arguments()));
    assert!(
        message.contains(OTHER_INSTANCE),
        "the error names the other instance: {message}"
    );
    assert_eq!(service.invocations().len(), 1, "one request, never resent");
}

/// Two admitted requests of two runs (two commissions, cases and run ids) are two requests, so
/// they must be sent under two keys: a host with standing grants keeps a key for good and answers
/// its reuse with the original attempt's replay. With the key derived from the action request id
/// alone, a host whose ids are unique only within one run (as `loom-agent`'s per-resume `Ids`
/// are) sends both under one key, and the second request is reported `Performed` naming the
/// first request's attempt though the host made one write.
#[test]
fn admitted_requests_of_two_runs_are_sent_under_two_keys() {
    let performed = Arc::new(Mutex::new(Vec::new()));
    let service = FakeService::start(keyed_host(Arc::clone(&performed)));
    let invoker = invoker_for(&service);

    let first = Recording::new(&invoker);
    let first_end = run(&first, 1, arguments()).expect("the first run ends");
    let second = Recording::new(&invoker);
    let second_end = run(&second, 2, arguments()).expect("the second run ends");

    let (first, second) = (first.admitted(), second.admitted());
    assert_eq!(
        (first.len(), second.len()),
        (1, 1),
        "one admitted request each"
    );
    assert_ne!(
        (&first[0].data().run_id, &first[0].data().case_id),
        (&second[0].data().run_id, &second[0].data().case_id),
        "the two admitted requests belong to two runs of two cases"
    );
    assert_ne!(
        idempotency_key(&first[0]),
        idempotency_key(&second[0]),
        "two admitted requests of two runs are sent under one key, so the host answers the second \
         with the first's replay"
    );
    let attempts = |end: &LoopEnd| -> Vec<Option<ConnectorAttemptId>> {
        end.effects
            .iter()
            .map(|effect| match effect {
                EffectOutcome::Performed(performed) => performed.attempt.clone(),
                _ => None,
            })
            .collect()
    };
    assert_ne!(
        attempts(&first_end),
        attempts(&second_end),
        "the second request is Performed naming the first request's attempt; the host made {:?}",
        performed.lock().unwrap_or_else(PoisonError::into_inner)
    );
}

/// Refusal and error text of a keyed write carries neither the resolved credential nor an input
/// value: `approval_required`, `idempotency_conflict`, `rate_limited` and the `unsupported`
/// refusal of the key by a host without standing grants.
#[test]
fn keyed_refusal_text_carries_neither_credential_nor_input() {
    let answers: Vec<(&str, Answering)> = vec![
        ("approval_required", keyed_refusal(403, "approval_required")),
        (
            "idempotency_conflict",
            keyed_refusal(409, "idempotency_conflict"),
        ),
        ("rate_limited", keyed_refusal(429, "rate_limited")),
        ("no standing grants", key_refused()),
    ];
    for (name, answer) in answers {
        let service = FakeService::start(answer);
        let invoker = invoker_for(&service);
        let message = effect_error(name, run(&invoker, 1, arguments()));
        for withheld in [SECRET, PAYLOAD] {
            assert!(
                !message.contains(withheld),
                "{name}: the error repeats `{withheld}`: {message}"
            );
        }
        assert_eq!(
            service.invocations().len(),
            1,
            "{name}: one request, never resent"
        );
    }
}

/// The decimal value a JSON number's text denotes, as (negative, significant digits, exponent);
/// zero is `(false, "", 0)` whatever its sign, as the invoker's own exactness check reads it.
fn denotes(text: &str) -> (bool, String, i64) {
    let (negative, rest) = text
        .strip_prefix('-')
        .map_or((false, text), |rest| (true, rest));
    let (mantissa, exponent) = match rest.find(['e', 'E']) {
        Some(at) => (
            &rest[..at],
            rest[at + 1..].parse::<i64>().expect("an exponent"),
        ),
        None => (rest, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{whole}{fraction}");
    let digits = digits.trim_start_matches('0');
    let trimmed = digits.trim_end_matches('0');
    if trimmed.is_empty() {
        return (false, String::new(), 0);
    }
    let shift = i64::try_from(digits.len() - trimmed.len()).expect("small");
    let places = i64::try_from(fraction.len()).expect("small");
    (negative, trimmed.to_owned(), exponent - places + shift)
}

/// An admitted number goes on the wire denoting its admitted value: a value beyond `f64`, beyond
/// `u64` and with more significant digits than `f64` keeps them, so the write Connectors receives
/// is the one the runtime admitted, whichever `serde_json` features the build unified. (The
/// spelling may change: `1e400` is sent `1e+400`, `-0` is sent `0`.)
#[test]
fn admitted_numbers_are_sent_with_their_admitted_spelling() {
    let spellings = [
        "1e400",
        "18446744073709551617",
        "0.1000000000000000000001",
        "-0",
    ];
    let service = FakeService::start(applied_as(
        INSTANCE,
        "00000000-0000-4000-8000-0000000000a1",
        false,
    ));
    let invoker = invoker_for(&service);
    let numbers = Value::Array(
        spellings
            .iter()
            .map(|spelling| Value::Number((*spelling).to_owned()))
            .collect(),
    );
    let outcome = run(
        &invoker,
        1,
        Value::Object(vec![("numbers".to_owned(), numbers)]),
    );
    let invocations = service.invocations();
    match outcome {
        Ok(end) => {
            assert!(
                matches!(&end.effects[..], [EffectOutcome::Performed(_)]),
                "{:?}",
                end.effects
            );
            assert_eq!(invocations.len(), 1, "one request");
            let body = String::from_utf8(invocations[0].clone()).expect("UTF-8");
            let start = body.find("\"numbers\":[").expect("the numbers are sent") + 11;
            let end = start + body[start..].find(']').expect("the array closes");
            let sent: Vec<&str> = body[start..end].split(',').collect();
            assert_eq!(sent.len(), spellings.len(), "{body}");
            for (admitted, sent) in spellings.iter().zip(sent) {
                assert_eq!(
                    denotes(sent),
                    denotes(admitted),
                    "`{admitted}` is sent as `{sent}`, another value: {body}"
                );
            }
        }
        // Refusing a number before any call also keeps its meaning.
        Err(error) => assert!(invocations.is_empty(), "{error:?} after {invocations:?}"),
    }
}
