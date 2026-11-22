//! Acceptance for `story:bound-commands-run-confined`: a command bound to a protocol's action
//! starts only through the runner the host injected. `bindings::CommandEffects` builds a child
//! command (argv, the arguments as JSON on standard input, the timeout, an environment of `PATH`
//! alone) and hands it to the runner; the library starts no process of its own. A host that passes
//! bindings and no runner is refused `NoRunner` before any process starts and before any model
//! call. The operator's opt-in selects Substrate's `local` adapter with a reason naming which
//! opt-in it was: the flag's runner, or the file's `unconfined_bindings: true`.
//!
//! Each case drives an installed protocol through `run_intent_with_options` with recorded models;
//! no network, no credential and no model call beyond the recording. The `local` cases run real
//! host processes; the `linux` case runs in a delegated scope through `tests/support` and skips
//! naming what is missing. Scratch goes under `CARGO_TARGET_TMPDIR`, which lies outside every
//! mount Substrate owns, so a bound script there can be mounted read-only at its own path.

#[path = "support/mod.rs"]
mod support;

use std::collections::VecDeque;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use b10x_loom_intake_slice::bindings::{ActionBinding, ActionBindings, bound_program_child};
use b10x_loom_intake_slice::clock::SystemClock;
use b10x_loom_intake_slice::confinement::{
    AppliedConfinement, Backend, ChildCommand, ChildOutput, ConfinementAdapter, ConfinementError,
    ConfinementRefusal, EngineConfinement, EngineRunner, KillBounds, SIGNAL_GRACE, TestExecution,
    TestRunner,
};
use b10x_loom_intake_slice::context::ContextPolicy;
use b10x_loom_intake_slice::executor::{TestCommand, UnconfinedRunner};
use b10x_loom_intake_slice::intent::{IntentRequest, run_intent_with_options};
use b10x_loom_intake_slice::run::{RunOptions, StopReason};
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

/// The reason the `local` record carries when the run's `--unconfined-bindings` flag opted in.
const FLAG_REASON: &str =
    "--unconfined-bindings: the operator runs the bound commands unconfined, as the operator";
/// The reason the `local` record carries when the file's `unconfined_bindings: true` opted in.
const FILE_REASON: &str = "unconfined_bindings: true in the run configuration file: the operator runs the bound commands unconfined, as the operator";

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

// ---------------------------------------------------------------------------------------------
// The library starts no process of its own

#[test]
fn bindings_starts_no_process_of_its_own() {
    // `ChildCommand::new` builds the command the runner starts; it starts nothing.
    let source = include_str!("../src/bindings.rs").replace("ChildCommand::new", "ChildCommand");
    for spawn in ["Command::new", "process::Command", ".spawn()"] {
        assert!(
            !source.contains(spawn),
            "src/bindings.rs names `{spawn}`: a bound command starts only through the runner"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// No runner

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn bindings_and_no_runner_are_refused_no_runner_before_any_process_or_model_call() {
    for (label, runner, unconfined) in [
        ("no-runner", None, None),
        ("no-runner-file-false", None, Some(false)),
        (
            "unconfined-runner",
            Some(Arc::new(UnconfinedRunner) as Arc<dyn TestRunner>),
            None,
        ),
    ] {
        let case = Case::new(&format!("no_runner_{label}"));
        let script = case.marking_script();
        // No model answer is recorded: a model call panics the case.
        let (result, output) = case.run(runner, unconfined, &script, Vec::new(), Vec::new());
        let run = result.unwrap_or_else(|error| panic!("{label}: {error}\n{output}"));
        assert_eq!(
            run.stop_reason,
            StopReason::ConfinementUnavailable,
            "{label}: {output}"
        );
        assert!(output.contains("NoRunner"), "{label}: {output}");
        assert_eq!(run.steps, 0, "{label}: {output}");
        assert!(!case.marker().exists(), "{label}: the command started");
    }
}

// ---------------------------------------------------------------------------------------------
// What the program receives

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_bound_program_receives_the_arguments_alone_on_stdin_and_path_alone_in_its_environment() {
    let case = Case::new("stdin_and_environment");
    // The environment the program was started with, read from its own `/proc` entry before the
    // shell adds anything, and its standard input as received.
    let script = case.script(
        "record-note",
        "input=$(/usr/bin/cat)\n\
         environ=$(/usr/bin/tr '\\000' '\\n' < /proc/$$/environ | /usr/bin/base64 -w0)\n\
         printf '{\"stdin\":%s,\"environ\":\"%s\"}\\n' \"$input\" \"$environ\"\n",
    );
    let (result, output) = case.run(
        None,
        Some(true),
        &script,
        pick(),
        select_with(json!({"text": "deploy finished", "tags": ["a", "b"]})),
    );
    let run = result.unwrap_or_else(|error| panic!("{error}\n{output}"));
    assert_eq!(run.steps, 1, "{output}");
    let report = effect(&output);
    assert_eq!(
        report["stdin"],
        json!({"text": "deploy finished", "tags": ["a", "b"]}),
        "{output}"
    );
    let environ = decode_base64(report["environ"].as_str().expect("the environment"));
    let path = std::env::var("PATH").expect("the test runs with a PATH");
    assert_eq!(environ, format!("PATH={path}\n"), "{output}");
}

// ---------------------------------------------------------------------------------------------
// Which opt-in selected `local`

/// The reasons a host opts in with are the library's own: the command line opens its flag's
/// runner with `UNCONFINED_FLAG_REASON`, and a file's opt-in can give no other reason than
/// `UNCONFINED_FILE_REASON`, the file holding no reason of its own.
#[test]
fn the_two_reasons_are_the_librarys_and_a_file_holds_no_reason() {
    assert_eq!(
        b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON,
        FLAG_REASON
    );
    assert_eq!(
        b10x_loom_intake_slice::bindings::UNCONFINED_FILE_REASON,
        FILE_REASON
    );
    let error = b10x_loom_intake_slice::bindings::decode(&format!(
        r#"{{"protocol":"{NAME}","unconfined_bindings":true,"local_reason":"{FLAG_REASON}","bindings":[]}}"#
    ))
    .expect_err("a file naming a reason is refused");
    assert!(error.contains("local_reason"), "{error}");
}

/// A host runner on `local` for a reason that is no opt-in's starts no bound command.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_host_runner_on_local_for_another_reason_is_refused_command_unconfined() {
    let case = Case::new("other_local_reason");
    let script = case.marking_script();
    let runner = EngineRunner::open(&EngineConfinement {
        adapter: ConfinementAdapter::Local,
        local_reason: Some("the test commands run on the host".to_owned()),
        cgroup_root: None,
    })
    .expect("local opens with a reason");
    let (result, output) = case.run(
        Some(Arc::new(runner) as Arc<dyn TestRunner>),
        None,
        &script,
        Vec::new(),
        Vec::new(),
    );
    let run = result.unwrap_or_else(|error| panic!("{error}\n{output}"));
    assert_eq!(
        run.stop_reason,
        StopReason::ConfinementUnavailable,
        "{output}"
    );
    assert!(output.contains("CommandUnconfined"), "{output}");
    assert!(!case.marker().exists(), "the command started: {output}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_file_member_selects_local_with_the_files_reason() {
    let case = Case::new("file_reason");
    let script = case.marking_script();
    let (result, output) = case.run(None, Some(true), &script, pick(), select());
    result.unwrap_or_else(|error| panic!("{error}\n{output}"));
    assert!(case.marker().exists(), "{output}");
    assert!(
        output.contains(&format!("confinement: local ({FILE_REASON})")),
        "{output}"
    );
    assert!(!output.contains(FLAG_REASON), "{output}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_flags_runner_selects_local_with_the_flags_reason_and_wins_over_the_file() {
    for in_file in [None, Some(true)] {
        let case = Case::new(&format!("flag_reason_{in_file:?}"));
        let script = case.marking_script();
        let runner = EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(FLAG_REASON.to_owned()),
            cgroup_root: None,
        })
        .expect("local opens with the flag's reason");
        let (result, output) = case.run(
            Some(Arc::new(runner) as Arc<dyn TestRunner>),
            in_file,
            &script,
            pick(),
            select(),
        );
        result.unwrap_or_else(|error| panic!("{in_file:?}: {error}\n{output}"));
        assert!(case.marker().exists(), "{in_file:?}: {output}");
        assert!(
            output.contains(&format!("confinement: local ({FLAG_REASON})")),
            "{in_file:?}: {output}"
        );
        assert!(!output.contains(FILE_REASON), "{in_file:?}: {output}");
    }
}

/// Under `local` a command that ends while a process it started in a session of its own holds its
/// output is sent one `Kill` at its timeout and is then refused by its runner, named so: the step
/// never says it was killed. The runner is the one `--unconfined-bindings` gives, which the run
/// takes before the file's opt-in, with a kill settle of 1 s in place of a host's 5 s.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn under_local_a_command_whose_output_outlives_its_group_is_refused_by_its_runner_never_killed() {
    let mut case = Case::new("held_output");
    case.timeout_seconds = 1;
    let held = Held(case.root.join("held"));
    let script = case.script(
        "record-note",
        &format!(
            "/usr/bin/setsid /usr/bin/sleep 30 &\necho $! > '{}'\nexit 0\n",
            held.0.display()
        ),
    );
    let runner = EngineRunner::open(&EngineConfinement {
        adapter: ConfinementAdapter::Local,
        local_reason: Some(b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON.to_owned()),
        cgroup_root: None,
    })
    .expect("local opens with the flag's reason")
    .with_kill_bounds(KillBounds {
        signal_grace: SIGNAL_GRACE,
        kill_settle: Duration::from_secs(1),
    });
    let (result, output) = case.run(
        Some(Arc::new(runner) as Arc<dyn TestRunner>),
        Some(true),
        &script,
        pick(),
        select(),
    );
    let run = result.unwrap_or_else(|error| panic!("{error}\n{output}"));
    assert_eq!(run.steps, 1, "{output}");
    assert!(
        output.contains(&format!(
            "{} was refused by its runner: CapabilityUnserved: ",
            script.display()
        )),
        "{output}"
    );
    assert!(!output.contains("was killed"), "{output}");
}

/// A runner that ran the command and reports neither its exit status nor a timeout: the step is
/// refused naming the lost status, never reported killed.
#[test]
fn a_command_whose_status_its_runner_could_not_read_is_refused_never_killed() {
    let case = Case::new("lost_status");
    let script = case.marking_script();
    let lost = Arc::new(Answering(TestExecution {
        exit_code: None,
        timed_out: false,
        output: String::new(),
        applied: AppliedConfinement {
            backend: Backend::Substrate,
            profile_digest: String::new(),
            capability_snapshot: String::new(),
            cgroup: String::new(),
            filesystem: "scoped".to_owned(),
            network: None,
            profile: "bindings".to_owned(),
            writable_scopes: Vec::new(),
            read_only_roots: Vec::new(),
            read_only_files: Vec::new(),
            workdir: None,
            adapter: None,
            facts: None,
            local_reason: None,
        },
        driver_record: None,
        child_output: ChildOutput {
            stdout: Vec::new(),
            stderr: Vec::new(),
            stdout_truncated: false,
            stderr_truncated: false,
        },
    }));
    let (result, output) = case.run(
        Some(lost as Arc<dyn TestRunner>),
        None,
        &script,
        pick(),
        select(),
    );
    let run = result.unwrap_or_else(|error| panic!("{error}\n{output}"));
    assert_eq!(run.steps, 1, "{output}");
    assert!(
        output.contains(&format!("{}'s status could not be read", script.display())),
        "{output}"
    );
    assert!(!output.contains("killed"), "{output}");
}

/// A host runner that answers every bound command with the execution it holds, running nothing.
struct Answering(TestExecution);

impl TestRunner for Answering {
    fn run(&self, _: &TestCommand, _: &Path) -> Result<TestExecution, ConfinementError> {
        Err(ConfinementError {
            reason: ConfinementRefusal::CapabilityUnserved,
            detail: "the fixture runs no test command".to_owned(),
        })
    }

    fn backend(&self) -> Backend {
        Backend::Substrate
    }

    fn run_child(&self, _: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        Ok(self.0.clone())
    }

    fn preflight_child(&self, _: &Path) -> Result<(), ConfinementError> {
        Ok(())
    }
}

/// Kills the process whose id the file at `.0` holds, when dropped.
struct Held(PathBuf);

impl Drop for Held {
    fn drop(&mut self) {
        if let Ok(pid) = std::fs::read_to_string(&self.0) {
            let _ = std::process::Command::new("/usr/bin/kill")
                .args(["-KILL", pid.trim()])
                .status();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Under `linux`

/// The text of the host file a confined command must not read.
const SECRET: &str = "the-host-file-outside-the-workspace";

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "Substrate's linux adapter serves Linux only"
)]
fn under_linux_a_bound_command_reads_no_host_file_and_writes_its_workspace() {
    // Gated: runs in a delegated scope, or skips naming what is missing.
    support::delegated(
        "under_linux_a_bound_command_reads_no_host_file_and_writes_its_workspace",
        &[],
        |runner, workspace| {
            let runner: Arc<dyn TestRunner> = Arc::new(runner.clone());

            // A host file outside the workspace and outside every read-only root.
            let case = Case::new("linux_reads");
            let secret = case.root.join("secret.txt");
            std::fs::write(&secret, SECRET).unwrap();
            let script = case.script(
                "record-note",
                &format!(
                    "set -e\nsecret=$(/usr/bin/cat '{}')\nprintf '{{\"secret\":\"%s\"}}\\n' \"$secret\"\n",
                    secret.display()
                ),
            );
            let (result, output) = case.run_in(
                Some(Arc::clone(&runner)),
                None,
                Some(workspace),
                &script,
                pick(),
                select(),
            );
            let run = result.unwrap_or_else(|error| panic!("{error}\n{output}"));
            assert_eq!(run.steps, 1, "{output}");
            assert!(!output.contains(SECRET), "the host file was read: {output}");
            assert!(output.contains("effect: refused: "), "{output}");
            assert!(output.contains("confinement: substrate"), "{output}");

            // A write inside the workspace.
            let case = Case::new("linux_writes");
            let script = case.script(
                "record-note",
                "printf written > /workspace/target/written\n\
                 printf '{\"note\":\"n-1\"}\\n'\n",
            );
            let (result, output) = case.run_in(
                Some(Arc::clone(&runner)),
                None,
                Some(workspace),
                &script,
                pick(),
                select(),
            );
            let run = result.unwrap_or_else(|error| panic!("{error}\n{output}"));
            assert_eq!(run.steps, 1, "{output}");
            assert!(output.contains(r#"effect: {"note":"n-1"}"#), "{output}");
            assert!(output.contains("confinement: substrate"), "{output}");
            assert_eq!(
                std::fs::read_to_string(workspace.join("target/written")).ok(),
                Some("written".to_owned()),
                "{output}"
            );
        },
    );
}

/// The program that moves a file's blocks (`EXT4_IOC_MOVE_EXT`).
const E4DEFRAG: &str = "/usr/bin/e4defrag";

/// story:substrate-0-24-0-pin, the ioctl floor of Substrate 0.24.0 (its ADR 0050): a confined
/// bound program cannot move the blocks of a file in its ext4 workspace, and the same call on the
/// same file without confinement can.
///
/// The confined program writes a 1 MiB file in 16 pieces, interleaved with a second file and each
/// synced, so the file holds several extents and `e4defrag` calls `EXT4_IOC_MOVE_EXT` (a file of
/// one extent is left alone). The seccomp program answers it `EACCES`: `e4defrag -v` names the
/// failed call and exits 1. e4defrag 1.47.4 prints the call's `errno` after it was overwritten
/// (`ioctl:Success`), so the case reads the refusal, not `Permission denied`.
///
/// The control: the same program, without writing the file again, runs `e4defrag -v` on the same
/// file through Substrate's `local` adapter, which installs no seccomp program. There it exits 0,
/// `[ OK ]`, and moves the file from more extents to fewer, so the file system serves the call,
/// the file was still fragmented after the confined call, and the confined refusal is the floor's.
/// Both calls leave the file's SHA-256 as it was.
///
/// The confined child is the one `bindings::bound_program_child` builds for the program, with one
/// read-only file beside it: `e4defrag` reads the mount table at `/etc/mtab`, which Substrate's
/// view does not hold, so the case mounts one naming `/workspace` as ext4; the control reads the
/// host's. The case skips naming what is missing without `e4defrag`, an ext4 workspace or
/// delegation, and fails instead where `LOOM_REQUIRE_DELEGATION` is set.
#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "Substrate's linux adapter serves Linux only"
)]
fn under_linux_a_bound_program_cannot_move_file_blocks() {
    support::delegated(
        "under_linux_a_bound_program_cannot_move_file_blocks",
        &[],
        |runner, workspace| {
            if let Some(missing) = ioctl_prerequisite(workspace) {
                assert!(
                    std::env::var_os("LOOM_REQUIRE_DELEGATION").is_none(),
                    "LOOM_REQUIRE_DELEGATION is set and a prerequisite is missing: {missing}"
                );
                return support::skip(&missing);
            }
            let case = Case::new("linux_move_ext");
            let mtab = case.root.join("mtab");
            std::fs::write(&mtab, "/dev/root /workspace ext4 rw 0 0\n").unwrap();
            let script = case.script(
                "move-blocks",
                &format!(
                    "f=target/blob\n\
                     if [ \"$1\" = make ]; then\n\
                     s=target/spacer\n\
                     i=0\n\
                     while [ \"$i\" -lt 16 ]; do\n\
                     /usr/bin/dd if=/dev/urandom of=\"$f\" bs=65536 count=1 seek=\"$i\" conv=notrunc,fsync status=none || exit 3\n\
                     /usr/bin/dd if=/dev/urandom of=\"$s\" bs=65536 count=1 seek=\"$i\" conv=notrunc,fsync status=none || exit 3\n\
                     i=$((i + 1))\n\
                     done\n\
                     /usr/bin/rm -f \"$s\"\n\
                     fi\n\
                     before=$(/usr/bin/sha256sum \"$f\" | /usr/bin/cut -d ' ' -f 1)\n\
                     size=$(/usr/bin/stat -c %s \"$f\")\n\
                     {E4DEFRAG} -v \"$f\" >target/e4defrag.out 2>target/e4defrag.err\n\
                     status=$?\n\
                     after=$(/usr/bin/sha256sum \"$f\" | /usr/bin/cut -d ' ' -f 1)\n\
                     printf 'dir=%s\\nstatus=%s\\nsize=%s\\nbefore=%s\\nafter=%s\\n' \"$(pwd)\" \"$status\" \"$size\" \"$before\" \"$after\"\n\
                     /usr/bin/cat target/e4defrag.out\n\
                     printf '\\nstderr:\\n'\n\
                     /usr/bin/cat target/e4defrag.err\n"
                ),
            );
            let argv = |mode: &str| [script.display().to_string(), mode.to_owned()];

            // Confined: Substrate's `linux` adapter, its seccomp floor installed.
            let child = bound_program_child(
                &argv("make"),
                "{}",
                Duration::from_secs(30),
                workspace,
                None,
            )
            .expect("the bound program's child")
            .with_read_only_file(&mtab, "/etc/mtab");
            assert_eq!(child.sandbox_path(&script), Some(script.clone()));
            let confined = runner.run_child(&child).expect("the program runs confined");
            assert_eq!(confined.applied.backend, Backend::Substrate);
            let confined = MoveBlocks::read("confined", &confined);

            // The control: the same call on the same file, through `local`, which installs no
            // seccomp program.
            let control_runner = EngineRunner::open(&EngineConfinement {
                adapter: ConfinementAdapter::Local,
                local_reason: Some(FLAG_REASON.to_owned()),
                cgroup_root: None,
            })
            .expect("local opens with a reason");
            let child = bound_program_child(
                &argv("again"),
                "{}",
                Duration::from_secs(30),
                workspace,
                control_runner.local_reason(),
            )
            .expect("the control's child");
            let control = control_runner
                .run_child(&child)
                .expect("the control runs unconfined");
            assert_eq!(control.applied.backend, Backend::None);
            let control = MoveBlocks::read("control", &control);
            let kept = format!("{}\n{}", confined.kept, control.kept);
            eprintln!("{kept}");

            assert_eq!(confined.status, "1", "the confined call fails: {kept}");
            assert!(
                confined
                    .stderr
                    .contains("Failed to defrag with EXT4_IOC_MOVE_EXT ioctl"),
                "EXT4_IOC_MOVE_EXT is refused under confinement: {kept}"
            );
            assert!(!confined.stdout.contains("[ OK ]"), "{kept}");
            assert_eq!(confined.before, confined.after, "{kept}");

            assert_eq!(control.status, "0", "the control succeeds: {kept}");
            assert!(control.stdout.contains("[ OK ]"), "{kept}");
            let (from, to) = control
                .extents()
                .unwrap_or_else(|| panic!("the control names its extents: {kept}"));
            assert!(
                from > to,
                "the control moved the blocks of a fragmented file ({from} -> {to}): {kept}"
            );
            assert_eq!(
                control.before, control.after,
                "a move of blocks keeps the content: {kept}"
            );

            // Both calls ran on one file: the confined one in `/workspace`, the control in the
            // same workspace at its host path, reading what the confined call left.
            assert_eq!(confined.dir, "/workspace", "{kept}");
            assert_eq!(
                Path::new(&control.dir).canonicalize().ok(),
                workspace.canonicalize().ok(),
                "the control runs in the same workspace: {kept}"
            );
            assert_eq!(confined.size, "1048576", "{kept}");
            assert_eq!(confined.before.len(), 64, "{kept}");
            assert_eq!(
                control.before, confined.after,
                "the control reads the file the confined call left: {kept}"
            );
        },
    );
}

/// What one run of the `move-blocks` program reported.
struct MoveBlocks {
    dir: String,
    status: String,
    size: String,
    before: String,
    after: String,
    stdout: String,
    stderr: String,
    kept: String,
}

impl MoveBlocks {
    fn read(label: &str, result: &TestExecution) -> Self {
        let output = String::from_utf8_lossy(&result.child_output.stdout).into_owned();
        let kept = format!("{label}: exit {:?}\n{output}", result.exit_code);
        assert_eq!(result.exit_code, Some(0), "{kept}");
        let (head, stderr) = output
            .split_once("\nstderr:\n")
            .unwrap_or_else(|| panic!("no stderr: {kept}"));
        let reported = |name: &str| -> String {
            head.lines()
                .find_map(|line| line.strip_prefix(&format!("{name}=")))
                .unwrap_or_else(|| panic!("no {name}: {kept}"))
                .to_owned()
        };
        Self {
            dir: reported("dir"),
            status: reported("status"),
            size: reported("size"),
            before: reported("before"),
            after: reported("after"),
            stdout: head.to_owned(),
            stderr: stderr.to_owned(),
            kept,
        }
    }

    /// The `extents: <from> -> <to>` `e4defrag -v` prints for a file it moved.
    fn extents(&self) -> Option<(u64, u64)> {
        let rest = self.stdout.split("extents: ").nth(1)?;
        let (from, rest) = rest.split_once(" -> ")?;
        let to: String = rest.chars().take_while(char::is_ascii_digit).collect();
        Some((from.trim().parse().ok()?, to.parse().ok()?))
    }
}

/// The directories a tool of the ioctl case is looked for in.
fn program_search_path() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default()
}

/// Where the tool `name` is.
fn locate_program(name: &str, search: &[PathBuf]) -> Option<PathBuf> {
    let _ = search;
    let path = Path::new("/usr/bin").join(name);
    path.is_file().then_some(path)
}

/// The locator finds a tool that only a later directory of its search holds, past a relative
/// entry and a non-executable file of the same name, and answers the file a link there names.
/// Its search covers `/usr/sbin` and `/sbin`, where Ubuntu's e2fsprogs puts `e4defrag`.
#[test]
fn the_locator_finds_a_tool_only_in_a_second_directory() {
    let root = tempfile::Builder::new()
        .prefix("locator-")
        .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
        .unwrap();
    let root = root.path().canonicalize().unwrap();
    let [first, second, third] = ["first", "second", "third"].map(|name| root.join(name));
    for directory in [&first, &second, &third] {
        std::fs::create_dir(directory).unwrap();
    }
    let name = "loom-locator-probe-tool";
    std::fs::write(first.join(name), "not executable").unwrap();
    let tool = third.join("real-tool");
    std::fs::write(&tool, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::os::unix::fs::symlink(&tool, second.join(name)).unwrap();

    let search = [PathBuf::from("relative"), first.clone(), second.clone()];
    assert_eq!(locate_program(name, &search), Some(tool.clone()));
    assert_eq!(locate_program(name, &[first.clone()]), None);
    assert_eq!(locate_program(name, &[]), None);

    let search = program_search_path();
    for directory in ["/usr/sbin", "/sbin", "/usr/bin", "/bin"] {
        assert!(
            search.iter().any(|entry| entry == Path::new(directory)),
            "{directory}: {search:?}"
        );
    }
}

/// What this host lacks for the ioctl case, if anything: `e4defrag`, or an ext4 workspace.
fn ioctl_prerequisite(workspace: &Path) -> Option<String> {
    if !Path::new(E4DEFRAG).is_file() {
        return Some(format!("executable {E4DEFRAG}"));
    }
    let kind = std::process::Command::new("/usr/bin/stat")
        .args(["-f", "-c", "%T"])
        .arg(workspace)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned());
    match kind.as_deref() {
        Some("ext2/ext3") => None,
        other => Some(format!(
            "an ext4 workspace ({} is on {other:?})",
            workspace.display()
        )),
    }
}

// ---------------------------------------------------------------------------------------------
// Fixtures

/// One case directory: the installed protocol and the bound scripts.
struct Case {
    root: PathBuf,
    catalog: ProtocolCatalog,
    timeout_seconds: i64,
}

impl Case {
    fn new(label: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("bound_command_confined")
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

    fn marker(&self) -> PathBuf {
        self.root.join("started")
    }

    /// A script that writes [`Case::marker`] and reports a note.
    fn marking_script(&self) -> PathBuf {
        self.script(
            "record-note",
            &format!(
                ": > '{}'\nprintf '{{\"note\":\"n-1\"}}\\n'\n",
                self.marker().display()
            ),
        )
    }

    fn script(&self, name: &str, body: &str) -> PathBuf {
        let path = self.root.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn run(
        &self,
        runner: Option<Arc<dyn TestRunner>>,
        unconfined_bindings: Option<bool>,
        script: &Path,
        router: Vec<(&'static str, Value)>,
        agent: Vec<(&'static str, Value)>,
    ) -> (
        Result<b10x_loom_intake_slice::run::SliceRun, b10x_loom_intake_slice::run::SliceError>,
        String,
    ) {
        self.run_in(runner, unconfined_bindings, None, script, router, agent)
    }

    fn run_in(
        &self,
        runner: Option<Arc<dyn TestRunner>>,
        unconfined_bindings: Option<bool>,
        workspace: Option<&Path>,
        script: &Path,
        router: Vec<(&'static str, Value)>,
        agent: Vec<(&'static str, Value)>,
    ) -> (
        Result<b10x_loom_intake_slice::run::SliceRun, b10x_loom_intake_slice::run::SliceError>,
        String,
    ) {
        let governor = CanonGovernor::new(MemoryCaseStore::default())
            .with_catalog(&self.catalog)
            .unwrap();
        let router = Recorded::new(router);
        let agent = Recorded::new(agent);
        let request = IntentRequest {
            intent: INTENT.into(),
            workspace: workspace.map(Path::to_owned),
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
                    argv: vec![script.display().to_string()],
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

fn pick() -> Vec<(&'static str, Value)> {
    vec![(
        "pick_protocol",
        json!({"protocol": NAME, "confidence": 0.99, "reasons": ["a team note"]}),
    )]
}

fn select() -> Vec<(&'static str, Value)> {
    select_with(json!({"text": "deploy finished"}))
}

fn select_with(arguments: Value) -> Vec<(&'static str, Value)> {
    vec![
        ("select_action", json!({"action": "note.record"})),
        ("action_arguments", arguments),
    ]
}

/// The one performed report the run printed as its step's effect.
fn effect(output: &str) -> Value {
    let line = output
        .lines()
        .find_map(|line| line.trim_start().strip_prefix("effect: "))
        .unwrap_or_else(|| panic!("no effect line: {output}"));
    serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}: {line}\n{output}"))
}

fn decode_base64(text: &str) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits = 0_u32;
    let mut count = 0;
    let mut bytes = Vec::new();
    for byte in text.bytes().filter(|byte| *byte != b'=') {
        let value = ALPHABET
            .iter()
            .position(|candidate| *candidate == byte)
            .unwrap_or_else(|| panic!("not base64: {text}"));
        bits = (bits << 6) | u32::try_from(value).unwrap();
        count += 6;
        if count >= 8 {
            count -= 8;
            bytes.push(u8::try_from((bits >> count) & 0xff).unwrap());
        }
    }
    String::from_utf8(bytes).unwrap()
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
