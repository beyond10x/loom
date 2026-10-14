//! Acceptance for story `slice-loop-cli`: the slice runs from an intent until it is blocked, and
//! says why it stopped (`intake.routing.SliceRun`, `intake.routing.StopReason`).
//!
//! Every case drives the library entry point the `b10x-loom run` binary calls,
//! [`b10x_loom_intake_slice::run::run`], with two recorded fake models: the classifier, which answers one
//! forced `pick_protocol` call, and the agent, which answers Loom's selections and argument requests
//! in order. A model asked more often than recorded panics, so a case that should never reach the
//! agent proves it by giving it no replies. No case reaches a model, the network or a credential.
//!
//! The governor is a `CanonGovernor` over memory. The loop reads its frontier and completion through
//! [`Frontiers`], which passes them on unchanged and counts the frontiers issued, except in the
//! nothing-admissible case, where it records every action of the frontier as blocked.
//!
//! The case inputs are the ones `b10x-loom run`'s own command line gives ([`run_inputs`]): with no
//! input flag, the run defaults (`standard`, `feature`, off a security boundary), under which every
//! case below reaches the outcome it reached before the inputs existed.
//!
//! Since `story:runtime-merge` the loop is Commission's runtime (`run_until_blocked`, Atlas ADR
//! 0082): it reads the frontier before each Loom run and again to revalidate each proposal, so
//! the frontier counts below count both reads.
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

use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_cli::{CaseInputs, ChangeKind, Cli, Command as CliCommand};
use b10x_loom_commission::model::json::{self as commission_json, Value as CommissionJson};
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, CaseId, CompletionDetermination, EvidenceData, EvidenceId, Frontier,
    FrontierData, GovernorError, ObservationData, ObservationId, Truth, frontier_state,
};
use b10x_loom_commission::ports::evidence::submit_evidence;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_intake_references::references;
use b10x_loom_intake_slice::clock::{Clock, SystemClock};
use b10x_loom_intake_slice::executor::{EDIT_SOURCE, TEST_RUN_SOURCE, TestCommand};
use b10x_loom_intake_slice::run::{
    PRODUCER, RunOptions, SliceRequest, StopReason, run_with_clock, slice_governor,
};
use chrono::{DateTime, FixedOffset, SecondsFormat, TimeDelta, Utc};
use clap::Parser;
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason as TurnStop, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest,
};
use loom_governor::{CanonGovernor, MemoryCaseStore};
use loom_protocols::ProtocolCatalog;
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
fn confinement_refusal_stops_the_loop_without_test_evidence() {
    use b10x_loom_intake_slice::confinement::{
        Backend, ConfinementError, ConfinementRefusal, TestExecution, TestRunner,
    };
    struct Refused;
    impl TestRunner for Refused {
        fn backend(&self) -> Backend {
            Backend::Substrate
        }
        fn run(
            &self,
            _: &TestCommand,
            _: &std::path::Path,
        ) -> Result<TestExecution, ConfinementError> {
            Err(ConfinementError {
                reason: ConfinementRefusal::BackendMissing,
                detail: "fixture".into(),
            })
        }
    }
    let fixture = Fixture::new("confined-refusal");
    let (run, output, _, observations) = drive_with_runner(
        &fixture,
        Row {
            name: "confined-refusal",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![json!({"action": "tests.run"}), json!({})],
            max_steps: 10,
            nothing_admissible: false,
        },
        std::sync::Arc::new(Refused),
    );
    assert_eq!(run.stop_reason, StopReason::ConfinementUnavailable);
    assert_eq!(run.steps, 1);
    assert!(output.contains("confinement: substrate"));
    assert!(output.contains("BackendMissing"));
    assert!(output.contains("evidence: none"));
    assert!(!output.contains("test_result pass"));
    assert!(
        !observations
            .iter()
            .any(|o| o.source == b10x_loom_intake_slice::executor::TEST_RUN_SOURCE)
    );
}

#[test]
fn the_slice_stops_for_each_reason() {
    // Main path: the edit fixes the failing test, the run passes, and the only useful action left
    // is the merge, which needs authority.
    let fixture = Fixture::new("main");
    let first = fixture.head();
    let (run, output, frontiers, evidence) = drive_counted(
        &fixture,
        Row {
            name: "main path",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![
                json!({"action": "implementation.edit"}),
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
    assert_eq!(performed, ["implementation.edit", "tests.run"], "{output}");
    let edit = line_starting(&output, "step 1: implementation.edit");
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
    // Was 3 (one before each Loom run); Commission's runtime also reads one to revalidate each of
    // the three proposals (story:runtime-merge).
    assert_eq!(
        frontiers, 6,
        "one frontier before each Loom run, one per revalidation"
    );
    assert_ne!(fixture.head(), first, "the edit was committed");
    assert_eq!(fixture.git(&["show", "HEAD:check.txt"]), "fixed\n");
    assert_eq!(fixture.git(&["rev-parse", "HEAD^"]).trim(), first);
    assert_eq!(
        fixture.git(&["branch", "--list"]).trim(),
        "* main",
        "nothing was merged or branched"
    );
    assert_eq!(fixture.status(), "", "the work tree is clean");
    assert_eq!(
        kinds(&evidence),
        ["implementation_change", "test_result"],
        "the edit's change and the passing run's result, and no regression test result: {output}"
    );

    // A step budget of 1: the edit is performed, then the budget is used up.
    let fixture = Fixture::new("budget");
    let first = fixture.head();
    let (run, output, frontiers, evidence) = drive_counted(
        &fixture,
        Row {
            name: "a step budget of 1",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![
                json!({"action": "implementation.edit"}),
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
    assert_eq!(steps(&output), ["implementation.edit"], "{output}");
    assert_stopped(&output, "stopped: StepBudget");
    assert_eq!(run.stop_reason, StopReason::StepBudget);
    assert_eq!(run.steps, 1);
    // Was 1; the one proposal's revalidation reads the frontier again (story:runtime-merge).
    assert_eq!(
        frontiers, 2,
        "the step's frontier and its revalidation, none after the budget is used up"
    );
    assert_ne!(fixture.head(), first, "the one step was performed");
    assert_eq!(kinds(&evidence), ["implementation_change"], "{output}");

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
                json!({"action": "implementation.edit"}),
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
            "implementation.edit",
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
                json!({"action": "implementation.edit"}),
                json!({
                    "files": [{"path": "check.txt", "contents": "fixed\n"}],
                    "message": "fix the check"
                }),
                json!({"action": "tests.run"}),
                json!({}),
                json!({"action": "implementation.edit"}),
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
            "implementation.edit",
            "tests.run",
            "implementation.edit",
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
                json!({"action": "implementation.edit"}),
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
        ["implementation.edit", "tests.run", "repository.publish"],
        "{output}"
    );
    assert_eq!(count_starting(&output, "effect: refused:"), 1, "{output}");
    assert_stopped(&output, "stopped: ApprovalRequired (repository.merge)");
    assert_eq!(run.stop_reason, StopReason::ApprovalRequired);
    assert_eq!(run.steps, 3);
}

/// `story:runtime-merge`: the slice runs on Commission's runtime, not a loop of its own. The
/// governor receives what only that runtime delivers: one `executor` observation per Loom run and
/// one `effect` observation per performed action, after the action (here after the test run's own
/// observation, which the local effect adapter delivers while it performs `tests.run`).
#[test]
fn the_slice_runs_on_commissions_runtime() {
    let fixture = Fixture::new("runtime");
    let (run, output, _, observations) = drive_observed(
        &fixture,
        Row {
            name: "main path on the runtime",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![
                json!({"action": "implementation.edit"}),
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
    assert_stopped(&output, "stopped: ApprovalRequired (repository.merge)");
    assert_eq!(run.steps, 2);
    let sources: Vec<&str> = observations.iter().map(|o| o.source.as_str()).collect();
    assert_eq!(
        sources,
        [
            "executor",
            // The committed edit's own observation, which its implementation_change cites (story
            // `change-evidence-order`).
            EDIT_SOURCE,
            "effect",
            "executor",
            b10x_loom_intake_slice::executor::TEST_RUN_SOURCE,
            "effect",
            "executor",
        ],
        "{output}"
    );
    let edit = &observations[2];
    assert_eq!(
        edit.payload.member("outcome"),
        Some(&b10x_loom_commission::model::json::Value::Text(
            "Performed".to_owned()
        )),
        "{edit:?}"
    );
    assert_eq!(
        edit.payload.member("action"),
        Some(&b10x_loom_commission::model::json::Value::Text(
            "implementation.edit".to_owned()
        )),
        "{edit:?}"
    );
}

/// `b10x-loom run --help` lists every flag the story names, the intent, and the default model.
#[test]
fn the_run_command_lists_its_flags() {
    let output = Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
        .args(["run", "--help"])
        .output()
        .expect("run b10x-loom");
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
/// Explicit unconfined mode lets this login fixture reach that failure without host delegation.
#[test]
fn the_exit_status_says_how_the_run_ended() {
    let output = Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
        .args(["run", "--help"])
        .output()
        .expect("run b10x-loom");
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
    let failed = Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
        .arg("run")
        .args(["--confinement", "none"])
        .arg("--workspace")
        .arg(fixture.workspace())
        .arg(INTENT)
        .env("HOME", &home)
        .env("CODEX_HOME", &home)
        .output()
        .expect("run b10x-loom");
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

    let usage = Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
        .args(["run"])
        .env("HOME", &home)
        .env("CODEX_HOME", &home)
        .output()
        .expect("run b10x-loom");
    assert_eq!(
        usage.status.code(),
        Some(2),
        "a missing intent is a usage error: {}",
        String::from_utf8_lossy(&usage.stderr)
    );
}

/// `b10x-loom run --change-kind bugfix`: an `implementation.edit` before a test failed on the
/// current revision is not admissible (`software.change/1`, the edit's precondition
/// `regression.reproduced`), so the agent's first edit is refused and commits nothing. Once
/// `tests.run` failed on that revision the edit is admissible, and the run reaches the approval gate
/// as a feature does. The same first edit under no input flag is performed (`the_slice_stops_for_each_reason`).
#[test]
fn a_bugfix_edits_the_implementation_only_after_a_failing_test() {
    let inputs = run_inputs(&["--change-kind", "bugfix"]);
    assert_eq!(inputs.change_kind, ChangeKind::Bugfix);
    let edit = json!({
        "files": [{"path": "check.txt", "contents": "fixed\n"}],
        "message": "fix the check"
    });
    let fixture = Fixture::new("bugfix");
    let first = fixture.head();
    let (run, output, _, _, _) = drive_with_inputs(
        &fixture,
        Row {
            name: "a bugfix that edits before its test failed",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![
                json!({"action": "implementation.edit"}),
                json!({"action": "tests.run"}),
                json!({}),
                json!({"action": "implementation.edit"}),
                edit,
                json!({"action": "tests.run"}),
                json!({}),
                json!({"action": "repository.merge"}),
                json!({}),
            ],
            max_steps: 10,
            nothing_admissible: false,
        },
        std::sync::Arc::new(b10x_loom_intake_slice::executor::UnconfinedRunner),
        inputs,
    );
    assert_picked(&output, SOFTWARE_CHANGE);
    assert_eq!(
        steps(&output),
        [
            "implementation.edit",
            "tests.run",
            "implementation.edit",
            "tests.run"
        ],
        "{output}"
    );
    let refused: Vec<&str> = output
        .lines()
        .filter(|line| line.trim_start().starts_with("effect: refused:"))
        .collect();
    assert_eq!(
        refused.len(),
        1,
        "only the edit before the failing test is refused: {output}"
    );
    let lines: Vec<&str> = output.lines().collect();
    let first_step = lines
        .iter()
        .position(|line| line.starts_with("step 1: "))
        .expect("a first step");
    assert!(
        lines[first_step + 1..]
            .iter()
            .take_while(|line| !line.starts_with("step 2: "))
            .any(|line| line.trim_start().starts_with("effect: refused:")),
        "the refused step is the first edit: {output}"
    );
    assert_eq!(
        count_lines(&output, "evidence: test_result fail"),
        1,
        "{output}"
    );
    assert_eq!(
        count_lines(&output, "evidence: test_result pass"),
        1,
        "{output}"
    );
    assert_stopped(&output, "stopped: ApprovalRequired (repository.merge)");
    assert_eq!(run.stop_reason, StopReason::ApprovalRequired);
    assert_eq!(
        fixture
            .git(&["rev-list", "--count", &format!("{first}..HEAD")])
            .trim(),
        "1",
        "only the edit after the failing test committed: {output}"
    );
    assert_eq!(fixture.git(&["show", "HEAD:check.txt"]), "fixed\n");
}

/// The rule for an action the slice can never perform (`release.rollback`, which needs a capability
/// and which neither a run's effects nor a resume's hand-off performs): it follows the rule of an
/// action the effects do not perform, and is never in the frontier the run reads
/// (`run::never_performed`). The agent is never offered it, a proposal of it is refused as any
/// action outside the frontier is, and the run neither awaits approval for it nor stops at a gate
/// it would make: an inspection that leaves the frontier unchanged goes on.
#[test]
fn the_run_never_offers_or_awaits_release_rollback() {
    let fixture = Fixture::new("rollback");
    let (run, output, _, _, requests) = drive_with_inputs(
        &fixture,
        Row {
            name: "an agent that proposes a rollback",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![
                json!({"action": "release.rollback"}),
                json!({"action": "repository.inspect"}),
                json!({"paths": ["check.txt"]}),
                json!({"action": "implementation.edit"}),
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
        std::sync::Arc::new(b10x_loom_intake_slice::executor::UnconfinedRunner),
        run_inputs(&[]),
    );
    assert!(
        requests[0].contains("repository.inspect") && !requests[0].contains("release.rollback"),
        "the first selection offers the frontier without the rollback: {}",
        requests[0]
    );
    assert_eq!(
        steps(&output),
        [
            "release.rollback",
            "repository.inspect",
            "implementation.edit",
            "tests.run"
        ],
        "{output}"
    );
    assert_eq!(
        count_starting(&output, "effect: refused:"),
        1,
        "the proposed rollback is refused, never awaited: {output}"
    );
    assert!(
        !output.contains("ApprovalRequired (release.rollback)"),
        "{output}"
    );
    assert_stopped(&output, "stopped: ApprovalRequired (repository.merge)");
    assert_eq!(run.stop_reason, StopReason::ApprovalRequired);
    assert_eq!(run.steps, 4);
}

/// Runs the slice for `row` on `fixture`; returns the run, its output and the frontiers issued.
fn drive(fixture: &Fixture, row: Row) -> (b10x_loom_intake_slice::run::SliceRun, String, usize) {
    let (slice, output, frontiers, _) = drive_observed(fixture, row);
    (slice, output, frontiers)
}

/// [`drive`], also returning every evidence record the governor holds for the case, in order.
fn drive_counted(
    fixture: &Fixture,
    row: Row,
) -> (
    b10x_loom_intake_slice::run::SliceRun,
    String,
    usize,
    Vec<EvidenceData>,
) {
    let driven = drive_clocked(
        fixture,
        row,
        std::sync::Arc::new(b10x_loom_intake_slice::executor::UnconfinedRunner),
        run_inputs(&[]),
        &SystemClock,
    );
    (driven.run, driven.output, driven.frontiers, driven.evidence)
}

/// [`drive`], also returning every observation the governor received, in order.
fn drive_observed(
    fixture: &Fixture,
    row: Row,
) -> (
    b10x_loom_intake_slice::run::SliceRun,
    String,
    usize,
    Vec<ObservationData>,
) {
    drive_with_runner(
        fixture,
        row,
        std::sync::Arc::new(b10x_loom_intake_slice::executor::UnconfinedRunner),
    )
}

fn drive_with_runner(
    fixture: &Fixture,
    row: Row,
    runner: std::sync::Arc<dyn b10x_loom_intake_slice::confinement::TestRunner>,
) -> (
    b10x_loom_intake_slice::run::SliceRun,
    String,
    usize,
    Vec<ObservationData>,
) {
    let (slice, output, frontiers, observations, _) =
        drive_with_inputs(fixture, row, runner, run_inputs(&[]));
    (slice, output, frontiers, observations)
}

/// The case inputs `b10x-loom run` gives the case for `flags`, parsed by its own command line.
fn run_inputs(flags: &[&str]) -> CaseInputs {
    let mut argv = vec!["b10x-loom", "run", "--workspace", "."];
    argv.extend_from_slice(flags);
    argv.push(INTENT);
    match Cli::try_parse_from(argv).expect("the flags parse").command {
        CliCommand::Run(run) => run.inputs(),
        _ => panic!("`run` parses as the run command"),
    }
}

/// [`drive_with_runner`] with the case `inputs`, also returning every request the agent was asked,
/// as JSON, in order.
fn drive_with_inputs(
    fixture: &Fixture,
    row: Row,
    runner: std::sync::Arc<dyn b10x_loom_intake_slice::confinement::TestRunner>,
    inputs: CaseInputs,
) -> (
    b10x_loom_intake_slice::run::SliceRun,
    String,
    usize,
    Vec<ObservationData>,
    Vec<String>,
) {
    let driven = drive_clocked(fixture, row, runner, inputs, &SystemClock);
    (
        driven.run,
        driven.output,
        driven.frontiers,
        driven.observations,
        driven.requests,
    )
}

/// What [`drive_clocked`] saw: the run, its output, the frontiers issued, every observation the
/// governor received and every request the agent was asked, in order, and, once a case opened,
/// every evidence record the governor holds for it, in the order received, and its frontier
/// after the run.
struct Driven {
    run: b10x_loom_intake_slice::run::SliceRun,
    output: String,
    frontiers: usize,
    observations: Vec<ObservationData>,
    requests: Vec<String>,
    evidence: Vec<EvidenceData>,
    frontier: Option<FrontierData>,
}

/// Runs the slice for `row` on `fixture` with the case `inputs`, its observations' instants read
/// from `clock`.
fn drive_clocked(
    fixture: &Fixture,
    row: Row,
    runner: std::sync::Arc<dyn b10x_loom_intake_slice::confinement::TestRunner>,
    inputs: CaseInputs,
    clock: &dyn Clock,
) -> Driven {
    let classifier = Recorded::new("recorded-classifier", vec![row.pick]);
    let agent = Recorded::new("recorded-agent", row.agent);
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let frontiers = Frontiers {
        governor: &governor,
        nothing_admissible: row.nothing_admissible,
        issued: AtomicUsize::new(0),
        case: Mutex::new(None),
    };
    let request = SliceRequest {
        runner,
        intent: INTENT.to_owned(),
        workspace: fixture.workspace().to_path_buf(),
        test: TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
        max_steps: row.max_steps,
        threshold: THRESHOLD,
        inputs,
        test_paths: b10x_loom_intake_slice::test_paths::TestPaths::default(),
    };
    let mut out = Vec::new();
    let result = run_with_clock(
        &request,
        &governor,
        &frontiers,
        &classifier,
        &agent,
        clock,
        &mut out,
        &RunOptions::default(),
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
    let issued = frontiers.issued.load(Ordering::SeqCst);
    let case = frontiers.case.lock().expect("case").clone();
    let (evidence, frontier) = match case {
        Some(case) => (
            governor.evidence(&case).expect("the case's evidence"),
            Some(governor.frontier(&case).expect("a frontier").into_data()),
        ),
        None => (Vec::new(), None),
    };
    Driven {
        run: slice,
        output,
        frontiers: issued,
        observations: governor.observations(),
        requests: agent.requests(),
        evidence,
        frontier,
    }
}

/// The kind of each of `evidence`, in order.
fn kinds(evidence: &[EvidenceData]) -> Vec<&str> {
    evidence.iter().map(|record| record.kind.as_str()).collect()
}

/// The text member `name` of `record`'s facts.
fn fact<'r>(record: &'r EvidenceData, name: &str) -> &'r str {
    match record.facts.member(name) {
        Some(CommissionJson::Text(text)) => text,
        other => panic!("{} has no text `{name}`: {other:?}", record.kind),
    }
}

/// Whether `frontier` holds `obligation` open.
fn obligation_open(frontier: &FrontierData, obligation: &str) -> bool {
    frontier
        .obligations
        .iter()
        .find(|entry| entry.obligation == obligation)
        .unwrap_or_else(|| panic!("the frontier lists `{obligation}`: {frontier:?}"))
        .open
}

/// `claim`'s value in `frontier`.
fn claim(frontier: &FrontierData, claim: &str) -> Truth {
    frontier
        .claims
        .iter()
        .find(|entry| entry.claim == claim)
        .unwrap_or_else(|| panic!("the frontier lists `{claim}`: {frontier:?}"))
        .value
}

/// A host clock for a test: its first reading is at [`Stepping::START`] and each later one
/// [`Stepping::STEP`] seconds after the one before, so each observation of a run falls in a second
/// of its own.
struct Stepping {
    readings: AtomicI64,
}

impl Stepping {
    /// 2026-10-03T13:20:00Z.
    const START: i64 = 1_791_000_000;
    const STEP: i64 = 10;

    fn new() -> Self {
        Self {
            readings: AtomicI64::new(0),
        }
    }

    /// The `n`th reading (from 0), as Canon writes an instant.
    fn reading(n: i64) -> String {
        DateTime::from_timestamp(Self::START + n * Self::STEP, 0)
            .expect("an instant")
            .to_rfc3339_opts(SecondsFormat::Secs, true)
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
    /// The case of the last frontier issued.
    case: Mutex<Option<CaseId>>,
}

impl Governor for Frontiers<'_> {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.governor.current_revision(case)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        self.issued.fetch_add(1, Ordering::SeqCst);
        *self.case.lock().expect("case") = Some(case.clone());
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
    /// Every request the model was asked, as JSON, in order.
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
            asked: AtomicUsize::new(0),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().expect("requests").clone()
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
            self.requests
                .lock()
                .expect("requests")
                .push(serde_json::to_string(request).expect("a request is JSON"));
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

/// Adversary pass 1 on `story:runtime-merge`: the gate-stop rule (`7dc84ef`) at the step budget.
/// The slice's own loop checked the gate right after the step, before the budget: a last step that
/// leaves the approval gate unchanged stopped `ApprovalRequired` (exit 0, the human gate) even when
/// that step used up `max_steps`. The same run as
/// `the_run_stops_at_the_approval_gate_when_the_model_idles`, with `max_steps` equal to its four
/// steps, must stop the same way.
#[test]
fn adversary_the_gate_stops_before_the_budget_on_the_last_step() {
    let fixture = Fixture::new("gate-at-budget");
    let (run, output, _) = drive(
        &fixture,
        Row {
            name: "the approval gate reached on the last budgeted step",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![
                json!({"action": "repository.inspect"}),
                json!({"paths": ["check.txt"]}),
                json!({"action": "implementation.edit"}),
                json!({
                    "files": [{"path": "check.txt", "contents": "fixed\n"}],
                    "message": "fix the check"
                }),
                json!({"action": "tests.run"}),
                json!({}),
                json!({"action": "tests.run"}),
                json!({}),
            ],
            max_steps: 4,
            nothing_admissible: false,
        },
    );
    assert_eq!(
        steps(&output),
        [
            "repository.inspect",
            "implementation.edit",
            "tests.run",
            "tests.run"
        ],
        "{output}"
    );
    assert_stopped(&output, "stopped: ApprovalRequired (repository.merge)");
    assert_eq!(run.stop_reason, StopReason::ApprovalRequired, "{output}");
    assert_eq!(run.steps, 4);
}

// Story `change-evidence-order`: the slice's evidence lets `software.change/1`'s order,
// independence and age rules decide (`intake.routing.SliceEvidence`).

/// A revision of every artifact `software.change/1` declares, for a case a test opens itself:
/// `implementation` at `r1`, `deployment` at `d1`.
fn software_revisions() -> BTreeMap<String, String> {
    [
        ("intent", "i1"),
        ("system_specification", "s1"),
        ("plan", "p1"),
        ("implementation", "r1"),
        ("release", "v1"),
        ("deployment", "d1"),
    ]
    .into_iter()
    .map(|(artifact, revision)| (artifact.to_owned(), revision.to_owned()))
    .collect()
}

/// The edit a run makes to fix the fixture's check.
fn fix_the_check() -> Value {
    json!({
        "files": [{"path": "check.txt", "contents": "fixed\n"}],
        "message": "fix the check"
    })
}

/// A bugfix that writes its regression test, runs it red, then edits: the first regression test
/// result fails and is observed before the implementation's first change, so
/// `regression.shown_first` holds and `red_first.required` is discharged. The test run after the
/// `tests.write` submits a `regression_test_result` beside its `test_result`, both citing the run's
/// one observation; the edit submits one `implementation_change`.
#[test]
fn a_bugfix_red_regression_test_before_its_edit_discharges_red_first() {
    let fixture = Fixture::new("red-first");
    let clock = Stepping::new();
    let driven = drive_clocked(
        &fixture,
        Row {
            name: "a bugfix whose regression test fails before its edit",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![
                json!({"action": "tests.write"}),
                json!({
                    "files": [{"path": "tests/regression.txt", "contents": "check.txt reads fixed\n"}],
                    "message": "the regression test"
                }),
                json!({"action": "tests.run"}),
                json!({}),
                json!({"action": "implementation.edit"}),
                fix_the_check(),
                json!({"action": "tests.run"}),
                json!({}),
                json!({"action": "repository.merge"}),
                json!({}),
            ],
            max_steps: 10,
            nothing_admissible: false,
        },
        std::sync::Arc::new(b10x_loom_intake_slice::executor::UnconfinedRunner),
        run_inputs(&["--change-kind", "bugfix"]),
        &clock,
    );
    let output = &driven.output;
    assert_eq!(
        steps(output),
        [
            "tests.write",
            "tests.run",
            "implementation.edit",
            "tests.run"
        ],
        "{output}"
    );
    assert_eq!(count_starting(output, "effect: refused:"), 0, "{output}");
    assert_eq!(
        count_lines(
            output,
            "evidence: test_result fail, regression_test_result fail"
        ),
        1,
        "{output}"
    );
    assert_eq!(
        count_lines(
            output,
            "evidence: test_result pass, regression_test_result pass"
        ),
        1,
        "{output}"
    );
    assert_stopped(output, "stopped: ApprovalRequired (repository.merge)");
    let evidence = &driven.evidence;
    assert_eq!(
        kinds(evidence),
        [
            "test_result",
            "regression_test_result",
            "implementation_change",
            "test_result",
            "regression_test_result"
        ],
        "{output}"
    );
    assert_eq!(
        evidence[0].observation_ids, evidence[1].observation_ids,
        "one observation yields both results"
    );
    assert_eq!(fact(&evidence[1], "result"), "fail");
    assert_eq!(fact(&evidence[4], "result"), "pass");
    let frontier = driven.frontier.as_ref().expect("the case's frontier");
    assert_eq!(claim(frontier, "regression.shown_first"), Truth::True);
    assert!(
        !obligation_open(frontier, "red_first.required"),
        "red_first.required is discharged: {frontier:?}"
    );
}

/// A bugfix whose suite fails before any `tests.write`, then edits: the edit is admitted (a test
/// failed on the current revision), but no regression test result was ever submitted, so the
/// implementation changed with no regression test shown first and `red_first.required` stays
/// open.
#[test]
fn a_bugfix_edit_before_any_regression_test_leaves_red_first_open() {
    let fixture = Fixture::new("no-red-first");
    let first = fixture.head();
    let clock = Stepping::new();
    let driven = drive_clocked(
        &fixture,
        Row {
            name: "a bugfix that edits with no regression test",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![
                json!({"action": "tests.run"}),
                json!({}),
                json!({"action": "implementation.edit"}),
                fix_the_check(),
                json!({"action": "tests.run"}),
                json!({}),
                json!({"action": "repository.merge"}),
                json!({}),
            ],
            max_steps: 10,
            nothing_admissible: false,
        },
        std::sync::Arc::new(b10x_loom_intake_slice::executor::UnconfinedRunner),
        run_inputs(&["--change-kind", "bugfix"]),
        &clock,
    );
    let output = &driven.output;
    assert_eq!(
        steps(output),
        ["tests.run", "implementation.edit", "tests.run"],
        "{output}"
    );
    assert_eq!(
        count_starting(output, "effect: refused:"),
        0,
        "the edit is admitted: {output}"
    );
    assert_ne!(fixture.head(), first, "the edit committed");
    assert_eq!(
        kinds(&driven.evidence),
        ["test_result", "implementation_change", "test_result"],
        "{output}"
    );
    let frontier = driven.frontier.as_ref().expect("the case's frontier");
    assert_ne!(claim(frontier, "regression.shown_first"), Truth::True);
    assert!(
        obligation_open(frontier, "red_first.required"),
        "red_first.required stays open: {frontier:?}"
    );
}

/// Every performed `implementation.edit` that committed submits one `implementation_change` about
/// the new `HEAD`, citing the edit's own observation. A refused edit, a `tests.write` and an edit
/// that wrote what the files already held submit none.
#[test]
fn an_edit_submits_implementation_change_about_the_new_head() {
    let fixture = Fixture::new("change");
    let first = fixture.head();
    let driven = drive_clocked(
        &fixture,
        Row {
            name: "edits, refused, written and repeated",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![
                json!({"action": "implementation.edit"}),
                json!({"files": [{"path": "../outside.txt", "contents": "x\n"}]}),
                json!({"action": "tests.write"}),
                json!({"files": [{"path": "tests/check_test.txt", "contents": "fixed\n"}]}),
                json!({"action": "implementation.edit"}),
                fix_the_check(),
                json!({"action": "implementation.edit"}),
                fix_the_check(),
                json!({"action": "tests.run"}),
                json!({}),
                json!({"action": "repository.merge"}),
                json!({}),
            ],
            max_steps: 10,
            nothing_admissible: false,
        },
        std::sync::Arc::new(b10x_loom_intake_slice::executor::UnconfinedRunner),
        run_inputs(&[]),
        &SystemClock,
    );
    let output = &driven.output;
    assert_eq!(
        steps(output),
        [
            "implementation.edit",
            "tests.write",
            "implementation.edit",
            "implementation.edit",
            "tests.run"
        ],
        "{output}"
    );
    assert_eq!(
        count_starting(output, "effect: refused:"),
        1,
        "only the edit outside the workspace is refused: {output}"
    );
    assert_eq!(
        count_lines(output, "evidence: implementation_change"),
        1,
        "{output}"
    );
    let head = fixture.head();
    assert_eq!(
        fixture
            .git(&["rev-list", "--count", &format!("{first}..HEAD")])
            .trim(),
        "2",
        "the test and the one edit that changed something committed: {output}"
    );
    let changes: Vec<&EvidenceData> = driven
        .evidence
        .iter()
        .filter(|record| record.kind == "implementation_change")
        .collect();
    assert_eq!(changes.len(), 1, "{:?}", kinds(&driven.evidence));
    let change = changes[0];
    assert_eq!(fact(change, "subject"), "implementation");
    assert_eq!(fact(change, "subject_revision"), head, "about the new HEAD");
    assert!(
        change.facts.member("result").is_none(),
        "a change has no result"
    );
    let [observation] = change.observation_ids.as_slice() else {
        panic!("one observation: {:?}", change.observation_ids)
    };
    let observed = driven
        .observations
        .iter()
        .find(|seen| &seen.observation_id == observation)
        .expect("the governor holds the cited observation");
    assert_eq!(observed.source, EDIT_SOURCE);
    assert_eq!(
        observed.payload.member("implementation"),
        Some(&CommissionJson::Text(head.clone()))
    );
    assert_eq!(
        kinds(&driven.evidence),
        ["implementation_change", "test_result"],
        "{output}"
    );
}

/// Every record the slice submits carries `observed_at`, the host clock's instant of the
/// observation it cites, and the verifier's trusted producer, in the evidence and in its record.
/// The producer reaching Canon is the trusted one: a producer the record's facts name is never
/// read, so one Canon would refuse as an identifier does not keep a record from applying.
#[test]
fn slice_evidence_carries_observed_at_and_the_trusted_producer() {
    let fixture = Fixture::new("observed-at");
    let clock = Stepping::new();
    let driven = drive_clocked(
        &fixture,
        Row {
            name: "a run with a stepping clock",
            pick: pick(SOFTWARE_CHANGE, 0.9),
            agent: vec![
                json!({"action": "implementation.edit"}),
                fix_the_check(),
                json!({"action": "tests.run"}),
                json!({}),
                json!({"action": "repository.merge"}),
                json!({}),
            ],
            max_steps: 10,
            nothing_admissible: false,
        },
        std::sync::Arc::new(b10x_loom_intake_slice::executor::UnconfinedRunner),
        run_inputs(&[]),
        &clock,
    );
    let output = &driven.output;
    assert_eq!(
        kinds(&driven.evidence),
        ["implementation_change", "test_result"],
        "{output}"
    );
    for (n, record) in driven.evidence.iter().enumerate() {
        let expected = Stepping::reading(i64::try_from(n).expect("a count"));
        assert_eq!(fact(record, "observed_at"), expected, "{}", record.kind);
        assert_eq!(record.producer, PRODUCER, "{}", record.kind);
        assert_eq!(fact(record, "producer"), PRODUCER, "{}", record.kind);
        let cited = driven
            .observations
            .iter()
            .find(|seen| record.observation_ids.contains(&seen.observation_id))
            .expect("the cited observation");
        assert_eq!(cited.observed_at.0, expected, "{}", record.kind);
    }
    assert!(
        driven
            .observations
            .iter()
            .any(|seen| seen.source == TEST_RUN_SOURCE)
    );

    // A record whose facts name a producer Canon refuses as an identifier still applies: the
    // producer Canon reads is the one `submit_evidence` attributed it to.
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = governor
        .open(
            SOFTWARE_CHANGE,
            software_revisions(),
            loom_governor::CaseInputs::try_from_pairs([
                ("risk", "trivial"),
                ("change_kind", "feature"),
                ("affects_security_boundary", "false"),
            ])
            .expect("inputs"),
        )
        .expect("a software change opens");
    let facts = commission_json::parse(
        r#"{"format":"canon-evidence/1","id":"forged-producer","kind":"test_result","subject":"implementation","subject_revision":"r1","result":"pass","observed_at":"2026-10-03T13:20:00Z","producer":"a model said so"}"#,
    )
    .expect("facts");
    submit_evidence(
        &governor,
        PRODUCER,
        EvidenceData {
            evidence_id: EvidenceId(Uuid("00000000-0000-8000-8000-000000000001".into())),
            case_id: case.clone(),
            kind: "test_result".into(),
            subject_revision: 1,
            producer: String::new(),
            observation_ids: vec![ObservationId(Uuid(
                "00000000-0000-8000-8000-000000000002".into(),
            ))],
            facts,
            provenance: CommissionJson::Null,
        },
    )
    .expect("submitted");
    let frontier = governor.frontier(&case).expect("a frontier").into_data();
    assert_eq!(
        claim(&frontier, "tests.pass"),
        Truth::True,
        "the record applies under its trusted producer: {frontier:?}"
    );
}

/// The governor a slice host builds (`slice_governor`, as `b10x-loom run` and `resume` build it)
/// evaluates at the host instant: an `operational_observation`, whose `max_age` is 30 minutes,
/// observed 31 minutes ago no longer applies, so `deployment.healthy` does not hold and
/// `health.required` stays open, while one observed a minute ago discharges it. A governor built
/// with no evaluation time lets the old observation apply.
#[test]
fn a_slice_governor_expires_an_operational_observation_after_max_age() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let inputs = || {
        loom_governor::CaseInputs::try_from_pairs([
            ("risk", "critical"),
            ("change_kind", "feature"),
            ("affects_security_boundary", "false"),
        ])
        .expect("inputs")
    };
    let revisions = software_revisions;
    let observation = |id: u32, ago: TimeDelta, case: &CaseId| {
        let observed_at = (Utc::now() - ago).to_rfc3339_opts(SecondsFormat::Secs, true);
        let facts = commission_json::parse(&format!(
            r#"{{"format":"canon-evidence/1","id":"health-{id}","kind":"operational_observation","subject":"deployment","subject_revision":"d1","result":"healthy","observed_at":"{observed_at}"}}"#
        ))
        .expect("facts");
        EvidenceData {
            evidence_id: EvidenceId(Uuid(format!("00000000-0000-8000-8000-{id:012}"))),
            case_id: case.clone(),
            kind: "operational_observation".into(),
            subject_revision: 1,
            producer: String::new(),
            observation_ids: vec![ObservationId(Uuid(format!(
                "00000000-0000-8000-9000-{id:012}"
            )))],
            facts,
            provenance: CommissionJson::Null,
        }
    };

    let governor =
        slice_governor(MemoryCaseStore::default(), &catalog).expect("the slice's governor");
    let case = governor
        .open(SOFTWARE_CHANGE, revisions(), inputs())
        .expect("a critical software change opens");
    submit_evidence(
        &governor,
        "host:operations",
        observation(1, TimeDelta::minutes(31), &case),
    )
    .expect("submitted");
    let frontier = governor.frontier(&case).expect("a frontier").into_data();
    assert_ne!(claim(&frontier, "deployment.healthy"), Truth::True);
    assert!(
        obligation_open(&frontier, "health.required"),
        "an observation older than 30 minutes no longer applies: {frontier:?}"
    );

    let clockless = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog)
        .expect("a governor with no evaluation time");
    let old = clockless
        .open(SOFTWARE_CHANGE, revisions(), inputs())
        .expect("opens");
    submit_evidence(
        &clockless,
        "host:operations",
        observation(2, TimeDelta::minutes(31), &old),
    )
    .expect("submitted");
    let frontier = clockless.frontier(&old).expect("a frontier").into_data();
    assert!(
        !obligation_open(&frontier, "health.required"),
        "with no evaluation time nothing expires: {frontier:?}"
    );

    submit_evidence(
        &governor,
        "host:operations",
        observation(3, TimeDelta::minutes(1), &case),
    )
    .expect("submitted");
    let frontier = governor.frontier(&case).expect("a frontier").into_data();
    assert_eq!(claim(&frontier, "deployment.healthy"), Truth::True);
    assert!(
        !obligation_open(&frontier, "health.required"),
        "a fresh observation discharges it: {frontier:?}"
    );
}
