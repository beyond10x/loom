// generated from intake v1
// model digest 0dcbb44891d966a08164d0845cc89345f20e9da1a2bff9d8412d32181f207f99
// contract digest 03a4eba225331c590679bbbce8fd69d4e4f51fa6589d185caf24c7551cb770fd
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
pub mod primitives;
pub mod protocols;
pub mod query;
pub mod results;
pub mod routing;

