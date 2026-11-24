//! Acceptance for `story:confinement-opens-from-engine-config`: a host names Loom's confinement
//! once, as engine configuration ([`EngineConfinement`]), and one constructor
//! ([`EngineRunner::open`]) builds the runner from it. Under `local` every launch runs through
//! Substrate's `local` adapter, unconfined and declared insecure, with the operator's reason on
//! its record; under `linux` the runner wraps today's host-driver route.
//!
//! No case starts a network connection or a model. The `local` cases run real host processes in
//! temporary directories; the `linux` cases marked gated run in a delegated scope through
//! `tests/support` and skip naming what is missing. The host-policy case starts this binary
//! again with `SUBSTRATE_CONFINEMENT_LOCAL=refuse` on that child process alone, so no other case
//! sees the variable.

use crate::support;

use std::collections::{BTreeMap, VecDeque};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use b10x_loom_intake_slice::clock::Clock;
use b10x_loom_intake_slice::confinement::{
    AppliedConfinement, Backend, BaselineEnvironment, ChildCommand, ChildError, ChildOutput,
    ConfinementAdapter, ConfinementError, ConfinementRefusal, EgressAperture, EngineConfinement,
    EngineRunner, ExecEnvironment, SubstrateRunner, TestRunner, discover_cgroup_root,
    escape_reason, hold_cgroup_root,
};
use b10x_loom_intake_slice::context::ContextPolicy;
use b10x_loom_intake_slice::executor::{TestCommand, UnconfinedRunner, applied_label};
use b10x_loom_intake_slice::intent::{IntentRequest, run_intent_with_options};
use b10x_loom_intake_slice::run::{RunOptions, SliceRequest, run_with_options};
use chrono::{DateTime, FixedOffset};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason as TurnStop, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest,
};
use loom_governor::{CanonGovernor, MemoryCaseStore};
use loom_protocols::ProtocolCatalog;
use serde_json::{Value, json};

/// The reason every `local` runner of these cases is opened with.
const REASON: &str = "the acceptance cases run host processes on purpose";

/// The program each deadline case starts: its group id, a background `sleep 30`, then a
/// `sleep 30` of its own.
const OUTLIVES: &str = "echo $$; sleep 30 & sleep 30";

// ---------------------------------------------------------------------------------------------
// Fixtures

fn local_confinement(reason: Option<&str>) -> EngineConfinement {
    EngineConfinement {
        adapter: ConfinementAdapter::Local,
        local_reason: reason.map(str::to_owned),
        cgroup_root: None,
    }
}

fn local_runner() -> EngineRunner {
    EngineRunner::open(&local_confinement(Some(REASON))).expect("local opens with a reason")
}

/// A workspace directory of its own, canonical, in a temporary directory that also holds
/// `outside/`, a directory beside the workspace.
struct Fixture {
    _directory: tempfile::TempDir,
    workspace: PathBuf,
    outside: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let workspace = root.join("workspace");
        let outside = root.join("outside");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(&outside).unwrap();
        Self {
            _directory: directory,
            workspace,
            outside,
        }
    }
}

/// The baseline `PATH` alone, so a child finds `sh`, `pwd` and `cat` under either adapter.
fn baseline() -> ExecEnvironment {
    ExecEnvironment {
        allow: vec![BaselineEnvironment::Path],
        set: BTreeMap::new(),
    }
}

/// `sh -c script` in `workspace` with the baseline environment.
fn shell(script: &str, workspace: &Path, args: &[&str]) -> ChildCommand {
    let mut words = vec!["-c".to_owned(), script.to_owned(), "sh".to_owned()];
    words.extend(args.iter().map(|arg| (*arg).to_owned()));
    ChildCommand::new("sh", words, workspace).with_environment(baseline())
}

/// A child's shell line that holds until the host creates `target/release` in its workspace
/// ([`release`]), at most 30 s: the child runs exactly as long as the host's check needs.
const HOLD: &str =
    "i=0; while [ ! -e target/release ] && [ $i -lt 600 ]; do sleep 0.05; i=$((i+1)); done";

/// Ends the [`HOLD`] of the child working in `workspace`.
fn release(workspace: &std::path::Path) {
    std::fs::write(workspace.join("target/release"), "").expect("the release marker");
}

/// The four launches of a runner, by name.
const LAUNCHES: [&str; 4] = ["run_child", "run_child_with_stdin", "spawn_child", "run"];

/// What one launch recorded.
struct Launched {
    name: &'static str,
    applied: AppliedConfinement,
}

/// Each of the four launches of `runner` starting `sh -c script` with `args` and the launch's
/// name, each to its end.
fn each_launch(
    runner: &EngineRunner,
    workspace: &Path,
    script: &str,
    args: &[&str],
) -> Vec<Launched> {
    LAUNCHES
        .iter()
        .map(|&name| {
            // Each launch gets its own last argument, so a write names the launch.
            let mut named: Vec<&str> = args.to_vec();
            named.push(name);
            let child = shell(script, workspace, &named);
            let applied = match name {
                "run_child" => {
                    let result = runner
                        .run_child(&child)
                        .expect("run_child runs under local");
                    assert_eq!(result.exit_code, Some(0), "{name}: {}", result.output);
                    result.applied
                }
                "run_child_with_stdin" => {
                    let result = runner
                        .run_child(&child.with_stdin(b"input\n".to_vec()))
                        .expect("run_child with stdin runs under local");
                    assert_eq!(result.exit_code, Some(0), "{name}: {}", result.output);
                    result.applied
                }
                "spawn_child" => {
                    let live = runner
                        .spawn_child(&child)
                        .expect("spawn_child starts under local");
                    let result = live.wait().expect("the live child ends");
                    assert_eq!(
                        result.exit.exit_code,
                        Some(0),
                        "{name}: {:?}",
                        result.output
                    );
                    result.applied
                }
                _ => {
                    let mut words = vec!["-c".to_owned(), script.to_owned(), "sh".to_owned()];
                    words.extend(named.iter().map(|arg| (*arg).to_owned()));
                    let command = TestCommand::new("sh", words);
                    let result = runner
                        .run(&command, workspace)
                        .expect("the test runs under local");
                    assert_eq!(result.exit_code, Some(0), "{name}: {}", result.output);
                    result.applied
                }
            };
            Launched { name, applied }
        })
        .collect()
}

fn stdout(output: &ChildOutput) -> String {
    String::from_utf8(output.stdout.clone()).expect("UTF-8 output")
}

/// No process is left in the group `group` (`kill(-group, 0)` answers `ESRCH`).
fn group_is_gone(group: i32) -> bool {
    matches!(
        nix::sys::signal::killpg(nix::unistd::Pid::from_raw(group), None),
        Err(nix::errno::Errno::ESRCH)
    )
}

/// The process group a deadline case's program printed first.
fn group_of(output: &ChildOutput) -> i32 {
    stdout(output)
        .lines()
        .next()
        .and_then(|line| line.trim().parse().ok())
        .unwrap_or_else(|| panic!("the program printed its group first: {output:?}"))
}

// ---------------------------------------------------------------------------------------------
// The constructor and the source

/// The workspace root, two directories above this crate.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root")
}

fn rust_files(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).expect("read a source directory") {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

/// Every file under the `src/` directory of each crate in `crates/`, as text with its comment
/// lines and white space removed, by its path relative to the workspace root. `tests/` is never
/// read.
fn squeezed_sources() -> Vec<(String, String)> {
    let root = workspace_root();
    let mut files = Vec::new();
    for entry in std::fs::read_dir(root.join("crates")).expect("read crates/") {
        let src = entry.expect("a crates/ entry").path().join("src");
        if src.is_dir() {
            rust_files(&src, &mut files);
        }
    }
    assert!(
        files
            .iter()
            .any(|path| path.ends_with("loom-intake-slice/src/confinement.rs")),
        "the scan reaches the slice's confinement: {files:?}"
    );
    files
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).expect("read a source file");
            let relative = path
                .strip_prefix(&root)
                .expect("under the workspace")
                .to_string_lossy()
                .into_owned();
            let code: String = text
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect();
            (
                relative,
                code.chars().filter(|c| !c.is_whitespace()).collect(),
            )
        })
        .collect()
}

/// Whether the keyword `word` starts at `at` of `text` (white space removed): what precedes it
/// ends an item or a modifier, so an identifier holding the word is no match.
fn keyword_at(text: &str, at: usize, word: &str) -> bool {
    let before = &text[..at];
    text[at..].starts_with(word)
        && (before.is_empty()
            || ["}", ";", "{", "]", ")", "pub", "async", "const", "unsafe"]
                .iter()
                .any(|end| before.ends_with(end)))
}

/// The impl and the name of the function whose body holds the byte `at` of `text` (white space
/// removed): the last `fn` before it, and the last `impl` before that.
fn enclosing(text: &str, at: usize) -> (String, String) {
    let last = |word: &str, end: usize| {
        text[..end]
            .match_indices(word)
            .map(|(start, _)| start)
            .filter(|&start| keyword_at(text, start, word))
            .last()
    };
    let function = last("fn", at).expect("inside a function");
    let name: String = text[function + 2..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    let implementation = last("impl", function)
        .map(|start| {
            text[start + 4..function]
                .chars()
                .take_while(|c| *c != '{')
                .collect()
        })
        .unwrap_or_default();
    (implementation, name)
}

#[test]
fn only_the_constructor_opens_an_adapter() {
    let sources = squeezed_sources();
    let mut opens = Vec::new();
    for (path, text) in &sources {
        for (at, _) in text.match_indices("confinement::open(") {
            let (implementation, function) = enclosing(text, at);
            let argument: String = text[at + "confinement::open(".len()..]
                .chars()
                .take(64)
                .collect();
            opens.push((path.clone(), implementation, function, argument));
        }
        assert!(
            !text.contains("AdapterRequest::Linux"),
            "{path} names AdapterRequest::Linux: linux stays on the host-driver route"
        );
    }
    assert_eq!(
        opens.len(),
        1,
        "exactly one call opens an adapter: {opens:?}"
    );
    let (path, implementation, function, argument) = &opens[0];
    assert_eq!(implementation, "EngineRunner", "{path}: {opens:?}");
    assert_eq!(function, "open", "{path}: {opens:?}");
    assert!(
        argument.starts_with("AdapterRequest::Local("),
        "the constructor opens local: {argument}"
    );
}

/// An adapter request named anywhere but in `EngineRunner::open`, an import at the top of a file
/// included, is a second open point however `open` is reached: an imported `open` too.
#[test]
fn no_adapter_request_is_named_outside_the_constructor() {
    let mut named = Vec::new();
    for (path, text) in squeezed_sources() {
        for (at, _) in text.match_indices("AdapterRequest") {
            let inside_a_function = text[..at]
                .match_indices("fn")
                .any(|(start, _)| keyword_at(&text, start, "fn"));
            let place = if inside_a_function {
                enclosing(&text, at)
            } else {
                (String::new(), String::new())
            };
            named.push((path.clone(), place));
        }
    }
    assert!(!named.is_empty(), "the constructor names its request");
    for (path, (implementation, function)) in &named {
        assert!(
            implementation == "EngineRunner" && function == "open",
            "{path} names AdapterRequest in `{implementation}::{function}`, outside \
             EngineRunner::open: {named:?}"
        );
    }
}

#[test]
fn only_the_constructor_takes_an_engine_confinement() {
    let mut takers = Vec::new();
    for (path, text) in squeezed_sources() {
        for (at, _) in text.match_indices("pubfn") {
            let signature = &text[at + "pubfn".len()..];
            let Some(open) = signature.find('(') else {
                continue;
            };
            let name = &signature[..open];
            // The parameter list, to its matching parenthesis.
            let mut depth = 0usize;
            let mut end = open;
            for (index, character) in signature[open..].char_indices() {
                match character {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = open + index;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if signature[open..end].contains("EngineConfinement") {
                let (implementation, _) = enclosing(&text, at + 5);
                takers.push((path.clone(), implementation, name.to_owned()));
            }
        }
    }
    assert_eq!(
        takers.len(),
        1,
        "exactly one public function takes an EngineConfinement: {takers:?}"
    );
    assert_eq!(takers[0].1, "EngineRunner", "{takers:?}");
    assert_eq!(takers[0].2, "open", "{takers:?}");
}

// ---------------------------------------------------------------------------------------------
// Launches under `local`, with a reason

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn each_local_launch_writes_outside_its_workspace() {
    let fixture = Fixture::new();
    let outside = fixture.outside.to_str().unwrap();
    each_launch(
        &local_runner(),
        &fixture.workspace,
        r#"printf written > "$1/$2""#,
        &[outside],
    );
    for name in LAUNCHES {
        let written = fixture.outside.join(name);
        assert_eq!(
            std::fs::read_to_string(&written).ok().as_deref(),
            Some("written"),
            "{name} wrote {} outside its workspace",
            written.display()
        );
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn each_local_record_names_the_adapter_and_reason() {
    let fixture = Fixture::new();
    for launched in each_launch(&local_runner(), &fixture.workspace, "true", &[]) {
        assert_eq!(
            launched.applied.adapter.as_deref(),
            Some("local"),
            "{}",
            launched.name
        );
        assert_eq!(
            launched.applied.local_reason.as_deref(),
            Some(REASON),
            "{}",
            launched.name
        );
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_record_claims_no_confinement() {
    let fixture = Fixture::new();
    let runner = local_runner();
    assert_eq!(runner.backend(), Backend::None);
    for launched in each_launch(&runner, &fixture.workspace, "true", &[]) {
        let applied = &launched.applied;
        assert_eq!(applied.backend, Backend::None, "{}", launched.name);
        assert_eq!(applied.filesystem, "unconfined", "{}", launched.name);
        assert_eq!(
            applied.network, None,
            "{}: absent, never empty",
            launched.name
        );
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_record_names_the_host_network() {
    let fixture = Fixture::new();
    for launched in each_launch(&local_runner(), &fixture.workspace, "true", &[]) {
        let facts = launched.applied.facts.clone().unwrap_or_default();
        for fact in ["host_network", "declared_insecure"] {
            assert!(
                facts.iter().any(|named| named == fact),
                "{} names {fact}: {facts:?}",
                launched.name
            );
        }
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_test_run_sees_host_paths() {
    let fixture = Fixture::new();
    let runner = local_runner();
    let command = TestCommand::new(
        "sh",
        ["-c", r#"printf "%s\n" "$PATH" "$HOME" "$CARGO_TARGET_DIR""#],
    );
    let result = runner
        .run(&command, &fixture.workspace)
        .expect("the test runs under local");
    assert_eq!(result.exit_code, Some(0), "{}", result.output);
    let printed = stdout(&result.child_output);
    let lines: Vec<&str> = printed.lines().collect();
    let target = fixture.workspace.join("target");
    assert_eq!(
        lines,
        [
            std::env::var("PATH").unwrap().as_str(),
            std::env::var("HOME").unwrap().as_str(),
            target.to_str().unwrap(),
        ],
        "{printed}"
    );
    for line in lines {
        assert!(
            !line.starts_with("/toolchain") && !line.starts_with("/workspace"),
            "a sandbox path reached a local test: {line}"
        );
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_child_starts_in_the_host_directory() {
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.workspace.join("sub")).unwrap();
    let child = ChildCommand::new("pwd", Vec::<String>::new(), &fixture.workspace)
        .with_environment(baseline())
        .with_workdir("/workspace/sub");
    let result = local_runner().run_child(&child).expect("pwd runs");
    assert_eq!(result.exit_code, Some(0), "{}", result.output);
    assert_eq!(
        stdout(&result.child_output),
        format!("{}\n", fixture.workspace.join("sub").display())
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_live_child_starts_in_the_host_directory() {
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.workspace.join("sub")).unwrap();
    let child = ChildCommand::new("pwd", Vec::<String>::new(), &fixture.workspace)
        .with_environment(baseline())
        .with_workdir("/workspace/sub");
    let mut live = local_runner().spawn_child(&child).expect("pwd starts live");
    let mut read = Vec::new();
    loop {
        match live.read(Duration::from_secs(5)) {
            Ok(Some(frame)) => read.extend(frame.bytes),
            Ok(None) => break,
            Err(ChildError::ReadTimedOut) => panic!("no output within 5 s: {read:?}"),
            Err(error) => panic!("the read failed: {error}"),
        }
    }
    assert_eq!(
        String::from_utf8(read).unwrap(),
        format!("{}\n", fixture.workspace.join("sub").display())
    );
    let result = live.wait().expect("the live child ends");
    assert_eq!(result.exit.exit_code, Some(0));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn child_path_under_local_is_the_host_path() {
    let fixture = Fixture::new();
    let file = fixture.workspace.join("data.txt");
    std::fs::write(&file, b"the bytes\n").unwrap();
    let runner = local_runner();
    let probe = ChildCommand::new("cat", Vec::<String>::new(), &fixture.workspace);
    let answer = runner
        .child_path(&probe, &file)
        .expect("a workspace file maps under local");
    assert_eq!(answer, file.canonicalize().unwrap());
    let child = ChildCommand::new("cat", [answer.to_str().unwrap()], &fixture.workspace)
        .with_environment(baseline());
    let result = runner.run_child(&child).expect("cat runs");
    assert_eq!(result.exit_code, Some(0), "{}", result.output);
    assert_eq!(result.child_output.stdout, b"the bytes\n");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_live_child_outlives_another_launch() {
    let fixture = Fixture::new();
    let runner = local_runner();
    let mut live = runner
        .spawn_child(&shell(
            r#"read line; echo "$line""#,
            &fixture.workspace,
            &[],
        ))
        .expect("the reader starts live");
    let other = runner
        .run_child(&shell("true", &fixture.workspace, &[]))
        .expect("another launch runs");
    assert_eq!(other.exit_code, Some(0));
    live.write_stdin(b"still here\n")
        .expect("the live child reads");
    live.close_stdin().expect("its input closes");
    let mut read = Vec::new();
    loop {
        match live.read(Duration::from_secs(5)) {
            Ok(Some(frame)) => read.extend(frame.bytes),
            Ok(None) => break,
            Err(error) => panic!("the live child delivered nothing: {error}"),
        }
    }
    assert_eq!(read, b"still here\n");
    assert_eq!(live.wait().expect("it ends").exit.exit_code, Some(0));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_live_result_has_no_driver_record() {
    let fixture = Fixture::new();
    let live = local_runner()
        .spawn_child(&shell("true", &fixture.workspace, &[]))
        .expect("starts live");
    let result = live.wait().expect("ends");
    assert_eq!(result.exit.exit_code, Some(0));
    assert_eq!(result.driver_record, None);
}

// ---------------------------------------------------------------------------------------------
// Deadline under `local`

const DEADLINE: Duration = Duration::from_secs(1);
const WITHIN: Duration = Duration::from_secs(3);

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_child_past_its_timeout_is_killed_with_its_group() {
    let fixture = Fixture::new();
    let child = shell(OUTLIVES, &fixture.workspace, &[]).with_timeout(DEADLINE);
    let started = Instant::now();
    let result = local_runner()
        .run_child(&child)
        .expect("a timed-out child is a result");
    assert!(
        started.elapsed() < WITHIN,
        "returned after {:?}",
        started.elapsed()
    );
    assert!(result.timed_out, "{}", result.output);
    assert_eq!(result.exit_code, None);
    assert!(group_is_gone(group_of(&result.child_output)));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_child_with_stdin_past_its_timeout_is_killed_with_its_group() {
    let fixture = Fixture::new();
    let child = shell(OUTLIVES, &fixture.workspace, &[])
        .with_timeout(DEADLINE)
        .with_stdin(b"unread\n".to_vec());
    let started = Instant::now();
    let result = local_runner()
        .run_child(&child)
        .expect("a timed-out child is a result");
    assert!(
        started.elapsed() < WITHIN,
        "returned after {:?}",
        started.elapsed()
    );
    assert!(result.timed_out, "{}", result.output);
    assert_eq!(result.exit_code, None);
    assert!(group_is_gone(group_of(&result.child_output)));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_live_child_past_its_timeout_is_killed_with_its_group() {
    let fixture = Fixture::new();
    let child = shell(OUTLIVES, &fixture.workspace, &[]).with_timeout(DEADLINE);
    let started = Instant::now();
    let live = local_runner().spawn_child(&child).expect("starts live");
    std::thread::sleep(DEADLINE + Duration::from_millis(500));
    let result = live.wait().expect("a timed-out live child is a result");
    assert!(
        started.elapsed() < WITHIN,
        "returned after {:?}",
        started.elapsed()
    );
    assert!(result.exit.timed_out, "{:?}", result.exit);
    assert_eq!(result.exit.exit_code, None);
    assert!(group_is_gone(group_of(&result.output)));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_test_past_its_timeout_is_killed_with_its_group() {
    let fixture = Fixture::new();
    let command = TestCommand::new("sh", ["-c", OUTLIVES]).with_timeout(DEADLINE);
    let started = Instant::now();
    let result = local_runner()
        .run(&command, &fixture.workspace)
        .expect("a timed-out test is a result");
    assert!(
        started.elapsed() < WITHIN,
        "returned after {:?}",
        started.elapsed()
    );
    assert!(result.timed_out, "{}", result.output);
    assert_eq!(result.exit_code, None);
    assert!(group_is_gone(group_of(&result.child_output)));
}

// ---------------------------------------------------------------------------------------------
// A cancel set before the launch (`story:cancel-a-running-bound-program`)

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_child_whose_cancel_is_set_is_refused_before_launch() {
    use b10x_loom_intake_slice::confinement::Cancel as ChildCancel;
    let fixture = Fixture::new();
    let marker = fixture.outside.join("marker");
    let cancel = ChildCancel::new();
    cancel.cancel();
    let child = shell(
        &format!("touch '{}'", marker.display()),
        &fixture.workspace,
        &[],
    )
    .with_cancel(cancel);
    // `linux` is refused before any check or driver open, so it needs no delegated scope.
    let linux = EngineRunner::open(&linux_confinement()).expect("linux builds");
    for (adapter, runner) in [("local", local_runner()), ("linux", linux)] {
        for child in [child.clone(), child.clone().with_stdin(b"input\n".to_vec())] {
            let error = runner
                .run_child(&child)
                .expect_err("a child whose cancel is set is refused");
            assert_eq!(
                error.reason,
                ConfinementRefusal::Cancelled,
                "{adapter}: {error}"
            );
            assert!(!marker.exists(), "{adapter}: the child started");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Refusals under `local`

#[test]
fn a_blank_reason_is_refused_before_any_child() {
    for reason in [Some(" \t\n "), Some(""), None] {
        let error = EngineRunner::open(&local_confinement(reason))
            .expect_err("a blank reason opens nothing");
        assert_eq!(
            error.reason,
            ConfinementRefusal::LocalReasonMissing,
            "{reason:?}: {error}"
        );
    }
}

/// Set on the copy of this binary the host-policy case starts.
const POLICY_CASE: &str = "LOOM_ENGINE_CONFINEMENT_POLICY_CASE";

#[test]
fn host_policy_refusal_runs_in_its_own_process() {
    const NAME: &str = "host_policy_refusal_runs_in_its_own_process";
    if std::env::var_os(POLICY_CASE).is_some() {
        let error = EngineRunner::open(&local_confinement(Some(REASON)))
            .expect_err("a host that forbids local refuses the constructor");
        assert_eq!(
            error.reason,
            ConfinementRefusal::CapabilityUnserved,
            "{error}"
        );
        assert!(
            error.detail.contains("SUBSTRATE_CONFINEMENT_LOCAL"),
            "{error}"
        );
        assert!(
            error.detail.contains("confinement.local-refused"),
            "{error}"
        );
        return;
    }
    let name = full_name!(NAME);
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name.as_str(), "--nocapture", "--test-threads=1"])
        .env(POLICY_CASE, "1")
        .env("SUBSTRATE_CONFINEMENT_LOCAL", "refuse")
        .output()
        .expect("the case starts its own binary");
    let printed = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}\n{printed}{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        printed
            .lines()
            .any(|line| line == format!("test {name} ... ok"))
            && printed.contains("test result: ok. 1 passed;"),
        "the child ran exactly the case: {printed}"
    );
}

fn aperture() -> EgressAperture {
    EgressAperture {
        name: "model".to_owned(),
        host: "api.example.test".to_owned(),
        port: 443,
        pinned: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 443)),
        max_bytes: None,
    }
}

/// `child` refused under `local` by both `run_child` and `spawn_child`, naming `family`, before
/// the marker it would write exists.
fn refused_before_launch(child: &ChildCommand, family: &str, marker: &Path) {
    let runner = local_runner();
    let errors: Vec<ConfinementError> = vec![
        runner.run_child(child).expect_err("run_child refuses"),
        runner.spawn_child(child).expect_err("spawn_child refuses"),
    ];
    for error in errors {
        assert_eq!(
            error.reason,
            ConfinementRefusal::CapabilityUnserved,
            "{error}"
        );
        assert!(error.detail.contains(family), "names {family}: {error}");
    }
    assert!(!marker.exists(), "no child started: {}", marker.display());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_child_with_egress_is_refused() {
    let fixture = Fixture::new();
    let marker = fixture.outside.join("egress");
    let child = shell(
        r#"printf x > "$1""#,
        &fixture.workspace,
        &[marker.to_str().unwrap()],
    )
    .with_egress(aperture());
    refused_before_launch(&child, "network_egress", &marker);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_child_with_a_read_only_root_is_refused() {
    let fixture = Fixture::new();
    let marker = fixture.outside.join("root");
    let root = fixture.outside.join("tools");
    std::fs::create_dir(&root).unwrap();
    let child = shell(
        r#"printf x > "$1""#,
        &fixture.workspace,
        &[marker.to_str().unwrap()],
    );
    refused_before_launch(
        &child.clone().with_read_only_root(&root, "/opt/tools"),
        "filesystem_view",
        &marker,
    );
    // A directory given at its own host path is a read-only root too.
    refused_before_launch(&child.at_host_path(&root), "filesystem_view", &marker);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_child_with_a_read_only_file_is_refused() {
    let fixture = Fixture::new();
    let marker = fixture.outside.join("file");
    let file = fixture.outside.join("settings.toml");
    std::fs::write(&file, "x = 1\n").unwrap();
    let child = shell(
        r#"printf x > "$1""#,
        &fixture.workspace,
        &[marker.to_str().unwrap()],
    );
    refused_before_launch(
        &child
            .clone()
            .with_read_only_file(&file, "/etc/settings.toml"),
        "filesystem_view",
        &marker,
    );
    // A file given at its own host path is a read-only file too.
    refused_before_launch(&child.at_host_path(&file), "filesystem_view", &marker);
}

// ---------------------------------------------------------------------------------------------
// Under `linux`

/// The `linux` confinement on the delegated cgroup root this process runs under.
fn linux_confinement() -> EngineConfinement {
    EngineConfinement {
        adapter: ConfinementAdapter::Linux,
        local_reason: None,
        cgroup_root: discover_cgroup_root().map(|root| root.to_string_lossy().into_owned()),
    }
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "Substrate's linux adapter serves Linux only"
)]
fn building_a_linux_runner_ends_no_run() {
    // Gated: runs in a delegated scope, or skips naming what is missing.
    support::delegated(
        &full_name!("building_a_linux_runner_ends_no_run"),
        &[],
        |_, workspace| {
            let first = EngineRunner::open(&linux_confinement()).expect("linux builds");
            let live = first
                .spawn_child(&shell(&format!("{HOLD}; exit 7"), workspace, &[]))
                .expect("the first runner's child starts");
            let _second = EngineRunner::open(&linux_confinement()).expect("linux builds again");
            release(workspace);
            let result = live.wait().expect("the first child ends");
            assert_eq!(result.exit.exit_code, Some(7), "{:?}", result.output);
        },
    );
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "Substrate's linux adapter serves Linux only"
)]
fn a_linux_record_has_no_reason() {
    // Gated: runs in a delegated scope, or skips naming what is missing.
    support::delegated(
        &full_name!("a_linux_record_has_no_reason"),
        &[],
        |_, workspace| {
            let runner = EngineRunner::open(&linux_confinement()).expect("linux builds");
            assert_eq!(runner.backend(), Backend::Substrate);
            let result = runner
                .run_child(&shell("true", workspace, &[]))
                .expect("the child runs confined");
            assert_eq!(result.exit_code, Some(0), "{}", result.output);
            assert_eq!(result.applied.adapter.as_deref(), Some("linux"));
            assert_eq!(result.applied.local_reason, None);
        },
    );
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "Substrate's linux adapter serves Linux only"
)]
fn an_undelegated_root_is_refused_and_nothing_falls_back() {
    let fixture = Fixture::new();
    let runner = EngineRunner::open(&EngineConfinement {
        adapter: ConfinementAdapter::Linux,
        local_reason: None,
        cgroup_root: Some(fixture.outside.to_string_lossy().into_owned()),
    })
    .expect("building a linux runner opens nothing");
    let marker = fixture.outside.join("ran");
    let error = runner
        .run_child(&shell(
            r#"printf x > "$1""#,
            &fixture.workspace,
            &[marker.to_str().unwrap()],
        ))
        .expect_err("an undelegated root runs nothing");
    assert_eq!(
        error.reason,
        ConfinementRefusal::CgroupUndelegated,
        "{error}"
    );
    assert!(!marker.exists(), "no child started");
}

/// `story:concurrent-confined-runs`: a process holds its delegated root once, and every run of the
/// process shares that hold. A second hold in the same process is refused, naming the root, while a
/// runner on the held root runs a child, so the runner takes no hold of its own; once the first hold
/// is dropped, the root is held again.
#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "Substrate's linux adapter serves Linux only"
)]
fn a_second_hold_of_a_root_in_this_process_is_refused() {
    // Gated: runs in a delegated scope, or skips naming what is missing.
    support::delegated(
        &full_name!("a_second_hold_of_a_root_in_this_process_is_refused"),
        &[],
        |runner, workspace| {
            let root = runner.cgroup_root().expect("the harness names its root");
            let hold = hold_cgroup_root(root).expect("the process holds its delegated root");
            let second = hold_cgroup_root(root).expect_err("a second hold in this process");
            assert_eq!(
                second.reason,
                ConfinementRefusal::CgroupUndelegated,
                "{second}"
            );
            assert!(
                second.detail.contains(&hold.root().display().to_string()),
                "the refusal names the root: {second}"
            );
            let held = SubstrateRunner::new(Some(hold.root().to_owned()));
            let result = held
                .run_child(&shell("true", workspace, &[]))
                .expect("a runner on the held root runs: it takes no hold");
            assert_eq!(result.exit_code, Some(0), "{}", result.output);
            drop(hold);
            let again = hold_cgroup_root(root).expect("a dropped hold frees the root");
            assert_eq!(again.root(), root.canonicalize().unwrap());
        },
    );
}

/// `story:concurrent-confined-runs`: a cgroup beside the one this process runs in, under the same
/// delegated scope, holds no process of it, so Substrate proves no `limits` there and a run on it
/// is refused `CgroupUndelegated`. That is why each run of a process does not get a cgroup root of
/// its own: every run shares the one root the process holds.
#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "Substrate's linux adapter serves Linux only"
)]
fn a_root_that_does_not_hold_this_process_is_refused_cgroup_undelegated() {
    // Gated: runs in a delegated scope, or skips naming what is missing.
    support::delegated(
        &full_name!("a_root_that_does_not_hold_this_process_is_refused_cgroup_undelegated"),
        &[],
        |runner, workspace| {
            let root = runner.cgroup_root().expect("the harness names its root");
            let sibling = root.join(format!("loom-run-{}", std::process::id()));
            std::fs::create_dir(&sibling).expect("a sibling cgroup of this process's own");
            let error = {
                let runner = EngineRunner::open(&EngineConfinement {
                    adapter: ConfinementAdapter::Linux,
                    local_reason: None,
                    cgroup_root: Some(sibling.to_string_lossy().into_owned()),
                })
                .expect("building a linux runner opens nothing");
                runner
                    .run_child(&shell("true", workspace, &[]))
                    .expect_err("a root that holds no process of this one runs nothing")
            };
            let _ = std::fs::remove_dir(&sibling);
            assert_eq!(
                error.reason,
                ConfinementRefusal::CgroupUndelegated,
                "{error}"
            );
            assert!(
                error.detail.contains("`limits`"),
                "Substrate proves no limits on a root that holds no process of this one: {error}"
            );
        },
    );
}

// ---------------------------------------------------------------------------------------------
// The run line

struct NoClock;
impl Clock for NoClock {
    fn read(&self) -> Result<DateTime<FixedOffset>, String> {
        panic!("a software change reads no query clock")
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_run_prints_its_adapter_and_reason() {
    let fixture = GitFixture::new();
    let catalog = ProtocolCatalog::bundled().unwrap();
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog)
        .unwrap();
    let classifier = Recorded::new(vec![(
        "pick_protocol",
        json!({"protocol":"software-change@1", "confidence":0.99, "reasons":["change code"]}),
    )]);
    let agent = Recorded::new(vec![
        ("select_action", json!({"action":"tests.run"})),
        ("action_arguments", json!({})),
    ]);
    let runner = EngineRunner::open(&local_confinement(Some("x"))).expect("local opens");
    let request = IntentRequest {
        intent: "Run the tests.".into(),
        workspace: Some(fixture.workspace.clone()),
        runner: Some(Arc::new(runner)),
        test: TestCommand::new("true", Vec::<String>::new()),
        max_steps: 1,
        threshold: 0.5,
        inputs: b10x_loom_intake_slice::case::RUN_DEFAULTS,
        test_paths: b10x_loom_intake_slice::test_paths::TestPaths::default(),
        bindings: None,
    };
    let mut out = Vec::new();
    let run = run_intent_with_options(
        &request,
        &catalog,
        &governor,
        &governor,
        &classifier,
        &agent,
        &NoClock,
        &mut out,
        &RunOptions {
            context_policy: ContextPolicy::Legacy,
            context_report: None,
            ..RunOptions::default()
        },
    );
    let printed = String::from_utf8(out).unwrap();
    assert!(run.is_ok(), "{run:?}\n{printed}");
    assert!(
        printed.lines().any(|line| line == "confinement: local (x)"),
        "{printed}"
    );
}

/// The `confinement:` lines a slice run of `runner` prints. The classifier is unsure, so the run
/// stops right after its confinement line and no test runs.
fn slice_run_lines(runner: Arc<dyn TestRunner>) -> Vec<String> {
    let fixture = Fixture::new();
    let request = SliceRequest {
        runner,
        intent: "Run the tests.".into(),
        workspace: fixture.workspace.clone(),
        test: TestCommand::new("true", Vec::<String>::new()),
        max_steps: 1,
        threshold: 0.5,
        inputs: b10x_loom_intake_slice::case::RUN_DEFAULTS,
        test_paths: b10x_loom_intake_slice::test_paths::TestPaths::default(),
    };
    let classifier = Recorded::new(vec![(
        "pick_protocol",
        json!({"protocol":"software-change@1", "confidence":0.1, "reasons":["unsure"]}),
    )]);
    let agent = Recorded::new(Vec::new());
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let mut out = Vec::new();
    let _ = run_with_options(
        &request,
        &governor,
        &governor,
        &classifier,
        &agent,
        &mut out,
        &RunOptions {
            context_policy: ContextPolicy::Legacy,
            context_report: None,
            ..RunOptions::default()
        },
    );
    String::from_utf8(out)
        .unwrap()
        .lines()
        .filter(|line| line.starts_with("confinement:"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn a_linux_slice_run_prints_substrate_as_before() {
    let linux = EngineRunner::open(&EngineConfinement {
        adapter: ConfinementAdapter::Linux,
        local_reason: None,
        cgroup_root: None,
    })
    .expect("linux builds");
    assert_eq!(slice_run_lines(Arc::new(linux)), ["confinement: substrate"]);
}

#[test]
fn an_unconfined_slice_run_prints_none_as_before() {
    assert_eq!(
        slice_run_lines(Arc::new(UnconfinedRunner)),
        ["confinement: none"]
    );
}

/// The applied record a test run's summary is named from (`TestRun::summary`, which the briefing
/// quotes): `local` with its reason escaped, `substrate` and `none` as before.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_test_run_summary_names_local_and_its_reason() {
    let fixture = Fixture::new();
    let local = EngineRunner::open(&local_confinement(Some("x\u{2028}y")))
        .expect("local opens")
        .run(
            &TestCommand::new("true", Vec::<String>::new()),
            &fixture.workspace,
        )
        .expect("true runs");
    assert_eq!(applied_label(&local.applied), "local (x\\u{2028}y)");
    let mut linux = local.applied.clone();
    linux.backend = Backend::Substrate;
    linux.adapter = Some("linux".to_owned());
    linux.local_reason = None;
    assert_eq!(applied_label(&linux), "substrate");
    let mut none = local.applied;
    none.adapter = None;
    none.local_reason = None;
    assert_eq!(applied_label(&none), "none");
}

/// Every character that breaks or hides a line is escaped wherever the reason is printed or
/// recorded: the label and the applied record agree.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_reason_breaks_no_line_in_its_label_or_record() {
    let reason = "x\u{2028}\u{2029}\u{85}\u{202e}\u{2066}\u{200f}\u{61c}\t\nend";
    let runner = EngineRunner::open(&local_confinement(Some(reason))).expect("local opens");
    let label = runner.label();
    assert_eq!(
        label,
        "local (x\\u{2028}\\u{2029}\\u{85}\\u{202e}\\u{2066}\\u{200f}\\u{61c}\\u{9}\\u{a}end)"
    );
    let fixture = Fixture::new();
    let ran = runner
        .run(
            &TestCommand::new("true", Vec::<String>::new()),
            &fixture.workspace,
        )
        .expect("true runs");
    assert_eq!(
        ran.applied
            .local_reason
            .as_deref()
            .map(|recorded| format!("local ({recorded})")),
        Some(label)
    );
}

/// Letters, digits, spaces and punctuation of other scripts are no format characters: they pass.
#[test]
fn a_reason_in_another_script_passes_unchanged() {
    let reason = "Prüfung läuft — 測試を実行する、τεστ; проверка «да» ١٢٣ अभी? 시험!";
    assert_eq!(escape_reason(reason), reason);
    let runner = EngineRunner::open(&local_confinement(Some(reason))).expect("local opens");
    assert_eq!(runner.label(), format!("local ({reason})"));
}

/// A typed `\u{a}` and a real newline are two reasons, and the label and the record tell them
/// apart: the backslash is escaped too.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn two_reasons_never_escape_to_the_same_text() {
    let fixture = Fixture::new();
    let test = TestCommand::new("true", Vec::<String>::new());
    let mut seen = Vec::new();
    for reason in ["a\\u{a}b", "a\nb"] {
        let runner = EngineRunner::open(&local_confinement(Some(reason))).expect("local opens");
        let recorded = runner
            .run(&test, &fixture.workspace)
            .expect("true runs")
            .applied
            .local_reason
            .expect("the reason is recorded");
        seen.push((runner.label(), recorded));
    }
    assert_eq!(
        seen,
        [
            ("local (a\\\\u{a}b)".to_owned(), "a\\\\u{a}b".to_owned()),
            ("local (a\\u{a}b)".to_owned(), "a\\u{a}b".to_owned()),
        ]
    );
}

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
                .expect("an expected model request");
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
                    final_usage: true,
                    ..TurnObservation::new(self.provenance.clone())
                },
            })
        })
    }
}

struct GitFixture {
    _directory: tempfile::TempDir,
    workspace: PathBuf,
}

impl GitFixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let workspace = directory.path().canonicalize().unwrap().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        let fixture = Self {
            _directory: directory,
            workspace,
        };
        fixture.git(&["init", "--quiet", "--initial-branch=main"]);
        fixture.git(&["config", "user.name", "Fixture Author"]);
        fixture.git(&["config", "user.email", "fixture@example.invalid"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(fixture.workspace.join("check.txt"), "fixed\n").unwrap();
        fixture.git(&["add", "check.txt"]);
        fixture.git(&["commit", "--quiet", "-m", "initial check"]);
        fixture
    }

    fn git(&self, args: &[&str]) {
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
    }
}
