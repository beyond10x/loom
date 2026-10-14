//! Loom's documented embedding, built as an external crate.
//!
//! The embedding is `crates/loom-sdk/examples/software_change.rs`, included here by path so the
//! documented embedding and the checked one are one file. This crate depends on `b10x-loom-sdk`
//! alone and is its own workspace. `main` does what the example's own `main` does, which a parent
//! module cannot call: it runs the example's `run` in `target/loom-sdk-example` under the current
//! directory and prints each step, then `stopped: ApprovalRequired (repository.merge)`.

// The example's own `main` and its public constants are not called from here.
#[allow(dead_code)]
#[path = "../../../crates/loom-sdk/examples/software_change.rs"]
mod example;

use std::error::Error;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let workspace = Path::new("target").join("loom-sdk-example");
    if workspace.exists() {
        fs::remove_dir_all(&workspace)?;
    }
    println!("workspace: {}", workspace.display());
    let end = example::run(&workspace)?;
    println!("stopped: {}", example::stop_reason(&end));
    Ok(())
}
