//! The run log of each case of a tree, and the head the tree keeps of it.
//!
//! The tree appends every event through [`b10x_loom_runlog::RunLog::append`] and then writes the
//! head of the log, `loghead-<stem>.json` beside it: the `seq` of the line it appended and the
//! SHA-256 of its bytes. The chain of the log names every line but the last; the head names the
//! last, so [`verify_case_log`] sees a changed byte anywhere. A restart between the append and the
//! head leaves the log one line past its head; the next writer takes that line over
//! ([`Logs::reconcile`]) when the line before it is the head, and writes no second one.

use std::path::{Path, PathBuf};

use b10x_loom_runlog::{RunLog, digest, encode, log_file_name, verify};
use loom::runlog::RunLogEvent;
use loom::tree::{LogFailure, TreeLogHead, TreeRefusalReason};
use loom_governor::case_file_stem;
use serde_json::json;

use crate::store::{document, integer, read_object, read_regular, text, write_file};
use crate::{Refused, TreeTime, unreadable};

/// The name of the file that holds the head of `case`'s run log.
pub fn head_file_name(case: &str) -> String {
    format!("loghead-{}.json", case_file_stem(case))
}

/// The head kept for `case` in `dir`; `None` when none is kept.
fn read_head(dir: &Path, case: &str) -> Result<Option<TreeLogHead>, Refused> {
    let path = dir.join(head_file_name(case));
    let Some(object) = read_object(&path)? else {
        return Ok(None);
    };
    let named = text(&object, "case", &path)?;
    if named != case {
        return Err(unreadable(
            path.display(),
            format!("is the head of case {named:?}, not of {case:?}"),
        ));
    }
    Ok(Some(TreeLogHead {
        case: case.to_owned(),
        seq: integer(&object, "seq", &path)?,
        digest: text(&object, "digest", &path)?.to_owned(),
    }))
}

fn write_head(dir: &Path, head: &TreeLogHead) -> Result<(), Refused> {
    write_file(
        dir,
        &head_file_name(&head.case),
        &document(&json!({"case": head.case, "seq": head.seq, "digest": head.digest})),
    )
}

/// The lines of a log's bytes, each without its line feed.
fn raw_lines(bytes: &[u8]) -> Vec<&[u8]> {
    match bytes.strip_suffix(b"\n") {
        Some(body) => body.split(|byte| *byte == b'\n').collect(),
        None if bytes.is_empty() => Vec::new(),
        None => bytes.split(|byte| *byte == b'\n').collect(),
    }
}

fn failure(case: &str, line: i64, detail: impl Into<String>) -> LogFailure {
    LogFailure {
        case: case.to_owned(),
        line,
        detail: detail.into(),
    }
}

/// Verifies the run log of `case` in the store `dir` against its chain and its head: `None` when
/// both hold, else the first line that fails, counted from 0. A log with no line and no head
/// holds.
///
/// # Errors
/// `StoreUnreadable` when the log or its head cannot be read.
pub fn verify_case_log(dir: &Path, case: &str) -> Result<Option<LogFailure>, Refused> {
    let bytes = read_regular(&dir.join(log_file_name(case)))?.unwrap_or_default();
    if let Err(refused) = verify(&bytes) {
        return Ok(Some(failure(
            case,
            refused.line,
            format!("{:?}: {}", refused.refusal, refused.detail),
        )));
    }
    let lines = raw_lines(&bytes);
    let head = read_head(dir, case)?;
    Ok(match (lines.last(), head) {
        (None, None) => None,
        (None, Some(head)) => Some(failure(
            case,
            head.seq,
            format!(
                "the log holds no line, and the tree wrote line {}",
                head.seq
            ),
        )),
        (Some(_), None) => Some(failure(
            case,
            0,
            "the log holds lines the tree has no head for",
        )),
        (Some(last), Some(head)) => {
            let index = i64::try_from(lines.len() - 1).unwrap_or(i64::MAX);
            if index != head.seq {
                Some(failure(
                    case,
                    index.min(head.seq.max(0)),
                    format!(
                        "the log ends at line {index}, and the tree last wrote line {}",
                        head.seq
                    ),
                ))
            } else if digest(last) != head.digest {
                Some(failure(
                    case,
                    index,
                    "the last line is not the line the tree wrote: a byte of it changed",
                ))
            } else {
                None
            }
        }
    })
}

/// The run logs of one store, written by the one writer that holds it.
pub(crate) struct Logs {
    log: RunLog,
    dir: PathBuf,
    time: TreeTime,
}

impl Logs {
    /// The logs of the store `dir`, whose instants come from `time`.
    pub(crate) fn open(dir: &Path, time: TreeTime) -> Result<Self, Refused> {
        let log =
            RunLog::open(dir.to_owned()).map_err(|error| unreadable("the run logs", error))?;
        Ok(Self {
            log,
            dir: dir.to_owned(),
            time,
        })
    }

    /// Appends `event` to `case`'s log, then writes its head.
    pub(crate) fn append(&self, case: &str, event: RunLogEvent) -> Result<(), Refused> {
        let at = self.time.now()?;
        let line = self.log.append(case, at, event).map_err(|error| {
            Refused::new(TreeRefusalReason::LogRefused, error.to_string()).case(case)
        })?;
        write_head(
            &self.dir,
            &TreeLogHead {
                case: case.to_owned(),
                seq: line.seq,
                digest: digest(encode(&line).as_bytes()),
            },
        )
    }

    /// Makes `case`'s log fit for an append: one that verifies with its head as it is, or whose
    /// one line past its head follows it, a restart having come between the append and the head
    /// (its head is then written for that line).
    ///
    /// # Errors
    /// `LogRefused` naming the case and the line when the log does not verify or its head does not
    /// name its last line or the one before it; nothing is written then.
    pub(crate) fn reconcile(&self, case: &str) -> Result<(), Refused> {
        let Some(found) = verify_case_log(&self.dir, case)? else {
            return Ok(());
        };
        let refused = || {
            Refused::new(
                TreeRefusalReason::LogRefused,
                format!("line {}: {}", found.line, found.detail),
            )
            .case(case)
        };
        let bytes = read_regular(&self.dir.join(log_file_name(case)))?.unwrap_or_default();
        if verify(&bytes).is_err() {
            return Err(refused());
        }
        let lines = raw_lines(&bytes);
        let Some(last) = lines.last() else {
            return Err(refused());
        };
        let follows = match read_head(&self.dir, case)? {
            None => lines.len() == 1,
            Some(head) => {
                usize::try_from(head.seq).ok() == Some(lines.len().wrapping_sub(2))
                    && digest(lines[lines.len() - 2]) == head.digest
            }
        };
        if !follows {
            return Err(refused());
        }
        write_head(
            &self.dir,
            &TreeLogHead {
                case: case.to_owned(),
                seq: i64::try_from(lines.len() - 1).unwrap_or(i64::MAX),
                digest: digest(last),
            },
        )
    }
}
