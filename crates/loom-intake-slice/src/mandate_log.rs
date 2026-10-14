//! The Mandate approval event log a state directory keeps beside its grants
//! (`intake.routing.MandateApprovalEntry`, `ess/intake/domains/routing.yaml`).
//!
//! Mandate v0.9.0 records no approval itself, and a consumption persists only when its caller keeps
//! the `ApprovedDecisionRecorded` event `check_with_approval` returns. [`MandateApprovalLog`] is
//! that keeping: `mandate-approvals.jsonl`, one entry per line in the order appended, holding the
//! approvals `approve --mandate` records and the events `resume --mandate` keeps. What an entry
//! means to Mandate is `loom_mandate::approval`'s; this module only reads and writes the file.
//!
//! Every operation holds the directory's lock, the one the commission's Suspension and the grants
//! are kept under ([`crate::state_dir`]). An append reads the log, decides the entry over what it
//! read ([`MandateApprovalLog::append_with`]) and writes the whole log with the entry to a
//! temporary file renamed over it, still holding the lock, so of two resumes presenting one
//! approval the second reads the first's consumption. No entry is ever changed or removed. A line
//! that does not read back as an entry makes the whole log unreadable, so nothing is decided over
//! part of it.

use std::path::{Path, PathBuf};

use intake_model::json as intake_json;
use intake_model::primitives::{Timestamp, Uuid};
use intake_model::routing::{
    ActionRequestId, CommissionId, MandateApprovalEntry, MandateApprovalId,
    MandateApprovalRecorded, MandateApprovedDecision,
};
use serde_json::{Map, Value, json};

use crate::grant::{exactly, text_of};
use crate::resume::ResumeStoreError;
use crate::state_dir::{StateDir, failed, read_text};

/// The log's file name in the state directory.
pub const FILE: &str = "mandate-approvals.jsonl";

/// The tag of an approval `approve --mandate` recorded.
const APPROVAL_RECORDED: &str = "ApprovalRecorded";

/// The tag of an `ApprovedDecisionRecorded` a resume kept.
const APPROVED_DECISION_RECORDED: &str = "ApprovedDecisionRecorded";

/// The members of an `ApprovalRecorded` entry, `actor` aside, which is absent when there is none.
const RECORDED: [&str; 13] = [
    "kind",
    "approval",
    "commission_id",
    "action_request_id",
    "organization",
    "subject",
    "action",
    "resource_type",
    "resource_id",
    "approver",
    "recorded_at",
    "expires_at",
    "actor",
];

/// The members of an `ApprovedDecisionRecorded` entry.
const DECIDED: [&str; 7] = [
    "kind",
    "approval",
    "commission_id",
    "action_request_id",
    "context",
    "decision",
    "recorded_at",
];

/// The approval event log of one state directory.
#[derive(Debug)]
pub struct MandateApprovalLog {
    dir: StateDir,
    path: PathBuf,
}

impl MandateApprovalLog {
    /// The log of `dir`, the directory created with its lock file when it does not exist yet and
    /// proven to take a write, as the other stores of the directory open it.
    ///
    /// # Errors
    /// [`ResumeStoreError`] when the directory or its lock file cannot be created, or the directory
    /// does not take a write.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, ResumeStoreError> {
        let dir = StateDir::open(dir.into(), "mandate")?;
        let path = dir.dir().join(FILE);
        Ok(Self { dir, path })
    }

    /// The log's file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every entry, in the order appended.
    ///
    /// # Errors
    /// [`ResumeStoreError`] when the file cannot be read or a line is no entry.
    pub fn entries(&self) -> Result<Vec<MandateApprovalEntry>, ResumeStoreError> {
        self.dir.locked(|| self.read())?
    }

    /// Appends `entry`.
    ///
    /// # Errors
    /// [`ResumeStoreError`] when the log cannot be read, the entry would not read back as written,
    /// or the file cannot be written; nothing is appended then.
    pub fn append(&self, entry: &MandateApprovalEntry) -> Result<(), ResumeStoreError> {
        self.append_with(|_| Ok::<_, ResumeStoreError>(entry.clone()))?
            .map(|_| ())
    }

    /// Holding the lock, reads every entry, asks `decide` for the entry to append over them, and
    /// appends it; `decide`'s refusal appends nothing. The outer error is the log's.
    ///
    /// # Errors
    /// [`ResumeStoreError`] when the log cannot be read, the entry would not read back as written,
    /// or the file cannot be written; nothing is appended then.
    pub fn append_with<E>(
        &self,
        decide: impl FnOnce(&[MandateApprovalEntry]) -> Result<MandateApprovalEntry, E>,
    ) -> Result<Result<MandateApprovalEntry, E>, ResumeStoreError> {
        self.dir.locked(|| {
            let entries = self.read()?;
            let entry = match decide(&entries) {
                Ok(entry) => entry,
                Err(refusal) => return Ok(Err(refusal)),
            };
            // The entry as it reads back: a JSON member (`context`, `decision`) is written with
            // its members in name order, so the entry is compared as written, not as given.
            let line = encode(&entry);
            let entry = match decode(&line) {
                Ok(read) if encode(&read) == line => read,
                _ => {
                    return Err(failed(
                        &self.path,
                        "the entry would not read back as written",
                    ));
                }
            };
            let mut text = read_text(&self.path)?.unwrap_or_default();
            text.push_str(&line);
            text.push('\n');
            self.dir.replace(&self.path, &text)?;
            Ok(Ok(entry))
        })?
    }

    /// Every entry of the file. Only called holding the lock.
    fn read(&self) -> Result<Vec<MandateApprovalEntry>, ResumeStoreError> {
        let Some(text) = read_text(&self.path)? else {
            return Ok(Vec::new());
        };
        text.lines()
            .enumerate()
            .map(|(index, line)| {
                decode(line)
                    .map_err(|problem| failed(&self.path, format!("line {}: {problem}", index + 1)))
            })
            .collect()
    }
}

/// `entry` as one line of the log.
fn encode(entry: &MandateApprovalEntry) -> String {
    let value = match entry {
        MandateApprovalEntry::ApprovalRecorded(recorded) => {
            let mut value = json!({
                "kind": APPROVAL_RECORDED,
                "approval": recorded.approval.0.0,
                "commission_id": recorded.commission_id.0.0,
                "action_request_id": recorded.action_request_id.0.0,
                "organization": recorded.organization.0,
                "subject": recorded.subject.0,
                "action": recorded.action,
                "resource_type": recorded.resource_type,
                "resource_id": recorded.resource_id.0,
                "approver": recorded.approver.0,
                "recorded_at": recorded.recorded_at.0,
                "expires_at": recorded.expires_at.0,
            });
            if let Some(actor) = &recorded.actor {
                value["actor"] = json!(actor.0);
            }
            value
        }
        MandateApprovalEntry::ApprovedDecisionRecorded(decided) => json!({
            "kind": APPROVED_DECISION_RECORDED,
            "approval": decided.approval.0.0,
            "commission_id": decided.commission_id.0.0,
            "action_request_id": decided.action_request_id.0.0,
            "context": serde_value(&decided.context),
            "decision": serde_value(&decided.decision),
            "recorded_at": decided.recorded_at.0,
        }),
    };
    value.to_string()
}

/// The entry `line` holds, refused unless it names exactly the members its kind declares.
fn decode(line: &str) -> Result<MandateApprovalEntry, String> {
    let value: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or("the entry names no `kind`")?;
    match kind {
        APPROVAL_RECORDED => {
            let object = if value.get("actor").is_some() {
                exactly(&value, &RECORDED)?
            } else {
                exactly(&value, &RECORDED[..RECORDED.len() - 1])?
            };
            Ok(MandateApprovalEntry::ApprovalRecorded(
                MandateApprovalRecorded {
                    approval: MandateApprovalId(Uuid(text_of(object, "approval")?)),
                    commission_id: CommissionId(Uuid(text_of(object, "commission_id")?)),
                    action_request_id: ActionRequestId(Uuid(text_of(object, "action_request_id")?)),
                    organization: Uuid(text_of(object, "organization")?),
                    subject: Uuid(text_of(object, "subject")?),
                    actor: match object.get("actor") {
                        Some(_) => Some(Uuid(text_of(object, "actor")?)),
                        None => None,
                    },
                    action: text_of(object, "action")?,
                    resource_type: text_of(object, "resource_type")?,
                    resource_id: Uuid(text_of(object, "resource_id")?),
                    approver: Uuid(text_of(object, "approver")?),
                    recorded_at: Timestamp(text_of(object, "recorded_at")?),
                    expires_at: Timestamp(text_of(object, "expires_at")?),
                },
            ))
        }
        APPROVED_DECISION_RECORDED => {
            let object = exactly(&value, &DECIDED)?;
            Ok(MandateApprovalEntry::ApprovedDecisionRecorded(
                MandateApprovedDecision {
                    approval: MandateApprovalId(Uuid(text_of(object, "approval")?)),
                    commission_id: CommissionId(Uuid(text_of(object, "commission_id")?)),
                    action_request_id: ActionRequestId(Uuid(text_of(object, "action_request_id")?)),
                    context: model_value(object, "context")?,
                    decision: model_value(object, "decision")?,
                    recorded_at: Timestamp(text_of(object, "recorded_at")?),
                },
            ))
        }
        other => Err(format!("the entry's kind `{other}` is not declared")),
    }
}

/// A generated JSON value as `serde_json`'s.
fn serde_value(value: &intake_json::Value) -> Value {
    let mut text = String::new();
    intake_json::push_value(&mut text, value);
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

/// The member `name` of `object`, an object, as a generated JSON value.
fn model_value(object: &Map<String, Value>, name: &str) -> Result<intake_json::Value, String> {
    let member = &object[name];
    if !member.is_object() {
        return Err(format!("`{name}` is not an object"));
    }
    intake_json::parse(&member.to_string()).map_err(|error| format!("`{name}`: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recorded(approval: &str) -> MandateApprovalEntry {
        MandateApprovalEntry::ApprovalRecorded(MandateApprovalRecorded {
            approval: MandateApprovalId(Uuid(approval.to_owned())),
            commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000001".into())),
            action_request_id: ActionRequestId(Uuid("00000000-0000-4000-8000-000000000002".into())),
            organization: Uuid("01010101-0101-0101-0101-010101010101".into()),
            subject: Uuid("02020202-0202-0202-0202-020202020202".into()),
            actor: None,
            action: "repository.merge".into(),
            resource_type: "loom.case".into(),
            resource_id: Uuid("03030303-0303-5303-8303-030303030303".into()),
            approver: Uuid("04040404-0404-0404-0404-040404040404".into()),
            recorded_at: Timestamp("2026-10-14T09:00:00Z".into()),
            expires_at: Timestamp("2026-10-14T10:00:00Z".into()),
        })
    }

    /// A kept decision's JSON members are appended whatever order their members came in, and the
    /// entry answered is the one that reads back.
    #[test]
    fn a_kept_decision_reads_back_as_appended() {
        let dir = tempfile::Builder::new()
            .prefix("mandate-log-decision-")
            .tempdir()
            .unwrap();
        let log = MandateApprovalLog::open(dir.path()).unwrap();
        let decided = MandateApprovalEntry::ApprovedDecisionRecorded(MandateApprovedDecision {
            approval: MandateApprovalId(Uuid("00000000-0000-4000-8000-00000000000a".into())),
            commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000001".into())),
            action_request_id: ActionRequestId(Uuid("00000000-0000-4000-8000-000000000002".into())),
            context: intake_json::parse(r#"{"subject":"s","audience":"loom"}"#).unwrap(),
            decision: intake_json::parse(r#"{"allowed":true,"reason":"Allowed","revision":7}"#)
                .unwrap(),
            recorded_at: Timestamp("2026-10-14T09:00:00Z".into()),
        });
        let appended = log
            .append_with(|_| Ok::<_, String>(decided.clone()))
            .unwrap()
            .unwrap();
        assert_eq!(log.entries().unwrap(), vec![appended.clone()]);
        assert_eq!(encode(&appended), encode(&decided));
    }

    /// Entries read back in the order appended, a refused decision appends nothing, and a line that
    /// is no entry makes the log unreadable rather than shorter.
    #[test]
    fn entries_read_back_in_order_and_a_foreign_line_is_refused() {
        let dir = tempfile::Builder::new()
            .prefix("mandate-log-")
            .tempdir()
            .unwrap();
        let log = MandateApprovalLog::open(dir.path()).unwrap();
        assert_eq!(log.entries().unwrap(), Vec::new());
        let first = recorded("00000000-0000-4000-8000-00000000000a");
        log.append(&first).unwrap();
        let mut second = recorded("00000000-0000-4000-8000-00000000000b");
        if let MandateApprovalEntry::ApprovalRecorded(entry) = &mut second {
            entry.actor = Some(Uuid("05050505-0505-0505-0505-050505050505".into()));
        }
        let appended = log
            .append_with(|entries| {
                assert_eq!(entries, std::slice::from_ref(&first));
                Ok::<_, String>(second.clone())
            })
            .unwrap();
        assert_eq!(appended, Ok(second.clone()));
        let refused = log
            .append_with(|_| Err::<MandateApprovalEntry, _>("refused".to_owned()))
            .unwrap();
        assert_eq!(refused, Err("refused".to_owned()));
        assert_eq!(log.entries().unwrap(), vec![first, second]);

        let mut text = std::fs::read_to_string(log.path()).unwrap();
        text.push_str("{\"kind\":\"ApprovalRecorded\",\"note\":\"x\"}\n");
        std::fs::write(log.path(), text).unwrap();
        assert!(log.entries().is_err());
        assert!(
            log.append(&recorded("00000000-0000-4000-8000-00000000000c"))
                .is_err()
        );
    }
}
