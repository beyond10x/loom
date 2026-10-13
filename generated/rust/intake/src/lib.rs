// generated from intake v1
// model digest 19ac3d8d1db2341a963fcfb8d85843ba6e1da6d764dc7e98dd4d43eed5077883
// contract digest 09dd5b3cae539d5e002453702e5eec93d09e4697eecb6cd2f7a112ab5e12c81c
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
pub mod events;
pub mod obligation;
pub mod primitives;
pub mod protocols;
pub mod query;
pub mod results;
pub mod routing;

