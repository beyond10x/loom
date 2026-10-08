//! Acceptance for `story:run-event-stream`: `b10x-loom run --output jsonl` writes one JSON object
//! per line on standard output (`intake.events.RunEventLine` in `ess/intake/domains/events.yaml`),
//! and its last line is the one terminal record, whose `exit_status` is the process exit status.
//!
//! The recorded cases drive the steps the binary takes (`src/main.rs`): route the intent with
//! `prepare_intent`, run the accepted route with `run_prepared_intent`, and end the stream with
//! [`EventStream::finish`], whose answer `main` returns as the exit status. Both models are
//! recorded fakes, wrapped by [`EventStream::observe`] as the binary wraps its Codex models; the
//! workspace is a git repository with one failing check, and the test command is `grep`. No case
//! reaches a model, the network or a credential. The binary-level case runs `b10x-loom` with an
//! empty `HOME` and `CODEX_HOME`, so it fails before any model, and reads no login.

use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use b10x_loom_cli::events::{EventStream, SCHEMA_VERSION};
use b10x_loom_cli::{Cli, Command as CliCommand, Output, exit_status};
use b10x_loom_intake_slice::clock::HostClock;
use b10x_loom_intake_slice::context_metrics::{ContextMetrics, ContextPolicy};
use b10x_loom_intake_slice::executor::{TestCommand, UnconfinedRunner};
use b10x_loom_intake_slice::intent::{
    IntentRequest, Preparation, prepare_intent, run_intent_with_options, run_prepared_intent,
};
use b10x_loom_intake_slice::run::{RunOptions, StopReason};
use clap::Parser;
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason as TurnStop, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest, Usage,
};
use loom_governor::{CanonGovernor, MemoryCaseStore};
use loom_protocols::ProtocolCatalog;
use serde_json::{Value, json};

const INTENT: &str = "Fix the check in check.txt and verify the change.";

/// The recorded run that reaches the merge gate: three steps, then `repository.merge` proposed.
fn to_the_merge_gate() -> Vec<(&'static str, Value)> {
    [
        action("tests.run", json!({})),
        action("repository.edit", edit("fixed\n")),
        action("tests.run", json!({})),
        action("repository.merge", json!({})),
    ]
    .concat()
}

#[test]
fn the_last_line_is_the_one_terminal_record_and_its_exit_status_is_the_exit_status() {
    for (name, script, max_steps, stopped, code) in [
        (
            "merge gate",
            to_the_merge_gate(),
            8,
            StopReason::ApprovalRequired,
            0,
        ),
        (
            "step budget",
            action("tests.run", json!({})),
            1,
            StopReason::StepBudget,
            3,
        ),
    ] {
        let (lines, status) = drive_jsonl(script, max_steps);
        assert_eq!(status, code, "{name}: {lines:#?}");
        assert_eq!(status, exit_status(stopped), "{name}");
        let terminals: Vec<&Value> = lines.iter().filter(|l| l["kind"] == "Terminal").collect();
        assert_eq!(
            terminals.len(),
            1,
            "{name}: one terminal record: {lines:#?}"
        );
        let last = lines.last().expect("the stream is not empty");
        assert_eq!(last["kind"], "Terminal", "{name}: the terminal is last");
        assert_eq!(last["exit_status"], json!(status), "{name}: {last}");
        assert_eq!(last["stop_reason"], json!(format!("{stopped:?}")), "{name}");
        assert_eq!(last["protocol"], "software-change@1", "{name}");
        for kind in ["Turn", "ToolCall", "Usage"] {
            assert!(
                lines.iter().any(|l| l["kind"] == kind),
                "{name}: at least one {kind} record: {lines:#?}"
            );
        }
        let route: Vec<&Value> = lines.iter().filter(|l| l["kind"] == "Route").collect();
        assert_eq!(route.len(), 1, "{name}: {lines:#?}");
        assert_eq!(route[0]["protocol"], "software-change@1");
        assert_eq!(route[0]["accepted"], true);
        let approvals: Vec<&Value> = lines.iter().filter(|l| l["kind"] == "Approval").collect();
        if stopped == StopReason::ApprovalRequired {
            assert_eq!(approvals.len(), 1, "{name}: {lines:#?}");
            assert_eq!(approvals[0]["actions"], json!(["repository.merge"]));
        } else {
            assert!(approvals.is_empty(), "{name}: {lines:#?}");
        }
    }
}

#[test]
fn every_line_is_one_object_carrying_the_schema_version() {
    let (lines, _) = drive_jsonl(to_the_merge_gate(), 8);
    assert_eq!(SCHEMA_VERSION, 1);
    for line in &lines {
        assert!(line.is_object(), "{line}");
        assert_eq!(line["schema_version"], json!(SCHEMA_VERSION), "{line}");
        assert!(line["kind"].is_string(), "{line}");
    }
    let turns = lines.iter().filter(|l| l["kind"] == "Turn").count();
    assert_eq!(turns, 9, "one classification and eight agent turns");
    let calls: Vec<&Value> = lines.iter().filter(|l| l["kind"] == "ToolCall").collect();
    assert_eq!(calls.len(), 9);
    assert_eq!(calls[0]["name"], "pick_protocol");
    assert_eq!(calls[1]["name"], "select_action");
    let arguments: Value = serde_json::from_str(calls[1]["arguments"].as_str().unwrap()).unwrap();
    assert_eq!(arguments, json!({"action": "tests.run"}));
    let usage = lines.iter().find(|l| l["kind"] == "Usage").unwrap();
    assert_eq!(usage["input_tokens"], 120);
    assert_eq!(usage["output_tokens"], 7);
    assert_eq!(usage["final_usage"], true);
    assert!(
        usage.get("cache_creation_input_tokens").is_none(),
        "an unreported counter is absent, never zero: {usage}"
    );
}

/// The human output of a recorded run, byte for byte as before the stream existed. The one value
/// that changes from run to run, the commit the edit makes, is replaced by `<commit>`.
#[test]
fn the_same_run_without_the_flag_prints_the_human_lines_byte_for_byte() {
    let fixture = Fixture::new();
    let catalog = ProtocolCatalog::bundled().unwrap();
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog)
        .unwrap();
    let classifier = Recorded::new(classify());
    let agent = Recorded::new(to_the_merge_gate());
    let mut out = Vec::new();
    run_intent_with_options(
        &request(&fixture, 8),
        &catalog,
        &governor,
        &governor,
        &classifier,
        &agent,
        &HostClock,
        &mut out,
        &RunOptions::default(),
    )
    .unwrap();
    let output = String::from_utf8(out).unwrap();
    let head = fixture.git(&["rev-parse", "HEAD"]);
    let output = output.replace(head.trim(), "<commit>");
    assert_eq!(output, HUMAN, "{output}");
    let Ok(Cli {
        command: CliCommand::Run(arguments),
    }) = Cli::try_parse_from(["b10x-loom", "run", INTENT])
    else {
        panic!("run parses")
    };
    assert_eq!(
        arguments.output,
        Output::Human,
        "human output is the default"
    );
}

/// `b10x-loom run --output jsonl` that fails (no Codex login) still ends its stream with the one
/// terminal record, and the process exits with that record's status.
#[test]
fn the_binary_ends_a_failed_run_with_a_terminal_record_carrying_its_exit_status() {
    let fixture = Fixture::new();
    let home = fixture.directory.path().join("home");
    std::fs::create_dir(&home).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
        .args(["run", "--output", "jsonl", "--confinement", "none"])
        .arg("--workspace")
        .arg(&fixture.workspace)
        .arg(INTENT)
        .env("HOME", &home)
        .env("CODEX_HOME", &home)
        .env("XDG_DATA_HOME", &home)
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect();
    let code = output.status.code().expect("an exit status");
    assert_eq!(code, 1, "{stdout}");
    let last = lines.last().expect("a terminal record");
    assert_eq!(lines.iter().filter(|l| l["kind"] == "Terminal").count(), 1);
    assert_eq!(last["kind"], "Terminal");
    assert_eq!(last["exit_status"], json!(code));
    assert_eq!(last["schema_version"], json!(SCHEMA_VERSION));
    assert!(last.get("stop_reason").is_none(), "{last}");
    assert!(last["error"].as_str().unwrap().contains("codex"), "{last}");
}

const HUMAN: &str = "picked software-change@1 (confidence 0.99)
  reason: change code
confinement: none
frontier: repository.edit (admissible), repository.inspect (admissible), repository.merge (blocked), tests.run (admissible)
step 1: tests.run {}
  effect: the test command exited with 1
    | confinement: none
  evidence: test_result fail
frontier: repository.edit (admissible), repository.inspect (admissible), repository.merge (blocked), tests.run (admissible)
step 2: repository.edit {\"files\":[{\"contents\":\"fixed\\n\",\"path\":\"check.txt\"}],\"message\":\"update the check\"}
  effect: committed; HEAD is <commit>
  evidence: none
frontier: repository.edit (admissible), repository.inspect (admissible), repository.merge (blocked), tests.run (admissible)
step 3: tests.run {}
  effect: the test command exited with 0
    | confinement: none
  evidence: test_result pass
frontier: repository.edit (admissible), repository.inspect (admissible), repository.merge (approval required), tests.run (admissible)
stopped: ApprovalRequired (repository.merge)
";

/// Runs `script` the way `b10x-loom run --output jsonl` does; returns the parsed lines and the
/// exit status `main` would return.
fn drive_jsonl(script: Vec<(&'static str, Value)>, max_steps: usize) -> (Vec<Value>, u8) {
    let fixture = Fixture::new();
    let buffer = Buffer::default();
    let stream = EventStream::new(buffer.clone());
    let catalog = ProtocolCatalog::bundled().unwrap();
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog)
        .unwrap();
    let classifier = Recorded::new(classify());
    let agent = Recorded::new(script);
    let classifier_seen = stream.observe(&classifier);
    let agent_seen = stream.observe(&agent);
    let options = RunOptions::default();
    let metrics = ContextMetrics::new(ContextPolicy::Legacy);
    let request = request(&fixture, max_steps);
    let mut human = std::io::sink();
    let ended = match prepare_intent(
        &request,
        &catalog,
        &classifier_seen,
        &mut human,
        &options,
        &metrics,
    )
    .unwrap()
    {
        Preparation::Ready(prepared) => {
            stream.route(prepared.protocol(), true).unwrap();
            run_prepared_intent(
                &request,
                &prepared,
                &catalog,
                &governor,
                &governor,
                &agent_seen,
                &HostClock,
                &mut human,
                &options,
                &metrics,
            )
            .unwrap()
        }
        Preparation::Stopped(run) => {
            stream.route(&run.protocol, false).unwrap();
            run
        }
    };
    let status = stream.finish(Ok(&ended)).unwrap();
    let text = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
    // The stream as `b10x-loom` writes it; `--nocapture` shows it (website/docs/reference/run-events.md).
    print!("{text}");
    let lines = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect();
    (lines, status)
}

fn request(fixture: &Fixture, max_steps: usize) -> IntentRequest {
    IntentRequest {
        intent: INTENT.into(),
        workspace: Some(fixture.workspace.clone()),
        test: TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
        runner: Some(Arc::new(UnconfinedRunner)),
        max_steps,
        threshold: 0.5,
    }
}

fn classify() -> Vec<(&'static str, Value)> {
    vec![(
        "pick_protocol",
        json!({"protocol": "software-change@1", "confidence": 0.99, "reasons": ["change code"]}),
    )]
}

fn action(name: &str, arguments: Value) -> Vec<(&'static str, Value)> {
    vec![
        ("select_action", json!({ "action": name })),
        ("action_arguments", arguments),
    ]
}

fn edit(contents: &str) -> Value {
    json!({"files": [{"path": "check.txt", "contents": contents}], "message": "update the check"})
}

/// Standard output, kept for the test to read.
#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A model that answers each request with the next recorded forced call, reporting fixed usage
/// with the cache-creation counter unreported.
struct Recorded {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<(&'static str, Value)>>,
    calls: AtomicUsize,
}

impl Recorded {
    fn new(script: Vec<(&'static str, Value)>) -> Self {
        let id = |text: &str| Id::new(text).unwrap();
        Self {
            provenance: Provenance {
                protocol: Protocol::Responses,
                provider: id("recorded"),
                account: id("fixture"),
                endpoint: id("in-process"),
                model: id("recorded"),
                binding_revision: id("recorded-1"),
            },
            capabilities: Capabilities {
                tools: true,
                tool_choice: true,
                ..Capabilities::text(128_000, 8192)
            },
            script: Mutex::new(script.into()),
            calls: AtomicUsize::new(0),
        }
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
        _: &'a mut dyn StreamSink,
        _: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        Box::pin(async move {
            request
                .validate_for(&self.provenance, &self.capabilities)
                .unwrap();
            let ToolChoice::Named(tool) = &request.tool_choice else {
                panic!("expected a forced tool")
            };
            let (expected, arguments) = self
                .script
                .lock()
                .unwrap()
                .pop_front()
                .expect("a recorded reply");
            assert_eq!(tool.as_str(), expected);
            let count = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(TurnOutcome {
                stop_reason: TurnStop::ToolCalls,
                items: vec![Item::ToolCall(ToolCall {
                    call_id: CallId::new(format!("call-{count}")).unwrap(),
                    name: tool.clone(),
                    arguments,
                })],
                observation: TurnObservation {
                    usage: Some(Usage {
                        input_tokens: Some(120),
                        output_tokens: Some(7),
                        cached_input_tokens: Some(0),
                        ..Usage::default()
                    }),
                    final_usage: true,
                    ..TurnObservation::new(self.provenance.clone())
                },
            })
        })
    }
}

/// A git repository with a local identity and one failing check.
struct Fixture {
    directory: tempfile::TempDir,
    workspace: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::Builder::new()
            .prefix("run-events-")
            .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
            .unwrap();
        let workspace = directory.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        let fixture = Self {
            directory,
            workspace,
        };
        fixture.git(&["init", "--quiet", "--initial-branch=main"]);
        fixture.git(&["config", "user.name", "Fixture Author"]);
        fixture.git(&["config", "user.email", "fixture@example.invalid"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(fixture.workspace.join("check.txt"), "broken\n").unwrap();
        fixture.git(&["add", "check.txt"]);
        fixture.git(&["commit", "--quiet", "-m", "a failing check"]);
        fixture
    }

    fn git(&self, args: &[&str]) -> String {
        let mut command = Command::new("git");
        for variable in b10x_loom_intake_slice::case::REDIRECTING_GIT_VARIABLES {
            command.env_remove(variable);
        }
        let output = command
            .args(args)
            .current_dir(&self.workspace)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
}
