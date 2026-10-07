// generated from intake v1
// model digest 791dfd631daf6ccb1e85217e76d8ca4a6fdba59b60b38fc670f733bbdff43ff9
// contract digest cdbea5c1c2bfca06a3ee6fbe99e68d74461962be6d0553de731a2f4897676fe3
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

