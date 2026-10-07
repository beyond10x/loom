// generated from intake v1
// model digest 27eba34a158e1027a48b4584d4a9cfd7db24040200cceaab4f1148ba47ce865e
// contract digest 6db3749de79840686760677941a2171cf13badb3ab1d8d4190151e3870d23df9
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
pub mod protocols;
pub mod query;
pub mod results;
pub mod routing;

