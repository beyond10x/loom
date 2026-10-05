//! Every git command the slice runs on the host (story `host-git-hardening`).
//!
//! `.git` lies inside the work tree a test command can write, and a configuration file can be
//! included from the work tree the model edits, so a test or the model can plant a hook or a
//! program-running key, and the next git call the slice makes would run it with the operator's
//! rights. Every git call is therefore built here, and only here:
//!
//! - `core.hooksPath=/dev/null`, so no hook runs (no file can exist under `/dev/null`), whatever
//!   the workspace's or the operator's configuration says;
//! - `core.fsmonitor=false`, `commit.gpgsign=false`, `tag.gpgsign=false` and `diff.external=`
//!   (empty, so an external diff fails instead of running), so no monitor, signing program or
//!   external diff runs;
//! - the variables that redirect git to another repository
//!   ([`crate::case::REDIRECTING_GIT_VARIABLES`]) are removed.
//!
//! When a case opens, `record` refuses the workspace with [`HostGitRefusal::ProgramKey`] when its
//! own configuration (any scope but `system`, `global` and the command line, includes followed)
//! sets a key that names a program git may run ([`names_a_program`]): a filter or merge driver, a
//! diff text converter or command, an editor, pager, SSH or askpass command, a signing program, a
//! credential helper, `uploadpack`/`receivepack` settings and the like. `core.hooksPath` and
//! `core.fsmonitor` are not refused, because the command line above overrides them. It then
//! records the git directory and the common directory (`git rev-parse --absolute-git-dir
//! --git-common-dir`) and the SHA-256 of the configuration files: `config`, `config.worktree` and
//! `info/attributes` as `git rev-parse --git-path` names them, and the git directory's
//! `commondir`, each as absent when it does not exist.
//!
//! Every later call (`recorded`) is refused before git starts:
//!
//! - [`HostGitRefusal::Unrecorded`] when no case opened on the workspace in this process;
//! - [`HostGitRefusal::GitDirChanged`] when the git directory or the common directory moved (a
//!   planted `commondir` or `.git` file);
//! - [`HostGitRefusal::ConfigChanged`] when a recorded file changed, appeared or vanished;
//! - [`HostGitRefusal::ProgramKey`] when the effective configuration now sets a program key from
//!   the workspace's own scopes (a file included from the work tree that an edit rewrote).
//!
//! Opening a case again on the same workspace records it again, under the same refusal. The
//! operator's own global and system configuration is read as before, and git run by the operator
//! or a bot outside Loom is untouched.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, PoisonError};

use sha2::{Digest, Sha256};

use crate::case::REDIRECTING_GIT_VARIABLES;

/// The configuration every host git call carries, before its own arguments.
const OVERRIDES: [&str; 5] = [
    "core.hooksPath=/dev/null",
    "core.fsmonitor=false",
    "commit.gpgsign=false",
    "tag.gpgsign=false",
    "diff.external=",
];

/// The files `record` digests, as `git rev-parse --git-path` names them.
const RECORDED_FILES: [&str; 3] = ["config", "config.worktree", "info/attributes"];

/// The configuration scopes that are the operator's or the slice's own, never the workspace's.
const TRUSTED_SCOPES: [&str; 3] = ["system", "global", "command"];

/// Why a host git call was not made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostGitRefusal {
    /// No case opened on `workspace` in this process, so there is no recording to check against.
    Unrecorded { workspace: PathBuf },
    /// `path`, a git configuration file of the workspace, changed, appeared or vanished since the
    /// case opened.
    ConfigChanged { path: PathBuf },
    /// The workspace's git directory or common directory is `now`, not `recorded` as when the
    /// case opened.
    GitDirChanged { recorded: PathBuf, now: PathBuf },
    /// The workspace's own configuration (`scope`, from `origin`) sets `key`, which names a
    /// program git may run.
    ProgramKey {
        key: String,
        scope: String,
        origin: String,
    },
    /// `path` could not be read, or git could not report on it.
    Unreadable { path: PathBuf, problem: String },
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
            Self::GitDirChanged { recorded, now } => write!(
                f,
                "the git directory moved from `{}` to `{}` since the case opened; host git is \
                 refused",
                recorded.display(),
                now.display()
            ),
            Self::ProgramKey { key, scope, origin } => write!(
                f,
                "the workspace's git configuration sets `{key}` ({scope}, {origin}), which names \
                 a program; host git is refused"
            ),
            Self::Unreadable { path, problem } => {
                write!(f, "`{}` cannot be read: {problem}", path.display())
            }
        }
    }
}

impl std::error::Error for HostGitRefusal {}

/// One workspace as its case opened: where git keeps it and each recorded file's digest, `None`
/// when absent.
#[derive(Clone)]
struct Recording {
    workspace: PathBuf,
    git_dirs: [PathBuf; 2],
    files: Vec<(PathBuf, Option<[u8; 32]>)>,
}

static RECORDINGS: Mutex<Vec<Recording>> = Mutex::new(Vec::new());

/// A git command in `dir` that does not check a recording: only for opening a case.
pub(crate) fn opening(dir: &Path) -> Command {
    build(dir)
}

/// A git command in `workspace`, refused unless the workspace is as `record` found it.
pub(crate) fn recorded(workspace: &Path) -> Result<Command, HostGitRefusal> {
    let key = canonical(workspace)?;
    let recording = RECORDINGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .find(|recording| recording.workspace == key)
        .cloned()
        .ok_or_else(|| HostGitRefusal::Unrecorded {
            workspace: key.clone(),
        })?;
    let now = git_dirs(&key)?;
    for (recorded, now) in recording.git_dirs.iter().zip(now) {
        if *recorded != now {
            return Err(HostGitRefusal::GitDirChanged {
                recorded: recorded.clone(),
                now,
            });
        }
    }
    for (path, digest) in &recording.files {
        if &digest_of(path)? != digest {
            return Err(HostGitRefusal::ConfigChanged { path: path.clone() });
        }
    }
    refuse_program_keys(&key)?;
    Ok(build(&key))
}

/// Records the work tree rooted at `workspace`, replacing any earlier recording of it; refused
/// when its own configuration sets a program key.
pub(crate) fn record(workspace: &Path) -> Result<(), HostGitRefusal> {
    let key = canonical(workspace)?;
    refuse_program_keys(&key)?;
    let git_dirs = git_dirs(&key)?;
    let mut paths = vec![git_dirs[0].join("commondir")];
    let mut command = opening(&key);
    command.arg("rev-parse");
    for file in RECORDED_FILES {
        command.args(["--git-path", file]);
    }
    let named: Vec<PathBuf> = lines(&key, &run(&key, &mut command)?)?
        .into_iter()
        .map(|line| key.join(line))
        .collect();
    if named.len() != RECORDED_FILES.len() {
        return Err(unreadable(
            &key,
            format!("git printed {} paths for {RECORDED_FILES:?}", named.len()),
        ));
    }
    paths.extend(named);
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let digest = digest_of(&path)?;
        files.push((path, digest));
    }
    let mut recordings = RECORDINGS.lock().unwrap_or_else(PoisonError::into_inner);
    recordings.retain(|recording| recording.workspace != key);
    recordings.push(Recording {
        workspace: key,
        git_dirs,
        files,
    });
    Ok(())
}

/// Whether the configuration key `key` (as `git config --list` prints it: section and name in
/// lower case) names a program git may run.
pub fn names_a_program(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    let section = lower.split('.').next().unwrap_or_default();
    let name = lower.rsplit('.').next().unwrap_or_default();
    match section {
        "filter" => matches!(name, "clean" | "smudge" | "process"),
        "diff" => matches!(name, "textconv" | "command" | "external"),
        "merge" => name == "driver",
        "core" => matches!(
            name,
            "editor" | "pager" | "sshcommand" | "askpass" | "gitproxy" | "alternaterefscommand"
        ),
        "sequence" => name == "editor",
        "gpg" => name == "program",
        "credential" => name == "helper",
        "uploadpack" | "receivepack" | "pager" => true,
        "remote" => matches!(name, "uploadpack" | "receivepack"),
        "difftool" | "mergetool" | "man" | "browser" => matches!(name, "cmd" | "path"),
        "trailer" => matches!(name, "command" | "cmd"),
        "interactive" => name == "difffilter",
        _ => false,
    }
}

/// Refused when the effective configuration in `workspace` sets a program key from a scope that
/// is not the operator's or the slice's own.
fn refuse_program_keys(workspace: &Path) -> Result<(), HostGitRefusal> {
    let mut command = opening(workspace);
    command.args(["config", "--list", "--show-scope", "--show-origin", "-z"]);
    let listed = run(workspace, &mut command)?;
    let mut fields = listed.split(|byte| *byte == 0);
    while let (Some(scope), Some(origin), Some(entry)) =
        (fields.next(), fields.next(), fields.next())
    {
        let scope = String::from_utf8_lossy(scope);
        if TRUSTED_SCOPES.contains(&scope.as_ref()) {
            continue;
        }
        let entry = String::from_utf8_lossy(entry);
        let key = entry.split('\n').next().unwrap_or_default();
        if names_a_program(key) {
            return Err(HostGitRefusal::ProgramKey {
                key: key.to_owned(),
                scope: scope.into_owned(),
                origin: String::from_utf8_lossy(origin).into_owned(),
            });
        }
    }
    Ok(())
}

/// The git directory and the common directory of `workspace`, resolved.
fn git_dirs(workspace: &Path) -> Result<[PathBuf; 2], HostGitRefusal> {
    let mut command = opening(workspace);
    command.args(["rev-parse", "--absolute-git-dir", "--git-common-dir"]);
    let printed = lines(workspace, &run(workspace, &mut command)?)?;
    let [git_dir, common] = printed.as_slice() else {
        return Err(unreadable(
            workspace,
            "git did not print a git directory and a common directory".to_owned(),
        ));
    };
    Ok([
        canonical(&workspace.join(git_dir))?,
        canonical(&workspace.join(common))?,
    ])
}

/// The one place git is started on the host.
fn build(dir: &Path) -> Command {
    let mut command = Command::new("git");
    for setting in OVERRIDES {
        command.args(["-c", setting]);
    }
    command.current_dir(dir).stdin(std::process::Stdio::null());
    for variable in REDIRECTING_GIT_VARIABLES {
        command.env_remove(variable);
    }
    command
}

/// Runs `command`; its standard output when it succeeds.
fn run(workspace: &Path, command: &mut Command) -> Result<Vec<u8>, HostGitRefusal> {
    let Output {
        status,
        stdout,
        stderr,
    } = command
        .output()
        .map_err(|error| unreadable(workspace, format!("git did not run: {error}")))?;
    if status.success() {
        Ok(stdout)
    } else {
        Err(unreadable(
            workspace,
            String::from_utf8_lossy(&stderr).trim().to_owned(),
        ))
    }
}

/// The paths git printed in `bytes`, one per line: any bytes on Unix, UTF-8 elsewhere.
fn lines(workspace: &Path, bytes: &[u8]) -> Result<Vec<PathBuf>, HostGitRefusal> {
    #[cfg(unix)]
    let _ = workspace;
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    bytes
        .split(|byte| *byte == b'\n')
        .map(|line| {
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStrExt as _;
                Ok(PathBuf::from(std::ffi::OsStr::from_bytes(line)))
            }
            #[cfg(not(unix))]
            {
                std::str::from_utf8(line).map(PathBuf::from).map_err(|_| {
                    unreadable(workspace, "git printed a path that is not UTF-8".to_owned())
                })
            }
        })
        .collect()
}

fn unreadable(path: &Path, problem: String) -> HostGitRefusal {
    HostGitRefusal::Unreadable {
        path: path.to_path_buf(),
        problem,
    }
}

fn canonical(path: &Path) -> Result<PathBuf, HostGitRefusal> {
    path.canonicalize()
        .map_err(|error| unreadable(path, error.to_string()))
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
