//! The run log of a case: one JSON line per event, each naming the SHA-256 of the line before it
//! (`loom.runlog`, `ess/domains/runlog.yaml`).
//!
//! A case's log is the file `runlog-<stem>.jsonl` in the store directory, beside the case file
//! `<stem>.json` the governor's [`FileCaseStore`](loom_governor::FileCaseStore) keeps, under the
//! same stem ([`loom_governor::case_file_stem`]). Each line is one [`RunLogLine`] as one JSON
//! object ended by a line feed: `seq`, `prev` (absent on line 0), `at`, and the event's `kind`
//! with its members beside them. `seq` counts the lines from 0; `prev` is the SHA-256 of the
//! previous line's bytes, without its line feed, as 64 lower-case hex digits ([`digest`]).
//!
//! [`RunLog::append`] holds the directory's lock, the [`LockedDir`] lock every governor store
//! takes, for the whole append: it reads the log, refuses one that does not verify with the
//! reader's refusal (writing nothing), and writes the whole log with the new line to a fresh
//! temporary file in the directory, syncs it and renames it over the log. A crash at any point
//! leaves every earlier line unchanged and either all of the new line or none of it; the
//! temporary file a crash leaves is removed when a store next opens the directory. Lines are
//! never rewritten. A line that would not read back as the line meant (an `at` that is no
//! RFC 3339 instant, say) is refused before anything is written, so an append never makes a log
//! that does not verify.
//!
//! [`verify`] reads a log and refuses, naming the first line that fails (counted from 0, as
//! `seq` counts them), in this order for each line:
//!
//! - `CutLine`: the last line does not end with a line feed, or does not read as a line;
//! - `SequenceGap`: the line's `seq` is not its position (a line deleted, or two swapped);
//! - `BrokenChain`: the line's `prev` is not the digest of the line before it, absent where it
//!   must be present, or present on line 0. A changed byte in a line's event leaves the line
//!   itself whole and is named by the line after it. A line before the last that does not read
//!   as a line is named by the line after it when that line's `prev` does not match it, and is
//!   itself named otherwise.
//!
//! An `ActionFinished` holds an `exit` or a `refusal`, exactly one: a line with both or neither
//! does not read as a line, so `verify` refuses it and an append refuses to write it. A
//! `ChildOpened` or `ChildClosed` holds `record` and `record_digest` together or neither, the
//! digest 64 lower-case hex digits, under the same rule. A log written before those members
//! existed reads with both absent; a reader from before them refuses a line that holds them.
//!
//! What the log cannot show: a change to the last line alone is not detected, as a cut at a line
//! boundary is not, since no line after it names its digest, and the next append makes the change
//! permanent. A log whose last lines were removed whole, or rewritten from some line on with a
//! fresh chain, verifies too. The log is not signed.
//!
//! The crate declares the events and writes what it is given; it emits nothing itself.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::fmt;
use std::fs;
use std::io::{self, Write as _};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use loom::json::{
    self, DecodeError, Value, closed_at, integer_at, items_at, member, member_at, members_at,
    push_integer, push_text, text_at, timestamp_at,
};
use loom::primitives::Timestamp;
use loom::runlog::{
    ActionFinished, ActionStarted, AuthorityRecorded, CaseOpened, ChildClosed, ChildOpened,
    Discarded, Evaluated, RevisionMove, RunLogEvent, RunLogLine, RunLogRefusal, RunLogRefused,
    Stopped, Terminated,
};
use loom_governor::{FileStoreError, LockedDir, case_file_stem};
use sha2::{Digest as _, Sha256};

/// What every log file's name starts with.
pub const PREFIX: &str = "runlog-";

/// What every log file's name ends with.
pub const EXTENSION: &str = ".jsonl";

/// The name of the log file of the case with id `case`: [`PREFIX`], the case file's stem and
/// [`EXTENSION`].
pub fn log_file_name(case: &str) -> String {
    format!("{PREFIX}{}{EXTENSION}", case_file_stem(case))
}

/// The digest a line's successor names in `prev`: the SHA-256 of `line`, the line's bytes without
/// its line feed, as 64 lower-case hex digits.
pub fn digest(line: &[u8]) -> String {
    Sha256::digest(line)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A step of one append, in the order they happen; [`RunLog::append_until`] hands each to its
/// caller once it is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppendStep {
    /// The temporary file was created, empty.
    Created,
    /// The first half of the log's new bytes was written to the temporary file.
    HalfWritten,
    /// Every byte was written to the temporary file.
    Written,
    /// The temporary file was synced.
    Synced,
    /// The temporary file was renamed over the log: the log holds the new line.
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

    /// Whether the log holds the new line once this step is done.
    pub fn renamed(self) -> bool {
        matches!(self, Self::Renamed | Self::DirectorySynced)
    }
}

/// Why a read or an append did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunLogError {
    /// The log does not verify; an append to it wrote nothing.
    Refused(RunLogRefused),
    /// The store could not be read or written, or the line would not read back as given; an
    /// append that failed so left the log as it was.
    Store(FileStoreError),
}

impl fmt::Display for RunLogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refused) => write!(
                f,
                "the run log is refused {:?} at line {}: {}",
                refused.refusal, refused.line, refused.detail
            ),
            Self::Store(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for RunLogError {}

impl From<FileStoreError> for RunLogError {
    fn from(error: FileStoreError) -> Self {
        Self::Store(error)
    }
}

fn failed(path: &Path, problem: impl fmt::Display) -> FileStoreError {
    FileStoreError {
        path: path.to_owned(),
        problem: problem.to_string(),
    }
}

/// The run logs kept in one store directory, under its lock.
#[derive(Debug)]
pub struct RunLog {
    dir: LockedDir,
}

impl RunLog {
    /// The run logs over `dir`, created when it does not exist yet. `dir` may be, and usually
    /// is, the directory a [`FileCaseStore`](loom_governor::FileCaseStore) keeps its cases in.
    /// Opening removes the temporary files a crashed write left, holding the lock.
    ///
    /// # Errors
    ///
    /// [`FileStoreError`] when the directory cannot be opened as a governor store's is
    /// ([`LockedDir::open`]).
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

    /// Every line of `case`'s log, read holding the lock and verified; none when the case has no
    /// log yet.
    ///
    /// # Errors
    ///
    /// [`RunLogError::Refused`] when the log does not verify ([`verify`]);
    /// [`RunLogError::Store`] when it cannot be read.
    pub fn read(&self, case: &str) -> Result<Vec<RunLogLine>, RunLogError> {
        let path = self.path(case);
        self.dir.locked(|| {
            let bytes = read_log(&path)?;
            verify(&bytes).map_err(RunLogError::Refused)
        })?
    }

    /// Appends `event` at `at` to `case`'s log as its next line, and returns that line.
    ///
    /// # Errors
    ///
    /// [`RunLogError::Refused`] with the reader's refusal when the log does not verify, and
    /// [`RunLogError::Store`] when it cannot be read or written or the line would not read back
    /// as given; in each case no byte of the log changed.
    pub fn append(
        &self,
        case: &str,
        at: Timestamp,
        event: RunLogEvent,
    ) -> Result<RunLogLine, RunLogError> {
        let written = self.append_until(case, at, event, &mut |_| ControlFlow::Continue(()))?;
        written.ok_or_else(|| {
            RunLogError::Store(failed(
                &self.path(case),
                "the append stopped though nothing asked it to",
            ))
        })
    }

    /// [`RunLog::append`], handing `stop` each [`AppendStep`] once it is done. When `stop`
    /// answers [`ControlFlow::Break`] the append ends right there, as a crash would end it: it
    /// releases the lock and does nothing else (its temporary file stays until a store opens the
    /// directory again), and answers `Ok(None)`. Otherwise it answers the line it wrote.
    ///
    /// # Errors
    ///
    /// As [`RunLog::append`].
    pub fn append_until(
        &self,
        case: &str,
        at: Timestamp,
        event: RunLogEvent,
        stop: &mut dyn FnMut(AppendStep) -> ControlFlow<()>,
    ) -> Result<Option<RunLogLine>, RunLogError> {
        let path = self.path(case);
        self.dir
            .locked(|| self.append_locked(&path, at, event, stop))?
    }

    fn append_locked(
        &self,
        path: &Path,
        at: Timestamp,
        event: RunLogEvent,
        stop: &mut dyn FnMut(AppendStep) -> ControlFlow<()>,
    ) -> Result<Option<RunLogLine>, RunLogError> {
        let mut bytes = read_log(path)?;
        let lines = verify(&bytes).map_err(RunLogError::Refused)?;
        let seq = i64::try_from(lines.len())
            .map_err(|_| failed(path, "the log holds more lines than a seq can number"))?;
        let prev = last_line(&bytes).map(digest);
        let line = RunLogLine {
            seq,
            prev,
            at,
            event,
        };
        let text = encode(&line);
        if decode_line(text.as_bytes()).as_ref() != Ok(&line) {
            return Err(failed(
                path,
                "the line would not read back as it was given; nothing was written",
            )
            .into());
        }
        bytes.extend_from_slice(text.as_bytes());
        bytes.push(b'\n');

        let (temporary, mut file) = self.dir.temporary()?;
        macro_rules! step {
            ($step:expr) => {
                if stop($step).is_break() {
                    return Ok(None);
                }
            };
        }
        let abandon = |error: FileStoreError| {
            let _ = fs::remove_file(&temporary);
            RunLogError::Store(error)
        };
        step!(AppendStep::Created);
        let (first, second) = bytes.split_at(bytes.len() / 2);
        file.write_all(first)
            .map_err(|error| abandon(failed(&temporary, error)))?;
        step!(AppendStep::HalfWritten);
        file.write_all(second)
            .map_err(|error| abandon(failed(&temporary, error)))?;
        step!(AppendStep::Written);
        file.sync_all()
            .map_err(|error| abandon(failed(&temporary, error)))?;
        step!(AppendStep::Synced);
        drop(file);
        fs::rename(&temporary, path).map_err(|error| abandon(failed(path, error)))?;
        step!(AppendStep::Renamed);
        // The rename has happened, so the new line is in the log; a failed directory sync only
        // weakens how soon it is durable and is not reported as a failed append.
        if let Ok(dir) = fs::File::open(self.dir.path()) {
            let _ = dir.sync_all();
        }
        step!(AppendStep::DirectorySynced);
        Ok(Some(line))
    }
}

/// The log's bytes, empty when there is no log. Only a regular file is a log: a symlink, a
/// directory or any other entry is refused, never followed.
fn read_log(path: &Path) -> Result<Vec<u8>, FileStoreError> {
    match fs::symlink_metadata(path) {
        Ok(entry) if entry.file_type().is_file() => {}
        Ok(_) => {
            return Err(failed(
                path,
                "is not a regular file, not a run log this crate wrote; it is not followed",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(failed(path, error)),
    }
    fs::read(path).map_err(|error| failed(path, error))
}

/// The last line of a log that verified, without its line feed; `None` when it holds none.
fn last_line(bytes: &[u8]) -> Option<&[u8]> {
    let body = bytes.strip_suffix(b"\n")?;
    Some(match body.iter().rposition(|byte| *byte == b'\n') {
        Some(end) => &body[end + 1..],
        None => body,
    })
}

fn refused(refusal: RunLogRefusal, line: usize, detail: impl Into<String>) -> RunLogRefused {
    RunLogRefused {
        refusal,
        line: i64::try_from(line).unwrap_or(i64::MAX),
        detail: detail.into(),
    }
}

/// Every line of the log `bytes`, when the chain holds; the refusal naming the first line that
/// fails it otherwise (see the crate documentation for the order of the checks). An empty log
/// holds no line.
///
/// # Errors
///
/// [`RunLogRefused`] naming the refusal, the line counted from 0 and what is wrong with it.
pub fn verify(bytes: &[u8]) -> Result<Vec<RunLogLine>, RunLogRefused> {
    let mut lines = Vec::new();
    if bytes.is_empty() {
        return Ok(lines);
    }
    let (body, terminated) = match bytes.strip_suffix(b"\n") {
        Some(body) => (body, true),
        None => (bytes, false),
    };
    let raw: Vec<&[u8]> = body.split(|byte| *byte == b'\n').collect();
    let last = raw.len() - 1;
    let mut previous: Option<&[u8]> = None;
    let mut unreadable: Option<(usize, String)> = None;
    for (index, text) in raw.iter().enumerate() {
        if index == last && !terminated {
            return Err(refused(
                RunLogRefusal::CutLine,
                index,
                "the last line does not end with a line feed: it was cut",
            ));
        }
        let line = match decode_line(text) {
            Ok(line) => line,
            Err(problem) if index == last => {
                return Err(refused(
                    RunLogRefusal::CutLine,
                    index,
                    format!("the last line does not read as a line: {problem}"),
                ));
            }
            Err(problem) => {
                if unreadable.is_some() {
                    return Err(refused(
                        RunLogRefusal::BrokenChain,
                        index,
                        format!("the line does not read, so it names no line before it: {problem}"),
                    ));
                }
                unreadable = Some((index, problem));
                previous = Some(text);
                continue;
            }
        };
        if usize::try_from(line.seq).ok() != Some(index) {
            return Err(refused(
                RunLogRefusal::SequenceGap,
                index,
                format!("seq is {} where line {index} must hold {index}", line.seq),
            ));
        }
        let expected = previous.map(digest);
        if line.prev != expected {
            let detail = match (&line.prev, &expected) {
                (Some(_), None) => "line 0 names a previous line".to_owned(),
                (None, Some(_)) => "the line names no previous line".to_owned(),
                _ => format!(
                    "prev is not the SHA-256 of line {}: the line before it changed",
                    index - 1
                ),
            };
            return Err(refused(RunLogRefusal::BrokenChain, index, detail));
        }
        if let Some((at, problem)) = unreadable.take() {
            return Err(refused(
                RunLogRefusal::BrokenChain,
                at,
                format!("the line does not read as a line: {problem}"),
            ));
        }
        lines.push(line);
        previous = Some(text);
    }
    Ok(lines)
}

/// The JSON line of `line`, without its line feed: `seq`, `prev` when present, `at`, the event's
/// `kind` and its members in declaration order, an absent optional member left out.
pub fn encode(line: &RunLogLine) -> String {
    let mut out = String::from("{");
    member(&mut out, "seq");
    push_integer(&mut out, line.seq);
    if let Some(prev) = &line.prev {
        member(&mut out, "prev");
        push_text(&mut out, prev);
    }
    member(&mut out, "at");
    push_text(&mut out, &line.at.0);
    member(&mut out, "kind");
    push_text(&mut out, kind(&line.event));
    let text = |out: &mut String, name: &str, value: &str| {
        member(out, name);
        push_text(out, value);
    };
    let optional = |out: &mut String, name: &str, value: &Option<String>| {
        if let Some(value) = value {
            member(out, name);
            push_text(out, value);
        }
    };
    let texts = |out: &mut String, name: &str, values: &[String]| {
        member(out, name);
        out.push('[');
        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            push_text(out, value);
        }
        out.push(']');
    };
    match &line.event {
        RunLogEvent::CaseOpened(event) => {
            text(&mut out, "case", &event.case);
            text(&mut out, "protocol", &event.protocol);
        }
        RunLogEvent::Evaluated(event) => {
            member(&mut out, "case_revision");
            push_integer(&mut out, event.case_revision);
            text(&mut out, "frontier", &event.frontier);
            text(&mut out, "decision_digest", &event.decision_digest);
        }
        RunLogEvent::ActionStarted(event) => text(&mut out, "action", &event.action),
        RunLogEvent::ActionFinished(event) => {
            text(&mut out, "action", &event.action);
            if let Some(exit) = event.exit {
                member(&mut out, "exit");
                push_integer(&mut out, exit);
            }
            optional(&mut out, "refusal", &event.refusal);
            optional(&mut out, "blob_digest", &event.blob_digest);
            texts(&mut out, "evidence", &event.evidence);
            member(&mut out, "revisions");
            out.push('[');
            for (index, moved) in event.revisions.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push('{');
                text(&mut out, "artifact", &moved.artifact);
                optional(&mut out, "from", &moved.from);
                text(&mut out, "to", &moved.to);
                out.push('}');
            }
            out.push(']');
        }
        RunLogEvent::Discarded(event) => {
            text(&mut out, "action", &event.action);
            text(&mut out, "reason", &event.reason);
        }
        RunLogEvent::ChildOpened(event) => {
            text(&mut out, "child", &event.child);
            text(&mut out, "protocol", &event.protocol);
            optional(&mut out, "record", &event.record);
            optional(&mut out, "record_digest", &event.record_digest);
        }
        RunLogEvent::ChildClosed(event) => {
            text(&mut out, "child", &event.child);
            optional(&mut out, "outcome", &event.outcome);
            optional(&mut out, "record", &event.record);
            optional(&mut out, "record_digest", &event.record_digest);
        }
        RunLogEvent::AuthorityRecorded(event) => {
            text(&mut out, "capability", &event.capability);
            text(&mut out, "decision", &event.decision);
            text(&mut out, "principal", &event.principal);
        }
        RunLogEvent::Stopped(event) => {
            text(&mut out, "stop", &event.stop);
            texts(&mut out, "next", &event.next);
        }
        RunLogEvent::Terminated(event) => text(&mut out, "reason", &event.reason),
    }
    out.push('}');
    out
}

/// The tag an event is written under.
fn kind(event: &RunLogEvent) -> &'static str {
    match event {
        RunLogEvent::CaseOpened(_) => "CaseOpened",
        RunLogEvent::Evaluated(_) => "Evaluated",
        RunLogEvent::ActionStarted(_) => "ActionStarted",
        RunLogEvent::ActionFinished(_) => "ActionFinished",
        RunLogEvent::Discarded(_) => "Discarded",
        RunLogEvent::ChildOpened(_) => "ChildOpened",
        RunLogEvent::ChildClosed(_) => "ChildClosed",
        RunLogEvent::AuthorityRecorded(_) => "AuthorityRecorded",
        RunLogEvent::Stopped(_) => "Stopped",
        RunLogEvent::Terminated(_) => "Terminated",
    }
}

/// The members a line of each kind may hold besides its own: `seq`, `prev`, `at`, `kind`.
fn event_members(kind: &str) -> Option<&'static [&'static str]> {
    Some(match kind {
        "CaseOpened" => &["case", "protocol"],
        "Evaluated" => &["case_revision", "frontier", "decision_digest"],
        "ActionStarted" => &["action"],
        "ActionFinished" => &[
            "action",
            "exit",
            "refusal",
            "blob_digest",
            "evidence",
            "revisions",
        ],
        "Discarded" => &["action", "reason"],
        "ChildOpened" => &["child", "protocol", "record", "record_digest"],
        "ChildClosed" => &["child", "outcome", "record", "record_digest"],
        "AuthorityRecorded" => &["capability", "decision", "principal"],
        "Stopped" => &["stop", "next"],
        "Terminated" => &["reason"],
        _ => return None,
    })
}

/// One line read back, or why it does not read: not UTF-8, not one JSON object, a member twice,
/// a member its kind does not declare, or one missing or of the wrong type.
fn decode_line(bytes: &[u8]) -> Result<RunLogLine, String> {
    let text = std::str::from_utf8(bytes).map_err(|error| format!("not UTF-8: {error}"))?;
    let value = json::parse(text).map_err(|error| format!("not JSON: {error}"))?;
    decode(&value).map_err(|error| error.to_string())
}

fn decode(value: &Value) -> Result<RunLogLine, DecodeError> {
    let members = members_at(value, "", "a run log line, a JSON object")?;
    for (index, (name, held)) in members.iter().enumerate() {
        if members[..index].iter().any(|(earlier, _)| earlier == name) {
            return Err(DecodeError::of(name, "a member named once", held));
        }
    }
    let kind = text_at(member_at(value, "", "kind")?, "kind", "an event kind")?;
    let Some(declared) = event_members(kind) else {
        return Err(DecodeError {
            at: "kind".to_owned(),
            expected: "a kind loom.runlog.RunLogEvent declares".to_owned(),
            found: format!("`{kind}`"),
        });
    };
    let mut allowed = vec!["seq", "prev", "at", "kind"];
    allowed.extend_from_slice(declared);
    closed_at(value, "", &allowed)?;
    let seq = integer_at(member_at(value, "", "seq")?, "seq", "an integer")?;
    let prev = optional_text(value, "", "prev")?;
    let at = Timestamp(
        timestamp_at(member_at(value, "", "at")?, "at", "an RFC 3339 instant")?.to_owned(),
    );
    let text = |name: &str| -> Result<String, DecodeError> {
        Ok(text_at(member_at(value, "", name)?, name, "a string")?.to_owned())
    };
    let texts = |name: &str| -> Result<Vec<String>, DecodeError> {
        items_at(member_at(value, "", name)?, name, "a list of strings")?
            .iter()
            .enumerate()
            .map(|(index, item)| {
                Ok(text_at(item, &format!("{name}.{index}"), "a string")?.to_owned())
            })
            .collect()
    };
    let event = match kind {
        "CaseOpened" => RunLogEvent::CaseOpened(CaseOpened {
            case: text("case")?,
            protocol: text("protocol")?,
        }),
        "Evaluated" => RunLogEvent::Evaluated(Evaluated {
            case_revision: integer_at(
                member_at(value, "", "case_revision")?,
                "case_revision",
                "an integer",
            )?,
            frontier: text("frontier")?,
            decision_digest: text("decision_digest")?,
        }),
        "ActionStarted" => RunLogEvent::ActionStarted(ActionStarted {
            action: text("action")?,
        }),
        "ActionFinished" if value.member("exit").is_some() == value.member("refusal").is_some() => {
            return Err(DecodeError {
                at: "exit".to_owned(),
                expected: "an exit or a refusal, exactly one".to_owned(),
                found: if value.member("exit").is_some() {
                    "both".to_owned()
                } else {
                    "neither".to_owned()
                },
            });
        }
        "ActionFinished" => RunLogEvent::ActionFinished(ActionFinished {
            action: text("action")?,
            exit: value
                .member("exit")
                .map(|exit| integer_at(exit, "exit", "an integer"))
                .transpose()?,
            refusal: optional_text(value, "", "refusal")?,
            blob_digest: optional_text(value, "", "blob_digest")?,
            evidence: texts("evidence")?,
            revisions: items_at(member_at(value, "", "revisions")?, "revisions", "a list")?
                .iter()
                .enumerate()
                .map(|(index, moved)| revision_move(moved, &format!("revisions.{index}")))
                .collect::<Result<_, _>>()?,
        }),
        "Discarded" => RunLogEvent::Discarded(Discarded {
            action: text("action")?,
            reason: text("reason")?,
        }),
        "ChildOpened" => {
            let (record, record_digest) = child_record(value)?;
            RunLogEvent::ChildOpened(ChildOpened {
                child: text("child")?,
                protocol: text("protocol")?,
                record,
                record_digest,
            })
        }
        "ChildClosed" => {
            let (record, record_digest) = child_record(value)?;
            RunLogEvent::ChildClosed(ChildClosed {
                child: text("child")?,
                outcome: optional_text(value, "", "outcome")?,
                record,
                record_digest,
            })
        }
        "AuthorityRecorded" => RunLogEvent::AuthorityRecorded(AuthorityRecorded {
            capability: text("capability")?,
            decision: text("decision")?,
            principal: text("principal")?,
        }),
        "Stopped" => RunLogEvent::Stopped(Stopped {
            stop: text("stop")?,
            next: texts("next")?,
            budget: None,
        }),
        _ => RunLogEvent::Terminated(Terminated {
            reason: text("reason")?,
        }),
    };
    Ok(RunLogLine {
        seq,
        prev,
        at,
        event,
    })
}

fn optional_text(value: &Value, at: &str, name: &str) -> Result<Option<String>, DecodeError> {
    value
        .member(name)
        .map(|held| text_at(held, &json::nested(at, name), "a string").map(str::to_owned))
        .transpose()
}

/// The `record` and `record_digest` of a child line: both or neither, the digest 64 lower-case
/// hex digits. ESS cannot state the pairing yet, so the reader holds it.
fn child_record(value: &Value) -> Result<(Option<String>, Option<String>), DecodeError> {
    let record = optional_text(value, "", "record")?;
    let digest = optional_text(value, "", "record_digest")?;
    let missing = |at: &str, beside: &str| DecodeError {
        at: at.to_owned(),
        expected: format!("a {at} beside {beside}: the two come together or not at all"),
        found: "none".to_owned(),
    };
    match (&record, &digest) {
        (Some(_), None) => Err(missing("record_digest", "record")),
        (None, Some(_)) => Err(missing("record", "record_digest")),
        (_, Some(held)) if !is_sha256(held) => Err(DecodeError {
            at: "record_digest".to_owned(),
            expected: "a SHA-256, 64 lower-case hex digits".to_owned(),
            found: if held.len() == 64 {
                "a character that is no lower-case hex digit".to_owned()
            } else {
                format!("{} bytes", held.len())
            },
        }),
        _ => Ok((record, digest)),
    }
}

/// Whether `text` is 64 lower-case hex digits.
fn is_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn revision_move(value: &Value, at: &str) -> Result<RevisionMove, DecodeError> {
    let members = members_at(value, at, "a revision move, a JSON object")?;
    for (index, (name, held)) in members.iter().enumerate() {
        if members[..index].iter().any(|(earlier, _)| earlier == name) {
            return Err(DecodeError::of(
                &json::nested(at, name),
                "a member named once",
                held,
            ));
        }
    }
    closed_at(value, at, &["artifact", "from", "to"])?;
    let text = |name: &str| -> Result<String, DecodeError> {
        let path = json::nested(at, name);
        Ok(text_at(member_at(value, at, name)?, &path, "a string")?.to_owned())
    };
    Ok(RevisionMove {
        artifact: text("artifact")?,
        from: optional_text(value, at, "from")?,
        to: text("to")?,
    })
}
