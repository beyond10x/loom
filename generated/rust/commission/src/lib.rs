// generated from commission v1
// model digest 350c43fc092c90e046f38c4d6de7ff571c845c296f6ba8d0f64258e21170d928
// contract digest 2354d7e1a107137146df3a47ff2f31ec3c25aeb71755bdbf6a47908cf2fe89f2
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

