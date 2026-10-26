//! What `b10x-loom agent stage-files` does: copy the keys of a mounted Kubernetes Secret volume
//! into a directory the caller owns, so a reader that refuses a file another user owns, or one
//! its group or others may read, accepts them.
//!
//! The kubelet owns a Secret volume's files and links each key to `..data/<key>`. Each entry of
//! `--from` whose name does not start with `..` and that resolves to a regular file inside
//! `--from` is copied, in name order, to `--to/<name>` with mode 0600; `--to` is created when
//! absent and set to mode 0700. A destination file left by an earlier run is replaced, never
//! followed: the copy is a new file, and a file there that no current key names is removed, so a
//! key dropped from the Secret is not left readable. `--to` must be a directory, not a link to
//! one, and a directory inside it is refused.
//!
//! What `b10x-loom agent prepare-dirs` does, [`prepare`]: make each directory one the caller owns
//! with mode exactly 0700 inside an emptyDir whose root the kubelet leaves mode 0777 (setgid with
//! `fsGroup`), which a non-root container cannot change.

use std::collections::BTreeSet;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::Path;

/// The mode of the staged directory.
pub const DIR_MODE: u32 = 0o700;
/// The mode of each staged file.
pub const FILE_MODE: u32 = 0o600;

/// Copies the keys of the Secret volume `from` into `to`, returning how many were staged.
///
/// # Errors
/// The reason, naming the path, when `from` cannot be read, `to` exists and is not a directory
/// or is a link, or a key cannot be read or written.
pub fn stage(from: &Path, to: &Path) -> Result<usize, String> {
    let named = |path: &Path, why: &dyn std::fmt::Display| format!("{}: {why}", path.display());
    let root = fs::canonicalize(from).map_err(|why| named(from, &why))?;
    match fs::symlink_metadata(to) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(named(to, &"is a link, not a directory"));
        }
        Ok(meta) if !meta.is_dir() => return Err(named(to, &"is not a directory")),
        Ok(_) => {}
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => DirBuilder::new()
            .mode(DIR_MODE)
            .create(to)
            .map_err(|why| named(to, &why))?,
        Err(why) => return Err(named(to, &why)),
    }
    fs::set_permissions(to, fs::Permissions::from_mode(DIR_MODE)).map_err(|why| named(to, &why))?;

    let mut names = Vec::new();
    for entry in fs::read_dir(&root).map_err(|why| named(from, &why))? {
        let entry = entry.map_err(|why| named(from, &why))?;
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            return Err(named(&entry.path(), &"its name is not UTF-8"));
        };
        if !name.starts_with("..") {
            names.push(name);
        }
    }
    names.sort();

    let mut staged = 0;
    let mut kept = BTreeSet::new();
    for name in names {
        let source = root.join(&name);
        let resolved = fs::canonicalize(&source).map_err(|why| named(&source, &why))?;
        if !resolved.starts_with(&root) {
            return Err(named(&source, &"resolves outside the Secret volume"));
        }
        if !fs::metadata(&resolved)
            .map_err(|why| named(&source, &why))?
            .is_file()
        {
            continue;
        }
        let bytes = fs::read(&resolved).map_err(|why| named(&source, &why))?;
        let target = to.join(&name);
        match fs::remove_file(&target) {
            Ok(()) => {}
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => {}
            Err(why) => return Err(named(&target, &why)),
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(FILE_MODE)
            .open(&target)
            .map_err(|why| named(&target, &why))?;
        file.set_permissions(fs::Permissions::from_mode(FILE_MODE))
            .map_err(|why| named(&target, &why))?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|why| named(&target, &why))?;
        kept.insert(name);
        staged += 1;
    }

    for entry in fs::read_dir(to).map_err(|why| named(to, &why))? {
        let entry = entry.map_err(|why| named(to, &why))?;
        let stale = entry.path();
        let current = entry
            .file_name()
            .to_str()
            .is_some_and(|name| kept.contains(name));
        if current {
            continue;
        }
        let kind = entry.file_type().map_err(|why| named(&stale, &why))?;
        if kind.is_dir() {
            return Err(named(&stale, &"is a directory no key of the Secret names"));
        }
        fs::remove_file(&stale).map_err(|why| named(&stale, &why))?;
    }
    Ok(staged)
}

/// Makes `dir` a directory this user owns with mode exactly [`DIR_MODE`]: created when absent,
/// kept with what it holds when present, and setgid, sticky and every other bit cleared, since a
/// directory made under a setgid emptyDir root inherits setgid. The mode is set through the
/// opened directory, never through a link in its place, and only the directory's owner may set
/// it, so a directory another user owns is refused.
///
/// # Errors
/// The reason, naming the path, when `dir` is a link or not a directory, cannot be made or
/// opened, or its mode cannot be set to [`DIR_MODE`].
pub fn prepare(dir: &Path) -> Result<(), String> {
    let named = |why: &dyn std::fmt::Display| format!("{}: {why}", dir.display());
    match DirBuilder::new().mode(DIR_MODE).create(dir) {
        Ok(()) => {}
        Err(why) if why.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(why) => return Err(named(&why)),
    }
    let opened = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(dir)
        .map_err(|why| match why.raw_os_error() {
            Some(libc::ELOOP) => named(&"is a link, not a directory"),
            Some(libc::ENOTDIR) => named(&"is not a directory"),
            _ => named(&why),
        })?;
    opened
        .set_permissions(fs::Permissions::from_mode(DIR_MODE))
        .map_err(|why| named(&why))?;
    let mode = opened
        .metadata()
        .map_err(|why| named(&why))?
        .permissions()
        .mode()
        & 0o7777;
    if mode != DIR_MODE {
        return Err(named(&format_args!(
            "its mode is {mode:o}, not {DIR_MODE:o}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{MetadataExt as _, symlink};

    use super::*;

    /// A Secret volume as the kubelet lays it out: `..data` linked to a timestamped directory,
    /// each key linked to `..data/<key>`.
    fn secret_volume(dir: &Path, keys: &[(&str, &str)]) {
        let stamped = dir.join("..2026_10_26_00_00_00.000000000");
        fs::create_dir(&stamped).expect("stamped dir");
        for (key, value) in keys {
            fs::write(stamped.join(key), value).expect("key");
            fs::set_permissions(stamped.join(key), fs::Permissions::from_mode(0o440))
                .expect("mode");
        }
        symlink("..2026_10_26_00_00_00.000000000", dir.join("..data")).expect("data link");
        for (key, _) in keys {
            symlink(format!("..data/{key}"), dir.join(key)).expect("key link");
        }
    }

    #[test]
    fn each_key_is_a_private_file_in_a_private_directory() {
        let scratch = tempfile::tempdir().expect("scratch");
        let from = scratch.path().join("secret");
        fs::create_dir(&from).expect("from");
        secret_volume(&from, &[("policy.json", "{}"), ("key.pem", "pem")]);
        let to = scratch.path().join("staged");

        assert_eq!(stage(&from, &to), Ok(2));

        let dir = fs::symlink_metadata(&to).expect("to");
        assert!(dir.is_dir());
        assert_eq!(dir.permissions().mode() & 0o777, DIR_MODE);
        let mut names: Vec<String> = fs::read_dir(&to)
            .expect("read")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .into_string()
                    .expect("utf-8")
            })
            .collect();
        names.sort();
        assert_eq!(names, ["key.pem", "policy.json"]);
        for (name, value) in [("policy.json", "{}"), ("key.pem", "pem")] {
            let meta = fs::symlink_metadata(to.join(name)).expect("file");
            assert!(meta.is_file(), "{name} is a regular file, not a link");
            assert_eq!(meta.permissions().mode() & 0o777, FILE_MODE);
            assert_eq!(meta.uid(), dir.uid());
            assert_eq!(fs::read_to_string(to.join(name)).expect("read"), value);
        }
    }

    #[test]
    fn a_second_run_replaces_the_files_and_never_follows_a_planted_link() {
        let scratch = tempfile::tempdir().expect("scratch");
        let from = scratch.path().join("secret");
        fs::create_dir(&from).expect("from");
        secret_volume(&from, &[("token", "fresh")]);
        let to = scratch.path().join("staged");
        let elsewhere = scratch.path().join("elsewhere");
        fs::write(&elsewhere, "untouched").expect("elsewhere");
        fs::create_dir(&to).expect("to");
        fs::set_permissions(&to, fs::Permissions::from_mode(0o755)).expect("mode");
        symlink(&elsewhere, to.join("token")).expect("planted link");

        assert_eq!(stage(&from, &to), Ok(1));

        assert_eq!(fs::read_to_string(&elsewhere).expect("read"), "untouched");
        let meta = fs::symlink_metadata(to.join("token")).expect("file");
        assert!(meta.is_file());
        assert_eq!(fs::read_to_string(to.join("token")).expect("read"), "fresh");
        assert_eq!(
            fs::symlink_metadata(&to).expect("to").permissions().mode() & 0o777,
            DIR_MODE
        );
    }

    #[test]
    fn a_destination_that_is_a_file_or_a_link_is_refused() {
        let scratch = tempfile::tempdir().expect("scratch");
        let from = scratch.path().join("secret");
        fs::create_dir(&from).expect("from");
        secret_volume(&from, &[("token", "value")]);
        let file = scratch.path().join("file");
        fs::write(&file, "").expect("file");
        assert!(stage(&from, &file).is_err());
        let real = scratch.path().join("real");
        fs::create_dir(&real).expect("real");
        let link = scratch.path().join("link");
        symlink(&real, &link).expect("link");
        assert!(stage(&from, &link).is_err());
        assert_eq!(fs::read_dir(&real).expect("read").count(), 0);
    }

    #[test]
    fn a_key_linked_outside_the_volume_is_refused() {
        let scratch = tempfile::tempdir().expect("scratch");
        let from = scratch.path().join("secret");
        fs::create_dir(&from).expect("from");
        let outside = scratch.path().join("outside");
        fs::write(&outside, "not the secret").expect("outside");
        symlink(&outside, from.join("token")).expect("link");
        let to = scratch.path().join("staged");
        assert!(stage(&from, &to).is_err());
        assert!(!to.join("token").exists());
    }
}
