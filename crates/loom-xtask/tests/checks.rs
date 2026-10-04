//! Acceptance 4 and 5 of `story:agent-executor`, run against copies of the repository.
//!
//! * `drift` fails, naming the file, when one byte of a file under `generated/rust/loom/src/`
//!   changes, and passes on an unchanged copy.
//! * `no-hand-model` fails, naming file and line, when `pub struct Selection { pub action: String }`
//!   is added to `crates/loom/src/lib.rs`, and passes on an unchanged copy.
//!
//! The repository is read from `CARGO_MANIFEST_DIR` at run time; copies land under
//! `CARGO_TARGET_TMPDIR`. Both checks need the `ess` binary on `PATH`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const HAND_TYPE: &str = "pub struct Selection { pub action: String }";

fn repository() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

/// A fresh, empty directory under `CARGO_TARGET_TMPDIR` for one test.
fn workspace(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("checks-{name}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
    dir
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap_or_else(|e| panic!("create {}: {e}", to.display()));
    for entry in fs::read_dir(from).unwrap_or_else(|e| panic!("read {}: {e}", from.display())) {
        let entry = entry.unwrap_or_else(|e| panic!("read {}: {e}", from.display()));
        let target = to.join(entry.file_name());
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target)
                .unwrap_or_else(|e| panic!("copy {}: {e}", entry.path().display()));
        }
    }
}

/// A copy of `ess/`, `generated/rust/loom/` and `crates/loom/src/` under a fresh directory.
fn copy_of_repository(name: &str) -> PathBuf {
    let from = repository();
    let to = workspace(name);
    for part in ["ess", "generated/rust/loom", "crates/loom/src"] {
        copy_tree(&from.join(part), &to.join(part));
    }
    to
}

fn xtask(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_loom-xtask"))
        .arg("--root")
        .arg(root)
        .args(args)
        .arg("--scratch")
        .arg(root.join("scratch"))
        .output()
        .unwrap_or_else(|e| panic!("run loom-xtask: {e}"))
}

fn no_hand_model(root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_loom-xtask"))
        .arg("--root")
        .arg(root)
        .arg("no-hand-model")
        .output()
        .unwrap_or_else(|e| panic!("run loom-xtask: {e}"))
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn drift_names_a_changed_generated_file() {
    let root = copy_of_repository("drift");

    let clean = xtask(&root, &["drift"]);
    assert!(
        clean.status.success(),
        "drift failed on an unchanged copy: {}",
        stderr(&clean)
    );

    let file = root.join("generated/rust/loom/src/run.rs");
    let mut bytes = fs::read(&file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
    let last = bytes.len() - 2;
    bytes[last] = if bytes[last] == b'x' { b'y' } else { b'x' };
    fs::write(&file, bytes).unwrap_or_else(|e| panic!("write {}: {e}", file.display()));

    let changed = xtask(&root, &["drift"]);
    assert!(
        !changed.status.success(),
        "drift passed with a byte of src/run.rs changed"
    );
    assert!(
        stderr(&changed).contains("src/run.rs: differs from synthesis"),
        "drift did not name src/run.rs: {}",
        stderr(&changed)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn no_hand_model_names_file_and_line_of_a_hand_written_selection() {
    let root = copy_of_repository("hand-model");

    let clean = no_hand_model(&root);
    assert!(
        clean.status.success(),
        "no-hand-model failed on an unchanged copy: {}",
        stderr(&clean)
    );

    let lib = root.join("crates/loom/src/lib.rs");
    let mut text =
        fs::read_to_string(&lib).unwrap_or_else(|e| panic!("read {}: {e}", lib.display()));
    if !text.ends_with('\n') {
        text.push('\n');
    }
    let line = text.lines().count() + 1;
    text.push_str(HAND_TYPE);
    text.push('\n');
    fs::write(&lib, text).unwrap_or_else(|e| panic!("write {}: {e}", lib.display()));

    let found = no_hand_model(&root);
    assert!(
        !found.status.success(),
        "no-hand-model passed with `{HAND_TYPE}` in lib.rs"
    );
    let expected = format!("{}:{line}: `struct Selection`", lib.display());
    assert!(
        stderr(&found).contains(&expected),
        "no-hand-model did not name {expected}: {}",
        stderr(&found)
    );
    let _ = fs::remove_dir_all(&root);
}
