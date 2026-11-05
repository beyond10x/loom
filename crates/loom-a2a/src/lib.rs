//! The A2A listener of a served agent (`expose.a2a` of a `loom.agent/1` spec).
//!
//! [`A2aServer`] answers A2A 1.0.1 over JSON-RPC 2.0 at `expose.a2a.path`, its protocol types the
//! `a2a` crate of the a2a repository, pinned by tag:
//!
//! - `GET /.well-known/agent-card.json`: the agent card, with the spec's name, description and
//!   skills, the JSON-RPC interface and the bearer scheme ([`A2aServer::card`]).
//! - `SendMessage` (also `message/send`): one turn of the agent on the message's text, through
//!   [`Answerer::answer`], answered with the task it ended, the turn's record line its artifact.
//!   The item the turn runs on is `a2a:<revision>`, the revision given by the message's
//!   `messageId` ([`item_revision`]), so the answerer can tell a message it already answered.
//! - `SendStreamingMessage` (also `message/stream`): the same turn, answered `text/event-stream`:
//!   the task as submitted, one `statusUpdate` per runtime event of the run, then an
//!   `artifactUpdate` with the record and the last `statusUpdate` with the state the task ended in.
//!   Each event must be written within the answer's bound, and a stream that carries no event for
//!   that long ([`STREAM_IDLE`]) is cut too, while the turn goes on and `GetTask` reads its end.
//!   The card advertises `capabilities.streaming: true`.
//! - `GetTask` (also `tasks/get`): the task's status. While its turn runs, each runtime event of
//!   the run moves it ([`tasks::follows`]); the record ends it ([`tasks::ended`]).
//! - `GET /ready` and `GET /live`: the pod's probes.
//!
//! Every request but a probe needs `Authorization: Bearer <token>`, the token the agent's Secret
//! gives it ([`Bearer`]); one without it, or with another, is refused `401` before its body is
//! read, so it runs no turn and a stream sends no event (`loom.agent.AdmitA2aRequest`,
//! [`admit::admit_a2a_request`]). Nothing is pushed, no stream is resumed, and a message that
//! continues a task is refused: each message is one turn. Tasks are held in memory while the
//! process runs.

#![forbid(unsafe_code)]

pub mod admit;
mod http;
mod server;
pub mod tasks;
pub mod wire;

pub use admit::{Bearer, a2a_refusal_spelling, admit_a2a_request};
pub use http::WRITE_DEADLINE;
pub use server::{
    A2aServer, Answerer, BEARER_SCHEME, CARD_PATH, LIVE_PATH, MAX_CONNECTIONS, MAX_REVISION,
    PROTOCOL_BINDING, PROTOCOL_VERSION, READY_PATH, STREAM_IDLE, TASK_NOT_FOUND, item_revision,
};
