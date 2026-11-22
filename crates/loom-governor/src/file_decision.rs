//! [`FileDecisionStore`]: the `DecisionStore` port of `b10x-loom-decision` in a directory, so a
//! decision request, and the records kept beside it, outlive the process that wrote them
//! (`story:decision-request-file-store`).
//!
//! Each request is one file, `decision-<hex of its id>.json`, or
//! `decision-sha256-<hex of its SHA-256>.json` for an id over 100 bytes, named as
//! [`case_file_stem`](crate::case_file_stem) names a case file, so no id names a file outside the
//! store and a longer id is kept, never refused. The file holds the request as the
//! `loom.governor` domain declares it ([`StoredDecisionRequest`], `ess/domains/governor.yaml`):
//! its place in the order requests were first stored, its state, the request field for field,
//! and the `EscalationSuppressedRecord`s appended beside it, in the order appended. The store
//! keeps the request and the records as the port hands them over and reads nothing into them.
//!
//! Every operation holds the directory's lock ([`LockedDir`]) for its duration, the lock
//! [`FileCaseStore`](crate::FileCaseStore) and [`FileSuspensionStore`](crate::FileSuspensionStore)
//! take, so a decision store may share one directory with them, in one process or in two; it
//! opens only its own `decision-` files. A write reads what is stored, compares it with the state
//! its caller read, checks the record key and writes, all under one hold of the lock: a write over
//! a request that changed since it was read, or of a new request whose id was stored since, is the
//! port's [`StoreConflict`] naming the state read and the state found, and one that would leave
//! two Open or Escalated requests with one `record_key` is a conflict naming the holder. An append
//! reads the request's file and writes it back with the record added under the same hold, so a
//! decision and two appends that race all land. Every write goes to a fresh temporary file in the
//! directory, is synced and renamed over its target, so a reader sees the previous file or the next
//! one and never part of one; a failed write leaves every byte of the previous file. Temporary
//! files a dead process left are removed when a store opens.
//!
//! A file is read only as this store writes it: one that does not decode, whose bytes are not the
//! ones the store writes for what it decodes to, that holds another request's id (it is not the
//! file that id names), that breaks an invariant of `loom.decision.DecisionRequest`, or that keeps
//! a record of another request is the port's [`StoreFailure`] naming the file, and it is never
//! repaired. The store has no key, so it cannot tell a file rewritten in the store's own spelling
//! from one it wrote.
//!
//! Beyond the port's comparison, the store refuses, as a [`StoreFailure`] that writes nothing, a
//! write that would decide a stored request again or change it once decided: a Decided request
//! moves only to Decided with its produced record recorded once, or to Superseded, each keeping
//! its decision and every other field: superseding it writes `superseded_by`, `superseded_reason`
//! and a produced record it did not hold yet, never its options, eligible principals, holder, case
//! or question. A Superseded or Expired request takes no write. A new request and an Open or
//! Escalated one take any write the port allows.

use std::fs;
use std::path::{Path, PathBuf};

use b10x_loom_decision::{AppendError, DecisionStore, StoreConflict, StoreFailure, WriteError};
use loom::decision::{
    ActionId, ArtifactId, ArtifactRevisionCitation, AutonomyLevel, CapabilityId, CaseId, Citation,
    Consequence, DecisionClassId, DecisionOption, DecisionRequestData, DecisionRequestId,
    DecisionRequestSnapshot, DecisionRequestState, EscalationSuppressedRecord, Grant, Ground,
    GroundSource, Holder, NoSuchRequest, OptionLetter, OutcomeId, PathLineCitation, Principal,
    PrincipalKind, ProducedDocument, ProducedRecord, ProposerKind, RecordCitation, RecordStore,
    Revision, Strength, SuppressionCause, UnblockKind, Unblocks,
};
use loom::governor::{StoredDecisionRequest, StoredDecisionRequestData};
use loom::json::{
    self, DecodeError, Value, bool_at, integer_at, items_at, member_at, members_at, nested,
    push_value, text_at,
};
use loom::primitives::Timestamp;
use sha2::{Digest as _, Sha256};

use crate::FileStoreError;
use crate::file_store::{HEX_NAMED, push_hex};
use crate::locked_dir::{LockedDir, read_if_present};

const PREFIX: &str = "decision-";
const EXTENSION: &str = ".json";

/// The name of the file a request with id `request` is kept in: `decision-<hex of the id>.json`
/// for an id of up to 100 bytes, `decision-sha256-<hex of its SHA-256>.json` for a longer one.
/// Only hexadecimal digits follow the prefix, so no id names a file outside the directory, and
/// the two forms never meet: a hex name has no `s` after `decision-`.
fn file_name(request: &str) -> String {
    let mut name = String::from(PREFIX);
    if request.len() <= HEX_NAMED {
        push_hex(&mut name, request.as_bytes());
    } else {
        name.push_str("sha256-");
        push_hex(&mut name, &Sha256::digest(request.as_bytes()));
    }
    name.push_str(EXTENSION);
    name
}

/// The port's failure for a file store's error: the file, and what went wrong with it.
fn failure(error: FileStoreError) -> StoreFailure {
    StoreFailure {
        at: error.path.display().to_string(),
        why: error.problem,
    }
}

fn failed(path: &Path, why: impl std::fmt::Display) -> StoreFailure {
    StoreFailure {
        at: path.display().to_string(),
        why: why.to_string(),
    }
}

fn is_open(state: DecisionRequestState) -> bool {
    matches!(
        state,
        DecisionRequestState::Open | DecisionRequestState::Escalated
    )
}

/// A [`DecisionStore`] keeping one file per decision request in a directory.
#[derive(Debug)]
pub struct FileDecisionStore {
    dir: LockedDir,
}

impl FileDecisionStore {
    /// A store over `dir`, created when it does not exist yet. `dir` may be the directory a
    /// [`FileCaseStore`](crate::FileCaseStore) or a
    /// [`FileSuspensionStore`](crate::FileSuspensionStore) keeps its files in.
    ///
    /// # Errors
    ///
    /// [`StoreFailure`] naming the path when the directory or its lock file cannot be created, a
    /// temporary file an earlier life left behind cannot be removed, or the directory does not
    /// take a write.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, StoreFailure> {
        LockedDir::open(dir.into())
            .map(|dir| Self { dir })
            .map_err(failure)
    }

    /// The directory the store keeps its files in.
    pub fn dir(&self) -> &Path {
        self.dir.path()
    }

    fn path(&self, request: &DecisionRequestId) -> PathBuf {
        self.dir.path().join(file_name(&request.0))
    }

    /// Runs `operation` holding the directory's lock; a lock that cannot be taken is `wrap`ped.
    fn locked<T, E>(
        &self,
        wrap: fn(StoreFailure) -> E,
        operation: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        self.dir
            .locked(operation)
            .map_err(|error| wrap(failure(error)))?
    }

    /// Every request file in the directory, in the order the requests were first stored. Only
    /// called holding the lock.
    fn all(&self) -> Result<Vec<StoredDecisionRequest>, StoreFailure> {
        let dir = self.dir.path();
        let entries = fs::read_dir(dir).map_err(|error| failed(dir, error))?;
        let mut held: Vec<(PathBuf, StoredDecisionRequest)> = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| failed(dir, error))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !(name.starts_with(PREFIX) && name.ends_with(EXTENSION)) {
                continue;
            }
            let path = entry.path();
            if let Some(stored) = held_file(&path)? {
                held.push((path, stored));
            }
        }
        held.sort_by_key(|(_, stored)| stored.position);
        for pair in held.windows(2) {
            if pair[0].1.position == pair[1].1.position {
                return Err(failed(
                    &pair[1].0,
                    format!(
                        "holds position {}, as `{}` does: the store gives each request its own",
                        pair[1].1.position,
                        pair[0].0.display()
                    ),
                ));
            }
        }
        Ok(held.into_iter().map(|(_, stored)| stored).collect())
    }

    /// Writes `stored` to `path`, refusing what would not read back exactly. Only called holding
    /// the lock.
    fn put(&self, path: &Path, stored: &StoredDecisionRequest) -> Result<(), StoreFailure> {
        let text = encode(stored);
        if checked(path, &text).ok().as_ref() != Some(stored) {
            return Err(failed(
                path,
                "the decision request would not read back as it is held; nothing was written",
            ));
        }
        self.dir.replace(path, &text).map_err(failure)
    }
}

fn kept(failure: StoreFailure) -> StoreFailure {
    failure
}

impl DecisionStore for FileDecisionStore {
    fn read(
        &self,
        request_id: &DecisionRequestId,
    ) -> Result<Option<DecisionRequestSnapshot>, StoreFailure> {
        let path = self.path(request_id);
        self.locked(kept, || Ok(held_file(&path)?.as_ref().map(snapshot)))
    }

    fn write(
        &mut self,
        read: Option<&DecisionRequestSnapshot>,
        next: DecisionRequestSnapshot,
    ) -> Result<(), WriteError> {
        let path = self.path(&next.data.request_id);
        self.locked(WriteError::Failure, || {
            let found = held_file(&path).map_err(WriteError::Failure)?;
            let found_snapshot = found.as_ref().map(snapshot);
            let conflict = |record_key_held_by| {
                WriteError::Conflict(StoreConflict {
                    request_id: next.data.request_id.clone(),
                    read: read.cloned().map(Box::new),
                    found: found_snapshot.clone().map(Box::new),
                    record_key_held_by,
                })
            };
            if found_snapshot.as_ref() != read {
                return Err(conflict(None));
            }
            if let Some(held) = &found_snapshot
                && let Some(why) = settled(held, &next)
            {
                return Err(WriteError::Failure(failed(&path, why)));
            }
            let all = if found.is_none() || is_open(next.state) {
                self.all().map_err(WriteError::Failure)?
            } else {
                Vec::new()
            };
            if is_open(next.state)
                && let Some(holder) = all.iter().find(|held| {
                    held.request.request_id != next.data.request_id
                        && is_open(held.state)
                        && held.request.record_key == next.data.record_key
                })
            {
                return Err(conflict(Some(holder.request.request_id.clone())));
            }
            let (position, records) = match found {
                Some(held) => (held.position, held.records),
                None => (
                    all.iter().map(|held| held.position).max().unwrap_or(0) + 1,
                    Vec::new(),
                ),
            };
            let stored = StoredDecisionRequest {
                position,
                state: next.state,
                request: stored_data(&next.data),
                records,
            };
            self.put(&path, &stored).map_err(WriteError::Failure)
        })
    }

    fn list(&self) -> Result<Vec<DecisionRequestSnapshot>, StoreFailure> {
        self.locked(kept, || Ok(self.all()?.iter().map(snapshot).collect()))
    }

    fn append_record(&mut self, record: EscalationSuppressedRecord) -> Result<(), AppendError> {
        let path = self.path(&record.request_id);
        self.locked(AppendError::Failure, || {
            let Some(mut held) = held_file(&path).map_err(AppendError::Failure)? else {
                return Err(AppendError::NoSuchRequest(NoSuchRequest {
                    request_id: record.request_id,
                }));
            };
            held.records.push(record);
            self.put(&path, &held).map_err(AppendError::Failure)
        })
    }

    fn records(
        &self,
        request_id: &DecisionRequestId,
    ) -> Result<Vec<EscalationSuppressedRecord>, StoreFailure> {
        let path = self.path(request_id);
        self.locked(kept, || {
            Ok(held_file(&path)?
                .map(|held| held.records)
                .unwrap_or_default())
        })
    }
}

/// Why a write of `next` over the stored `held` would decide it again or change it once decided,
/// or `None` when it does neither.
fn settled(held: &DecisionRequestSnapshot, next: &DecisionRequestSnapshot) -> Option<String> {
    use DecisionRequestState::{Decided, Expired, Superseded};
    let request = &held.data.request_id.0;
    match (held.state, next.state) {
        (Superseded | Expired, _) if next != held => Some(format!(
            "decision request {request} is {:?} and takes no further write",
            held.state
        )),
        (Decided, Decided | Superseded) => {
            let (was, now) = (&held.data, &next.data);
            let decision_kept = was.decided_option == now.decided_option
                && was.decided_answer == now.decided_answer
                && was.decided_by == now.decided_by
                && was.decided_by_kind == now.decided_by_kind
                && was.decided_at == now.decided_at
                && was.decided_case_revision == now.decided_case_revision
                && was.decision_reasons == now.decision_reasons;
            let record_kept =
                was.produced_record.is_none() || was.produced_record == now.produced_record;
            // What the write leaves of `now` once its own changes are taken back: a produced
            // record recorded for the first time and, for Superseded, the supersession.
            let mut unrecorded = now.clone();
            unrecorded.produced_record.clone_from(&was.produced_record);
            if next.state == Superseded {
                unrecorded.superseded_by.clone_from(&was.superseded_by);
                unrecorded
                    .superseded_reason
                    .clone_from(&was.superseded_reason);
            }
            if !decision_kept {
                Some(format!(
                    "decision request {request} is decided; its decision is never taken again"
                ))
            } else if !record_kept {
                Some(format!(
                    "decision request {request} has its produced record recorded; it is never \
                     replaced"
                ))
            } else if next.state == Decided && unrecorded != *was {
                Some(format!(
                    "decision request {request} is decided; a write records its produced record \
                     and changes nothing else"
                ))
            } else if unrecorded != *was {
                Some(format!(
                    "decision request {request} is decided; superseding it records the \
                     supersession and changes nothing else"
                ))
            } else {
                None
            }
        }
        (Decided, _) => Some(format!(
            "decision request {request} is decided and moves only to Superseded"
        )),
        _ => None,
    }
}

/// The request kept in `path`, or `None` when there is no such file. Only called holding the
/// lock.
fn held_file(path: &Path) -> Result<Option<StoredDecisionRequest>, StoreFailure> {
    let Some(text) = read_if_present(path).map_err(failure)? else {
        return Ok(None);
    };
    checked(path, &text).map(Some)
}

/// The request `text` holds, refused naming `path` unless it is exactly what this store writes
/// into the file its id names.
fn checked(path: &Path, text: &str) -> Result<StoredDecisionRequest, StoreFailure> {
    let unreadable = |why: &dyn std::fmt::Display| {
        failed(path, format!("does not read as a decision request: {why}"))
    };
    let stored = json::parse(text)
        .map_err(|error| unreadable(&error))
        .and_then(|value| decode(&value).map_err(|error| unreadable(&error)))?;
    if encode(&stored) != text {
        return Err(failed(
            path,
            "is not the document this store writes for what it holds: its bytes were changed",
        ));
    }
    let request = &stored.request.request_id.0;
    let own = file_name(request);
    if path.file_name().and_then(|name| name.to_str()) != Some(own.as_str()) {
        return Err(failed(
            path,
            format!("holds decision request `{request}`, which is kept in `{own}`"),
        ));
    }
    if let Some(other) = stored
        .records
        .iter()
        .find(|record| record.request_id != stored.request.request_id)
    {
        return Err(failed(
            path,
            format!(
                "keeps a record of decision request `{}` beside `{request}`",
                other.request_id.0
            ),
        ));
    }
    if let Some(broken) = snapshot(&stored).data.broken_invariant() {
        return Err(failed(
            path,
            format!("holds decision request `{request}`, which breaks {broken}"),
        ));
    }
    Ok(stored)
}

/// The snapshot the port hands over for `stored`.
fn snapshot(stored: &StoredDecisionRequest) -> DecisionRequestSnapshot {
    let StoredDecisionRequestData {
        request_id,
        case_id,
        question_key,
        record_key,
        unblocks,
        subject_artifact,
        subject_revision,
        subject_path,
        question,
        options,
        autonomy_level,
        decision_class,
        holder,
        eligible,
        addressed_to,
        opened_at,
        expires_at,
        supersedes,
        recommended_option,
        recommended_answer,
        recommendation_strength,
        default_on_silence,
        grounds,
        recommended_by,
        grounding_report,
        no_recommendation_reason,
        escalated_to,
        escalation_reason,
        decided_option,
        decided_answer,
        decided_by,
        decided_by_kind,
        decided_at,
        decided_case_revision,
        decision_reasons,
        produced_record,
        superseded_by,
        superseded_reason,
    } = stored.request.clone();
    DecisionRequestSnapshot {
        state: stored.state,
        data: DecisionRequestData {
            request_id,
            case_id,
            question_key,
            record_key,
            unblocks,
            subject_artifact,
            subject_revision,
            subject_path,
            question,
            options,
            autonomy_level,
            decision_class,
            holder,
            eligible,
            addressed_to,
            opened_at,
            expires_at,
            supersedes,
            recommended_option,
            recommended_answer,
            recommendation_strength,
            default_on_silence,
            grounds,
            recommended_by,
            grounding_report,
            no_recommendation_reason,
            escalated_to,
            escalation_reason,
            decided_option,
            decided_answer,
            decided_by,
            decided_by_kind,
            decided_at,
            decided_case_revision,
            decision_reasons,
            produced_record,
            superseded_by,
            superseded_reason,
        },
    }
}

/// The request's data as the governor domain declares it.
fn stored_data(data: &DecisionRequestData) -> StoredDecisionRequestData {
    let DecisionRequestData {
        request_id,
        case_id,
        question_key,
        record_key,
        unblocks,
        subject_artifact,
        subject_revision,
        subject_path,
        question,
        options,
        autonomy_level,
        decision_class,
        holder,
        eligible,
        addressed_to,
        opened_at,
        expires_at,
        supersedes,
        recommended_option,
        recommended_answer,
        recommendation_strength,
        default_on_silence,
        grounds,
        recommended_by,
        grounding_report,
        no_recommendation_reason,
        escalated_to,
        escalation_reason,
        decided_option,
        decided_answer,
        decided_by,
        decided_by_kind,
        decided_at,
        decided_case_revision,
        decision_reasons,
        produced_record,
        superseded_by,
        superseded_reason,
    } = data.clone();
    StoredDecisionRequestData {
        request_id,
        case_id,
        question_key,
        record_key,
        unblocks,
        subject_artifact,
        subject_revision,
        subject_path,
        question,
        options,
        autonomy_level,
        decision_class,
        holder,
        eligible,
        addressed_to,
        opened_at,
        expires_at,
        supersedes,
        recommended_option,
        recommended_answer,
        recommendation_strength,
        default_on_silence,
        grounds,
        recommended_by,
        grounding_report,
        no_recommendation_reason,
        escalated_to,
        escalation_reason,
        decided_option,
        decided_answer,
        decided_by,
        decided_by_kind,
        decided_at,
        decided_case_revision,
        decision_reasons,
        produced_record,
        superseded_by,
        superseded_reason,
    }
}

// ---- the document: every member always written, an absent optional one as `null` -------------

/// The names of a closed set, as `ess/domains/decision.yaml` and `governor.yaml` declare them.
type Names<T> = [(T, &'static str)];

const STATES: &Names<DecisionRequestState> = &[
    (DecisionRequestState::Open, "Open"),
    (DecisionRequestState::Escalated, "Escalated"),
    (DecisionRequestState::Decided, "Decided"),
    (DecisionRequestState::Superseded, "Superseded"),
    (DecisionRequestState::Expired, "Expired"),
];
const UNBLOCK_KINDS: &Names<UnblockKind> = &[
    (UnblockKind::Approval, "approval"),
    (UnblockKind::Question, "question"),
    (UnblockKind::Outcome, "outcome"),
];
const GRANTS: &Names<Grant> = &[(Grant::Granted, "granted"), (Grant::Denied, "denied")];
const PRODUCED_DOCUMENTS: &Names<ProducedDocument> = &[
    (ProducedDocument::CanonAuthority, "canon_authority"),
    (ProducedDocument::CanonDecisions, "canon_decisions"),
    (ProducedDocument::CanonEvidence, "canon_evidence"),
    (ProducedDocument::ArtifactRevision, "artifact_revision"),
];
const STRENGTHS: &Names<Strength> = &[
    (Strength::None, "none"),
    (Strength::Weak, "weak"),
    (Strength::Moderate, "moderate"),
    (Strength::Strong, "strong"),
];
const GROUND_SOURCES: &Names<GroundSource> = &[
    (GroundSource::Idea, "idea"),
    (GroundSource::Vision, "vision"),
    (GroundSource::Requirement, "requirement"),
    (GroundSource::PriorDecision, "prior_decision"),
    (GroundSource::Prd, "prd"),
    (GroundSource::Story, "story"),
    (GroundSource::Spec, "spec"),
    (GroundSource::Policy, "policy"),
    (GroundSource::EvidenceRecord, "evidence_record"),
    (GroundSource::AutonomyRule, "autonomy_rule"),
    (GroundSource::Goals, "goals"),
];
const RECORD_STORES: &Names<RecordStore> = &[
    (RecordStore::SdcDecisions, "sdc_decisions"),
    (RecordStore::CanonEvidence, "canon_evidence"),
    (RecordStore::CanonAuthority, "canon_authority"),
    (RecordStore::CanonDecisions, "canon_decisions"),
    (RecordStore::AepArtifact, "aep_artifact"),
];
const AUTONOMY_LEVELS: &Names<AutonomyLevel> = &[
    (AutonomyLevel::A0, "A0"),
    (AutonomyLevel::A1, "A1"),
    (AutonomyLevel::A2, "A2"),
    (AutonomyLevel::A3, "A3"),
    (AutonomyLevel::A4, "A4"),
    (AutonomyLevel::A5, "A5"),
];
const HOLDERS: &Names<Holder> = &[
    (Holder::Person, "person"),
    (Holder::Advised, "advised"),
    (Holder::Decider, "decider"),
    (Holder::DeciderFinal, "decider_final"),
    (Holder::Rule, "rule"),
    (Holder::RuleFinal, "rule_final"),
    (Holder::RuleAndDecider, "rule_and_decider"),
    (Holder::RuleAndDeciderFinal, "rule_and_decider_final"),
];
const PRINCIPAL_KINDS: &Names<PrincipalKind> = &[
    (PrincipalKind::Person, "person"),
    (PrincipalKind::Decider, "decider"),
    (PrincipalKind::Rule, "rule"),
    (PrincipalKind::Silence, "silence"),
];
const PROPOSER_KINDS: &Names<ProposerKind> = &[
    (ProposerKind::Rule, "rule"),
    (ProposerKind::Llm, "llm"),
    (ProposerKind::Person, "person"),
    (ProposerKind::PriorDecision, "prior_decision"),
];
const SUPPRESSION_CAUSES: &Names<SuppressionCause> = &[
    (SuppressionCause::Escalate, "escalate"),
    (SuppressionCause::Failure, "failure"),
    (SuppressionCause::Withheld, "withheld"),
    (SuppressionCause::Veto, "veto"),
    (SuppressionCause::RuleEscalate, "rule_escalate"),
];

fn name<T: Copy + PartialEq>(names: &Names<T>, value: T) -> Value {
    Value::Text(
        names
            .iter()
            .find(|(known, _)| *known == value)
            .map_or("", |(_, name)| *name)
            .to_owned(),
    )
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn integer(value: i64) -> Value {
    Value::Number(value.to_string())
}

fn optional<T>(value: Option<&T>, encode: impl FnOnce(&T) -> Value) -> Value {
    value.map_or(Value::Null, encode)
}

fn texts<'a>(values: impl IntoIterator<Item = &'a str>) -> Value {
    Value::Array(values.into_iter().map(text).collect())
}

fn object<const N: usize>(members: [(&str, Value); N]) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
    )
}

fn encode(stored: &StoredDecisionRequest) -> String {
    let value = object([
        ("position", integer(stored.position)),
        ("state", name(STATES, stored.state)),
        ("request", request_value(&stored.request)),
        (
            "records",
            Value::Array(stored.records.iter().map(record_value).collect()),
        ),
    ]);
    let mut out = String::new();
    push_value(&mut out, &value);
    out
}

fn request_value(data: &StoredDecisionRequestData) -> Value {
    let principal = |held: &Principal| text(&held.0);
    let letter = |held: &OptionLetter| text(&held.0);
    let instant = |held: &Timestamp| text(&held.0);
    let id = |held: &DecisionRequestId| text(&held.0);
    let revision = |held: &Revision| text(&held.0);
    let plain = |held: &String| text(held);
    object([
        ("request_id", id(&data.request_id)),
        ("case_id", text(&data.case_id.0)),
        ("question_key", text(&data.question_key)),
        ("record_key", text(&data.record_key)),
        ("unblocks", unblocks_value(&data.unblocks)),
        ("subject_artifact", text(&data.subject_artifact.0)),
        ("subject_revision", revision(&data.subject_revision)),
        ("subject_path", optional(data.subject_path.as_ref(), plain)),
        ("question", text(&data.question)),
        (
            "options",
            Value::Array(data.options.iter().map(option_value).collect()),
        ),
        ("autonomy_level", name(AUTONOMY_LEVELS, data.autonomy_level)),
        ("decision_class", text(&data.decision_class.0)),
        ("holder", name(HOLDERS, data.holder)),
        (
            "eligible",
            Value::Array(data.eligible.iter().map(principal).collect()),
        ),
        ("addressed_to", principal(&data.addressed_to)),
        ("opened_at", instant(&data.opened_at)),
        ("expires_at", optional(data.expires_at.as_ref(), instant)),
        ("supersedes", optional(data.supersedes.as_ref(), id)),
        (
            "recommended_option",
            optional(data.recommended_option.as_ref(), letter),
        ),
        (
            "recommended_answer",
            optional(data.recommended_answer.as_ref(), plain),
        ),
        (
            "recommendation_strength",
            name(STRENGTHS, data.recommendation_strength),
        ),
        ("default_on_silence", Value::Bool(data.default_on_silence)),
        (
            "grounds",
            Value::Array(data.grounds.iter().map(ground_value).collect()),
        ),
        (
            "recommended_by",
            optional(data.recommended_by.as_ref(), |kind| {
                name(PROPOSER_KINDS, *kind)
            }),
        ),
        (
            "grounding_report",
            optional(data.grounding_report.as_ref(), plain),
        ),
        (
            "no_recommendation_reason",
            optional(data.no_recommendation_reason.as_ref(), plain),
        ),
        (
            "escalated_to",
            optional(data.escalated_to.as_ref(), principal),
        ),
        (
            "escalation_reason",
            optional(data.escalation_reason.as_ref(), plain),
        ),
        (
            "decided_option",
            optional(data.decided_option.as_ref(), letter),
        ),
        (
            "decided_answer",
            optional(data.decided_answer.as_ref(), plain),
        ),
        ("decided_by", optional(data.decided_by.as_ref(), principal)),
        (
            "decided_by_kind",
            optional(data.decided_by_kind.as_ref(), |kind| {
                name(PRINCIPAL_KINDS, *kind)
            }),
        ),
        ("decided_at", optional(data.decided_at.as_ref(), instant)),
        (
            "decided_case_revision",
            optional(data.decided_case_revision.as_ref(), revision),
        ),
        (
            "decision_reasons",
            texts(data.decision_reasons.iter().map(String::as_str)),
        ),
        (
            "produced_record",
            optional(data.produced_record.as_ref(), produced_value),
        ),
        ("superseded_by", optional(data.superseded_by.as_ref(), id)),
        (
            "superseded_reason",
            optional(data.superseded_reason.as_ref(), plain),
        ),
    ])
}

fn unblocks_value(unblocks: &Unblocks) -> Value {
    object([
        ("kind", name(UNBLOCK_KINDS, unblocks.kind)),
        ("action", text(&unblocks.action.0)),
        (
            "capability",
            optional(unblocks.capability.as_ref(), |held| text(&held.0)),
        ),
        (
            "question_id",
            optional(unblocks.question_id.as_ref(), |held| text(held)),
        ),
        (
            "outcome",
            optional(unblocks.outcome.as_ref(), |held| text(&held.0)),
        ),
    ])
}

fn option_value(option: &DecisionOption) -> Value {
    let consequence = &option.consequence;
    object([
        ("letter", text(&option.letter.0)),
        ("does", text(&option.does)),
        ("cost", text(&option.cost)),
        (
            "consequence",
            object([
                (
                    "admits",
                    texts(consequence.admits.iter().map(|action| action.0.as_str())),
                ),
                (
                    "blocks",
                    texts(consequence.blocks.iter().map(|action| action.0.as_str())),
                ),
                (
                    "grant",
                    optional(consequence.grant.as_ref(), |grant| name(GRANTS, *grant)),
                ),
                (
                    "answer",
                    optional(consequence.answer.as_ref(), |held| text(held)),
                ),
                (
                    "produces",
                    optional(consequence.produces.as_ref(), |document| {
                        name(PRODUCED_DOCUMENTS, *document)
                    }),
                ),
            ]),
        ),
    ])
}

fn ground_value(ground: &Ground) -> Value {
    let (form, value) = match &ground.citation {
        Citation::PathLine(cited) => (
            "path_line",
            object([
                ("path", text(&cited.path)),
                ("line", integer(cited.line)),
                (
                    "line_end",
                    optional(cited.line_end.as_ref(), |l| integer(*l)),
                ),
                ("digest", optional(cited.digest.as_ref(), |d| text(d))),
            ]),
        ),
        Citation::RecordId(cited) => (
            "record_id",
            object([
                ("store", name(RECORD_STORES, cited.store)),
                ("record_id", text(&cited.record_id)),
            ]),
        ),
        Citation::ArtifactRevision(cited) => (
            "artifact_revision",
            object([
                ("artifact", text(&cited.artifact.0)),
                ("revision", text(&cited.revision.0)),
                ("case", optional(cited.case.as_ref(), |c| text(&c.0))),
                ("entry", optional(cited.entry.as_ref(), |e| text(e))),
                ("digest", optional(cited.digest.as_ref(), |d| text(d))),
            ]),
        ),
    };
    object([
        ("source", name(GROUND_SOURCES, ground.source)),
        ("citation", object([("form", text(form)), ("value", value)])),
        (
            "supports",
            optional(ground.supports.as_ref(), |held| text(&held.0)),
        ),
        ("how", text(&ground.how)),
    ])
}

fn produced_value(produced: &ProducedRecord) -> Value {
    object([
        ("document", name(PRODUCED_DOCUMENTS, produced.document)),
        ("record", text(&produced.record)),
        ("case_revision", text(&produced.case_revision.0)),
        (
            "aep_record",
            optional(produced.aep_record.as_ref(), |held| text(held)),
        ),
    ])
}

fn record_value(record: &EscalationSuppressedRecord) -> Value {
    object([
        ("request_id", text(&record.request_id.0)),
        ("decision_class", text(&record.decision_class.0)),
        ("subject_artifact", text(&record.subject_artifact.0)),
        ("subject_revision", text(&record.subject_revision.0)),
        ("reasons", texts(record.reasons.iter().map(String::as_str))),
        ("best", optional(record.best.as_ref(), |held| text(&held.0))),
        ("cause", name(SUPPRESSION_CAUSES, record.cause)),
        ("kept_at", text(&record.kept_at.0)),
    ])
}

/// An object whose members are exactly the declared ones, each once.
struct Object<'a> {
    value: &'a Value,
    at: String,
}

impl<'a> Object<'a> {
    fn of(value: &'a Value, at: &str, declared: &[&str]) -> Result<Self, DecodeError> {
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
        Ok(Self {
            value,
            at: at.to_owned(),
        })
    }

    fn get(&self, member: &str) -> Result<(&'a Value, String), DecodeError> {
        Ok((
            member_at(self.value, &self.at, member)?,
            nested(&self.at, member),
        ))
    }

    fn text(&self, member: &str) -> Result<String, DecodeError> {
        let (value, at) = self.get(member)?;
        text_at(value, &at, "a string").map(str::to_owned)
    }

    fn integer(&self, member: &str) -> Result<i64, DecodeError> {
        let (value, at) = self.get(member)?;
        integer_at(value, &at, "an integer")
    }

    fn boolean(&self, member: &str) -> Result<bool, DecodeError> {
        let (value, at) = self.get(member)?;
        bool_at(value, &at, "a boolean")
    }

    fn named<T: Copy>(&self, member: &str, names: &Names<T>) -> Result<T, DecodeError> {
        let (value, at) = self.get(member)?;
        named(value, &at, names)
    }

    /// The member decoded by `decode`, or `None` when it is `null`.
    fn optional<T>(
        &self,
        member: &str,
        decode: impl FnOnce(&'a Value, &str) -> Result<T, DecodeError>,
    ) -> Result<Option<T>, DecodeError> {
        let (value, at) = self.get(member)?;
        match value {
            Value::Null => Ok(None),
            present => decode(present, &at).map(Some),
        }
    }

    fn optional_text(&self, member: &str) -> Result<Option<String>, DecodeError> {
        self.optional(member, |value, at| {
            text_at(value, at, "a string or null").map(str::to_owned)
        })
    }

    fn list<T>(
        &self,
        member: &str,
        decode: impl Fn(&'a Value, &str) -> Result<T, DecodeError>,
    ) -> Result<Vec<T>, DecodeError> {
        let (value, at) = self.get(member)?;
        items_at(value, &at, "an array")?
            .iter()
            .enumerate()
            .map(|(index, item)| decode(item, &format!("{at}[{index}]")))
            .collect()
    }

    fn texts(&self, member: &str) -> Result<Vec<String>, DecodeError> {
        self.list(member, |value, at| {
            text_at(value, at, "a string").map(str::to_owned)
        })
    }
}

fn named<T: Copy>(value: &Value, at: &str, names: &Names<T>) -> Result<T, DecodeError> {
    let spelled = text_at(value, at, "a declared name")?;
    names
        .iter()
        .find(|(_, name)| *name == spelled)
        .map(|(known, _)| *known)
        .ok_or_else(|| DecodeError::of(at, "a declared name", value))
}

fn plain(value: &Value, at: &str) -> Result<String, DecodeError> {
    text_at(value, at, "a string").map(str::to_owned)
}

fn decode(value: &Value) -> Result<StoredDecisionRequest, DecodeError> {
    let stored = Object::of(value, "", &["position", "state", "request", "records"])?;
    let (request, request_at) = stored.get("request")?;
    Ok(StoredDecisionRequest {
        position: stored.integer("position")?,
        state: stored.named("state", STATES)?,
        request: decode_request(request, &request_at)?,
        records: stored.list("records", decode_record)?,
    })
}

const REQUEST_MEMBERS: [&str; 38] = [
    "request_id",
    "case_id",
    "question_key",
    "record_key",
    "unblocks",
    "subject_artifact",
    "subject_revision",
    "subject_path",
    "question",
    "options",
    "autonomy_level",
    "decision_class",
    "holder",
    "eligible",
    "addressed_to",
    "opened_at",
    "expires_at",
    "supersedes",
    "recommended_option",
    "recommended_answer",
    "recommendation_strength",
    "default_on_silence",
    "grounds",
    "recommended_by",
    "grounding_report",
    "no_recommendation_reason",
    "escalated_to",
    "escalation_reason",
    "decided_option",
    "decided_answer",
    "decided_by",
    "decided_by_kind",
    "decided_at",
    "decided_case_revision",
    "decision_reasons",
    "produced_record",
    "superseded_by",
    "superseded_reason",
];

fn decode_request(value: &Value, at: &str) -> Result<StoredDecisionRequestData, DecodeError> {
    let request = Object::of(value, at, &REQUEST_MEMBERS)?;
    let (unblocks, unblocks_at) = request.get("unblocks")?;
    let principal = |value: &Value, at: &str| plain(value, at).map(Principal);
    let letter = |value: &Value, at: &str| plain(value, at).map(OptionLetter);
    let instant = |value: &Value, at: &str| plain(value, at).map(Timestamp);
    let id = |value: &Value, at: &str| plain(value, at).map(DecisionRequestId);
    let revision = |value: &Value, at: &str| plain(value, at).map(Revision);
    Ok(StoredDecisionRequestData {
        request_id: DecisionRequestId(request.text("request_id")?),
        case_id: CaseId(request.text("case_id")?),
        question_key: request.text("question_key")?,
        record_key: request.text("record_key")?,
        unblocks: decode_unblocks(unblocks, &unblocks_at)?,
        subject_artifact: ArtifactId(request.text("subject_artifact")?),
        subject_revision: Revision(request.text("subject_revision")?),
        subject_path: request.optional_text("subject_path")?,
        question: request.text("question")?,
        options: request.list("options", decode_option)?,
        autonomy_level: request.named("autonomy_level", AUTONOMY_LEVELS)?,
        decision_class: DecisionClassId(request.text("decision_class")?),
        holder: request.named("holder", HOLDERS)?,
        eligible: request.list("eligible", principal)?,
        addressed_to: Principal(request.text("addressed_to")?),
        opened_at: Timestamp(request.text("opened_at")?),
        expires_at: request.optional("expires_at", instant)?,
        supersedes: request.optional("supersedes", id)?,
        recommended_option: request.optional("recommended_option", letter)?,
        recommended_answer: request.optional_text("recommended_answer")?,
        recommendation_strength: request.named("recommendation_strength", STRENGTHS)?,
        default_on_silence: request.boolean("default_on_silence")?,
        grounds: request.list("grounds", decode_ground)?,
        recommended_by: request.optional("recommended_by", |value, at| {
            named(value, at, PROPOSER_KINDS)
        })?,
        grounding_report: request.optional_text("grounding_report")?,
        no_recommendation_reason: request.optional_text("no_recommendation_reason")?,
        escalated_to: request.optional("escalated_to", principal)?,
        escalation_reason: request.optional_text("escalation_reason")?,
        decided_option: request.optional("decided_option", letter)?,
        decided_answer: request.optional_text("decided_answer")?,
        decided_by: request.optional("decided_by", principal)?,
        decided_by_kind: request.optional("decided_by_kind", |value, at| {
            named(value, at, PRINCIPAL_KINDS)
        })?,
        decided_at: request.optional("decided_at", instant)?,
        decided_case_revision: request.optional("decided_case_revision", revision)?,
        decision_reasons: request.texts("decision_reasons")?,
        produced_record: request.optional("produced_record", decode_produced)?,
        superseded_by: request.optional("superseded_by", id)?,
        superseded_reason: request.optional_text("superseded_reason")?,
    })
}

fn decode_unblocks(value: &Value, at: &str) -> Result<Unblocks, DecodeError> {
    let unblocks = Object::of(
        value,
        at,
        &["kind", "action", "capability", "question_id", "outcome"],
    )?;
    Ok(Unblocks {
        kind: unblocks.named("kind", UNBLOCK_KINDS)?,
        action: ActionId(unblocks.text("action")?),
        capability: unblocks
            .optional("capability", |value, at| plain(value, at).map(CapabilityId))?,
        question_id: unblocks.optional_text("question_id")?,
        outcome: unblocks.optional("outcome", |value, at| plain(value, at).map(OutcomeId))?,
    })
}

fn decode_option(value: &Value, at: &str) -> Result<DecisionOption, DecodeError> {
    let option = Object::of(value, at, &["letter", "does", "cost", "consequence"])?;
    let (consequence, consequence_at) = option.get("consequence")?;
    let consequence = Object::of(
        consequence,
        &consequence_at,
        &["admits", "blocks", "grant", "answer", "produces"],
    )?;
    let action = |value: &Value, at: &str| plain(value, at).map(ActionId);
    Ok(DecisionOption {
        letter: OptionLetter(option.text("letter")?),
        does: option.text("does")?,
        cost: option.text("cost")?,
        consequence: Consequence {
            admits: consequence.list("admits", action)?,
            blocks: consequence.list("blocks", action)?,
            grant: consequence.optional("grant", |value, at| named(value, at, GRANTS))?,
            answer: consequence.optional_text("answer")?,
            produces: consequence
                .optional("produces", |value, at| named(value, at, PRODUCED_DOCUMENTS))?,
        },
    })
}

fn decode_ground(value: &Value, at: &str) -> Result<Ground, DecodeError> {
    let ground = Object::of(value, at, &["source", "citation", "supports", "how"])?;
    let (citation, citation_at) = ground.get("citation")?;
    let citation = Object::of(citation, &citation_at, &["form", "value"])?;
    let (cited, cited_at) = citation.get("value")?;
    let citation = match citation.text("form")?.as_str() {
        "path_line" => {
            let cited = Object::of(cited, &cited_at, &["path", "line", "line_end", "digest"])?;
            Citation::PathLine(PathLineCitation {
                path: cited.text("path")?,
                line: cited.integer("line")?,
                line_end: cited.optional("line_end", |value, at| {
                    integer_at(value, at, "an integer or null")
                })?,
                digest: cited.optional_text("digest")?,
            })
        }
        "record_id" => {
            let cited = Object::of(cited, &cited_at, &["store", "record_id"])?;
            Citation::RecordId(RecordCitation {
                store: cited.named("store", RECORD_STORES)?,
                record_id: cited.text("record_id")?,
            })
        }
        "artifact_revision" => {
            let cited = Object::of(
                cited,
                &cited_at,
                &["artifact", "revision", "case", "entry", "digest"],
            )?;
            Citation::ArtifactRevision(ArtifactRevisionCitation {
                artifact: ArtifactId(cited.text("artifact")?),
                revision: Revision(cited.text("revision")?),
                case: cited.optional("case", |value, at| plain(value, at).map(CaseId))?,
                entry: cited.optional_text("entry")?,
                digest: cited.optional_text("digest")?,
            })
        }
        _ => {
            let (form, form_at) = citation.get("form")?;
            return Err(DecodeError::of(&form_at, "a Citation form", form));
        }
    };
    Ok(Ground {
        source: ground.named("source", GROUND_SOURCES)?,
        citation,
        supports: ground.optional("supports", |value, at| plain(value, at).map(OptionLetter))?,
        how: ground.text("how")?,
    })
}

fn decode_produced(value: &Value, at: &str) -> Result<ProducedRecord, DecodeError> {
    let produced = Object::of(
        value,
        at,
        &["document", "record", "case_revision", "aep_record"],
    )?;
    Ok(ProducedRecord {
        document: produced.named("document", PRODUCED_DOCUMENTS)?,
        record: produced.text("record")?,
        case_revision: Revision(produced.text("case_revision")?),
        aep_record: produced.optional_text("aep_record")?,
    })
}

fn decode_record(value: &Value, at: &str) -> Result<EscalationSuppressedRecord, DecodeError> {
    let record = Object::of(
        value,
        at,
        &[
            "request_id",
            "decision_class",
            "subject_artifact",
            "subject_revision",
            "reasons",
            "best",
            "cause",
            "kept_at",
        ],
    )?;
    Ok(EscalationSuppressedRecord {
        request_id: DecisionRequestId(record.text("request_id")?),
        decision_class: DecisionClassId(record.text("decision_class")?),
        subject_artifact: ArtifactId(record.text("subject_artifact")?),
        subject_revision: Revision(record.text("subject_revision")?),
        reasons: record.texts("reasons")?,
        best: record.optional("best", |value, at| plain(value, at).map(OptionLetter))?,
        cause: record.named("cause", SUPPRESSION_CAUSES)?,
        kept_at: Timestamp(record.text("kept_at")?),
    })
}
