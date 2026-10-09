// generated from loom v1
// model digest e1bb43210be65dc6d79dc74d2c988eabf79b6c4dc3b946e02758171a1ad72c2a
// contract digest 381342ab4314fb0e17b8c1c01145eb7abb6bd79a86ff290d77fac2b2d3aa1a10
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

pub mod behaviour;
pub mod datasource;
pub mod evaluation;
pub mod governor;
pub mod json;
pub mod obligation;
pub mod primitives;
pub mod run;

