//! `tree open`: the root case of a tree in an empty store, from a registered protocol, a
//! `loom.bindings/1` file and the initial artifact revisions.
//!
//! Everything `tree open` reads is checked before it writes anything: the store (absent or
//! empty), the workspace (an existing directory), `--at`, the root's bindings file and, through the
//! `bindings` member of each `open` executor, every bindings file of the tree, each decoded and
//! checked against its protocol; a binding with an `agent` executor anywhere in the tree is
//! refused `AgentNotRunYet`, naming the file, the action and [`crate::AGENT_STORY`]. The root
//! case is then opened in memory with Canon's checks, so a revision the governor refuses is
//! refused before the store is made; only then is the store created, held, and the case, the
//! manifest and the first line of the root's run log written.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use b10x_canon::model::Protocol;
use b10x_loom_bindings::{check, content_revision, decode};
use b10x_loom_commission::model::responsibility::{CaseId, ObservationData};
use loom::bindings::{BindingsDocument, Executor};
use loom::runlog::{CaseOpened, RunLogEvent};
use loom::tree::{TreeCatalog, TreeManifest, TreeRefusalReason};
use loom_governor::{
    CanonGovernor, CaseIndependence, CaseInputs, CaseState, CaseStore, FallibleCaseStore as _,
    FileCaseStore, MemoryCaseStore,
};
use loom_protocols::ProtocolCatalog;

use crate::lock::{HOLDER_FILE, LOCK_FILE, StoreLock};
use crate::log::Logs;
use crate::protocols::{KeptProtocol, keep};
use crate::store::{
    MANIFEST, MAX_DOCUMENT_BYTES, absolute, document, manifest_json, read_regular, write_file,
};
use crate::{AGENT_STORY, Refused, TreeTime, unreadable};

/// How deep the bindings files of a tree may nest below the root's.
pub const MAX_TREE_DEPTH: usize = 16;

/// What `tree open` is given.
#[derive(Debug)]
pub struct OpenRequest<'a> {
    /// The store directory: absent, or an empty directory.
    pub store: PathBuf,
    /// The existing directory every case of the tree works in.
    pub workspace: PathBuf,
    /// The registration of the root's protocol, `name@major`.
    pub protocol: String,
    /// The root's `loom.bindings/1` file.
    pub bindings: PathBuf,
    /// The initial revision of artifacts, by artifact; an artifact not named takes the revision of
    /// what is at the path the bindings place it at.
    pub revisions: Vec<(String, String)>,
    /// The id of the root case.
    pub case: String,
    /// Where the command reads its instants.
    pub time: TreeTime,
    /// The tree's catalog, which the governor admits exactly.
    pub catalog: &'a ProtocolCatalog,
    /// The protocols a `--protocols` directory gave ([`crate::read_protocols_directory`]),
    /// kept in the store; `catalog` is then theirs. Empty for the host's catalog.
    pub protocols: Vec<KeptProtocol>,
}

/// A tree `tree open` opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedTree {
    /// The store directory, absolute.
    pub store: PathBuf,
    /// The root case.
    pub root: String,
    /// The registration of the root's protocol.
    pub protocol: String,
    /// The workspace, absolute.
    pub workspace: PathBuf,
}

/// A bindings file of the tree, read and checked.
#[derive(Debug, Clone)]
pub(crate) struct LoadedBindings {
    /// The file, absolute.
    pub(crate) file: PathBuf,
    /// The document.
    pub(crate) document: BindingsDocument,
}

/// Opens the tree `request` names (see the module documentation).
///
/// # Errors
/// [`Refused`], with nothing written: `StoreNotEmpty`, `WorkspaceUnusable`, `BindingsRefused`,
/// `AgentNotRunYet`, `UnknownProtocol`, `InvalidRevision`, `StoreUnreadable` and, when another
/// command took the store while this one checked it, `StoreInUse`.
pub fn open_tree(request: &OpenRequest<'_>) -> Result<OpenedTree, Refused> {
    let time = request.time.clone();
    let store = vacant_store(&request.store)?;
    let workspace = workspace(&request.workspace)?;
    let entry = request.catalog.get(&request.protocol).ok_or_else(|| {
        Refused::new(
            if request.protocols.is_empty() {
                TreeRefusalReason::UnknownProtocol
            } else {
                TreeRefusalReason::ProtocolNotInDirectory
            },
            format!(
                "the tree's catalog holds no protocol registered as {:?}",
                crate::time::cut(&request.protocol)
            ),
        )
    })?;
    let root_file = absolute_file(&request.bindings)?;
    let root = load(&root_file, entry.name(), &entry.model)?;
    walk(
        &root,
        &entry.model,
        request.catalog,
        0,
        &mut BTreeSet::new(),
    )?;
    let revisions = initial_revisions(request, &entry.model, &root.document, &workspace)?;
    let independence = independence(&root)?;

    // Canon's checks of the opening, in memory: what the governor refuses is refused before the
    // store is made.
    let staged = MemoryCaseStore::default();
    let governor = CanonGovernor::new(Staged(&staged))
        .with_catalog(request.catalog)
        .map_err(|error| {
            Refused::new(
                TreeRefusalReason::UnknownProtocol,
                format!("the tree's catalog is refused: {error}"),
            )
        })?;
    let case = CaseId(request.case.clone());
    governor
        .open_case_with_independence(
            case.clone(),
            entry.name(),
            revisions,
            CaseInputs::none(),
            independence,
        )
        .map_err(|error| {
            Refused::new(TreeRefusalReason::InvalidRevision, error.to_string())
                .case(request.case.clone())
        })?;
    let state = CaseStore::get(&staged, &case)
        .ok_or_else(|| unreadable("the opened case", "is not held in memory"))?;
    let manifest = manifest_json(&TreeManifest {
        format: crate::FORMAT.to_owned(),
        root: request.case.clone(),
        workspace: path_text(&workspace)?,
        bindings: path_text(&root.file)?,
        catalog: if request.protocols.is_empty() {
            TreeCatalog::Host
        } else {
            TreeCatalog::Directory
        },
    });

    fs::create_dir_all(&store).map_err(|error| unreadable(store.display(), error))?;
    let store = fs::canonicalize(&store).map_err(|error| unreadable(store.display(), error))?;
    let _held = StoreLock::create(&store)?;
    // Another command may have written between the check above and the lock.
    if let Some(name) = occupied(&store)? {
        return Err(Refused::new(
            TreeRefusalReason::StoreNotEmpty,
            format!("{} holds {name:?}", store.display()),
        ));
    }
    if !request.protocols.is_empty() {
        keep(&store, &request.protocols)?;
    }
    let cases =
        FileCaseStore::open(store.clone()).map_err(|error| unreadable("the case store", error))?;
    let inserted = cases
        .insert(state)
        .map_err(|error| unreadable("the root case", error))?;
    if !inserted {
        return Err(unreadable("the root case", "is held already"));
    }
    write_file(&store, MANIFEST, &document(&manifest))?;
    let logs = Logs::open(&store, time)?;
    logs.append(
        &request.case,
        RunLogEvent::CaseOpened(CaseOpened {
            case: request.case.clone(),
            protocol: entry.name().to_owned(),
        }),
    )?;
    Ok(OpenedTree {
        store,
        root: request.case.clone(),
        protocol: entry.name().to_owned(),
        workspace,
    })
}

/// A store `tree open` may make: absent, or an empty directory. Nothing is created here.
fn vacant_store(store: &Path) -> Result<PathBuf, Refused> {
    let store = absolute(store, "store", TreeRefusalReason::StoreUnreadable)?;
    match fs::symlink_metadata(store) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(store.to_owned()),
        Err(error) => Err(unreadable(store.display(), error)),
        Ok(entry) if !entry.is_dir() => Err(Refused::new(
            TreeRefusalReason::StoreNotEmpty,
            format!("{} is not a directory", store.display()),
        )),
        Ok(_) => match occupied(store)? {
            Some(name) => Err(Refused::new(
                TreeRefusalReason::StoreNotEmpty,
                format!("{} holds {name:?}", store.display()),
            )),
            None => Ok(store.to_owned()),
        },
    }
}

/// The first entry of the store directory `store` other than the lock and holder files, when it
/// holds one.
fn occupied(store: &Path) -> Result<Option<String>, Refused> {
    let entries = fs::read_dir(store).map_err(|error| unreadable(store.display(), error))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| unreadable(store.display(), error))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name != LOCK_FILE
            && name != HOLDER_FILE
            && !name.starts_with(&format!(".{HOLDER_FILE}-"))
        {
            names.push(name);
        }
    }
    names.sort();
    Ok(names.into_iter().next())
}

/// The workspace as an absolute path, which must be an existing directory.
fn workspace(workspace: &Path) -> Result<PathBuf, Refused> {
    let unusable = |detail: String| Refused::new(TreeRefusalReason::WorkspaceUnusable, detail);
    let workspace = absolute(workspace, "workspace", TreeRefusalReason::WorkspaceUnusable)?;
    let resolved = fs::canonicalize(workspace)
        .map_err(|error| unusable(format!("{}: {error}", workspace.display())))?;
    if !resolved.is_dir() {
        return Err(unusable(format!(
            "{} is not a directory",
            workspace.display()
        )));
    }
    Ok(resolved)
}

/// `file` as an absolute path to an existing file.
fn absolute_file(file: &Path) -> Result<PathBuf, Refused> {
    let file = absolute(file, "bindings file", TreeRefusalReason::BindingsRefused)?;
    fs::canonicalize(file).map_err(|error| {
        Refused::new(
            TreeRefusalReason::BindingsRefused,
            format!("the bindings file cannot be read: {error}"),
        )
        .file(file.display().to_string())
    })
}

/// The text of `path`, which must be valid UTF-8.
fn path_text(path: &Path) -> Result<String, Refused> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| unreadable(path.display(), "is not a UTF-8 path"))
}

/// The bindings file `file`, decoded and checked against `protocol`, registered as
/// `registration`.
pub(crate) fn load(
    file: &Path,
    registration: &str,
    protocol: &Protocol,
) -> Result<LoadedBindings, Refused> {
    let named = file.display().to_string();
    let refused = |detail: String| {
        Refused::new(TreeRefusalReason::BindingsRefused, detail).file(named.clone())
    };
    let bytes = read_regular(file)?.ok_or_else(|| refused("the file does not exist".into()))?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(refused("the file is larger than 1 MiB".into()));
    }
    let text = String::from_utf8(bytes).map_err(|_| refused("the file is not UTF-8".into()))?;
    let document = decode(&text).map_err(|refusal| refused(refusal.to_string()))?;
    // An agent binding is named before any other defect of the file.
    agent_refusal(file, &document)?;
    check(&document, registration, protocol).map_err(|refusal| refused(refusal.to_string()))?;
    // With `program_root: bindings` a program path is the file beside the bindings file.
    let directory = file.parent().unwrap_or(file);
    let document = b10x_loom_bindings::resolve_programs(&document, directory)
        .map_err(|refusal| refused(refusal.to_string()))?;
    Ok(LoadedBindings {
        file: file.to_owned(),
        document,
    })
}

/// `AgentNotRunYet` for the first binding of `document`, read from `file`, with an `agent`
/// executor.
fn agent_refusal(file: &Path, document: &BindingsDocument) -> Result<(), Refused> {
    match document
        .actions
        .iter()
        .find(|bound| bound.executor.kind == Executor::Agent)
    {
        Some(bound) => {
            let mut refused = Refused::new(
                TreeRefusalReason::AgentNotRunYet,
                format!(
                    "action {} is bound to an agent executor, which no tree runs before {AGENT_STORY}",
                    bound.action
                ),
            )
            .file(file.display().to_string())
            .action(bound.action.clone());
            refused.0.story = Some(AGENT_STORY.to_owned());
            Err(refused)
        }
        None => Ok(()),
    }
}

/// Reads, through the `bindings` member of every `open` executor of `loaded`, each child's
/// bindings file (relative to the directory of `loaded`'s file), checked against the protocol
/// `case_open` names, resolved by its id to the one registration of the catalog that declares it,
/// and the files below it, to [`MAX_TREE_DEPTH`].
fn walk(
    loaded: &LoadedBindings,
    protocol: &Protocol,
    catalog: &ProtocolCatalog,
    depth: usize,
    visiting: &mut BTreeSet<PathBuf>,
) -> Result<(), Refused> {
    if depth > MAX_TREE_DEPTH || !visiting.insert(loaded.file.clone()) {
        return Ok(());
    }
    let directory = loaded.file.parent().map(Path::to_owned).unwrap_or_default();
    for bound in &loaded.document.actions {
        if bound.executor.kind != Executor::Open {
            continue;
        }
        let refused = |detail: String| {
            Refused::new(TreeRefusalReason::BindingsRefused, detail)
                .file(loaded.file.display().to_string())
                .action(bound.action.clone())
        };
        let Some(opening) = protocol
            .actions
            .iter()
            .find(|(declared, _)| declared.to_string() == bound.action)
            .and_then(|(_, declared)| declared.case_open.as_ref())
        else {
            return Err(refused(format!(
                "action {} is bound to open, and its protocol declares no case_open on it",
                bound.action
            )));
        };
        let id = opening.protocol.as_str();
        let candidates: Vec<_> = catalog
            .iter()
            .filter(|candidate| candidate.model.protocol.id.as_str() == id)
            .collect();
        let [child] = candidates.as_slice() else {
            return Err(refused(format!(
                "the tree's catalog holds {} registrations of protocol {id}, which case_open of {} names; it must hold one",
                candidates.len(),
                bound.action
            )));
        };
        let Some(relative) = bound.executor.bindings.as_deref() else {
            return Err(refused(
                "an open executor names its child's bindings file".into(),
            ));
        };
        let file = directory.join(relative);
        // A link among the directories of the path may lead anywhere: the file read is the one
        // whose real path lies below the directory of the file that names it.
        let real = fs::canonicalize(&file).map_err(|error| {
            refused(format!(
                "the child bindings file {relative:?} cannot be read: {error}"
            ))
        })?;
        if !real.starts_with(&directory) {
            return Err(refused(format!(
                "the child bindings file {relative:?} lies outside the directory of the file that names it"
            )));
        }
        let loaded_child = load(&real, child.name(), &child.model)?;
        walk(&loaded_child, &child.model, catalog, depth + 1, visiting)?;
    }
    visiting.remove(&loaded.file);
    Ok(())
}

/// The revision of every artifact `protocol` declares: the one `request` gives, else that of what
/// is at the path `document` places it at.
fn initial_revisions(
    request: &OpenRequest<'_>,
    protocol: &Protocol,
    document: &BindingsDocument,
    workspace: &Path,
) -> Result<BTreeMap<String, String>, Refused> {
    let invalid = |detail: String| Refused::new(TreeRefusalReason::InvalidRevision, detail);
    let declared: BTreeSet<String> = protocol.artifacts.ids().map(ToString::to_string).collect();
    let mut revisions = BTreeMap::new();
    for (artifact, revision) in &request.revisions {
        if !declared.contains(artifact) {
            return Err(invalid(format!(
                "artifact {:?} is not one protocol {} declares",
                crate::time::cut(artifact),
                request.protocol
            )));
        }
        if revisions
            .insert(artifact.clone(), revision.clone())
            .is_some()
        {
            return Err(invalid(format!(
                "artifact {artifact} is given two revisions"
            )));
        }
    }
    for artifact in &declared {
        if revisions.contains_key(artifact) {
            continue;
        }
        let Some(placed) = document
            .artifacts
            .iter()
            .find(|placed| placed.artifact == *artifact)
        else {
            return Err(invalid(format!(
                "artifact {artifact} is given no revision and the bindings place it at no path"
            )));
        };
        let revision = content_revision(workspace, &placed.path).map_err(|(reason, detail)| {
            invalid(format!(
                "artifact {artifact} at {}: {reason:?}: {detail}",
                placed.path
            ))
        })?;
        revisions.insert(artifact.clone(), revision);
    }
    Ok(revisions)
}

/// The independence the root's bindings file declares.
fn independence(root: &LoadedBindings) -> Result<CaseIndependence, Refused> {
    root.document
        .independence
        .iter()
        .try_fold(CaseIndependence::none(), |independence, declared| {
            independence.with(
                declared.producer.as_str(),
                declared.dimensions.iter().cloned(),
            )
        })
        .map_err(|error| {
            Refused::new(TreeRefusalReason::BindingsRefused, error.to_string())
                .file(root.file.display().to_string())
        })
}

/// A memory store a governor stages the opening in.
struct Staged<'a>(&'a MemoryCaseStore);

impl CaseStore for Staged<'_> {
    fn insert(&self, state: CaseState) -> bool {
        CaseStore::insert(self.0, state)
    }

    fn get(&self, case: &CaseId) -> Option<CaseState> {
        CaseStore::get(self.0, case)
    }

    fn update<T>(&self, case: &CaseId, change: impl FnOnce(&mut CaseState) -> T) -> Option<T> {
        CaseStore::update(self.0, case, change)
    }

    fn observe(&self, observation: ObservationData) {
        CaseStore::observe(self.0, observation);
    }

    fn observations(&self) -> Vec<ObservationData> {
        CaseStore::observations(self.0)
    }
}
