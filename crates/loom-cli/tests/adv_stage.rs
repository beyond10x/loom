//! Adversarial cases for `b10x-loom agent stage-files` (`story:loom-agent-trial-fixes`): the
//! staged directory under a kubelet-style emptyDir root (setgid, world-writable) satisfies the
//! `b10x-gates` protected reader, and a rerun after the Secret changed mirrors the Secret.

use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};

use b10x_loom_cli::stage::stage;

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("adv_stage")
        .join(name);
    if dir.exists() {
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).ok();
        fs::remove_dir_all(&dir).expect("clean scratch");
    }
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

/// A Secret volume as the kubelet's atomic writer lays it out, version `stamp`: `..data` links
/// to `..<stamp>`, each key to `..data/<key>`. A second call swaps `..data` and the key links
/// to the new version and removes the old one, as an update does.
fn secret_volume(dir: &Path, stamp: &str, keys: &[(&str, &str)]) {
    let stamped_name = format!("..{stamp}");
    let stamped = dir.join(&stamped_name);
    fs::create_dir(&stamped).expect("stamped dir");
    for (key, value) in keys {
        fs::write(stamped.join(key), value).expect("key");
        fs::set_permissions(stamped.join(key), fs::Permissions::from_mode(0o440)).expect("mode");
    }
    let mut old = None;
    if let Ok(target) = fs::read_link(dir.join("..data")) {
        old = Some(dir.join(target));
        fs::remove_file(dir.join("..data")).expect("unlink data");
    }
    symlink(&stamped_name, dir.join("..data")).expect("data link");
    for entry in fs::read_dir(dir).expect("read") {
        let entry = entry.expect("entry");
        let name = entry.file_name().into_string().expect("utf-8");
        if !name.starts_with("..") {
            fs::remove_file(entry.path()).expect("old key link");
        }
    }
    for (key, _) in keys {
        symlink(format!("..data/{key}"), dir.join(key)).expect("key link");
    }
    if let Some(old) = old {
        for entry in fs::read_dir(&old).expect("old") {
            fs::remove_file(entry.expect("entry").path()).expect("old key");
        }
        fs::remove_dir(&old).expect("old dir");
    }
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

/// The emptyDir root as the kubelet leaves it for a Memory medium with `fsGroup`: world-writable,
/// sticky and setgid. The staged `files` directory must still be exactly 0700 (no inherited
/// setgid), each file 0600, and the gates reader accept both.
#[test]
fn adv_staged_files_under_a_setgid_world_writable_root_pass_the_gates_reader() {
    let base = scratch("setgid");
    let from = base.join("secret");
    fs::create_dir(&from).expect("from");
    secret_volume(&from, "v1", &[("policy.json", "{}"), ("bot.pem", "pem")]);
    let empty_dir = base.join("secret-files");
    fs::create_dir(&empty_dir).expect("emptyDir");
    fs::set_permissions(&empty_dir, fs::Permissions::from_mode(0o3777)).expect("root mode");
    let to = empty_dir.join("files");

    assert_eq!(stage(&from, &to), Ok(2));

    let caller = fs::metadata(&base).expect("base").uid();
    assert_eq!(
        fs::symlink_metadata(&to).expect("to").permissions().mode() & 0o7777,
        0o700
    );
    for key in ["policy.json", "bot.pem"] {
        assert_eq!(gates_accepts(&to.join(key), caller), Ok(()), "{key}");
    }
}

/// A Secret updated between two runs of the init container in one pod (the emptyDir outlives a
/// sandbox restart): the key the new version dropped must not stay staged, or the container
/// reads a credential the Secret no longer holds.
#[test]
fn adv_a_key_removed_from_the_secret_is_not_left_staged_by_a_rerun() {
    let base = scratch("rerun");
    let from = base.join("secret");
    fs::create_dir(&from).expect("from");
    secret_volume(
        &from,
        "v1",
        &[("policy.json", "{}"), ("old.pem", "revoked")],
    );
    let to = base.join("files");
    assert_eq!(stage(&from, &to), Ok(2));

    secret_volume(&from, "v2", &[("policy.json", "{\"v\":2}")]);
    assert_eq!(stage(&from, &to), Ok(1));

    assert_eq!(
        fs::read_to_string(to.join("policy.json")).expect("policy"),
        "{\"v\":2}"
    );
    let mut staged: Vec<String> = fs::read_dir(&to)
        .expect("read")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("utf-8")
        })
        .collect();
    staged.sort();
    assert_eq!(
        staged,
        ["policy.json"],
        "the staged directory mirrors the Secret"
    );
}
