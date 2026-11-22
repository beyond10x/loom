//! The records that send a bound action back to work, and the feedback document its program is
//! given (story `bound-program-retry-records`).
//!
//! [`retry_records`] reads them from the governor holding the case: each evidence record of the
//! case whose facts name a `kind` and a `result` the binding's `retry_when` lists, about the
//! revision the case holds for the facts' `subject`, never `none`, in the order the case received
//! them. Every member of a `RetryRecord` is read from the held record and from the observation it
//! cites; nothing a program prints or a request names is one.
//!
//! On a step of [`crate::ProgramEffects`] with retry records, after the run count and before
//! anything starts, the port keeps one `loom.retry-feedback/1` document as `blobs/<sha256>` of the
//! store directory and sets [`RETRY_FEEDBACK_VARIABLE`] on the bound program's child to its
//! canonical absolute path, mounted read-only under a runner that is not `local`. With no retry
//! record the variable is not set and nothing is written. The `arguments_from` program is given
//! neither.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::responsibility::{CaseId, GovernorError};
use b10x_loom_commission::ports::effect::EffectError;
use b10x_loom_intake_slice::confinement::ChildCommand;
use loom::bindings::{BoundAction, RetryFeedback, RetryLog, RetryRecord};
use loom_governor::{CanonGovernor, EvaluationTime, FallibleCaseStore};
use sha2::{Digest as _, Sha256};

use crate::program::{BLOBS, PROGRAM_SOURCE, keep_blob, sha256};

/// The variable that names the feedback document of a retried bound program. The port alone sets
/// it; no document, request, report or host variable does.
pub const RETRY_FEEDBACK_VARIABLE: &str = "LOOM_RETRY_FEEDBACK";
/// The `format` of the feedback document.
pub const RETRY_FEEDBACK_FORMAT: &str = "loom.retry-feedback/1";
/// The most lines of a cited blob a `RetryLog` holds: the last ones.
pub const RETRY_LOG_LINES: usize = 60;
/// The most bytes of a `RetryLog`'s tail.
pub const RETRY_LOG_BYTES: usize = 65536;

/// The revision of an artifact not written yet: a record about it sends nobody back.
const NO_REVISION: &str = "none";

/// The records that send `binding` back to work in `case`, in the order the case received them
/// (see the module documentation); none for a binding whose `retry_when` is empty.
///
/// # Errors
/// The governor cannot answer for the case, its evidence or its observations.
pub fn retry_records<S: FallibleCaseStore, T: EvaluationTime>(
    governor: &CanonGovernor<S, T>,
    case: &CaseId,
    binding: &BoundAction,
) -> Result<Vec<RetryRecord>, GovernorError> {
    if binding.retry_when.is_empty() {
        return Ok(Vec::new());
    }
    let revisions = governor.revisions(case)?;
    let mut observations = None;
    let mut records = Vec::new();
    for held in governor.evidence(case)? {
        if held.case_id != *case {
            continue;
        }
        let facts = &held.facts;
        let (Some(kind), Some(result), Some(subject), Some(subject_revision), Some(observed_at)) = (
            only_text(facts, "kind"),
            only_text(facts, "result"),
            only_text(facts, "subject"),
            only_text(facts, "subject_revision"),
            only_text(facts, "observed_at"),
        ) else {
            continue;
        };
        if kind != held.kind {
            continue;
        }
        let listed = binding.retry_when.iter().any(|condition| {
            condition.kind == kind && condition.results.iter().any(|listed| listed == result)
        });
        if !listed
            || subject_revision == NO_REVISION
            || revisions.get(subject).map(String::as_str) != Some(subject_revision)
        {
            continue;
        }
        let received = match &observations {
            Some(received) => received,
            None => observations.insert(governor.try_observations()?),
        };
        // The first cited run of a bound program in this case: its action, exit and blob.
        let run = held.observation_ids.iter().find_map(|cited| {
            received
                .iter()
                .find(|observation| &observation.observation_id == cited)
                .filter(|observation| {
                    observation.source == PROGRAM_SOURCE
                        && only_text(&observation.payload, "case") == Some(case.0.as_str())
                })
        });
        let payload = run.map(|observation| &observation.payload);
        records.push(RetryRecord {
            evidence_id: held.evidence_id.0.0.clone(),
            kind: kind.to_owned(),
            subject: subject.to_owned(),
            subject_revision: subject_revision.to_owned(),
            result: result.to_owned(),
            producer: held.producer.clone(),
            observed_at: loom::primitives::Timestamp(observed_at.to_owned()),
            action: payload
                .and_then(|payload| only_text(payload, "action"))
                .map(str::to_owned),
            exit: payload.and_then(|payload| only_integer(payload, "exit")),
            blob_sha256: payload
                .and_then(|payload| only_text(payload, "blob_sha256"))
                .filter(|digest| is_digest(digest))
                .map(str::to_owned),
        });
    }
    Ok(records)
}

/// The feedback document a step was given: its canonical path and the evidence ids of its
/// records, which the run's observation names as `retry_of`.
pub(crate) struct Feedback {
    path: PathBuf,
    value: String,
    pub(crate) retry_of: Vec<String>,
}

/// Keeps the feedback document of `action` in `case` under `store` when `binding` has retry
/// records; `None` when it has none, and then nothing is written.
pub(crate) fn feedback<S: FallibleCaseStore, T: EvaluationTime>(
    governor: &CanonGovernor<S, T>,
    case: &CaseId,
    binding: &BoundAction,
    store: &Path,
) -> Result<Option<Feedback>, EffectError> {
    let records = retry_records(governor, case, binding)
        .map_err(|error| EffectError::new(format!("the retry records: {error:?}")))?;
    if records.is_empty() {
        return Ok(None);
    }
    let mut logs: Vec<RetryLog> = Vec::new();
    for digest in records
        .iter()
        .filter_map(|record| record.blob_sha256.as_ref())
    {
        if logs.iter().any(|log| &log.blob_sha256 == digest) {
            continue;
        }
        if let Some(tail) = log_tail(store, digest) {
            logs.push(RetryLog {
                blob_sha256: digest.clone(),
                tail,
            });
        }
    }
    let document = RetryFeedback {
        format: RETRY_FEEDBACK_FORMAT.to_owned(),
        case: case.0.clone(),
        action: binding.action.clone(),
        records,
        logs,
    };
    let bytes = feedback_json(&document).to_string().into_bytes();
    let digest = sha256(&bytes);
    keep_blob(store, &digest, &bytes)
        .map_err(|error| EffectError::new(format!("the retry feedback {digest}: {error}")))?;
    let path = std::fs::canonicalize(store.join(BLOBS).join(&digest))
        .map_err(|error| EffectError::new(format!("the retry feedback {digest}: {error}")))?;
    let value = path
        .to_str()
        .ok_or_else(|| {
            EffectError::new(format!(
                "the retry feedback {digest}: its path is not UTF-8"
            ))
        })?
        .to_owned();
    Ok(Some(Feedback {
        path,
        value,
        retry_of: document
            .records
            .iter()
            .map(|record| record.evidence_id.clone())
            .collect(),
    }))
}

/// `child`, built by [`crate::bound_case_child`], with the environment `ProgramEnvironment`
/// describes: the variables that function set (`PATH`, `LOOM_CASE`, `LOOM_TIMEOUT_SECONDS` and
/// the opening names, story bound-program-contract) and [`RETRY_FEEDBACK_VARIABLE`] at the path of
/// `feedback`, mounted read-only at that path when `local_reason` is `None` (a runner that is not
/// `local`); with no feedback, no such variable. Every other variable is the port's, so a
/// feedback document cannot change one.
pub(crate) fn given(
    child: ChildCommand,
    feedback: Option<&Feedback>,
    local_reason: Option<&str>,
) -> ChildCommand {
    let Some(feedback) = feedback else {
        return child;
    };
    let mut environment = child.environment().clone();
    environment
        .set
        .insert(RETRY_FEEDBACK_VARIABLE.to_owned(), feedback.value.clone());
    let child = child.with_environment(environment);
    match Some(feedback) {
        Some(feedback) if local_reason.is_none() => child.at_host_path(&feedback.path),
        _ => child,
    }
}

/// The tail of the blob `digest` of `store`: its last [`RETRY_LOG_LINES`] lines, at most the last
/// [`RETRY_LOG_BYTES`] bytes of them, invalid UTF-8 replaced by U+FFFD. `None` when the blob is
/// not a regular file in a `blobs` directory, cannot be read, or does not hash to its name.
fn log_tail(store: &Path, digest: &str) -> Option<String> {
    let blobs = store.join(BLOBS);
    if !std::fs::symlink_metadata(&blobs).ok()?.file_type().is_dir() {
        return None;
    }
    let path = blobs.join(digest);
    if !std::fs::symlink_metadata(&path).ok()?.file_type().is_file() {
        return None;
    }
    let mut file = std::fs::File::open(&path).ok()?;
    if !file.metadata().ok()?.file_type().is_file() {
        return None;
    }
    // Hashed whole, kept from the end only: the tail lies in the last RETRY_LOG_BYTES bytes.
    let mut hasher = Sha256::new();
    let mut window: Vec<u8> = Vec::new();
    let mut cut = false;
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut chunk).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&chunk[..read]);
        window.extend_from_slice(&chunk[..read]);
        if window.len() > 2 * RETRY_LOG_BYTES {
            window.drain(..window.len() - RETRY_LOG_BYTES);
            cut = true;
        }
    }
    let hashed = hasher
        .finalize()
        .iter()
        .fold(String::new(), |mut hex, byte| {
            use std::fmt::Write as _;
            let _ = write!(hex, "{byte:02x}");
            hex
        });
    if hashed != digest {
        return None;
    }
    if window.len() > RETRY_LOG_BYTES {
        window.drain(..window.len() - RETRY_LOG_BYTES);
        cut = true;
    }
    Some(tail_of(&window, cut))
}

/// The last [`RETRY_LOG_LINES`] lines of `window`, the end of a blob (`cut`: not its start).
fn tail_of(window: &[u8], cut: bool) -> String {
    // A line break that ends the text ends its last line and starts none.
    let body = window.strip_suffix(b"\n").unwrap_or(window);
    let mut start = 0;
    let mut breaks = 0;
    for (index, byte) in body.iter().enumerate().rev() {
        if *byte == b'\n' {
            breaks += 1;
            if breaks == RETRY_LOG_LINES {
                start = index + 1;
                break;
            }
        }
    }
    let mut bytes = &window[start..];
    if cut && start == 0 {
        // A character cut by the window is not part of the tail.
        while let Some((first, rest)) = bytes.split_first() {
            if first & 0b1100_0000 != 0b1000_0000 {
                break;
            }
            bytes = rest;
        }
    }
    let mut tail = String::from_utf8_lossy(bytes).into_owned();
    // A replacement character is longer than the byte it replaces: the bound holds on the text.
    while tail.len() > RETRY_LOG_BYTES {
        let first = tail.chars().next().map_or(1, char::len_utf8);
        tail.drain(..first);
    }
    tail
}

fn feedback_json(document: &RetryFeedback) -> serde_json::Value {
    serde_json::json!({
        "format": document.format,
        "case": document.case,
        "action": document.action,
        "records": document.records.iter().map(|record| serde_json::json!({
            "evidence_id": record.evidence_id,
            "kind": record.kind,
            "subject": record.subject,
            "subject_revision": record.subject_revision,
            "result": record.result,
            "producer": record.producer,
            "observed_at": record.observed_at.0,
            "action": record.action,
            "exit": record.exit,
            "blob_sha256": record.blob_sha256,
        })).collect::<Vec<_>>(),
        "logs": document.logs.iter().map(|log| serde_json::json!({
            "blob_sha256": log.blob_sha256,
            "tail": log.tail,
        })).collect::<Vec<_>>(),
    })
}

/// The text member `name` of the object `value`, when the object names it exactly once.
fn only_text<'v>(value: &'v commission_json::Value, name: &str) -> Option<&'v str> {
    let commission_json::Value::Object(members) = value else {
        return None;
    };
    let mut named = members.iter().filter(|(member, _)| member == name);
    match (named.next(), named.next()) {
        (Some((_, commission_json::Value::Text(text))), None) => Some(text),
        _ => None,
    }
}

/// The whole-number member `name` of the object `value`, when the object names it exactly once
/// and it is an integer an `i64` holds.
fn only_integer(value: &commission_json::Value, name: &str) -> Option<i64> {
    let commission_json::Value::Object(members) = value else {
        return None;
    };
    let mut named = members.iter().filter(|(member, _)| member == name);
    match (named.next(), named.next()) {
        (Some((_, commission_json::Value::Number(spelling))), None) => spelling.parse().ok(),
        _ => None,
    }
}

/// Whether `text` is a SHA-256 as the port names a blob: 64 lowercase hexadecimal digits.
fn is_digest(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tail_of_fewer_lines_is_the_whole_text() {
        assert_eq!(tail_of(b"a\nb\n", false), "a\nb\n");
        assert_eq!(tail_of(b"a\nb", false), "a\nb");
    }

    #[test]
    fn a_cut_character_is_dropped_and_invalid_bytes_are_replaced() {
        assert_eq!(
            tail_of(&[0x82, 0xac, b'x', 0xff, b'\n'], true),
            "x\u{fffd}\n"
        );
    }

    #[test]
    fn a_tail_of_replacement_characters_holds_at_most_the_bound() {
        let window = vec![0xffu8; RETRY_LOG_BYTES];
        assert!(tail_of(&window, false).len() <= RETRY_LOG_BYTES);
    }

    #[test]
    fn only_a_sha256_names_a_blob() {
        assert!(is_digest(&"a".repeat(64)));
        assert!(!is_digest("../../etc/passwd"));
        assert!(!is_digest(&"A".repeat(64)));
    }
}
