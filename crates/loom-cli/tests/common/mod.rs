//! Helpers that more than one test binary of `b10x-loom-cli` shares: `tests/main.rs` declares this
//! module, and `tests/tree_run.rs` reaches the same files through `#[path]`.

pub mod delegated_cgroup;
pub mod delegation_probe;
