//! Adversary pass 1 on `story:a2a-record-and-streaming` (wave 2026-11-04-w1, unit U1).
//!
//! Two claims of the unit, driven against the listener on a loopback port this file binds:
//!
//! - the item of an A2A turn is `a2a:<revision>`, and the record replays a turn by that item, so
//!   two different `messageId`s must never give one item;
//! - `ess/domains/agent.yaml` (`AdmitA2aRequest`): "a stream whose peer takes nothing for that
//!   long is cut", and AGENTS.md: "a caller that takes nothing that long is cut off, freeing its
//!   admitted slot". The events are the size a real turn's are, not mebibytes.
//!
//! No model, no network beyond 127.0.0.1.

use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use b10x_loom_a2a::{A2aServer, Answerer, Bearer, WRITE_DEADLINE, item_revision};
use b10x_loom_plugin::{InboundItem, RecordLine, RecordOutcome};
use loom::agent::{A2aCard, A2aExposure};
use loom_sdk::EventSink;
use loom_sdk::commission::model::json::Value as CommissionJson;
use loom_sdk::commission::model::primitives::Uuid;
use loom_sdk::commission::model::responsibility::{
    ProposedActionArguments, RunId, RuntimeEvent, RuntimeEventActionSelected,
    RuntimeEventStepStarted,
};
use serde_json::{Value, json};

const TOKEN: &str = "a2a-adversary-bearer-u1w1104";
const PATH: &str = "/a2a";

fn exposure() -> A2aExposure {
    A2aExposure {
        port: 8080,
        path: PATH.to_owned(),
        card: A2aCard {
            name: "adversary".to_owned(),
            description: "Answers".to_owned(),
            skills: vec!["logs".to_owned()],
        },
    }
}

fn run_id() -> RunId {
    RunId(Uuid("00000000-0000-4000-8d00-0000000000a1".to_owned()))
}

fn proposed(item: &InboundItem) -> RecordLine {
    RecordLine {
        item: item.id.clone(),
        intent: None,
        confidence: None,
        outcome: RecordOutcome::Proposed,
        reads: Vec::new(),
        proposal: Some(format!("answer to {}", item.text)),
        proposed_case: None,
        detail: None,
        steps: Vec::new(),
    }
}

/// What a gated turn holds: where it says it started, and where it waits for the go.
type Gate = (Mutex<Sender<()>>, Mutex<Receiver<()>>);

/// A turn emitting two small runtime events, as a real step does; when gated, it waits for the
/// test's go between them.
struct Turn {
    items: Mutex<Vec<InboundItem>>,
    gate: Option<Gate>,
}

impl Turn {
    fn new() -> Self {
        Self {
            items: Mutex::new(Vec::new()),
            gate: None,
        }
    }

    fn items(&self) -> Vec<InboundItem> {
        self.items
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Answerer for Turn {
    fn answer(&self, item: &InboundItem, events: &mut dyn EventSink) -> Result<RecordLine, String> {
        self.items
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(item.clone());
        events.emit(&RuntimeEvent::StepStarted(RuntimeEventStepStarted {
            run_id: run_id(),
            step: 1,
            case_revision: 1,
            frontier_id: None,
        }));
        if let Some((started, go)) = &self.gate {
            let _ = started
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .send(());
            let _ = go
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .recv_timeout(Duration::from_secs(60));
        }
        events.emit(&RuntimeEvent::ActionSelected(RuntimeEventActionSelected {
            run_id: run_id(),
            action: "reply.propose".to_owned(),
            arguments: ProposedActionArguments(CommissionJson::Null),
        }));
        Ok(proposed(item))
    }
}

struct StopOnDrop<'a>(&'a AtomicBool);

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Lets a gated turn go on when dropped, so a failed assertion does not hold the listener.
struct GoOnDrop(Sender<()>);

impl Drop for GoOnDrop {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

fn with_listener<A: Answerer + Sync>(agent: &A, case: impl FnOnce(SocketAddr)) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let address = listener.local_addr().unwrap();
    let server = A2aServer::new(exposure(), Bearer::new(TOKEN).expect("a bearer"))
        .expect("the exposure is served");
    let stop = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let served = scope.spawn(|| server.serve(listener, agent, &stop));
        {
            let _stop = StopOnDrop(&stop);
            case(address);
        }
        served
            .join()
            .expect("the listener did not panic")
            .expect("the listener ended cleanly");
    });
}

fn post(address: SocketAddr, body: &Value) -> TcpStream {
    let mut stream = TcpStream::connect(address).expect("the listener accepts");
    let body = body.to_string();
    stream
        .write_all(
            format!(
                "POST {PATH} HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {TOKEN}\r\n\
                 Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .unwrap();
    stream
}

fn send(address: SocketAddr, method: &str, message_id: &str, text: &str) -> String {
    let mut stream = post(
        address,
        &json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": {"message": {
            "messageId": message_id, "role": "ROLE_USER", "parts": [{"text": text}]}}}),
    );
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("the answer is read");
    raw
}

/// A `messageId` that is no plain token is hashed to `m-` and 32 hex digits, and that hash is
/// itself a plain token, kept as it is: a second caller whose `messageId` is that spelling gets
/// the same item `a2a:<revision>`, so `Answering` replays the first caller's record to it and
/// runs no turn. Two different `messageId`s must give two items.
#[test]
fn adversary_two_different_message_ids_never_give_one_item() {
    let first = "what is the deploy status?";
    let second = item_revision(first);
    assert_ne!(first, second);
    let agent = Turn::new();
    with_listener(&agent, |address| {
        for (id, text) in [
            (first, "Is the deploy finished?"),
            (second.as_str(), "Who am I?"),
        ] {
            let raw = send(address, "SendMessage", id, text);
            assert!(raw.starts_with("HTTP/1.1 200"), "{raw}");
        }
    });
    let items = agent.items();
    assert_eq!(items.len(), 2, "{items:?}");
    assert_ne!(
        items[0].id, items[1].id,
        "messageId {first:?} and messageId {second:?} are two messages, given one item"
    );
}

/// The unit's contract (`AdmitA2aRequest` in `ess/domains/agent.yaml`, AGENTS.md § Agent spec):
/// a stream whose caller takes nothing for the answer's bound is cut and its admitted slot freed.
/// With events the size a turn's are, the caller here takes nothing for the bound and 4 s more,
/// and the stream is still open: nothing is cut, and the slot stays held until the turn ends.
#[test]
fn adversary_a_stream_whose_caller_takes_nothing_for_the_bound_is_cut() {
    let (started_tx, started) = channel();
    let (go, go_rx) = channel();
    let agent = Turn {
        items: Mutex::new(Vec::new()),
        gate: Some((Mutex::new(started_tx), Mutex::new(go_rx))),
    };
    with_listener(&agent, |address| {
        let _go = GoOnDrop(go);
        let asked = Instant::now();
        let mut idle = post(
            address,
            &json!({"jsonrpc": "2.0", "id": 1, "method": "SendStreamingMessage",
                "params": {"message": {"messageId": "idle-1", "role": "ROLE_USER",
                "parts": [{"text": "Is the deploy finished?"}]}}}),
        );
        started
            .recv_timeout(Duration::from_secs(30))
            .expect("the turn started");
        std::thread::sleep(WRITE_DEADLINE.saturating_sub(asked.elapsed()) + Duration::from_secs(4));
        idle.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut held = Vec::new();
        let mut chunk = [0_u8; 4096];
        let ended = loop {
            match idle.read(&mut chunk) {
                Ok(0) => break Ok(()),
                Ok(n) => held.extend_from_slice(&chunk[..n]),
                Err(error) => break Err(error),
            }
        };
        let held = String::from_utf8_lossy(&held);
        assert!(held.starts_with("HTTP/1.1 200 OK"), "{held}");
        assert!(
            !held.contains("TASK_STATE_COMPLETED"),
            "the turn is still gated: {held}"
        );
        assert!(
            ended.is_ok()
                || ended
                    .as_ref()
                    .is_err_and(|error| error.kind() == ErrorKind::ConnectionReset),
            "a caller that took nothing for {:?} (bound {WRITE_DEADLINE:?}) still holds an \
             open stream: {ended:?}, {} bytes buffered",
            asked.elapsed(),
            held.len()
        );
    });
}
