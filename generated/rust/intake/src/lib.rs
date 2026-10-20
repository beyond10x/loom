// generated from intake v1
// model digest 04c180bd51b47a13b3848f21e849c765b36b48b6b4ca1bf5684b50f2f134945c
// contract digest 908a52d11757a390dac329e10b9835c22ddbb918efa0885066a58a1ac7ce5cc1
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

