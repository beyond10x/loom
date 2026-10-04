//! Adversary pass 1 (wave 2026-10-04-w3, unit commission/port-skeleton).
//!
//! `story:port-skeleton` § Outcome (b) removes the two `PENDING_REPLACEMENT` entries that let a
//! hand-written `ExecutorOutcome` and `AuthorityDecision` stand beside the generated types. After
//! it, `no-hand-model` must refuse a hand-written definition of either, in any file under
//! `crates/commission/src` — including the port modules the next wave's stories fill. No other
//! case in the workspace defines either name, so restoring the allowance leaves the suite green.
//!
//! The check runs through the real `commission-xtask` binary of the tree this test sits in
//! (`cargo run -p commission-xtask`, rooted at `CARGO_MANIFEST_DIR/../..`, read when the test runs).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("run through cargo");
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

fn case_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary_skeleton")
        .join(format!("{name}-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear case dir");
    }
    fs::create_dir_all(&dir).expect("create case dir");
    dir
}

#[test]
fn adversary_skeleton_no_hand_model_refuses_hand_written_bootstrap_outcomes_in_port_modules() {
    let root = root();
    let case = case_dir("bootstrap_outcomes_in_ports");
    let src = case.join("src");
    fs::create_dir_all(src.join("ports")).expect("create ports");
    fs::write(src.join("lib.rs"), "pub mod ports;\n").expect("write lib.rs");
    fs::write(
        src.join("ports/mod.rs"),
        "pub mod authority;\npub mod executor;\n",
    )
    .expect("write ports/mod.rs");
    fs::write(
        src.join("ports/executor.rs"),
        "pub enum ExecutorOutcome {\n    NoUsefulAction,\n}\n",
    )
    .expect("write ports/executor.rs");
    fs::write(
        src.join("ports/authority.rs"),
        "pub enum AuthorityDecision {\n    Allow,\n}\n",
    )
    .expect("write ports/authority.rs");

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = Command::new(cargo)
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(&root)
        .args(["run", "-q", "--locked", "-p", "commission-xtask", "--"])
        .arg("--root")
        .arg(&root)
        .arg("no-hand-model")
        .arg("--src")
        .arg(&src)
        .arg("--generated")
        .arg(root.join("generated/rust/commission"))
        .output()
        .expect("run commission-xtask");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let _ = fs::remove_dir_all(&case);

    assert!(
        !out.status.success(),
        "no-hand-model passed hand-written ExecutorOutcome and AuthorityDecision:\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    for (file, item) in [
        ("ports/executor.rs:1", "enum ExecutorOutcome"),
        ("ports/authority.rs:1", "enum AuthorityDecision"),
    ] {
        assert!(
            stderr
                .lines()
                .any(|line| line.contains(file) && line.contains(item)),
            "no-hand-model did not refuse `{item}` at {file}:\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }
}
