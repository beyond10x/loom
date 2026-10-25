//! Adversary cases for `story:agent-http-binding`: the bearer, the byte bound, the path policy and
//! the refusal text of an `http` binding's read. Every request goes to a listener on the
//! loopback; no network.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use b10x_loom_agent::{HttpGet, path_allowed};
use b10x_loom_plugin::datasource::SourceName;
use b10x_loom_plugin::effects::{HttpAnswer, HttpReadError, HttpReader};
use loom_sdk::agent::HttpBinding;

/// A variable cargo sets for a test process to the empty text: the package declares no homepage.
const EMPTY_VARIABLE: &str = "CARGO_PKG_HOMEPAGE";

fn listen() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    (listener, base)
}

/// Waits up to `wait` for one connection, answers it with `response` and returns the request
/// head; `None` when nothing connected.
fn answer_within(
    listener: TcpListener,
    wait: Duration,
    response: Vec<u8>,
) -> JoinHandle<Option<String>> {
    thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + wait;
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    if Instant::now() > deadline {
                        return None;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("accept: {error}"),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut seen = Vec::new();
        let mut buffer = [0; 1024];
        while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
            let length = socket.read(&mut buffer).expect("a readable socket");
            if length == 0 {
                break;
            }
            seen.extend_from_slice(&buffer[..length]);
        }
        let _ = socket.write_all(&response);
        socket.shutdown(std::net::Shutdown::Both).ok();
        Some(String::from_utf8_lossy(&seen).into_owned())
    })
}

fn ok(body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

fn binding(
    base_url: &str,
    allow: &[&str],
    bearer_env: Option<&str>,
    max_bytes: i64,
) -> HttpBinding {
    HttpBinding {
        name: SourceName("logs".to_owned()),
        base_url: base_url.to_owned(),
        allow_paths: allow.iter().map(|path| (*path).to_owned()).collect(),
        method: "GET".to_owned(),
        bearer_env: bearer_env.map(str::to_owned),
        max_bytes,
    }
}

fn reader() -> HttpGet {
    HttpGet::with_timeout(Duration::from_secs(5))
}

/// A `bearer_env` naming a variable that holds the empty text is a bearer the host has no value
/// for (`ess/domains/agent.yaml` `AdmitHttpRead.bearer_missing`; `bearer` in `http.rs` itself
/// calls such a variable one that "holds no text"): the read is refused before any request,
/// rather than sent with `authorization: Bearer ` and an empty token.
#[test]
fn adv_an_empty_bearer_variable_is_refused_before_any_request() {
    assert_eq!(
        std::env::var(EMPTY_VARIABLE).as_deref(),
        Ok(""),
        "precondition: cargo sets {EMPTY_VARIABLE} to the empty text"
    );
    let (listener, base) = listen();
    let server = answer_within(listener, Duration::from_secs(3), ok(b"{}"));
    let result = reader().read(
        &binding(&base, &["/api/v1/query"], Some(EMPTY_VARIABLE), 4096),
        "/api/v1/query",
        &[],
    );
    let request = server.join().expect("the listener");
    assert!(
        matches!(&result, Err(HttpReadError::Refused(why)) if why.starts_with("BearerMissing: ")),
        "an empty bearer variable is not refused: {result:?}; the endpoint received: {request:?}"
    );
    assert_eq!(request, None, "a request was made");
}

/// `HttpAnswer::body` is "at most `max_bytes`" of the body. A body cut at `max_bytes` in the middle
/// of a UTF-8 character is decoded lossily, and the replacement character makes the text the
/// model reads longer than the bound.
#[test]
fn adv_a_cut_answer_stays_within_max_bytes_on_a_character_boundary() {
    let (listener, base) = listen();
    // 200 bytes: 100 two-byte characters. The bound 101 falls inside the 51st.
    let body = "\u{e9}".repeat(100);
    let server = answer_within(listener, Duration::from_secs(3), ok(body.as_bytes()));
    let answer: HttpAnswer = reader()
        .read(
            &binding(&base, &["/api/v1/query"], None, 101),
            "/api/v1/query",
            &[],
        )
        .expect("the read answers");
    server.join().expect("the listener");
    assert!(answer.cut, "marked as cut");
    assert!(
        answer.body.len() <= 101,
        "the cut answer is {} bytes, past max_bytes 101: {:?}",
        answer.body.len(),
        answer.body.chars().rev().take(3).collect::<String>()
    );
}

/// The same bound for a body that is not UTF-8: every byte the lossy decoding replaces becomes
/// three, so a body exactly at `max_bytes`, read whole, reaches the model at three times it.
#[test]
fn adv_a_non_utf8_answer_stays_within_max_bytes() {
    let (listener, base) = listen();
    let server = answer_within(listener, Duration::from_secs(3), ok(&[0xFF; 100]));
    let answer = reader()
        .read(
            &binding(&base, &["/api/v1/query"], None, 100),
            "/api/v1/query",
            &[],
        )
        .expect("the read answers");
    server.join().expect("the listener");
    assert!(!answer.cut, "exactly at the bound is whole");
    assert!(
        answer.body.len() <= 100,
        "the answer is {} bytes, past max_bytes 100",
        answer.body.len()
    );
}

/// D2: a `..` segment is refused "even when listed". The URL parser treats `\` as `/` in an
/// `http` path, so `\..\` is a `..` segment the server resolves: the read leaves the listed
/// prefix and reaches `/secret`.
#[test]
fn adv_a_backslash_dot_dot_path_is_refused_even_when_listed() {
    let path = "/api\\..\\secret";
    let (listener, base) = listen();
    let server = answer_within(listener, Duration::from_secs(3), ok(b"{}"));
    let result = reader().read(&binding(&base, &[path], None, 4096), path, &[]);
    let request = server.join().expect("the listener");
    assert!(
        !path_allowed(path, &[path.to_owned()]),
        "path_allowed admits {path:?}; the read answered {result:?} and the endpoint received \
         {:?}",
        request.as_deref().and_then(|head| head.lines().next())
    );
}

/// The refusal of a redirect names the location, and reaches the model's transcript. The
/// location is the endpoint's text, so it is held to the binding's `max_bytes` as the body is.
#[test]
fn adv_a_redirect_location_is_held_to_max_bytes() {
    let location = format!("http://127.0.0.1:9/{}", "x".repeat(8 * 1024));
    let (listener, base) = listen();
    let server = answer_within(
        listener,
        Duration::from_secs(3),
        format!(
            "HTTP/1.1 302 Found\r\nlocation: {location}\r\ncontent-length: 0\r\nconnection: \
             close\r\n\r\n"
        )
        .into_bytes(),
    );
    let result = reader().read(
        &binding(&base, &["/api/v1/query"], None, 100),
        "/api/v1/query",
        &[],
    );
    server.join().expect("the listener");
    let Err(HttpReadError::Refused(why)) = result else {
        panic!("not a refusal: {result:?}");
    };
    assert!(why.starts_with("Redirected: "), "{why}");
    assert!(
        why.len() <= 100 + 200,
        "the refusal carries {} bytes of the endpoint's location past max_bytes 100",
        why.len()
    );
}

/// A listener that accepts and never answers is unavailable once the reader's timeout passes.
#[test]
fn adv_a_listener_that_never_answers_is_unavailable_within_the_timeout() {
    let (listener, base) = listen();
    let holder = thread::spawn(move || {
        let (socket, _) = listener.accept().expect("a connection");
        thread::sleep(Duration::from_secs(4));
        drop(socket);
    });
    let started = Instant::now();
    let result = HttpGet::with_timeout(Duration::from_secs(1)).read(
        &binding(&base, &["/api/v1/query"], None, 4096),
        "/api/v1/query",
        &[],
    );
    let took = started.elapsed();
    holder.join().unwrap();
    assert!(
        matches!(&result, Err(HttpReadError::Unavailable(_))),
        "{result:?}"
    );
    assert!(took < Duration::from_secs(3), "the read took {took:?}");
}

/// A chunked body with no content-length that never ends is cut at `max_bytes`: the reader stops
/// reading one byte past the bound.
#[test]
fn adv_an_endless_chunked_body_is_cut_at_max_bytes() {
    let (listener, base) = listen();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("a connection");
        let mut buffer = [0; 4096];
        let _ = socket.read(&mut buffer);
        let _ = socket.write_all(b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n");
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(4) {
            if socket.write_all(b"10\r\naaaaaaaaaaaaaaaa\r\n").is_err() {
                break;
            }
        }
    });
    let answer = reader()
        .read(
            &binding(&base, &["/api/v1/query"], None, 100),
            "/api/v1/query",
            &[],
        )
        .expect("the read answers");
    server.join().unwrap();
    assert_eq!(answer.body, "a".repeat(100));
    assert!(answer.cut);
}
