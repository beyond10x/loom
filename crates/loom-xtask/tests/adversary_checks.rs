//! Adversary pass 1 on `story:agent-executor`: what `drift` and `no-hand-model` miss.
//!
//! The repository is read from `CARGO_MANIFEST_DIR` at run time; copies land under
//! `CARGO_TARGET_TMPDIR`. Both checks need the `ess` binary on `PATH`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn repository() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

fn workspace(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adversary-{name}-{}-{nanos}", std::process::id()));
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

fn copy_of_repository(name: &str) -> PathBuf {
    let from = repository();
    let to = workspace(name);
    for part in ["ess", "generated/rust/loom", "crates/loom-executor/src"] {
        copy_tree(&from.join(part), &to.join(part));
    }
    to
}

fn xtask(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_loom-xtask"))
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("run loom-xtask: {e}"))
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn append(file: &Path, text: &str) -> usize {
    let mut body =
        fs::read_to_string(file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
    if !body.ends_with('\n') {
        body.push('\n');
    }
    let line = body.lines().count() + 1;
    body.push_str(text);
    body.push('\n');
    fs::write(file, body).unwrap_or_else(|e| panic!("write {}: {e}", file.display()));
    line
}

/// The generated crate declares `SessionData`, `SessionSnapshot`, `AnySession` and the state
/// markers beside `Session`; they are model types as much as `Session` is (Commission's own
/// `no-hand-model` reserves every type its generated crate declares). A hand-written one in
/// `crates/loom-executor/src` must be refused.
#[test]
fn no_hand_model_refuses_derived_model_types() {
    let root = copy_of_repository("derived");
    let lib = root.join("crates/loom-executor/src/lib.rs");
    let mut missed = Vec::new();
    for item in [
        "pub struct SessionData { pub commission_run: String }",
        "pub struct SelectionSnapshot;",
        "pub enum AnySession { A }",
        "pub struct TurnData;",
    ] {
        let original = fs::read_to_string(&lib).unwrap_or_else(|e| panic!("read: {e}"));
        let line = append(&lib, item);
        let found = xtask(&root, &["no-hand-model"]);
        let expected = format!("{}:{line}:", lib.display());
        if found.status.success() || !stderr(&found).contains(&expected) {
            missed.push(format!("{item} (exit {:?})", found.status.code()));
        }
        fs::write(&lib, original).unwrap_or_else(|e| panic!("restore copy: {e}"));
    }
    let _ = fs::remove_dir_all(&root);
    assert!(
        missed.is_empty(),
        "no-hand-model passed with a hand-written generated model type in lib.rs:\n{}",
        missed.join("\n")
    );
}

/// `drift` compares every file synthesis writes: a hand edit of the generated manifest or of
/// `PLAN.md` is drift, named by file.
#[test]
fn drift_names_edited_manifest_and_plan() {
    for file in ["Cargo.toml", "PLAN.md", "plan.json"] {
        let root = copy_of_repository("drift-meta");
        let path = root.join("generated/rust/loom").join(file);
        append(&path, " ");
        let out = xtask(
            &root,
            &[
                "drift",
                "--scratch",
                root.join("scratch").to_str().unwrap_or_default(),
            ],
        );
        let named = format!("{file}: differs from synthesis");
        let message = stderr(&out);
        let _ = fs::remove_dir_all(&root);
        assert!(
            !out.status.success() && message.contains(&named),
            "drift did not name an edited {file}: {message}"
        );
    }
}
