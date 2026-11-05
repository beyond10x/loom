//! The plugin's state directory: `state.json` and `record.jsonl`.
//!
//! [`StateDir::open`] refuses a directory inside one of the configuration's `workspace_roots` or
//! `checkouts`, and only then creates it. The check reads paths only and runs no git command
//! (`host_git_hardening`): each path is made absolute and walked component by component, every
//! prefix that exists resolved through the filesystem (symbolic links included) and a `..` taking
//! the parent of what is resolved so far. A directory is inside a root when its resolved path
//! starts with the root's, component-wise; the root itself counts.
//!
//! `state.json` ([`PluginState`]) is written to a new temporary file beside it, synced, and renamed
//! over it, so a reader sees the old state or the new one and never a part. `record.jsonl` gains one
//! line per handled item, appended; under the lock, bytes after its last newline, the torn trace of
//! an append that did not finish, are dropped. The record is the source of truth for handled items: every
//! item it holds a line for counts as handled when the state is read, so a line appended before a
//! save that failed is never followed by a second line for the same item.
//!
//! One host holds a state directory at a time: [`StateDir::open`] takes an exclusive lock on the
//! record (`File::try_lock`), held until the [`StateDir`] and every [`HeldRecord`] of it are
//! dropped, and refuses a directory another host holds with [`PluginError::StateInUse`], naming
//! it, once the lock stayed held for [`LOCK_WAIT`].
//!
//! A host that answers beside its poll (`b10x-loom agent serve` with `expose.a2a`) records the
//! answers in the same record, from another thread: [`HeldRecord::of`] finds the record this
//! process holds for a directory, and appends through it are serialized with the holder's own,
//! under the holder's lock, which the handle keeps held while it lives. No second lock is taken,
//! so a second host, in this process or another, is still refused.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{ErrorKind, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use loom::plugin::{ItemId, PluginConfig, PluginState, RecordLine};

use crate::PluginError;
use crate::codec::{decode_record_line, decode_state, encode_record_line, encode_state};

/// The state file, in the state directory.
pub const STATE_FILE: &str = "state.json";
/// The record, in the state directory.
pub const RECORD_FILE: &str = "record.jsonl";

/// How long [`StateDir::open`] waits for a held lock before it refuses the directory.
pub const LOCK_WAIT: Duration = Duration::from_secs(2);

/// The records this process holds, by their directory as the file system resolves it.
static HELD: Mutex<Vec<(PathBuf, Weak<Held>)>> = Mutex::new(Vec::new());

fn held_records() -> MutexGuard<'static, Vec<(PathBuf, Weak<Held>)>> {
    HELD.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A record this process holds: the file, opened for appending and exclusively locked until the
/// last handle on it is dropped, and the lock its appends and reads take in this process.
#[derive(Debug)]
struct Held {
    root: PathBuf,
    file: File,
    appending: Mutex<()>,
}

impl Held {
    fn appending(&self) -> MutexGuard<'_, ()> {
        self.appending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn append(&self, line: &RecordLine) -> Result<(), PluginError> {
        let mut text = encode_record_line(line);
        text.push('\n');
        let _one = self.appending();
        (&self.file)
            .write_all(text.as_bytes())
            .and_then(|()| self.file.sync_data())
            .map_err(|error| {
                PluginError::State(format!(
                    "{} cannot be appended to: {error}",
                    self.root.join(RECORD_FILE).display()
                ))
            })
    }
}

/// A plugin's state directory, checked, created and held by this host.
#[derive(Debug)]
pub struct StateDir {
    root: PathBuf,
    /// The record, opened for appending and exclusively locked while this value or a
    /// [`HeldRecord`] of it lives.
    record: Arc<Held>,
}

/// The record of a state directory this process holds, shared with the [`StateDir`] that holds
/// it: lines appended through it are serialized with the holder's own, and the record stays
/// locked while it lives, even once the [`StateDir`] is dropped.
#[derive(Debug, Clone)]
pub struct HeldRecord(Arc<Held>);

impl HeldRecord {
    /// The record this process holds for the directory `state`, when a [`StateDir`] of it, or a
    /// handle on its record, is alive; `None` otherwise, and for a directory that does not exist.
    /// Paths are compared as the file system resolves them.
    pub fn of(state: &Path) -> Option<Self> {
        let resolved = fs::canonicalize(state).ok()?;
        let mut held = held_records();
        held.retain(|(_, record)| record.strong_count() > 0);
        held.iter()
            .filter(|(root, _)| *root == resolved)
            .find_map(|(_, record)| record.upgrade())
            .map(Self)
    }

    /// The state directory, as its holder was given it.
    pub fn path(&self) -> &Path {
        &self.0.root
    }

    /// The first line the record holds for `item`, when it holds one.
    ///
    /// # Errors
    /// The record cannot be read, or a line of it is not a record line.
    pub fn line_of(&self, item: &ItemId) -> Result<Option<RecordLine>, PluginError> {
        let _one = self.0.appending();
        Ok(read_record(&self.0.root)?
            .into_iter()
            .find(|line| line.item == *item))
    }

    /// Appends `line` to the record, after any line this process is appending.
    ///
    /// # Errors
    /// The record cannot be written.
    pub fn append(&self, line: &RecordLine) -> Result<(), PluginError> {
        self.0.append(line)
    }
}

impl StateDir {
    /// Checks `path` against `config`'s workspace roots and checkouts, creates it, and takes the
    /// exclusive lock on its record.
    ///
    /// # Errors
    /// [`PluginError::StateInsideCheckout`] for a directory inside one, [`PluginError::StateInUse`]
    /// when another host holds it, [`PluginError::State`] when a path cannot be resolved, or the
    /// directory or its record cannot be created or locked.
    pub fn open(path: &Path, config: &PluginConfig) -> Result<Self, PluginError> {
        refuse_inside(path, config)?;
        fs::create_dir_all(path).map_err(|error| {
            PluginError::State(format!("{} cannot be created: {error}", path.display()))
        })?;
        let record_path = path.join(RECORD_FILE);
        let record = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&record_path)
            .map_err(|error| {
                PluginError::State(format!(
                    "{} cannot be opened: {error}",
                    record_path.display()
                ))
            })?;
        let asked = Instant::now();
        loop {
            match record.try_lock() {
                Ok(()) => break,
                // A child another thread forks carries the descriptor until it executes, and holds
                // the lock that long; a host holds it for its whole run.
                Err(TryLockError::WouldBlock) if asked.elapsed() < LOCK_WAIT => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(TryLockError::WouldBlock) => {
                    return Err(PluginError::StateInUse {
                        state: path.display().to_string(),
                    });
                }
                Err(TryLockError::Error(error)) => {
                    return Err(PluginError::State(format!(
                        "{} cannot be locked: {error}",
                        record_path.display()
                    )));
                }
            }
        }
        repair(&record, &record_path)?;
        let held = Arc::new(Held {
            root: path.to_path_buf(),
            file: record,
            appending: Mutex::new(()),
        });
        if let Ok(resolved) = fs::canonicalize(path) {
            let mut records = held_records();
            records.retain(|(_, record)| record.strong_count() > 0);
            records.push((resolved, Arc::downgrade(&held)));
        }
        Ok(Self {
            root: path.to_path_buf(),
            record: held,
        })
    }

    /// The directory.
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// A handle on the record this host holds, for another thread of this process to append
    /// through (see the module documentation).
    pub fn held(&self) -> HeldRecord {
        HeldRecord(Arc::clone(&self.record))
    }

    /// The saved state, with every item the record holds a line for counted as handled; an empty
    /// state when nothing is saved or recorded yet.
    ///
    /// # Errors
    /// A state file or record that cannot be read or is not one.
    pub fn load(&self) -> Result<PluginState, PluginError> {
        // A line another thread of this process is appending is read whole or not at all.
        let _one = self.record.appending();
        read(&self.root)
    }

    /// Saves `state` through a temporary file and a rename.
    ///
    /// # Errors
    /// The temporary file cannot be written or renamed into place.
    pub fn save(&self, state: &PluginState) -> Result<(), PluginError> {
        let target = self.root.join(STATE_FILE);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let temporary = self
            .root
            .join(format!(".{STATE_FILE}.{}.{nanos}.tmp", std::process::id()));
        let failed =
            |what: &str, error: std::io::Error| PluginError::State(format!("{what}: {error}"));
        let written = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(encode_state(state).as_bytes())?;
            file.write_all(b"\n")?;
            file.sync_all()
        })();
        if let Err(error) = written {
            let _ = fs::remove_file(&temporary);
            return Err(failed(
                &format!("{} cannot be written", temporary.display()),
                error,
            ));
        }
        fs::rename(&temporary, &target).map_err(|error| {
            let _ = fs::remove_file(&temporary);
            failed(
                &format!("{} cannot be renamed into place", target.display()),
                error,
            )
        })?;
        // The rename is durable once the directory is; a directory that cannot be synced leaves
        // the new state in place all the same.
        if let Ok(directory) = File::open(&self.root) {
            let _ = directory.sync_all();
        }
        Ok(())
    }

    /// Appends `line` to the record.
    ///
    /// # Errors
    /// The record cannot be opened or written.
    pub fn append(&self, line: &RecordLine) -> Result<(), PluginError> {
        self.record.append(line)
    }

    /// Every record line, in order; none when nothing is recorded yet.
    ///
    /// # Errors
    /// A record that cannot be read, or a line that is not a record line.
    pub fn record(&self) -> Result<Vec<RecordLine>, PluginError> {
        let _one = self.record.appending();
        read_record(&self.root)
    }
}

/// Drops the bytes after the record's last newline: the torn trace of an append that did not
/// finish, whose item was never saved as handled, so the host handles it again. Called holding
/// the record's lock. A corrupt line before the last is left alone and stays an error.
fn repair(record: &File, path: &Path) -> Result<(), PluginError> {
    let failed = |error: std::io::Error| {
        PluginError::State(format!("{} cannot be repaired: {error}", path.display()))
    };
    let bytes = fs::read(path).map_err(failed)?;
    let whole = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |at| at + 1);
    if whole < bytes.len() {
        let length = u64::try_from(whole)
            .map_err(|_| PluginError::State(format!("{} is too long to repair", path.display())))?;
        record.set_len(length).map_err(failed)?;
        record.sync_data().map_err(failed)?;
    }
    Ok(())
}

/// The state of the directory `root` as [`StateDir::load`] reads it: an empty state when the
/// state file and the record are absent.
fn read(root: &Path) -> Result<PluginState, PluginError> {
    let path = root.join(STATE_FILE);
    let mut state = match fs::read_to_string(&path) {
        Ok(text) => decode_state(&text).map_err(|error| {
            PluginError::State(format!("{} is not a plugin state: {error}", path.display()))
        })?,
        Err(error) if error.kind() == ErrorKind::NotFound => PluginState {
            cursors: Vec::new(),
            handled: Vec::new(),
            failing: Vec::new(),
        },
        Err(error) => {
            return Err(PluginError::State(format!(
                "{} cannot be read: {error}",
                path.display()
            )));
        }
    };
    for line in read_record(root)? {
        if !state.handled.contains(&line.item) {
            state.handled.push(line.item);
        }
    }
    let handled: &[ItemId] = &state.handled;
    let failing = state
        .failing
        .iter()
        .filter(|failing| !handled.contains(&failing.item.id))
        .cloned()
        .collect();
    state.failing = failing;
    Ok(state)
}

/// Every line of the record in `root`; none when it is absent.
fn read_record(root: &Path) -> Result<Vec<RecordLine>, PluginError> {
    let path = root.join(RECORD_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(PluginError::State(format!(
                "{} cannot be read: {error}",
                path.display()
            )));
        }
    };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(at, line)| {
            decode_record_line(line).map_err(|error| {
                PluginError::State(format!(
                    "line {} of {} is not a record line: {error}",
                    at + 1,
                    path.display()
                ))
            })
        })
        .collect()
}

/// Refuses `state` when it lies inside a workspace root or checkout `config` names.
///
/// # Errors
/// As [`StateDir::open`], before anything is created.
pub fn refuse_inside(state: &Path, config: &PluginConfig) -> Result<(), PluginError> {
    let resolved = resolve(state)?;
    for root in config.workspace_roots.iter().chain(&config.checkouts) {
        let root = resolve(Path::new(root))?;
        if resolved.starts_with(&root) {
            return Err(PluginError::StateInsideCheckout {
                state: resolved.display().to_string(),
                inside: root.display().to_string(),
            });
        }
    }
    Ok(())
}

/// `path` made absolute and walked component by component: each name is appended and, where the
/// result exists, resolved by the filesystem (symbolic links included); a `..` takes the parent of
/// what is resolved so far, as the kernel does.
fn resolve(path: &Path) -> Result<PathBuf, PluginError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| PluginError::State(format!("no current directory: {error}")))?
            .join(path)
    };
    let mut resolved = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => resolved.push(component),
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Normal(name) => {
                resolved.push(name);
                if let Ok(real) = fs::canonicalize(&resolved) {
                    resolved = real;
                }
            }
        }
    }
    Ok(resolved)
}
