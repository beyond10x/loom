//! Acceptance for `story:laya-selector`: against a local stub that speaks the Laya interface, the
//! selector returns the stub's chosen candidate with the stub's probability as its confidence,
//! returns a selection error when the stub names an action outside the candidate set, and neither
//! `b10x-loom-cli` nor `b10x-loom-sdk` depends on `b10x-loom-selector-laya`.
//!
//! The stub binds a loopback port inside this process and answers `POST /v1/systemone` as the Laya
//! repository README at commit `1adc59f` describes it (see the crate's documentation), with a
//! scripted status and body, and records every request it receives. No test makes a network call.
//!
//! - `selector_returns_the_stubs_choice_with_its_probability_as_confidence`: the acceptance, read
//!   both off the selector and off Loom's `selection::select`, and the request the stub received.
//! - `a_choice_outside_the_candidate_set_is_a_selection_error`: the acceptance's refusal
//!   (`docs/examples/laya-fast-selection.md`: `release.rollback` at 0.99 is still refused).
//! - `every_transport_or_answer_failure_is_unavailable`: the story's `## Transport` list.
//! - `more_than_the_server_cap_is_refused_before_sending` and
//!   `no_candidates_is_nothing_admissible_and_sends_nothing`: the bounds checked before a request.
//! - `the_endpoint_is_configuration`: a base URL with a prefix and a trailing slash.
//! - `no_product_crate_depends_on_the_selector`: the acceptance's dependency rule, over the
//!   resolved graph `cargo metadata` reports.

use std::collections::{BTreeSet, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::iter::repeat_n;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use b10x_loom_executor::model::primitives::{Decimal, Uuid};
use b10x_loom_executor::model::run::{
    ActionCatalogue, ActionCatalogueData, CatalogueEntry, CatalogueEntryStatus, CatalogueId,
    SelectionId, SelectionStrategy, TurnId, action_catalogue_state,
};
use b10x_loom_executor::selection::{Choice, SelectionContext, SelectionRefusal, select};
use b10x_loom_executor::{ActionSelector, SelectorError};
use b10x_loom_selector_laya::{DEFAULT_INSTRUCTIONS, LayaSelector, MAX_CANDIDATES, QUESTION};
use serde_json::{Value, json};

const PROMPT: &str = "Identify the likely source of the latency spike after the last deploy.";

/// The candidate set, as `docs/examples/laya-fast-selection.md` has it, plus one entry that needs
/// approval: the selector is handed the whole catalogue and offers Laya all of it.
const CANDIDATES: [(&str, CatalogueEntryStatus); 4] = [
    ("metrics.inspect", CatalogueEntryStatus::Admissible),
    ("logs.search", CatalogueEntryStatus::Admissible),
    ("release.inspect", CatalogueEntryStatus::Admissible),
    ("release.pause", CatalogueEntryStatus::ApprovalRequired),
];

/// The candidate the stub chooses, and the probability it gives.
const CHOSEN: &str = "release.inspect";
const PROBABILITY: &str = "0.96";

/// An action the catalogue does not list.
const OUTSIDE: &str = "release.rollback";

/// The timeout the timeout case runs with, and how long its stub holds the answer.
const SHORT_TIMEOUT: Duration = Duration::from_millis(200);
const STALL: Duration = Duration::from_millis(1500);

// ---------------------------------------------------------------------------------------------
// The stub Laya server.

/// One request the stub received.
#[derive(Debug, Clone)]
struct Received {
    method: String,
    path: String,
    content_type: Option<String>,
    authorization: Option<String>,
    body: Vec<u8>,
}

impl Received {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).expect("the selection request is JSON")
    }
}

/// One scripted answer: a status, a body, and how long to hold it.
#[derive(Debug, Clone)]
struct Scripted {
    status: u16,
    body: String,
    stall: Duration,
}

fn answer(status: u16, body: impl Into<String>) -> Scripted {
    Scripted {
        status,
        body: body.into(),
        stall: Duration::ZERO,
    }
}

/// A Laya answer choosing `choice` with `answer_confidence`, in the shape the README gives, with
/// the fields the selector ignores present.
fn laya_answer(choice: &str, answer_confidence: &str) -> String {
    format!(
        r#"{{"answers":{{"{QUESTION}":{{"choice":"{choice}","confidence":0.41,"answer_confidence":{answer_confidence},"probabilities":{{"{choice}":{answer_confidence}}}}}}},"usage":{{"input_tokens":57,"output_tokens":1}},"routing":{{"model":"english"}}}}"#
    )
}

/// A Laya server on a loopback port of this process, answering requests with its script in order;
/// once the script is spent, every request is a 500.
struct StubLaya {
    addr: SocketAddr,
    received: Arc<Mutex<Vec<Received>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl StubLaya {
    fn start(script: Vec<Scripted>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let addr = listener.local_addr().expect("the bound address");
        let received = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let received = Arc::clone(&received);
            let stop = Arc::clone(&stop);
            let mut script = VecDeque::from(script);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    serve(stream, &received, &mut script);
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
        format!("http://{}", self.addr)
    }

    fn received(&self) -> Vec<Received> {
        self.received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for StubLaya {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop so it sees the flag.
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Reads one HTTP/1.1 request from `stream`, records it, answers it with the next scripted answer
/// for `POST /v1/systemone` (404 for anything else), then closes.
fn serve(stream: TcpStream, received: &Mutex<Vec<Received>>, script: &mut VecDeque<Scripted>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();
    let mut length = 0usize;
    let mut content_type = None;
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
                "content-type" => content_type = Some(value.to_owned()),
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
        content_type,
        authorization,
        body,
    };
    received
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(request.clone());
    let scripted = if request.method == "POST" && request.path.ends_with("/v1/systemone") {
        script
            .pop_front()
            .unwrap_or_else(|| answer(500, r#"{"detail":"script spent"}"#))
    } else {
        answer(404, r#"{"detail":"Not Found"}"#)
    };
    std::thread::sleep(scripted.stall);
    let mut stream = reader.into_inner();
    let _ = write!(
        stream,
        "HTTP/1.1 {} Scripted\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
         connection: close\r\n\r\n{}",
        scripted.status,
        scripted.body.len(),
        scripted.body
    );
    let _ = stream.flush();
}

// ---------------------------------------------------------------------------------------------
// Fixtures.

fn uuid(n: u32) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn entries() -> Vec<CatalogueEntry> {
    CANDIDATES
        .iter()
        .map(|(action, status)| CatalogueEntry {
            action: (*action).to_owned(),
            status: *status,
        })
        .collect()
}

fn catalogue() -> ActionCatalogue<action_catalogue_state::Projected> {
    ActionCatalogue::new(ActionCatalogueData {
        catalogue_id: CatalogueId(uuid(1)),
        turn_id: TurnId(uuid(2)),
        frontier: "frontier-3".to_owned(),
        case_revision: 3,
        entries: entries(),
    })
}

fn context() -> SelectionContext {
    SelectionContext {
        prompt: PROMPT.to_owned(),
    }
}

fn selector(stub: &StubLaya) -> LayaSelector {
    LayaSelector::new(&stub.url()).expect("a loopback endpoint is accepted")
}

/// The reason of an `Unavailable`, or a panic naming `case` for anything else.
fn unavailable(case: &str, result: Result<Choice, SelectorError>) -> String {
    match result {
        Err(SelectorError::Unavailable(reason)) => reason,
        other => panic!("{case}: expected Unavailable, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// The acceptance.

#[test]
fn selector_returns_the_stubs_choice_with_its_probability_as_confidence() {
    let stub = StubLaya::start(vec![
        answer(200, laya_answer(CHOSEN, PROBABILITY)),
        answer(200, laya_answer(CHOSEN, PROBABILITY)),
    ]);
    let selector = selector(&stub);
    assert_eq!(selector.strategy(), SelectionStrategy::FastTyped);

    let choice = selector
        .select(&context(), &entries())
        .expect("the stub's choice is a candidate");
    assert_eq!(choice.action, CHOSEN);
    assert_eq!(
        choice.confidence,
        Some(Decimal(PROBABILITY.to_owned())),
        "the confidence is the stub's answer_confidence, not its confidence"
    );

    let selection = select(&selector, &context(), &catalogue(), SelectionId(uuid(4)))
        .expect("Loom accepts the stub's choice");
    let data = selection.data();
    assert_eq!(data.action, CHOSEN);
    assert_eq!(data.confidence, Some(Decimal(PROBABILITY.to_owned())));
    assert_eq!(data.strategy, SelectionStrategy::FastTyped);
    assert_eq!(data.case_revision, 3);

    let received = stub.received();
    assert_eq!(received.len(), 2, "one request per selection: {received:?}");
    for request in &received {
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/systemone");
        assert_eq!(request.content_type.as_deref(), Some("application/json"));
        assert_eq!(
            request.authorization, None,
            "this story sends no credential"
        );
        let body = request.json();
        assert_eq!(body["state"], json!({"goal": PROMPT}));
        let question = &body["questions"][QUESTION];
        assert_eq!(question["type"], "choice");
        assert_eq!(question["instructions"], DEFAULT_INSTRUCTIONS);
        let offered: BTreeSet<&str> = question["criteria"]
            .as_object()
            .expect("the criteria are an object")
            .keys()
            .map(String::as_str)
            .collect();
        let candidates: BTreeSet<&str> = CANDIDATES.iter().map(|(action, _)| *action).collect();
        assert_eq!(
            offered, candidates,
            "Laya is offered exactly the candidate set"
        );
        assert_eq!(
            body["questions"].as_object().map(serde_json::Map::len),
            Some(1),
            "one question"
        );
    }
}

#[test]
fn a_choice_outside_the_candidate_set_is_a_selection_error() {
    let stub = StubLaya::start(vec![
        answer(200, laya_answer(OUTSIDE, "0.99")),
        answer(200, laya_answer(OUTSIDE, "0.99")),
    ]);
    let selector = selector(&stub);

    let reason = unavailable("outside", selector.select(&context(), &entries()));
    assert!(
        reason.contains(OUTSIDE),
        "the refusal names the action: {reason}"
    );

    match select(&selector, &context(), &catalogue(), SelectionId(uuid(5))) {
        Err(SelectionRefusal::Selector(SelectorError::Unavailable(_))) => {}
        Err(other) => panic!("expected the selector's Unavailable, got {other:?}"),
        Ok(selection) => panic!(
            "Loom made a selection of an action outside the candidates: {:?}",
            selection.data()
        ),
    }
    assert_eq!(stub.received().len(), 2);
}

// ---------------------------------------------------------------------------------------------
// Failures.

#[test]
fn every_transport_or_answer_failure_is_unavailable() {
    let nested = |depth: usize| {
        format!(
            r#"{{"answers":{{"{QUESTION}":{{"choice":"{CHOSEN}","answer_confidence":0.9}}}},"extra":{}{}}}"#,
            "[".repeat(depth),
            "]".repeat(depth)
        )
    };
    let cases: Vec<(&str, Scripted, &str)> = vec![
        (
            "malformed request",
            answer(422, r#"{"detail":"bad"}"#),
            "422",
        ),
        (
            "too many options",
            answer(413, r#"{"detail":"too many"}"#),
            "413",
        ),
        ("server error", answer(500, r#"{"detail":"boom"}"#), "500"),
        ("not JSON", answer(200, "this is not json"), "not JSON"),
        ("no answers", answer(200, r#"{"usage":{}}"#), "answers"),
        (
            "no choice",
            answer(
                200,
                format!(r#"{{"answers":{{"{QUESTION}":{{"answer_confidence":0.9}}}}}}"#),
            ),
            "choice",
        ),
        (
            "choice not a string",
            answer(
                200,
                format!(r#"{{"answers":{{"{QUESTION}":{{"choice":7,"answer_confidence":0.9}}}}}}"#),
            ),
            "choice",
        ),
        (
            "no answer_confidence",
            answer(
                200,
                format!(
                    r#"{{"answers":{{"{QUESTION}":{{"choice":"{CHOSEN}","confidence":0.9}}}}}}"#
                ),
            ),
            "answer_confidence",
        ),
        (
            "answer_confidence a string",
            answer(200, laya_answer(CHOSEN, r#""0.9""#)),
            "answer_confidence",
        ),
        (
            "probability above one",
            answer(200, laya_answer(CHOSEN, "1.2")),
            "[0, 1]",
        ),
        (
            "probability below zero",
            answer(200, laya_answer(CHOSEN, "-0.1")),
            "[0, 1]",
        ),
        ("nested 129 levels", answer(200, nested(129)), "not JSON"),
        ("nested 1000 levels", answer(200, nested(1000)), "not JSON"),
    ];
    let (names, script): (Vec<_>, Vec<_>) = cases
        .iter()
        .map(|(name, scripted, _)| (*name, scripted.clone()))
        .unzip();
    let stub = StubLaya::start(script);
    let selector = selector(&stub);
    for (name, _, needle) in &cases {
        let reason = unavailable(name, selector.select(&context(), &entries()));
        assert!(
            reason.contains(needle),
            "{name}: the reason does not say `{needle}`: {reason}"
        );
    }
    assert_eq!(
        stub.received().len(),
        names.len(),
        "each case is sent exactly once, never retried"
    );

    // A connection nobody accepts.
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        listener.local_addr().expect("the bound address")
    };
    let refused = LayaSelector::new(&format!("http://{closed}")).expect("a loopback endpoint");
    unavailable("connection refused", refused.select(&context(), &entries()));

    // An answer slower than the timeout.
    let slow = StubLaya::start(vec![Scripted {
        stall: STALL,
        ..answer(200, laya_answer(CHOSEN, PROBABILITY))
    }]);
    let impatient =
        LayaSelector::with_timeout(&slow.url(), SHORT_TIMEOUT).expect("a loopback endpoint");
    let reason = unavailable("timeout", impatient.select(&context(), &entries()));
    assert!(reason.contains("deadline"), "timeout: {reason}");
}

#[test]
fn more_than_the_server_cap_is_refused_before_sending() {
    let stub = StubLaya::start(vec![answer(200, laya_answer("action.0", "0.5"))]);
    let selector = selector(&stub);
    let many: Vec<CatalogueEntry> = (0..=MAX_CANDIDATES)
        .map(|n| CatalogueEntry {
            action: format!("action.{n}"),
            status: CatalogueEntryStatus::Admissible,
        })
        .collect();
    let reason = unavailable("over the cap", selector.select(&context(), &many));
    assert!(reason.contains(&MAX_CANDIDATES.to_string()), "{reason}");
    assert!(stub.received().is_empty(), "nothing is sent over the cap");

    // Exactly the cap is sent; a repeated id counts once.
    let mut at_cap = many;
    at_cap.truncate(MAX_CANDIDATES);
    at_cap.extend(repeat_n(at_cap[0].clone(), 3));
    let choice = selector
        .select(&context(), &at_cap)
        .expect("the cap itself is answered");
    assert_eq!(choice.action, "action.0");
    assert_eq!(stub.received().len(), 1);
}

#[test]
fn no_candidates_is_nothing_admissible_and_sends_nothing() {
    let stub = StubLaya::start(vec![]);
    let selector = selector(&stub);
    assert_eq!(
        selector.select(&context(), &[]),
        Err(SelectorError::NothingAdmissible)
    );
    assert!(stub.received().is_empty());
}

#[test]
fn the_endpoint_is_configuration() {
    let stub = StubLaya::start(vec![answer(200, laya_answer(CHOSEN, "1"))]);
    let prefixed =
        LayaSelector::new(&format!("{}/laya/", stub.url())).expect("a prefixed endpoint");
    assert_eq!(prefixed.url(), format!("{}/laya/v1/systemone", stub.url()));
    let choice = prefixed
        .select(&context(), &entries())
        .expect("the prefixed endpoint answers");
    assert_eq!(choice.confidence, Some(Decimal("1".to_owned())));
    assert_eq!(stub.received()[0].path, "/laya/v1/systemone");

    for refused in ["", "127.0.0.1:8000", "ftp://host/"] {
        assert!(
            LayaSelector::new(refused).is_err(),
            "`{refused}` is not an endpoint"
        );
    }
    assert!(LayaSelector::with_timeout(&stub.url(), Duration::ZERO).is_err());
}

// ---------------------------------------------------------------------------------------------
// The dependency rule.

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo"),
    );
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("the crate sits two levels below the repository root")
        .to_path_buf()
}

/// Every package id `from` reaches through the resolved graph, over every dependency kind.
fn reachable(metadata: &Value, from: &str) -> BTreeSet<String> {
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .expect("cargo metadata has a resolve graph");
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([from.to_owned()]);
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let node = nodes
            .iter()
            .find(|node| node["id"] == id.as_str())
            .unwrap_or_else(|| panic!("{id} is not in the resolve graph"));
        for dep in node["deps"].as_array().into_iter().flatten() {
            if let Some(pkg) = dep["pkg"].as_str() {
                queue.push_back(pkg.to_owned());
            }
        }
    }
    seen
}

#[test]
fn no_product_crate_depends_on_the_selector() {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--locked", "--offline"])
        .arg("--manifest-path")
        .arg(repo_root().join("Cargo.toml"))
        .output()
        .expect("spawn cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed ({}):\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("cargo metadata is JSON");
    let id_of = |name: &str| -> String {
        metadata["packages"]
            .as_array()
            .expect("cargo metadata has packages")
            .iter()
            .find(|package| package["name"] == name)
            .and_then(|package| package["id"].as_str())
            .unwrap_or_else(|| panic!("no package {name}"))
            .to_owned()
    };
    let selector = id_of("b10x-loom-selector-laya");
    for product in ["b10x-loom-cli", "b10x-loom-sdk"] {
        let reached = reachable(&metadata, &id_of(product));
        assert!(
            reached.len() > 1,
            "{product} reaches nothing: the graph walk is not running"
        );
        assert!(
            !reached.contains(&selector),
            "{product} depends on b10x-loom-selector-laya"
        );
    }
}
