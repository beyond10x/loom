//! One writer per store: an exclusive lock on [`LOCK_FILE`] held for the whole of a command, and
//! the holder's process id in [`HOLDER_FILE`] while it is held.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use loom::tree::TreeRefusalReason;

use crate::{Refused, unreadable};

/// The file of the store a writer holds an exclusive lock on. It is created by `tree open` and
/// never written.
pub const LOCK_FILE: &str = "tree.lock";

/// The file that names the holder's process id while a writer holds the store, removed when it
/// lets go. The lock decides who holds the store; this file only names the holder.
pub const HOLDER_FILE: &str = "tree.holder";

/// The most bytes of [`HOLDER_FILE`] a refused writer reads.
const MAX_HOLDER_BYTES: u64 = 32;

/// A store held for one writer until this is dropped.
#[derive(Debug)]
pub struct StoreLock {
    /// The open lock file; its lock is released when it is closed.
    _file: File,
    holder: PathBuf,
}

impl StoreLock {
    /// Holds the tree store `store` for this process, which must already hold [`LOCK_FILE`].
    ///
    /// # Errors
    /// `StoreInUse` naming the holder's process id when another writer holds the store: nothing
    /// was written. `NotATree` when the store holds no lock file. `StoreUnreadable` when the lock
    /// file cannot be opened or locked, or the holder file cannot be written.
    pub fn acquire(store: &Path) -> Result<Self, Refused> {
        let path = store.join(LOCK_FILE);
        match fs::symlink_metadata(&path) {
            Ok(entry) if entry.file_type().is_file() => {}
            Ok(_) => {
                return Err(unreadable(
                    path.display(),
                    "is not a regular file; it is not followed",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(Refused::new(
                    TreeRefusalReason::NotATree,
                    format!("{} holds no tree: it has no {LOCK_FILE}", store.display()),
                ));
            }
            Err(error) => return Err(unreadable(path.display(), error)),
        }
        Self::take(store, &path)
    }

    /// Creates [`LOCK_FILE`] in `store` when it is absent and holds the store: what `tree open`
    /// takes once it has checked everything it reads.
    ///
    /// # Errors
    /// As [`StoreLock::acquire`].
    pub(crate) fn create(store: &Path) -> Result<Self, Refused> {
        let path = store.join(LOCK_FILE);
        if fs::symlink_metadata(&path).is_err() {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => {}
                // Another command made it first; the lock decides between the two.
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(unreadable(path.display(), error)),
            }
        }
        Self::acquire(store)
    }

    fn take(store: &Path, path: &Path) -> Result<Self, Refused> {
        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(|error| unreadable(path.display(), error))?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                let holder = holder(store);
                let mut refused = Refused::new(
                    TreeRefusalReason::StoreInUse,
                    match holder {
                        Some(pid) => format!(
                            "{} is held by process {pid}, another tree command",
                            store.display()
                        ),
                        None => format!(
                            "{} is held by another tree command, which has not named its process yet",
                            store.display()
                        ),
                    },
                );
                refused.0.holder = holder;
                return Err(refused);
            }
            Err(TryLockError::Error(error)) => return Err(unreadable(path.display(), error)),
        }
        let holder = store.join(HOLDER_FILE);
        // Written to a fresh file and renamed over the name, so a link planted at the name is
        // replaced, never followed.
        let temporary = store.join(format!(".{HOLDER_FILE}-{}", std::process::id()));
        let named = (|| {
            let _ = fs::remove_file(&temporary);
            let mut out = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            writeln!(out, "{}", std::process::id())?;
            loom_governor::durable::sync_file(&out)?;
            fs::rename(&temporary, &holder)
        })();
        named.map_err(|error| {
            let _ = fs::remove_file(&temporary);
            unreadable(holder.display(), error)
        })?;
        Ok(Self {
            _file: file,
            holder,
        })
    }
}

impl Drop for StoreLock {
    fn drop(&mut self) {
        // Removed before the lock is released, so a later writer never reads this process's id.
        let _ = fs::remove_file(&self.holder);
    }
}

/// The process id [`HOLDER_FILE`] of `store` names, when it names one.
fn holder(store: &Path) -> Option<i64> {
    use std::io::Read as _;
    let path = store.join(HOLDER_FILE);
    let entry = fs::symlink_metadata(&path).ok()?;
    if !entry.file_type().is_file() {
        return None;
    }
    let mut text = String::new();
    File::open(&path)
        .ok()?
        .take(MAX_HOLDER_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    text.trim().parse::<u32>().ok().map(i64::from)
}
