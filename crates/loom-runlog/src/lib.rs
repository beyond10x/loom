//! The run log of a case. Stub: the surface the tests in `tests/run_log.rs` name, before the log
//! is written.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::fmt;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use loom::primitives::Timestamp;
use loom::runlog::{RunLogEvent, RunLogLine, RunLogRefused};
use loom_governor::{FileStoreError, LockedDir, case_file_stem};

/// The name of the log file of the case with id `case`.
pub fn log_file_name(case: &str) -> String {
    format!("runlog-{}.jsonl", case_file_stem(case))
}

/// A step of one append.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppendStep {
    /// The temporary file was created.
    Created,
    /// Half the bytes were written.
    HalfWritten,
    /// Every byte was written.
    Written,
    /// The temporary file was synced.
    Synced,
    /// The temporary file was renamed over the log.
    Renamed,
    /// The directory was synced.
    DirectorySynced,
}

impl AppendStep {
    /// Every step, in order.
    pub const ALL: [Self; 6] = [
        Self::Created,
        Self::HalfWritten,
        Self::Written,
        Self::Synced,
        Self::Renamed,
        Self::DirectorySynced,
    ];

    /// Whether the log holds the new line once this step is reached.
    pub fn renamed(self) -> bool {
        matches!(self, Self::Renamed | Self::DirectorySynced)
    }
}

/// Why a read or an append did not happen.
#[derive(Debug)]
pub enum RunLogError {
    /// The log does not verify.
    Refused(RunLogRefused),
    /// The store could not be read or written.
    Store(FileStoreError),
}

impl fmt::Display for RunLogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refused) => write!(f, "line {}: {}", refused.line, refused.detail),
            Self::Store(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for RunLogError {}

/// The run logs kept in one store directory.
#[derive(Debug)]
pub struct RunLog {
    dir: LockedDir,
}

impl RunLog {
    /// The run logs over `dir`.
    ///
    /// # Errors
    ///
    /// [`FileStoreError`] when the directory does not open.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, FileStoreError> {
        LockedDir::open(dir.into()).map(|dir| Self { dir })
    }

    /// The store directory.
    pub fn dir(&self) -> &Path {
        self.dir.path()
    }

    /// The log file of `case`.
    pub fn path(&self, case: &str) -> PathBuf {
        self.dir.path().join(log_file_name(case))
    }

    /// The lines of `case`'s log.
    ///
    /// # Errors
    ///
    /// Never, in this stub.
    pub fn read(&self, _case: &str) -> Result<Vec<RunLogLine>, RunLogError> {
        Ok(Vec::new())
    }

    /// Appends `event` to `case`'s log.
    ///
    /// # Errors
    ///
    /// Always, in this stub.
    pub fn append(
        &self,
        case: &str,
        at: Timestamp,
        event: RunLogEvent,
    ) -> Result<RunLogLine, RunLogError> {
        self.append_until(case, at, event, &mut |_| ControlFlow::Continue(()))?
            .ok_or_else(|| {
                RunLogError::Store(FileStoreError {
                    path: self.path(case),
                    problem: "not written".to_owned(),
                })
            })
    }

    /// Appends `event`, handing `stop` each step.
    ///
    /// # Errors
    ///
    /// Never, in this stub.
    pub fn append_until(
        &self,
        _case: &str,
        _at: Timestamp,
        _event: RunLogEvent,
        _stop: &mut dyn FnMut(AppendStep) -> ControlFlow<()>,
    ) -> Result<Option<RunLogLine>, RunLogError> {
        Ok(None)
    }
}

/// The lines of a log.
///
/// # Errors
///
/// Never, in this stub.
pub fn verify(_bytes: &[u8]) -> Result<Vec<RunLogLine>, RunLogRefused> {
    Ok(Vec::new())
}
