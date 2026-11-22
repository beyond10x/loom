//! [`select`]: the values a selection gives, read from what a cycle left and nothing else: the
//! cycle's store and workspace (`<scratch>/<n>/store`, `work`), the JSON output of its steps, the
//! suite's own directory, and the catalog the suite runs with. No program runs, no network is
//! reached, and a path read from a document, or a link, whose real path leaves its root gives
//! `PathOutsideCycle`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use b10x_loom_runlog::log_file_name;
use loom::tree::{
    CaseSelector, CycleArea, DifferenceReason, FollowTarget, PickPosition, ReadAs, SelectItem,
};
use loom_governor::case_file_stem;
use loom_protocols::ProtocolCatalog;
use serde_json::Value;

use super::check::holds;
use super::path::{glob_matches, parse_value_path, stays_below, values_at};
use super::value::sha256_hex;
use crate::store::read_manifest;

/// The most bytes of one file a selection reads.
pub const MAX_SELECTED_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// The most paths a `files` source gives.
pub const MAX_GLOB_PATHS: usize = 10_000;

/// The most entries a `files` source visits before it gives up.
const MAX_GLOB_VISITS: usize = 100_000;

/// The source of `loom/bound-program` observations, the program runs of a case.
const PROGRAM_SOURCE: &str = "loom/bound-program";

/// What a cycle left, as the checks read it.
#[derive(Debug, Clone, Copy)]
pub struct CycleView<'a> {
    /// The cycle's workspace, `<scratch>/<n>/work`.
    pub work: &'a Path,
    /// The cycle's store, `<scratch>/<n>/store`.
    pub store: &'a Path,
    /// The suite's directory.
    pub suite: &'a Path,
    /// The JSON output of each named `replay`, `status` and `verify` step so far.
    pub steps: &'a BTreeMap<String, Value>,
    /// The catalog the suite runs with.
    pub catalog: &'a ProtocolCatalog,
}

/// One value a selection gives, with the bytes of the file it was read from when it is a whole
/// file, which `as`, `size` and `digest` read.
#[derive(Debug, Clone, PartialEq)]
pub struct Selected {
    /// The value.
    pub value: Value,
    /// The bytes of the whole file it was read from.
    pub bytes: Option<Arc<Vec<u8>>>,
}

impl Selected {
    fn plain(value: Value) -> Self {
        Self { value, bytes: None }
    }
}

/// Why a selection gave no list of values: the difference it is, and what went wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelectFailure {
    pub(crate) reason: DifferenceReason,
    pub(crate) detail: String,
}

fn failure(reason: DifferenceReason, detail: impl Into<String>) -> SelectFailure {
    SelectFailure {
        reason,
        detail: detail.into(),
    }
}

fn outside(detail: impl Into<String>) -> SelectFailure {
    failure(DifferenceReason::PathOutsideCycle, detail)
}

fn too_large(detail: impl Into<String>) -> SelectFailure {
    failure(DifferenceReason::SelectionTooLarge, detail)
}

fn step_failed(detail: impl Into<String>) -> SelectFailure {
    failure(DifferenceReason::StepFailed, detail)
}

/// The values `items` select from `view`, in order.
///
/// # Errors
/// The difference a read gives: `PathOutsideCycle`, `BlobChanged`, `SelectionTooLarge`, or
/// `StepFailed` when a file of the store does not read as the tree writes it.
pub fn select(items: &[SelectItem], view: &CycleView<'_>) -> Result<Vec<Selected>, String> {
    select_values(items, view)
        .map_err(|failure| format!("{:?}: {}", failure.reason, failure.detail))
}

pub(crate) fn select_values(
    items: &[SelectItem],
    view: &CycleView<'_>,
) -> Result<Vec<Selected>, SelectFailure> {
    let Some((first, operations)) = items.split_first() else {
        return Ok(Vec::new());
    };
    let mut values = source(first, view)?;
    for operation in operations {
        values = apply(operation, values, view)?;
    }
    Ok(values)
}

/// Reads at most [`MAX_SELECTED_FILE_BYTES`] of the regular file `path`, whose real path must lie
/// in `root`; `None` when nothing is there or it is a directory. Anything else that is no regular
/// file (a named pipe a program left, a device, a socket) is `StepFailed` and never read: the path
/// is checked before it is opened and the opened handle again, and on Unix the open itself does
/// not wait for a writer (`O_NONBLOCK`, which has no effect on a regular file).
fn read_inside(root: &Path, path: &Path) -> Result<Option<Vec<u8>>, SelectFailure> {
    let not_regular = || step_failed("a selected path is not a regular file; it is not read");
    let Ok(root) = fs::canonicalize(root) else {
        return Ok(None);
    };
    let real = match fs::canonicalize(path) {
        Ok(real) => real,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(step_failed(format!("a file cannot be read: {error}"))),
    };
    if !real.starts_with(&root) {
        return Err(outside("a path or a link leaves the cycle's directory"));
    }
    let metadata = fs::metadata(&real).map_err(|error| step_failed(error.to_string()))?;
    if metadata.is_dir() {
        return Ok(None);
    }
    if !metadata.is_file() {
        return Err(not_regular());
    }
    if metadata.len() > MAX_SELECTED_FILE_BYTES {
        return Err(too_large(format!(
            "a file of {} bytes is over 16 MiB",
            metadata.len()
        )));
    }
    let cannot = |error: io::Error| step_failed(format!("a file cannot be read: {error}"));
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(&real).map_err(cannot)?;
    if !file.metadata().map_err(cannot)?.is_file() {
        return Err(not_regular());
    }
    let mut bytes = Vec::new();
    file.take(MAX_SELECTED_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(cannot)?;
    if bytes.len() as u64 > MAX_SELECTED_FILE_BYTES {
        return Err(too_large("a file is over 16 MiB"));
    }
    Ok(Some(bytes))
}

/// How a file's bytes read when nothing says otherwise: by the extension of `name`.
fn read_as_of(name: &str) -> ReadAs {
    match Path::new(name).extension().and_then(|e| e.to_str()) {
        Some("json") => ReadAs::Json,
        Some("jsonl") => ReadAs::Jsonl,
        Some("yaml" | "yml") => ReadAs::Yaml,
        _ => ReadAs::Text,
    }
}

/// The values `bytes` give read as `how`: one value, or one per line for `lines` and `jsonl`.
fn read_bytes(bytes: Arc<Vec<u8>>, how: ReadAs) -> Vec<Selected> {
    let text = String::from_utf8_lossy(&bytes).into_owned();
    match how {
        ReadAs::Text => vec![Selected {
            value: Value::String(text),
            bytes: Some(bytes),
        }],
        ReadAs::Json => vec![Selected {
            value: serde_json::from_slice(&bytes).unwrap_or(Value::String(text)),
            bytes: Some(bytes),
        }],
        ReadAs::Yaml => vec![Selected {
            value: serde_yaml_ng::from_str::<Value>(&text).unwrap_or(Value::String(text)),
            bytes: Some(bytes),
        }],
        ReadAs::Lines => text
            .lines()
            .map(|line| Selected::plain(Value::String(line.to_owned())))
            .collect(),
        ReadAs::Jsonl => text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                Selected::plain(
                    serde_json::from_str(line).unwrap_or_else(|_| Value::String(line.to_owned())),
                )
            })
            .collect(),
    }
}

fn read_file(root: &Path, relative: &str) -> Result<Vec<Selected>, SelectFailure> {
    if !stays_below(relative) {
        return Err(outside(
            "a path is absolute or leaves the cycle's directory",
        ));
    }
    Ok(match read_inside(root, &root.join(relative))? {
        Some(bytes) => read_bytes(Arc::new(bytes), read_as_of(relative)),
        None => Vec::new(),
    })
}

/// One case of the tree with the registration that governs it.
pub(crate) struct TreeCase {
    pub(crate) case: String,
    pub(crate) protocol: String,
}

/// The JSON object of each line of `case`'s run log, `prev` left out.
fn log_lines(store: &Path, case: &str) -> Result<Vec<Value>, SelectFailure> {
    let Some(bytes) = read_inside(store, &store.join(log_file_name(case)))? else {
        return Ok(Vec::new());
    };
    let text = String::from_utf8_lossy(&bytes);
    text.lines()
        .enumerate()
        .map(|(index, line)| match serde_json::from_str::<Value>(line) {
            Ok(Value::Object(mut object)) => {
                object.remove("prev");
                Ok(Value::Object(object))
            }
            _ => Err(step_failed(format!(
                "line {index} of the run log of case {case:?} does not read"
            ))),
        })
        .collect()
}

/// The cases of the tree in `store`, the root first and each case before its children, in the
/// order they were opened, read from the root in `tree.json` and the `ChildOpened` lines.
pub(crate) fn tree_cases(store: &Path) -> Result<Vec<TreeCase>, SelectFailure> {
    let Ok(manifest) = read_manifest(store) else {
        return Ok(Vec::new());
    };
    let root_protocol = log_lines(store, &manifest.root)?
        .iter()
        .find(|line| line["kind"] == "CaseOpened")
        .and_then(|line| line["protocol"].as_str().map(str::to_owned))
        .unwrap_or_default();
    let mut cases = Vec::new();
    let mut seen = BTreeSet::new();
    let mut pending = vec![(manifest.root, root_protocol)];
    while let Some((case, protocol)) = pending.pop() {
        if !seen.insert(case.clone()) || seen.len() > 10_000 {
            continue;
        }
        let children: Vec<(String, String)> = log_lines(store, &case)?
            .iter()
            .filter(|line| line["kind"] == "ChildOpened")
            .filter_map(|line| {
                Some((
                    line["child"].as_str()?.to_owned(),
                    line["protocol"].as_str()?.to_owned(),
                ))
            })
            .collect();
        pending.extend(children.into_iter().rev());
        cases.push(TreeCase { case, protocol });
    }
    Ok(cases)
}

/// Whether the registration `held` is the protocol `named`: the registration itself, its name
/// before `@`, or the protocol id the catalog registers as it (`tour.leg` is `tour-leg@1`).
pub(crate) fn is_protocol(held: &str, named: &str) -> bool {
    held == named
        || held.split_once('@').is_some_and(|(stem, major)| {
            stem == named || loom_protocols::registration_for(named, major).as_deref() == Some(held)
        })
}

/// The cases `selector` names, and whether it names several (whose items then carry `case`).
fn cases_of(selector: &CaseSelector, store: &Path) -> Result<(Vec<String>, bool), SelectFailure> {
    if let Some(path) = &selector.path {
        let root = read_manifest(store)
            .map(|manifest| manifest.root)
            .unwrap_or_default();
        let case = if path == "." {
            root
        } else {
            format!("{root}.{path}")
        };
        return Ok((vec![case], false));
    }
    let cases = tree_cases(store)?;
    let chosen = cases
        .into_iter()
        .filter(|case| {
            if selector.every {
                true
            } else if let Some(protocol) = &selector.protocol {
                is_protocol(&case.protocol, protocol)
            } else if let Some(name) = &selector.name {
                case.case.rsplit('.').next() == Some(name.as_str())
            } else {
                false
            }
        })
        .map(|case| case.case)
        .collect();
    Ok((chosen, true))
}

fn with_case(value: Value, case: &str, several: bool) -> Selected {
    match value {
        Value::Object(mut object) if several => {
            object.insert("case".into(), Value::String(case.to_owned()));
            Selected::plain(Value::Object(object))
        }
        other => Selected::plain(other),
    }
}

/// The `canon-evidence/1` records `case` holds, in the order it received them, each with the
/// stored evidence id beside it as `evidence_id` when the record names none of its own.
fn records_of(store: &Path, case: &str) -> Result<Vec<Value>, SelectFailure> {
    let path = store.join(format!("{}.json", case_file_stem(case)));
    let Some(bytes) = read_inside(store, &path)? else {
        return Ok(Vec::new());
    };
    let stored: Value = serde_json::from_slice(&bytes)
        .map_err(|_| step_failed(format!("the case file of {case:?} does not read")))?;
    Ok(stored["evidence"]
        .as_array()
        .map(|evidence| {
            evidence
                .iter()
                .filter_map(|entry| entry["data"]["facts"].as_object().cloned())
                .map(Value::Object)
                .collect()
        })
        .unwrap_or_default())
}

/// The ids a stored evidence entry is known by: its record's `id` and the stored `evidence_id`.
fn evidence_index(store: &Path) -> Result<BTreeMap<String, Value>, SelectFailure> {
    let mut index = BTreeMap::new();
    for case in tree_cases(store)? {
        let path = store.join(format!("{}.json", case_file_stem(&case.case)));
        let Some(bytes) = read_inside(store, &path)? else {
            continue;
        };
        let stored: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        for entry in stored["evidence"].as_array().into_iter().flatten() {
            let facts = &entry["data"]["facts"];
            for id in [&facts["id"], &entry["data"]["evidence_id"]] {
                if let Some(id) = id.as_str() {
                    index.entry(id.to_owned()).or_insert_with(|| facts.clone());
                }
            }
        }
    }
    Ok(index)
}

fn runs_of(store: &Path, case: &str) -> Result<Vec<Value>, SelectFailure> {
    let Some(bytes) = read_inside(store, &store.join("observations.json"))? else {
        return Ok(Vec::new());
    };
    let observations: Value = serde_json::from_slice(&bytes)
        .map_err(|_| step_failed("observations.json does not read"))?;
    Ok(observations
        .as_array()
        .into_iter()
        .flatten()
        .filter(|observation| observation["source"] == PROGRAM_SOURCE)
        .map(|observation| observation["payload"].clone())
        .filter(|payload| payload["case"] == case)
        .collect())
}

fn root_of(area: CycleArea, view: &CycleView<'_>) -> PathBuf {
    match area {
        CycleArea::Work => view.work.to_path_buf(),
        CycleArea::Store => view.store.to_path_buf(),
    }
}

/// The relative paths below `root` matching `glob`, files and directories, in byte order; a link
/// is listed and never entered.
fn glob(root: &Path, glob: &str) -> Result<Vec<Selected>, SelectFailure> {
    let pattern: Vec<&str> = glob
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    let mut found = Vec::new();
    let mut visits = 0usize;
    let mut pending = vec![(root.to_path_buf(), Vec::<String>::new())];
    while let Some((dir, prefix)) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            visits += 1;
            if visits > MAX_GLOB_VISITS {
                return Err(too_large(format!(
                    "a glob visited more than {MAX_GLOB_VISITS} entries"
                )));
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let mut relative = prefix.clone();
            relative.push(name);
            let segments: Vec<&str> = relative.iter().map(String::as_str).collect();
            if glob_matches(&pattern, &segments) {
                found.push(relative.join("/"));
                if found.len() > MAX_GLOB_PATHS {
                    return Err(too_large(format!(
                        "a glob gave more than {MAX_GLOB_PATHS} paths"
                    )));
                }
            }
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                pending.push((entry.path(), relative));
            }
        }
    }
    found.sort();
    Ok(found
        .into_iter()
        .map(|path| Selected::plain(Value::String(path)))
        .collect())
}

fn source(item: &SelectItem, view: &CycleView<'_>) -> Result<Vec<Selected>, SelectFailure> {
    if let Some(log) = &item.log {
        let (cases, several) = cases_of(&log.case, view.store)?;
        let mut values = Vec::new();
        for case in cases {
            for line in log_lines(view.store, &case)? {
                let kind = line["kind"].as_str().unwrap_or_default();
                if log.event.is_empty() || log.event.iter().any(|event| event == kind) {
                    values.push(with_case(line, &case, several));
                }
            }
        }
        return Ok(values);
    }
    if let Some(records) = &item.records {
        let (cases, several) = cases_of(&records.case, view.store)?;
        let mut values = Vec::new();
        for case in cases {
            for record in records_of(view.store, &case)? {
                let kind = record["kind"].as_str().unwrap_or_default();
                if records.kind.is_empty() || records.kind.iter().any(|named| named == kind) {
                    values.push(with_case(record, &case, several));
                }
            }
        }
        return Ok(values);
    }
    if let Some(runs) = &item.runs {
        let (cases, several) = cases_of(&runs.case, view.store)?;
        let mut values = Vec::new();
        for case in cases {
            for run in runs_of(view.store, &case)? {
                values.push(with_case(run, &case, several));
            }
        }
        return Ok(values);
    }
    if let Some(cases) = &item.cases {
        let status = crate::tree_status(view.store, view.catalog)
            .map_err(|refused| step_failed(format!("tree status: {:?}", refused.reason())))?;
        let status = crate::status_json(&status);
        return Ok(status["cases"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|case| match &cases.protocol {
                Some(protocol) => {
                    is_protocol(case["protocol"].as_str().unwrap_or_default(), protocol)
                }
                None => true,
            })
            .cloned()
            .map(Selected::plain)
            .collect());
    }
    if let Some(doc) = &item.doc {
        return read_file(&root_of(doc.r#in, view), &doc.path);
    }
    if let Some(files) = &item.files {
        return glob(&root_of(files.r#in, view), &files.glob);
    }
    if let Some(step) = &item.step {
        return Ok(view
            .steps
            .get(step)
            .cloned()
            .map(Selected::plain)
            .into_iter()
            .collect());
    }
    if let Some(file) = &item.suite {
        return read_file(view.suite, file).map_err(|failure| SelectFailure {
            detail: failure
                .detail
                .replace("cycle's directory", "suite's directory"),
            ..failure
        });
    }
    // `requests` and `calls` wait for their stories; a cycle using one is pending, never run.
    Err(step_failed("a source this binary does not run"))
}

/// Whether `text` is a SHA-256 named as 64 lower-case hex digits.
fn is_digest(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn follow_blob(
    value: &Value,
    member: Option<&str>,
    store: &Path,
) -> Result<Option<Selected>, SelectFailure> {
    let digest = match value {
        Value::String(text) => text.as_str(),
        other => match other
            .get(member.unwrap_or("blob_digest"))
            .and_then(Value::as_str)
        {
            Some(digest) => digest,
            None => return Ok(None),
        },
    };
    let digest = digest.strip_prefix("sha256:").unwrap_or(digest);
    if !is_digest(digest) {
        return Ok(None);
    }
    let blobs = store.join("blobs");
    let Some(bytes) = read_inside(store, &blobs.join(digest))? else {
        return Ok(None);
    };
    if sha256_hex(&bytes) != digest {
        return Err(failure(
            DifferenceReason::BlobChanged,
            "a blob's bytes do not hash to its name",
        ));
    }
    let bytes = Arc::new(bytes);
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    Ok(Some(Selected {
        value,
        bytes: Some(bytes),
    }))
}

fn apply(
    operation: &SelectItem,
    values: Vec<Selected>,
    view: &CycleView<'_>,
) -> Result<Vec<Selected>, SelectFailure> {
    if let Some(conditions) = &operation.r#where {
        let mut kept = Vec::new();
        for selected in values {
            if holds(&selected.value, conditions)? {
                kept.push(selected);
            }
        }
        return Ok(kept);
    }
    if let Some(pick) = &operation.pick {
        let length = i64::try_from(values.len()).unwrap_or(i64::MAX);
        let at = match pick.position {
            PickPosition::First => 0,
            PickPosition::Last => length - 1,
            PickPosition::Index => {
                let index = pick.index.unwrap_or(0);
                if index < 0 { length + index } else { index }
            }
        };
        return Ok(usize::try_from(at)
            .ok()
            .and_then(|at| values.into_iter().nth(at))
            .into_iter()
            .collect());
    }
    if let Some(path) = &operation.path {
        let path = parse_value_path(path).map_err(step_failed)?;
        return Ok(values
            .iter()
            .flat_map(|selected| values_at(&selected.value, &path))
            .map(Selected::plain)
            .collect());
    }
    if let Some(follow) = &operation.follow {
        let mut followed = Vec::new();
        match follow.target {
            FollowTarget::Blob => {
                for selected in &values {
                    followed.extend(follow_blob(
                        &selected.value,
                        follow.member.as_deref(),
                        view.store,
                    )?);
                }
            }
            FollowTarget::Evidence => {
                let index = evidence_index(view.store)?;
                for selected in &values {
                    let ids: Vec<&str> = match &selected.value {
                        Value::String(id) => vec![id.as_str()],
                        other => other["evidence"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .collect(),
                    };
                    for id in ids {
                        if let Some(record) = index.get(id) {
                            followed.push(Selected::plain(record.clone()));
                        }
                    }
                }
            }
            FollowTarget::File => {
                for selected in &values {
                    let Value::String(named) = &selected.value else {
                        continue;
                    };
                    let relative = match &follow.below {
                        Some(below) => format!("{}/{below}", named.trim_end_matches('/')),
                        None => named.clone(),
                    };
                    followed.extend(read_file(view.work, &relative)?);
                }
            }
        }
        return Ok(followed);
    }
    if let Some(how) = operation.r#as {
        return Ok(values
            .into_iter()
            .flat_map(|selected| {
                let bytes = match (&selected.bytes, &selected.value) {
                    (Some(bytes), _) => Some(bytes.clone()),
                    (None, Value::String(text)) => Some(Arc::new(text.clone().into_bytes())),
                    _ => None,
                };
                match bytes {
                    Some(bytes) => read_bytes(bytes, how),
                    None => vec![selected],
                }
            })
            .collect());
    }
    Ok(values)
}
