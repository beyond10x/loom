// SPDX-License-Identifier: Apache-2.0

//! Adversary cases for `story:llm-credentials-bearer` (wave 2026-10-08 w2).
//!
//! No model or network call: every endpoint is a local listener, every resolver a fake.

use std::future::Future;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use b10x_loom_executor::credentials::{ResolvedBearer, decode_wire_credential};
use b10x_loom_executor::harness::messages::{self, MessagesClient};
use b10x_loom_executor::harness::responses::{self, ResponsesClient};
use b10x_loom_executor::harness::wire::{
    BearerSource, Item, ModelPort, Sampling, ToolChoice, TurnRequest, VecSink, WireErrorCode,
};
use b10x_loom_executor::model::run::{CredentialKind, CredentialReference, WireCredential};
use llm_credentials::{
    ResolvedSecret, Secret, SecretError, SecretRef, SecretResolver, SecretVersion,
};

const MODEL: &str = "test-model";
const REFERENCE: &str = "adversary-login";
const SECRET: &str = "SECRET-adversary-value";

type Future_<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
type Headers = Vec<(String, String)>;

/// Answers every reference with fixed raw bytes, as llm's file and environment adapters do
/// ("nothing is trimmed or decoded").
struct RawResolver(Vec<u8>);

impl SecretResolver for RawResolver {
    fn resolve<'a>(
        &'a self,
        _reference: &'a SecretRef,
    ) -> Future_<'a, Result<ResolvedSecret, SecretError>> {
        Box::pin(async move {
            Ok(ResolvedSecret {
                secret: Secret::new(self.0.clone())?,
                version: SecretVersion::new("generation-1".to_owned())?,
            })
        })
    }
}

/// Resolves only once a task on the caller's runtime has run: an embedder's resolver that
/// answers from its own application runtime.
struct NeedsCallerRuntime(Arc<AtomicBool>);

impl SecretResolver for NeedsCallerRuntime {
    fn resolve<'a>(
        &'a self,
        _reference: &'a SecretRef,
    ) -> Future_<'a, Result<ResolvedSecret, SecretError>> {
        Box::pin(async move {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !self.0.load(Ordering::SeqCst) {
                if Instant::now() > deadline {
                    return Err(SecretError::Unavailable);
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Ok(ResolvedSecret {
                secret: Secret::new(SECRET.as_bytes().to_vec())?,
                version: SecretVersion::new("generation-1".to_owned())?,
            })
        })
    }
}

fn credential(kind: CredentialKind) -> WireCredential {
    WireCredential {
        reference: CredentialReference(REFERENCE.to_owned()),
        kind,
    }
}

fn request() -> TurnRequest {
    TurnRequest {
        model: MODEL.to_owned(),
        instructions: "be useful".to_owned(),
        items: vec![Item::user("hello")],
        tools: Vec::new(),
        max_output_tokens: None,
        sampling: Sampling::default(),
        tool_choice: ToolChoice::Auto,
    }
}

struct Recorder {
    base_url: String,
    headers: Arc<Mutex<Vec<Headers>>>,
}

impl Recorder {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base_url = format!("http://{}/v1", listener.local_addr().expect("address"));
        let headers = Arc::new(Mutex::new(Vec::new()));
        let recorded = headers.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                serve(stream, &recorded);
            }
        });
        Self { base_url, headers }
    }

    fn header(&self, request: usize, name: &str) -> Option<String> {
        self.headers
            .lock()
            .expect("lock")
            .get(request)?
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.clone())
    }
}

fn serve(mut stream: TcpStream, recorded: &Mutex<Vec<Headers>>) {
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut headers = Vec::new();
    let mut length = 0usize;
    loop {
        line.clear();
        if reader.read_line(&mut line).is_err() {
            return;
        }
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else {
            return;
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            length = value.trim().parse().unwrap_or(0);
        }
        headers.push((name.trim().to_owned(), value.trim().to_owned()));
    }
    let mut body = vec![0; length];
    if reader.read_exact(&mut body).is_err() {
        return;
    }
    recorded.lock().expect("lock").push(headers);
    let refusal = r#"{"error":{"message":"recorded and refused"}}"#;
    let response = format!(
        "HTTP/1.1 400 Bad Request\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
         connection: close\r\n\r\n{refusal}",
        refusal.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// The unit's API-key test checks `kind()` only; this drives the header the Messages wire sends.
#[test]
fn adv_a_messages_wire_presents_an_api_key_reference_under_the_key_header_only() {
    let endpoint = Recorder::start();
    let bearer = ResolvedBearer::new(
        &credential(CredentialKind::ApiKey),
        Arc::new(RawResolver(SECRET.as_bytes().to_vec())),
    )
    .expect("a valid reference");
    let mut client = MessagesClient::new(
        messages::Endpoint::new(endpoint.base_url.clone(), MODEL, 200_000).expect("endpoint"),
        Arc::new(bearer),
    )
    .expect("client");

    let _ = client.turn(&request(), &mut VecSink::new());

    assert_eq!(
        endpoint.header(0, messages::API_KEY_HEADER).as_deref(),
        Some(SECRET)
    );
    assert_eq!(endpoint.header(0, messages::OAUTH_HEADER), None);
}

/// llm's file and environment adapters return raw bytes, untrimmed, and leave validation to the
/// consumer. A key file written with `echo` ends in a newline. The call must stop as a credential
/// refusal naming the reference, not as a retriable transport failure that names nothing.
#[test]
fn adv_a_secret_with_a_trailing_newline_is_refused_as_a_credential_naming_the_reference() {
    let endpoint = Recorder::start();
    let bearer = ResolvedBearer::new(
        &credential(CredentialKind::Oauth),
        Arc::new(RawResolver(format!("{SECRET}\n").into_bytes())),
    )
    .expect("a valid reference");
    let mut client = ResponsesClient::new(
        responses::Endpoint::new(endpoint.base_url.clone(), MODEL, 200_000).expect("endpoint"),
        Arc::new(bearer),
    )
    .expect("client");

    let error = client
        .turn(&request(), &mut VecSink::new())
        .expect_err("an unpresentable credential, no call");

    assert!(!format!("{error:?}").contains(SECRET), "{error:?}");
    assert_eq!(error.code, WireErrorCode::Unauthorized, "{error:?}");
    assert!(!error.retriable, "{error:?}");
    assert!(error.message.contains(REFERENCE), "{error:?}");
}

/// A secret carrying CR/LF must never be offered to the transport as a header value.
#[test]
fn adv_a_secret_carrying_a_header_break_is_refused_by_the_source() {
    let bearer = ResolvedBearer::new(
        &credential(CredentialKind::Oauth),
        Arc::new(RawResolver(
            format!("{SECRET}\r\nx-injected: yes").into_bytes(),
        )),
    )
    .expect("a valid reference");

    let error = bearer
        .bearer()
        .expect_err("a value no header can carry is refused at the source");

    assert_eq!(error.code, WireErrorCode::Unauthorized, "{error:?}");
    assert!(error.message.contains(REFERENCE), "{error:?}");
    assert!(!error.message.contains(SECRET), "{error:?}");
}

/// An empty resolved secret: the wire refuses it, but the refusal must name the reference
/// (Checks: an error that names the reference and never the secret value).
#[test]
fn adv_an_empty_resolved_secret_is_refused_naming_the_reference() {
    let bearer = ResolvedBearer::new(
        &credential(CredentialKind::Oauth),
        Arc::new(RawResolver(Vec::new())),
    )
    .expect("a valid reference");
    let mut client = ResponsesClient::new(
        responses::Endpoint::new("http://127.0.0.1:9/v1", MODEL, 200_000).expect("endpoint"),
        Arc::new(bearer),
    )
    .expect("client");

    let error = client
        .turn(&request(), &mut VecSink::new())
        .expect_err("an empty credential, no call");

    assert_eq!(error.code, WireErrorCode::Unauthorized, "{error:?}");
    assert!(error.message.contains(REFERENCE), "{error:?}");
}

/// The doc on `ResolvedBearer` says the bridge "never blocks one [runtime] it does not own".
/// Called on a current-thread runtime, it blocks that runtime's only thread until the resolver
/// answers, so a resolver that needs the caller's runtime to make progress never does.
#[test]
fn adv_the_bridge_does_not_block_the_callers_runtime() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let ready = Arc::new(AtomicBool::new(false));
    let bearer = ResolvedBearer::new(
        &credential(CredentialKind::Oauth),
        Arc::new(NeedsCallerRuntime(ready.clone())),
    )
    .expect("a valid reference");

    let result = runtime.block_on(async move {
        let flag = ready.clone();
        tokio::spawn(async move { flag.store(true, Ordering::SeqCst) });
        bearer.bearer()
    });

    assert!(
        result.is_ok(),
        "the caller's runtime was blocked: {result:?}"
    );
}

/// A configuration value that is not a reference and a kind is refused with an error that echoes
/// no value: an operator who pastes a secret into the wrong field must not see it in a log.
#[test]
fn adv_a_configuration_decode_error_never_echoes_a_value() {
    let misplaced = serde_json::json!({"reference": REFERENCE, "kind": SECRET});

    let error = decode_wire_credential(&misplaced).expect_err("not a kind");

    assert!(!error.message.contains(SECRET), "{error:?}");
}
