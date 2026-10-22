// generated from intake v1
// model digest cf1fbbfc1206e350609f5f4f3522b47696f03a8049cc32a47b5ed52b150d1d84
// contract digest 7dd547604a8ca8784e13b80a1979b25542e31ca93b347da47fcc3787eeb2950c
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

