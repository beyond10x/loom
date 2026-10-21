//! Acceptance for `story:bound-command-process-group`: a command bound to an action
//! (`run --action-bindings`) that runs past its timeout is killed together with every process it
//! started, so nothing it forked keeps running after the run reports the timeout.
//!
//! Each case drives an installed protocol through `run_intent_with_options` with recorded models;
//! no network and no credential. The bound command is a script in the case's temporary directory
//! that forks a sleeping child and writes the child's pid to a file there. A child still alive when
//! a case ends is killed by the case's guard, so none outlives the test run.

use std::collections::VecDeque;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use b10x_loom_intake_slice::bindings::{ActionBinding, ActionBindings};
use b10x_loom_intake_slice::clock::SystemClock;
use b10x_loom_intake_slice::context::ContextPolicy;
use b10x_loom_intake_slice::executor::TestCommand;
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

const INTENT: &str = "record a note that the deploy finished";
const NAME: &str = "team-notes@1";
/// How long a case waits for a killed child to be gone: the kernel's reaping, never longer.
const GONE_WITHIN: Duration = Duration::from_secs(5);

/// One action, `note.record`; the case is done once a note is recorded.
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

#[test]
fn a_timed_out_command_leaves_no_process_it_started() {
    // The command forks a sleeping child and outlives its one-second timeout itself.
    let case = Case::new("#!/bin/sh\nsleep 120 &\necho $! > \"$PIDFILE\"\nexec sleep 120\n");
    let output = case.run();
    assert!(output.contains("was killed after 1 s"), "{output}");
    case.child.assert_gone();
}

#[test]
fn a_command_that_ends_while_its_child_holds_its_output_leaves_no_process_at_the_timeout() {
    // The command ends at once, but the child it forked keeps its standard output open past the
    // timeout, so the run reports the timeout with the command itself already reaped.
    let case = Case::new(
        "#!/bin/sh\nsleep 120 &\necho $! > \"$PIDFILE\"\necho '{\"note\":\"n-1\"}'\nexit 0\n",
    );
    let output = case.run();
    assert!(output.contains("was killed after 1 s"), "{output}");
    case.child.assert_gone();
}

/// One run of `team-notes@1` whose `note.record` is bound to a script.
struct Case {
    /// First, so it is dropped (and a child still running killed) before `dir` removes the pid
    /// file that names the child.
    child: Child,
    dir: tempfile::TempDir,
    catalog: ProtocolCatalog,
    script: PathBuf,
}

impl Case {
    fn new(body: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("notes.yaml");
        std::fs::write(&source, PROTOCOL).unwrap();
        let store = InstallStore::new(dir.path().join("installed"));
        store.install_file(NAME, &source, false).unwrap();
        let catalog = store.catalog().unwrap();
        // The command's environment holds `PATH` alone, so the pid file's path is in the script.
        let pidfile = dir.path().join("child.pid");
        let script = dir.path().join("record-note");
        let body = body.replace("\"$PIDFILE\"", &format!("'{}'", pidfile.display()));
        std::fs::write(&script, body).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            child: Child { pidfile },
            dir,
            catalog,
            script,
        }
    }

    /// Runs the intent to its one step and returns what the run printed.
    fn run(&self) -> String {
        let governor = CanonGovernor::new(MemoryCaseStore::default())
            .with_catalog(&self.catalog)
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
                timeout_seconds: Some(1),
                unconfined_bindings: Some(true),
                bindings: vec![ActionBinding {
                    action: "note.record".into(),
                    argv: vec![self.script.display().to_string()],
                }],
            }),
        };
        let mut output = Vec::new();
        let result = run_intent_with_options(
            &request,
            &self.catalog,
            &governor,
            &governor,
            &router,
            &agent,
            &SystemClock,
            &mut output,
            &RunOptions {
                context_policy: ContextPolicy::Legacy,
                context_report: Some(self.dir.path().join("metrics.json")),
            },
        );
        let output = String::from_utf8_lossy(&output).into_owned();
        let run = result.unwrap_or_else(|error| panic!("{error}\n{output}"));
        assert_eq!(run.steps, 1, "{output}");
        output
    }
}

/// The child the bound command forked, named by the pid it wrote.
struct Child {
    pidfile: PathBuf,
}

impl Child {
    fn pid(&self) -> Option<u32> {
        std::fs::read_to_string(&self.pidfile)
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    /// Asserts the child is gone, waiting at most [`GONE_WITHIN`] for it to be reaped.
    fn assert_gone(&self) {
        let pid = self.pid().expect("the command wrote its child's pid");
        let deadline = Instant::now() + GONE_WITHIN;
        while running(pid) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            !running(pid),
            "the child {pid} the command started still runs after its timeout"
        );
    }
}

impl Drop for Child {
    /// Kills the child when a case ends with it still running, so it never outlives the test run.
    fn drop(&mut self) {
        if let Some(pid) = self.pid()
            && running(pid)
            && is_sleep(pid)
        {
            let _ = std::process::Command::new("kill")
                .args(["-s", "KILL", "--", &pid.to_string()])
                .status();
            let deadline = Instant::now() + GONE_WITHIN;
            while running(pid) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// Whether `pid` is a process that has not ended: it exists and is no zombie.
fn running(pid: u32) -> bool {
    let Ok(stat) = std::fs::read_to_string(proc(pid).join("stat")) else {
        return false;
    };
    let state = stat
        .rsplit_once(')')
        .and_then(|(_, rest)| rest.trim_start().chars().next());
    !matches!(state, Some('Z' | 'X') | None)
}

/// Whether `pid` is still the `sleep` the script forked, never a process that reused its pid.
fn is_sleep(pid: u32) -> bool {
    std::fs::read(proc(pid).join("cmdline")).is_ok_and(|line| line.starts_with(b"sleep\0"))
}

fn proc(pid: u32) -> PathBuf {
    Path::new("/proc").join(pid.to_string())
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
