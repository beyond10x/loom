//! Security review of `story:bound-commands-run-confined` (wave 2026-11-21-w6, unit U1):
//! integrity conformance checks of the change's own contract, driven through the public
//! `run_intent_with_options` with recorded models (no network, no credential, no model call
//! beyond the recording).
//!
//! - A bound command's timeout holds under `local` whatever the size of its arguments: the step
//!   returns within the timeout and one kill settle, as it does for small arguments.
//! - Model-chosen arguments reach the program on standard input only, never in its argv.
//! - A host runner on `linux` that cannot be served is refused before any model call and never
//!   falls back to running the command unconfined.

use std::collections::VecDeque;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use b10x_loom_intake_slice::bindings::{ActionBinding, ActionBindings, UNCONFINED_FLAG_REASON};
use b10x_loom_intake_slice::clock::SystemClock;
use b10x_loom_intake_slice::confinement::{
    ConfinementAdapter, EngineConfinement, EngineRunner, KillBounds, SIGNAL_GRACE, SubstrateRunner,
    TestRunner,
};
use b10x_loom_intake_slice::context::ContextPolicy;
use b10x_loom_intake_slice::executor::TestCommand;
use b10x_loom_intake_slice::intent::{IntentRequest, run_intent_with_options};
use b10x_loom_intake_slice::run::{RunOptions, SliceError, SliceRun, StopReason};
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

/// The `local` engine's wait after its deadline's `Kill` in the case below: 1 s, in place of a
/// host's [`b10x_loom_intake_slice::confinement::KILL_SETTLE`] (5 s).
const KILL_SETTLE: Duration = Duration::from_secs(1);

/// The runner `--unconfined-bindings` gives (`UNCONFINED_FLAG_REASON`), which the run takes before
/// the file's opt-in, with [`KILL_SETTLE`] as its kill settle.
fn short_settle_runner() -> Arc<dyn TestRunner> {
    let runner = EngineRunner::open(&EngineConfinement {
        adapter: ConfinementAdapter::Local,
        local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
        cgroup_root: None,
    })
    .expect("local opens with the flag's reason")
    .with_kill_bounds(KillBounds {
        signal_grace: SIGNAL_GRACE,
        kill_settle: KILL_SETTLE,
    });
    Arc::new(runner)
}

/// Under `local` (the flag's runner, beside the file's opt-in), a bound command that leaves a
/// process in a session of its own holding its standard input and output is refused at its
/// timeout plus one kill settle when its arguments are small (`bound_command_confined.rs`,
/// `under_local_a_command_whose_output_outlives_its_group_is_refused_by_its_runner_never_killed`).
/// The same command given arguments larger than one pipe buffer must return in the same bound:
/// the timeout is the operator's limit on the step, and the size of the model's arguments must
/// not lift it. The settle is 1 s here ([`KILL_SETTLE`]), 5 s on a host.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn under_local_the_timeout_holds_when_the_arguments_exceed_one_pipe_buffer() {
    let mut case = Case::new("timeout_large_arguments");
    case.timeout_seconds = 1;
    let held = Held(case.root.join("held"));
    // The held process keeps the shell's standard input (fd 3 duplicates it, since an
    // asynchronous list's standard input is otherwise /dev/null) and its output, and reads
    // nothing; the program itself ends at once without reading its input.
    let script = case.script(
        "record-note",
        &format!(
            "exec 3<&0\n/usr/bin/setsid /usr/bin/sleep 20 <&3 &\necho $! > '{}'\nexit 0\n",
            held.0.display()
        ),
    );
    // Within the model's argument bound (`llm_core::MAX_TOOL_ARGUMENT_BYTES`, 64 KiB, counted as
    // serde_json writes `\b`, two bytes), and three times that on standard input, where Loom's
    // `json_text` writes each backspace as `\u0008`, six bytes: past one 64 KiB pipe buffer.
    let large = "\u{8}".repeat(30_000);
    let started = Instant::now();
    let (result, output) = case.run_opted(
        Some(short_settle_runner()),
        Some(true),
        &script,
        &[],
        pick(),
        select_with(json!({ "text": large })),
    );
    let took = started.elapsed();
    let run = result.unwrap_or_else(|error| panic!("{error}\n{}", tail(&output)));
    assert_eq!(run.steps, 1, "{}", tail(&output));
    let bound = Duration::from_secs(1) + KILL_SETTLE + Duration::from_secs(6);
    assert!(
        took < bound,
        "the step took {took:?} for a timeout of 1 s (bound {bound:?}): the run waited for a process \
         outside the command's group to let go of its standard input\n{}",
        tail(&output)
    );
}

/// Under `local`, a program that ends by itself without reading arguments larger than one pipe
/// buffer ends as itself: the pending write is dropped once the run ended, and the step is the
/// program's own outcome (its report, and its exit status when it is not 0), never a refusal of
/// its runner.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn under_local_a_program_that_reads_none_of_its_large_arguments_ends_as_itself() {
    let large = "\u{8}".repeat(30_000);
    let case = Case::new("unread_large_arguments");
    let script = case.script("record-note", "printf '{\"note\":\"n-1\"}\\n'\nexit 0\n");
    let (result, output) = case.run(&script, &[], pick(), select_with(json!({ "text": large })));
    let run = result.unwrap_or_else(|error| panic!("{error}\n{}", tail(&output)));
    assert_eq!(run.steps, 1, "{}", tail(&output));
    assert_eq!(effect(&output), json!({"note": "n-1"}), "{}", tail(&output));
    let case = Case::new("unread_large_arguments_failing");
    let script = case.script("record-note", "echo failed-on-purpose >&2\nexit 3\n");
    let (result, output) = case.run(&script, &[], pick(), select_with(json!({ "text": large })));
    let run = result.unwrap_or_else(|error| panic!("{error}\n{}", tail(&output)));
    assert_eq!(run.steps, 1, "{}", tail(&output));
    assert!(
        output.contains(&format!(
            "{} ended exit status: 3: failed-on-purpose",
            script.display()
        )),
        "{}",
        tail(&output)
    );
    assert!(
        !output.contains("refused by its runner"),
        "{}",
        tail(&output)
    );
}

/// The model chooses the arguments; only the binding's own argv reaches the program's argv.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_bound_programs_argv_is_the_bindings_alone() {
    let case = Case::new("argv_is_the_bindings");
    let script = case.script(
        "record-note",
        "/usr/bin/cat > /dev/null\n\
         printf '{\"count\":%s,\"first\":\"%s\",\"second\":\"%s\"}\\n' \"$#\" \"$1\" \"$2\"\n",
    );
    let (result, output) = case.run(
        &script,
        &["fixed-argument"],
        pick(),
        select_with(json!({"text": "--model-chosen", "argv": ["injected"]})),
    );
    let run = result.unwrap_or_else(|error| panic!("{error}\n{output}"));
    assert_eq!(run.steps, 1, "{output}");
    let report = effect(&output);
    assert_eq!(report["count"], json!(1), "{output}");
    assert_eq!(report["first"], json!("fixed-argument"), "{output}");
    assert_eq!(report["second"], json!(""), "{output}");
}

/// A host runner on `linux` whose cgroup root does not exist cannot be served: the run stops
/// before any model call, no command starts, and nothing runs it unconfined instead.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_linux_runner_that_cannot_be_served_is_refused_and_never_falls_back_to_local() {
    let case = Case::new("linux_unserved");
    let marker = case.root.join("started");
    let script = case.script(
        "record-note",
        &format!(
            ": > '{}'\nprintf '{{\"note\":\"n-1\"}}\\n'\n",
            marker.display()
        ),
    );
    let runner = SubstrateRunner::new(Some(case.root.join("no-such-cgroup-root")));
    let (result, output) = case.run_with(
        Some(Arc::new(runner) as Arc<dyn TestRunner>),
        &script,
        &[],
        Vec::new(),
        Vec::new(),
    );
    let run = result.unwrap_or_else(|error| panic!("{error}\n{output}"));
    assert_eq!(
        run.stop_reason,
        StopReason::ConfinementUnavailable,
        "{output}"
    );
    assert_eq!(run.steps, 0, "{output}");
    assert!(!marker.exists(), "the command started: {output}");
    assert!(!output.contains("confinement: local"), "{output}");
}

// ---------------------------------------------------------------------------------------------
// Fixtures

/// Kills the process whose id the file at `.0` holds, when dropped.
struct Held(PathBuf);

impl Drop for Held {
    fn drop(&mut self) {
        if let Ok(pid) = std::fs::read_to_string(&self.0)
            && let Ok(pid) = pid.trim().parse::<i32>()
        {
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
    }
}

struct Case {
    root: PathBuf,
    catalog: ProtocolCatalog,
    timeout_seconds: i64,
}

impl Case {
    fn new(label: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("security_w20261121w6_u1")
            .join(format!("{label}-{}", std::process::id()));
        if root.exists() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("notes.yaml");
        std::fs::write(&source, PROTOCOL).unwrap();
        let store = InstallStore::new(root.join("installed"));
        store.install_file(NAME, &source, false).unwrap();
        let catalog = store.catalog().unwrap();
        Self {
            root,
            catalog,
            timeout_seconds: 30,
        }
    }

    fn script(&self, name: &str, body: &str) -> PathBuf {
        let path = self.root.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// A run with the file's opt-in (`unconfined_bindings: true`) and no host runner.
    fn run(
        &self,
        script: &Path,
        args: &[&str],
        router: Vec<(&'static str, Value)>,
        agent: Vec<(&'static str, Value)>,
    ) -> (Result<SliceRun, SliceError>, String) {
        self.run_opted(None, Some(true), script, args, router, agent)
    }

    /// A run with no opt-in and the host runner `runner`.
    fn run_with(
        &self,
        runner: Option<Arc<dyn TestRunner>>,
        script: &Path,
        args: &[&str],
        router: Vec<(&'static str, Value)>,
        agent: Vec<(&'static str, Value)>,
    ) -> (Result<SliceRun, SliceError>, String) {
        self.run_opted(runner, None, script, args, router, agent)
    }

    fn run_opted(
        &self,
        runner: Option<Arc<dyn TestRunner>>,
        unconfined_bindings: Option<bool>,
        script: &Path,
        args: &[&str],
        router: Vec<(&'static str, Value)>,
        agent: Vec<(&'static str, Value)>,
    ) -> (Result<SliceRun, SliceError>, String) {
        let governor = CanonGovernor::new(MemoryCaseStore::default())
            .with_catalog(&self.catalog)
            .unwrap();
        let router = Recorded::new(router);
        let agent = Recorded::new(agent);
        let mut argv = vec![script.display().to_string()];
        argv.extend(args.iter().map(|arg| (*arg).to_owned()));
        let request = IntentRequest {
            intent: INTENT.into(),
            workspace: None,
            runner,
            test: TestCommand::new("", std::iter::empty::<&str>()),
            max_steps: 1,
            threshold: 0.5,
            inputs: b10x_loom_intake_slice::case::RUN_DEFAULTS,
            test_paths: b10x_loom_intake_slice::test_paths::TestPaths::default(),
            bindings: Some(ActionBindings {
                protocol: NAME.into(),
                timeout_seconds: Some(self.timeout_seconds),
                unconfined_bindings,
                bindings: vec![ActionBinding {
                    action: "note.record".into(),
                    argv,
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
                context_report: None,
                ..RunOptions::default()
            },
        );
        (result, String::from_utf8_lossy(&output).into_owned())
    }
}

/// The run's output with each line cut to 300 characters, so a long argument does not flood the
/// failure message.
fn tail(output: &str) -> String {
    output
        .lines()
        .map(|line| line.chars().take(300).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn pick() -> Vec<(&'static str, Value)> {
    vec![(
        "pick_protocol",
        json!({"protocol": NAME, "confidence": 0.99, "reasons": ["a team note"]}),
    )]
}

fn select_with(arguments: Value) -> Vec<(&'static str, Value)> {
    vec![
        ("select_action", json!({"action": "note.record"})),
        ("action_arguments", arguments),
    ]
}

fn effect(output: &str) -> Value {
    let line = output
        .lines()
        .find_map(|line| line.trim_start().strip_prefix("effect: "))
        .unwrap_or_else(|| panic!("no effect line: {output}"));
    serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}: {line}\n{output}"))
}

/// A model answering each forced tool call with the next recorded arguments; a call past the
/// recording panics.
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
                .unwrap_or_else(|| panic!("a model call no case recorded: {tool}"));
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
