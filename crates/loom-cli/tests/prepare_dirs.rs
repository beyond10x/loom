//! `b10x-loom agent prepare-dirs` (`story:writable-dirs-mode-0700`): the directory it makes in a
//! kubelet-style emptyDir root (world-writable, setgid, sticky) is the caller's, mode exactly
//! 0700, so a file written there at 0600 satisfies the `b10x-gates` protected reader; a rerun
//! keeps what the directory holds, and a link or a file in its place is refused.

use std::fs;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("prepare_dirs")
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean scratch");
    }
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

/// An emptyDir root as the kubelet leaves it with `fsGroup`: world-writable, sticky and setgid.
fn empty_dir(base: &Path, name: &str) -> PathBuf {
    let root = base.join(name);
    fs::create_dir(&root).expect("emptyDir");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o3777)).expect("root mode");
    root
}

fn prepare(dirs: &[&Path]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_b10x-loom"));
    command.args(["agent", "prepare-dirs"]);
    for dir in dirs {
        command.arg("--dir").arg(dir);
    }
    command.output().expect("b10x-loom runs")
}

/// `b10x-gates` `protected_read` (gates/src/policy.rs:206-231): the file's uid is the caller's
/// and `mode & 077 == 0`; its parent's uid is the caller's and `mode & 022 == 0`.
fn gates_accepts(path: &Path, caller: u32) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err("not a regular file".into());
    }
    if meta.uid() != caller || meta.permissions().mode() & 0o077 != 0 {
        return Err(format!(
            "file uid {} mode {:o}",
            meta.uid(),
            meta.permissions().mode()
        ));
    }
    let parent = fs::metadata(path.parent().expect("parent")).map_err(|e| e.to_string())?;
    if parent.uid() != caller || parent.permissions().mode() & 0o022 != 0 {
        return Err(format!(
            "dir uid {} mode {:o}",
            parent.uid(),
            parent.permissions().mode()
        ));
    }
    Ok(())
}

/// A credential file written at 0600 into `dir`, as a program in the pod writes one.
fn write_private(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .expect("a private file");
    path
}

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o7777
}

#[test]
fn the_prepared_dirs_under_a_setgid_world_writable_root_pass_the_gates_reader() {
    let base = scratch("setgid");
    let tmp = empty_dir(&base, "tmp").join("tmp");
    let home = empty_dir(&base, "state").join("home");

    let output = prepare(&[&tmp, &home]);

    assert!(output.status.success(), "{output:?}");
    let caller = fs::metadata(&base).expect("base").uid();
    for dir in [&tmp, &home] {
        let meta = fs::symlink_metadata(dir).expect("the prepared directory");
        assert!(meta.is_dir(), "{}", dir.display());
        assert_eq!(meta.uid(), caller, "{}", dir.display());
        assert_eq!(
            mode(dir),
            0o700,
            "exactly 0700, no setgid inherited from the root: {}",
            dir.display()
        );
        let token = write_private(dir, "token");
        assert_eq!(gates_accepts(&token, caller), Ok(()), "{}", dir.display());
    }
}

#[test]
fn a_rerun_succeeds_and_keeps_what_the_dirs_hold() {
    let base = scratch("rerun");
    let home = empty_dir(&base, "state").join("home");
    assert!(prepare(&[&home]).status.success());
    let kept = write_private(&home, "session");
    fs::set_permissions(&home, fs::Permissions::from_mode(0o2777)).expect("loosened");

    let output = prepare(&[&home]);

    assert!(output.status.success(), "{output:?}");
    assert_eq!(mode(&home), 0o700, "a rerun restores the mode");
    assert!(kept.is_file(), "a rerun keeps what the directory holds");
}

#[test]
fn a_link_or_a_file_in_place_of_a_dir_is_refused() {
    let base = scratch("refused");
    let root = empty_dir(&base, "tmp");
    let elsewhere = base.join("elsewhere");
    fs::create_dir(&elsewhere).expect("elsewhere");
    fs::set_permissions(&elsewhere, fs::Permissions::from_mode(0o755)).expect("mode");
    let link = root.join("tmp");
    symlink(&elsewhere, &link).expect("a planted link");

    let output = prepare(&[&link]);

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(mode(&elsewhere), 0o755, "the link is not followed");
    let file = root.join("home");
    fs::write(&file, "").expect("a file");
    assert_eq!(prepare(&[&file]).status.code(), Some(1));
    assert_eq!(
        prepare(&[]).status.code(),
        Some(2),
        "at least one --dir is required"
    );
}
