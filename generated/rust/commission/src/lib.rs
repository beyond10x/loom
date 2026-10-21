// generated from commission v1
// model digest 3c8fd62cc537868801ef5f8fd3dcc0be5e0f74acf288eccfca0336e5245c0efb
// contract digest 5b702c6cb6d14e2446886f45b028e6c0c016ba1b3e9c7c2ecda0fb917551e418
// do not edit: regenerate with `ess synthesize --layout crate`

//! Semantic types synthesised from the `commission` specification, v1.
//!
//! The responsibility model for governed autonomous workers: an agent revision commissioned to a durable case under a governor, the frontier the governor returns, and the runs that act on it.
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
pub mod json;
pub mod obligation;
pub mod primitives;
pub mod responsibility;

