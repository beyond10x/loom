// generated from intake v1
// contract digest 580c92e6eb1d15ced150fd8c74b88b52f6a70116a600645f3a0299a0c5b3b9c5
// do not edit: regenerate with `ess synthesize --layout crate`

//! Semantic types synthesised from the `intake` specification, v1.
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
pub mod bindings;
pub mod confinement;
pub mod context;
pub mod events;
pub mod json;
pub mod obligation;
pub mod primitives;
pub mod protocols;
pub mod query;
pub mod results;
pub mod routing;

