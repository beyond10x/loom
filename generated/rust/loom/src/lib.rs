// generated from loom v1
// model digest 3e1ee968ef2fb0a16532ff464c05cda3aea1e31214346df30127c1b34add8a76
// contract digest 1a7a553e3b0762a3bff46ca087b038a5715a83a7557d5ae2be080d28630c379d
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
pub mod obligation;
pub mod plugin;
pub mod primitives;
pub mod run;
pub mod slack;
pub mod workflow;

