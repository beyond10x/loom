//! The A2A listener of a served agent (`expose.a2a` of a `loom.agent/1` spec).
//!
//! [`A2aServer`] answers A2A 1.0.1 over JSON-RPC 2.0 at `expose.a2a.path`, its protocol types the
//! `a2a` crate of the a2a repository, pinned by tag:
//!
//! - `GET /.well-known/agent-card.json`: the agent card, with the spec's name, description and
//!   skills, the JSON-RPC interface and the bearer scheme ([`A2aServer::card`]).
//! - `SendMessage` (also `message/send`): one turn of the agent on the message's text, through
//!   [`Answerer::answer`], answered with the task it ended, the turn's record line its artifact.
//! - `GetTask` (also `tasks/get`): the task's status. While its turn runs, each runtime event of
//!   the run moves it ([`tasks::follows`]); the record ends it ([`tasks::ended`]).
//! - `GET /ready` and `GET /live`: the pod's probes.
//!
//! Every request but a probe needs `Authorization: Bearer <token>`, the token the agent's Secret
//! gives it ([`Bearer`]); one without it, or with another, is refused `401` before its body is
//! read, so it runs no turn (`loom.agent.AdmitA2aRequest`, [`admit::admit_a2a_request`]). Nothing
//! is streamed, nothing is pushed, and a message that continues a task is refused: each message is
//! one turn. Tasks are held in memory while the process runs.

#![forbid(unsafe_code)]

pub mod admit;
mod http;
mod server;
pub mod tasks;
pub mod wire;

pub use admit::{Bearer, a2a_refusal_spelling, admit_a2a_request};
pub use server::{
    A2aServer, Answerer, BEARER_SCHEME, CARD_PATH, LIVE_PATH, MAX_CONNECTIONS, MAX_REVISION,
    PROTOCOL_BINDING, PROTOCOL_VERSION, READY_PATH, TASK_NOT_FOUND, item_revision,
};
