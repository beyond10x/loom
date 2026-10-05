//! Opens the slice's case through the governor (story `case-frontier`).
//!
//! The slice never evaluates Canon. It derives the revision of every artifact the picked protocol
//! declares, hands them to the governor's `open`, and later reports the workspace's new `HEAD`
//! through the governor's `update_revision`. Frontier and completion are the governor's.
//!
//! # Revisions a case starts with
//!
//! - `intent`: `intent-sha256-` followed by the lowercase hex SHA-256 of the intent text's UTF-8
//!   bytes ([`intent_revision`]). The prefix keeps it apart from `r0` and from a git object name.
//! - `implementation`: the workspace's git `HEAD`, the full object name `git rev-parse` prints.
//! - every other artifact the protocol declares: `r0`.
//!
//! # Workspace
//!
//! The workspace is the root of a non-bare git work tree: the workspace path, resolved, is the path
//! `git rev-parse --show-toplevel` prints there, resolved. A bare repository and a `.git` directory
//! (where git refuses `--show-toplevel`), a directory inside another repository's work tree (git's
//! discovery would otherwise walk up to it) and a directory that is no repository are refused as
//! [`CaseError::Workspace`]. The root is read as the bytes git prints, less one trailing newline,
//! so a root whose name ends in whitespace or (on Unix) is not UTF-8 is still a workspace; only
//! `HEAD`'s object name must be UTF-8. Every git call runs in the
//! workspace without the inherited variables that redirect git to another repository
//! ([`REDIRECTING_GIT_VARIABLES`]), so a slice started from a git hook or under a tool that sets
//! them still reads the workspace. Git otherwise runs with the operator's normal configuration.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use b10x_loom_commission::model::responsibility::CaseId;
use loom_governor::{CanonGovernor, CaseStore, OpenError, UpdateError};
use sha2::{Digest, Sha256};

/// The artifact whose revision is the intent text's hash.
pub const INTENT: &str = "intent";
/// The artifact whose revision is the workspace's git `HEAD`.
pub const IMPLEMENTATION: &str = "implementation";
/// The revision of every other declared artifact when a case opens.
pub const INITIAL: &str = "r0";

/// Why a case was not opened, or a `HEAD` not reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseError {
    /// The pick is not `<name>@<major>` of an ELS built-in.
    UnknownProtocol { pick: String, problem: String },
    /// The workspace's `HEAD` could not be read.
    Workspace { problem: String },
    /// The governor refused to open the case.
    Open(OpenError),
    /// The governor refused the new revision.
    Update(UpdateError),
}

impl fmt::Display for CaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProtocol { pick, problem } => {
                write!(f, "`{pick}` is not an ELS built-in protocol: {problem}")
            }
            Self::Workspace { problem } => {
                write!(f, "the workspace's HEAD cannot be read: {problem}")
            }
            Self::Open(error) => write!(f, "the governor did not open the case: {error}"),
            Self::Update(error) => {
                write!(f, "the governor did not record the new HEAD: {error}")
            }
        }
    }
}

impl std::error::Error for CaseError {}

/// Opens a case on the picked protocol (`<name>@<major>`, e.g. `software-change@1`) through
/// `governor`, starting from `intent` and the git repository at `workspace`, and returns its id.
pub fn open<S: CaseStore>(
    governor: &CanonGovernor<S>,
    pick: &str,
    intent: &str,
    workspace: &Path,
) -> Result<CaseId, CaseError> {
    let declared = declared_artifacts(pick)?;
    let head = head(workspace)?;
    let revisions: BTreeMap<String, String> = declared
        .into_iter()
        .map(|artifact| {
            let revision = match artifact.as_str() {
                INTENT => intent_revision(intent),
                IMPLEMENTATION => head.clone(),
                _ => INITIAL.to_owned(),
            };
            (artifact, revision)
        })
        .collect();
    governor.open(pick, revisions).map_err(CaseError::Open)
}

/// Reports the workspace's current `HEAD` as the case's `implementation` revision and returns the
/// case revision the governor holds afterwards (one higher when `HEAD` moved, unchanged otherwise).
pub fn report_head<S: CaseStore>(
    governor: &CanonGovernor<S>,
    case: &CaseId,
    workspace: &Path,
) -> Result<i64, CaseError> {
    let head = head(workspace)?;
    governor
        .update_revision(case, IMPLEMENTATION, &head)
        .map_err(CaseError::Update)
}

/// The `intent` revision: `intent-sha256-<lowercase hex SHA-256 of the UTF-8 text>`.
pub fn intent_revision(intent: &str) -> String {
    let digest = Sha256::digest(intent.as_bytes());
    let mut revision = String::with_capacity("intent-sha256-".len() + 64);
    revision.push_str("intent-sha256-");
    for byte in digest {
        revision.push_str(&format!("{byte:02x}"));
    }
    revision
}

/// Every artifact the ELS built-in `pick` declares, in declaration order.
fn declared_artifacts(pick: &str) -> Result<Vec<String>, CaseError> {
    let refused = |problem: &str| CaseError::UnknownProtocol {
        pick: pick.to_owned(),
        problem: problem.to_owned(),
    };
    let (name, major) = pick
        .split_once('@')
        .ok_or_else(|| refused("it is not `<name>@<major>`"))?;
    let major: u32 = major
        .parse()
        .ok()
        .filter(|parsed: &u32| parsed.to_string() == major)
        .ok_or_else(|| refused("its major is not a number"))?;
    let builtin = canon_engineering::registry::get(name, major)
        .map_err(|error| refused(&error.to_string()))?;
    Ok(builtin
        .model
        .artifacts
        .ids()
        .map(|artifact| artifact.as_str().to_owned())
        .collect())
}

/// The full object name of the workspace's `HEAD` commit.
///
/// Refused unless `workspace` is the root of a non-bare work tree (see the module documentation).
fn head(workspace: &Path) -> Result<String, CaseError> {
    let refused = |problem: String| CaseError::Workspace { problem };
    let root = workspace.canonicalize().map_err(|error| {
        refused(format!(
            "`{}` cannot be resolved: {error}",
            workspace.display()
        ))
    })?;
    let printed = git_bytes(&root, &["rev-parse", "--show-toplevel"])?;
    let printed = printed.strip_suffix(b"\n").unwrap_or(&printed);
    let toplevel = path_from_bytes(printed)?;
    let toplevel = toplevel.canonicalize().map_err(|error| {
        refused(format!(
            "the work tree `{}` cannot be resolved: {error}",
            toplevel.display()
        ))
    })?;
    if toplevel != root {
        return Err(refused(format!(
            "`{}` is not the root of a work tree; its work tree is `{}`",
            root.display(),
            toplevel.display()
        )));
    }
    let head = git(&root, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    if head.is_empty() {
        return Err(refused("git printed no HEAD".to_owned()));
    }
    Ok(head)
}

/// The variables that make git read a repository other than the one its working directory is in.
pub const REDIRECTING_GIT_VARIABLES: [&str; 6] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
];

/// The path git printed as `bytes`: any bytes on Unix, UTF-8 elsewhere.
#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> Result<PathBuf, CaseError> {
    use std::os::unix::ffi::OsStrExt as _;
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
}

/// The path git printed as `bytes`: any bytes on Unix, UTF-8 elsewhere.
#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> Result<PathBuf, CaseError> {
    std::str::from_utf8(bytes)
        .map(PathBuf::from)
        .map_err(|_| CaseError::Workspace {
            problem: "git printed a work tree root that is not UTF-8".to_owned(),
        })
}

/// Runs git as [`git_bytes`] does and returns its standard output as UTF-8, trimmed.
fn git(dir: &Path, args: &[&str]) -> Result<String, CaseError> {
    String::from_utf8(git_bytes(dir, args)?)
        .map(|stdout| stdout.trim().to_owned())
        .map_err(|_| CaseError::Workspace {
            problem: format!("git {args:?} printed output that is not UTF-8"),
        })
}

/// Runs git with `args` in `dir`, without [`REDIRECTING_GIT_VARIABLES`], and returns its standard
/// output as printed; a git that does not run or fails is a [`CaseError::Workspace`].
fn git_bytes(dir: &Path, args: &[&str]) -> Result<Vec<u8>, CaseError> {
    let mut command = Command::new("git");
    command.args(args).current_dir(dir);
    for variable in REDIRECTING_GIT_VARIABLES {
        command.env_remove(variable);
    }
    let output = command.output().map_err(|error| CaseError::Workspace {
        problem: format!("git did not run: {error}"),
    })?;
    if !output.status.success() {
        return Err(CaseError::Workspace {
            problem: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(output.stdout)
}
