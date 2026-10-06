// generated from intake v1
// model digest 3f5061fef390c4dc8c27551efdb85452d71f8ce8cb803ed1591bf3a2cbaa8684
// contract digest 56faa06c4640455cfd2398e5c789037ef0cc12087603ed1708c9d2ed2ba6bfa7
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
pub mod primitives;
pub mod results;
pub mod routing;

