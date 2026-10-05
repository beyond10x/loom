//! Performs a proposed `software.change/1` action inside the workspace (story `selector-executor`).
//!
//! Three actions are performed; every other one, `repository.merge` included, is refused with
//! [`ExecuteError::NotExecuted`] before its arguments are read:
//!
//! - `repository.inspect`, `{"paths": [<path>, ...]}`: returns each file's contents
//!   ([`Report::Inspected`]).
//! - `repository.edit`, `{"files": [{"path": <path>, "contents": <text>}, ...], "message": <text>}`
//!   (`message` optional): writes the files, commits exactly those paths with the workspace's
//!   identity and no hook or signing program, and reports the new `HEAD` to the governor as the
//!   case's `implementation` revision ([`Report::Edited`]).
//! - `tests.run`, `{}`: runs the configured [`TestCommand`] in the workspace, without a shell, and
//!   delivers what it observed to the governor as an observation ([`Report::TestsRun`]). The
//!   command is killed at its timeout ([`TestCommand::DEFAULT_TIMEOUT`] unless
//!   [`TestCommand::with_timeout`] sets another), with its process group on Unix; the run is then
//!   marked timed out and has no exit code. Its standard output and error are read while it runs,
//!   and only the last 8 KiB of each is kept.
//!
//! Arguments are checked before anything is done: an argument the action does not take, or one of
//! the wrong type, is [`ExecuteError::InvalidArguments`]. A commit message holding a control
//! character other than a newline or a tab is refused the same way.
//!
//! # The workspace boundary
//!
//! A path is relative to the workspace root and names a file under it. It is refused with
//! [`ExecuteError::OutsideWorkspace`] when it is empty, absolute, contains `..` or a NUL, names the
//! git directory (`.git`, in any ASCII case, as any component) or passes through a symbolic link
//! (any existing component, the file itself included, that is a link). A path the workspace's git
//! ignores (`git check-ignore --no-index`, so tracked files too) is refused with
//! [`ExecuteError::Ignored`]: the executor neither reads it nor writes it, so an ignored file can
//! neither leave the workspace through `repository.inspect` nor be written beside a commit that
//! does not hold it.
//!
//! An edit is checked whole before any file is written: one refused path refuses the edit, and
//! nothing is written. Once writing starts the edit is atomic: if a write, `git add` or `git commit`
//! fails (a NUL in the message, a path git refuses), every written path is put back to its
//! previous bytes or removed, the directories made for it are removed and the git index is restored,
//! so the work tree is as it was. Putting back is best effort; a path that cannot be put back is
//! left as it is, and the next `tests.run` then finds an unclean work tree, which yields no
//! evidence. An edit whose commit succeeded but whose new `HEAD` the governor did not record keeps
//! its commit and returns [`ExecuteError::Case`].
//!
//! # Host git
//!
//! Every git call is host git ([`crate::git`]): it runs no hook of the workspace's, no
//! `core.fsmonitor` command and no signing program, and it is refused with
//! [`ExecuteError::HostGit`] before git starts when the workspace's git configuration files changed
//! since the case opened (a test that wrote a filter driver or a `gpg.program` into `.git/config`),
//! when its git or common directory moved (a planted `commondir`), when its own configuration
//! now names a program (a work-tree file it includes, rewritten by an edit), or when no case
//! opened on the workspace in this process. An edit refused so writes nothing and commits nothing.
//!
//! # Known limit: no sandbox
//!
//! The test command runs model-edited code with the operator's rights, the operator's environment
//! and network access. The slice has no sandbox; run it only on workspaces and models trusted with
//! that.
//!
//! # Trust
//!
//! The executor reports what it did; a report is never evidence. Only the test-result verifier
//! ([`crate::verifier`]) submits evidence, and only from the exit status of the command this
//! executor ran: a [`TestRun`] has no public constructor, and nothing a model says reaches it.

use std::collections::VecDeque;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{Read, Write as _};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    CaseId, ExecutorOutcomeProposedAction, GovernorError, Observation, ObservationData,
    ObservationId,
};
use b10x_loom_commission::ports::evidence::ObservationPort as _;
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::{CanonGovernor, CaseStore};
use sha2::{Digest, Sha256};

use crate::case::{self, CaseError, REDIRECTING_GIT_VARIABLES};
use crate::git::{self, HostGitRefusal};

/// `repository.inspect`.
pub const INSPECT: &str = "repository.inspect";
/// `repository.edit`.
pub const EDIT: &str = "repository.edit";
/// `tests.run`.
pub const TESTS_RUN: &str = "tests.run";
/// The observation source of a test run.
pub const TEST_RUN_SOURCE: &str = "intake-slice/tests.run";

/// The most output of a test run kept in its report: the tail, in bytes.
const OUTPUT_TAIL: usize = 8 * 1024;

/// The command `tests.run` runs in the workspace: a program and its arguments, never a shell line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestCommand {
    program: OsString,
    args: Vec<OsString>,
    timeout: Duration,
}

impl TestCommand {
    /// How long a test command may run unless [`TestCommand::with_timeout`] says otherwise.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

    /// `program` with `args`, under [`TestCommand::DEFAULT_TIMEOUT`].
    pub fn new<A: Into<OsString>>(
        program: impl Into<OsString>,
        args: impl IntoIterator<Item = A>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            timeout: Self::DEFAULT_TIMEOUT,
        }
    }

    /// The same command, killed once it has run for `timeout`.
    #[must_use]
    pub fn with_timeout(self, timeout: Duration) -> Self {
        Self { timeout, ..self }
    }

    /// The command as text, for observations and reports.
    fn describe(&self) -> Vec<String> {
        std::iter::once(&self.program)
            .chain(&self.args)
            .map(|part| part.to_string_lossy().into_owned())
            .collect()
    }
}

/// One file `repository.inspect` read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectedFile {
    /// The path as the arguments named it.
    pub path: String,
    /// Its contents.
    pub contents: String,
}

/// One run of the test command, as the executor observed it. Only the executor makes one.
#[derive(Debug, Clone)]
pub struct TestRun {
    case: CaseId,
    observation: ObservationId,
    exit_code: Option<i32>,
    implementation: Option<String>,
    case_revision: i64,
    timed_out: bool,
    timeout: Duration,
    output: String,
}

impl TestRun {
    /// The command's exit code; `None` when a signal ended it.
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    /// The `HEAD` the command ran on; `None` when the work tree was not clean, so the run was
    /// about no committed revision.
    pub fn implementation(&self) -> Option<&str> {
        self.implementation.as_deref()
    }

    /// The case revision when the command ran.
    pub fn case_revision(&self) -> i64 {
        self.case_revision
    }

    /// The case the run belongs to.
    pub fn case(&self) -> &CaseId {
        &self.case
    }

    /// The observation the executor delivered for the run.
    pub fn observation(&self) -> &ObservationId {
        &self.observation
    }

    /// Whether the command was killed at its timeout.
    pub fn timed_out(&self) -> bool {
        self.timed_out
    }
}

/// What the executor did. An observation, never evidence.
#[derive(Debug, Clone)]
pub enum Report {
    /// `repository.inspect`: the files, in the order named.
    Inspected(Vec<InspectedFile>),
    /// `repository.edit`: the workspace's `HEAD` afterwards, reported to the governor. Unchanged
    /// when the edit wrote what the files already held.
    Edited { revision: String },
    /// `tests.run`.
    TestsRun(TestRun),
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inspected(files) => {
                for file in files {
                    writeln!(f, "{}:\n{}", file.path, file.contents)?;
                }
                Ok(())
            }
            Self::Edited { revision } => write!(f, "committed; HEAD is {revision}"),
            Self::TestsRun(run) => {
                match run.exit_code {
                    _ if run.timed_out => write!(
                        f,
                        "the test command timed out after {} s and was killed",
                        run.timeout.as_secs_f64()
                    )?,
                    Some(code) => write!(f, "the test command exited with {code}")?,
                    None => f.write_str("the test command was ended by a signal")?,
                }
                if run.implementation.is_none() {
                    f.write_str(" on a work tree with uncommitted changes")?;
                }
                write!(f, "\n{}", run.output)
            }
        }
    }
}

/// Why an action was not performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecuteError {
    /// The slice never executes `action`.
    NotExecuted { action: String },
    /// `path` does not name a file inside the workspace.
    OutsideWorkspace { path: String },
    /// The workspace's git ignores `path`; the executor neither reads nor writes it.
    Ignored { path: String },
    /// The arguments are not what `action` takes.
    InvalidArguments { action: String, problem: String },
    /// The workspace, git or the test command failed.
    Workspace { problem: String },
    /// The new `HEAD` was not reported to the governor.
    Case(CaseError),
    /// The governor did not take the test run's observation.
    Governor(GovernorError),
    /// Host git was refused before it ran: the workspace's git configuration changed since the
    /// case opened, or no case opened on the workspace in this process.
    HostGit(HostGitRefusal),
}

impl fmt::Display for ExecuteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotExecuted { action } => write!(f, "the slice never executes `{action}`"),
            Self::OutsideWorkspace { path } => write!(f, "`{path}` is outside the workspace"),
            Self::Ignored { path } => write!(f, "`{path}` is ignored by the workspace's git"),
            Self::InvalidArguments { action, problem } => {
                write!(f, "invalid arguments for `{action}`: {problem}")
            }
            Self::Workspace { problem } => write!(f, "the workspace failed: {problem}"),
            Self::Case(error) => error.fmt(f),
            Self::Governor(error) => write!(f, "the governor refused the observation: {error:?}"),
            Self::HostGit(refusal) => refusal.fmt(f),
        }
    }
}

impl std::error::Error for ExecuteError {}

/// The JSON Schema of the arguments `action` takes, as published to the model that writes them.
pub fn arguments_schema(action: &str) -> serde_json::Value {
    let path = serde_json::json!({"type": "string", "description": "a path relative to the workspace root"});
    match action {
        INSPECT => serde_json::json!({
            "type": "object",
            "properties": {"paths": {"type": "array", "items": path}},
            "required": ["paths"],
            "additionalProperties": false
        }),
        EDIT => serde_json::json!({
            "type": "object",
            "properties": {
                "files": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {"path": path, "contents": {"type": "string"}},
                        "required": ["path", "contents"],
                        "additionalProperties": false
                    }
                },
                "message": {"type": "string"}
            },
            "required": ["files"],
            "additionalProperties": false
        }),
        TESTS_RUN => serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
        _ => serde_json::json!({"type": "object"}),
    }
}

/// Performs proposed actions in one workspace, for one case of `governor`.
pub struct LocalExecutor<'g, S> {
    governor: &'g CanonGovernor<S>,
    case: CaseId,
    workspace: PathBuf,
    test: TestCommand,
}

impl<'g, S: CaseStore> LocalExecutor<'g, S> {
    /// An executor for `case` in the git work tree at `workspace`, running `test` for `tests.run`.
    pub fn new(
        governor: &'g CanonGovernor<S>,
        case: CaseId,
        workspace: impl Into<PathBuf>,
        test: TestCommand,
    ) -> Self {
        Self {
            governor,
            case,
            workspace: workspace.into(),
            test,
        }
    }

    /// Performs `proposal`, or refuses it (see the module documentation).
    pub fn execute(
        &self,
        proposal: &ExecutorOutcomeProposedAction,
    ) -> Result<Report, ExecuteError> {
        let action = proposal.action.as_str();
        if ![INSPECT, EDIT, TESTS_RUN].contains(&action) {
            return Err(ExecuteError::NotExecuted {
                action: action.to_owned(),
            });
        }
        let arguments = Arguments::of(action, &proposal.arguments.0)?;
        let root = self
            .workspace
            .canonicalize()
            .map_err(|error| ExecuteError::Workspace {
                problem: format!("`{}` cannot be resolved: {error}", self.workspace.display()),
            })?;
        match action {
            INSPECT => self.inspect(&root, &arguments),
            EDIT => self.edit(&root, &arguments),
            _ => {
                arguments.only(&[])?;
                self.run_tests(&root)
            }
        }
    }

    fn inspect(&self, root: &Path, arguments: &Arguments<'_>) -> Result<Report, ExecuteError> {
        arguments.only(&["paths"])?;
        let paths = arguments.texts("paths")?;
        let resolved = paths
            .iter()
            .map(|path| inside(root, path))
            .collect::<Result<Vec<_>, _>>()?;
        let relatives: Vec<&OsStr> = resolved.iter().map(|(rel, _)| rel.as_os_str()).collect();
        refuse_ignored(root, &relatives)?;
        let mut files = Vec::with_capacity(paths.len());
        for (path, (_, file)) in paths.into_iter().zip(resolved) {
            let contents =
                std::fs::read_to_string(&file).map_err(|error| ExecuteError::Workspace {
                    problem: format!("`{path}` cannot be read: {error}"),
                })?;
            files.push(InspectedFile {
                path: path.to_owned(),
                contents,
            });
        }
        Ok(Report::Inspected(files))
    }

    fn edit(&self, root: &Path, arguments: &Arguments<'_>) -> Result<Report, ExecuteError> {
        arguments.only(&["files", "message"])?;
        let message = match arguments.member("message") {
            None => "intake-slice edit".to_owned(),
            Some(Value::Text(message)) if !message.trim().is_empty() => {
                if message
                    .chars()
                    .any(|c| c.is_control() && c != '\n' && c != '\t')
                {
                    return Err(arguments.invalid(
                        "`message` holds a control character other than a newline or a tab",
                    ));
                }
                message.clone()
            }
            Some(_) => return Err(arguments.invalid("`message` is not a non-empty string")),
        };
        let Some(Value::Array(entries)) = arguments.member("files") else {
            return Err(arguments.invalid("`files` is not an array"));
        };
        if entries.is_empty() {
            return Err(arguments.invalid("`files` is empty"));
        }
        let mut files = Vec::with_capacity(entries.len());
        for entry in entries {
            let entry = Arguments::of(EDIT, entry)?;
            entry.only(&["path", "contents"])?;
            let (Some(Value::Text(path)), Some(Value::Text(contents))) =
                (entry.member("path"), entry.member("contents"))
            else {
                return Err(entry.invalid("a file is not `{path, contents}` strings"));
            };
            let (relative, file) = inside(root, path)?;
            if file.is_dir() {
                return Err(entry.invalid(&format!("`{path}` is a directory")));
            }
            files.push((relative, file, contents.as_str()));
        }
        let relatives: Vec<&OsStr> = files.iter().map(|(rel, _, _)| rel.as_os_str()).collect();
        refuse_ignored(root, &relatives)?;
        let mut undo = Undo::new(root)?;
        if let Err(error) = write_and_commit(root, &files, &relatives, &message, &mut undo) {
            undo.restore();
            return Err(error);
        }
        case::report_head(self.governor, &self.case, root).map_err(ExecuteError::Case)?;
        Ok(Report::Edited {
            revision: head(root)?,
        })
    }

    fn run_tests(&self, root: &Path) -> Result<Report, ExecuteError> {
        let clean = git_output(root, &with_paths(&["status", "--porcelain"], &[]))?
            .stdout
            .is_empty();
        let implementation = if clean { Some(head(root)?) } else { None };
        let case_revision = self
            .governor
            .current_revision(&self.case)
            .map_err(ExecuteError::Governor)?;
        let finished = run_bounded(&self.test, root)?;
        let exit_code = finished.exit_code;
        let observation = ObservationId(fresh_uuid("observation", &self.case.0));
        let mut payload = vec![
            (
                "command".to_owned(),
                Value::Array(self.test.describe().into_iter().map(Value::Text).collect()),
            ),
            (
                "exit_code".to_owned(),
                exit_code.map_or(Value::Null, |code| Value::Number(code.to_string())),
            ),
            (
                "case_revision".to_owned(),
                Value::Number(case_revision.to_string()),
            ),
        ];
        payload.push((
            "implementation".to_owned(),
            implementation.clone().map_or(Value::Null, Value::Text),
        ));
        payload.push(("timed_out".to_owned(), Value::Bool(finished.timed_out)));
        self.governor
            .observe(Observation::new(ObservationData {
                observation_id: observation.clone(),
                source: TEST_RUN_SOURCE.to_owned(),
                subject: case::IMPLEMENTATION.to_owned(),
                observed_at: now(),
                payload: Value::Object(payload),
            }))
            .map_err(ExecuteError::Governor)?;
        Ok(Report::TestsRun(TestRun {
            case: self.case.clone(),
            observation,
            exit_code,
            implementation,
            case_revision,
            timed_out: finished.timed_out,
            timeout: self.test.timeout,
            output: finished.output,
        }))
    }
}

/// Writes `files`, recording in `undo` what each path held, then stages and commits exactly those
/// paths. An error leaves restoring to the caller.
fn write_and_commit(
    root: &Path,
    files: &[(PathBuf, PathBuf, &str)],
    relatives: &[&OsStr],
    message: &str,
    undo: &mut Undo,
) -> Result<(), ExecuteError> {
    for (_, file, contents) in files {
        undo.remember(file)?;
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        std::fs::write(file, contents).map_err(io)?;
    }
    git(root, &with_paths(&["add", "--"], relatives))?;
    let unchanged = git_status(
        root,
        &with_paths(
            &[
                "diff",
                "--cached",
                "--quiet",
                "--no-ext-diff",
                "--no-textconv",
                "--",
            ],
            relatives,
        ),
    )?;
    if !unchanged {
        git(
            root,
            &with_paths(
                &["commit", "--quiet", "--message", message, "--"],
                relatives,
            ),
        )?;
    }
    Ok(())
}

/// What an edit changed, to put back when it fails: each path's previous bytes (or that it did not
/// exist), the directories made for it and the git index as it was.
struct Undo {
    index: PathBuf,
    saved_index: Option<Vec<u8>>,
    files: Vec<(PathBuf, Option<Vec<u8>>)>,
    directories: Vec<PathBuf>,
}

impl Undo {
    fn new(root: &Path) -> Result<Self, ExecuteError> {
        let printed = git_output(
            root,
            &with_paths(&["rev-parse", "--git-path", "index"], &[]),
        )?;
        let index = root.join(String::from_utf8_lossy(&printed.stdout).trim_end_matches('\n'));
        let saved_index = std::fs::read(&index).ok();
        Ok(Self {
            index,
            saved_index,
            files: Vec::new(),
            directories: Vec::new(),
        })
    }

    /// Records what `file` holds now, and which of its directories do not exist yet, before it is
    /// first written.
    fn remember(&mut self, file: &Path) -> Result<(), ExecuteError> {
        if self.files.iter().any(|(held, _)| held == file) {
            return Ok(());
        }
        let mut missing = Vec::new();
        let mut directory = file.parent();
        while let Some(path) = directory {
            if std::fs::symlink_metadata(path).is_ok() {
                break;
            }
            missing.push(path.to_path_buf());
            directory = path.parent();
        }
        for path in missing.into_iter().rev() {
            if !self.directories.contains(&path) {
                self.directories.push(path);
            }
        }
        let previous = if std::fs::symlink_metadata(file).is_ok() {
            Some(std::fs::read(file).map_err(io)?)
        } else {
            None
        };
        self.files.push((file.to_path_buf(), previous));
        Ok(())
    }

    /// Puts every remembered path, directory and the index back as they were. Best effort: a path
    /// that cannot be put back is left as it is.
    fn restore(&self) {
        for (file, previous) in self.files.iter().rev() {
            let _ = match previous {
                Some(bytes) => std::fs::write(file, bytes),
                None => std::fs::remove_file(file),
            };
        }
        for directory in self.directories.iter().rev() {
            let _ = std::fs::remove_dir(directory);
        }
        let _ = match &self.saved_index {
            Some(bytes) => std::fs::write(&self.index, bytes),
            None => std::fs::remove_file(&self.index),
        };
    }
}

/// Refuses the first of `relatives` the workspace's git ignores, whether or not it is tracked.
fn refuse_ignored(root: &Path, relatives: &[&OsStr]) -> Result<(), ExecuteError> {
    let mut input = Vec::new();
    for relative in relatives {
        input.extend_from_slice(relative.to_string_lossy().as_bytes());
        input.push(0);
    }
    let mut git = git::recorded(root).map_err(ExecuteError::HostGit)?;
    let mut child = git
        .args(["check-ignore", "-z", "--stdin", "--no-index"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ExecuteError::Workspace {
            problem: format!("git did not run: {error}"),
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&input).map_err(io)?;
    }
    let output = child.wait_with_output().map_err(io)?;
    match output.status.code() {
        Some(1) => Ok(()),
        Some(0) => {
            let first = output
                .stdout
                .split(|byte| *byte == 0)
                .next()
                .unwrap_or_default();
            Err(ExecuteError::Ignored {
                path: String::from_utf8_lossy(first).into_owned(),
            })
        }
        _ => Err(ExecuteError::Workspace {
            problem: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        }),
    }
}

/// How long the readers of a finished command's output are waited for.
const OUTPUT_GRACE: Duration = Duration::from_secs(2);

/// A test command that ended, by itself or at its timeout.
struct Finished {
    exit_code: Option<i32>,
    timed_out: bool,
    output: String,
}

/// Runs `test` in `root`, without a shell, killing it (and on Unix its process group) at its
/// timeout. Standard output and error are read while it runs; the last [`OUTPUT_TAIL`] bytes of
/// each are kept.
fn run_bounded(test: &TestCommand, root: &Path) -> Result<Finished, ExecuteError> {
    let mut command = Command::new(&test.program);
    command
        .args(&test.args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for variable in REDIRECTING_GIT_VARIABLES {
        command.env_remove(variable);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|error| ExecuteError::Workspace {
        problem: format!("the test command did not start: {error}"),
    })?;
    let stdout = Tail::read(child.stdout.take());
    let stderr = Tail::read(child.stderr.take());
    let deadline = Instant::now() + test.timeout;
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                timed_out = true;
                kill(&mut child);
                break child.wait().map_err(io)?;
            }
            Err(error) => {
                kill(&mut child);
                let _ = child.wait();
                return Err(io(error));
            }
        }
    };
    let mut output = String::new();
    for (name, tail) in [("stdout", stdout), ("stderr", stderr)] {
        let text = tail.finish();
        if !text.is_empty() {
            output.push_str(&format!("{name}:\n{text}"));
            if !text.ends_with('\n') {
                output.push('\n');
            }
        }
    }
    Ok(Finished {
        exit_code: if timed_out { None } else { status.code() },
        timed_out,
        output,
    })
}

/// Kills `child`, and on Unix the process group it leads, so what it started dies with it.
fn kill(child: &mut Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-s", "KILL", "--", &format!("-{}", child.id())])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

/// The last [`OUTPUT_TAIL`] bytes of a pipe, read on a thread of its own.
struct Tail {
    kept: Arc<Mutex<VecDeque<u8>>>,
    done: mpsc::Receiver<()>,
}

impl Tail {
    fn read(pipe: Option<impl Read + Send + 'static>) -> Self {
        let kept = Arc::new(Mutex::new(VecDeque::new()));
        let (sender, done) = mpsc::channel();
        if let Some(mut pipe) = pipe {
            let held = Arc::clone(&kept);
            std::thread::spawn(move || {
                let mut buffer = [0u8; 8192];
                loop {
                    match pipe.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(read) => {
                            let mut kept = held.lock().unwrap_or_else(PoisonError::into_inner);
                            kept.extend(&buffer[..read]);
                            let excess = kept.len().saturating_sub(OUTPUT_TAIL);
                            kept.drain(..excess);
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(_) => break,
                    }
                }
                let _ = sender.send(());
            });
        }
        Self { kept, done }
    }

    /// What was kept, once the pipe closed or [`OUTPUT_GRACE`] passed: a process the command left
    /// behind may hold the pipe open.
    fn finish(self) -> String {
        let _ = self.done.recv_timeout(OUTPUT_GRACE);
        let kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        let bytes: Vec<u8> = kept.iter().copied().collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

/// The object members of an action's arguments.
struct Arguments<'a> {
    action: &'a str,
    members: &'a [(String, Value)],
}

impl<'a> Arguments<'a> {
    fn of(action: &'a str, value: &'a Value) -> Result<Self, ExecuteError> {
        let Value::Object(members) = value else {
            return Err(ExecuteError::InvalidArguments {
                action: action.to_owned(),
                problem: format!("the arguments are {}, not an object", value.describes()),
            });
        };
        let arguments = Self { action, members };
        for (n, (name, _)) in members.iter().enumerate() {
            if members[..n].iter().any(|(earlier, _)| earlier == name) {
                return Err(arguments.invalid(&format!("`{name}` is given twice")));
            }
        }
        Ok(arguments)
    }

    fn invalid(&self, problem: &str) -> ExecuteError {
        ExecuteError::InvalidArguments {
            action: self.action.to_owned(),
            problem: problem.to_owned(),
        }
    }

    fn only(&self, allowed: &[&str]) -> Result<(), ExecuteError> {
        match self
            .members
            .iter()
            .find(|(name, _)| !allowed.contains(&name.as_str()))
        {
            Some((name, _)) => Err(self.invalid(&format!("`{name}` is not an argument it takes"))),
            None => Ok(()),
        }
    }

    fn member(&self, name: &str) -> Option<&'a Value> {
        self.members
            .iter()
            .find(|(held, _)| held == name)
            .map(|(_, value)| value)
    }

    fn texts(&self, name: &str) -> Result<Vec<&'a str>, ExecuteError> {
        let Some(Value::Array(items)) = self.member(name) else {
            return Err(self.invalid(&format!("`{name}` is not an array")));
        };
        items
            .iter()
            .map(|item| match item {
                Value::Text(text) => Ok(text.as_str()),
                _ => Err(self.invalid(&format!("`{name}` holds something that is not a string"))),
            })
            .collect()
    }
}

/// `path`, relative to `root`, and resolved under it: refused unless it names a file inside the
/// workspace (see the module documentation).
fn inside(root: &Path, path: &str) -> Result<(PathBuf, PathBuf), ExecuteError> {
    let refused = || ExecuteError::OutsideWorkspace {
        path: path.to_owned(),
    };
    if path.is_empty() || path.contains('\0') {
        return Err(refused());
    }
    let mut relative = PathBuf::new();
    let mut resolved = root.to_path_buf();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(name) => {
                if name.eq_ignore_ascii_case(".git") {
                    return Err(refused());
                }
                relative.push(name);
                resolved.push(name);
                if std::fs::symlink_metadata(&resolved)
                    .is_ok_and(|metadata| metadata.file_type().is_symlink())
                {
                    return Err(refused());
                }
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(refused());
            }
        }
    }
    if relative.as_os_str().is_empty() {
        return Err(refused());
    }
    Ok((relative, resolved))
}

fn io(error: std::io::Error) -> ExecuteError {
    ExecuteError::Workspace {
        problem: error.to_string(),
    }
}

/// The full object name of `HEAD`.
fn head(root: &Path) -> Result<String, ExecuteError> {
    let output = git_output(
        root,
        &with_paths(&["rev-parse", "--verify", "HEAD^{commit}"], &[]),
    )?;
    String::from_utf8(output.stdout)
        .map(|head| head.trim().to_owned())
        .map_err(|_| ExecuteError::Workspace {
            problem: "git printed a HEAD that is not UTF-8".to_owned(),
        })
}

/// `fixed` followed by `paths`, as git arguments.
fn with_paths<'a>(fixed: &[&'a str], paths: &[&'a OsStr]) -> Vec<&'a OsStr> {
    fixed
        .iter()
        .map(|arg| OsStr::new(*arg))
        .chain(paths.iter().copied())
        .collect()
}

/// Runs host git ([`crate::git`]) with literal pathspecs in `root`; a git that fails is an error, a
/// refused one [`ExecuteError::HostGit`].
fn git(root: &Path, args: &[&OsStr]) -> Result<(), ExecuteError> {
    git_output(root, args).map(drop)
}

fn git_output(root: &Path, args: &[&OsStr]) -> Result<Output, ExecuteError> {
    let output = git_command(root, args)?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(ExecuteError::Workspace {
            problem: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }
}

/// Whether git exits 0 (`true`) or 1 (`false`); any other exit is an error.
fn git_status(root: &Path, args: &[&OsStr]) -> Result<bool, ExecuteError> {
    let output = git_command(root, args)?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(ExecuteError::Workspace {
            problem: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        }),
    }
}

fn git_command(root: &Path, args: &[&OsStr]) -> Result<Output, ExecuteError> {
    let mut git = git::recorded(root).map_err(ExecuteError::HostGit)?;
    git.arg("--literal-pathspecs")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| ExecuteError::Workspace {
            problem: format!("git did not run: {error}"),
        })
}

/// A fresh version-8 UUID: the SHA-256 of `kind`, `seed`, the process, the clock and a counter.
pub(crate) fn fresh_uuid(kind: &str, seed: &str) -> Uuid {
    static ISSUED: AtomicU64 = AtomicU64::new(0);
    let issued = ISSUED.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let digest = Sha256::digest(
        format!("{kind}\n{seed}\n{}\n{nanos}\n{issued}", std::process::id()).as_bytes(),
    );
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Uuid(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

/// The current UTC time, RFC 3339, to the second.
pub(crate) fn now() -> Timestamp {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    let days = i64::try_from(seconds / 86_400).unwrap_or_default();
    let of_day = seconds % 86_400;
    // Civil date from days since 1970-01-01 (H. Hinnant, `civil_from_days`).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    Timestamp(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        of_day / 3_600,
        of_day % 3_600 / 60,
        of_day % 60
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_that_leave_the_workspace_are_refused() {
        let root = Path::new("workspace-root-that-does-not-exist");
        for path in [
            "",
            "/etc/passwd",
            "../x",
            "a/../../x",
            "a/../b",
            ".git/config",
            "a/.GIT/x",
        ] {
            assert!(
                matches!(
                    inside(root, path),
                    Err(ExecuteError::OutsideWorkspace { .. })
                ),
                "`{path}`"
            );
        }
        let (relative, resolved) = inside(root, "./src/./lib.rs").expect("inside");
        assert_eq!(relative, Path::new("src/lib.rs"));
        assert_eq!(resolved, root.join("src/lib.rs"));
    }

    #[test]
    fn the_clock_reads_as_rfc_3339() {
        let Timestamp(text) = now();
        assert_eq!(text.len(), 20, "{text}");
        assert!(text.ends_with('Z') && text.as_bytes()[10] == b'T', "{text}");
        assert!(text.as_str() >= "2026-01-01", "{text}");
    }
}
