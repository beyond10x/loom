// generated from intake v1
// model digest 8158f2c6f08736511dded8320855abb57ef13c8b3b492ffef5ed2fd4bb05d87d
// contract digest 6edfc386e180a0eea5e77b3cac17ef9f12f629f4d81c0918f6db81cbb71e4848
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

pub mod confinement;
pub mod context;
pub mod primitives;
pub mod protocols;
pub mod query;
pub mod results;
pub mod routing;

