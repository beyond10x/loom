// generated from commission v1
// model digest f53571e132b2f71840edfe761bc401e64fb69f09c02e5231f9f99c89c76020a5
// contract digest d8e6b1ede68b8946fc3b58215b93b34033de96d39a3f84cf8669f4b6e8966278
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

