//! The listener: routes, the JSON-RPC methods and the one thread that runs turns.
//!
//! Each connection is read on a thread of its own, at most [`MAX_CONNECTIONS`] at once; past it a
//! connection is answered `503`. Every request is admitted first ([`crate::admit::admit`]), on its
//! request line and headers: a probe is answered whatever it carries, every other request needs
//! the bearer and is refused `401` before its body is read. Turns run one at a time on the thread
//! that called [`A2aServer::serve`], which is the only one that touches the [`Answerer`], so an
//! answerer need not be shared between threads; a connection hands its message over and waits for
//! the record. A probe, the card and a task's status never wait for a turn.

use std::collections::BTreeMap;
use std::io;
use std::net::TcpListener;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use a2a::discovery::{
    AgentCapabilities, AgentCard, AgentInterface, AgentSkill, HTTPAuthSecurityScheme,
    SecurityRequirement, SecurityScheme, StringList,
};
use a2a::tasks::{ContextId, Message, Role, TaskSnapshot};
use b10x_loom_plugin::{InboundItem, ItemId, RecordLine};
use loom::agent::{A2aExposure, A2aRefused};
use loom_sdk::EventSink;
use serde_json::{Value, json};

use crate::admit::{AdmitA2aRequestOutcome, Bearer, a2a_refusal_spelling, admit};
use crate::http::{Connection, Head, Response};
use crate::tasks::{TaskEvents, Tasks};
use crate::wire;

/// Where the agent card is published (A2A 1.0.1 § 8.2).
pub const CARD_PATH: &str = "/.well-known/agent-card.json";

/// The readiness probe.
pub const READY_PATH: &str = "/ready";

/// The liveness probe.
pub const LIVE_PATH: &str = "/live";

/// The most connections read at once.
pub const MAX_CONNECTIONS: usize = 32;

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

/// One message handed to the thread that runs turns.
struct Job {
    task: String,
    item: InboundItem,
    reply: Sender<Result<RecordLine, String>>,
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
                streaming: Some(false),
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
        let active = AtomicUsize::new(0);
        let active = &active;
        std::thread::scope(|scope| {
            let acceptor = scope.spawn(move || -> io::Result<()> {
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let Ok(connection) = Connection::new(stream) else {
                                continue;
                            };
                            if active.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
                                connection.answer(&Response::error(503, "too many connections"));
                                continue;
                            }
                            active.fetch_add(1, Ordering::SeqCst);
                            let jobs = jobs.clone();
                            scope.spawn(move || {
                                self.connection(connection, &jobs, stop);
                                active.fetch_sub(1, Ordering::SeqCst);
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

    /// Runs one turn and hands its record back. A turn that panics fails its task.
    fn run<A: Answerer + ?Sized>(&self, agent: &A, job: Job) {
        let mut events = TaskEvents::new(&self.tasks, &job.task);
        let result = catch_unwind(AssertUnwindSafe(|| agent.answer(&job.item, &mut events)))
            .unwrap_or_else(|_| Err("the turn panicked".to_owned()));
        let _ = job.reply.send(result);
    }

    /// Reads, admits and answers one request.
    fn connection(&self, mut connection: Connection, jobs: &Sender<Job>, stop: &AtomicBool) {
        let head = match connection.head() {
            Ok(head) => head,
            Err(response) => return connection.answer(&response),
        };
        let probe = head.method == "GET" && (head.path == READY_PATH || head.path == LIVE_PATH);
        match admit(&self.bearer, probe, head.header("authorization")) {
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
            AdmitA2aRequestOutcome::Admitted => {}
        }
        let response = if head.path == CARD_PATH {
            if head.method == "GET" {
                Response::json(200, &wire::card(&self.card(&base(&head))))
            } else {
                not_allowed("GET")
            }
        } else if head.path == self.exposure.path {
            if head.method == "POST" {
                match connection.body(&head) {
                    Ok(body) => self.rpc(&body, jobs),
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

    /// One JSON-RPC 2.0 request.
    fn rpc(&self, body: &[u8], jobs: &Sender<Job>) -> Response {
        let Ok(request) = serde_json::from_slice::<Value>(body) else {
            return rpc_error(&Value::Null, -32700, "the body is no JSON");
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        if !matches!(id, Value::Null | Value::String(_) | Value::Number(_)) {
            return rpc_error(&Value::Null, -32600, "`id` is no string or number");
        }
        if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return rpc_error(&id, -32600, "the request is no JSON-RPC 2.0 request");
        }
        let Some(method) = request.get("method").and_then(Value::as_str) else {
            return rpc_error(&id, -32600, "the request names no method");
        };
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        match method {
            "SendMessage" | "message/send" => self.send(&id, &params, jobs),
            "GetTask" | "tasks/get" => self.get_task(&id, &params),
            _ => rpc_error(
                &id,
                -32601,
                "the method is not served: SendMessage (message/send) and GetTask (tasks/get) are",
            ),
        }
    }

    /// `SendMessage`: one turn on the message's text, answered with the task it ended.
    fn send(&self, id: &Value, params: &Value, jobs: &Sender<Job>) -> Response {
        let Some(message) = params.get("message").filter(|message| message.is_object()) else {
            return rpc_error(id, -32602, "`params.message` is required");
        };
        if message
            .get("taskId")
            .is_some_and(|task| !task.is_null() && task != "")
        {
            return rpc_error(
                id,
                -32602,
                "each message is one turn: a message continuing a task is not answered",
            );
        }
        let Some(text) = message
            .get("parts")
            .and_then(Value::as_array)
            .and_then(|parts| wire::request_text(parts))
        else {
            return rpc_error(id, -32602, "the message carries no text part");
        };
        let message_id = message
            .get("messageId")
            .and_then(Value::as_str)
            .filter(|given| !given.is_empty())
            .map_or_else(|| "message".to_owned(), str::to_owned);
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
        let item = InboundItem {
            id: ItemId(format!("a2a:{task_id}")),
            revision: message_id,
            text,
            details: loom::json::parse(&details.to_string()).unwrap_or(loom::json::Value::Null),
        };
        let (reply, answer) = mpsc::channel();
        let ended = if jobs
            .send(Job {
                task: task_id.clone(),
                item,
                reply,
            })
            .is_err()
        {
            self.tasks.fail(&task_id, "the agent is stopping")
        } else {
            match answer.recv() {
                Ok(Ok(record)) => self.tasks.end(&task_id, &record),
                Ok(Err(why)) => self
                    .tasks
                    .fail(&task_id, &format!("the turn could not run: {why}")),
                Err(_) => self
                    .tasks
                    .fail(&task_id, "the agent stopped before the turn"),
            }
        };
        match ended {
            Some(task) => rpc_result(id, &json!({"task": wire::task(&task)})),
            None => rpc_error(id, TASK_NOT_FOUND, "the task is no longer held"),
        }
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
