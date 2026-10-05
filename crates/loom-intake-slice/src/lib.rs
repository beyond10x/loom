#![forbid(unsafe_code)]

//! The vertical slice: open a case on the picked protocol through the governor, and run
//! Commission's runtime over it with Loom as the executor and the local effect adapter performing
//! what the runtime admits (stories S3-S5, `story:runtime-merge`). The slice has no loop of its
//! own.
//!
//! - [`case`] opens the slice's case through `beyond10x/governor` and reports the workspace's new
//!   `HEAD` to it; the slice never evaluates Canon itself.
//! - [`selector`] implements Loom's action selector and argument generator over a model.
//! - [`executor`] performs a proposed `software.change/1` action inside the workspace; it never
//!   merges, pushes or deploys.
//! - [`effect`] is Commission's effect port over the executor: the local effect adapter.
//! - [`verifier`] is the only part that submits evidence, and only from the exit status of the test
//!   command the executor ran (Atlas ADR 0074). Nothing a model says becomes evidence.
//! - [`run`] drives them: references, classification, the case, then Commission's
//!   `run_until_blocked` until the run stops for a stated reason.

pub mod case;
pub mod effect;
pub mod executor;
pub mod run;
pub mod selector;
pub mod verifier;
