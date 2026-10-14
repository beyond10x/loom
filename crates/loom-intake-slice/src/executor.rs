//! Performs a proposed `software.change/1` action inside the workspace (story `selector-executor`).
//!
//! Four actions are performed; every other one, `repository.merge` and `release.rollback`
//! included, is refused with [`ExecuteError::NotExecuted`] before its arguments are read:
//!
//! - `repository.inspect`, `{"paths": [<path>, ...]}`: returns each file's contents
//!   ([`Report::Inspected`]).
//! - `tests.write` and `implementation.edit`, each
//!   `{"files": [{"path": <path>, "contents": <text>}, ...], "message": <text>}`
//!   (`message` optional): writes the files, commits exactly those paths with the workspace's
//!   identity and no hook or signing program, and reports the new `HEAD` to the governor as the
//!   case's `implementation` revision ([`Report::Edited`]). `tests.write` writes only test paths:
//!   a path the host's test-path patterns ([`TestPaths`], [`LocalExecutor::with_test_paths`]) do
//!   not match refuses it with [`ExecuteError::NotATestPath`], naming the first such path, for
//!   every change kind. `implementation.edit` is not restricted by them. An `implementation.edit`
//!   that committed also delivers an observation of the change ([`EDIT_SOURCE`]) and reports it
//!   ([`Change`]); one that wrote what the files already held, and every `tests.write`, deliver
//!   none. A `tests.write` that committed is remembered: every later run is marked
//!   [`TestRun::after_tests_write`]. One that wrote what the files already held wrote no test
//!   and is not.
//! - `tests.run`, `{}`: runs the configured [`TestCommand`] in the workspace, without a shell, and
//!   delivers what it observed to the governor as an observation ([`Report::TestsRun`]). The
//!   command is killed at its timeout ([`TestCommand::DEFAULT_TIMEOUT`] unless
//!   [`TestCommand::with_timeout`] sets another), with its process group on Unix; the run is then
//!   marked timed out and has no exit code. Its standard output and error are read while it runs,
//!   and only the last 8 KiB of each is kept. On a case whose `change_kind` is `bugfix`, when the
//!   host gave a regression-test command ([`TestCommand::with_regression`]), a `tests.write` of
//!   this executor committed and the work tree was clean, the run then runs that command once per
//!   test path those writes committed with new contents, in the order first committed, `{path}`
//!   filled, through the same runner and timeout, each delivered as its own observation
//!   ([`REGRESSION_RUN_SOURCE`], [`TestRun::regressions`]).
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
//! An edit is checked whole before any file is written: one refused path (outside the workspace,
//! not a test path for `tests.write`, ignored) refuses the edit, and nothing is written. Once
//! writing starts the edit is atomic: if a write, `git add` or `git commit` fails (a NUL in the
//! message, a path git refuses), every written path is put back to its previous bytes or removed,
//! the directories made for it are removed and the git index is restored, so the work tree is as
//! it was. Putting back is best effort; a path that cannot be put back is
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
//! # Test confinement
//!
//! Test commands use the injected runner, defaulting to Substrate. Only an explicitly supplied
//! unconfined runner uses the operator's environment and rights.
//!
//! # Trust
//!
//! The executor reports what it did; a report is never evidence. Only the test-result verifier
//! ([`crate::verifier`]) submits evidence, and only from the exit status of the command this
//! executor ran or the edit it committed: neither a [`TestRun`] nor a [`Change`] has a public
//! constructor, and nothing a model says reaches them. Every observation's `observed_at` is read
//! from the host clock ([`LocalExecutor::with_clock`], [`SystemClock`] by default); a clock that
//! gives no instant is [`ExecuteError::Clock`], and the observation is not delivered.
//!
//! Canon orders records in whole seconds, so the executor keeps its observations' instants
//! strictly increasing in whole seconds: before it delivers an observation whose host-clock
//! second is not later than its previous observation's, it waits on the clock ([`Clock::wait`])
//! for that second to pass, at most [`CLOCK_WAIT`] in all. A clock that still has not passed it
//! (one that stepped back) is [`ExecuteError::Clock`], naming both instants, and the observation
//! is not delivered, so no record is submitted for it; an edit already committed stays committed.
//! The previous observation is this executor's own, so a resume starts afresh.

use std::cell::{Cell, RefCell};
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
use loom_governor::{CanonGovernor, EvaluationTime, FallibleCaseStore, NoEvaluationTime};
use sha2::{Digest, Sha256};

use crate::case::{self, CaseError, REDIRECTING_GIT_VARIABLES};
use crate::clock::{Clock, SystemClock};
use crate::confinement::{ConfinementError, SubstrateRunner, TestExecution, TestRunner};
use crate::git::{self, HostGitRefusal};
pub use crate::test_paths::TestPaths;
use chrono::{DateTime, SecondsFormat, Utc};
use intake_model::confinement::{AppliedConfinement, Backend, ConfinementRefusal};

/// `repository.inspect`.
pub const INSPECT: &str = "repository.inspect";
/// `tests.write`: changes the tests, with the effect of [`IMPLEMENTATION_EDIT`], on the host's
/// test paths only ([`TestPaths`]).
pub const TESTS_WRITE: &str = "tests.write";
/// `implementation.edit`: changes the implementation. `software.change/1` admits it for a bugfix
/// only once its regression is reproduced.
pub const IMPLEMENTATION_EDIT: &str = "implementation.edit";
/// The two write actions, performed alike: the files are written and committed.
pub const WRITES: [&str; 2] = [TESTS_WRITE, IMPLEMENTATION_EDIT];
/// `tests.run`.
pub const TESTS_RUN: &str = "tests.run";
/// The observation source of a test run.
pub const TEST_RUN_SOURCE: &str = "intake-slice/tests.run";
/// The observation source of an `implementation.edit` that committed.
pub const EDIT_SOURCE: &str = "intake-slice/implementation.edit";
/// The observation source of a regression-test command's run on one committed test path.
pub const REGRESSION_RUN_SOURCE: &str = "intake-slice/regression-test.run";
/// The most the executor waits, in all, for the host clock to pass the second of its previous
/// observation before it delivers the next one.
pub const CLOCK_WAIT: Duration = Duration::from_secs(2);
/// The shortest single wait on the clock, so a reading just short of the next second still waits.
const MIN_CLOCK_PAUSE: Duration = Duration::from_millis(1);

/// The most output of a test run kept in its report: the tail, in bytes.
const OUTPUT_TAIL: usize = 8 * 1024;

/// The host's regression-test command (`intake.routing.SliceRun.regression_test_command`): the
/// text as given, split at white space like the test command, with [`RegressionTestCommand::PATH`]
/// in at least one word. The executor fills it, in every word, with one test path a `tests.write`
/// committed, and runs the result without a shell, under the test command's timeout and runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegressionTestCommand {
    text: String,
}

/// Why a regression-test command was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegressionTestCommandError {
    /// The command as given.
    pub command: String,
}

impl fmt::Display for RegressionTestCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the regression-test command `{}` names no `{}` word, so it would not run the test \
             file a tests.write committed",
            self.command,
            RegressionTestCommand::PATH
        )
    }
}

impl std::error::Error for RegressionTestCommandError {}

impl RegressionTestCommand {
    /// The placeholder the executor fills with a committed test path.
    pub const PATH: &'static str = "{path}";

    /// The command `text` gives.
    ///
    /// # Errors
    /// [`RegressionTestCommandError`] when no word of `text` holds [`RegressionTestCommand::PATH`]
    /// (an empty command included).
    pub fn new(text: impl Into<String>) -> Result<Self, RegressionTestCommandError> {
        let text = text.into();
        if text
            .split_whitespace()
            .any(|word| word.contains(Self::PATH))
        {
            Ok(Self { text })
        } else {
            Err(RegressionTestCommandError { command: text })
        }
    }

    /// The command as given.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The command that runs the test at `path`, under `timeout`.
    fn for_path(&self, path: &str, timeout: Duration) -> TestCommand {
        let mut words = self
            .text
            .split_whitespace()
            .map(|word| word.replace(Self::PATH, path));
        let program = words.next().unwrap_or_default();
        TestCommand::new(program, words.collect::<Vec<_>>()).with_timeout(timeout)
    }
}

/// The command `tests.run` runs in the workspace: a program and its arguments, never a shell line,
/// and the host's regression-test command when it gave one ([`TestCommand::with_regression`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestCommand {
    program: OsString,
    args: Vec<OsString>,
    timeout: Duration,
    regression: Option<RegressionTestCommand>,
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
            regression: None,
        }
    }

    /// The same command, killed once it has run for `timeout`.
    #[must_use]
    pub fn with_timeout(self, timeout: Duration) -> Self {
        Self { timeout, ..self }
    }

    /// The same command, with the host's regression-test command: on a bugfix, every `tests.run`
    /// after a `tests.write` that committed also runs it once per committed test path
    /// ([`TestRun::regressions`]), and only its result is a `regression_test_result`. Without one,
    /// no run is a regression test.
    #[must_use]
    pub fn with_regression(self, regression: RegressionTestCommand) -> Self {
        Self {
            regression: Some(regression),
            ..self
        }
    }

    /// The host's regression-test command, when it gave one.
    pub fn regression(&self) -> Option<&RegressionTestCommand> {
        self.regression.as_ref()
    }

    /// The executable selected by the operator.
    pub fn program(&self) -> &OsStr {
        &self.program
    }

    /// The operator's arguments, without shell expansion.
    pub fn args(&self) -> &[OsString] {
        &self.args
    }

    /// The maximum duration of one execution.
    pub fn timeout(&self) -> Duration {
        self.timeout
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
    applied: Box<AppliedConfinement>,
    observed_at: Timestamp,
    after_tests_write: bool,
    regressions: Vec<RegressionRun>,
}

/// One run of the host's regression-test command on one committed test path, made by a
/// `tests.run` after the suite's run. Only the executor makes one.
#[derive(Debug, Clone)]
pub struct RegressionRun {
    path: String,
    run: TestRun,
}

impl RegressionRun {
    /// The committed test path the command ran with `{path}` filled.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The command's run: its own observation, exit status and revision.
    pub fn run(&self) -> &TestRun {
        &self.run
    }
}

impl TestRun {
    /// The regression-test command's runs that followed this suite run, one per test path the
    /// executor's `tests.write` actions committed, in the order first committed: empty unless the
    /// case is a bugfix, the host gave a regression-test command, a `tests.write` committed before
    /// the run, and the work tree was clean. A regression run's own list is empty.
    pub fn regressions(&self) -> &[RegressionRun] {
        &self.regressions
    }

    /// The host clock's instant of the run's observation, UTC to the second.
    pub fn observed_at(&self) -> &Timestamp {
        &self.observed_at
    }

    /// Whether the executor had performed a `tests.write` that committed before this run: from
    /// its own history, never from a model. One that wrote what the files already held, or a
    /// refused one, wrote no test and does not count.
    pub fn after_tests_write(&self) -> bool {
        self.after_tests_write
    }

    /// The already captured, potentially lossy output tail; never the full process output.
    pub fn output_tail(&self) -> &str {
        &self.output
    }

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

    /// The confinement actually applied to this execution.
    pub fn applied_confinement(&self) -> &AppliedConfinement {
        &self.applied
    }

    /// What the executor reports of this run besides its output: how the command ended, whether
    /// the work tree had uncommitted changes, and the confinement applied. [`Report`] writes this
    /// and then the output; the briefing quotes it beside the captured output.
    pub fn summary(&self) -> String {
        let ended = match self.exit_code {
            _ if self.timed_out => format!(
                "the test command timed out after {} s and was killed",
                self.timeout.as_secs_f64()
            ),
            Some(code) => format!("the test command exited with {code}"),
            None => "the test command was ended by a signal".to_owned(),
        };
        let tree = if self.implementation.is_none() {
            " on a work tree with uncommitted changes"
        } else {
            ""
        };
        format!(
            "{ended}{tree}\nconfinement: {}",
            backend_name(self.applied.backend)
        )
    }
}

/// An `implementation.edit` that committed, moving the workspace's `HEAD`, as the executor
/// observed it. Only the executor makes one.
#[derive(Debug, Clone)]
pub struct Change {
    case: CaseId,
    observation: ObservationId,
    revision: String,
    case_revision: i64,
    observed_at: Timestamp,
}

impl Change {
    /// The case the edit belongs to.
    pub fn case(&self) -> &CaseId {
        &self.case
    }

    /// The observation the executor delivered for the edit.
    pub fn observation(&self) -> &ObservationId {
        &self.observation
    }

    /// The new `HEAD`, the implementation revision the edit committed.
    pub fn revision(&self) -> &str {
        &self.revision
    }

    /// The case revision once the new `HEAD` was reported.
    pub fn case_revision(&self) -> i64 {
        self.case_revision
    }

    /// The host clock's instant of the edit's observation, UTC to the second.
    pub fn observed_at(&self) -> &Timestamp {
        &self.observed_at
    }
}

/// What the executor did. An observation, never evidence.
#[derive(Debug, Clone)]
pub enum Report {
    /// `repository.inspect`: the files, in the order named.
    Inspected(Vec<InspectedFile>),
    /// `tests.write` and `implementation.edit`: the workspace's `HEAD` afterwards, reported to the
    /// governor. Unchanged when the edit wrote what the files already held. `change` is the
    /// observed change of an `implementation.edit` that committed, and `None` for every other.
    Edited {
        revision: String,
        change: Option<Change>,
    },
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
            Self::Edited { revision, .. } => write!(f, "committed; HEAD is {revision}"),
            Self::TestsRun(run) => {
                write!(f, "{}\n{}", run.summary(), run.output)?;
                for regression in &run.regressions {
                    write!(
                        f,
                        "\nregression test {}: {}\n{}",
                        regression.path,
                        regression.run.summary(),
                        regression.run.output
                    )?;
                }
                Ok(())
            }
        }
    }
}

/// Why an action was not performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecuteError {
    /// No test was launched under the requested confinement.
    Confinement(ConfinementError),
    /// The slice never executes `action`.
    NotExecuted { action: String },
    /// `path` does not name a file inside the workspace.
    OutsideWorkspace { path: String },
    /// The workspace's git ignores `path`; the executor neither reads nor writes it.
    Ignored { path: String },
    /// `tests.write` names `path`, which none of the host's test-path patterns matches.
    NotATestPath { path: String },
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
    /// The host clock gave no instant for an observation, or one not later, in whole seconds,
    /// than the executor's previous observation's after waiting at most [`CLOCK_WAIT`] (a clock
    /// that stepped back), so none was delivered. An edit already committed stays committed.
    Clock(String),
}

impl fmt::Display for ExecuteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Confinement(error) => error.fmt(f),
            Self::NotExecuted { action } => write!(f, "the slice never executes `{action}`"),
            Self::OutsideWorkspace { path } => write!(f, "`{path}` is outside the workspace"),
            Self::Ignored { path } => write!(f, "`{path}` is ignored by the workspace's git"),
            Self::NotATestPath { path } => write!(
                f,
                "`{path}` is not a test path: `{TESTS_WRITE}` writes only paths the host's \
                 test-path patterns match; `{IMPLEMENTATION_EDIT}` changes the implementation"
            ),
            Self::InvalidArguments { action, problem } => {
                write!(f, "invalid arguments for `{action}`: {problem}")
            }
            Self::Workspace { problem } => write!(f, "the workspace failed: {problem}"),
            Self::Case(error) => error.fmt(f),
            Self::Governor(error) => write!(f, "the governor refused the observation: {error:?}"),
            Self::HostGit(refusal) => refusal.fmt(f),
            Self::Clock(error) => write!(f, "the host clock gave no usable instant: {error}"),
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
        TESTS_WRITE | IMPLEMENTATION_EDIT => serde_json::json!({
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
pub struct LocalExecutor<'g, S, T = NoEvaluationTime> {
    governor: &'g CanonGovernor<S, T>,
    case: CaseId,
    workspace: PathBuf,
    test: TestCommand,
    runner: Arc<dyn TestRunner>,
    test_paths: TestPaths,
    clock: &'g dyn Clock,
    tests_written: Cell<bool>,
    /// Every test path a `tests.write` of this executor committed with new contents, in the order
    /// first committed, each once.
    test_files: RefCell<Vec<String>>,
    /// The whole UTC second of the last observation this executor delivered.
    last_observed: Cell<Option<i64>>,
}

impl<'g, S: FallibleCaseStore, T: EvaluationTime> LocalExecutor<'g, S, T> {
    /// An executor for `case` in the git work tree at `workspace`, running `test` for `tests.run`,
    /// with `tests.write` held to [`TestPaths::DEFAULTS`] until
    /// [`LocalExecutor::with_test_paths`] gives the host's own, and its observations' instants
    /// read from [`SystemClock`] until [`LocalExecutor::with_clock`] gives another.
    pub fn new(
        governor: &'g CanonGovernor<S, T>,
        case: CaseId,
        workspace: impl Into<PathBuf>,
        test: TestCommand,
    ) -> Self {
        Self {
            governor,
            case,
            workspace: workspace.into(),
            test,
            runner: Arc::new(SubstrateRunner::new(None)),
            test_paths: TestPaths::default(),
            clock: &SystemClock,
            tests_written: Cell::new(false),
            test_files: RefCell::new(Vec::new()),
            last_observed: Cell::new(None),
        }
    }

    /// Read every observation's `observed_at` from the host's `clock`: trusted context, never a
    /// model's.
    #[must_use]
    pub fn with_clock(mut self, clock: &'g dyn Clock) -> Self {
        self.clock = clock;
        self
    }

    /// Hold `tests.write` to the host's `test_paths`: trusted context, never a model's.
    #[must_use]
    pub fn with_test_paths(mut self, test_paths: TestPaths) -> Self {
        self.test_paths = test_paths;
        self
    }

    /// Select an explicit test runner. The default constructor requires Substrate.
    pub fn with_runner(mut self, runner: Arc<dyn TestRunner>) -> Self {
        self.runner = runner;
        self
    }

    /// Performs `proposal`, or refuses it (see the module documentation).
    pub fn execute(
        &self,
        proposal: &ExecutorOutcomeProposedAction,
    ) -> Result<Report, ExecuteError> {
        let action = proposal.action.as_str();
        if ![INSPECT, TESTS_WRITE, IMPLEMENTATION_EDIT, TESTS_RUN].contains(&action) {
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
            TESTS_WRITE | IMPLEMENTATION_EDIT => self.edit(action, &root, &arguments),
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

    fn edit(
        &self,
        action: &str,
        root: &Path,
        arguments: &Arguments<'_>,
    ) -> Result<Report, ExecuteError> {
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
            let entry = Arguments::of(action, entry)?;
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
        if action == TESTS_WRITE
            && let Some((outside, _, _)) = files
                .iter()
                .find(|(relative, _, _)| !self.test_paths.matches(relative))
        {
            return Err(ExecuteError::NotATestPath {
                path: outside.to_string_lossy().into_owned(),
            });
        }
        let relatives: Vec<&OsStr> = files.iter().map(|(rel, _, _)| rel.as_os_str()).collect();
        refuse_ignored(root, &relatives)?;
        let mut undo = Undo::new(root)?;
        let committed = match write_and_commit(root, &files, &relatives, &message, &mut undo) {
            Ok(committed) => committed,
            Err(error) => {
                undo.restore();
                return Err(error);
            }
        };
        let case_revision =
            case::report_head(self.governor, &self.case, root).map_err(ExecuteError::Case)?;
        let revision = head(root)?;
        // Only a `tests.write` that committed wrote a test: one that wrote what the files already
        // held changed nothing, so no later run follows a regression test it wrote.
        if action == TESTS_WRITE && committed {
            self.tests_written.set(true);
            let mut test_files = self.test_files.borrow_mut();
            for (relative, file, _) in &files {
                let path = relative.to_string_lossy().into_owned();
                if undo.changed(file) && !test_files.contains(&path) {
                    test_files.push(path);
                }
            }
        }
        let change = if action == IMPLEMENTATION_EDIT && committed {
            Some(self.observe_change(&revision, case_revision)?)
        } else {
            None
        };
        Ok(Report::Edited { revision, change })
    }

    /// The host clock's instant for the next observation, UTC to the second, with its whole
    /// second: strictly later than the previous observation's. When the clock has not passed the
    /// previous observation's second, it is waited on until it has, at most [`CLOCK_WAIT`] in all;
    /// a clock that still has not (one that stepped back) is [`ExecuteError::Clock`], naming both
    /// instants. Nothing is delivered here: the caller records the second once it delivered.
    fn next_instant(&self) -> Result<(Timestamp, i64), ExecuteError> {
        let mut read = self.clock.read().map_err(ExecuteError::Clock)?;
        if let Some(previous) = self.last_observed.get() {
            let mut waited = Duration::ZERO;
            while read.timestamp() <= previous && waited < CLOCK_WAIT {
                let behind = u64::try_from(previous + 1 - read.timestamp()).unwrap_or(u64::MAX);
                let until_next = Duration::from_secs(behind).saturating_sub(Duration::from_nanos(
                    u64::from(read.timestamp_subsec_nanos()),
                ));
                let pause = until_next.max(MIN_CLOCK_PAUSE).min(CLOCK_WAIT - waited);
                self.clock.wait(pause);
                waited += pause;
                read = self.clock.read().map_err(ExecuteError::Clock)?;
            }
            if read.timestamp() <= previous {
                return Err(ExecuteError::Clock(format!(
                    "it reads {}, not later in whole seconds than the previous observation's {}, \
                     after waiting {} ms for that second to pass",
                    utc_second(read.timestamp()),
                    utc_second(previous),
                    waited.as_millis()
                )));
            }
        }
        Ok((Timestamp(utc_second(read.timestamp())), read.timestamp()))
    }

    /// Delivers the observation of an `implementation.edit` that committed `revision`, at the host
    /// clock's instant.
    fn observe_change(&self, revision: &str, case_revision: i64) -> Result<Change, ExecuteError> {
        let (observed_at, second) = self.next_instant()?;
        let observation = ObservationId(fresh_uuid("observation", &self.case.0));
        self.governor
            .observe(Observation::new(ObservationData {
                observation_id: observation.clone(),
                source: EDIT_SOURCE.to_owned(),
                subject: case::IMPLEMENTATION.to_owned(),
                observed_at: observed_at.clone(),
                payload: Value::Object(vec![
                    (
                        "action".to_owned(),
                        Value::Text(IMPLEMENTATION_EDIT.to_owned()),
                    ),
                    (
                        "implementation".to_owned(),
                        Value::Text(revision.to_owned()),
                    ),
                    (
                        "case_revision".to_owned(),
                        Value::Number(case_revision.to_string()),
                    ),
                ]),
            }))
            .map_err(ExecuteError::Governor)?;
        self.last_observed.set(Some(second));
        Ok(Change {
            case: self.case.clone(),
            observation,
            revision: revision.to_owned(),
            case_revision,
            observed_at,
        })
    }

    /// Runs the suite, then, on a bugfix whose host gave a regression-test command, that command
    /// once per committed test path (see [`TestRun::regressions`]).
    fn run_tests(&self, root: &Path) -> Result<Report, ExecuteError> {
        let mut suite = self.run_command(root, &self.test, TEST_RUN_SOURCE, None)?;
        let test_files = self.test_files.borrow().clone();
        if let Some(regression) = self.test.regression()
            && suite.implementation.is_some()
            && !test_files.is_empty()
            && self.bugfix()?
        {
            for path in test_files {
                let command = regression.for_path(&path, self.test.timeout);
                let run = self.run_command(root, &command, REGRESSION_RUN_SOURCE, Some(&path))?;
                suite.regressions.push(RegressionRun { path, run });
            }
        }
        Ok(Report::TestsRun(suite))
    }

    /// Whether the case is a bugfix, as the host gave its `change_kind` when it opened.
    fn bugfix(&self) -> Result<bool, ExecuteError> {
        let inputs = self
            .governor
            .inputs(&self.case)
            .map_err(ExecuteError::Governor)?;
        Ok(inputs.get("change_kind") == Some("bugfix"))
    }

    /// Runs `command` in the workspace and delivers its observation from `source`, naming the
    /// committed test `path` a regression-test run ran.
    fn run_command(
        &self,
        root: &Path,
        command: &TestCommand,
        source: &str,
        path: Option<&str>,
    ) -> Result<TestRun, ExecuteError> {
        let clean = git_output(root, &with_paths(&["status", "--porcelain"], &[]))?
            .stdout
            .is_empty();
        let implementation = if clean { Some(head(root)?) } else { None };
        let case_revision = self
            .governor
            .current_revision(&self.case)
            .map_err(ExecuteError::Governor)?;
        let finished = self
            .runner
            .run(command, root)
            .map_err(ExecuteError::Confinement)?;
        let exit_code = finished.exit_code;
        let observation = ObservationId(fresh_uuid("observation", &self.case.0));
        let mut payload = vec![
            (
                "command".to_owned(),
                Value::Array(command.describe().into_iter().map(Value::Text).collect()),
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
        payload.push(("confinement".to_owned(), observation_confinement(&finished)));
        if let Some(path) = path {
            payload.push(("test_path".to_owned(), Value::Text(path.to_owned())));
        }
        let (observed_at, second) = self.next_instant()?;
        self.governor
            .observe(Observation::new(ObservationData {
                observation_id: observation.clone(),
                source: source.to_owned(),
                subject: case::IMPLEMENTATION.to_owned(),
                observed_at: observed_at.clone(),
                payload: Value::Object(payload),
            }))
            .map_err(ExecuteError::Governor)?;
        self.last_observed.set(Some(second));
        Ok(TestRun {
            case: self.case.clone(),
            observation,
            exit_code,
            implementation,
            case_revision,
            timed_out: finished.timed_out,
            timeout: command.timeout,
            output: finished.output,
            applied: Box::new(finished.applied),
            observed_at,
            after_tests_write: self.tests_written.get(),
            regressions: Vec::new(),
        })
    }
}

/// Writes `files`, recording in `undo` what each path held, then stages and commits exactly those
/// paths; `true` when it committed, `false` when the files already held what was written. An error
/// leaves restoring to the caller.
fn write_and_commit(
    root: &Path,
    files: &[(PathBuf, PathBuf, &str)],
    relatives: &[&OsStr],
    message: &str,
    undo: &mut Undo,
) -> Result<bool, ExecuteError> {
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
    Ok(!unchanged)
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

    /// Whether `file` holds other bytes now than it held before it was first written (a file that
    /// did not exist did change).
    fn changed(&self, file: &Path) -> bool {
        self.files
            .iter()
            .find(|(held, _)| held == file)
            .is_some_and(|(_, previous)| previous.as_deref() != std::fs::read(file).ok().as_deref())
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

/// Explicit opt-out for trusted test commands; never selected as a fallback.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnconfinedRunner;

impl TestRunner for UnconfinedRunner {
    fn backend(&self) -> Backend {
        Backend::None
    }

    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        let finished = run_bounded(command, root).map_err(|error| ConfinementError {
            reason: ConfinementRefusal::CapabilityUnserved,
            detail: error.to_string(),
        })?;
        Ok(TestExecution {
            exit_code: finished.exit_code,
            timed_out: finished.timed_out,
            output: finished.output,
            applied: AppliedConfinement {
                backend: Backend::None,
                profile_digest: String::new(),
                capability_snapshot: String::new(),
                cgroup: String::new(),
                filesystem: "unconfined".to_owned(),
                network: "unconfined".to_owned(),
                profile: "none".to_owned(),
                writable_scopes: Vec::new(),
                read_only_roots: Vec::new(),
            },
            driver_record: None,
        })
    }
}

/// Stable spelling for user output and observation payloads.
pub fn backend_name(backend: Backend) -> &'static str {
    match backend {
        Backend::Substrate => "substrate",
        Backend::None => "none",
    }
}

fn observation_confinement(execution: &TestExecution) -> Value {
    let applied = &execution.applied;
    let record = serde_json::json!({
        "backend": backend_name(applied.backend),
        "profile_digest": applied.profile_digest,
        "capability_snapshot": applied.capability_snapshot,
        "cgroup": applied.cgroup,
        "filesystem": applied.filesystem,
        "network": applied.network,
        "profile": applied.profile,
        "writable_scopes": applied.writable_scopes,
        "read_only_roots": applied.read_only_roots.iter().map(|root|
            serde_json::json!({"host_path": root.host_path, "mount": root.mount})
        ).collect::<Vec<_>>(),
        "driver_record": execution.driver_record,
    });
    fn convert(value: serde_json::Value) -> Value {
        match value {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(value) => Value::Bool(value),
            serde_json::Value::Number(value) => Value::Number(value.to_string()),
            serde_json::Value::String(value) => Value::Text(value),
            serde_json::Value::Array(values) => {
                Value::Array(values.into_iter().map(convert).collect())
            }
            serde_json::Value::Object(values) => Value::Object(
                values
                    .into_iter()
                    .map(|(key, value)| (key, convert(value)))
                    .collect(),
            ),
        }
    }
    convert(record)
}

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

/// `seconds` since the Unix epoch as Canon writes an instant: UTC, `YYYY-MM-DDTHH:MM:SSZ`.
fn utc_second(seconds: i64) -> String {
    DateTime::<Utc>::from_timestamp(seconds, 0).map_or_else(
        || format!("{seconds} s after the Unix epoch"),
        |instant| instant.to_rfc3339_opts(SecondsFormat::Secs, true),
    )
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
