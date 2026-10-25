//! `crd-generate` and `crd-drift` over a copy of the repository's `ess/` and `deploy/crd/`
//! (`story:loom-agent-resource-domain`):
//!
//! * `crd-drift` passes on an unchanged copy;
//! * it fails, naming the file, when one byte of the committed manifest changes and when the file is
//!   missing;
//! * `crd-generate` writes the committed manifest's bytes back, after which `crd-drift` passes.
//!
//! Both need the `ess` binary on `PATH`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const FILE: &str = "deploy/crd/loomagents.loom.beyond10x.dev.yaml";

fn repository() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
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

/// A copy of `ess/` and `deploy/crd/` under a fresh directory below `CARGO_TARGET_TMPDIR`.
fn copy_of_repository() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let to = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("crd-{}-{nanos}", std::process::id()));
    for part in ["ess", "deploy/crd"] {
        copy_tree(&repository().join(part), &to.join(part));
    }
    to
}

fn xtask(root: &Path, command: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_loom-xtask"))
        .arg("--root")
        .arg(root)
        .arg(command)
        .arg("--scratch")
        .arg(root.join("scratch"))
        .output()
        .unwrap_or_else(|e| panic!("run loom-xtask: {e}"))
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn crd_drift_names_a_changed_or_missing_manifest_and_crd_generate_restores_it() {
    let root = copy_of_repository();
    let file = root.join(FILE);
    let committed = fs::read(&file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));

    let clean = xtask(&root, "crd-drift");
    assert!(
        clean.status.success(),
        "crd-drift failed on an unchanged copy: {}",
        stderr(&clean)
    );

    let mut bytes = committed.clone();
    let last = bytes.len() - 2;
    bytes[last] = if bytes[last] == b'x' { b'y' } else { b'x' };
    fs::write(&file, bytes).unwrap_or_else(|e| panic!("write {}: {e}", file.display()));
    let changed = xtask(&root, "crd-drift");
    assert_eq!(changed.status.code(), Some(1), "{}", stderr(&changed));
    assert!(
        stderr(&changed).contains(&format!("{FILE}: differs from a fresh projection of ess/")),
        "crd-drift did not name the changed file: {}",
        stderr(&changed)
    );

    fs::remove_file(&file).unwrap_or_else(|e| panic!("remove {}: {e}", file.display()));
    let missing = xtask(&root, "crd-drift");
    assert_eq!(missing.status.code(), Some(1), "{}", stderr(&missing));
    assert!(
        stderr(&missing).contains(&format!("{FILE}: missing")),
        "crd-drift did not name the missing file: {}",
        stderr(&missing)
    );

    let generated = xtask(&root, "crd-generate");
    assert!(
        generated.status.success(),
        "crd-generate failed: {}",
        stderr(&generated)
    );
    assert!(
        fs::read(&file).ok() == Some(committed),
        "crd-generate wrote other bytes than the committed manifest"
    );
    let restored = xtask(&root, "crd-drift");
    assert!(
        restored.status.success(),
        "crd-drift failed after crd-generate: {}",
        stderr(&restored)
    );
    let _ = fs::remove_dir_all(&root);
}
