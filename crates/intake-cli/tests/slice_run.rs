//! Acceptance for story `slice-loop-cli`: the slice runs from an intent until it is blocked, and
//! says why it stopped (`intake.routing.SliceRun`, `intake.routing.StopReason`).
//!
//! Every case drives the library entry point the `b10x-intake run` binary calls,
//! [`intake_slice::run::run`], with two recorded fake models: the classifier, which answers one
//! forced `pick_protocol` call, and the agent, which answers Loom's selections and argument requests
//! in order. A model asked more often than recorded panics, so a case that should never reach the
//! agent proves it by giving it no replies. No case reaches a model, the network or a credential.
//!
//! The governor is a `CanonGovernor` over memory. The loop reads its frontier and completion through
//! [`Frontiers`], which passes them on unchanged and counts the frontiers issued, except in the
//! nothing-admissible case, where it records every action of the frontier as blocked.
//!
//! The fixture workspace is a git repository under `CARGO_TARGET_TMPDIR` with one failing test: the
//! test command is `grep -qx fixed check.txt` (no shell; `grep` is on every Linux runner) and
//! `check.txt` holds `broken`. The fixture's own git calls run with no system or global
//! configuration and give the repository a local identity, which the executor's commits use.
//!
//! The output lines this pins:
//!
//! - `reference: <kind> <value>`, one per extracted reference, in order, before anything else;
//! - `picked <name>@<major>` at the start of the pick's line, only for a pick the router accepted;
//! - `step <n>: <action> <arguments>`, one per performed action, then indented `effect:` and
//!   `evidence:` lines, the latter `evidence: test_result <result>` for a verified test run;
//! - `stopped: <StopReason>`, with its detail in parentheses where it has one, as the last line.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_commission::model::responsibility::{
    ActionStatus, CaseId, CompletionDetermination, Frontier, GovernorError, frontier_state,
};
use b10x_commission::ports::governor::Governor;
use governor::{CanonGovernor, MemoryCaseStore};
use intake_references::references;
use intake_slice::executor::TestCommand;
use intake_slice::run::{SliceRequest, StopReason, run};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason as TurnStop, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest,
};
use serde_json::{Value, json};

/// The intent names a tracker key and a GitHub issue in mixed case, so the issue's canonical value
/// is not a substring of the intent: printing the intent back is not printing its references.
const INTENT: &str = "Make the failing check pass (OPS-42), tracked in HTTPS://GitHub.com/Beyond10x/Intake/issues/7.";
const SOFTWARE_CHANGE: &str = "software-change@1";
const INCIDENT_RESPONSE: &str = "incident-response@1";
const THRESHOLD: f64 = 0.5;

/// One row of the story's acceptance table.
struct Row {
    name: &'static str,
    pick: Value,
    agent: Vec<Value>,
    max_steps: usize,
    nothing_admissible: bool,
}

#[test]
fn the_slice_stops_for_each_reason() {
    // Main path: the edit fixes the failing test, the run passes, and the only useful action left
    // is the merge, which needs authority.
    let fixture = Fixture::new("main");
    let first = fixture.head();
    let (run, output, frontiers) = drive(
        &fixture,
        Row {
            name: "main path",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![
                json!({"action": "repository.edit"}),
                json!({
                    "files": [{"path": "check.txt", "contents": "fixed\n"}],
                    "message": "fix the check"
                }),
                json!({"action": "tests.run"}),
                json!({}),
                json!({"action": "repository.merge"}),
                json!({}),
            ],
            max_steps: 10,
            nothing_admissible: false,
        },
    );
    assert_picked(&output, SOFTWARE_CHANGE);
    let performed = steps(&output);
    assert_eq!(performed, ["repository.edit", "tests.run"], "{output}");
    let edit = line_starting(&output, "step 1: repository.edit");
    assert!(
        edit.contains("check.txt"),
        "a step prints its arguments: {output}"
    );
    assert_eq!(
        count_lines(&output, "evidence: test_result pass"),
        1,
        "the passing run is the one verified result: {output}"
    );
    assert_eq!(
        count_lines(&output, "evidence: test_result fail"),
        0,
        "{output}"
    );
    assert_eq!(
        count_starting(&output, "effect:"),
        2,
        "each step prints its effect: {output}"
    );
    assert_eq!(
        count_starting(&output, "evidence:"),
        2,
        "each step prints its evidence: {output}"
    );
    assert_stopped(&output, "stopped: ApprovalRequired (repository.merge)");
    assert_eq!(run.stop_reason, StopReason::ApprovalRequired);
    assert_eq!(run.protocol, SOFTWARE_CHANGE);
    assert_eq!(run.steps, 2);
    assert_eq!(frontiers, 3, "one frontier before each Loom run");
    assert_ne!(fixture.head(), first, "the edit was committed");
    assert_eq!(fixture.git(&["show", "HEAD:check.txt"]), "fixed\n");
    assert_eq!(fixture.git(&["rev-parse", "HEAD^"]).trim(), first);
    assert_eq!(
        fixture.git(&["branch", "--list"]).trim(),
        "* main",
        "nothing was merged or branched"
    );
    assert_eq!(fixture.status(), "", "the work tree is clean");

    // A step budget of 1: the edit is performed, then the budget is used up.
    let fixture = Fixture::new("budget");
    let first = fixture.head();
    let (run, output, frontiers) = drive(
        &fixture,
        Row {
            name: "a step budget of 1",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![
                json!({"action": "repository.edit"}),
                json!({
                    "files": [{"path": "check.txt", "contents": "fixed\n"}],
                    "message": "fix the check"
                }),
            ],
            max_steps: 1,
            nothing_admissible: false,
        },
    );
    assert_picked(&output, SOFTWARE_CHANGE);
    assert_eq!(steps(&output), ["repository.edit"], "{output}");
    assert_stopped(&output, "stopped: StepBudget");
    assert_eq!(run.stop_reason, StopReason::StepBudget);
    assert_eq!(run.steps, 1);
    assert_eq!(frontiers, 1, "no frontier after the budget is used up");
    assert_ne!(fixture.head(), first, "the one step was performed");

    // A pick of incident-response@1: the case opens and its frontier is issued, but the slice
    // executes only software.change/1, so the agent is never asked.
    let fixture = Fixture::new("incident");
    let first = fixture.head();
    let (run, output, frontiers) = drive(
        &fixture,
        Row {
            name: "a recorded pick of incident-response@1",
            pick: pick(INCIDENT_RESPONSE, 0.9),
            agent: Vec::new(),
            max_steps: 10,
            nothing_admissible: false,
        },
    );
    assert_picked(&output, INCIDENT_RESPONSE);
    assert!(steps(&output).is_empty(), "{output}");
    assert_stopped(&output, "stopped: NoLocalExecutor");
    assert_eq!(run.stop_reason, StopReason::NoLocalExecutor);
    assert_eq!(run.protocol, INCIDENT_RESPONSE);
    assert_eq!(run.steps, 0);
    assert_eq!(frontiers, 1, "it stops after its first frontier");
    assert_eq!(fixture.head(), first);

    // A confidence below the threshold: the router refuses, so no case is opened.
    let fixture = Fixture::new("unsure");
    let (run, output, frontiers) = drive(
        &fixture,
        Row {
            name: "a recorded confidence below the threshold",
            pick: pick(SOFTWARE_CHANGE, 0.2),
            agent: Vec::new(),
            max_steps: 10,
            nothing_admissible: false,
        },
    );
    assert_eq!(
        count_starting(&output, "picked "),
        0,
        "a refused pick is not printed as picked: {output}"
    );
    assert!(steps(&output).is_empty(), "{output}");
    assert_stopped(&output, "stopped: Refused (unsure)");
    assert_eq!(run.stop_reason, StopReason::Refused);
    assert_eq!(run.steps, 0);
    assert_eq!(frontiers, 0, "a refused pick opens no case");

    // A frontier with nothing admissible: Loom has nothing to propose, so the agent is never asked.
    let fixture = Fixture::new("nothing");
    let first = fixture.head();
    let (run, output, frontiers) = drive(
        &fixture,
        Row {
            name: "a recorded frontier with nothing admissible",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: Vec::new(),
            max_steps: 10,
            nothing_admissible: true,
        },
    );
    assert_picked(&output, SOFTWARE_CHANGE);
    assert!(steps(&output).is_empty(), "{output}");
    assert_stopped(&output, "stopped: NothingAdmissible");
    assert_eq!(run.stop_reason, StopReason::NothingAdmissible);
    assert_eq!(run.protocol, SOFTWARE_CHANGE);
    assert_eq!(run.steps, 0);
    assert_eq!(frontiers, 1);
    assert_eq!(fixture.head(), first);
}

/// The live run's question: after the passing test run the frontier lists the merge as needing
/// approval, and a model that keeps choosing what it may do (here a second test run at the same
/// revision) changes nothing. A step that leaves such a frontier unchanged stops the run for
/// approval, after that step is printed, instead of spinning to the step budget.
#[test]
fn the_run_stops_at_the_approval_gate_when_the_model_idles() {
    let fixture = Fixture::new("idle-at-gate");
    let first = fixture.head();
    let (run, output, _) = drive(
        &fixture,
        Row {
            name: "a model idling at the approval gate",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![
                json!({"action": "repository.inspect"}),
                json!({"paths": ["check.txt"]}),
                json!({"action": "repository.edit"}),
                json!({
                    "files": [{"path": "check.txt", "contents": "fixed\n"}],
                    "message": "fix the check"
                }),
                json!({"action": "tests.run"}),
                json!({}),
                json!({"action": "tests.run"}),
                json!({}),
            ],
            max_steps: 10,
            nothing_admissible: false,
        },
    );
    assert_picked(&output, SOFTWARE_CHANGE);
    assert_eq!(
        steps(&output),
        [
            "repository.inspect",
            "repository.edit",
            "tests.run",
            "tests.run"
        ],
        "{output}"
    );
    assert_eq!(
        count_lines(&output, "evidence: test_result pass"),
        2,
        "both test runs pass: {output}"
    );
    assert_stopped(&output, "stopped: ApprovalRequired (repository.merge)");
    assert_eq!(run.stop_reason, StopReason::ApprovalRequired);
    assert_eq!(run.steps, 4);
    assert_ne!(fixture.head(), first, "the edit was committed");
    assert_eq!(
        fixture.git(&["branch", "--list"]).trim(),
        "* main",
        "nothing was merged or branched"
    );
}

/// A step at the approval gate that changes the frontier does not stop the run: an edit after the
/// passing run makes that run's evidence stale, the merge is no longer the action needing approval,
/// and the run goes on until a step leaves the gate as it found it.
#[test]
fn a_step_that_moves_the_frontier_at_the_gate_goes_on() {
    let fixture = Fixture::new("moved-at-gate");
    let (run, output, _) = drive(
        &fixture,
        Row {
            name: "an edit at the approval gate",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![
                json!({"action": "repository.edit"}),
                json!({
                    "files": [{"path": "check.txt", "contents": "fixed\n"}],
                    "message": "fix the check"
                }),
                json!({"action": "tests.run"}),
                json!({}),
                json!({"action": "repository.edit"}),
                json!({
                    "files": [{"path": "notes.txt", "contents": "a note\n"}],
                    "message": "add a note"
                }),
                json!({"action": "tests.run"}),
                json!({}),
                json!({"action": "tests.run"}),
                json!({}),
            ],
            max_steps: 10,
            nothing_admissible: false,
        },
    );
    assert_eq!(
        steps(&output),
        [
            "repository.edit",
            "tests.run",
            "repository.edit",
            "tests.run",
            "tests.run"
        ],
        "{output}"
    );
    assert_stopped(&output, "stopped: ApprovalRequired (repository.merge)");
    assert_eq!(run.stop_reason, StopReason::ApprovalRequired);
    assert_eq!(run.steps, 5);
}

/// A selection refused at the approval gate is a step that changes nothing, so it stops the run for
/// approval like a performed one.
#[test]
fn a_refused_selection_at_the_gate_stops_for_approval() {
    let fixture = Fixture::new("refused-at-gate");
    let (run, output, _) = drive(
        &fixture,
        Row {
            name: "an unlisted selection at the approval gate",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![
                json!({"action": "repository.edit"}),
                json!({
                    "files": [{"path": "check.txt", "contents": "fixed\n"}],
                    "message": "fix the check"
                }),
                json!({"action": "tests.run"}),
                json!({}),
                json!({"action": "repository.publish"}),
            ],
            max_steps: 10,
            nothing_admissible: false,
        },
    );
    assert_eq!(
        steps(&output),
        ["repository.edit", "tests.run", "repository.publish"],
        "{output}"
    );
    assert_eq!(count_starting(&output, "effect: refused:"), 1, "{output}");
    assert_stopped(&output, "stopped: ApprovalRequired (repository.merge)");
    assert_eq!(run.stop_reason, StopReason::ApprovalRequired);
    assert_eq!(run.steps, 3);
}

/// `b10x-intake run --help` lists every flag the story names, the intent, and the default model.
#[test]
fn the_run_command_lists_its_flags() {
    let output = Command::new(env!("CARGO_BIN_EXE_b10x-intake"))
        .args(["run", "--help"])
        .output()
        .expect("run b10x-intake");
    assert!(
        output.status.success(),
        "`run --help` succeeds: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let help = String::from_utf8(output.stdout).expect("help is UTF-8");
    let tokens: Vec<&str> = help
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|token| !token.is_empty())
        .collect();
    for flag in [
        "--workspace",
        "--test-cmd",
        "--max-steps",
        "--model",
        "--classifier-model",
        "--threshold",
    ] {
        assert!(tokens.contains(&flag), "`run --help` lists {flag}: {help}");
    }
    assert!(
        help.contains("<INTENT>"),
        "the intent is positional: {help}"
    );
    assert!(
        help.matches("gpt-5.6-sol").count() >= 2,
        "both models default to gpt-5.6-sol: {help}"
    );
}

/// The exit status says how the run ended, and `run --help` states it: 0 when the run stops for
/// approval (the slice reached its human gate), 3 for every other stop reason, 1 for a failure and
/// 2 for a command line that is not valid. Which stop reason maps to 0 or 3 is pinned by the
/// binary's unit test; a stop reason needs a model, which no test here reaches. A failure is driven
/// here with no Codex login (`HOME` and `CODEX_HOME` an empty directory), so no login is read.
#[test]
fn the_exit_status_says_how_the_run_ended() {
    let output = Command::new(env!("CARGO_BIN_EXE_b10x-intake"))
        .args(["run", "--help"])
        .output()
        .expect("run b10x-intake");
    let help = String::from_utf8(output.stdout).expect("help is UTF-8");
    let status_line = |code: &str| -> String {
        help.lines()
            .map(str::trim)
            .find(|line| line.starts_with(&format!("{code} ")))
            .unwrap_or_else(|| panic!("`run --help` states exit status {code}: {help}"))
            .to_owned()
    };
    assert!(help.contains("Exit status:"), "{help}");
    assert!(status_line("0").contains("ApprovalRequired"), "{help}");
    let other = status_line("3");
    for reason in [
        "NothingAdmissible",
        "StepBudget",
        "NoLocalExecutor",
        "Refused",
    ] {
        assert!(other.contains(reason), "3 covers {reason}: {help}");
    }
    status_line("1");
    status_line("2");

    let fixture = Fixture::new("exit-status");
    let home = fixture.root.join("home");
    std::fs::create_dir_all(&home).expect("create an empty home");
    let failed = Command::new(env!("CARGO_BIN_EXE_b10x-intake"))
        .arg("run")
        .arg("--workspace")
        .arg(fixture.workspace())
        .arg(INTENT)
        .env("HOME", &home)
        .env("CODEX_HOME", &home)
        .output()
        .expect("run b10x-intake");
    let stderr = String::from_utf8_lossy(&failed.stderr);
    assert_eq!(
        failed.status.code(),
        Some(1),
        "no login is a failure: {stderr}"
    );
    assert!(
        stderr.contains("codex"),
        "the failure says to log in: {stderr}"
    );

    let usage = Command::new(env!("CARGO_BIN_EXE_b10x-intake"))
        .args(["run", INTENT])
        .env("HOME", &home)
        .env("CODEX_HOME", &home)
        .output()
        .expect("run b10x-intake");
    assert_eq!(
        usage.status.code(),
        Some(2),
        "a missing --workspace is a usage error: {}",
        String::from_utf8_lossy(&usage.stderr)
    );
}

/// Runs the slice for `row` on `fixture`; returns the run, its output and the frontiers issued.
fn drive(fixture: &Fixture, row: Row) -> (intake_slice::run::SliceRun, String, usize) {
    let classifier = Recorded::new("recorded-classifier", vec![row.pick]);
    let agent = Recorded::new("recorded-agent", row.agent);
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let frontiers = Frontiers {
        governor: &governor,
        nothing_admissible: row.nothing_admissible,
        issued: AtomicUsize::new(0),
    };
    let request = SliceRequest {
        intent: INTENT.to_owned(),
        workspace: fixture.workspace().to_path_buf(),
        test: TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
        max_steps: row.max_steps,
        threshold: THRESHOLD,
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
    let output = String::from_utf8(out).expect("the output is UTF-8");
    let slice =
        result.unwrap_or_else(|error| panic!("{}: the slice runs: {error}\n{output}", row.name));
    assert_eq!(
        classifier.asked(),
        1,
        "{}: one classification, no retry",
        row.name
    );
    assert_eq!(
        agent.remaining(),
        0,
        "{}: every recorded agent reply was asked for",
        row.name
    );
    assert_references(&output, row.name);
    (slice, output, frontiers.issued.load(Ordering::SeqCst))
}

/// The references the intent holds are printed first, in order, one line each.
fn assert_references(output: &str, row: &str) {
    let found = references(INTENT);
    assert_eq!(found.len(), 2, "{found:?}");
    let expected: Vec<String> = found
        .iter()
        .map(|reference| format!("reference: {:?} {}", reference.kind, reference.value))
        .collect();
    let printed: Vec<&str> = output.lines().take(expected.len()).collect();
    assert_eq!(
        printed, expected,
        "{row}: the references come first: {output}"
    );
}

fn assert_picked(output: &str, protocol: &str) {
    assert_eq!(
        count_starting(output, &format!("picked {protocol}")),
        1,
        "the pick is printed: {output}"
    );
}

fn assert_stopped(output: &str, expected: &str) {
    assert_eq!(
        output.lines().rev().find(|line| !line.trim().is_empty()),
        Some(expected),
        "the stop reason is the last line: {output}"
    );
    assert_eq!(count_starting(output, "stopped:"), 1, "{output}");
}

/// The action of every `step <n>: <action> ...` line, in order, numbered from 1.
fn steps(output: &str) -> Vec<String> {
    output
        .lines()
        .filter(|line| line.starts_with("step "))
        .enumerate()
        .map(|(n, line)| {
            let rest = line
                .strip_prefix(&format!("step {}: ", n + 1))
                .unwrap_or_else(|| panic!("steps are numbered from 1: {line}"));
            rest.split_whitespace()
                .next()
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

fn line_starting<'a>(output: &'a str, prefix: &str) -> &'a str {
    output
        .lines()
        .find(|line| line.starts_with(prefix))
        .unwrap_or_else(|| panic!("no line starts with `{prefix}`: {output}"))
}

fn count_starting(output: &str, prefix: &str) -> usize {
    output
        .lines()
        .filter(|line| line.trim_start().starts_with(prefix))
        .count()
}

fn count_lines(output: &str, expected: &str) -> usize {
    output
        .lines()
        .filter(|line| line.trim() == expected)
        .count()
}

fn pick(protocol: &str, confidence: f64) -> Value {
    json!({
        "protocol": protocol,
        "confidence": confidence,
        "reasons": ["the recorded reason"]
    })
}

/// The governor's frontier and completion as the loop reads them: passed on unchanged and counted,
/// or, for `nothing_admissible`, with every action recorded as blocked.
struct Frontiers<'g> {
    governor: &'g CanonGovernor<MemoryCaseStore>,
    nothing_admissible: bool,
    issued: AtomicUsize,
}

impl Governor for Frontiers<'_> {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.governor.current_revision(case)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        self.issued.fetch_add(1, Ordering::SeqCst);
        let frontier = self.governor.frontier(case)?;
        if !self.nothing_admissible {
            return Ok(frontier);
        }
        let mut data = frontier.into_data();
        assert!(!data.actions.is_empty(), "the protocol declares actions");
        for action in &mut data.actions {
            action.status = ActionStatus::Blocked;
            action.reasons = vec!["recorded: blocked".to_owned()];
        }
        Ok(Frontier::new(data))
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.governor.completion(case)
    }
}

/// A model that answers each request with the next recorded reply, as a forced call of the one tool
/// the request names, and panics when asked more often than recorded.
struct Recorded {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<Value>>,
    asked: AtomicUsize,
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
            asked: AtomicUsize::new(0),
        }
    }

    fn remaining(&self) -> usize {
        self.script.lock().expect("script").len()
    }

    fn asked(&self) -> usize {
        self.asked.load(Ordering::SeqCst)
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
            if let Err(error) = request.validate_for(&self.provenance, &self.capabilities) {
                panic!("the slice sent a request the model refuses: {error}");
            }
            let ToolChoice::Named(tool) = &request.tool_choice else {
                panic!(
                    "the recorded model answers only a forced call of one named tool: {:?}",
                    request.tool_choice
                );
            };
            let Some(arguments) = self.script.lock().expect("script").pop_front() else {
                panic!(
                    "{} was asked more often than recorded",
                    self.provenance.model.as_str()
                );
            };
            let asked = self.asked.fetch_add(1, Ordering::SeqCst) + 1;
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

/// A git repository with a local identity and one failing test, under `CARGO_TARGET_TMPDIR`.
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
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("slice-run-{label}-{}-{nanos}", std::process::id()));
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

    fn status(&self) -> String {
        self.git(&["status", "--porcelain", "--untracked-files=all"])
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
