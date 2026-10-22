//! Security review of `story:checked-timeout-deadlines` in the slice: every timeout the slice turns
//! into a deadline is added with checked arithmetic, a timeout the clock cannot hold starts
//! nothing, and a large timeout the clock can hold, or a zero one, still runs and ends the command
//! without a panic.
//!
//! The test command runs through `UnconfinedRunner` directly; the bound command runs through
//! `run_intent_with_options` with recorded models, as `bound_command.rs` does. No network, no
//! credential, no model call. Scratch goes under `CARGO_TARGET_TMPDIR`.

use std::collections::VecDeque;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use b10x_loom_intake_slice::bindings::{ActionBinding, ActionBindings};
use b10x_loom_intake_slice::clock::SystemClock;
use b10x_loom_intake_slice::confinement::TestRunner as _;
use b10x_loom_intake_slice::context::ContextPolicy;
use b10x_loom_intake_slice::executor::{TestCommand, UnconfinedRunner};
use b10x_loom_intake_slice::intent::{IntentRequest, run_intent_with_options};
use b10x_loom_intake_slice::run::RunOptions;
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason as TurnStop, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest,
};
use loom_governor::{CanonGovernor, MemoryCaseStore};
use loom_protocols::{InstallStore, ProtocolCatalog};
use serde_json::{Value, json};

/// The largest whole-second timeout a monotonic clock that started at boot can hold: half of
/// `i64::MAX` seconds, far past any uptime and far below the overflow.
const HOLDABLE: u64 = (i64::MAX as u64) / 2;

fn scratch(test: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("security_w20261022w2_u1")
        .join(test);
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    std::fs::create_dir_all(&root).unwrap();
    root
}

// ---------------------------------------------------------------------------------------------
// The test command (`executor::run_bounded` through `UnconfinedRunner`).

/// Every timeout past what the clock can hold, not only `Duration::MAX`, is refused before the
/// command starts: `u64::MAX` whole seconds and `i64::MAX` seconds (the first value the
/// monotonic clock's signed seconds cannot add once it is past zero).
#[test]
fn security_a_test_command_timeout_past_the_clock_never_starts_the_command() {
    let root = scratch("test_command_past_the_clock");
    for (index, timeout) in [
        Duration::MAX,
        Duration::from_secs(u64::MAX),
        Duration::from_secs(i64::MAX as u64),
    ]
    .into_iter()
    .enumerate()
    {
        let marker = root.join(format!("started-{index}"));
        let command = TestCommand::new("touch", [marker.as_os_str()]).with_timeout(timeout);
        let answer = UnconfinedRunner.run(&command, &root);
        let error = answer.expect_err("a timeout the clock cannot hold is refused");
        assert!(
            error.detail.contains(&format!("{timeout:?}")),
            "the refusal names the timeout {timeout:?}: {error}"
        );
        assert!(!marker.exists(), "{timeout:?}: the test command started");
    }
}

/// A timeout the clock can hold, however large, runs the command to its end: the deadline and
/// every wait derived from it are taken without a panic.
#[test]
fn security_a_test_command_with_a_large_holdable_timeout_runs_to_its_end() {
    let root = scratch("test_command_large_holdable");
    let marker = root.join("started");
    let command =
        TestCommand::new("touch", [marker.as_os_str()]).with_timeout(Duration::from_secs(HOLDABLE));
    let execution = UnconfinedRunner
        .run(&command, &root)
        .expect("a timeout the clock holds runs the command");
    assert_eq!(execution.exit_code, Some(0), "{}", execution.output);
    assert!(!execution.timed_out);
    assert!(marker.exists(), "the test command did not run");
}

/// A zero timeout is no panic: the command is ended at once and reported timed out.
#[test]
fn security_a_zero_test_command_timeout_ends_the_command_without_a_panic() {
    let root = scratch("test_command_zero");
    let command = TestCommand::new("sleep", ["30"]).with_timeout(Duration::ZERO);
    let started = Instant::now();
    let execution = UnconfinedRunner
        .run(&command, &root)
        .expect("a zero timeout ends the command");
    assert!(execution.timed_out, "{}", execution.output);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the command ran past a zero timeout: {:?}",
        started.elapsed()
    );
}

// ---------------------------------------------------------------------------------------------
// The bound command (`bindings::CommandEffects`), through the library path an SDK host takes.

const INTENT: &str = "record a note that the deploy finished";
const NAME: &str = "team-notes@1";

const PROTOCOL: &str = r"format: protocol/1
protocol:
  id: team.notes
  revision: 1
  description: Record a team note.
artifacts:
  intent:
    description: the request
evidence_kinds:
  note_recorded:
    description: the note was recorded
claims:
  note.recorded:
    true_when:
      evidence:
        kind: note_recorded
        result: recorded
        subject: intent
actions:
  note.record:
    description: record the note
    effect: write
    may_produce:
      - evidence: note_recorded
outcomes:
  done:
    description: the note is recorded
    requires:
      all:
        - claim: note.recorded
";

/// Runs `team-notes@1` once with `note.record` bound to a script that writes a marker, under
/// `timeout_seconds`; returns what the run printed and whether the script ran.
fn bound_run(test: &str, timeout_seconds: i64) -> (String, bool) {
    let root = scratch(test);
    let source = root.join("notes.yaml");
    std::fs::write(&source, PROTOCOL).unwrap();
    let store = InstallStore::new(root.join("installed"));
    store.install_file(NAME, &source, false).unwrap();
    let catalog: ProtocolCatalog = store.catalog().unwrap();
    let marker = root.join("started");
    let script = root.join("record-note");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\n: > '{}'\necho '{{\"note\":\"n-1\"}}'\n",
            marker.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog)
        .unwrap();
    let router = Recorded::new(vec![(
        "pick_protocol",
        json!({"protocol": NAME, "confidence": 0.99, "reasons": ["a team note"]}),
    )]);
    let agent = Recorded::new(vec![
        ("select_action", json!({"action": "note.record"})),
        ("action_arguments", json!({"text": "deploy finished"})),
    ]);
    let request = IntentRequest {
        intent: INTENT.into(),
        workspace: None,
        runner: None,
        test: TestCommand::new("", std::iter::empty::<&str>()),
        max_steps: 1,
        threshold: 0.5,
        inputs: b10x_loom_intake_slice::case::RUN_DEFAULTS,
        test_paths: b10x_loom_intake_slice::test_paths::TestPaths::default(),
        bindings: Some(ActionBindings {
            protocol: NAME.into(),
            timeout_seconds: Some(timeout_seconds),
            unconfined_bindings: Some(true),
            bindings: vec![ActionBinding {
                action: "note.record".into(),
                argv: vec![script.display().to_string()],
            }],
        }),
    };
    let mut output = Vec::new();
    let result = run_intent_with_options(
        &request,
        &catalog,
        &governor,
        &governor,
        &router,
        &agent,
        &SystemClock,
        &mut output,
        &RunOptions {
            context_policy: ContextPolicy::Legacy,
            context_report: Some(root.join("metrics.json")),
        },
    );
    let output = String::from_utf8_lossy(&output).into_owned();
    result.unwrap_or_else(|error| panic!("{error}\n{output}"));
    (output, marker.exists())
}

/// A bound command whose timeout the clock can hold, however large, runs: the deadline taken
/// before it starts and the waits derived from it are no panic and no refusal.
#[test]
fn security_a_bound_command_with_a_large_holdable_timeout_runs() {
    let seconds = i64::try_from(HOLDABLE).unwrap();
    let (output, ran) = bound_run("bound_large_holdable", seconds);
    assert!(ran, "the bound command did not run: {output}");
    assert!(
        !output.contains("past what this host's clock can hold"),
        "{output}"
    );
}

/// A model answering each forced tool call with the next recorded arguments.
struct Recorded {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<(&'static str, Value)>>,
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
                model: id("recorded-model"),
                binding_revision: id("recorded-1"),
            },
            capabilities: Capabilities {
                tools: true,
                tool_choice: true,
                ..Capabilities::text(128_000, 8192)
            },
            script: Mutex::new(script.into()),
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
        _sink: &'a mut dyn StreamSink,
        _cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        Box::pin(async move {
            let ToolChoice::Named(tool) = &request.tool_choice else {
                panic!("forced tool expected")
            };
            let (expected, arguments) = self
                .script
                .lock()
                .unwrap()
                .pop_front()
                .expect("no unrecorded model requests");
            assert_eq!(tool.as_str(), expected);
            Ok(TurnOutcome {
                stop_reason: TurnStop::ToolCalls,
                items: vec![Item::ToolCall(ToolCall {
                    call_id: CallId::new(format!("call-{expected}")).unwrap(),
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
