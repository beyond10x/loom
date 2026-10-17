//! Conformance checks for `story:regression-evidence-integrity` (wave 2026-10-14-w2, U5), the
//! slice half: a bugfix's `regression_test_result` comes only from the host's regression-test
//! command run on a path a committing `tests.write` wrote, once per path, `{path}` filled, through
//! the run's own test runner; a resume context kept without the field reads as no command.
//!
//! The fixture is the one `regression_command.rs` uses: a git repository whose suite is
//! `grep -qx fixed check.txt`, `check.txt` holding `broken`, and a committed
//! `tests/existing_test.txt` holding `fixed`. No model and no network: the executor is driven with
//! hand-built proposals.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::json as cjson;
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, ExecutorOutcomeProposedAction, ObservationData, ProposedActionArguments,
};
use b10x_loom_intake_slice::case::{self, CaseInputs, ChangeKind, Risk};
use b10x_loom_intake_slice::clock::Clock;
use b10x_loom_intake_slice::confinement::{ConfinementError, TestExecution, TestRunner};
use b10x_loom_intake_slice::executor::{
    LocalExecutor, REGRESSION_RUN_SOURCE, RegressionTestCommand, Report, TestCommand,
    UnconfinedRunner,
};
use b10x_loom_intake_slice::intent::resumed_request;
use b10x_loom_intake_slice::resume::{
    AcceptedRoute, AgentRevisionId, CatalogDigest, CommissionId, ResumeContextData,
    ResumeContextStore,
};
use b10x_loom_intake_slice::verifier::TestResultVerifier;
use chrono::{DateTime, FixedOffset};
use intake_model::confinement::Backend;
use intake_model::context::ContextPolicy;
use intake_model::primitives::{Decimal, Uuid};
use loom_governor::{CanonGovernor, MemoryCaseStore};
use serde_json::json;

const PICK: &str = "software-change@1";
const INTENT: &str = "Make the failing check pass.";
const PRODUCER: &str = "intake-slice/verifier";
const REGRESSION: &str = "grep -qx -f {path} check.txt";

/// A `tests.write` that changes an existing test and a later one that writes it back leave the
/// test as the run found it: the run wrote no regression test there, so a failing run of the
/// pre-existing test is no `regression_test_result`.
#[test]
fn security_a_test_written_and_then_written_back_is_no_regression_test() {
    let fixture = Fixture::new("written-back");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = open(&governor, &fixture, ChangeKind::Bugfix);
    let clock = Stepping::new();
    let runner = Recording::new();
    let executor =
        executor(&governor, &case, &fixture, Some(REGRESSION), &runner).with_clock(&clock);
    let verifier = TestResultVerifier::new(&governor, case.clone(), PRODUCER);

    write(&executor, &[("tests/existing_test.txt", "broken\n")]);
    write(&executor, &[("tests/existing_test.txt", "fixed\n")]);
    assert_eq!(
        fixture.git(&[
            "diff",
            "--stat",
            "HEAD~2",
            "HEAD",
            "--",
            "tests/existing_test.txt"
        ]),
        "",
        "the test is as the run found it"
    );
    let report = run(&executor);
    verifier.verify(&report).expect("the run is verified");

    assert_eq!(
        kinds(&evidence(&governor, &case)),
        ["test_result"],
        "a pre-existing test the run wrote back is no regression test it wrote"
    );
}

/// A path holding white space and shell metacharacters reaches the regression command as one
/// argument, never through a shell, and runs through the executor's own runner.
#[test]
fn security_a_path_with_shell_metacharacters_is_one_argument_and_runs_through_the_runner() {
    let fixture = Fixture::new("metachar");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = open(&governor, &fixture, ChangeKind::Bugfix);
    let clock = Stepping::new();
    let runner = Recording::new();
    let executor =
        executor(&governor, &case, &fixture, Some(REGRESSION), &runner).with_clock(&clock);
    let verifier = TestResultVerifier::new(&governor, case.clone(), PRODUCER);

    let path = "tests/a b;touch pwned&$(touch pwned2)`touch pwned3`|x>y.txt";
    write(&executor, &[(path, "fixed\n")]);
    let report = run(&executor);
    let Report::TestsRun(suite) = &report else {
        panic!("{report:?}")
    };
    assert_eq!(suite.regressions().len(), 1);
    assert_eq!(suite.regressions()[0].path(), path);
    verifier.verify(&report).expect("the run is verified");

    assert_eq!(
        runner.commands(),
        [
            vec!["grep", "-qx", "fixed", "check.txt"],
            vec!["grep", "-qx", "-f", path, "check.txt"],
        ],
        "the suite and the regression command both ran through the executor's runner"
    );
    assert_eq!(
        regression_commands(&governor),
        [vec!["grep", "-qx", "-f", path, "check.txt"]]
    );
    for planted in ["pwned", "pwned2", "pwned3", "y.txt", "tests/y.txt"] {
        assert!(
            !fixture.workspace().join(planted).exists(),
            "`{planted}` was created: the path reached a shell"
        );
    }
    assert_eq!(
        kinds(&evidence(&governor, &case)),
        ["test_result", "regression_test_result"]
    );
}

/// `{path}` in two words is filled in both, and a path that itself holds `{path}` is not filled
/// again.
#[test]
fn security_every_path_placeholder_is_filled_once_with_the_path() {
    let fixture = Fixture::new("twice");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = open(&governor, &fixture, ChangeKind::Bugfix);
    let clock = Stepping::new();
    let runner = Recording::new();
    let executor = executor(
        &governor,
        &case,
        &fixture,
        Some("grep -qx -f {path} --label={path} check.txt"),
        &runner,
    )
    .with_clock(&clock);

    write(&executor, &[("tests/{path}.txt", "fixed\n")]);
    run(&executor);
    assert_eq!(
        regression_commands(&governor),
        [vec![
            "grep",
            "-qx",
            "-f",
            "tests/{path}.txt",
            "--label=tests/{path}.txt",
            "check.txt"
        ]]
    );
}

/// A path outside the repository, or outside the host's test paths, is never written and so
/// never reaches the regression command.
#[test]
fn security_a_path_outside_the_workspace_never_reaches_the_regression_command() {
    let fixture = Fixture::new("outside");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = open(&governor, &fixture, ChangeKind::Bugfix);
    let clock = Stepping::new();
    let runner = Recording::new();
    let executor =
        executor(&governor, &case, &fixture, Some(REGRESSION), &runner).with_clock(&clock);

    let outside = fixture.root.join("outside.txt");
    for path in [
        "../outside.txt",
        "tests/../../outside.txt",
        outside.to_str().expect("UTF-8"),
        "check.txt",
    ] {
        let files = json!({"files": [{"path": path, "contents": "fixed\n"}]});
        assert!(
            executor.execute(&proposal("tests.write", &files)).is_err(),
            "`{path}` is written"
        );
    }
    run(&executor);
    assert!(regression_commands(&governor).is_empty());
    assert!(!outside.exists());
}

/// On a case that is not a bugfix the regression command is never run, however a test was
/// written.
#[test]
fn security_a_feature_case_never_runs_the_regression_command() {
    let fixture = Fixture::new("feature");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = open(&governor, &fixture, ChangeKind::Feature);
    let clock = Stepping::new();
    let runner = Recording::new();
    let executor =
        executor(&governor, &case, &fixture, Some(REGRESSION), &runner).with_clock(&clock);
    let verifier = TestResultVerifier::new(&governor, case.clone(), PRODUCER);

    write(&executor, &[("tests/new.txt", "fixed\n")]);
    verifier.verify(&run(&executor)).expect("verified");
    assert_eq!(runner.commands().len(), 1, "only the suite ran");
    assert_eq!(kinds(&evidence(&governor, &case)), ["test_result"]);
}

/// A resumed run is a new executor: a test written before the suspension is not its own, so its
/// runs gain no regression result.
#[test]
fn security_a_resumed_executor_gains_no_regression_result_for_a_test_written_before() {
    let fixture = Fixture::new("resumed");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = open(&governor, &fixture, ChangeKind::Bugfix);
    let clock = Stepping::new();
    let runner = Recording::new();
    {
        let before =
            executor(&governor, &case, &fixture, Some(REGRESSION), &runner).with_clock(&clock);
        write(&before, &[("tests/regression.txt", "fixed\n")]);
    }
    let resumed =
        executor(&governor, &case, &fixture, Some(REGRESSION), &runner).with_clock(&clock);
    let verifier = TestResultVerifier::new(&governor, case.clone(), PRODUCER);
    verifier.verify(&run(&resumed)).expect("verified");
    assert!(regression_commands(&governor).is_empty());
    assert_eq!(kinds(&evidence(&governor, &case)), ["test_result"]);
}

/// A context kept by 0.21.0, which names no `regression_test_command`, reads as no command, and
/// the request resumed from it runs no regression test.
#[test]
fn security_a_context_kept_without_the_field_resumes_with_no_regression_command() {
    let fixture = Fixture::new("legacy-context");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = open(&governor, &fixture, ChangeKind::Bugfix);
    let dir = fixture.root.join("state");
    let store = ResumeContextStore::open(&dir).expect("the store opens");
    let context = context(&case, Some(REGRESSION.to_owned()));
    store.keep(&context).expect("kept");

    // Rewrite the kept file as 0.21.0 wrote it: the same members, without the new one.
    let file = std::fs::read_dir(&dir)
        .expect("read the state directory")
        .map(|entry| entry.expect("an entry").path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("resume-"))
        })
        .expect("the kept context file");
    let mut kept: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("read")).expect("JSON");
    let members = kept.as_object_mut().expect("an object");
    assert!(members.remove("regression_test_command").is_some());
    std::fs::write(&file, serde_json::to_string(&kept).expect("JSON")).expect("rewrite");

    let read = store
        .get(&context.commission_id)
        .expect("a 0.21.0 context reads")
        .expect("it is kept");
    assert_eq!(read.regression_test_command, None);
    let request = resumed_request(&read, &governor, None).expect("it resumes");
    assert!(request.test.regression().is_none());

    // A kept command that does not name `{path}` does not resume as one that runs the suite.
    let mut no_path = context.clone();
    no_path.regression_test_command = Some("grep -qx fixed check.txt".to_owned());
    assert!(resumed_request(&no_path, &governor, None).is_err());
}

fn context(case: &CaseId, regression: Option<String>) -> ResumeContextData {
    ResumeContextData {
        commission_id: CommissionId(Uuid("00000000-0000-4000-8000-0000000b1105".into())),
        case_id: b10x_loom_intake_slice::resume::CaseId(case.0.clone()),
        agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000002".into())),
        intent: INTENT.to_owned(),
        route: AcceptedRoute {
            protocol: PICK.into(),
            intent_revision: "intent-sha256-00".into(),
            catalog: vec![CatalogDigest {
                name: PICK.into(),
                digest: "ab".repeat(32),
            }],
            context_policy: ContextPolicy::Legacy,
            threshold: Decimal("0.5".into()),
        },
        workspace: Some("/work/space".into()),
        test_command: vec![
            "grep".into(),
            "-qx".into(),
            "fixed".into(),
            "check.txt".into(),
        ],
        test_paths: vec!["tests/**".into()],
        regression_test_command: regression,
        max_steps: 6,
        confinement: Backend::None,
        cgroup_root: None,
        model: "lab-agent".into(),
        catalog: None,
        config: None,
    }
}

/// The unconfined runner, recording every command it is given.
struct Recording {
    commands: Mutex<Vec<Vec<String>>>,
}

impl Recording {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            commands: Mutex::new(Vec::new()),
        })
    }

    fn commands(&self) -> Vec<Vec<String>> {
        self.commands.lock().expect("lock").clone()
    }
}

impl TestRunner for Recording {
    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        let words = std::iter::once(command.program())
            .chain(command.args().iter().map(std::ffi::OsString::as_os_str))
            .map(|word| word.to_string_lossy().into_owned())
            .collect();
        self.commands.lock().expect("lock").push(words);
        UnconfinedRunner.run(command, root)
    }

    fn backend(&self) -> Backend {
        UnconfinedRunner.backend()
    }
}

fn open(governor: &CanonGovernor<MemoryCaseStore>, fixture: &Fixture, kind: ChangeKind) -> CaseId {
    let inputs = CaseInputs {
        risk: Risk::Standard,
        change_kind: kind,
        affects_security_boundary: false,
    };
    case::open(governor, PICK, INTENT, fixture.workspace(), &inputs).expect("the case opens")
}

fn executor<'g>(
    governor: &'g CanonGovernor<MemoryCaseStore>,
    case: &CaseId,
    fixture: &Fixture,
    regression: Option<&str>,
    runner: &Arc<Recording>,
) -> LocalExecutor<'g, MemoryCaseStore> {
    let mut test = TestCommand::new("grep", ["-qx", "fixed", "check.txt"]);
    if let Some(regression) = regression {
        test = test.with_regression(RegressionTestCommand::new(regression).expect("a command"));
    }
    LocalExecutor::new(governor, case.clone(), fixture.workspace(), test)
        .with_runner(Arc::clone(runner) as Arc<dyn TestRunner>)
}

fn write(executor: &LocalExecutor<'_, MemoryCaseStore>, files: &[(&str, &str)]) {
    let files: Vec<_> = files
        .iter()
        .map(|(path, contents)| json!({"path": path, "contents": contents}))
        .collect();
    executor
        .execute(&proposal("tests.write", &json!({ "files": files })))
        .expect("the tests are written");
}

fn run(executor: &LocalExecutor<'_, MemoryCaseStore>) -> Report {
    executor
        .execute(&proposal("tests.run", &json!({})))
        .expect("tests.run is performed")
}

fn evidence(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId) -> Vec<EvidenceData> {
    governor
        .evidence(case)
        .expect("the governor holds the case")
}

fn kinds(evidence: &[EvidenceData]) -> Vec<&str> {
    evidence.iter().map(|record| record.kind.as_str()).collect()
}

/// The command words of every regression-test run the governor observed, in order.
fn regression_commands(governor: &CanonGovernor<MemoryCaseStore>) -> Vec<Vec<String>> {
    governor
        .observations()
        .iter()
        .filter(|observation: &&ObservationData| observation.source == REGRESSION_RUN_SOURCE)
        .map(|observation| match observation.payload.member("command") {
            Some(cjson::Value::Array(words)) => words
                .iter()
                .map(|word| match word {
                    cjson::Value::Text(word) => word.clone(),
                    other => panic!("a command word is not text: {other:?}"),
                })
                .collect(),
            other => panic!("no command: {other:?}"),
        })
        .collect()
}

fn proposal(action: &str, arguments: &serde_json::Value) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: ProposedActionArguments(
            cjson::parse(&arguments.to_string()).expect("arguments are JSON"),
        ),
    }
}

/// A host clock that steps ten seconds per reading.
struct Stepping {
    readings: AtomicI64,
}

impl Stepping {
    const START: i64 = 1_791_000_000;
    const STEP: i64 = 10;

    fn new() -> Self {
        Self {
            readings: AtomicI64::new(0),
        }
    }
}

impl Clock for Stepping {
    fn read(&self) -> Result<DateTime<FixedOffset>, String> {
        let n = self.readings.fetch_add(1, Ordering::SeqCst);
        DateTime::from_timestamp(Self::START + n * Self::STEP, 0)
            .map(|instant| instant.fixed_offset())
            .ok_or_else(|| "no instant".to_owned())
    }
}

/// A git repository with a local identity, a failing check and an existing committed test file.
struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "security-w11-u5-{label}-{}-{nanos}",
            std::process::id()
        ));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(workspace.join("tests")).expect("create the workspace");
        let fixture = Self { root, workspace };
        fixture.git(&["init", "--quiet", "--initial-branch=main"]);
        fixture.git(&["config", "user.name", "Fixture Author"]);
        fixture.git(&["config", "user.email", "fixture@example.invalid"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(fixture.workspace.join("check.txt"), "broken\n").expect("write check.txt");
        std::fs::write(fixture.workspace.join("tests/existing_test.txt"), "fixed\n")
            .expect("write the existing test");
        fixture.git(&["add", "--all"]);
        fixture.git(&[
            "commit",
            "--quiet",
            "--message",
            "a failing check and its test",
        ]);
        fixture
    }

    fn workspace(&self) -> &Path {
        &self.workspace
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.workspace)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git prints UTF-8")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
