// generated from intake v1
// model digest b3281773ee88313834099ce04aab0b9af436baedbe62ff1a432c305c3ed756d4
// contract digest eba8d734a7767f27148ac6b7d3a96ad427989af6d293d3c3496fba405caae95a
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

