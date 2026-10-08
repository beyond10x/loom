// generated from loom v1
// model digest fceae929c72d09b0daae25cf633a1e1ff9f15b5dedb71f4802c193c4cdcb7346
// contract digest f7a202983357f2892c4cdac99b9d5de1e11fca800c6db3b1671858c0c2e3a6da
// do not edit: regenerate with `ess synthesize --layout crate`

//! Semantic types synthesised from the `loom` specification, v1.
//!
//! The native agent harness: one bounded run of a model inside a governed frontier, with the model-visible catalogue projected from that frontier and action selection kept apart from argument generation.
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
pub mod obligation;
pub mod primitives;
pub mod run;

