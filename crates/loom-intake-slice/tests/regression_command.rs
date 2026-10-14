//! A bugfix's `regression_test_result` is the result of the host's regression-test command run on
//! the test file the run's `tests.write` committed, never of the whole suite
//! (`story:regression-evidence-integrity`).
//!
//! The fixture workspace is a git repository under `CARGO_TARGET_TMPDIR` whose suite is
//! `grep -qx fixed check.txt` (no shell), which reads no test file, with `check.txt` holding
//! `broken`. The regression-test command is `grep -qx -f {path} check.txt`: it reads the written
//! test file as the line `check.txt` must hold, so it runs that file and nothing else.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::json as cjson;
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, ExecutorOutcomeProposedAction, FrontierData, ObservationData,
    ProposedActionArguments,
};
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_intake_slice::case::{self, CaseInputs, ChangeKind, Risk};
use b10x_loom_intake_slice::clock::Clock;
use b10x_loom_intake_slice::executor::{
    LocalExecutor, REGRESSION_RUN_SOURCE, RegressionTestCommand, Report, TestCommand,
    UnconfinedRunner,
};
use b10x_loom_intake_slice::verifier::TestResultVerifier;
use chrono::{DateTime, FixedOffset};
use loom_governor::{CanonGovernor, MemoryCaseStore};
use serde_json::json;

const PICK: &str = "software-change@1";
const INTENT: &str = "Make the failing check pass.";
const PRODUCER: &str = "intake-slice/verifier";
const REGRESSION: &str = "grep -qx -f {path} check.txt";

/// The acceptance case: a bugfix whose suite does not run the file its `tests.write` wrote, and
/// whose host gave no regression-test command, submits no `regression_test_result`, however red
/// the suite is; so an edit after it leaves `red_first.required` open.
#[test]
fn a_bugfix_without_a_regression_test_command_submits_no_regression_result() {
    let fixture = Fixture::new("no-command");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = open(&governor, &fixture);
    let clock = Stepping::new();
    let executor = executor(&governor, &case, &fixture, None).with_clock(&clock);
    let verifier = TestResultVerifier::new(&governor, case.clone(), PRODUCER);

    write(&executor, &[("tests/regression.txt", "fixed\n")]);
    let red = run(&executor);
    verifier.verify(&red).expect("the run is verified");
    let edit = executor
        .execute(&proposal(
            "implementation.edit",
            &json!({"files": [{"path": "check.txt", "contents": "fixed\n"}]}),
        ))
        .expect("the edit is performed");
    verifier.verify(&edit).expect("the edit is verified");

    assert_eq!(
        kinds(&evidence(&governor, &case)),
        ["test_result", "implementation_change"],
        "a failing suite that does not run the written test is no regression result"
    );
    let frontier = governor.frontier(&case).expect("a frontier").into_data();
    assert!(
        obligation_open(&frontier, "red_first.required"),
        "no regression test ran, so red_first.required stays open: {frontier:?}"
    );
}

/// With a regression-test command, only that command's result is the regression result: a red
/// suite beside a regression test that passes on the written file submits `test_result fail`
/// and `regression_test_result pass`, each citing its own observation.
#[test]
fn only_the_regression_test_commands_result_is_the_regression_result() {
    let fixture = Fixture::new("command-result");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = open(&governor, &fixture);
    let clock = Stepping::new();
    let executor = executor(&governor, &case, &fixture, Some(REGRESSION)).with_clock(&clock);
    let verifier = TestResultVerifier::new(&governor, case.clone(), PRODUCER);

    // The written test asks for what `check.txt` already holds, so it passes.
    write(&executor, &[("tests/regression.txt", "broken\n")]);
    let report = run(&executor);
    verifier.verify(&report).expect("the run is verified");

    let held = evidence(&governor, &case);
    assert_eq!(kinds(&held), ["test_result", "regression_test_result"]);
    assert_eq!(text(&held[0].facts, "result"), "fail", "the suite is red");
    assert_eq!(
        text(&held[1].facts, "result"),
        "pass",
        "the regression test on the written file passes"
    );
    assert_ne!(
        held[0].observation_ids, held[1].observation_ids,
        "the regression result cites its own run, not the suite's"
    );
}

/// The command runs once per test path the run's `tests.write` actions committed, in the order
/// first committed, with `{path}` filled; a path written with what it already held is not one.
/// Each run is its own observation, received by the governor, and its record's `observed_at` is
/// that observation's instant.
#[test]
fn the_regression_test_command_runs_once_per_committed_test_path_with_the_path_filled() {
    let fixture = Fixture::new("per-path");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = open(&governor, &fixture);
    let clock = Stepping::new();
    let executor = executor(&governor, &case, &fixture, Some(REGRESSION)).with_clock(&clock);
    let verifier = TestResultVerifier::new(&governor, case.clone(), PRODUCER);

    write(
        &executor,
        &[
            ("tests/first.txt", "fixed\n"),
            ("tests/existing_test.txt", "fixed\n"),
        ],
    );
    write(
        &executor,
        &[
            ("tests/second.txt", "fixed\n"),
            ("tests/first.txt", "fixed\n"),
        ],
    );
    let report = run(&executor);
    let Report::TestsRun(suite) = &report else {
        panic!("{report:?}")
    };
    let paths: Vec<&str> = suite
        .regressions()
        .iter()
        .map(|regression| regression.path())
        .collect();
    assert_eq!(paths, ["tests/first.txt", "tests/second.txt"]);
    verifier.verify(&report).expect("the run is verified");

    let held = evidence(&governor, &case);
    assert_eq!(
        kinds(&held),
        [
            "test_result",
            "regression_test_result",
            "regression_test_result"
        ]
    );
    let observations = governor.observations();
    let regression_runs: Vec<&ObservationData> = observations
        .iter()
        .filter(|observation| observation.source == REGRESSION_RUN_SOURCE)
        .collect();
    assert_eq!(regression_runs.len(), 2, "{observations:?}");
    for (observation, path) in regression_runs
        .iter()
        .zip(["tests/first.txt", "tests/second.txt"])
    {
        let command: Vec<&str> = match observation.payload.member("command") {
            Some(cjson::Value::Array(words)) => words
                .iter()
                .map(|word| match word {
                    cjson::Value::Text(word) => word.as_str(),
                    other => panic!("a command word is not text: {other:?}"),
                })
                .collect(),
            other => panic!("no command: {other:?}"),
        };
        assert_eq!(command, ["grep", "-qx", "-f", path, "check.txt"]);
    }
    for (record, observation) in held[1..].iter().zip(&regression_runs) {
        assert_eq!(
            record.observation_ids,
            std::slice::from_ref(&observation.observation_id)
        );
        assert_eq!(
            text(&record.facts, "observed_at"),
            observation.observed_at.0
        );
        assert_eq!(text(&record.facts, "result"), "fail");
    }
}

/// A bugfix that writes its regression test, runs it red through the command and then edits
/// discharges `red_first.required`.
#[test]
fn a_red_regression_test_command_before_the_edit_discharges_red_first() {
    let fixture = Fixture::new("red-first");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = open(&governor, &fixture);
    let clock = Stepping::new();
    let executor = executor(&governor, &case, &fixture, Some(REGRESSION)).with_clock(&clock);
    let verifier = TestResultVerifier::new(&governor, case.clone(), PRODUCER);

    write(&executor, &[("tests/regression.txt", "fixed\n")]);
    verifier.verify(&run(&executor)).expect("verified");
    let edit = executor
        .execute(&proposal(
            "implementation.edit",
            &json!({"files": [{"path": "check.txt", "contents": "fixed\n"}]}),
        ))
        .expect("the edit is performed");
    verifier.verify(&edit).expect("verified");
    verifier.verify(&run(&executor)).expect("verified");

    let frontier = governor.frontier(&case).expect("a frontier").into_data();
    assert!(
        !obligation_open(&frontier, "red_first.required"),
        "the regression test failed before the edit: {frontier:?}"
    );
}

/// A regression-test command must name the written file: one with no `{path}` word, or none at
/// all, is refused.
#[test]
fn a_regression_test_command_without_the_path_placeholder_is_refused() {
    assert!(RegressionTestCommand::new("cargo test").is_err());
    assert!(RegressionTestCommand::new("   ").is_err());
    let command = RegressionTestCommand::new("cargo  test --test {path}").expect("a command");
    assert_eq!(command.text(), "cargo  test --test {path}");
}

fn open(governor: &CanonGovernor<MemoryCaseStore>, fixture: &Fixture) -> CaseId {
    let inputs = CaseInputs {
        risk: Risk::Standard,
        change_kind: ChangeKind::Bugfix,
        affects_security_boundary: false,
    };
    case::open(governor, PICK, INTENT, fixture.workspace(), &inputs).expect("the case opens")
}

fn executor<'g>(
    governor: &'g CanonGovernor<MemoryCaseStore>,
    case: &CaseId,
    fixture: &Fixture,
    regression: Option<&str>,
) -> LocalExecutor<'g, MemoryCaseStore> {
    let mut test = TestCommand::new("grep", ["-qx", "fixed", "check.txt"]);
    if let Some(regression) = regression {
        test = test.with_regression(RegressionTestCommand::new(regression).expect("a command"));
    }
    LocalExecutor::new(governor, case.clone(), fixture.workspace(), test)
        .with_runner(Arc::new(UnconfinedRunner))
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

fn text<'a>(value: &'a cjson::Value, member: &str) -> &'a str {
    match value.member(member) {
        Some(cjson::Value::Text(text)) => text,
        other => panic!("`{member}` is not text: {other:?}"),
    }
}

fn obligation_open(frontier: &FrontierData, obligation: &str) -> bool {
    frontier
        .obligations
        .iter()
        .find(|entry| entry.obligation == obligation)
        .unwrap_or_else(|| panic!("the frontier lists `{obligation}`: {frontier:?}"))
        .open
}

fn proposal(action: &str, arguments: &serde_json::Value) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: ProposedActionArguments(
            cjson::parse(&arguments.to_string()).expect("arguments are JSON"),
        ),
    }
}

/// A host clock that steps ten seconds per reading, so every observation has a second of its own.
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
            "regression-command-{label}-{}-{nanos}",
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
