// SPDX-License-Identifier: Apache-2.0

//! `story:llm-credentials-bearer`: a ported wire takes its credential from the reference a run
//! configuration names (`loom.run.WireCredential`), resolved by an injected llm-credentials
//! `SecretResolver` at call time.
//!
//! Every endpoint here is a local listener that records the headers it was sent and answers 400,
//! so a turn ends right after the request that carried the credential. No network, no credential
//! file: the resolver is a fake holding values written in this file.

use std::collections::BTreeMap;
use std::future::Future;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use b10x_loom_executor::credentials::{
    ResolvedBearer, decode_wire_credential, encode_wire_credential,
};
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
const REFERENCE: &str = "codex-login";
const SECRET: &str = "SECRET-from-the-fake-resolver";

type Future_<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The headers of one recorded request, in the order they arrived.
type Headers = Vec<(String, String)>;

/// Answers each reference it holds with its value; any other reference is `Missing`, and one
/// named in `refusing` is refused with `Unavailable`.
struct FakeResolver {
    secrets: BTreeMap<String, String>,
    refusing: Vec<String>,
    asked: Mutex<Vec<String>>,
}

impl FakeResolver {
    fn holding(reference: &str, value: &str) -> Self {
        Self {
            secrets: BTreeMap::from([(reference.to_owned(), value.to_owned())]),
            refusing: Vec::new(),
            asked: Mutex::new(Vec::new()),
        }
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().expect("lock").clone()
    }
}

impl SecretResolver for FakeResolver {
    fn resolve<'a>(
        &'a self,
        reference: &'a SecretRef,
    ) -> Future_<'a, Result<ResolvedSecret, SecretError>> {
        Box::pin(async move {
            // Awaits once, so a resolver that is really asynchronous is what the bridge drives.
            tokio::task::yield_now().await;
            self.asked
                .lock()
                .expect("lock")
                .push(reference.as_str().to_owned());
            if self.refusing.iter().any(|name| name == reference.as_str()) {
                return Err(SecretError::Unavailable);
            }
            let value = self
                .secrets
                .get(reference.as_str())
                .ok_or(SecretError::Missing)?;
            Ok(ResolvedSecret {
                secret: Secret::new(value.clone().into_bytes())?,
                version: SecretVersion::new("generation-1".to_owned())?,
            })
        })
    }
}

fn credential(reference: &str, kind: CredentialKind) -> WireCredential {
    WireCredential {
        reference: CredentialReference(reference.to_owned()),
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

// --- the recorded local endpoint ----------------------------------------------------------------

/// A local endpoint that records every request's headers and refuses it with a 400.
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

    fn requests(&self) -> Vec<Headers> {
        self.headers.lock().expect("lock").clone()
    }

    fn header(&self, request: usize, name: &str) -> Option<String> {
        self.requests()
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

// --- acceptance ---------------------------------------------------------------------------------

#[test]
fn a_responses_wire_sends_the_secret_the_resolver_returns_for_an_oauth_reference() {
    let endpoint = Recorder::start();
    let resolver = Arc::new(FakeResolver::holding(REFERENCE, SECRET));
    let bearer = ResolvedBearer::new(
        &credential(REFERENCE, CredentialKind::Oauth),
        resolver.clone(),
    )
    .expect("a valid reference");
    let mut client = ResponsesClient::new(
        responses::Endpoint::new(endpoint.base_url.clone(), MODEL, 200_000).expect("endpoint"),
        Arc::new(bearer),
    )
    .expect("client");

    let _ = client.turn(&request(), &mut VecSink::new());

    assert_eq!(
        endpoint.header(0, "authorization").as_deref(),
        Some(format!("Bearer {SECRET}").as_str()),
        "the request carries the resolved secret as its bearer"
    );
    assert_eq!(resolver.asked(), vec![REFERENCE.to_owned()]);
}

#[test]
fn a_messages_wire_presents_an_oauth_reference_as_a_token_obtained_for_a_person() {
    let endpoint = Recorder::start();
    let resolver = Arc::new(FakeResolver::holding(REFERENCE, SECRET));
    let bearer = ResolvedBearer::new(&credential(REFERENCE, CredentialKind::Oauth), resolver)
        .expect("a valid reference");
    assert_eq!(bearer.kind(), CredentialKind::Oauth);
    let mut client = MessagesClient::new(
        messages::Endpoint::new(endpoint.base_url.clone(), MODEL, 200_000).expect("endpoint"),
        Arc::new(bearer),
    )
    .expect("client");

    let _ = client.turn(&request(), &mut VecSink::new());

    assert_eq!(
        endpoint.header(0, messages::OAUTH_HEADER).as_deref(),
        Some(format!("Bearer {SECRET}").as_str())
    );
    assert_eq!(endpoint.header(0, messages::API_KEY_HEADER), None);
}

#[test]
fn an_api_key_reference_is_presented_as_a_key() {
    let resolver = Arc::new(FakeResolver::holding("program-key", SECRET));
    let bearer = ResolvedBearer::new(&credential("program-key", CredentialKind::ApiKey), resolver)
        .expect("a valid reference");
    assert_eq!(bearer.kind(), CredentialKind::ApiKey);
    assert_eq!(bearer.bearer().expect("resolved").expose(), SECRET);
}

#[tokio::test]
async fn the_bridge_resolves_inside_a_running_runtime() {
    // A wire is blocking, but its caller may be on a Tokio thread; the bridge must neither panic
    // with a nested runtime nor deadlock the one it is called from.
    let resolver = Arc::new(FakeResolver::holding(REFERENCE, SECRET));
    let bearer = ResolvedBearer::new(&credential(REFERENCE, CredentialKind::Oauth), resolver)
        .expect("a valid reference");
    assert_eq!(bearer.bearer().expect("resolved").expose(), SECRET);
}

// --- checks -------------------------------------------------------------------------------------

#[test]
fn a_missing_reference_stops_the_call_naming_the_reference_and_never_a_secret() {
    let endpoint = Recorder::start();
    let resolver = Arc::new(FakeResolver::holding("another-login", SECRET));
    let bearer = ResolvedBearer::new(&credential(REFERENCE, CredentialKind::Oauth), resolver)
        .expect("a valid reference");
    let mut client = ResponsesClient::new(
        responses::Endpoint::new(endpoint.base_url.clone(), MODEL, 200_000).expect("endpoint"),
        Arc::new(bearer),
    )
    .expect("client");

    let error = client
        .turn(&request(), &mut VecSink::new())
        .expect_err("no credential, no call");

    assert_eq!(error.code, WireErrorCode::Unauthorized, "{error:?}");
    assert!(error.message.contains(REFERENCE), "{error:?}");
    assert!(!error.message.contains(SECRET), "{error:?}");
    assert!(!format!("{error:?}").contains(SECRET));
    assert!(
        endpoint.requests().is_empty(),
        "nothing reached the endpoint"
    );
}

#[test]
fn a_resolver_refusal_stops_the_call_naming_the_reference_and_never_a_secret() {
    let resolver = Arc::new(FakeResolver {
        secrets: BTreeMap::from([(REFERENCE.to_owned(), SECRET.to_owned())]),
        refusing: vec![REFERENCE.to_owned()],
        asked: Mutex::new(Vec::new()),
    });
    let bearer = ResolvedBearer::new(&credential(REFERENCE, CredentialKind::Oauth), resolver)
        .expect("a valid reference");

    let error = bearer.bearer().expect_err("refused");

    assert_eq!(error.code, WireErrorCode::Unauthorized, "{error:?}");
    assert!(error.message.contains(REFERENCE), "{error:?}");
    assert!(!error.message.contains(SECRET), "{error:?}");
}

#[test]
fn a_reference_the_resolver_could_never_look_up_is_refused_when_the_source_is_built() {
    let resolver = Arc::new(FakeResolver::holding(REFERENCE, SECRET));
    let error = ResolvedBearer::new(&credential("", CredentialKind::Oauth), resolver)
        .expect_err("an empty reference names nothing");
    assert_eq!(error.code, WireErrorCode::Unauthorized, "{error:?}");
}

#[test]
fn the_configuration_serialises_with_no_secret_value_in_it() {
    let configured = credential(REFERENCE, CredentialKind::Oauth);
    let resolver = Arc::new(FakeResolver::holding(REFERENCE, SECRET));
    let bearer = ResolvedBearer::new(&configured, resolver).expect("a valid reference");
    // The source has been used, so the secret has existed in this process.
    assert_eq!(bearer.bearer().expect("resolved").expose(), SECRET);

    let encoded = encode_wire_credential(&configured);
    let text = serde_json::to_string(&encoded).expect("encodes");

    assert_eq!(
        encoded,
        serde_json::json!({"reference": REFERENCE, "kind": "oauth"})
    );
    assert!(!text.contains(SECRET), "{text}");
    assert_eq!(
        decode_wire_credential(&encoded).expect("decodes"),
        configured
    );
    assert!(!format!("{bearer:?}").contains(SECRET));
}
