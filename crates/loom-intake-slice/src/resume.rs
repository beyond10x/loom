//! The resume context (`intake.routing.ResumeContext`, `ess/intake/domains/routing.yaml`): what a
//! later resume needs to rebuild a commission's run without the arguments of the run that started
//! it, kept in the directory `run --state` keeps the case and the commission's Suspension in.
//!
//! [`ResumeContextStore`] keeps one file per commission, `resume-<hex of the commission id>.json`,
//! holding the generated [`ResumeContextData`] as a JSON object with one member per declared field.
//! Its write discipline is the governor's file stores' (`loom_governor::FileCaseStore`,
//! `loom_governor::FileSuspensionStore`): every operation holds an exclusive lock on `<dir>/.lock`,
//! the lock those stores take, and a write goes to a fresh temporary file in the same directory
//! (`.tmp-…`, which those stores remove when a dead process left one), is synced, and is renamed
//! over its target, so a reader sees no document or the whole one. Before anything is written it
//! is read back and compared with what was meant. A context is kept once: a second one for the
//! same commission is refused and changes nothing. A document naming a member its declaration does
//! not, missing one, or holding another commission's context is refused on read.
//!
//! The context grants nothing: a resume still needs a grant, and the runtime revalidates the
//! suspended request against the case as it is then.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use intake_model::confinement::Backend;
use intake_model::context::ContextPolicy;
use intake_model::primitives::{Decimal, Uuid};
pub use intake_model::routing::{
    AcceptedRoute, AgentRevisionId, CaseId, CatalogDigest, CommissionId, ResumeContextData,
};
use serde_json::{Map, Value, json};

/// The lock file the governor's file stores take; this store takes the same one.
const LOCK: &str = ".lock";

/// The prefix of a temporary file; the governor's file stores remove the ones a dead process left.
const TEMPORARY: &str = ".tmp-";

/// The prefix of a kept context's file name.
const PREFIX: &str = "resume-";

/// The longest commission id, in bytes, the store keeps: `resume-`, 200 hex digits and `.json`
/// is 212 bytes, under the 255 a file name may have.
pub const LONGEST_COMMISSION_ID: usize = 100;

/// The members a kept context has, in the order written.
const MEMBERS: [&str; 12] = [
    "commission_id",
    "case_id",
    "agent_revision_id",
    "intent",
    "route",
    "workspace",
    "test_command",
    "max_steps",
    "confinement",
    "cgroup_root",
    "model",
    "catalog",
];

/// The members a kept route has.
const ROUTE_MEMBERS: [&str; 5] = [
    "protocol",
    "intent_revision",
    "catalog",
    "context_policy",
    "threshold",
];

/// Temporary file names this process has used, so two writes never share one.
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// Why the store could not keep or read a context: the file and what went wrong with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeStoreError {
    /// The file or directory the operation was about.
    pub path: PathBuf,
    /// What went wrong.
    pub problem: String,
}

impl fmt::Display for ResumeStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.problem)
    }
}

impl std::error::Error for ResumeStoreError {}

fn failed(path: &Path, problem: impl fmt::Display) -> ResumeStoreError {
    ResumeStoreError {
        path: path.to_owned(),
        problem: problem.to_string(),
    }
}

/// The resume contexts kept in one directory, one file per commission.
#[derive(Debug)]
pub struct ResumeContextStore {
    dir: PathBuf,
}

impl ResumeContextStore {
    /// A store over `dir`, created with its lock file when it does not exist yet. `dir` may be the
    /// directory the governor's file stores keep their files in.
    ///
    /// # Errors
    /// [`ResumeStoreError`] when the directory or its lock file cannot be created.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, ResumeStoreError> {
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

    /// Keeps `context` for its commission.
    ///
    /// # Errors
    /// [`ResumeStoreError`] when a context is already kept for the commission, the context would
    /// not read back as written, or the file cannot be written; nothing is kept then.
    pub fn keep(&self, context: &ResumeContextData) -> Result<(), ResumeStoreError> {
        let path = self.path(&context.commission_id)?;
        let text = encode(context);
        if decode(&text, &context.commission_id).as_ref() != Ok(context) {
            return Err(failed(&path, "the context would not read back as written"));
        }
        self.locked(|| {
            if path.exists() {
                return Err(failed(
                    &path,
                    "a context is already kept for this commission",
                ));
            }
            self.replace(&path, &text)
        })?
    }

    /// The context kept for `commission`, or `None` when none is.
    ///
    /// # Errors
    /// [`ResumeStoreError`] when the file cannot be read or is not a context of `commission`.
    pub fn get(
        &self,
        commission: &CommissionId,
    ) -> Result<Option<ResumeContextData>, ResumeStoreError> {
        let path = self.path(commission)?;
        self.locked(|| read(&path, commission))?
    }

    /// Every context kept in the directory, by file name.
    ///
    /// # Errors
    /// [`ResumeStoreError`] when the directory or a context file cannot be read, or a file does not
    /// hold the context of the commission it is named for.
    pub fn list(&self) -> Result<Vec<ResumeContextData>, ResumeStoreError> {
        self.locked(|| {
            let mut names = Vec::new();
            for entry in fs::read_dir(&self.dir).map_err(|error| failed(&self.dir, error))? {
                let entry = entry.map_err(|error| failed(&self.dir, error))?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if let Some(commission) = commission_named(&name) {
                    names.push((name, commission));
                }
            }
            names.sort_by(|left, right| left.0.cmp(&right.0));
            let mut contexts = Vec::new();
            for (name, commission) in names {
                let path = self.dir.join(&name);
                let context = read(&path, &commission)?
                    .ok_or_else(|| failed(&path, "removed while the directory was read"))?;
                contexts.push(context);
            }
            Ok(contexts)
        })?
    }

    /// The file that holds `commission`'s context, named so no id names a file outside the store.
    fn path(&self, commission: &CommissionId) -> Result<PathBuf, ResumeStoreError> {
        let id = commission.0.0.as_bytes();
        if id.len() > LONGEST_COMMISSION_ID {
            return Err(failed(
                &self.dir,
                format!(
                    "a commission id of {} bytes is longer than the {LONGEST_COMMISSION_ID} this \
                     store keeps",
                    id.len()
                ),
            ));
        }
        let mut name = String::from(PREFIX);
        for byte in id {
            name.push_str(&format!("{byte:02x}"));
        }
        name.push_str(".json");
        Ok(self.dir.join(name))
    }

    /// Runs `operation` holding the directory's lock. The outer error is the lock's.
    fn locked<T>(&self, operation: impl FnOnce() -> T) -> Result<T, ResumeStoreError> {
        let path = self.dir.join(LOCK);
        let lock = OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|error| failed(&path, error))?;
        lock.lock().map_err(|error| failed(&path, error))?;
        let result = operation();
        let _ = lock.unlock();
        Ok(result)
    }

    /// Writes `text` to a new temporary file in the directory, syncs it, and renames it over
    /// `path`; on failure the temporary file is removed and `path` is untouched. Only called
    /// holding the lock.
    fn replace(&self, path: &Path, text: &str) -> Result<(), ResumeStoreError> {
        let (temporary, mut file) = loop {
            let temporary = self.dir.join(format!(
                "{TEMPORARY}{}-resume-{}",
                std::process::id(),
                NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => break (temporary, file),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
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
        if let Ok(dir) = File::open(&self.dir) {
            let _ = dir.sync_all();
        }
        Ok(())
    }
}

/// The commission a file name holds the context of, or `None` for any other file.
fn commission_named(name: &str) -> Option<CommissionId> {
    let hex = name.strip_prefix(PREFIX)?.strip_suffix(".json")?;
    if hex.len() % 2 != 0 {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
        .collect();
    String::from_utf8(bytes?)
        .ok()
        .map(|id| CommissionId(Uuid(id)))
}

/// The context in `path`, or `None` when there is no such file. Only called holding the lock.
fn read(
    path: &Path,
    commission: &CommissionId,
) -> Result<Option<ResumeContextData>, ResumeStoreError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(failed(path, error)),
    };
    decode(&text, commission)
        .map(Some)
        .map_err(|problem| failed(path, problem))
}

/// `context` as the JSON document the store writes.
fn encode(context: &ResumeContextData) -> String {
    let route = &context.route;
    let catalog: Vec<Value> = route
        .catalog
        .iter()
        .map(|entry| json!({"name": entry.name, "digest": entry.digest}))
        .collect();
    let value = json!({
        "commission_id": context.commission_id.0.0,
        "case_id": context.case_id.0,
        "agent_revision_id": context.agent_revision_id.0.0,
        "intent": context.intent,
        "route": {
            "protocol": route.protocol,
            "intent_revision": route.intent_revision,
            "catalog": catalog,
            "context_policy": policy_name(route.context_policy),
            "threshold": route.threshold.0,
        },
        "workspace": context.workspace,
        "test_command": context.test_command,
        "max_steps": context.max_steps,
        "confinement": backend_name(context.confinement),
        "cgroup_root": context.cgroup_root,
        "model": context.model,
        "catalog": context.catalog,
    });
    let mut text = serde_json::to_string_pretty(&value).unwrap_or_default();
    text.push('\n');
    text
}

/// The context `text` holds, refused unless it is `commission`'s and names exactly its members.
fn decode(text: &str, commission: &CommissionId) -> Result<ResumeContextData, String> {
    let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let object = exactly(&value, &MEMBERS, "the context")?;
    let route = exactly(&object["route"], &ROUTE_MEMBERS, "route")?;
    let catalog = route["catalog"]
        .as_array()
        .ok_or("route.catalog is not a list")?
        .iter()
        .map(|entry| {
            let entry = exactly(entry, &["name", "digest"], "a route.catalog entry")?;
            Ok(CatalogDigest {
                name: text_of(entry, "name")?,
                digest: text_of(entry, "digest")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let context = ResumeContextData {
        commission_id: CommissionId(Uuid(text_of(object, "commission_id")?)),
        case_id: CaseId(text_of(object, "case_id")?),
        agent_revision_id: AgentRevisionId(Uuid(text_of(object, "agent_revision_id")?)),
        intent: text_of(object, "intent")?,
        route: AcceptedRoute {
            protocol: text_of(route, "protocol")?,
            intent_revision: text_of(route, "intent_revision")?,
            catalog,
            context_policy: match route["context_policy"].as_str() {
                Some("Legacy") => ContextPolicy::Legacy,
                Some("Bounded") => ContextPolicy::Bounded,
                _ => return Err("route.context_policy is not Legacy or Bounded".into()),
            },
            threshold: Decimal(text_of(route, "threshold")?),
        },
        workspace: optional_text(object, "workspace")?,
        test_command: object["test_command"]
            .as_array()
            .ok_or("test_command is not a list")?
            .iter()
            .map(|word| {
                word.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| "test_command holds a word that is not text".to_owned())
            })
            .collect::<Result<_, _>>()?,
        max_steps: object["max_steps"]
            .as_i64()
            .ok_or("max_steps is not an integer")?,
        confinement: match object["confinement"].as_str() {
            Some("Substrate") => Backend::Substrate,
            Some("None") => Backend::None,
            _ => return Err("confinement is not Substrate or None".into()),
        },
        cgroup_root: optional_text(object, "cgroup_root")?,
        model: text_of(object, "model")?,
        catalog: optional_text(object, "catalog")?,
    };
    if &context.commission_id != commission {
        return Err(format!(
            "holds the context of commission {}, not {}",
            context.commission_id.0.0, commission.0.0
        ));
    }
    Ok(context)
}

/// `value` as an object naming exactly `members`.
fn exactly<'v>(
    value: &'v Value,
    members: &[&str],
    what: &str,
) -> Result<&'v Map<String, Value>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{what} is not an object"))?;
    if let Some(unknown) = object.keys().find(|key| !members.contains(&key.as_str())) {
        return Err(format!(
            "{what} names `{unknown}`, which it does not declare"
        ));
    }
    if let Some(missing) = members.iter().find(|member| !object.contains_key(**member)) {
        return Err(format!("{what} has no `{missing}`"));
    }
    Ok(object)
}

fn text_of(object: &Map<String, Value>, member: &str) -> Result<String, String> {
    object[member]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("`{member}` is not text"))
}

fn optional_text(object: &Map<String, Value>, member: &str) -> Result<Option<String>, String> {
    match &object[member] {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text.clone())),
        _ => Err(format!("`{member}` is neither text nor null")),
    }
}

fn policy_name(policy: ContextPolicy) -> &'static str {
    match policy {
        ContextPolicy::Legacy => "Legacy",
        ContextPolicy::Bounded => "Bounded",
    }
}

fn backend_name(backend: Backend) -> &'static str {
    match backend {
        Backend::Substrate => "Substrate",
        Backend::None => "None",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(commission: &str) -> ResumeContextData {
        ResumeContextData {
            commission_id: CommissionId(Uuid(commission.to_owned())),
            case_id: CaseId("case-1".to_owned()),
            agent_revision_id: AgentRevisionId(Uuid(
                "00000000-0000-4000-8000-000000000002".to_owned(),
            )),
            intent: "Make the \"check\" pass.\n".to_owned(),
            route: AcceptedRoute {
                protocol: "software-change@1".to_owned(),
                intent_revision: "intent-0123".to_owned(),
                catalog: vec![CatalogDigest {
                    name: "software-change@1".to_owned(),
                    digest: "ab".repeat(32),
                }],
                context_policy: ContextPolicy::Bounded,
                threshold: Decimal("0.5".to_owned()),
            },
            workspace: Some("/work/space".to_owned()),
            test_command: vec!["grep".to_owned(), "-qx".to_owned()],
            max_steps: 20,
            confinement: Backend::Substrate,
            cgroup_root: None,
            model: "lab-agent".to_owned(),
            catalog: Some("/catalog.toml".to_owned()),
        }
    }

    fn scratch(label: &str) -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(&format!("resume-store-{label}-"))
            .tempdir()
            .expect("a scratch directory")
    }

    #[test]
    fn a_kept_context_reads_back_as_kept_and_is_kept_once() {
        let dir = scratch("kept");
        let store = ResumeContextStore::open(dir.path()).unwrap();
        let first = context("00000000-0000-4000-8000-000000000001");
        let second = context("00000000-0000-4000-8000-000000000003");
        store.keep(&first).unwrap();
        store.keep(&second).unwrap();
        assert_eq!(
            store.get(&first.commission_id).unwrap(),
            Some(first.clone())
        );
        assert_eq!(store.list().unwrap(), [first.clone(), second]);
        let mut again = first.clone();
        again.model = "another".to_owned();
        assert!(store.keep(&again).is_err());
        assert_eq!(store.get(&first.commission_id).unwrap(), Some(first));
        let left: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(TEMPORARY))
            .collect();
        assert!(left.is_empty(), "{left:?}");
    }

    #[test]
    fn an_absent_context_is_none_and_a_foreign_or_extended_one_is_refused() {
        let dir = scratch("refused");
        let store = ResumeContextStore::open(dir.path()).unwrap();
        let kept = context("00000000-0000-4000-8000-000000000001");
        let other = CommissionId(Uuid("00000000-0000-4000-8000-000000000009".to_owned()));
        assert_eq!(store.get(&other).unwrap(), None);
        // Another commission's context under this commission's name.
        fs::write(store.path(&other).unwrap(), encode(&kept)).unwrap();
        assert!(store.get(&other).is_err());
        assert!(store.list().is_err());
        fs::remove_file(store.path(&other).unwrap()).unwrap();
        // A member the declaration does not name.
        let mut value: Value = serde_json::from_str(&encode(&kept)).unwrap();
        value["authority"] = json!("granted");
        fs::write(store.path(&kept.commission_id).unwrap(), value.to_string()).unwrap();
        assert!(store.get(&kept.commission_id).is_err());
    }
}
