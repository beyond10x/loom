//! Adversary cases for `story:bound-command-process-group`: a bound command (`run
//! --action-bindings`) that times out is killed with every process it started.
//!
//! Each case drives an installed protocol through `run_intent_with_options` with recorded models;
//! no network and no credential. The bound command is a script in the case's temporary directory;
//! it names its programs by absolute path and appends the pid of every process it starts to a file
//! there. Every process still running when a case ends is killed by the case's guard, which drops
//! before the temporary directory holding the pid file, so none outlives the test run.

use std::collections::VecDeque;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
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
/// How long a case waits for a killed process to be gone: the kernel's reaping, never longer.
const GONE_WITHIN: Duration = Duration::from_secs(5);
/// How long past its one-second timeout a run may take to return.
const RETURNS_WITHIN: Duration = Duration::from_secs(4);
/// Set in the environment of a re-executed test binary: the case directory it runs in.
const CHILD_DIR: &str = "LOOM_ADVERSARY_W6_U3_DIR";

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

/// A bound command whose child outlives the timeout; the run's own `PATH` holds no `kill`
/// program, as an operator's minimal service `PATH` may not. The run reports the command killed;
/// the child it started must be gone as well, or the report is false.
#[test]
fn adversary_a_timed_out_command_leaves_no_process_when_no_kill_program_is_on_path() {
    if let Some(dir) = std::env::var_os(CHILD_DIR) {
        // The re-executed binary: run the case with the `PATH` the parent gave it.
        let output = run_case(Path::new(&dir));
        assert!(output.contains("was killed after 1 s"), "{output}");
        return;
    }
    let case = Case::new("/usr/bin/sleep 120 &\necho $! >> PIDS\nexec /usr/bin/sleep 120\n");
    let empty = case.dir.path().join("empty-path");
    std::fs::create_dir(&empty).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "adversary_a_timed_out_command_leaves_no_process_when_no_kill_program_is_on_path",
            "--exact",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env(CHILD_DIR, case.dir.path())
        .env("PATH", &empty)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    if child.try_wait().unwrap().is_none() {
        let _ = child.kill();
    }
    let out = child.wait_with_output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "the re-executed case failed:\n{text}");
    case.guard.assert_all_gone();
}

/// A chain three processes deep that ignores SIGTERM, under a command that outlives its timeout:
/// every process of the chain is gone, and the run returns within a bound.
#[test]
fn adversary_a_deep_chain_that_ignores_sigterm_is_gone_and_the_run_returns_in_bounded_time() {
    let case = Case::new(
        "trap '' TERM\n\
         ( ( /usr/bin/sleep 120 & echo $! >> PIDS; wait ) & echo $! >> PIDS; wait ) &\n\
         echo $! >> PIDS\n\
         exec /usr/bin/sleep 120\n",
    );
    let started = Instant::now();
    let output = run_case(case.dir.path());
    let took = started.elapsed();
    assert!(output.contains("was killed after 1 s"), "{output}");
    assert!(
        took < Duration::from_secs(1) + RETURNS_WITHIN,
        "the run took {took:?}"
    );
    case.guard.assert_all_gone();
}

/// A command that ends at once while the child it started holds only its standard error open:
/// the run reports the timeout within a bound and the child is gone.
#[test]
fn adversary_a_child_holding_only_stderr_is_gone_and_the_run_returns_in_bounded_time() {
    let case = Case::new(
        "/usr/bin/sleep 120 > /dev/null &\necho $! >> PIDS\necho '{\"note\":\"n-1\"}'\nexit 0\n",
    );
    let started = Instant::now();
    let output = run_case(case.dir.path());
    let took = started.elapsed();
    assert!(output.contains("was killed after 1 s"), "{output}");
    assert!(
        took < Duration::from_secs(1) + RETURNS_WITHIN,
        "the run took {took:?}"
    );
    case.guard.assert_all_gone();
}

/// One case directory: the installed protocol, the script and the pid file.
struct Case {
    /// First, so it is dropped (and every process still running killed) before `dir` removes the
    /// pid file that names them.
    guard: Guard,
    dir: tempfile::TempDir,
}

impl Case {
    fn new(body: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("notes.yaml");
        std::fs::write(&source, PROTOCOL).unwrap();
        let store = InstallStore::new(dir.path().join("installed"));
        store.install_file(NAME, &source, false).unwrap();
        let pids = dir.path().join("pids");
        let script = dir.path().join("record-note");
        let body = format!("#!/usr/bin/sh\necho $$ >> PIDS\n{body}")
            .replace("PIDS", &format!("'{}'", pids.display()));
        std::fs::write(&script, body).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            guard: Guard { pids },
            dir,
        }
    }
}

/// Runs the intent in case directory `dir` to its one step and returns what the run printed.
fn run_case(dir: &Path) -> String {
    let catalog: ProtocolCatalog = InstallStore::new(dir.join("installed")).catalog().unwrap();
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
            timeout_seconds: Some(1),
            unconfined_bindings: Some(true),
            bindings: vec![ActionBinding {
                action: "note.record".into(),
                argv: vec![dir.join("record-note").display().to_string()],
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
            context_report: Some(dir.join("metrics.json")),
        },
    );
    let output = String::from_utf8_lossy(&output).into_owned();
    let run = result.unwrap_or_else(|error| panic!("{error}\n{output}"));
    assert_eq!(run.steps, 1, "{output}");
    output
}

/// Every process the script started, named by the pids it appended to `pids`.
struct Guard {
    pids: PathBuf,
}

impl Guard {
    fn pids(&self) -> Vec<i32> {
        std::fs::read_to_string(&self.pids)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| line.trim().parse().ok())
            .collect()
    }

    /// Asserts every process the script started is gone, waiting at most [`GONE_WITHIN`].
    fn assert_all_gone(&self) {
        let pids = self.pids();
        assert!(pids.len() >= 2, "the script recorded {pids:?}");
        let deadline = Instant::now() + GONE_WITHIN;
        while pids.iter().any(|pid| running(*pid)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let left: Vec<i32> = pids.into_iter().filter(|pid| running(*pid)).collect();
        assert!(
            left.is_empty(),
            "processes the timed-out command started still run: {left:?}"
        );
    }

    /// Whether `pid` is still a process of this case: a `sleep` or the case's script.
    fn ours(&self, pid: i32) -> bool {
        let dir = self.pids.parent().unwrap().display().to_string();
        std::fs::read(proc(pid).join("cmdline")).is_ok_and(|line| {
            line.starts_with(b"/usr/bin/sleep\0")
                || String::from_utf8_lossy(&line).contains(dir.as_str())
        })
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        for pid in self.pids() {
            if running(pid) && self.ours(pid) {
                // SAFETY: `kill` takes no pointer; the pid was checked to be this case's process.
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                }
            }
        }
        let deadline = Instant::now() + GONE_WITHIN;
        while self.pids().iter().any(|pid| running(*pid)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// Whether `pid` is a process that has not ended: it exists and is no zombie.
fn running(pid: i32) -> bool {
    let Ok(stat) = std::fs::read_to_string(proc(pid).join("stat")) else {
        return false;
    };
    let state = stat
        .rsplit_once(')')
        .and_then(|(_, rest)| rest.trim_start().chars().next());
    !matches!(state, Some('Z' | 'X') | None)
}

fn proc(pid: i32) -> PathBuf {
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
