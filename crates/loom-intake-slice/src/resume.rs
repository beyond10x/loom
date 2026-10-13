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
//! not, missing one, or holding another commission's context is refused on read; a listing holds
//! every context it can read and names each file it cannot ([`ResumeContexts`]), so one damaged
//! file hides no other.
//!
//! Opening the store proves the directory takes a write, as the governor's stores do: holding the
//! lock, it creates a temporary file, writes it, syncs it and removes it. A lock file an earlier
//! open left is no such proof, so a directory that has since become read-only is refused on open.
//!
//! A context is kept before its commission runs, so a run that dies after holding the commission's
//! Suspension still leaves what resumes it. A run that ends without the Suspension held removes the
//! context ([`ResumeContextStore::remove`], `intake.routing.DropResumeContext`): nothing can resume
//! that commission. A run that dies before it ends leaves its context behind with no Suspension, and
//! a store cannot tell that orphan from a context waiting on its resume. **A reader pairs each
//! context with the commission's Suspension and never takes the listing alone for the commissions
//! that can be resumed.**
//!
//! The context grants nothing: a resume still needs a grant, and the runtime revalidates the
//! suspended request against the case as it is then.

use std::fmt;
use std::fs;
use std::io;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use intake_model::confinement::Backend;
use intake_model::context::ContextPolicy;
use intake_model::obligation::UnmetObligation;
use intake_model::primitives::{Decimal, Uuid};
use intake_model::routing::obligations::DropResumeContextBehavior;
pub use intake_model::routing::{
    AcceptedRoute, AgentRevisionId, CaseId, CatalogDigest, CommissionId, DropResumeContext,
    DropResumeContextOutcome, ResumeContextData, ResumeContextNotFound, ResumeContextStorageFailed,
};
use serde_json::{Map, Value, json};

use crate::state_dir::{StateDir, failed, read_text};

/// The prefix of a kept context's file name.
const PREFIX: &str = "resume-";

/// The longest commission id, in bytes, the store keeps: `resume-`, 200 hex digits and `.json`
/// is 212 bytes, under the 255 a file name may have.
pub const LONGEST_COMMISSION_ID: usize = crate::state_dir::LONGEST_ID;

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

/// A context file the store lists but cannot read: the commission its name is for, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadableContext {
    /// The commission the file is named for.
    pub commission_id: CommissionId,
    /// The file, and why it holds no context of that commission.
    pub error: ResumeStoreError,
}

/// What [`ResumeContextStore::list`] found: every context it read, by file name, and every
/// context file it could not read. It dereferences to the contexts it read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResumeContexts {
    /// The contexts read, in file name order.
    pub contexts: Vec<ResumeContextData>,
    /// The context files that could not be read, in file name order.
    pub unreadable: Vec<UnreadableContext>,
}

impl Deref for ResumeContexts {
    type Target = [ResumeContextData];

    fn deref(&self) -> &Self::Target {
        &self.contexts
    }
}

impl<'c> IntoIterator for &'c ResumeContexts {
    type Item = &'c ResumeContextData;
    type IntoIter = std::slice::Iter<'c, ResumeContextData>;

    fn into_iter(self) -> Self::IntoIter {
        self.contexts.iter()
    }
}

impl IntoIterator for ResumeContexts {
    type Item = ResumeContextData;
    type IntoIter = std::vec::IntoIter<ResumeContextData>;

    fn into_iter(self) -> Self::IntoIter {
        self.contexts.into_iter()
    }
}

/// The resume contexts kept in one directory, one file per commission.
#[derive(Debug)]
pub struct ResumeContextStore {
    dir: StateDir,
}

impl ResumeContextStore {
    /// A store over `dir`, created with its lock file when it does not exist yet, and proven to
    /// take a write. `dir` may be the directory the governor's file stores keep their files in.
    ///
    /// # Errors
    /// [`ResumeStoreError`] when the directory or its lock file cannot be created, or the directory
    /// does not take a write: a temporary file cannot be created, written, synced and removed in
    /// it.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, ResumeStoreError> {
        Ok(Self {
            dir: StateDir::open(dir.into(), "resume")?,
        })
    }

    /// The directory the store keeps its files in.
    pub fn dir(&self) -> &Path {
        self.dir.dir()
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
        self.dir.locked(|| {
            if path.exists() {
                return Err(failed(
                    &path,
                    "a context is already kept for this commission",
                ));
            }
            self.dir.replace(&path, &text)
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
        self.dir.locked(|| read(&path, commission))?
    }

    /// Every context kept in the directory, by file name: the ones read, and each context file
    /// that cannot be read or does not hold the context of the commission it is named for. One
    /// such file hides no other context.
    ///
    /// Not every listed context can be resumed: a run that died before it ended leaves its
    /// context with no Suspension (see the module documentation).
    ///
    /// # Errors
    /// [`ResumeStoreError`] when the directory itself cannot be locked or read.
    pub fn list(&self) -> Result<ResumeContexts, ResumeStoreError> {
        let dir = self.dir.dir();
        self.dir.locked(|| {
            let mut names = Vec::new();
            for entry in fs::read_dir(dir).map_err(|error| failed(dir, error))? {
                let entry = entry.map_err(|error| failed(dir, error))?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if let Some(commission) = commission_named(&name) {
                    names.push((name, commission));
                }
            }
            names.sort_by(|left, right| left.0.cmp(&right.0));
            let mut listed = ResumeContexts::default();
            for (name, commission) in names {
                match read(&dir.join(&name), &commission) {
                    Ok(Some(context)) => listed.contexts.push(context),
                    // Removed since the directory was read: no context, and nothing unreadable.
                    Ok(None) => {}
                    Err(error) => listed.unreadable.push(UnreadableContext {
                        commission_id: commission,
                        error,
                    }),
                }
            }
            Ok(listed)
        })?
    }

    /// Removes `input`'s commission's context (`intake.routing.DropResumeContext`): `Dropped` once
    /// it is gone, `NotFound` when none was kept, and `StorageFailed`, with the store's reason,
    /// when the file cannot be removed; the context is kept then.
    pub fn remove(&self, input: &DropResumeContext) -> DropResumeContextOutcome {
        let storage_failed = |error: ResumeStoreError| DropResumeContextOutcome::StorageFailed {
            error: ResumeContextStorageFailed {
                reason: error.to_string(),
            },
        };
        let path = match self.path(&input.commission_id) {
            Ok(path) => path,
            Err(error) => return storage_failed(error),
        };
        let removed = self.dir.locked(|| match fs::remove_file(&path) {
            Ok(()) => {
                // The file is gone; a failed directory sync is not a failed remove.
                self.dir.sync();
                DropResumeContextOutcome::Dropped
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                DropResumeContextOutcome::NotFound {
                    error: ResumeContextNotFound {
                        commission_id: input.commission_id.clone(),
                    },
                }
            }
            Err(error) => storage_failed(failed(&path, error)),
        });
        removed.unwrap_or_else(storage_failed)
    }

    /// The file that holds `commission`'s context, named so no id names a file outside the store.
    fn path(&self, commission: &CommissionId) -> Result<PathBuf, ResumeStoreError> {
        self.dir.file(PREFIX, "a commission id", &commission.0.0)
    }
}

/// `intake.routing.DropResumeContext`, the obligation the specification leaves to the store: the
/// store decides `storage-failed` by whether its file can be removed.
impl DropResumeContextBehavior for ResumeContextStore {
    fn drop_resume_context(
        &mut self,
        input: DropResumeContext,
    ) -> Result<DropResumeContextOutcome, UnmetObligation> {
        Ok(self.remove(&input))
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
    let Some(text) = read_text(path)? else {
        return Ok(None);
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
    use crate::state_dir::{LOCK, TEMPORARY};

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
        assert_eq!(store.list().unwrap().contexts, [first.clone(), second]);
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

    /// A directory the store cannot write is refused when the store opens, though an earlier
    /// open left its lock there; nothing is left behind.
    #[test]
    fn a_directory_the_store_cannot_write_is_refused_when_it_opens() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = scratch("read-only");
        ResumeContextStore::open(dir.path()).unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
        if fs::write(dir.path().join("probe"), "").is_ok() {
            fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
            panic!("this test needs a directory its user cannot write");
        }
        let opened = ResumeContextStore::open(dir.path());
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        let error = opened.expect_err("a directory the store cannot write is refused");
        assert!(error.path.starts_with(dir.path()), "{error}");
        let left: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, [LOCK]);
    }

    /// One context file that cannot be read does not hide the others: the listing holds every
    /// readable context and names the unreadable file.
    #[test]
    fn one_unreadable_context_does_not_hide_the_others() {
        let dir = scratch("unreadable");
        let store = ResumeContextStore::open(dir.path()).unwrap();
        let kept = context("00000000-0000-4000-8000-000000000001");
        store.keep(&kept).unwrap();
        let broken = CommissionId(Uuid("00000000-0000-4000-8000-000000000002".to_owned()));
        let broken_path = store.path(&broken).unwrap();
        fs::write(&broken_path, "{\"commission_id\":").unwrap();
        let listed = store
            .list()
            .expect("one unreadable file does not refuse the listing");
        assert_eq!(listed.len(), 1, "{listed:?}");
        assert_eq!(listed[0], kept);
        assert_eq!(listed.unreadable.len(), 1, "{listed:?}");
        assert_eq!(listed.unreadable[0].commission_id, broken);
        assert_eq!(listed.unreadable[0].error.path, broken_path);
    }

    /// A context is removed once: `Dropped`, then `NotFound`; another commission's context stays.
    #[test]
    fn a_removed_context_is_gone_and_removing_it_again_is_not_found() {
        let dir = scratch("remove");
        let mut store = ResumeContextStore::open(dir.path()).unwrap();
        let first = context("00000000-0000-4000-8000-000000000001");
        let second = context("00000000-0000-4000-8000-000000000003");
        store.keep(&first).unwrap();
        store.keep(&second).unwrap();
        let input = DropResumeContext {
            commission_id: first.commission_id.clone(),
        };
        assert_eq!(store.remove(&input), DropResumeContextOutcome::Dropped);
        assert_eq!(store.get(&first.commission_id).unwrap(), None);
        assert_eq!(
            store.drop_resume_context(input),
            Ok(DropResumeContextOutcome::NotFound {
                error: ResumeContextNotFound {
                    commission_id: first.commission_id.clone()
                }
            })
        );
        assert_eq!(store.list().unwrap().contexts, [second]);
    }

    /// A context whose file cannot be removed is `StorageFailed`, naming why, and is kept.
    #[test]
    fn a_context_that_cannot_be_removed_is_storage_failed_and_kept() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = scratch("remove-denied");
        let store = ResumeContextStore::open(dir.path()).unwrap();
        let kept = context("00000000-0000-4000-8000-000000000001");
        store.keep(&kept).unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let outcome = store.remove(&DropResumeContext {
            commission_id: kept.commission_id.clone(),
        });
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        let DropResumeContextOutcome::StorageFailed { error } = outcome else {
            panic!("a file the store cannot remove is storage-failed: {outcome:?}");
        };
        assert!(error.reason.contains("denied"), "{}", error.reason);
        assert_eq!(store.get(&kept.commission_id).unwrap(), Some(kept));
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
        let listed = store.list().unwrap();
        assert!(listed.contexts.is_empty(), "{listed:?}");
        assert_eq!(listed.unreadable.len(), 1, "{listed:?}");
        assert_eq!(listed.unreadable[0].commission_id, other);
        fs::remove_file(store.path(&other).unwrap()).unwrap();
        // A member the declaration does not name.
        let mut value: Value = serde_json::from_str(&encode(&kept)).unwrap();
        value["authority"] = json!("granted");
        fs::write(store.path(&kept.commission_id).unwrap(), value.to_string()).unwrap();
        assert!(store.get(&kept.commission_id).is_err());
    }
}
