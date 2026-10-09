// generated from loom v1
// model digest 866bbd9d47246b4227f3631ebb34d83e265512af496b4fe99b0098c8cc298c10
// contract digest 08c02cd39830806f4c6eaa95dfea4ecf631c548ae35d9b4e43d6ab0db1b725ec
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
pub mod datasource;
pub mod evaluation;
pub mod governor;
pub mod json;
pub mod obligation;
pub mod plugin;
pub mod primitives;
pub mod run;
pub mod slack;

