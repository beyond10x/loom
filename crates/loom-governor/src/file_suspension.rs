//! [`FileSuspensionStore`]: Commission's suspension port in a directory, so a run suspended on an
//! approval outlives the process that suspended it.
//!
//! A host passes it to `run_with_suspension` and `resume_after_grant` as their [`SuspensionPort`].
//! Each commission's Suspension is one file, `suspension-<hex of the commission id>.json`, holding
//! the Suspension's state (`Pending` or `Resuming`) and fields as `ess/commission/` declares them,
//! as JSON written with the generated JSON writer and read with its reader.
//! `HoldSuspension`, `TakeSuspension` and `ReleaseSuspension` are decided by Commission's own
//! decisions (`decide_hold`, `decide_take`, `decide_release`) over the file as it is read, and
//! their write made, under one hold of the directory's lock, so of two resumes over one directory
//! only one takes a Suspension, and a take or a release naming another request than the stored
//! Suspension's is refused (`other-request`) and changes nothing. A store that cannot read or write
//! answers the command's `storage-failed` and changes nothing. Every write goes to a fresh
//! temporary file in the same directory, is synced and renamed over its target, so a reader sees
//! the previous document or the next one and never part of one; a failed write leaves what was
//! stored before. Before anything is written it is read back and compared with what was meant, so
//! a Suspension that would not come back exactly is refused rather than written. A document naming
//! a member its declaration does not, or one member twice, is refused on read, as is one holding
//! another commission's Suspension. Temporary files a dead process left are removed when a store
//! opens.
//!
//! Every operation holds an exclusive lock on `<dir>/.lock` for its duration, the lock
//! [`FileCaseStore`](crate::FileCaseStore) takes, through the same code. A suspension store and a
//! case store may share one directory, in one process or in two: their file names never meet and
//! neither reads or writes the other's files.
//!
//! A commission id over [`LONGEST_COMMISSION_ID`] bytes is refused: its file name would pass the
//! 255 bytes a file name may have.

use std::path::{Path, PathBuf};

use b10x_loom_commission::model::json::{
    self, DecodeError, Value, integer_at, items_at, member, member_at, members_at, nested,
    push_integer, push_text, push_value, text_at, uuid_at,
};
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, BudgetLimit, CaseId, CommissionId, HoldSuspension, HoldSuspensionOutcome,
    HumanDecisionRequest, ProposedActionArguments, ReleaseSuspension, ReleaseSuspensionOutcome,
    RunBudgetBreach, RunId, SuspensionData, SuspensionKey, SuspensionReason, SuspensionSnapshot,
    SuspensionState, SuspensionStorageFailed, TakeSuspension, TakeSuspensionOutcome,
};
use b10x_loom_commission::ports::suspension::{
    SuspensionPort, SuspensionStoreError, SuspensionWrite, decide_hold, decide_release, decide_take,
};

use crate::FileStoreError;
use crate::locked_dir::{LockedDir, read_if_present};

/// The longest commission id, in bytes, the store keeps: `suspension-`, 200 hex digits and `.json`
/// is 216 bytes.
pub const LONGEST_COMMISSION_ID: usize = 100;

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
    dir: LockedDir,
}

/// The port's error for a store failure: the file and what went wrong with it.
fn storage(error: FileStoreError) -> SuspensionStoreError {
    SuspensionStoreError::new(error.to_string())
}

fn failed(path: &Path, problem: impl std::fmt::Display) -> SuspensionStoreError {
    SuspensionStoreError::new(format!("{}: {problem}", path.display()))
}

impl FileSuspensionStore {
    /// A store over `dir`, created when it does not exist yet. `dir` may be the directory a
    /// [`FileCaseStore`](crate::FileCaseStore) keeps its cases in.
    ///
    /// # Errors
    ///
    /// [`SuspensionStoreError`] when the directory or its lock file cannot be created, or a
    /// temporary file an earlier life left behind cannot be removed, or the directory does not
    /// take a write: a temporary file cannot be created, written, synced and removed in it.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, SuspensionStoreError> {
        LockedDir::open(dir.into())
            .map(|dir| Self { dir })
            .map_err(storage)
    }

    /// The directory the store keeps its files in.
    pub fn dir(&self) -> &Path {
        self.dir.path()
    }

    /// The file that holds `commission`'s Suspension, named so no id names a file outside the
    /// store.
    fn path(&self, commission: &CommissionId) -> Result<PathBuf, SuspensionStoreError> {
        let id = commission.0.0.as_bytes();
        if id.len() > LONGEST_COMMISSION_ID {
            return Err(SuspensionStoreError::new(format!(
                "a commission id of {} bytes is longer than the {LONGEST_COMMISSION_ID} this store \
                 keeps",
                id.len()
            )));
        }
        let mut name = String::from("suspension-");
        for byte in id {
            name.push_str(&format!("{byte:02x}"));
        }
        name.push_str(".json");
        Ok(self.dir.path().join(name))
    }

    /// Runs `operation` holding the directory's lock.
    fn locked<T>(
        &self,
        operation: impl FnOnce() -> Result<T, SuspensionStoreError>,
    ) -> Result<T, SuspensionStoreError> {
        self.dir.locked(operation).map_err(storage)?
    }

    /// The Suspension stored for `commission` in `path`, or `None` when there is no such file. Only
    /// called holding the lock.
    fn read(
        path: &Path,
        commission: &CommissionId,
    ) -> Result<Option<SuspensionSnapshot>, SuspensionStoreError> {
        let Some(text) = read_if_present(path).map_err(storage)? else {
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
                if reread.as_ref() != Some(&*snapshot) {
                    return Err(failed(
                        path,
                        "the Suspension would not read back as it is held; nothing was written",
                    ));
                }
                self.dir.replace(path, &text).map_err(storage)
            }
            SuspensionWrite::Delete(_) => self.dir.remove(path).map_err(storage),
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
        SuspensionReason::LoopBudget(value) => ("LoopBudget", value.clone()),
        SuspensionReason::ExternalAvailability(value) => ("ExternalAvailability", value.clone()),
        SuspensionReason::RunBudget(breach) => ("RunBudget", breach_payload(breach)),
    };
    push_text(out, kind);
    member(out, "payload");
    push_value(out, &payload);
    out.push('}');
}

/// The names of `BudgetLimit`, as `ess/commission/` declares them.
const LIMITS: [(BudgetLimit, &str); 8] = [
    (BudgetLimit::MaxSteps, "MaxSteps"),
    (BudgetLimit::MaxTrials, "MaxTrials"),
    (BudgetLimit::MaxTokensIn, "MaxTokensIn"),
    (BudgetLimit::MaxTokensOut, "MaxTokensOut"),
    (BudgetLimit::MaxDurationMs, "MaxDurationMs"),
    (BudgetLimit::MaxCostMicroUsd, "MaxCostMicroUsd"),
    (BudgetLimit::MaxTokens, "MaxTokens"),
    (BudgetLimit::Deadline, "Deadline"),
];

/// A `RunBudgetBreach` as an object of its fields, an absent optional field left out.
fn breach_payload(breach: &RunBudgetBreach) -> Value {
    let mut members = Vec::new();
    if let Some(action) = &breach.action {
        members.push(("action".to_owned(), Value::Text(action.clone())));
    }
    let limit = LIMITS
        .iter()
        .find(|(limit, _)| *limit == breach.limit)
        .map_or("", |(_, name)| name);
    members.push(("limit".to_owned(), Value::Text(limit.to_owned())));
    members.push(("max".to_owned(), Value::Number(breach.max.to_string())));
    if let Some(charged) = breach.charged {
        members.push(("charged".to_owned(), Value::Number(charged.to_string())));
    }
    Value::Object(members)
}

/// The `RunBudgetBreach` [`breach_payload`] wrote.
fn decode_breach(payload: &Value) -> Result<RunBudgetBreach, DecodeError> {
    let at = "reason.payload";
    exactly(payload, at, &["action", "limit", "max", "charged"])?;
    let optional = |name: &str| {
        members_at(payload, at, "an object").map(|members| {
            members
                .iter()
                .find(|(member, _)| member == name)
                .map(|(_, value)| value)
        })
    };
    let action = optional("action")?
        .map(|value| text_at(value, &nested(at, "action"), "a string").map(str::to_owned))
        .transpose()?;
    let limit_value = member_at(payload, at, "limit")?;
    let name = text_at(limit_value, &nested(at, "limit"), "a BudgetLimit name")?;
    let limit = LIMITS
        .iter()
        .find(|(_, known)| *known == name)
        .map(|(limit, _)| *limit)
        .ok_or_else(|| DecodeError::of(&nested(at, "limit"), "a BudgetLimit name", limit_value))?;
    let max = integer_at(
        member_at(payload, at, "max")?,
        &nested(at, "max"),
        "an integer",
    )?;
    let charged = optional("charged")?
        .map(|value| integer_at(value, &nested(at, "charged"), "an integer"))
        .transpose()?;
    Ok(RunBudgetBreach {
        action,
        limit,
        max,
        charged,
    })
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
        "LoopBudget" => SuspensionReason::LoopBudget(payload),
        "ExternalAvailability" => SuspensionReason::ExternalAvailability(payload),
        "RunBudget" => SuspensionReason::RunBudget(decode_breach(&payload)?),
        _ => {
            return Err(DecodeError::of(
                &nested(at, "kind"),
                "a SuspensionReason variant",
                member_at(value, at, "kind")?,
            ));
        }
    })
}
