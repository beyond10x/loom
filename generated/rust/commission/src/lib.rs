// generated from commission v1
// model digest 13cc0558175429879e963bf7c04bdfe9e35b2c66d61ec900197077546340ca2e
// contract digest 08d3ce481bae0546b594ce01e47eee1e824d3a2be8bddc909791bd308f005d05
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

