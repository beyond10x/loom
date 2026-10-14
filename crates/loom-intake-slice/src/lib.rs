#![forbid(unsafe_code)]

//! The vertical slice: open a case on the picked protocol through the governor, and run
//! Commission's runtime over it with Loom as the executor and the local effect adapter performing
//! what the runtime admits (stories S3-S5, `story:runtime-merge`). The slice has no loop of its
//! own.
//!
//! - [`case`] opens the slice's case through `beyond10x/governor` and reports the workspace's new
//!   `HEAD` to it; the slice never evaluates Canon itself.
//! - [`selector`] implements Loom's action selector and argument generator over a model.
//! - [`executor`] performs a proposed `software.change/1` action inside the workspace, `tests.write`
//!   only on the host's [`test_paths`]; it never merges, pushes or deploys.
//! - [`git`] builds every git command the slice runs on the host: no hook, fsmonitor or signing
//!   program runs, and a call is refused when the workspace's own configuration names a program or
//!   changed since the case opened.
//! - [`effect`] is Commission's effect port over the executor: the local effect adapter.
//! - [`verifier`] is the only part that submits evidence, and only from the exit status of the test
//!   command the executor ran (Atlas ADR 0074). Nothing a model says becomes evidence.
//! - [`run`] drives them: references, classification, the case, then Commission's
//!   `run_until_blocked` until the run stops for a stated reason.
//! - [`resume`] keeps the resume context `run --state` writes beside the case and the
//!   commission's Suspension.
//! - [`grant`] keeps the grant `approve` writes there, and reads it as a resume's authority
//!   provider; [`handoff`] records the merge hand-off a resumed `repository.merge` performs, which
//!   moves no Git ref.

pub mod case;
pub mod confinement;
pub mod context;
pub mod context_metrics;
pub mod effect;
pub mod executor;
pub mod git;
pub mod grant;
pub mod handoff;
pub mod results;
pub mod resume;
pub mod run;
pub mod selector;
mod state_dir;
pub mod test_paths;
pub mod verifier;

pub mod clock;
pub mod intent;

mod model_retry;
