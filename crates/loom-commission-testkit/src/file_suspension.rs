//! [`FileSuspensionStore`]: Commission's suspension port in a directory, so a run suspended on an
//! approval outlives the process that suspended it (`story:approval-suspend-resume-slice`).
//!
//! A test store, not a production host's. Each commission's Suspension is one file,
//! `suspension-<hex of the commission id>.json`, holding the Suspension's state (`Pending` or
//! `Resuming`) and fields as JSON written with the generated JSON writer and read with its reader.
//! `HoldSuspension`, `TakeSuspension` and `ReleaseSuspension` are decided by Commission's own
//! decisions (`decide_hold`, `decide_take`, `decide_release`) over the file as it is read, and
//! their write made, under one hold of the lock below, so of two resumes over one directory only
//! one takes a Suspension. A store that cannot read or write answers the command's
//! `storage-failed` and changes nothing. Every write goes to a fresh temporary
//! file in the same directory, is synced and renamed over its target, so a reader sees the previous
//! document or the next one and never part of one; a failed write leaves what was stored before.
//! Before anything is written it is read back and compared with what was meant, so a Suspension
//! that would not come back exactly is refused rather than written. A document naming a member its
//! declaration does not, or one member twice, is refused on read, as is one holding another
//! commission's Suspension. Temporary files a dead process left are removed when a store opens.
//!
//! Every operation holds an exclusive lock on `<dir>/.lock` for its duration, so two stores over
//! one directory, in one process or in two, never interleave a read and a write of the same file.
//!
//! A commission id over [`LONGEST_ID`] bytes is refused: its file name would pass the 255 bytes a
//! file name may have.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use b10x_loom_commission::model::json::{
    self, DecodeError, Value, integer_at, items_at, member, member_at, members_at, nested,
    push_integer, push_text, push_value, text_at, uuid_at,
};
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, CaseId, CommissionId, HoldSuspension, HoldSuspensionOutcome,
    HumanDecisionRequest, ProposedActionArguments, ReleaseSuspension, ReleaseSuspensionOutcome,
    RunId, SuspensionData, SuspensionKey, SuspensionReason, SuspensionSnapshot, SuspensionState,
    SuspensionStorageFailed, TakeSuspension, TakeSuspensionOutcome,
};
use b10x_loom_commission::ports::suspension::{
    SuspensionPort, SuspensionStoreError, SuspensionWrite, decide_hold, decide_release, decide_take,
};

/// The longest commission id, in bytes, the store keeps: `suspension-`, 200 hex digits and `.json`
/// is 216 bytes.
pub const LONGEST_ID: usize = 100;

const LOCK: &str = ".lock";
const TEMPORARY: &str = ".tmp-";

/// How many temporary names in use one write skips before it reports the last as its error.
const TEMPORARY_ATTEMPTS: u32 = 1024;

/// Temporary file names this process has used, so two writes in one process never share one.
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// The members a stored Suspension has, in the order written.
const MEMBERS: [&str; 10] = [
    "state",
    "commission_id",
    "run_id",
    "run_case_revision",
    "action_request_id",
    "case_id",
    "expected_case_revision",
    "action",
    "arguments",
    "reason",
];

/// A [`SuspensionPort`] keeping one file per commission in a directory.
#[derive(Debug)]
pub struct FileSuspensionStore {
    dir: PathBuf,
}

fn failed(path: &Path, problem: impl std::fmt::Display) -> SuspensionStoreError {
    SuspensionStoreError::new(format!("{}: {problem}", path.display()))
}

impl FileSuspensionStore {
    /// A store over `dir`, created when it does not exist yet.
    ///
    /// # Errors
    ///
    /// [`SuspensionStoreError`] when the directory or its lock file cannot be created, or a
    /// temporary file an earlier life left behind cannot be removed.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, SuspensionStoreError> {
        let dir = dir.into();
        fs::create_dir_all(&dir).map_err(|error| failed(&dir, error))?;
        let lock = dir.join(LOCK);
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock)
            .map_err(|error| failed(&lock, error))?;
        let store = Self { dir };
        store.locked(|| store.remove_stale_temporaries())?;
        Ok(store)
    }

    /// The directory the store keeps its files in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Removes every temporary file in the directory. Only called holding the lock, when no write
    /// of any store over the directory is in progress.
    fn remove_stale_temporaries(&self) -> Result<(), SuspensionStoreError> {
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

    /// The file that holds `commission`'s Suspension, named so no id names a file outside the
    /// store.
    fn path(&self, commission: &CommissionId) -> Result<PathBuf, SuspensionStoreError> {
        let id = commission.0.0.as_bytes();
        if id.len() > LONGEST_ID {
            return Err(SuspensionStoreError::new(format!(
                "a commission id of {} bytes is longer than the {LONGEST_ID} this store keeps",
                id.len()
            )));
        }
        let mut name = String::from("suspension-");
        for byte in id {
            name.push_str(&format!("{byte:02x}"));
        }
        name.push_str(".json");
        Ok(self.dir.join(name))
    }

    /// Runs `operation` holding the directory's lock.
    fn locked<T>(
        &self,
        operation: impl FnOnce() -> Result<T, SuspensionStoreError>,
    ) -> Result<T, SuspensionStoreError> {
        let path = self.dir.join(LOCK);
        let lock = OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|error| failed(&path, error))?;
        lock.lock().map_err(|error| failed(&path, error))?;
        let result = operation();
        // Closing the file releases the lock; an unlock error changes nothing about the result.
        let _ = lock.unlock();
        result
    }

    /// Writes `text` to a new temporary file in the store's directory, syncs it, and renames it
    /// over `path`. On any failure the temporary file is removed and `path` is untouched.
    fn replace(&self, path: &Path, text: &str) -> Result<(), SuspensionStoreError> {
        let mut attempts = 0;
        let (temporary, mut file) = loop {
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
                Ok(file) => break (temporary, file),
                Err(error)
                    if error.kind() == io::ErrorKind::AlreadyExists
                        && attempts < TEMPORARY_ATTEMPTS =>
                {
                    attempts += 1;
                }
                Err(error) => return Err(failed(&temporary, error)),
            }
        };
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
        if let Ok(dir) = File::open(&self.dir) {
            let _ = dir.sync_all();
        }
        Ok(())
    }
}

impl FileSuspensionStore {
    /// The Suspension stored for `commission` in `path`, or `None` when there is no such file. Only
    /// called holding the lock.
    fn read(
        path: &Path,
        commission: &CommissionId,
    ) -> Result<Option<SuspensionSnapshot>, SuspensionStoreError> {
        let Some(text) = read_if_present(path)? else {
            return Ok(None);
        };
        let held = json::parse(&text)
            .map_err(|error| failed(path, error))
            .and_then(|value| decode(&value).map_err(|error| failed(path, error)))?;
        if held.data.commission_id != *commission {
            return Err(failed(
                path,
                format!(
                    "holds the Suspension of commission `{}`, not `{}`",
                    held.data.commission_id.0.0, commission.0.0
                ),
            ));
        }
        Ok(Some(held))
    }

    /// Enacts `write` on `path`. Only called holding the lock. A Suspension that would not read
    /// back exactly is refused before anything is written.
    fn enact(&self, path: &Path, write: SuspensionWrite) -> Result<(), SuspensionStoreError> {
        match write {
            SuspensionWrite::Put(snapshot) => {
                let mut text = String::new();
                encode(&mut text, &snapshot);
                let reread = json::parse(&text)
                    .ok()
                    .and_then(|value| decode(&value).ok());
                if reread.as_ref() != Some(&snapshot) {
                    return Err(failed(
                        path,
                        "the Suspension would not read back as it is held; nothing was written",
                    ));
                }
                self.replace(path, &text)
            }
            SuspensionWrite::Delete(_) => match fs::remove_file(path) {
                Ok(()) => {
                    if let Ok(dir) = File::open(&self.dir) {
                        let _ = dir.sync_all();
                    }
                    Ok(())
                }
                Err(error) => Err(failed(path, error)),
            },
        }
    }

    /// Decides one command over what is stored for `commission` with `decide`, and enacts its
    /// write, all holding the lock. A store that cannot read or write answers `refused`.
    fn command<O>(
        &self,
        commission: &CommissionId,
        decide: impl FnOnce(Option<SuspensionSnapshot>) -> (O, Option<SuspensionWrite>),
        refused: impl FnOnce(SuspensionStorageFailed) -> O,
    ) -> O {
        let answered = self.path(commission).and_then(|path| {
            self.locked(|| {
                let (outcome, write) = decide(Self::read(&path, commission)?);
                if let Some(write) = write {
                    self.enact(&path, write)?;
                }
                Ok(outcome)
            })
        });
        answered.unwrap_or_else(|error| refused(error.refusal()))
    }
}

impl SuspensionPort for FileSuspensionStore {
    fn hold(&self, input: HoldSuspension) -> HoldSuspensionOutcome {
        let refused = |error| HoldSuspensionOutcome::StorageFailed { error };
        if input.suspension.commission_id != input.commission_id {
            return refused(
                SuspensionStoreError::new(
                    "the Suspension's identity names another commission than its `commission_id`",
                )
                .refusal(),
            );
        }
        let commission = input.commission_id.clone();
        self.command(
            &commission,
            |held| decide_hold(held.as_ref(), input),
            refused,
        )
    }

    fn take(&self, input: TakeSuspension) -> TakeSuspensionOutcome {
        let commission = input.suspension.commission_id.clone();
        self.command(
            &commission,
            |held| decide_take(held, input),
            |error| TakeSuspensionOutcome::StorageFailed { error },
        )
    }

    fn release(&self, input: ReleaseSuspension) -> ReleaseSuspensionOutcome {
        let commission = input.suspension.commission_id.clone();
        self.command(
            &commission,
            |held| decide_release(held, input),
            |error| ReleaseSuspensionOutcome::StorageFailed { error },
        )
    }

    fn get(
        &self,
        commission: &CommissionId,
    ) -> Result<Option<SuspensionSnapshot>, SuspensionStoreError> {
        let path = self.path(commission)?;
        self.locked(|| Self::read(&path, commission))
    }
}

/// The file's text, or `None` when there is no such file.
fn read_if_present(path: &Path) -> Result<Option<String>, SuspensionStoreError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(failed(path, error)),
    }
}

fn encode(out: &mut String, snapshot: &SuspensionSnapshot) {
    let data = &snapshot.data;
    out.push('{');
    member(out, "state");
    push_text(
        out,
        match snapshot.state {
            SuspensionState::Pending => "Pending",
            SuspensionState::Resuming => "Resuming",
        },
    );
    member(out, "commission_id");
    push_text(out, &data.commission_id.0.0);
    member(out, "run_id");
    push_text(out, &data.run_id.0.0);
    member(out, "run_case_revision");
    push_integer(out, data.run_case_revision);
    member(out, "action_request_id");
    push_text(out, &data.action_request_id.0.0);
    member(out, "case_id");
    push_text(out, &data.case_id.0);
    member(out, "expected_case_revision");
    push_integer(out, data.expected_case_revision);
    member(out, "action");
    push_text(out, &data.action);
    member(out, "arguments");
    push_value(out, &data.arguments.0);
    member(out, "reason");
    encode_reason(out, &data.reason);
    out.push('}');
}

/// A reason as `{"kind": <variant>, "payload": <its payload>}`.
fn encode_reason(out: &mut String, reason: &SuspensionReason) {
    out.push('{');
    member(out, "kind");
    let (kind, payload) = match reason {
        SuspensionReason::Authority(value) => ("Authority", value.clone()),
        SuspensionReason::Human(request) => ("Human", request.0.clone()),
        SuspensionReason::Evidence(requirements) => (
            "Evidence",
            Value::Array(requirements.iter().cloned().map(Value::Text).collect()),
        ),
        SuspensionReason::Time(value) => ("Time", value.clone()),
        SuspensionReason::Dependency(cases) => (
            "Dependency",
            Value::Array(
                cases
                    .iter()
                    .map(|case| Value::Text(case.0.clone()))
                    .collect(),
            ),
        ),
        SuspensionReason::Budget(value) => ("Budget", value.clone()),
        SuspensionReason::ExternalAvailability(value) => ("ExternalAvailability", value.clone()),
    };
    push_text(out, kind);
    member(out, "payload");
    push_value(out, &payload);
    out.push('}');
}

/// Refuses an object with a member `declared` does not name, or one member twice.
fn exactly(value: &Value, at: &str, declared: &[&str]) -> Result<(), DecodeError> {
    let members = members_at(value, at, "an object")?;
    for (index, (name, _)) in members.iter().enumerate() {
        if !declared.contains(&name.as_str()) {
            return Err(DecodeError {
                at: nested(at, name),
                expected: "no such member".to_owned(),
                found: "a member the declaration does not name".to_owned(),
            });
        }
        if members[..index].iter().any(|(earlier, _)| earlier == name) {
            return Err(DecodeError {
                at: nested(at, name),
                expected: "one member of this name".to_owned(),
                found: "a second one".to_owned(),
            });
        }
    }
    Ok(())
}

fn uuid(value: &Value, name: &str) -> Result<Uuid, DecodeError> {
    let at = nested("", name);
    uuid_at(member_at(value, "", name)?, &at, "a UUID").map(|uuid| Uuid(uuid.to_owned()))
}

fn text(value: &Value, name: &str) -> Result<String, DecodeError> {
    let at = nested("", name);
    text_at(member_at(value, "", name)?, &at, "a string").map(str::to_owned)
}

fn integer(value: &Value, name: &str) -> Result<i64, DecodeError> {
    let at = nested("", name);
    integer_at(member_at(value, "", name)?, &at, "an integer")
}

fn decode(value: &Value) -> Result<SuspensionSnapshot, DecodeError> {
    exactly(value, "", &MEMBERS)?;
    let state = match text(value, "state")?.as_str() {
        "Pending" => SuspensionState::Pending,
        "Resuming" => SuspensionState::Resuming,
        _ => {
            return Err(DecodeError::of(
                &nested("", "state"),
                "a Suspension state",
                member_at(value, "", "state")?,
            ));
        }
    };
    let commission_id = CommissionId(uuid(value, "commission_id")?);
    let data = SuspensionData {
        suspension: SuspensionKey {
            commission_id: commission_id.clone(),
        },
        commission_id,
        run_id: RunId(uuid(value, "run_id")?),
        run_case_revision: integer(value, "run_case_revision")?,
        action_request_id: ActionRequestId(uuid(value, "action_request_id")?),
        case_id: CaseId(text(value, "case_id")?),
        expected_case_revision: integer(value, "expected_case_revision")?,
        action: text(value, "action")?,
        arguments: ProposedActionArguments(member_at(value, "", "arguments")?.clone()),
        reason: decode_reason(member_at(value, "", "reason")?)?,
    };
    Ok(SuspensionSnapshot { state, data })
}

fn decode_reason(value: &Value) -> Result<SuspensionReason, DecodeError> {
    let at = "reason";
    exactly(value, at, &["kind", "payload"])?;
    let kind = text_at(
        member_at(value, at, "kind")?,
        &nested(at, "kind"),
        "a reason kind",
    )?;
    let payload = member_at(value, at, "payload")?.clone();
    let strings = |payload: &Value| -> Result<Vec<String>, DecodeError> {
        let path = nested(at, "payload");
        items_at(payload, &path, "an array of strings")?
            .iter()
            .map(|item| text_at(item, &path, "a string").map(str::to_owned))
            .collect()
    };
    Ok(match kind {
        "Authority" => SuspensionReason::Authority(payload),
        "Human" => SuspensionReason::Human(HumanDecisionRequest(payload)),
        "Evidence" => SuspensionReason::Evidence(strings(&payload)?),
        "Time" => SuspensionReason::Time(payload),
        "Dependency" => {
            SuspensionReason::Dependency(strings(&payload)?.into_iter().map(CaseId).collect())
        }
        "Budget" => SuspensionReason::Budget(payload),
        "ExternalAvailability" => SuspensionReason::ExternalAvailability(payload),
        _ => {
            return Err(DecodeError::of(
                &nested(at, "kind"),
                "a SuspensionReason variant",
                member_at(value, at, "kind")?,
            ));
        }
    })
}
