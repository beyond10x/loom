// generated from intake v1
// model digest e5f987ee3613bbc2c1f7decd667a2b91fb1819f149cc970316814040874257c4
// contract digest 3bcee985eed31b4d9ea384c5bf3512a98619ba6016205bfe0291f1860171e18a
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

