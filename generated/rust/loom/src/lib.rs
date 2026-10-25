// generated from loom v1
// model digest 36d9f391b47bf4ff392161dfbb07aa4606f2481a4cc3a3d4ac33a45bc10d5732
// contract digest 00b46a8d1edd40edd961dc9dcf2afe82e3d81210aec3a4882dbcfa4b17960ae1
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

pub mod agent;
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
pub mod session;
pub mod slack;
pub mod workflow;

