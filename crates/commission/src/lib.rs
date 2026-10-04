#![forbid(unsafe_code)]

//! Commission: the responsibility model and the runtime contracts of governed autonomous workers.
//!
//! The responsibility model (Agent, AgentRevision, Case, Commission and their ids, and the port
//! vocabulary) is generated from `ess/` into `generated/rust/commission/` and re-exported here as
//! [`model`]. It is never written by hand. Each port and runtime module is written by the story
//! that fills it, over the generated types.

/// The responsibility model, synthesized from the ESS specification.
pub use commission as model;

pub mod action_request;
pub mod admission;
pub mod outcome;
pub mod ports;
pub mod runtime;
