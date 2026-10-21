// generated from loom v1
// model digest 1d444202ff5ea55b90c19ce388d5b307090dc8b11d651cdd4e3ec3305a5c4630
// contract digest 747f718a7d6e4cdc7f8cb59838b8d3065716a8e71730de52af6f47cf9f5d6546
// do not edit: regenerate with `ess synthesize --layout crate`

//! Semantic types synthesised from the `loom` specification, v1.
//!
//! The native agent harness: one bounded run of a model inside a governed frontier, with the model-visible catalogue projected from that frontier and action selection kept apart from argument generation.
//!
//! Generated, not written: the specification is the source of truth, and the door to changing
//! anything here is `ess synthesize`. What is deliberately absent — behaviour, queries,
//! escalations — is listed with reasons in the `PLAN.md` beside this workspace, and every entry
//! there is owed through a typed seam in an `obligations` module here.

// `deny`, not the source workspace's lint set: this crate must hold on its own, and an undocumented
// public item here is an emitter defect worth failing the gate over.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod authority;
pub mod behaviour;
pub mod datasource;
pub mod evaluation;
pub mod governor;
pub mod json;
pub mod metering;
pub mod models;
pub mod obligation;
pub mod plugin;
pub mod primitives;
pub mod run;
pub mod slack;
pub mod workflow;

