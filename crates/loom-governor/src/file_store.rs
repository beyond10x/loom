//! [`FileCaseStore`]: a [`FallibleCaseStore`] in a directory, so a held case outlives the process
//! that held it.
//!
//! Each case is one file, `case-<hex of its id>.json`, holding the case as the `loom.governor`
//! domain declares it ([`StoredCase`], `ess/domains/governor.yaml`); the observations are one file
//! beside them, `observations.json`, a list of [`StoredObservation`]. Every write goes to a fresh
//! temporary file in the same directory, is synced, and is renamed over its target, so a reader sees
//! the previous document or the next one and never part of one. A write that fails returns
//! [`FileStoreError`] and leaves the previous document in place, including when an update's change
//! already ran: the change is applied to a copy.
//!
//! Every operation holds an exclusive lock on `<dir>/.lock` for its duration, so two stores over
//! one directory, in one process or in two, never interleave a read and a write of the same file.
//!
//! The document is written with the generated JSON writer and read with its reader, and before
//! anything is written it is read back and compared with what was meant: a case whose JSON values
//! would not come back exactly (a hand-built number spelling no reader accepts, or nesting past the
//! reader's limit) is refused rather than written. JSON values keep their member order and number
//! spelling, so the frontier id, which hashes the decision bytes, is the same after a restart.
//!
//! Protocol definitions are not stored: the host registers the same protocols before it reads.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{
    Timestamp as CommissionTimestamp, Uuid as CommissionUuid,
};
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, ObservationData, ObservationId,
};
use loom::governor::{
    StoredArtifact, StoredCase, StoredCaseId, StoredEvidence, StoredEvidenceData, StoredObservation,
};
use loom::json::{
    self, DecodeError, Value, bool_at, integer_at, items_at, member, member_at, nested, push_bool,
    push_integer, push_text, push_value, text_at,
};
use loom::primitives::{Timestamp, Uuid};

use crate::{CaseState, FallibleCaseStore, HeldEvidence};

const OBSERVATIONS: &str = "observations.json";
const LOCK: &str = ".lock";
const TEMPORARY: &str = ".tmp-";

/// Temporary file names this process has used, so two writes in one process never share one.
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// A [`FallibleCaseStore`] keeping one file per case in a directory.
#[derive(Debug)]
pub struct FileCaseStore {
    dir: PathBuf,
}

/// Why the store could not read or write: the file and what went wrong with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStoreError {
    /// The file or directory the operation was about.
    pub path: PathBuf,
    /// What went wrong.
    pub problem: String,
}

impl fmt::Display for FileStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.problem)
    }
}

impl std::error::Error for FileStoreError {}

fn failed(path: &Path, problem: impl fmt::Display) -> FileStoreError {
    FileStoreError {
        path: path.to_owned(),
        problem: problem.to_string(),
    }
}

impl FileCaseStore {
    /// A store over `dir`, created when it does not exist yet.
    ///
    /// # Errors
    ///
    /// [`FileStoreError`] when the directory or its lock file cannot be created.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, FileStoreError> {
        let dir = dir.into();
        fs::create_dir_all(&dir).map_err(|error| failed(&dir, error))?;
        let lock = dir.join(LOCK);
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock)
            .map_err(|error| failed(&lock, error))?;
        Ok(Self { dir })
    }

    /// The directory the store keeps its files in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The file that holds `case`: its id hex-encoded, so no id names a file outside the store.
    fn case_path(&self, case: &CaseId) -> PathBuf {
        let mut name = String::from("case-");
        for byte in case.0.bytes() {
            name.push_str(&format!("{byte:02x}"));
        }
        name.push_str(".json");
        self.dir.join(name)
    }

    /// Runs `operation` holding the directory's lock.
    fn locked<T>(
        &self,
        operation: impl FnOnce() -> Result<T, FileStoreError>,
    ) -> Result<T, FileStoreError> {
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

    fn read_case(&self, case: &CaseId) -> Result<Option<CaseState>, FileStoreError> {
        let path = self.case_path(case);
        let Some(text) = read_if_present(&path)? else {
            return Ok(None);
        };
        let stored = json::parse(&text)
            .map_err(|error| failed(&path, error))
            .and_then(|value| decode_case(&value).map_err(|error| failed(&path, error)))?;
        if stored.id.0 != case.0 {
            return Err(failed(
                &path,
                format!("holds case `{}`, not `{}`", stored.id.0, case.0),
            ));
        }
        held_case(stored)
            .map(Some)
            .map_err(|problem| failed(&path, problem))
    }

    fn write_case(&self, state: &CaseState) -> Result<(), FileStoreError> {
        let path = self.case_path(&state.id);
        let stored = stored_case(state);
        let mut text = String::new();
        encode_case(&mut text, &stored);
        let reread = json::parse(&text)
            .ok()
            .and_then(|value| decode_case(&value).ok());
        if reread.as_ref() != Some(&stored) {
            return Err(failed(
                &path,
                "the case would not read back as it is held; nothing was written",
            ));
        }
        self.replace(&path, &text)
    }

    fn read_observations(&self) -> Result<Vec<ObservationData>, FileStoreError> {
        let path = self.dir.join(OBSERVATIONS);
        let Some(text) = read_if_present(&path)? else {
            return Ok(Vec::new());
        };
        let value = json::parse(&text).map_err(|error| failed(&path, error))?;
        items_at(&value, "", "an array of observations")
            .and_then(|items| {
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| decode_observation(item, &format!("[{index}]")))
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| failed(&path, error))
            .map(|stored| stored.into_iter().map(observation_data).collect())
    }

    fn write_observations(&self, observations: &[ObservationData]) -> Result<(), FileStoreError> {
        let path = self.dir.join(OBSERVATIONS);
        let stored: Vec<StoredObservation> = observations.iter().map(stored_observation).collect();
        let mut text = String::from("[");
        for (index, observation) in stored.iter().enumerate() {
            if index > 0 {
                text.push(',');
            }
            encode_observation(&mut text, observation);
        }
        text.push(']');
        let reread = json::parse(&text).ok().and_then(|value| {
            let items = items_at(&value, "", "").ok()?;
            items
                .iter()
                .map(|item| decode_observation(item, "").ok())
                .collect::<Option<Vec<_>>>()
        });
        if reread.as_ref() != Some(&stored) {
            return Err(failed(
                &path,
                "the observations would not read back as they are kept; nothing was written",
            ));
        }
        self.replace(&path, &text)
    }

    /// Writes `text` to a new temporary file in the store's directory, syncs it, and renames it
    /// over `path`. On any failure the temporary file is removed and `path` is untouched.
    fn replace(&self, path: &Path, text: &str) -> Result<(), FileStoreError> {
        let temporary = self.dir.join(format!(
            "{TEMPORARY}{}-{}",
            std::process::id(),
            NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
        ));
        let written = (|| -> io::Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
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

/// The file's text, or `None` when there is no such file.
fn read_if_present(path: &Path) -> Result<Option<String>, FileStoreError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(failed(path, error)),
    }
}

impl FallibleCaseStore for FileCaseStore {
    type Error = FileStoreError;

    fn insert(&self, state: CaseState) -> Result<bool, Self::Error> {
        self.locked(|| {
            let path = self.case_path(&state.id);
            match fs::symlink_metadata(&path) {
                Ok(_) => return Ok(false),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(failed(&path, error)),
            }
            self.write_case(&state)?;
            Ok(true)
        })
    }

    fn get(&self, case: &CaseId) -> Result<Option<CaseState>, Self::Error> {
        self.locked(|| self.read_case(case))
    }

    fn update<T>(
        &self,
        case: &CaseId,
        change: impl FnOnce(&mut CaseState) -> T,
    ) -> Result<Option<T>, Self::Error> {
        self.locked(|| {
            let Some(mut state) = self.read_case(case)? else {
                return Ok(None);
            };
            let result = change(&mut state);
            self.write_case(&state)?;
            Ok(Some(result))
        })
    }

    fn observe(&self, observation: ObservationData) -> Result<(), Self::Error> {
        self.locked(|| {
            let mut observations = self.read_observations()?;
            observations.push(observation);
            self.write_observations(&observations)
        })
    }

    fn observations(&self) -> Result<Vec<ObservationData>, Self::Error> {
        self.locked(|| self.read_observations())
    }
}

// ---- the held case and the stored case --------------------------------------------------------

fn stored_case(state: &CaseState) -> StoredCase {
    StoredCase {
        id: StoredCaseId(state.id.0.clone()),
        protocol: state.protocol.clone(),
        revision: state.revision,
        artifacts: state
            .artifacts
            .iter()
            .map(|(artifact, revision)| StoredArtifact {
                artifact: artifact.clone(),
                revision: revision.clone(),
            })
            .collect(),
        evidence: state
            .evidence
            .iter()
            .map(|held| StoredEvidence {
                data: StoredEvidenceData {
                    evidence_id: Uuid(held.data.evidence_id.0.0.clone()),
                    case_id: StoredCaseId(held.data.case_id.0.clone()),
                    kind: held.data.kind.clone(),
                    subject_revision: held.data.subject_revision,
                    producer: held.data.producer.clone(),
                    observation_ids: held
                        .data
                        .observation_ids
                        .iter()
                        .map(|id| Uuid(id.0.0.clone()))
                        .collect(),
                    facts: to_loom(&held.data.facts),
                    provenance: to_loom(&held.data.provenance),
                },
                applies: held.applies,
            })
            .collect(),
    }
}

/// The held case a stored one describes; an error when it names one artifact twice.
fn held_case(stored: StoredCase) -> Result<CaseState, String> {
    let mut artifacts = std::collections::BTreeMap::new();
    for StoredArtifact { artifact, revision } in stored.artifacts {
        if artifacts.contains_key(&artifact) {
            return Err(format!("artifact `{artifact}` is named twice"));
        }
        artifacts.insert(artifact, revision);
    }
    Ok(CaseState {
        id: CaseId(stored.id.0),
        protocol: stored.protocol,
        revision: stored.revision,
        artifacts,
        evidence: stored
            .evidence
            .into_iter()
            .map(|held| HeldEvidence {
                data: EvidenceData {
                    evidence_id: EvidenceId(CommissionUuid(held.data.evidence_id.0)),
                    case_id: CaseId(held.data.case_id.0),
                    kind: held.data.kind,
                    subject_revision: held.data.subject_revision,
                    producer: held.data.producer,
                    observation_ids: held
                        .data
                        .observation_ids
                        .into_iter()
                        .map(|id| ObservationId(CommissionUuid(id.0)))
                        .collect(),
                    facts: to_commission(held.data.facts),
                    provenance: to_commission(held.data.provenance),
                },
                applies: held.applies,
            })
            .collect(),
    })
}

fn stored_observation(observation: &ObservationData) -> StoredObservation {
    StoredObservation {
        observation_id: Uuid(observation.observation_id.0.0.clone()),
        source: observation.source.clone(),
        subject: observation.subject.clone(),
        observed_at: Timestamp(observation.observed_at.0.clone()),
        payload: to_loom(&observation.payload),
    }
}

fn observation_data(stored: StoredObservation) -> ObservationData {
    ObservationData {
        observation_id: ObservationId(CommissionUuid(stored.observation_id.0)),
        source: stored.source,
        subject: stored.subject,
        observed_at: CommissionTimestamp(stored.observed_at.0),
        payload: to_commission(stored.payload),
    }
}

/// Commission's JSON value as Loom's: the same shape, member order and number spelling.
fn to_loom(value: &commission_json::Value) -> Value {
    match value {
        commission_json::Value::Null => Value::Null,
        commission_json::Value::Bool(flag) => Value::Bool(*flag),
        commission_json::Value::Number(spelling) => Value::Number(spelling.clone()),
        commission_json::Value::Text(text) => Value::Text(text.clone()),
        commission_json::Value::Array(items) => Value::Array(items.iter().map(to_loom).collect()),
        commission_json::Value::Object(members) => Value::Object(
            members
                .iter()
                .map(|(name, item)| (name.clone(), to_loom(item)))
                .collect(),
        ),
    }
}

/// Loom's JSON value as Commission's: the same shape, member order and number spelling.
fn to_commission(value: Value) -> commission_json::Value {
    match value {
        Value::Null => commission_json::Value::Null,
        Value::Bool(flag) => commission_json::Value::Bool(flag),
        Value::Number(spelling) => commission_json::Value::Number(spelling),
        Value::Text(text) => commission_json::Value::Text(text),
        Value::Array(items) => {
            commission_json::Value::Array(items.into_iter().map(to_commission).collect())
        }
        Value::Object(members) => commission_json::Value::Object(
            members
                .into_iter()
                .map(|(name, item)| (name, to_commission(item)))
                .collect(),
        ),
    }
}

// ---- the codec ------------------------------------------------------------------------------------
//
// The generated `loom` crate carries the JSON reader and writer and no codec for its types, so the
// members below are written and read field for field as `loom.governor` declares them.

fn encode_case(out: &mut String, case: &StoredCase) {
    out.push('{');
    member(out, "id");
    push_text(out, &case.id.0);
    member(out, "protocol");
    push_text(out, &case.protocol);
    member(out, "revision");
    push_integer(out, case.revision);
    member(out, "artifacts");
    out.push('[');
    for (index, artifact) in case.artifacts.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('{');
        member(out, "artifact");
        push_text(out, &artifact.artifact);
        member(out, "revision");
        push_text(out, &artifact.revision);
        out.push('}');
    }
    out.push(']');
    member(out, "evidence");
    out.push('[');
    for (index, held) in case.evidence.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('{');
        member(out, "data");
        encode_evidence_data(out, &held.data);
        member(out, "applies");
        push_bool(out, held.applies);
        out.push('}');
    }
    out.push(']');
    out.push('}');
}

fn encode_evidence_data(out: &mut String, data: &StoredEvidenceData) {
    out.push('{');
    member(out, "evidence_id");
    push_text(out, &data.evidence_id.0);
    member(out, "case_id");
    push_text(out, &data.case_id.0);
    member(out, "kind");
    push_text(out, &data.kind);
    member(out, "subject_revision");
    push_integer(out, data.subject_revision);
    member(out, "producer");
    push_text(out, &data.producer);
    member(out, "observation_ids");
    out.push('[');
    for (index, id) in data.observation_ids.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        push_text(out, &id.0);
    }
    out.push(']');
    member(out, "facts");
    push_value(out, &data.facts);
    member(out, "provenance");
    push_value(out, &data.provenance);
    out.push('}');
}

fn encode_observation(out: &mut String, observation: &StoredObservation) {
    out.push('{');
    member(out, "observation_id");
    push_text(out, &observation.observation_id.0);
    member(out, "source");
    push_text(out, &observation.source);
    member(out, "subject");
    push_text(out, &observation.subject);
    member(out, "observed_at");
    push_text(out, &observation.observed_at.0);
    member(out, "payload");
    push_value(out, &observation.payload);
    out.push('}');
}

/// The text member `name` of the object at `at`.
fn text_member(value: &Value, at: &str, name: &str) -> Result<String, DecodeError> {
    text_at(member_at(value, at, name)?, &nested(at, name), "a string").map(str::to_owned)
}

fn decode_case(value: &Value) -> Result<StoredCase, DecodeError> {
    let at = "";
    let artifacts = items_at(member_at(value, at, "artifacts")?, "artifacts", "an array")?
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let at = format!("artifacts[{index}]");
            Ok(StoredArtifact {
                artifact: text_member(item, &at, "artifact")?,
                revision: text_member(item, &at, "revision")?,
            })
        })
        .collect::<Result<_, DecodeError>>()?;
    let evidence = items_at(member_at(value, at, "evidence")?, "evidence", "an array")?
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let at = format!("evidence[{index}]");
            Ok(StoredEvidence {
                data: decode_evidence_data(member_at(item, &at, "data")?, &nested(&at, "data"))?,
                applies: bool_at(
                    member_at(item, &at, "applies")?,
                    &nested(&at, "applies"),
                    "a boolean",
                )?,
            })
        })
        .collect::<Result<_, DecodeError>>()?;
    Ok(StoredCase {
        id: StoredCaseId(text_member(value, at, "id")?),
        protocol: text_member(value, at, "protocol")?,
        revision: integer_at(member_at(value, at, "revision")?, "revision", "an integer")?,
        artifacts,
        evidence,
    })
}

fn decode_evidence_data(value: &Value, at: &str) -> Result<StoredEvidenceData, DecodeError> {
    let ids_at = nested(at, "observation_ids");
    Ok(StoredEvidenceData {
        evidence_id: Uuid(text_member(value, at, "evidence_id")?),
        case_id: StoredCaseId(text_member(value, at, "case_id")?),
        kind: text_member(value, at, "kind")?,
        subject_revision: integer_at(
            member_at(value, at, "subject_revision")?,
            &nested(at, "subject_revision"),
            "an integer",
        )?,
        producer: text_member(value, at, "producer")?,
        observation_ids: items_at(
            member_at(value, at, "observation_ids")?,
            &ids_at,
            "an array",
        )?
        .iter()
        .enumerate()
        .map(|(index, id)| {
            text_at(id, &format!("{ids_at}[{index}]"), "a string").map(|id| Uuid(id.to_owned()))
        })
        .collect::<Result<_, _>>()?,
        facts: member_at(value, at, "facts")?.clone(),
        provenance: member_at(value, at, "provenance")?.clone(),
    })
}

fn decode_observation(value: &Value, at: &str) -> Result<StoredObservation, DecodeError> {
    Ok(StoredObservation {
        observation_id: Uuid(text_member(value, at, "observation_id")?),
        source: text_member(value, at, "source")?,
        subject: text_member(value, at, "subject")?,
        observed_at: Timestamp(text_member(value, at, "observed_at")?),
        payload: member_at(value, at, "payload")?.clone(),
    })
}
