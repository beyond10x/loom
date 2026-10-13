//! The write discipline every store the slice keeps in a state directory follows
//! ([`crate::resume::ResumeContextStore`], [`crate::grant::GrantStore`],
//! [`crate::handoff::MergeHandoffStore`]), and the governor's file stores follow too
//! (`loom_governor::FileCaseStore`, `loom_governor::FileSuspensionStore`): every operation holds
//! an exclusive lock on `<dir>/.lock`, the lock those stores take, and a write goes to a fresh
//! temporary file in the same directory (`.tmp-…`, which those stores remove when a dead process
//! left one), is synced, and is renamed over its target, so a reader sees no document or the whole
//! one.
//!
//! A read takes only a regular file as a record ([`read_text`]): a symbolic link or any other entry
//! where a record would be is refused, never followed.
//!
//! Opening proves the directory takes a write: holding the lock, a temporary file is created,
//! written, synced and removed. A lock file an earlier open left is no such proof, so a directory
//! that has since become read-only is refused on open.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::resume::ResumeStoreError;

/// The lock file the governor's file stores take; every store here takes the same one.
pub(crate) const LOCK: &str = ".lock";

/// The prefix of a temporary file; the governor's file stores remove the ones a dead process left.
pub(crate) const TEMPORARY: &str = ".tmp-";

/// The longest id, in bytes, a file is named for: a prefix, 200 hex digits and `.json` stay under
/// the 255 bytes a file name may have.
pub(crate) const LONGEST_ID: usize = 100;

/// Temporary file names this process has used, so two writes never share one.
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

pub(crate) fn failed(path: &Path, problem: impl std::fmt::Display) -> ResumeStoreError {
    ResumeStoreError {
        path: path.to_owned(),
        problem: problem.to_string(),
    }
}

/// One state directory, opened and proven to take a write.
#[derive(Debug)]
pub(crate) struct StateDir {
    dir: PathBuf,
    /// What the store's temporary files are named after, so two stores' never collide.
    label: &'static str,
}

impl StateDir {
    /// `dir`, created with its lock file when it does not exist yet, and proven to take a write.
    pub(crate) fn open(dir: PathBuf, label: &'static str) -> Result<Self, ResumeStoreError> {
        fs::create_dir_all(&dir).map_err(|error| failed(&dir, error))?;
        let lock = dir.join(LOCK);
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock)
            .map_err(|error| failed(&lock, error))?;
        let opened = Self { dir, label };
        opened.locked(|| opened.probe())??;
        Ok(opened)
    }

    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    /// The file `<prefix><hex of id>.json`, named so no id names a file outside the directory.
    pub(crate) fn file(
        &self,
        prefix: &str,
        what: &str,
        id: &str,
    ) -> Result<PathBuf, ResumeStoreError> {
        let bytes = id.as_bytes();
        if bytes.len() > LONGEST_ID {
            return Err(failed(
                &self.dir,
                format!(
                    "{what} of {} bytes is longer than the {LONGEST_ID} this store keeps",
                    bytes.len()
                ),
            ));
        }
        let mut name = String::from(prefix);
        for byte in bytes {
            name.push_str(&format!("{byte:02x}"));
        }
        name.push_str(".json");
        Ok(self.dir.join(name))
    }

    /// Runs `operation` holding the directory's lock. The outer error is the lock's.
    pub(crate) fn locked<T>(&self, operation: impl FnOnce() -> T) -> Result<T, ResumeStoreError> {
        let path = self.dir.join(LOCK);
        let lock = OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|error| failed(&path, error))?;
        lock.lock().map_err(|error| failed(&path, error))?;
        let result = operation();
        let _ = lock.unlock();
        Ok(result)
    }

    /// Writes `text` to a new temporary file in the directory, syncs it, and renames it over
    /// `path`; on failure the temporary file is removed and `path` is untouched. Only called
    /// holding the lock.
    pub(crate) fn replace(&self, path: &Path, text: &str) -> Result<(), ResumeStoreError> {
        let (temporary, mut file) = self.temporary()?;
        let written = (|| -> io::Result<()> {
            file.write_all(text.as_bytes())?;
            file.sync_all()
        })()
        .map_err(|error| failed(&temporary, error))
        .and_then(|()| fs::rename(&temporary, path).map_err(|error| failed(path, error)));
        if written.is_err() {
            let _ = fs::remove_file(&temporary);
            return written;
        }
        self.sync();
        Ok(())
    }

    /// Proves the directory takes a write as [`Self::replace`] makes one. Only called holding the
    /// lock. A probe a crash interrupted is a temporary file the governor's stores remove when
    /// they open.
    fn probe(&self) -> Result<(), ResumeStoreError> {
        let (temporary, mut file) = self.temporary()?;
        let written = file
            .write_all(b"probe\n")
            .and_then(|()| file.sync_all())
            .map_err(|error| failed(&temporary, error));
        drop(file);
        let removed = fs::remove_file(&temporary).map_err(|error| failed(&temporary, error));
        written.and(removed)
    }

    /// A new temporary file in the directory, created for writing. Only called holding the lock.
    fn temporary(&self) -> Result<(PathBuf, File), ResumeStoreError> {
        loop {
            let temporary = self.dir.join(format!(
                "{TEMPORARY}{}-{}-{}",
                std::process::id(),
                self.label,
                NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => return Ok((temporary, file)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(failed(&temporary, error)),
            }
        }
    }

    /// Syncs the directory, so a rename or a removal in it is durable. A failure only weakens how
    /// soon, and is not reported.
    pub(crate) fn sync(&self) {
        if let Ok(dir) = File::open(&self.dir) {
            let _ = dir.sync_all();
        }
    }
}

/// The text of the file `path`, or `None` when there is no such file. Only called holding the
/// lock.
///
/// Only a regular file in the directory is a record: a symlink (to a record kept elsewhere, or to
/// nothing), a directory or any other entry at `path` is refused, never followed, so no store reads
/// a record its own writes did not put there. The file opened is checked to be the entry examined.
pub(crate) fn read_text(path: &Path) -> Result<Option<String>, ResumeStoreError> {
    use std::io::Read as _;
    use std::os::unix::fs::MetadataExt as _;

    let entry = match fs::symlink_metadata(path) {
        Ok(entry) => entry,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(failed(path, error)),
    };
    if !entry.file_type().is_file() {
        return Err(failed(path, not_a_record(&entry.file_type())));
    }
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(failed(path, error)),
    };
    let opened = file.metadata().map_err(|error| failed(path, error))?;
    if !opened.file_type().is_file() || (opened.dev(), opened.ino()) != (entry.dev(), entry.ino()) {
        return Err(failed(
            path,
            "changed while it was read: the file opened is not the entry examined",
        ));
    }
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|error| failed(path, error))?;
    Ok(Some(text))
}

/// Why an entry that is not a regular file is not read as a record.
fn not_a_record(kind: &fs::FileType) -> &'static str {
    if kind.is_symlink() {
        "is a symbolic link, not a record this store wrote; it is not followed"
    } else if kind.is_dir() {
        "is a directory, not a record this store wrote"
    } else {
        "is not a regular file, not a record this store wrote"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every store here reads its records through `read_text` (`GrantStore`, `ResumeContextStore`,
    /// `MergeHandoffStore`), so a record is only ever a regular file in the directory: a symlink,
    /// to a record kept elsewhere or to nothing, and a directory are refused, never read as a
    /// record and never as no record.
    #[test]
    fn only_a_regular_file_is_read_as_a_record() {
        let dir = tempfile::Builder::new()
            .prefix("read-text-")
            .tempdir()
            .unwrap();
        let record = dir.path().join("record.json");
        std::fs::write(&record, "{}").unwrap();
        assert_eq!(read_text(&record).unwrap().as_deref(), Some("{}"));
        assert_eq!(read_text(&dir.path().join("absent.json")).unwrap(), None);

        let linked = dir.path().join("linked.json");
        std::os::unix::fs::symlink(&record, &linked).unwrap();
        let refused = read_text(&linked).unwrap_err();
        assert!(refused.problem.contains("symbolic link"), "{refused:?}");

        let dangling = dir.path().join("dangling.json");
        std::os::unix::fs::symlink(dir.path().join("nothing.json"), &dangling).unwrap();
        assert!(
            read_text(&dangling).is_err(),
            "a dangling link is not no record"
        );

        let directory = dir.path().join("directory.json");
        std::fs::create_dir(&directory).unwrap();
        assert!(read_text(&directory).is_err());
    }
}
