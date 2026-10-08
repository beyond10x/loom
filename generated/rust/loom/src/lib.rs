// generated from loom v1
// model digest 394821f0396bb3a17570338485b73d8deca7ebb2683022f51d2a1bfd9ff6b071
// contract digest 13338a26fdf5612984f1fb0d4081af14c5d2dfeb6c2a7cb563fdb3f59f0c9697
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
pub mod evaluation;
pub mod governor;
pub mod json;
pub mod obligation;
pub mod primitives;
pub mod run;

