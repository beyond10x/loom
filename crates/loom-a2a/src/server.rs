//! The listener: routes, the JSON-RPC methods and the one thread that runs turns.
//!
//! Each connection is read on a thread of its own. Until its request is admitted it holds one of
//! [`MAX_CONNECTIONS`] unadmitted slots; a connection arriving when they are all held takes the
//! slot of the oldest, which is cut off, so peers that send nothing, or nothing whole, cannot keep
//! a probe from being read. An admitted request holds one of [`MAX_CONNECTIONS`] admitted slots
//! until it is answered; past them it is answered `503`. A probe or a refusal takes no admitted
//! slot. Every request is admitted first ([`crate::admit::Presented`]), on its
//! request line and headers: a probe is answered whatever it carries, every other request needs
//! the bearer and is refused `401` before its body is read. Once the body names its method, a
//! `SendStreamingMessage` is decided `streamed`, any other method `admitted`. Turns run one at a
//! time on the thread that called [`A2aServer::serve`], which is the only one that touches the
//! [`Answerer`], so an answerer need not be shared between threads; a connection hands its message
//! over and waits for the turn's end, and a streamed one writes each status change of the task as
//! the turn makes it. That thread ends the task, so a stream cut off mid-turn leaves the turn
//! running and its task readable by `GetTask`. A probe, the card and a task's status never wait
//! for a turn.

use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use a2a::discovery::{
    AgentCapabilities, AgentCard, AgentInterface, AgentSkill, HTTPAuthSecurityScheme,
    SecurityRequirement, SecurityScheme, StringList,
};
use a2a::tasks::{
    ContextId, Message, Role, TaskArtifactUpdateEvent, TaskSnapshot, TaskStatus,
    TaskStatusUpdateEvent,
};
use b10x_loom_plugin::{InboundItem, ItemId, RecordLine};
use loom::agent::{A2aExposure, A2aRefused};
use loom_sdk::EventSink;
use loom_sdk::commission::model::responsibility::RuntimeEvent;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::admit::{AdmitA2aRequestOutcome, Bearer, Presented, a2a_refusal_spelling};
use crate::http::{Connection, Head, Response};
use crate::tasks::{TaskEvents, Tasks};
use crate::wire;

/// Where the agent card is published (A2A 1.0.1 § 8.2).
pub const CARD_PATH: &str = "/.well-known/agent-card.json";

/// The readiness probe.
pub const READY_PATH: &str = "/ready";

/// The liveness probe.
pub const LIVE_PATH: &str = "/live";

/// The most connections read at once before their request is admitted, and the most admitted
/// requests answered at once (see the module documentation).
pub const MAX_CONNECTIONS: usize = 32;

/// How long a stream may carry no event before it is cut: the answer's bound. A caller that
/// stopped reading cannot be told from one that reads while the events fit the connection's
/// buffers, so a stream idle that long is cut either way, freeing its admitted slot.
pub const STREAM_IDLE: Duration = crate::http::WRITE_DEADLINE;

/// The A2A protocol version the card names.
pub const PROTOCOL_VERSION: &str = "1.0";

/// The protocol binding the card names: JSON-RPC 2.0 over HTTP, at `expose.a2a.path`.
pub const PROTOCOL_BINDING: &str = "JSONRPC";

/// The name the card gives the bearer scheme.
pub const BEARER_SCHEME: &str = "bearer";

/// The JSON-RPC error of an unknown task (A2A 1.0.1 `TaskNotFoundError`).
pub const TASK_NOT_FOUND: i64 = -32001;

/// The agent behind the listener: one turn per message.
pub trait Answerer {
    /// Runs one turn on `item`, handing each runtime event of its run to `events` as it happens,
    /// and answers the turn's record line.
    ///
    /// # Errors
    /// The turn could not run (no model, a service that is down, a store that failed): the task
    /// fails naming it.
    fn answer(&self, item: &InboundItem, events: &mut dyn EventSink) -> Result<RecordLine, String>;
}

/// The A2A listener of one agent: its exposure, the bearer every request but a probe carries, and
/// its tasks.
pub struct A2aServer {
    exposure: A2aExposure,
    bearer: Bearer,
    tasks: Tasks,
}

/// One message handed to the thread that runs turns: its task, its item, whether its connection
/// streams, and where the task's changes go.
struct Job {
    task: String,
    item: InboundItem,
    stream: bool,
    updates: Sender<Update>,
}

/// A change of a task, as the thread running its turn hands it to the task's connection.
enum Update {
    /// The task's status after a runtime event of its turn; sent only to a streaming connection.
    Status(TaskStatus),
    /// The task as its turn ended it, `None` when it is no longer held.
    Ended(Option<TaskSnapshot>),
}

/// What a connection answers to its JSON-RPC request.
enum Answer {
    /// One JSON answer.
    Whole(Response),
    /// The events of a streamed task (see [`A2aServer::stream`]).
    Stream(Box<Streamed>),
}

/// A streamed task: the request's id, the task as it was submitted, and its changes.
struct Streamed {
    id: Value,
    task: TaskSnapshot,
    updates: Receiver<Update>,
}

/// The sink of a turn: each event moves the task ([`TaskEvents`]) and, when the task's connection
/// streams, hands the task's new status to it.
struct Following<'a> {
    events: TaskEvents<'a>,
    tasks: &'a Tasks,
    task: &'a str,
    updates: Option<&'a Sender<Update>>,
}

impl EventSink for Following<'_> {
    fn emit(&mut self, event: &RuntimeEvent) {
        self.events.emit(event);
        if let Some(updates) = self.updates
            && let Some(task) = self.tasks.get(self.task)
        {
            // A stream cut off has dropped its end: the turn goes on without it.
            let _ = updates.send(Update::Status(task.status));
        }
    }
}

/// The connection slots of one listener (see the module documentation).
#[derive(Default)]
struct Slots {
    /// The connections not admitted yet, oldest first, each with a handle that cuts it off.
    unadmitted: Mutex<Pending>,
    /// The admitted requests not answered yet.
    admitted: AtomicUsize,
}

/// The unadmitted connections and the id the next one gets.
#[derive(Default)]
struct Pending {
    next: u64,
    open: VecDeque<(u64, TcpStream)>,
}

impl Slots {
    fn open(&self) -> MutexGuard<'_, Pending> {
        self.unadmitted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Takes an unadmitted slot for the connection `handle` refers to, cutting off the oldest
    /// unadmitted connection when every slot is held; answers its id.
    fn unadmitted(&self, handle: TcpStream) -> u64 {
        let mut open = self.open();
        while open.open.len() >= MAX_CONNECTIONS {
            if let Some((_, oldest)) = open.open.pop_front() {
                let _ = oldest.shutdown(Shutdown::Both);
            }
        }
        let id = open.next;
        open.next += 1;
        open.open.push_back((id, handle));
        id
    }

    /// Takes an admitted slot, or `None` when every one is held.
    fn admitted(&self) -> Option<Admitted<'_>> {
        if self.admitted.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            self.admitted.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(Admitted(&self.admitted))
    }
}

/// An unadmitted slot, given back when dropped (unless the connection was cut off already).
struct Unadmitted<'a> {
    slots: &'a Slots,
    id: u64,
}

impl Drop for Unadmitted<'_> {
    fn drop(&mut self) {
        self.slots.open().open.retain(|(id, _)| *id != self.id);
    }
}

/// An admitted slot, given back when dropped.
struct Admitted<'a>(&'a AtomicUsize);

impl Drop for Admitted<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl A2aServer {
    /// The listener of `exposure`, admitting requests that carry `bearer`.
    ///
    /// # Errors
    /// `expose.a2a.path` is not an absolute path, or is one of the paths the listener answers
    /// itself (the card and the probes).
    pub fn new(exposure: A2aExposure, bearer: Bearer) -> Result<Self, String> {
        let path = exposure.path.as_str();
        if !path.starts_with('/') || path.contains(['?', '#', ' ']) {
            return Err(format!(
                "`expose.a2a.path` `{path}` is no absolute path without a query"
            ));
        }
        if [CARD_PATH, READY_PATH, LIVE_PATH].contains(&path) {
            return Err(format!(
                "`expose.a2a.path` `{path}` is a path the listener answers itself"
            ));
        }
        Ok(Self {
            exposure,
            bearer,
            tasks: Tasks::default(),
        })
    }

    /// The agent card, its interface at `base` (`http://<host>`) and `expose.a2a.path`: the
    /// spec's name, description and skills, the JSON-RPC binding, and the bearer scheme every
    /// request needs.
    pub fn card(&self, base: &str) -> AgentCard {
        let card = &self.exposure.card;
        AgentCard {
            name: card.name.clone(),
            description: card.description.clone(),
            supported_interfaces: vec![AgentInterface {
                url: format!("{base}{}", self.exposure.path),
                protocol_binding: PROTOCOL_BINDING.to_owned(),
                tenant: None,
                protocol_version: PROTOCOL_VERSION.to_owned(),
            }],
            provider: None,
            version: env!("CARGO_PKG_VERSION").to_owned(),
            documentation_url: None,
            capabilities: AgentCapabilities {
                // Every listener answers `SendStreamingMessage` ([`A2aServer::stream`]).
                streaming: Some(true),
                push_notifications: Some(false),
                extensions: None,
                extended_agent_card: Some(false),
            },
            security_schemes: Some(BTreeMap::from([(
                BEARER_SCHEME.to_owned(),
                SecurityScheme {
                    api_key_security_scheme: None,
                    http_auth_security_scheme: Some(HTTPAuthSecurityScheme {
                        description: Some("the bearer the agent's Secret gives it".to_owned()),
                        scheme: "Bearer".to_owned(),
                        bearer_format: None,
                    }),
                    oauth2_security_scheme: None,
                    open_id_connect_security_scheme: None,
                    mtls_security_scheme: None,
                },
            )])),
            security_requirements: Some(vec![SecurityRequirement {
                schemes: BTreeMap::from([(
                    BEARER_SCHEME.to_owned(),
                    StringList { list: Vec::new() },
                )]),
            }]),
            default_input_modes: vec!["text/plain".to_owned()],
            default_output_modes: vec!["text/plain".to_owned(), "application/json".to_owned()],
            skills: card
                .skills
                .iter()
                .map(|skill| AgentSkill {
                    id: skill.clone(),
                    name: skill.clone(),
                    description: format!("{}: {skill}", card.description),
                    tags: vec![skill.clone()],
                    examples: None,
                    input_modes: None,
                    output_modes: None,
                    security_requirements: None,
                })
                .collect(),
            signatures: None,
            icon_url: None,
        }
    }

    /// The task `id`, when this listener holds it.
    pub fn task(&self, id: &str) -> Option<TaskSnapshot> {
        self.tasks.get(id)
    }

    /// Answers on `listener` until `stop` is set, running every turn on `agent` on this thread
    /// (see the module documentation). Once `stop` is set no connection is accepted; the
    /// connections already read are answered, their turns included, before this returns.
    ///
    /// # Errors
    /// The listener cannot be polled, or accepting failed for a reason other than one connection.
    pub fn serve<A: Answerer + ?Sized>(
        &self,
        listener: TcpListener,
        agent: &A,
        stop: &AtomicBool,
    ) -> io::Result<()> {
        listener.set_nonblocking(true)?;
        let (jobs, queue) = mpsc::channel::<Job>();
        let slots = Slots::default();
        let slots = &slots;
        std::thread::scope(|scope| {
            let acceptor = scope.spawn(move || -> io::Result<()> {
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let Ok(handle) = stream.try_clone() else {
                                continue;
                            };
                            let Ok(connection) = Connection::new(stream) else {
                                continue;
                            };
                            let id = slots.unadmitted(handle);
                            let jobs = jobs.clone();
                            scope.spawn(move || {
                                self.connection(connection, id, slots, &jobs, stop);
                            });
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(20));
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                io::ErrorKind::ConnectionAborted
                                    | io::ErrorKind::ConnectionReset
                                    | io::ErrorKind::Interrupted
                            ) => {}
                        Err(error) => return Err(error),
                    }
                }
                Ok(())
            });
            // Ends once the acceptor and every connection dropped their sender.
            for job in queue {
                self.run(agent, job);
            }
            acceptor
                .join()
                .unwrap_or_else(|_| Err(io::Error::other("the acceptor panicked")))
        })
    }

    /// Runs one turn, ends its task with the turn's record and hands the ended task back. A turn
    /// that could not run, or panicked, fails its task.
    fn run<A: Answerer + ?Sized>(&self, agent: &A, job: Job) {
        let mut events = Following {
            events: TaskEvents::new(&self.tasks, &job.task),
            tasks: &self.tasks,
            task: &job.task,
            updates: job.stream.then_some(&job.updates),
        };
        let result = catch_unwind(AssertUnwindSafe(|| agent.answer(&job.item, &mut events)))
            .unwrap_or_else(|_| Err("the turn panicked".to_owned()));
        let ended = match result {
            Ok(record) => self.tasks.end(&job.task, &record),
            Err(why) => self
                .tasks
                .fail(&job.task, &format!("the turn could not run: {why}")),
        };
        let _ = job.updates.send(Update::Ended(ended));
    }

    /// Reads, admits and answers one request, `id` its unadmitted slot.
    fn connection(
        &self,
        mut connection: Connection,
        id: u64,
        slots: &Slots,
        jobs: &Sender<Job>,
        stop: &AtomicBool,
    ) {
        // Held until the request is admitted or answered, whichever comes first.
        let unadmitted = Unadmitted { slots, id };
        let head = match connection.head() {
            Ok(head) => head,
            Err(response) => return connection.answer(&response),
        };
        let probe = head.method == "GET" && (head.path == READY_PATH || head.path == LIVE_PATH);
        let presented = Presented::new(&self.bearer, probe, head.header("authorization"));
        match presented.decide(false) {
            AdmitA2aRequestOutcome::MissingBearer { error }
            | AdmitA2aRequestOutcome::WrongBearer { error } => {
                return connection.answer(&refused(&error));
            }
            AdmitA2aRequestOutcome::Probe => {
                return connection.answer(&if head.path == LIVE_PATH {
                    Response::json(200, &json!({"live": true}))
                } else if stop.load(Ordering::SeqCst) {
                    Response::json(503, &json!({"ready": false}))
                } else {
                    Response::json(200, &json!({"ready": true}))
                });
            }
            AdmitA2aRequestOutcome::Admitted | AdmitA2aRequestOutcome::Streamed => {}
        }
        let Some(_admitted) = slots.admitted() else {
            return connection.answer(&Response::error(503, "too many requests at once"));
        };
        drop(unadmitted);
        let response = if head.path == CARD_PATH {
            if head.method == "GET" {
                Response::json(200, &wire::card(&self.card(&base(&head))))
            } else {
                not_allowed("GET")
            }
        } else if head.path == self.exposure.path {
            if head.method == "POST" {
                match connection.body(&head) {
                    Ok(body) => match self.rpc(&body, presented, jobs) {
                        Answer::Whole(response) => response,
                        Answer::Stream(streamed) => return self.stream(connection, *streamed),
                    },
                    Err(response) => response,
                }
            } else {
                not_allowed("POST")
            }
        } else {
            Response::error(404, "no such path")
        };
        connection.answer(&response);
    }

    /// One JSON-RPC 2.0 request, `presented` what its head presented.
    fn rpc(&self, body: &[u8], presented: Presented, jobs: &Sender<Job>) -> Answer {
        let whole = Answer::Whole;
        let Ok(request) = serde_json::from_slice::<Value>(body) else {
            return whole(rpc_error(&Value::Null, -32700, "the body is no JSON"));
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        if !matches!(id, Value::Null | Value::String(_) | Value::Number(_)) {
            return whole(rpc_error(
                &Value::Null,
                -32600,
                "`id` is no string or number",
            ));
        }
        if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return whole(rpc_error(
                &id,
                -32600,
                "the request is no JSON-RPC 2.0 request",
            ));
        }
        let Some(method) = request.get("method").and_then(Value::as_str) else {
            return whole(rpc_error(&id, -32600, "the request names no method"));
        };
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        let stream = matches!(method, "SendStreamingMessage" | "message/stream");
        match presented.decide(stream) {
            AdmitA2aRequestOutcome::MissingBearer { error }
            | AdmitA2aRequestOutcome::WrongBearer { error } => {
                return whole(refused(&error));
            }
            AdmitA2aRequestOutcome::Probe => {
                return whole(rpc_error(&id, -32600, "a probe names no method"));
            }
            AdmitA2aRequestOutcome::Streamed => return self.send(&id, &params, jobs, true),
            AdmitA2aRequestOutcome::Admitted => {}
        }
        match method {
            "SendMessage" | "message/send" => self.send(&id, &params, jobs, false),
            "GetTask" | "tasks/get" => whole(self.get_task(&id, &params)),
            _ => whole(rpc_error(
                &id,
                -32601,
                "the method is not served: SendMessage (message/send), SendStreamingMessage \
                 (message/stream) and GetTask (tasks/get) are",
            )),
        }
    }

    /// `SendMessage`: one turn on the message's text, answered with the task it ended; with
    /// `stream`, `SendStreamingMessage`: the same turn, its task streamed ([`Self::stream`]).
    fn send(&self, id: &Value, params: &Value, jobs: &Sender<Job>, stream: bool) -> Answer {
        let whole = Answer::Whole;
        let Some(message) = params.get("message").filter(|message| message.is_object()) else {
            return whole(rpc_error(id, -32602, "`params.message` is required"));
        };
        if message
            .get("taskId")
            .is_some_and(|task| !task.is_null() && task != "")
        {
            return whole(rpc_error(
                id,
                -32602,
                "each message is one turn: a message continuing a task is not answered",
            ));
        }
        let Some(text) = message
            .get("parts")
            .and_then(Value::as_array)
            .and_then(|parts| wire::request_text(parts))
        else {
            return whole(rpc_error(id, -32602, "the message carries no text part"));
        };
        // The record keys the turn by it, so a message without one could not be told from another.
        let Some(message_id) = message
            .get("messageId")
            .and_then(Value::as_str)
            .filter(|given| !given.is_empty())
            .map(str::to_owned)
        else {
            return whole(rpc_error(
                id,
                -32602,
                "`params.message.messageId` is required",
            ));
        };
        let request = Message {
            message_id: message_id.clone(),
            context_id: message
                .get("contextId")
                .and_then(Value::as_str)
                .filter(|given| !given.is_empty())
                .map(|given| ContextId(given.to_owned())),
            task_id: None,
            role: Role::ROLEUSER,
            parts: vec![wire::text_part(&text)],
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        };
        let task = self.tasks.submit(request);
        let task_id = task.id.0.clone();
        let details = json!({
            "source": "a2a",
            "task": task_id,
            "context": task.context_id.as_ref().map(|context| context.0.clone()),
            "message": message_id,
        });
        let revision = item_revision(&message_id);
        let item = InboundItem {
            id: ItemId(format!("a2a:{revision}")),
            revision,
            text,
            details: loom::json::parse(&details.to_string()).unwrap_or(loom::json::Value::Null),
        };
        let (updates, received) = mpsc::channel();
        if let Err(mpsc::SendError(job)) = jobs.send(Job {
            task: task_id.clone(),
            item,
            stream,
            updates,
        }) {
            let failed = self.tasks.fail(&task_id, "the agent is stopping");
            let _ = job.updates.send(Update::Ended(failed));
        }
        if stream {
            return Answer::Stream(Box::new(Streamed {
                id: id.clone(),
                task,
                updates: received,
            }));
        }
        match self.ended(&task_id, &received) {
            Some(task) => whole(rpc_result(id, &json!({"task": wire::task(&task)}))),
            None => whole(rpc_error(id, TASK_NOT_FOUND, "the task is no longer held")),
        }
    }

    /// The task `task` once its turn ended: the end `updates` brings, every status before it
    /// passed over; failed when the thread running turns went away without one.
    fn ended(&self, task: &str, updates: &Receiver<Update>) -> Option<TaskSnapshot> {
        loop {
            match updates.recv() {
                Ok(Update::Status(_)) => {}
                Ok(Update::Ended(ended)) => return ended,
                Err(_) => {
                    return self.tasks.fail(task, "the agent stopped before the turn");
                }
            }
        }
    }

    /// Answers a streamed task with server-sent events, each a JSON-RPC response to the request
    /// whose result is a `StreamResponse`: the task as it was submitted, a `statusUpdate` for
    /// each status change its turn makes, then the task's end: an `artifactUpdate` for its record
    /// and a last `statusUpdate` with the state it ended in. Two things cut the stream, each at
    /// the answer's bound: an event not written whole within it
    /// ([`crate::http::EventStream::event`]), and [`STREAM_IDLE`] without an event, since a caller
    /// that stopped reading cannot be told from one that reads while the events fit the
    /// connection's buffers. A cut frees the admitted slot; the turn goes on, its task ended as it
    /// would be and readable by `GetTask`.
    fn stream(&self, connection: Connection, streamed: Streamed) {
        let Streamed { id, task, updates } = streamed;
        let Ok(mut events) = connection.stream() else {
            return;
        };
        let event = |result: Value| json!({"jsonrpc": "2.0", "id": id, "result": result});
        if events
            .event(&event(json!({"task": wire::task(&task)})).to_string())
            .is_err()
        {
            return events.cut();
        }
        let ended = loop {
            match updates.recv_timeout(STREAM_IDLE) {
                Ok(Update::Status(status)) => {
                    let update = status_update(&task, status);
                    if events
                        .event(&event(json!({"statusUpdate": update})).to_string())
                        .is_err()
                    {
                        return events.cut();
                    }
                }
                Ok(Update::Ended(ended)) => break ended,
                // Idle for the bound: the caller took no event within it.
                Err(RecvTimeoutError::Timeout) => return events.cut(),
                Err(RecvTimeoutError::Disconnected) => {
                    break self
                        .tasks
                        .fail(&task.id.0, "the agent stopped before the turn");
                }
            }
        };
        let last: Vec<Value> = match ended {
            Some(ended) => {
                let mut last: Vec<Value> = ended
                    .artifacts
                    .iter()
                    .flatten()
                    .map(|artifact| {
                        event(json!({"artifactUpdate": wire::artifact_update(
                            &TaskArtifactUpdateEvent {
                                task_id: ended.id.clone(),
                                context_id: context(&ended),
                                artifact: artifact.clone(),
                                append: None,
                                last_chunk: Some(true),
                                metadata: None,
                            },
                        )}))
                    })
                    .collect();
                let status = status_update(&ended, ended.status.clone());
                last.push(event(json!({ "statusUpdate": status })));
                last
            }
            None => vec![json!({"jsonrpc": "2.0", "id": id, "error": {
                "code": TASK_NOT_FOUND, "message": "the task is no longer held"}})],
        };
        for answer in last {
            if events.event(&answer.to_string()).is_err() {
                return events.cut();
            }
        }
        events.end();
    }

    /// `GetTask`: the task's current status, its record once it ended.
    fn get_task(&self, id: &Value, params: &Value) -> Response {
        let Some(task) = params.get("id").and_then(Value::as_str) else {
            return rpc_error(id, -32602, "`params.id` is required");
        };
        match self.tasks.get(task) {
            Some(task) => rpc_result(id, &wire::task(&task)),
            None => rpc_error(id, TASK_NOT_FOUND, "no such task"),
        }
    }
}

/// The longest `messageId` kept as the item revision verbatim.
pub const MAX_REVISION: usize = 128;

/// The item revision of a message whose `messageId` is `message_id`: the id itself when it is a
/// plain token (ASCII letters and digits, `-`, `_` and `.`, at most [`MAX_REVISION`] bytes) not
/// spelt as a hashed revision, otherwise the hashed revision: `m-` and the first 32 hex digits of
/// its SHA-256. No kept id has the hashed spelling, so two different ids share a revision only
/// when SHA-256 collides on them: the mapping keys the record's duplicate check, and a message is
/// never answered with another's line. The revision reaches the turn's context outside the quoted
/// item text, so no caller text that could end its quoting (a backtick, a line break) or carry an
/// instruction is ever kept in it.
pub fn item_revision(message_id: &str) -> String {
    let plain = !message_id.is_empty()
        && message_id.len() <= MAX_REVISION
        && message_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if plain && !hashed_spelling(message_id) {
        return message_id.to_owned();
    }
    let digest = Sha256::digest(message_id.as_bytes());
    let mut revision = String::with_capacity(HASHED_PREFIX.len() + HASHED_DIGITS);
    revision.push_str(HASHED_PREFIX);
    for byte in &digest[..HASHED_DIGITS / 2] {
        revision.push_str(&format!("{byte:02x}"));
    }
    revision
}

/// What a hashed revision starts with.
const HASHED_PREFIX: &str = "m-";

/// The hex digits of a hashed revision after its prefix.
const HASHED_DIGITS: usize = 32;

/// Whether `token` is spelt as a hashed revision: `m-` and 32 lowercase hex digits.
fn hashed_spelling(token: &str) -> bool {
    token.strip_prefix(HASHED_PREFIX).is_some_and(|digits| {
        digits.len() == HASHED_DIGITS
            && digits
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

/// The context of `task`; every task the listener holds has one ([`Tasks::submit`]).
fn context(task: &TaskSnapshot) -> ContextId {
    task.context_id
        .clone()
        .unwrap_or_else(|| ContextId(String::new()))
}

/// The `statusUpdate` of `task` moving to `status`.
fn status_update(task: &TaskSnapshot, status: TaskStatus) -> Value {
    wire::status_update(&TaskStatusUpdateEvent {
        task_id: task.id.clone(),
        context_id: context(task),
        status,
        metadata: None,
    })
}

/// `http://<host>` from the request's `Host` header, when it names a host and port only;
/// `http://localhost` otherwise.
fn base(head: &Head) -> String {
    let host = head
        .header("host")
        .filter(|host| {
            !host.is_empty()
                && host
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
        })
        .unwrap_or("localhost");
    format!("http://{host}")
}

fn refused(error: &A2aRefused) -> Response {
    let reason = a2a_refusal_spelling(error.reason);
    let detail = match error.reason {
        loom::agent::A2aRefusalReason::MissingBearer => "the request carries no bearer",
        loom::agent::A2aRefusalReason::WrongBearer => "the bearer is not this agent's",
    };
    let mut response = Response::json(401, &json!({"error": detail, "reason": reason}));
    response
        .headers
        .push(("WWW-Authenticate", "Bearer".to_owned()));
    response
}

fn not_allowed(allowed: &str) -> Response {
    let mut response = Response::error(405, "the method is not allowed here");
    response.headers.push(("Allow", allowed.to_owned()));
    response
}

fn rpc_result(id: &Value, result: &Value) -> Response {
    Response::json(200, &json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

fn rpc_error(id: &Value, code: i64, message: &str) -> Response {
    Response::json(
        200,
        &json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
    )
}
