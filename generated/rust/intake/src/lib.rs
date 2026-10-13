// generated from intake v1
// model digest 58fc59c73e160b3816e4c2add829c8c5a6c05ea1c6901838a7d61a911e157818
// contract digest c7bf5a36bfa3d72e856cecfa8dd5cf67906769eb0ed8049fb9d732fc4c997443
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

