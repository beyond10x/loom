// generated from intake v1
// model digest f7fab3ba3958266082725d66c0d412f3b6ea6a13a27a13510350f0b62657a8e3
// contract digest de8b096fd7c892c2a32f23173c969f86f7576e033c100ea6b2e2e7537e8531e1
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
pub mod results;
pub mod routing;

