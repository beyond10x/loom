// generated from intake v1
// model digest c5457c9e4735af8938031585724e656e9eaa6bc6ff00f6761d8287a591db7f8c
// contract digest f815c76fa97e91e076dd53cce6f3557b05973d54a2494e2d06a54f7355fd9c76
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

