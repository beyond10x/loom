//! The directory discipline the governor's file stores share: one exclusive lock on `<dir>/.lock`
//! held for each operation, and every write made to a fresh temporary file in the same directory,
//! synced, and renamed over its target.
//!
//! [`FileCaseStore`](crate::FileCaseStore), [`FileSuspensionStore`](crate::FileSuspensionStore)
//! and [`FileWorkflowRunStore`](crate::FileWorkflowRunStore) all keep their files through a
//! [`LockedDir`], so any of them opened over one directory, in one process or in two, take the same
//! lock and never interleave a read and a write; their file names never meet (`case-…`,
//! `observations.json`, `suspension-…`, `workflow-run-…`). Temporary files are named
//! `.tmp-<process id>-<counter>` from one counter for the whole process, and a store that opens
//! removes the ones a dead process left, holding the lock, so it never removes one a live write
//! is using.
//!
//! A store that opens also proves, holding the lock, that the directory takes a write: it creates
//! a fresh temporary file, writes it, syncs it and removes it. A lock file an earlier open left is
//! no such proof, so a directory that has since become read-only is refused when the store opens,
//! before its owner starts anything that would end at the first write.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::FileStoreError;

const LOCK: &str = ".lock";
const TEMPORARY: &str = ".tmp-";

/// How many temporary names in use one write skips before it reports the last as its error.
const TEMPORARY_ATTEMPTS: u32 = 1024;

/// Temporary file names this process has used, so two writes in one process never share one.
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// The `FileStoreError` for `path`.
pub(crate) fn failed(path: &Path, problem: impl std::fmt::Display) -> FileStoreError {
    FileStoreError {
        path: path.to_owned(),
        problem: problem.to_string(),
    }
}

/// A directory a store keeps its files in, with its lock.
#[derive(Debug)]
pub(crate) struct LockedDir {
    dir: PathBuf,
}

impl LockedDir {
    /// The directory `dir`, created with its lock file when it does not exist yet, without the
    /// temporary files an earlier life left behind, and proven to take a write.
    pub(crate) fn open(dir: PathBuf) -> Result<Self, FileStoreError> {
        fs::create_dir_all(&dir).map_err(|error| failed(&dir, error))?;
        let lock = dir.join(LOCK);
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock)
            .map_err(|error| failed(&lock, error))?;
        let opened = Self { dir };
        opened.locked(|| {
            opened.remove_stale_temporaries()?;
            opened.probe()
        })??;
        Ok(opened)
    }

    /// Proves the directory takes a write as a store makes one: a fresh temporary file is
    /// created, written, synced and removed. Only called holding the lock. A probe a crash
    /// interrupted is a temporary file like any other, removed by the next open.
    fn probe(&self) -> Result<(), FileStoreError> {
        let (temporary, mut file) = self.temporary()?;
        let written = file
            .write_all(b"probe\n")
            .and_then(|()| file.sync_all())
            .map_err(|error| failed(&temporary, error));
        drop(file);
        let removed = fs::remove_file(&temporary).map_err(|error| failed(&temporary, error));
        written.and(removed)
    }

    /// The directory.
    pub(crate) fn path(&self) -> &Path {
        &self.dir
    }

    /// Removes every temporary file in the directory. Only called holding the lock, when no write
    /// of any store over the directory is in progress, so each one was left by a write that died
    /// before its rename and holds nothing a store's state depends on.
    fn remove_stale_temporaries(&self) -> Result<(), FileStoreError> {
        let entries = fs::read_dir(&self.dir).map_err(|error| failed(&self.dir, error))?;
        for entry in entries {
            let entry = entry.map_err(|error| failed(&self.dir, error))?;
            if entry.file_name().to_string_lossy().starts_with(TEMPORARY) {
                let path = entry.path();
                fs::remove_file(&path).map_err(|error| failed(&path, error))?;
            }
        }
        Ok(())
    }

    /// Runs `operation` holding the directory's lock. The outer error is the lock's: `operation`
    /// did not run.
    pub(crate) fn locked<T>(&self, operation: impl FnOnce() -> T) -> Result<T, FileStoreError> {
        let path = self.dir.join(LOCK);
        let lock = OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|error| failed(&path, error))?;
        lock.lock().map_err(|error| failed(&path, error))?;
        let result = operation();
        // Closing the file releases the lock; an unlock error changes nothing about the result.
        let _ = lock.unlock();
        Ok(result)
    }

    /// Writes `text` to a new temporary file in the directory, syncs it, and renames it over
    /// `path`. On any failure the temporary file is removed and `path` is untouched. Only called
    /// holding the lock.
    pub(crate) fn replace(&self, path: &Path, text: &str) -> Result<(), FileStoreError> {
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
        // The rename has happened, so the new document is the state; a failed directory sync only
        // weakens how soon it is durable and is not reported as a failed write.
        self.sync();
        Ok(())
    }

    /// A new temporary file in the directory, created for writing. Only called holding the lock.
    fn temporary(&self) -> Result<(PathBuf, File), FileStoreError> {
        // A name in use, left by an earlier life with the same process id, is skipped for the
        // next one rather than reported: it says nothing about this write.
        let mut attempts = 0;
        loop {
            let temporary = self.dir.join(format!(
                "{TEMPORARY}{}-{}",
                std::process::id(),
                NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => return Ok((temporary, file)),
                Err(error)
                    if error.kind() == io::ErrorKind::AlreadyExists
                        && attempts < TEMPORARY_ATTEMPTS =>
                {
                    attempts += 1;
                }
                Err(error) => return Err(failed(&temporary, error)),
            }
        }
    }

    /// Removes `path`. Only called holding the lock.
    pub(crate) fn remove(&self, path: &Path) -> Result<(), FileStoreError> {
        fs::remove_file(path).map_err(|error| failed(path, error))?;
        // As after a rename: the file is gone, and a failed directory sync is not a failed remove.
        self.sync();
        Ok(())
    }

    fn sync(&self) {
        if let Ok(dir) = File::open(&self.dir) {
            let _ = dir.sync_all();
        }
    }
}

/// The file's text, or `None` when there is no such file.
///
/// Only a regular file is a record: a symlink (to a record kept elsewhere, or to nothing), a
/// directory or any other entry at `path` is refused, never followed. The file opened is checked
/// to be the entry examined.
pub(crate) fn read_if_present(path: &Path) -> Result<Option<String>, FileStoreError> {
    use std::io::Read as _;
    use std::os::unix::fs::MetadataExt as _;

    let entry = match fs::symlink_metadata(path) {
        Ok(entry) => entry,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(failed(path, error)),
    };
    if !entry.file_type().is_file() {
        return Err(failed(
            path,
            "is not a regular file, not a record this store wrote; it is not followed",
        ));
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
