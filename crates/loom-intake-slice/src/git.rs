//! Every git command the slice runs on the host (story `host-git-hardening`).
//!
//! `.git` lies inside the work tree a test command can write, so whatever the test command runs can
//! plant a hook in `.git/hooks` or a command in `.git/config`, and the next git call the slice
//! makes would run it with the operator's rights. Every git call is therefore built here, and only
//! here:
//!
//! - `core.hooksPath` names an empty directory the slice made for that one call and removes after
//!   it, so no hook runs, whatever the workspace's or the operator's configuration says;
//! - `core.fsmonitor=false`, `commit.gpgsign=false` and `tag.gpgsign=false`, so no monitor and no
//!   signing program runs;
//! - the variables that redirect git to another repository ([`crate::case::REDIRECTING_GIT_VARIABLES`]) are
//!   removed.
//!
//! When a case opens, `record` takes the SHA-256 of the workspace's git configuration files: the
//! repository's `config` and `config.worktree` and its `info/attributes`, as `git rev-parse
//! --git-path` names them, each as absent when it does not exist. Every later call (`recorded`)
//! reads them again and is refused with [`HostGitRefusal::ConfigChanged`] when one changed, appeared
//! or vanished, so a filter driver, a `gpg.program`, a `core.sshCommand` or anything else a test
//! wrote there never reaches git. A workspace no case opened in this process is refused with
//! [`HostGitRefusal::Unrecorded`]. Opening a case again on the same workspace records it again.
//!
//! The operator's own global and system configuration is read as before, and git run by the
//! operator or a bot outside Loom is untouched.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use sha2::{Digest, Sha256};

use crate::case::REDIRECTING_GIT_VARIABLES;

/// The configuration every host git call carries, before its own arguments.
const OVERRIDES: [&str; 3] = [
    "core.fsmonitor=false",
    "commit.gpgsign=false",
    "tag.gpgsign=false",
];

/// The files [`record`] digests, as `git rev-parse --git-path` names them.
const RECORDED_FILES: [&str; 3] = ["config", "config.worktree", "info/attributes"];

/// Why a host git call was not made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostGitRefusal {
    /// No case opened on `workspace` in this process, so there is no recording to check against.
    Unrecorded { workspace: PathBuf },
    /// `path`, a git configuration file of the workspace, changed, appeared or vanished since the
    /// case opened.
    ConfigChanged { path: PathBuf },
    /// `path` could not be read.
    Unreadable { path: PathBuf, problem: String },
    /// The empty hooks directory could not be made.
    NoHooksDirectory { problem: String },
}

impl fmt::Display for HostGitRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unrecorded { workspace } => write!(
                f,
                "no case opened on `{}` in this process; host git is refused",
                workspace.display()
            ),
            Self::ConfigChanged { path } => write!(
                f,
                "`{}` changed since the case opened; host git is refused",
                path.display()
            ),
            Self::Unreadable { path, problem } => {
                write!(f, "`{}` cannot be read: {problem}", path.display())
            }
            Self::NoHooksDirectory { problem } => {
                write!(f, "the empty hooks directory cannot be made: {problem}")
            }
        }
    }
}

impl std::error::Error for HostGitRefusal {}

/// A git command for the host, with its hooks directory; dropping it removes the directory.
pub(crate) struct HostGit {
    command: Command,
    hooks: PathBuf,
}

impl HostGit {
    /// The command, to add arguments to and run while this value lives.
    pub(crate) fn command(&mut self) -> &mut Command {
        &mut self.command
    }
}

impl Drop for HostGit {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.hooks);
    }
}

/// One workspace's recorded configuration: each file and its digest, `None` when absent.
struct Recording {
    workspace: PathBuf,
    files: Vec<(PathBuf, Option<[u8; 32]>)>,
}

static RECORDINGS: Mutex<Vec<Recording>> = Mutex::new(Vec::new());

/// A git command in `dir` that does not check a recording: only for opening a case.
pub(crate) fn opening(dir: &Path) -> Result<HostGit, HostGitRefusal> {
    build(dir)
}

/// A git command in `workspace`, refused unless the workspace's configuration is as [`record`]
/// found it.
pub(crate) fn recorded(workspace: &Path) -> Result<HostGit, HostGitRefusal> {
    let key = canonical(workspace)?;
    {
        let recordings = RECORDINGS.lock().unwrap_or_else(PoisonError::into_inner);
        let recording = recordings
            .iter()
            .find(|recording| recording.workspace == key)
            .ok_or_else(|| HostGitRefusal::Unrecorded {
                workspace: key.clone(),
            })?;
        for (path, digest) in &recording.files {
            if &digest_of(path)? != digest {
                return Err(HostGitRefusal::ConfigChanged { path: path.clone() });
            }
        }
    }
    build(&key)
}

/// Records the git configuration files of the work tree rooted at `workspace`, replacing any
/// earlier recording of it.
pub(crate) fn record(workspace: &Path) -> Result<(), HostGitRefusal> {
    let key = canonical(workspace)?;
    let mut git = opening(&key)?;
    git.command().arg("rev-parse");
    for file in RECORDED_FILES {
        git.command().args(["--git-path", file]);
    }
    let output = git
        .command()
        .output()
        .map_err(|error| HostGitRefusal::Unreadable {
            path: key.clone(),
            problem: format!("git did not run: {error}"),
        })?;
    if !output.status.success() {
        return Err(HostGitRefusal::Unreadable {
            path: key,
            problem: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let printed = String::from_utf8(output.stdout).map_err(|_| HostGitRefusal::Unreadable {
        path: key.clone(),
        problem: "git printed a path that is not UTF-8".to_owned(),
    })?;
    let paths: Vec<PathBuf> = printed.lines().map(|line| key.join(line)).collect();
    if paths.len() != RECORDED_FILES.len() {
        return Err(HostGitRefusal::Unreadable {
            path: key,
            problem: format!("git printed {} paths for {RECORDED_FILES:?}", paths.len()),
        });
    }
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let digest = digest_of(&path)?;
        files.push((path, digest));
    }
    let mut recordings = RECORDINGS.lock().unwrap_or_else(PoisonError::into_inner);
    recordings.retain(|recording| recording.workspace != key);
    recordings.push(Recording {
        workspace: key,
        files,
    });
    Ok(())
}

/// The one place git is started on the host.
fn build(dir: &Path) -> Result<HostGit, HostGitRefusal> {
    let hooks = empty_directory()?;
    let mut command = Command::new("git");
    command
        .arg("-c")
        .arg(format!("core.hooksPath={}", hooks.display()));
    for setting in OVERRIDES {
        command.args(["-c", setting]);
    }
    command.current_dir(dir);
    for variable in REDIRECTING_GIT_VARIABLES {
        command.env_remove(variable);
    }
    Ok(HostGit { command, hooks })
}

/// A new, empty directory only this process's user may enter, under the temporary directory.
fn empty_directory() -> Result<PathBuf, HostGitRefusal> {
    static MADE: AtomicU64 = AtomicU64::new(0);
    let base = std::env::temp_dir();
    let mut last = String::new();
    for _ in 0..8 {
        let made = MADE.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.subsec_nanos())
            .unwrap_or_default();
        let path = base.join(format!(
            "b10x-loom-no-hooks-{}-{made}-{nanos}",
            std::process::id()
        ));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(error) => last = error.to_string(),
        }
    }
    Err(HostGitRefusal::NoHooksDirectory { problem: last })
}

fn canonical(workspace: &Path) -> Result<PathBuf, HostGitRefusal> {
    workspace
        .canonicalize()
        .map_err(|error| HostGitRefusal::Unreadable {
            path: workspace.to_path_buf(),
            problem: error.to_string(),
        })
}

/// The SHA-256 of `path`'s bytes; `None` when it does not exist.
fn digest_of(path: &Path) -> Result<Option<[u8; 32]>, HostGitRefusal> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(Sha256::digest(&bytes).into())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(HostGitRefusal::Unreadable {
            path: path.to_path_buf(),
            problem: error.to_string(),
        }),
    }
}
