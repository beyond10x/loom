//! What `b10x-loom agent stage-files` does: copy the keys of a mounted Kubernetes Secret volume
//! into a directory the caller owns, so a reader that refuses a file another user owns, or one
//! its group or others may read, accepts them.
//!
//! The kubelet owns a Secret volume's files and links each key to `..data/<key>`. [`stage`] first
//! reads everything it decides on and writes nothing: whether `--to` is absent, a directory, a
//! link or something else; the entries of `--from` whose names do not start with `..`, whether
//! each name is UTF-8 and whether each resolves inside `--from`; and whether `--to` holds a
//! directory no key names. `loom.resource.StageFiles` (`ess/domains/resource.yaml`), decided by
//! `b10x_loom_resource::stage::stage_files`, answers from those facts: the first refusal in
//! declaration order, or the modes to stage with. Only then does it write: `--to` is created when
//! absent and set to the directory mode, 0700; each key that resolves to a regular file is copied,
//! in name order, to `--to/<name>` with the file mode, 0600; a key that is not a regular file is
//! skipped. A destination file left by an earlier run is replaced, never followed: the copy is a
//! new file, and a file there that no current key names is removed, so a key dropped from the
//! Secret is not left readable. A refused stage writes nothing.
//!
//! What `b10x-loom agent prepare-dirs` does, [`prepare`]: make each directory one the caller owns
//! with mode exactly 0700 inside an emptyDir whose root the kubelet leaves mode 0777 (setgid with
//! `fsGroup`), which a non-root container cannot change.

use std::collections::BTreeSet;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use b10x_loom_resource::model::{StageDestination, StageFiles, StageFilesOutcome};
use b10x_loom_resource::stage::stage_files;
pub use b10x_loom_resource::stage::{DIR_MODE, FILE_MODE};

/// What [`stage`] read before deciding: the decision's facts, the path each refusal would name,
/// and the keys to copy.
struct Read {
    /// The decision's input.
    facts: StageFiles,
    /// The entry of `from` whose name is not UTF-8, the first in directory order.
    not_utf8: Option<PathBuf>,
    /// The key that resolves outside `from`, the first in name order.
    outside: Option<PathBuf>,
    /// The directory in `to` no key names, the first in directory order.
    stale: Option<PathBuf>,
    /// Each key that resolves to a regular file inside `from`, in name order, with that file.
    files: Vec<(String, PathBuf)>,
}

/// Reads what [`stage`] decides on, writing nothing.
fn read(from: &Path, to: &Path) -> Result<Read, String> {
    let named = |path: &Path, why: &dyn std::fmt::Display| format!("{}: {why}", path.display());
    let root = fs::canonicalize(from).map_err(|why| named(from, &why))?;
    let destination = match fs::symlink_metadata(to) {
        Ok(meta) if meta.file_type().is_symlink() => StageDestination::Link,
        Ok(meta) if meta.is_dir() => StageDestination::Directory,
        Ok(_) => StageDestination::Other,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => StageDestination::Absent,
        Err(why) => return Err(named(to, &why)),
    };

    let mut not_utf8 = None;
    let mut names = Vec::new();
    for entry in fs::read_dir(&root).map_err(|why| named(from, &why))? {
        let entry = entry.map_err(|why| named(from, &why))?;
        match entry.file_name().to_str() {
            Some(name) if name.starts_with("..") => {}
            Some(name) => names.push(name.to_owned()),
            None => {
                not_utf8.get_or_insert_with(|| entry.path());
            }
        }
    }
    names.sort();

    let mut outside = None;
    let mut files = Vec::new();
    for name in names {
        let source = root.join(&name);
        let resolved = fs::canonicalize(&source).map_err(|why| named(&source, &why))?;
        if !resolved.starts_with(&root) {
            outside.get_or_insert(source);
            continue;
        }
        if fs::metadata(&resolved)
            .map_err(|why| named(&source, &why))?
            .is_file()
        {
            files.push((name, resolved));
        }
    }

    let mut stale = None;
    if destination == StageDestination::Directory {
        let kept: BTreeSet<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
        for entry in fs::read_dir(to).map_err(|why| named(to, &why))? {
            let entry = entry.map_err(|why| named(to, &why))?;
            let current = entry
                .file_name()
                .to_str()
                .is_some_and(|name| kept.contains(name));
            if !current
                && entry
                    .file_type()
                    .map_err(|why| named(&entry.path(), &why))?
                    .is_dir()
            {
                stale.get_or_insert_with(|| entry.path());
            }
        }
    }

    Ok(Read {
        facts: StageFiles {
            destination,
            names_utf8: not_utf8.is_none(),
            keys_inside: outside.is_none(),
            stale_directory: stale.is_some(),
        },
        not_utf8,
        outside,
        stale,
        files,
    })
}

/// Copies the keys of the Secret volume `from` into `to`, returning how many were staged.
///
/// # Errors
/// The reason, naming the path, when `from` cannot be read, `loom.resource.StageFiles` refuses
/// what was read (nothing is written then), or a key cannot be read or written.
pub fn stage(from: &Path, to: &Path) -> Result<usize, String> {
    let named = |path: &Path, why: &dyn std::fmt::Display| format!("{}: {why}", path.display());
    let read = read(from, to)?;
    let named_or_to =
        |path: Option<&PathBuf>, why: &str| named(path.map_or(to, PathBuf::as_path), &why);
    let modes = match stage_files(&read.facts) {
        StageFilesOutcome::DestinationIsALink { .. } => {
            return Err(named(to, &"is a link, not a directory"));
        }
        StageFilesOutcome::DestinationNotADirectory { .. } => {
            return Err(named(to, &"is not a directory"));
        }
        StageFilesOutcome::NameNotUtf8 { .. } => {
            return Err(named_or_to(read.not_utf8.as_ref(), "its name is not UTF-8"));
        }
        StageFilesOutcome::KeyOutsideVolume { .. } => {
            return Err(named_or_to(
                read.outside.as_ref(),
                "resolves outside the Secret volume",
            ));
        }
        StageFilesOutcome::StaleDirectory { .. } => {
            return Err(named_or_to(
                read.stale.as_ref(),
                "is a directory no key of the Secret names",
            ));
        }
        StageFilesOutcome::Staged { response } => response,
    };
    let mode = |value: i64| {
        u32::try_from(value).map_err(|_| named(to, &format_args!("mode {value} is not a mode")))
    };
    let (dir_mode, file_mode) = (mode(modes.directory_mode)?, mode(modes.file_mode)?);

    if read.facts.destination == StageDestination::Absent {
        DirBuilder::new()
            .mode(dir_mode)
            .create(to)
            .map_err(|why| named(to, &why))?;
    }
    fs::set_permissions(to, fs::Permissions::from_mode(dir_mode)).map_err(|why| named(to, &why))?;

    let mut kept = BTreeSet::new();
    for (name, resolved) in &read.files {
        let source = from.join(name);
        let bytes = fs::read(resolved).map_err(|why| named(&source, &why))?;
        let target = to.join(name);
        match fs::remove_file(&target) {
            Ok(()) => {}
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => {}
            Err(why) => return Err(named(&target, &why)),
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(file_mode)
            .open(&target)
            .map_err(|why| named(&target, &why))?;
        file.set_permissions(fs::Permissions::from_mode(file_mode))
            .map_err(|why| named(&target, &why))?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|why| named(&target, &why))?;
        kept.insert(name.as_str());
    }

    for entry in fs::read_dir(to).map_err(|why| named(to, &why))? {
        let entry = entry.map_err(|why| named(to, &why))?;
        let current = entry
            .file_name()
            .to_str()
            .is_some_and(|name| kept.contains(name));
        if !current {
            let stale = entry.path();
            fs::remove_file(&stale).map_err(|why| named(&stale, &why))?;
        }
    }
    Ok(kept.len())
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

    /// `loom.resource.StageFiles` decides from what was read before anything is written: a key
    /// linked outside the volume refuses the stage before the destination is created, and a
    /// directory no key names refuses it before any key is copied or stale file removed.
    #[test]
    fn a_refused_stage_writes_nothing() {
        let scratch = tempfile::tempdir().expect("scratch");
        let from = scratch.path().join("secret");
        fs::create_dir(&from).expect("from");
        secret_volume(&from, &[("a-token", "value")]);
        let outside = scratch.path().join("outside");
        fs::write(&outside, "not the secret").expect("outside");
        symlink(&outside, from.join("z-token")).expect("link");
        let to = scratch.path().join("staged");
        let refused = stage(&from, &to).expect_err("a key outside the volume");
        assert!(
            refused.ends_with("resolves outside the Secret volume"),
            "{refused}"
        );
        assert!(
            !to.exists(),
            "the destination was created before the refusal"
        );

        let from = scratch.path().join("secret-2");
        fs::create_dir(&from).expect("from");
        secret_volume(&from, &[("token", "fresh")]);
        let to = scratch.path().join("staged-2");
        fs::create_dir(&to).expect("to");
        fs::set_permissions(&to, fs::Permissions::from_mode(0o755)).expect("mode");
        fs::write(to.join("dropped"), "old").expect("stale file");
        fs::create_dir(to.join("left-over")).expect("stale directory");
        let refused = stage(&from, &to).expect_err("a directory no key names");
        assert!(
            refused.ends_with("is a directory no key of the Secret names"),
            "{refused}"
        );
        assert!(
            !to.join("token").exists(),
            "a key was copied before the refusal"
        );
        assert_eq!(fs::read_to_string(to.join("dropped")).expect("kept"), "old");
        assert_eq!(
            fs::symlink_metadata(&to).expect("to").permissions().mode() & 0o777,
            0o755,
            "the destination's mode was set before the refusal"
        );
    }

    /// The refusal names the path the decision refused for: the destination, or the key.
    #[test]
    fn each_refusal_names_its_path() {
        let scratch = tempfile::tempdir().expect("scratch");
        let from = scratch.path().join("secret");
        fs::create_dir(&from).expect("from");
        secret_volume(&from, &[("token", "value")]);
        let file = scratch.path().join("file");
        fs::write(&file, "").expect("file");
        assert_eq!(
            stage(&from, &file),
            Err(format!("{}: is not a directory", file.display()))
        );
        let link = scratch.path().join("link");
        symlink(scratch.path(), &link).expect("link");
        assert_eq!(
            stage(&from, &link),
            Err(format!("{}: is a link, not a directory", link.display()))
        );
    }
}
