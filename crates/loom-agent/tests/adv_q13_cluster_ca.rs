//! Adversary cases for `story:http-binding-cluster-ca` (wave 2026-10-26-q13, unit u3): the trust
//! reaching a served turn through `SpecPlugin::with_http_trust`, and the file kinds and encodings
//! `LOOM_HTTP_CA_FILE` may name that the unit's own suite does not try. Every endpoint is a TLS
//! listener on the loopback; every certificate is minted when the test runs.

#[path = "../../loom-plugin/tests/support/mod.rs"]
mod support;

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use b10x_loom_agent::http::{CA_FILE_MAX_BYTES, http_trust};
use b10x_loom_agent::{HttpTrustRefusalReason, SpecPlugin};
use b10x_loom_plugin::{Host, Intent, Plugin};
use b10x_loom_plugin_slack::TurnModels;
use loom_sdk::agent::parse;
use rcgen::{BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde_json::json;
use support::{Fixture, Recorded, Scripted, classification, item};

const BODY: &str = r#"{"status":"success","data":[]}"#;

struct Minted {
    ca_pem: String,
    ca_der: Vec<u8>,
    ca_key_pem: String,
    endpoint: (CertificateDer<'static>, PrivateKeyDer<'static>),
}

fn mint() -> Minted {
    let ca_key = KeyPair::generate().expect("a CA key");
    let mut ca = CertificateParams::new(Vec::<String>::new()).expect("CA parameters");
    ca.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca.distinguished_name
        .push(DnType::CommonName, "loom adversary cluster CA");
    let ca_cert = ca.self_signed(&ca_key).expect("a self-signed CA");
    let key = KeyPair::generate().expect("an endpoint key");
    let mut endpoint = CertificateParams::new(vec!["127.0.0.1".to_owned()]).expect("parameters");
    endpoint.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let cert = endpoint
        .signed_by(&key, &ca_cert, &ca_key)
        .expect("an endpoint certificate");
    Minted {
        ca_pem: ca_cert.pem(),
        ca_der: ca_cert.der().to_vec(),
        ca_key_pem: ca_key.serialize_pem(),
        endpoint: (
            cert.der().clone(),
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der())),
        ),
    }
}

fn listen_tls(
    endpoint: (CertificateDer<'static>, PrivateKeyDer<'static>),
) -> (TcpListener, Arc<rustls::ServerConfig>, String) {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("protocol versions")
        .with_no_client_auth()
        .with_single_cert(vec![endpoint.0], endpoint.1)
        .expect("a server configuration");
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let base = format!(
        "https://127.0.0.1:{}",
        listener.local_addr().expect("an address").port()
    );
    (listener, Arc::new(config), base)
}

/// The request head the first connection sent after the handshake, or `None`.
fn answer_once(
    listener: TcpListener,
    config: Arc<rustls::ServerConfig>,
) -> JoinHandle<Option<String>> {
    thread::spawn(move || {
        let (socket, _) = listener.accept().ok()?;
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .ok()?;
        let connection = rustls::ServerConnection::new(config).ok()?;
        let mut tls = rustls::StreamOwned::new(connection, socket);
        let mut seen = Vec::new();
        let mut buffer = [0; 1024];
        while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
            let length = tls.read(&mut buffer).ok()?;
            if length == 0 {
                return None;
            }
            seen.extend_from_slice(&buffer[..length]);
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{BODY}",
            BODY.len()
        );
        tls.write_all(response.as_bytes()).ok()?;
        tls.conn.send_close_notify();
        tls.flush().ok()?;
        tls.sock.shutdown(Shutdown::Both).ok();
        Some(String::from_utf8_lossy(&seen).into_owned())
    })
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("adv-q13-cluster-ca")
        .join(format!("{name}-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("a fresh scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn reason_of(path: &Path) -> Result<usize, (HttpTrustRefusalReason, String)> {
    http_trust(Some(path.as_os_str()))
        .map(|trust| trust.roots())
        .map_err(|refused| (refused.reason, refused.to_string()))
}

fn spec_text(fixture: &Fixture, base: &str) -> String {
    format!(
        "format: loom.agent/1
agent: {{id: test-operator, name: Test operator, brief: Answer questions from the logs.}}
model:
  routes: routes.yaml
  roles: [{{role: agent, model: small}}, {{role: classifier, model: small}}]
loops:
  outer: {{poll_interval_seconds: 0, sources: [chat], min_age_minutes: 10, lookback_minutes: 120}}
  inner: {{protocol: operator-reply@1, step_budget: 8, model_budget: 16, classify_threshold: 0.5}}
states:
  - {{name: answer, when: {{intent: Ask}}, actions: [source.read, reply.propose, reply.send, reply.decline], sources: [logs]}}
bindings:
  - {{name: chat, kind: connectors, adapter: slack, connection: slack-fixture, list: conversations.list, get: conversations.history, search: conversations.replies, write: chat.postMessage}}
  - {{name: logs, kind: http, base_url: {base}, allow_paths: [/api/v1/query], method: GET, max_bytes: 4096}}
channel: {{binding: chat, allow: [C0FIXTURE1], bot_user_id: U0BOT}}
writes: {{mode: approval, max_per_hour: 5}}
policy: {{capabilities: [datasource.read, reply.propose, reply.send]}}
runtime:
  state_dir: {state}
  connectors: {{program: {program}, state_dir: {connectors}}}
  confinement: none
",
        state = fixture.plugin_state().display(),
        program = fixture.root.join("connectors").display(),
        connectors = fixture.connectors_state().display(),
    )
}

/// The served agent's turn, built as `agent serve` builds it (`SpecPlugin::with_http_trust`),
/// reads an `https` binding a cluster CA signs: the trust reaches the plugin's reader, not only
/// a hand-built `HttpGet`. Nothing in the unit's suite drives `with_http_trust`.
#[test]
fn adv_a_served_turn_reads_an_https_binding_through_the_plugin_trust() {
    let minted = mint();
    let dir = scratch("served");
    let ca = dir.join("ca.pem");
    std::fs::write(&ca, minted.ca_pem.as_bytes()).unwrap();
    let trust = http_trust(Some(ca.as_os_str())).expect("the CA file is taken");
    let fixture = Fixture::new("adv-q13-cluster-ca", "served");
    std::fs::create_dir_all(fixture.plugin_state()).unwrap();
    let (listener, config, base) = listen_tls(minted.endpoint);
    let served = answer_once(listener, config);
    let spec = parse(&spec_text(&fixture, &base)).expect("the spec decodes");
    let (model, _seen) = Scripted::new(vec![
        (
            "source_read",
            json!({"source": "logs", "kind": "get", "input": {"path": "/api/v1/query", "query": {"query": "up"}}}),
        ),
        ("reply_propose", json!({"text": "Nothing is wrong."})),
        (
            "reply_send",
            json!({"channel": "C0FIXTURE1", "text": "Nothing is wrong."}),
        ),
    ]);
    let slot = Mutex::new(Some(model));
    let turns: TurnModels = Box::new(move || {
        Ok(slot
            .lock()
            .unwrap()
            .take()
            .expect("one turn")
            .into_turn_model())
    });
    let plugin = SpecPlugin::new(spec, &fixture.plugin_state())
        .expect("the spec is served")
        .with_classifier(Box::new(Recorded::classifying("ask", &["logs"], 0.9)))
        .with_turn_models(turns)
        .with_http_trust(trust);
    let host = Host::new(plugin.config()).with_environment(fixture.environment());
    let item = item("item-1");
    let classified = classification(Intent::Ask, &["logs"], "0.9");
    let projection = plugin.project(&host, &item, &classified);
    let _ = plugin.turn(&host, &item, &classified, &projection);
    let head = served
        .join()
        .unwrap()
        .expect("the served turn's read completed the handshake with the cluster CA's endpoint");
    assert!(
        head.starts_with("GET /api/v1/query?query=up HTTP/1.1\r\n"),
        "{head}"
    );
}

/// A character device is not a regular file: `/dev/zero` is `Unreadable`, not read to the bound
/// and refused `TooLarge`.
#[cfg(target_os = "linux")]
#[test]
fn adv_a_character_device_is_unreadable() {
    let (reason, message) = reason_of(Path::new("/dev/zero")).expect_err("a device is refused");
    assert_eq!(reason, HttpTrustRefusalReason::Unreadable, "{message}");
    assert!(
        message.contains("LOOM_HTTP_CA_FILE: Unreadable"),
        "{message}"
    );
}

/// A FIFO with no writer is refused `Unreadable` at once; opening it would block the host.
#[cfg(unix)]
#[test]
fn adv_a_fifo_is_unreadable_without_blocking() {
    let dir = scratch("fifo");
    let fifo = dir.join("ca.pem");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo runs");
    assert!(made.success(), "mkfifo {}", fifo.display());
    let (send, receive) = mpsc::channel();
    let path = fifo.clone();
    thread::spawn(move || {
        let _ = send.send(reason_of(&path).map_err(|(reason, _)| reason));
    });
    let answer = receive.recv_timeout(Duration::from_secs(10));
    if answer.is_err() {
        // Release the blocked reader before failing, so the test process can end.
        drop(std::fs::OpenOptions::new().write(true).open(&fifo));
    }
    assert_eq!(
        answer.expect("http_trust returned within 10 s on a FIFO"),
        Err(HttpTrustRefusalReason::Unreadable)
    );
}

/// A symlink to a CA file is the CA file (a Kubernetes Secret volume's keys are symlinks); a
/// symlink to a device is the device.
#[cfg(unix)]
#[test]
fn adv_a_symlink_is_its_target() {
    let dir = scratch("symlink");
    let ca = dir.join("..data-ca.pem");
    std::fs::write(&ca, mint().ca_pem.as_bytes()).unwrap();
    let link = dir.join("ca.crt");
    std::os::unix::fs::symlink(&ca, &link).unwrap();
    assert_eq!(reason_of(&link), Ok(1));
    let device = dir.join("zero.pem");
    std::os::unix::fs::symlink("/dev/zero", &device).unwrap();
    assert_eq!(
        reason_of(&device).map_err(|(reason, _)| reason),
        Err(HttpTrustRefusalReason::Unreadable)
    );
    let dangling = dir.join("dangling.pem");
    std::os::unix::fs::symlink(dir.join("nothing"), &dangling).unwrap();
    assert_eq!(
        reason_of(&dangling).map_err(|(reason, _)| reason),
        Err(HttpTrustRefusalReason::Unreadable)
    );
}

/// A DER certificate is not PEM: `NoCertificate`, and none of its bytes are printed.
#[test]
fn adv_a_der_certificate_is_no_certificate() {
    let dir = scratch("der");
    let minted = mint();
    let path = dir.join("ca.der");
    std::fs::write(&path, &minted.ca_der).unwrap();
    let (reason, message) = reason_of(&path).expect_err("DER is refused");
    assert_eq!(reason, HttpTrustRefusalReason::NoCertificate, "{message}");
}

/// A file holding only a private key holds no certificate, and the key is never printed; a key
/// beside a certificate is skipped, the certificate taken.
#[test]
fn adv_a_private_key_is_never_a_root_and_never_printed() {
    let dir = scratch("key");
    let minted = mint();
    let key_only = dir.join("tls.key");
    std::fs::write(&key_only, minted.ca_key_pem.as_bytes()).unwrap();
    let (reason, message) = reason_of(&key_only).expect_err("a key alone is refused");
    assert_eq!(reason, HttpTrustRefusalReason::NoCertificate, "{message}");
    assert!(!message.contains("PRIVATE KEY"), "{message}");
    for line in minted.ca_key_pem.lines().filter(|line| line.len() > 16) {
        assert!(!message.contains(line), "the key is printed: {message}");
    }
    let both = dir.join("tls.pem");
    std::fs::write(&both, format!("{}{}", minted.ca_key_pem, minted.ca_pem)).unwrap();
    assert_eq!(reason_of(&both), Ok(1));
}

/// A bundle in the shape distributions ship (a label line and a blank line between blocks) is
/// taken whole.
#[test]
fn adv_text_between_pem_blocks_is_taken() {
    let dir = scratch("labelled");
    let bundle = format!(
        "# loom adversary bundle\n\nCA one\n======\n{}\nCA two\n======\n{}\n",
        mint().ca_pem,
        mint().ca_pem
    );
    let path = dir.join("bundle.pem");
    std::fs::write(&path, bundle).unwrap();
    assert_eq!(reason_of(&path), Ok(2));
}

/// Exactly the bound of bytes holding no certificate is `NoCertificate`, not `TooLarge`; one
/// byte more of the same is `TooLarge`.
#[test]
fn adv_the_bound_separates_no_certificate_from_too_large() {
    let dir = scratch("bound");
    let bound = usize::try_from(CA_FILE_MAX_BYTES).unwrap();
    let at = dir.join("at.pem");
    std::fs::write(&at, vec![b'x'; bound]).unwrap();
    assert_eq!(
        reason_of(&at).map_err(|(reason, _)| reason),
        Err(HttpTrustRefusalReason::NoCertificate)
    );
    let past = dir.join("past.pem");
    std::fs::write(&past, vec![b'x'; bound + 1]).unwrap();
    let (reason, message) = reason_of(&past).expect_err("past the bound");
    assert_eq!(reason, HttpTrustRefusalReason::TooLarge, "{message}");
    assert!(!message.contains("xxxxxxxx"), "{message}");
}

/// A value that is not valid UTF-8 still names a path (not trimmed, not `webpki-only`).
#[cfg(unix)]
#[test]
fn adv_a_non_utf8_value_names_a_path() {
    use std::os::unix::ffi::OsStrExt;
    let value = OsStr::from_bytes(b"/nonexistent-\xff-ca.pem");
    let refused = http_trust(Some(value)).expect_err("a path that does not exist");
    assert_eq!(refused.reason, HttpTrustRefusalReason::Unreadable);
}
