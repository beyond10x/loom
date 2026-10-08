// generated from commission v1
// model digest 43a3c632efbe62d6009e162f9228cf9e37cae5f9d01b17fe7605529448a1ff38
// contract digest 66bcc392ea844a2f5a3d58c7ee43d70995938794d6d94bc82c744f2b7312e04e
// do not edit: regenerate with `ess synthesize --layout crate`

//! Semantic types synthesised from the `commission` specification, v1.
//!
//! The responsibility model for governed autonomous workers: an agent revision commissioned to a durable case under a governor, the frontier the governor returns, and the runs that act on it.
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
pub mod json;
pub mod obligation;
pub mod primitives;
pub mod responsibility;

