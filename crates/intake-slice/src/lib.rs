#![forbid(unsafe_code)]

//! The vertical slice: open a case on the picked protocol through the governor, run Loom over it
//! and execute local actions until blocked (stories S3-S5). Temporary: Commission's local runtime
//! loop replaces it.
//!
//! - [`case`] opens the slice's case through `beyond10x/governor` and reports the workspace's new
//!   `HEAD` to it; the slice never evaluates Canon itself.
//! - [`selector`] implements Loom's action selector and argument generator over a model.
//! - [`executor`] performs a proposed `software.change/1` action inside the workspace; it never
//!   merges, pushes or deploys.
//! - [`verifier`] is the only part that submits evidence, and only from the exit status of the test
//!   command the executor ran (Atlas ADR 0074). Nothing a model says becomes evidence.
//! - [`run`] is the loop that drives them: references, classification, the case, then frontier,
//!   Loom, executor, verifier and completion until the run stops for a stated reason.

pub mod case;
pub mod executor;
pub mod run;
pub mod selector;
pub mod verifier;
