//! Adversary cases for story `slice-loop-cli` (wave 2026-10-04-w17, pass 1).
//!
//! Each case drives the library entry point `b10x-loom run` calls, [`intake_slice::run::run`],
//! with recorded fake models (never the network or a credential), on a fixture git repository
//! under `CARGO_TARGET_TMPDIR` whose own git calls run with no system or global configuration. The
//! binary-level cases run `CARGO_BIN_EXE_b10x-loom` with `HOME` and `CODEX_HOME` pointed at an
//! empty directory under `CARGO_TARGET_TMPDIR`, so no login is ever read.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_commission::model::responsibility::{
    CaseId, CompletionDetermination, CompletionDeterminationComplete, Frontier, GovernorError,
    frontier_state,
};
use b10x_commission::ports::governor::Governor;
use governor::{CanonGovernor, MemoryCaseStore};
use intake_slice::executor::TestCommand;
use intake_slice::run::{SliceError, SliceRequest, SliceRun, StopReason, run};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason as TurnStop, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest,
};
use serde_json::{Value, json};

const INTENT: &str = "Make the failing check pass.";
const SOFTWARE_CHANGE: &str = "software-change@1";

fn pick(protocol: &str, confidence: f64) -> Value {
    json!({"protocol": protocol, "confidence": confidence, "reasons": ["the recorded reason"]})
}

fn select(action: &str) -> Value {
    json!({"action": action})
}

fn fix() -> Value {
    json!({"files": [{"path": "check.txt", "contents": "fixed\n"}], "message": "fix the check"})
}

/// What one drive returned.
struct Drive {
    result: Result<SliceRun, SliceError>,
    output: String,
    frontiers: usize,
    /// The agent model's requests, in order, as their `Debug` text.
    agent_requests: Vec<String>,
}

/// The governor as the loop reads it: frontiers passed on and counted; the completion passed on,
/// or, when `complete` is set, reported `Complete` once the case has moved past the revision of the
/// first frontier read, so after a step.
///
/// Before `story:runtime-merge` it reported `Complete` on every call, which the slice's own loop
/// read only after a step. Commission's runtime reads the completion before every step, so the
/// fixture now reports it complete where the case is: after the edit moved it.
struct Frontiers<'g> {
    governor: &'g CanonGovernor<MemoryCaseStore>,
    complete: bool,
    issued: AtomicUsize,
    first: Mutex<Option<i64>>,
}

impl Governor for Frontiers<'_> {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.governor.current_revision(case)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        self.issued.fetch_add(1, Ordering::SeqCst);
        let frontier = self.governor.frontier(case)?;
        self.first
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_or_insert(frontier.data().case_revision);
        Ok(frontier)
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        let first = *self
            .first
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let moved = first.is_some_and(|first| {
            self.governor
                .current_revision(case)
                .is_ok_and(|now| now != first)
        });
        if self.complete && moved {
            return Ok(CompletionDetermination::Complete(
                CompletionDeterminationComplete {
                    outcome: "recorded-complete".to_owned(),
                },
            ));
        }
        self.governor.completion(case)
    }
}

fn drive_at(
    workspace: &Path,
    classifier: Value,
    agent: Vec<Value>,
    max_steps: usize,
    complete: bool,
) -> Drive {
    let classifier = Recorded::new("recorded-classifier", vec![classifier]);
    let agent = Recorded::new("recorded-agent", agent);
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let frontiers = Frontiers {
        governor: &governor,
        complete,
        issued: AtomicUsize::new(0),
        first: Mutex::new(None),
    };
    let request = SliceRequest {
        intent: INTENT.to_owned(),
        workspace: workspace.to_path_buf(),
        test: TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
        max_steps,
        threshold: 0.5,
    };
    let mut out = Vec::new();
    let result = run(
        &request,
        &governor,
        &frontiers,
        &classifier,
        &agent,
        &mut out,
    );
    Drive {
        result,
        output: String::from_utf8(out).expect("the output is UTF-8"),
        frontiers: frontiers.issued.load(Ordering::SeqCst),
        agent_requests: agent.requests(),
    }
}

fn drive(fixture: &Fixture, classifier: Value, agent: Vec<Value>, max_steps: usize) -> Drive {
    drive_at(fixture.workspace(), classifier, agent, max_steps, false)
}

fn ok(drive: &Drive) -> &SliceRun {
    match &drive.result {
        Ok(run) => run,
        Err(error) => panic!(
            "the run ends with a stop reason, not an error: {error}\n{}",
            drive.output
        ),
    }
}

fn last_line(output: &str) -> &str {
    output
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
}

fn count_starting(output: &str, prefix: &str) -> usize {
    output
        .lines()
        .filter(|line| line.trim_start().starts_with(prefix))
        .count()
}

fn step_actions(output: &str) -> Vec<String> {
    output
        .lines()
        .filter(|line| line.starts_with("step "))
        .map(|line| {
            line.split_whitespace()
                .nth(2)
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

fn control_lines(output: &str) -> Vec<String> {
    output
        .lines()
        .filter(|line| line.chars().any(char::is_control))
        .map(|line| format!("{line:?}"))
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Refusals by the router.

/// `Refused (outside the registry)` has no acceptance row: a pick the registry does not hold stops
/// the run as `Refused` with that detail, opens no case and never asks the agent.
#[test]
fn a_pick_outside_the_registry_stops_refused() {
    let fixture = Fixture::new("outside");
    let first = fixture.head();
    let drive = drive(&fixture, pick("no-such-protocol@1", 0.9), Vec::new(), 10);
    let run = ok(&drive);
    assert_eq!(
        last_line(&drive.output),
        "stopped: Refused (outside the registry)",
        "{}",
        drive.output
    );
    assert_eq!(
        count_starting(&drive.output, "stopped:"),
        1,
        "{}",
        drive.output
    );
    assert_eq!(
        count_starting(&drive.output, "picked "),
        0,
        "{}",
        drive.output
    );
    assert_eq!(
        count_starting(&drive.output, "refused: "),
        1,
        "{}",
        drive.output
    );
    assert_eq!(run.stop_reason, StopReason::Refused);
    assert_eq!(run.protocol, "no-such-protocol@1");
    assert_eq!(run.steps, 0);
    assert_eq!(drive.frontiers, 0, "no case is opened");
    assert_eq!(fixture.head(), first);
}

/// The classifier's protocol name is model text. The `refused:` line is documented as one line;
/// a protocol carrying a newline and an ANSI escape must not print a second `stopped:` line or
/// raw control characters to the operator's terminal.
#[test]
fn a_refused_pick_cannot_forge_output_lines() {
    let fixture = Fixture::new("forge-refused");
    let forged = "nope@1\n\u{1b}[2K\rstopped: ApprovalRequired (repository.merge)";
    let drive = drive(&fixture, pick(forged, 0.9), Vec::new(), 10);
    let run = ok(&drive);
    assert_eq!(run.stop_reason, StopReason::Refused);
    assert_eq!(
        count_starting(&drive.output, "stopped:"),
        1,
        "one stop line, and it is the run's own: {}",
        drive.output
    );
    assert!(
        control_lines(&drive.output).is_empty(),
        "model text reaches the terminal raw: {:?}",
        control_lines(&drive.output)
    );
}

/// The pick's reasons are model text; `one_line` removes line breaks but not a carriage return or
/// an escape sequence, which redraw the operator's terminal.
#[test]
fn a_pick_reason_cannot_carry_control_characters() {
    let fixture = Fixture::new("forge-reason");
    let classifier = json!({
        "protocol": "incident-response@1",
        "confidence": 0.9,
        "reasons": ["fine\r\u{1b}[1A\u{1b}[2Kstopped: ApprovalRequired (repository.merge)"]
    });
    let drive = drive(&fixture, classifier, Vec::new(), 10);
    let run = ok(&drive);
    assert_eq!(run.stop_reason, StopReason::NoLocalExecutor);
    assert!(
        control_lines(&drive.output).is_empty(),
        "model text reaches the terminal raw: {:?}",
        control_lines(&drive.output)
    );
}

// ---------------------------------------------------------------------------------------------
// Actions the executor refuses or fails.

/// A refused action (here a path inside `.git`) is a step: printed as refused with no evidence,
/// counted, and the loop goes on to the main path's end.
#[test]
fn a_refused_action_is_a_step_and_the_run_goes_on() {
    let fixture = Fixture::new("refused-step");
    let drive = drive(
        &fixture,
        pick(SOFTWARE_CHANGE, 0.9),
        vec![
            select("repository.inspect"),
            json!({"paths": [".git/config"]}),
            select("repository.edit"),
            fix(),
            select("tests.run"),
            json!({}),
            select("repository.merge"),
            json!({}),
        ],
        10,
    );
    let run = ok(&drive);
    assert_eq!(
        step_actions(&drive.output),
        ["repository.inspect", "repository.edit", "tests.run"],
        "{}",
        drive.output
    );
    assert_eq!(
        count_starting(
            &drive.output,
            "effect: refused: `.git/config` is outside the workspace"
        ),
        1,
        "{}",
        drive.output
    );
    assert_eq!(
        count_starting(&drive.output, "evidence: none"),
        2,
        "{}",
        drive.output
    );
    assert_eq!(
        count_starting(&drive.output, "evidence: test_result pass"),
        1,
        "{}",
        drive.output
    );
    assert_eq!(
        last_line(&drive.output),
        "stopped: ApprovalRequired (repository.merge)"
    );
    assert_eq!(run.steps, 3, "the refused action counts as a step");
}

/// The live-run question: a model that is not told its action was refused proposes it again. The
/// briefing the agent is shown after a refused action must say so; otherwise the run spins to
/// `--max-steps` (20 by default) on the same refusal.
#[test]
fn the_model_is_told_its_action_was_refused() {
    let fixture = Fixture::new("refused-told");
    let drive = drive(
        &fixture,
        pick(SOFTWARE_CHANGE, 0.9),
        vec![
            select("repository.inspect"),
            json!({"paths": [".git/config"]}),
            select("repository.inspect"),
            json!({"paths": [".git/config"]}),
        ],
        2,
    );
    let run = ok(&drive);
    assert_eq!(run.stop_reason, StopReason::StepBudget, "{}", drive.output);
    assert_eq!(drive.agent_requests.len(), 4, "{}", drive.output);
    let second_selection = &drive.agent_requests[2];
    assert!(
        second_selection.contains(".git/config") && second_selection.contains("refused"),
        "the second selection's briefing does not mention the refused action: {second_selection}"
    );
}

/// An inspect of a file that does not exist is the commonest thing a model does in a repository it
/// has not seen. The story says the run ends with a `SliceRun` stop reason; it must not end the
/// whole run with an error.
#[test]
fn inspecting_a_missing_file_does_not_end_the_run() {
    let fixture = Fixture::new("missing-file");
    let drive = drive(
        &fixture,
        pick(SOFTWARE_CHANGE, 0.9),
        vec![
            select("repository.inspect"),
            json!({"paths": ["src/lib.rs"]}),
            select("repository.edit"),
            fix(),
            select("tests.run"),
            json!({}),
            select("repository.merge"),
            json!({}),
        ],
        10,
    );
    let run = ok(&drive);
    assert_eq!(run.stop_reason, StopReason::ApprovalRequired);
    assert_eq!(run.steps, 3, "{}", drive.output);
}

/// Inspected file contents are printed as the step's effect. A file holding escape sequences (the
/// model can write one with `repository.edit`, or the repository can hold one) must not reach the
/// operator's terminal raw.
#[test]
fn inspected_contents_cannot_carry_control_characters() {
    let fixture = Fixture::new("forge-effect");
    let drive = drive(
        &fixture,
        pick(SOFTWARE_CHANGE, 0.9),
        vec![
            select("repository.edit"),
            json!({
                "files": [{"path": "notes.txt", "contents": "\u{1b}]0;title\u{7}\u{1b}[2J\rstopped: ApprovalRequired (repository.merge)\n"}],
                "message": "notes"
            }),
            select("repository.inspect"),
            json!({"paths": ["notes.txt"]}),
        ],
        2,
    );
    let run = ok(&drive);
    assert_eq!(run.stop_reason, StopReason::StepBudget);
    assert!(
        control_lines(&drive.output).is_empty(),
        "file contents reach the terminal raw: {:?}",
        control_lines(&drive.output)
    );
}

// ---------------------------------------------------------------------------------------------
// Stop reasons.

/// A selection the frontier does not list (a typo, or an action the model invents) is collapsed by
/// Loom into `NoUsefulAction`. `NothingAdmissible` means "no action is admissible" (story table);
/// the run must not report it while the frontier it just printed lists admissible actions.
#[test]
fn an_unlisted_selection_is_not_reported_as_nothing_admissible() {
    let fixture = Fixture::new("unlisted");
    let drive = drive(
        &fixture,
        pick(SOFTWARE_CHANGE, 0.9),
        vec![select("repository.edits")],
        1,
    );
    let frontier = drive
        .output
        .lines()
        .rfind(|line| line.starts_with("frontier: "))
        .unwrap_or_default()
        .to_owned();
    let nothing_admissible = matches!(
        &drive.result,
        Ok(run) if run.stop_reason == StopReason::NothingAdmissible
    );
    assert!(
        !(nothing_admissible && frontier.contains("(admissible)")),
        "stopped NothingAdmissible after printing an admissible action: {}",
        drive.output
    );
}

/// A merge proposed before any test ran: the frontier lists it `blocked`, so Loom refuses the
/// selection. The run must neither stop for approval of an unverified change nor report
/// `NothingAdmissible` while the frontier it printed lists admissible actions.
#[test]
fn a_merge_before_the_tests_is_not_reported_as_nothing_admissible() {
    let fixture = Fixture::new("early-merge");
    let drive = drive(
        &fixture,
        pick(SOFTWARE_CHANGE, 0.9),
        vec![
            select("repository.merge"),
            json!({}),
            select("repository.edit"),
            fix(),
            select("tests.run"),
            json!({}),
            select("repository.merge"),
            json!({}),
        ],
        10,
    );
    let run = ok(&drive);
    assert!(
        run.steps > 0 || run.stop_reason != StopReason::ApprovalRequired,
        "stopped for approval of a merge with no step performed and admissible actions listed: {}",
        drive.output
    );
    assert_ne!(
        run.stop_reason,
        StopReason::NothingAdmissible,
        "the frontier it printed lists admissible actions: {}",
        drive.output
    );
}

/// A budget of 0 stops before any frontier is read or the agent asked.
#[test]
fn a_budget_of_zero_stops_before_the_first_frontier() {
    let fixture = Fixture::new("zero");
    let drive = drive(&fixture, pick(SOFTWARE_CHANGE, 0.9), Vec::new(), 0);
    let run = ok(&drive);
    assert_eq!(run.stop_reason, StopReason::StepBudget);
    assert_eq!(run.steps, 0);
    assert_eq!(drive.frontiers, 0);
    assert_eq!(last_line(&drive.output), "stopped: StepBudget");
}

/// The governor reporting the case complete after a step is an error naming the outcome, and the
/// run prints no stop line.
#[test]
fn a_complete_case_is_an_error_not_a_stop() {
    let fixture = Fixture::new("complete");
    let drive = drive_at(
        fixture.workspace(),
        pick(SOFTWARE_CHANGE, 0.9),
        vec![select("repository.edit"), fix()],
        10,
        true,
    );
    match &drive.result {
        Err(SliceError::Complete(outcome)) => assert_eq!(outcome, "recorded-complete"),
        other => panic!("expected Complete, got {other:?}\n{}", drive.output),
    }
    assert_eq!(
        count_starting(&drive.output, "stopped:"),
        0,
        "{}",
        drive.output
    );
    assert_eq!(step_actions(&drive.output), ["repository.edit"]);
}

/// A workspace that is not a git root is an error, after the one classification, and no step runs.
#[test]
fn a_workspace_that_is_not_a_git_root_is_an_error() {
    let fixture = Fixture::new("not-root");
    let sub = fixture.workspace().join("sub");
    std::fs::create_dir_all(&sub).expect("create sub");
    let drive = drive_at(&sub, pick(SOFTWARE_CHANGE, 0.9), Vec::new(), 10, false);
    assert!(
        matches!(drive.result, Err(SliceError::Case(_))),
        "{:?}\n{}",
        drive.result,
        drive.output
    );
    assert_eq!(drive.frontiers, 0);
}

// ---------------------------------------------------------------------------------------------
// The binary.

fn bin(label: &str, args: &[&str]) -> std::process::Output {
    let home = scratch(label).join("home");
    std::fs::create_dir_all(&home).expect("create home");
    let workspace = Fixture::new(label);
    let mut command = Command::new(env!("CARGO_BIN_EXE_b10x-loom"));
    command
        .arg("run")
        .arg("--workspace")
        .arg(workspace.workspace())
        .args(args)
        .env("HOME", &home)
        .env("CODEX_HOME", &home)
        .env_remove("RUST_LOG");
    let output = command.output().expect("run b10x-loom");
    drop(workspace);
    let _ = std::fs::remove_dir_all(home.parent().expect("scratch"));
    output
}

/// `--test-cmd` that names no program fails before any model is built: exit status 1, a message
/// naming the flag, nothing on standard output.
#[test]
fn a_test_command_with_no_program_fails_before_any_model() {
    for test_cmd in ["", "   ", "\t"] {
        let output = bin("empty-cmd", &["--test-cmd", test_cmd, INTENT]);
        assert_eq!(output.status.code(), Some(1), "{test_cmd:?}");
        assert!(output.stdout.is_empty(), "{test_cmd:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("--test-cmd names no program"),
            "{test_cmd:?}: {stderr}"
        );
    }
}

/// A threshold outside 0..1, or not a number, fails with status 1 and a message about the
/// threshold, before any model call: with no Codex login present, a call would fail on the login
/// instead and name no threshold.
#[test]
fn an_invalid_threshold_fails_before_any_model_call() {
    for threshold in ["NaN", "2", "-0.5", "inf"] {
        let flag = format!("--threshold={threshold}");
        let output = bin("threshold", &[&flag, INTENT]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "--threshold {threshold}: {stderr}"
        );
        assert!(
            stderr.contains("threshold") && stderr.contains("not a number from 0 to 1"),
            "--threshold {threshold}: {stderr}"
        );
        assert!(output.stdout.is_empty(), "--threshold {threshold}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fixtures.

fn scratch(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "adv-slice-run-{label}-{}-{nanos}",
        std::process::id()
    ))
}

/// A recorded model that answers each request with the next recorded reply as a forced call of
/// the named tool, keeps each request's `Debug` text, and panics when asked more than recorded.
struct Recorded {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<Value>>,
    requests: Mutex<Vec<String>>,
}

impl Recorded {
    fn new(model: &str, script: Vec<Value>) -> Self {
        let id = |value: &str| Id::new(value).expect("a fixture id");
        Self {
            provenance: Provenance {
                protocol: Protocol::Responses,
                provider: id("recorded"),
                account: id("fixture"),
                endpoint: id("in-process"),
                model: id(model),
                binding_revision: id("recorded-1"),
            },
            capabilities: Capabilities {
                tools: true,
                tool_choice: true,
                ..Capabilities::text(128_000, 8_192)
            },
            script: Mutex::new(script.into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().expect("requests").clone()
    }
}

impl Model for Recorded {
    fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        _sink: &'a mut dyn StreamSink,
        _cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        Box::pin(async move {
            let ToolChoice::Named(tool) = &request.tool_choice else {
                panic!("a forced call of one named tool only");
            };
            let asked = {
                let mut requests = self.requests.lock().expect("requests");
                requests.push(format!("{request:?}"));
                requests.len()
            };
            let Some(arguments) = self.script.lock().expect("script").pop_front() else {
                panic!(
                    "{} was asked more often than recorded",
                    self.provenance.model.as_str()
                );
            };
            Ok(TurnOutcome {
                stop_reason: TurnStop::ToolCalls,
                items: vec![Item::ToolCall(ToolCall {
                    call_id: CallId::new(format!("call-{asked}")).expect("a call id"),
                    name: tool.clone(),
                    arguments,
                })],
                observation: TurnObservation {
                    final_usage: true,
                    ..TurnObservation::new(self.provenance.clone())
                },
            })
        })
    }
}

/// A git repository with a local identity and one failing check, under `CARGO_TARGET_TMPDIR`.
struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let root = scratch(label);
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).expect("create the workspace");
        let fixture = Self { root, workspace };
        fixture.git(&["init", "--quiet", "--initial-branch=main"]);
        fixture.git(&["config", "user.name", "Fixture Author"]);
        fixture.git(&["config", "user.email", "fixture@example.invalid"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(fixture.workspace.join("check.txt"), "broken\n").expect("write check.txt");
        fixture.git(&["add", "--all"]);
        fixture.git(&["commit", "--quiet", "--message", "a failing check"]);
        fixture
    }

    fn workspace(&self) -> &Path {
        &self.workspace
    }

    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
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
