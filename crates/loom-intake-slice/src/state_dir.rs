//! The write discipline every store the slice keeps in a state directory follows
//! ([`crate::resume::ResumeContextStore`], [`crate::grant::GrantStore`],
//! [`crate::handoff::MergeHandoffStore`]), and the governor's file stores follow too
//! (`loom_governor::FileCaseStore`, `loom_governor::FileSuspensionStore`): every operation holds
//! an exclusive lock on `<dir>/.lock`, the lock those stores take, and a write goes to a fresh
//! temporary file in the same directory (`.tmp-…`, which those stores remove when a dead process
//! left one), is synced, and is renamed over its target, so a reader sees no document or the whole
//! one.
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
pub(crate) fn read_text(path: &Path) -> Result<Option<String>, ResumeStoreError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(failed(path, error)),
    }
}
