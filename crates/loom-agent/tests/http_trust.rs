//! Acceptance for `story:http-binding-cluster-ca`: an `http` binding reads an `https` endpoint a
//! cluster CA signs when the host setting `LOOM_HTTP_CA_FILE` names that CA's PEM file, and
//! `loom.agent.AdmitHttpTrust` refuses a file that cannot be read, is past 1048576 bytes or holds
//! no certificate. The CA and the endpoint's certificate are minted when the test runs (no key is
//! committed), the endpoint is a TLS listener on the loopback, and no network is reached. The
//! variable's value is passed to `http_trust`, never set in this process's environment.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use b10x_loom_agent::http::{CA_FILE_ENV, CA_FILE_MAX_BYTES, HttpTrust, http_trust};
use b10x_loom_agent::{
    AdmitHttpTrust, AdmitHttpTrustOutcome, HttpGet, HttpTrustRefusalReason, admit_http_trust,
    http_trust_refusal_spelling,
};
use b10x_loom_plugin::datasource::SourceName;
use b10x_loom_plugin::effects::{HttpAnswer, HttpReadError, HttpReader};
use loom_sdk::agent::HttpBinding;
use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    KeyUsagePurpose,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// What the endpoint answers.
const BODY: &str = r#"{"status":"success","data":["job","instance"]}"#;

/// A test CA, and an endpoint certificate for `127.0.0.1` and `localhost` it signed.
struct Minted {
    /// The CA's certificate, PEM: what `LOOM_HTTP_CA_FILE` names.
    ca_pem: String,
    /// The endpoint's certificate and key, DER.
    endpoint: (CertificateDer<'static>, PrivateKeyDer<'static>),
}

fn mint() -> Minted {
    let ca_key = KeyPair::generate().expect("a CA key");
    let mut ca = CertificateParams::new(Vec::<String>::new()).expect("CA parameters");
    ca.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca.distinguished_name
        .push(DnType::CommonName, "loom test cluster CA");
    ca.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let ca_cert = ca.self_signed(&ca_key).expect("a self-signed CA");
    let key = KeyPair::generate().expect("an endpoint key");
    let mut endpoint = CertificateParams::new(vec!["127.0.0.1".to_owned(), "localhost".to_owned()])
        .expect("endpoint parameters");
    endpoint
        .distinguished_name
        .push(DnType::CommonName, "loom test endpoint");
    endpoint.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let cert = endpoint
        .signed_by(&key, &ca_cert, &ca_key)
        .expect("an endpoint certificate the CA signed");
    Minted {
        ca_pem: ca_cert.pem(),
        endpoint: (
            cert.der().clone(),
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der())),
        ),
    }
}

/// A TLS listener on the loopback presenting `endpoint`, and its `https` base URL.
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

/// Answers the first connection with `200` and [`BODY`] once the client completes the handshake
/// and sends its request; returns the request's head, or `None` when the handshake or the request
/// never came (the client refused the certificate).
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

fn binding(base_url: &str) -> HttpBinding {
    HttpBinding {
        name: SourceName("logs".to_owned()),
        base_url: base_url.to_owned(),
        allow_paths: vec!["/api/v1/labels".to_owned()],
        method: "GET".to_owned(),
        bearer_env: None,
        max_bytes: 4096,
    }
}

/// A directory of this test's own under cargo's scratch directory for integration tests.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("http-trust")
        .join(format!("{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// Writes `bytes` to `name` in `dir` and returns its path.
fn file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("a scratch file");
    path
}

/// The trust `path` gives, as the host decides it from the variable's value.
fn trust_of(path: &Path) -> Result<HttpTrust, String> {
    http_trust(Some(path.as_os_str())).map_err(|refused| refused.to_string())
}

/// The refusal of `path`: its reason, and the message the host prints.
fn refused(path: &Path) -> (HttpTrustRefusalReason, String) {
    match http_trust(Some(path.as_os_str())) {
        Ok(trust) => panic!("{} was taken: {trust:?}", path.display()),
        Err(refused) => (refused.reason, refused.to_string()),
    }
}

/// The message names the variable and the reason in the enum's spelling, and none of the file.
fn names_the_variable_and_not_the_file(message: &str, reason: HttpTrustRefusalReason, file: &[u8]) {
    assert!(message.contains(CA_FILE_ENV), "{message}");
    assert!(
        message.contains(http_trust_refusal_spelling(reason)),
        "{message}"
    );
    assert!(
        !message.contains("BEGIN"),
        "the file is never printed: {message}"
    );
    let text = String::from_utf8_lossy(file);
    for line in text.lines().filter(|line| line.len() > 16) {
        assert!(
            !message.contains(line),
            "the file is never printed: {message}"
        );
    }
}

#[test]
fn without_the_setting_an_endpoint_a_cluster_ca_signs_is_unavailable() {
    let minted = mint();
    let (listener, config, base) = listen_tls(minted.endpoint);
    let served = answer_once(listener, config);
    let reader = HttpGet::with_timeout(Duration::from_secs(10));
    match reader.read(&binding(&base), "/api/v1/labels", &[]) {
        Err(HttpReadError::Unavailable(why)) => {
            assert!(why.starts_with("the request failed"), "{why}");
        }
        other => panic!("not unavailable: {other:?}"),
    }
    assert_eq!(
        served.join().unwrap(),
        None,
        "no request reached the endpoint"
    );
}

#[test]
fn with_the_ca_file_the_endpoint_a_cluster_ca_signs_is_read() {
    let minted = mint();
    let dir = scratch("read");
    let ca = file(&dir, "ca.pem", minted.ca_pem.as_bytes());
    let trust = trust_of(&ca).expect("the CA file is taken");
    assert_eq!(trust.roots(), 1);
    let (listener, config, base) = listen_tls(minted.endpoint);
    let served = answer_once(listener, config);
    let reader = HttpGet::with_timeout(Duration::from_secs(10)).with_trust(trust);
    let answer = reader
        .read(&binding(&base), "/api/v1/labels", &[])
        .expect("the endpoint is read");
    assert_eq!(
        answer,
        HttpAnswer {
            status: 200,
            body: BODY.to_owned(),
            cut: false,
        }
    );
    let head = served
        .join()
        .unwrap()
        .expect("the request reached the endpoint");
    assert!(
        head.starts_with("GET /api/v1/labels HTTP/1.1\r\n"),
        "{head}"
    );
}

/// A bundle of several CAs is taken whole, each a root beside the webpki roots.
#[test]
fn a_ca_bundle_extends_the_roots_by_each_of_its_certificates() {
    let dir = scratch("bundle");
    let bundle = format!("{}{}", mint().ca_pem, mint().ca_pem);
    let trust = trust_of(&file(&dir, "bundle.pem", bundle.as_bytes())).expect("a bundle is taken");
    assert_eq!(trust.roots(), 2);
    assert_eq!(HttpTrust::default().roots(), 0);
}

#[test]
fn an_unset_empty_or_blank_variable_keeps_the_webpki_roots_alone() {
    for value in [None, Some(""), Some("  "), Some("\n")] {
        let trust = http_trust(value.map(OsStr::new)).expect("no setting is no refusal");
        assert_eq!(trust.roots(), 0, "{value:?}");
    }
}

#[test]
fn a_missing_ca_file_is_refused_unreadable() {
    let dir = scratch("missing");
    let (reason, message) = refused(&dir.join("no-such-ca.pem"));
    assert_eq!(reason, HttpTrustRefusalReason::Unreadable);
    names_the_variable_and_not_the_file(&message, reason, b"");
    let (reason, _) = refused(&dir);
    assert_eq!(reason, HttpTrustRefusalReason::Unreadable, "a directory");
}

#[test]
fn an_empty_ca_file_is_refused_no_certificate() {
    let dir = scratch("empty");
    let (reason, message) = refused(&file(&dir, "empty.pem", b""));
    assert_eq!(reason, HttpTrustRefusalReason::NoCertificate);
    names_the_variable_and_not_the_file(&message, reason, b"");
}

#[test]
fn a_ca_file_holding_no_certificate_is_refused_no_certificate() {
    let dir = scratch("no-certificate");
    let key = KeyPair::generate().expect("a key");
    let public = key.public_key_pem();
    let garbage = "-----BEGIN CERTIFICATE-----\naGVsbG8gd29ybGQsIHRoaXMgaXMgbm8gY2VydGlmaWNhdGU=\n-----END CERTIFICATE-----\n";
    // A good certificate beside one no TLS client takes: the file is taken whole or not at all.
    let mixed = format!("{}{garbage}", mint().ca_pem);
    for (name, bytes) in [
        ("mixed.pem", mixed.as_bytes().to_vec()),
        ("public-key.pem", public.as_bytes().to_vec()),
        (
            "text.pem",
            b"the cluster CA lives in the service account's ca.crt\n".to_vec(),
        ),
        ("not-a-certificate.pem", garbage.as_bytes().to_vec()),
    ] {
        let (reason, message) = refused(&file(&dir, name, &bytes));
        assert_eq!(reason, HttpTrustRefusalReason::NoCertificate, "{name}");
        names_the_variable_and_not_the_file(&message, reason, &bytes);
    }
}

#[test]
fn a_ca_file_over_one_mebibyte_is_refused_too_large() {
    let dir = scratch("too-large");
    let pem = mint().ca_pem;
    let bound = usize::try_from(CA_FILE_MAX_BYTES).unwrap();
    // A certificate padded to exactly the bound is taken; one byte more is refused, whatever the
    // file holds.
    let mut at_bound = vec![b'\n'; bound - pem.len()];
    at_bound.extend_from_slice(pem.as_bytes());
    assert_eq!(at_bound.len(), bound);
    let trust = trust_of(&file(&dir, "at-bound.pem", &at_bound)).expect("1048576 bytes are taken");
    assert_eq!(trust.roots(), 1);
    let mut past = vec![b'\n'];
    past.extend_from_slice(&at_bound);
    let (reason, message) = refused(&file(&dir, "past-bound.pem", &past));
    assert_eq!(reason, HttpTrustRefusalReason::TooLarge);
    names_the_variable_and_not_the_file(&message, reason, pem.as_bytes());
}

/// The decision itself, as `task conform` drives it: not named is `webpki-only` whatever the rest
/// says, and named, the first refusal in the order unreadable, too large, no certificate.
#[test]
fn admit_http_trust_decides_in_its_declared_order() {
    let decide = |named, read, bytes, certificates| {
        admit_http_trust(&AdmitHttpTrust {
            named,
            read,
            bytes,
            certificates,
        })
    };
    assert_eq!(
        decide(false, false, 9_999_999, 0),
        AdmitHttpTrustOutcome::WebpkiOnly
    );
    assert!(matches!(
        decide(true, false, 9_999_999, 0),
        AdmitHttpTrustOutcome::Unreadable { .. }
    ));
    assert!(matches!(
        decide(true, true, CA_FILE_MAX_BYTES + 1, 0),
        AdmitHttpTrustOutcome::TooLarge { .. }
    ));
    assert!(matches!(
        decide(true, true, CA_FILE_MAX_BYTES, 0),
        AdmitHttpTrustOutcome::NoCertificate { .. }
    ));
    assert_eq!(
        decide(true, true, CA_FILE_MAX_BYTES, 1),
        AdmitHttpTrustOutcome::Extended
    );
}
