//! Acceptance for `story:run-cgroup-root-holds-the-root`: `b10x-loom run --cgroup-root R` holds
//! `R` the way `tree run` does, so two runs never share a root and `run` and `tree run` refuse each
//! other's root. A `resume` of a commission whose kept context names `R` holds it too, and a root
//! named on the command line is never re-executed into a scope of its own.
//!
//! The cases live in the crate's shared test binary, not in `tree_run`'s, whose starts of
//! `b10x-loom` are bounded (`START_BUDGETS` in `crates/loom-xtask/src/test_step.rs`): the binary's
//! hold is what these cases test, so each starts `b10x-loom`.
//!
//! They work as `tests/tree_run/delegated_cgroup.rs` does: a delegated scope of the user's systemd
//! manager (`fixtures/tree-delegated/scope.sh`) is the root `R`, `enter.sh` moves each command into
//! a child of it, and the `tree run` side is a `cell@1` tree whose stub waits for its
//! `target/release`. A case skips naming the missing prerequisite without bubblewrap or such a
//! scope, and fails instead when `LOOM_REQUIRE_DELEGATION` is set.
//!
//! A `run` that is to be refused gets no model: its `HOME` is an empty scratch directory, so a run
//! that reached a model call would fail on the missing route, never on the network. A `run` that is
//! to hold `R` drives `lab-wait@1`, one action bound to a POSIX sh command that waits for its
//! workspace's `target/release`, with the models of the fixture catalog
//! (`fixtures/catalog-route/catalog.toml`) at a loopback socket this file opens. No case reaches
//! the network or a credential, and each ends within 60 s.

use std::io::{BufRead as _, BufReader, ErrorKind, Read as _, Write as _};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{CommissionId, SuspensionState};
use b10x_loom_commission::ports::suspension::SuspensionPort;
use b10x_loom_intake_slice::clock::test_clock::VARIABLE as TEST_CLOCK;
use b10x_loom_intake_slice::resume::ResumeContextStore;
use b10x_loom_tree::{OpenRequest, TreeTime, open_tree};
use loom_governor::FileSuspensionStore;
use loom_protocols::InstallStore;
use serde_json::{Value, json};

const DELEGATED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tree-delegated");

const CATALOG: &str = include_str!("../fixtures/catalog-route/catalog.toml");

const BINARY: &str = env!("CARGO_BIN_EXE_b10x-loom");

/// Set, a missing prerequisite fails the test instead of skipping it.
const REQUIRE: &str = "LOOM_REQUIRE_DELEGATION";

/// The flags of a delegated scope of the user's systemd manager.
const SCOPE: [&str; 6] = ["--user", "--scope", "--quiet", "-p", "Delegate=yes", "--"];

/// The text of a refused hold (`hold_cgroup_root` in `crates/loom-intake-slice/src/confinement.rs`).
const HELD: &str = "held by another process";

/// The hint `run`'s preflight refusal gives before it starts itself again in a scope of its own.
const RUN_HINT: &str = "use systemd-run --user -p Delegate=yes --scope or --cgroup-root";

/// The bound of every test of this module.
const BOUND: Duration = Duration::from_secs(60);

/// The instant every write of a tree command records.
const AT: &str = "2026-11-22T08:00:00Z";

const INTENT: &str = "wait in the bound command";

/// One action, `wait.run`, whose evidence completes the case.
const PROTOCOL: &str = r"format: protocol/1
protocol:
  id: lab.wait
  revision: 1
  description: Wait in one bound command.
artifacts:
  intent:
    description: the request
evidence_kinds:
  waited:
    description: the bound command waited and ended
claims:
  wait.done:
    true_when:
      evidence:
        kind: waited
        result: done
        subject: intent
actions:
  wait.run:
    description: wait in the bound command
    effect: write
    may_produce:
      - evidence: waited
outcomes:
  done:
    description: the bound command ended
    requires:
      claim: wait.done
";

/// The bound command of `wait.run`: says it started, waits until `target/release` exists (at most
/// 30 s) and reports its evidence.
const WAIT: &str = "#!/bin/sh
cat > /dev/null
: > target/started
n=0
while [ ! -e target/release ]; do
  n=$((n + 1))
  [ \"$n\" -le 300 ] || exit 7
  sleep 0.1
done
printf '%s\\n' '{\"evidence\":[{\"kind\":\"waited\",\"result\":\"done\"}]}'
";

/// Skips the test naming `what`, or fails it when [`REQUIRE`] is set.
fn missing(what: &str) -> bool {
    assert!(
        std::env::var_os(REQUIRE).is_none(),
        "{REQUIRE} is set and a prerequisite is missing: {what}"
    );
    eprintln!("skipped: missing prerequisite: {what}");
    false
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn shown(output: &Output) -> String {
    format!(
        "status {:?}\n--- stdout\n{}--- stderr\n{}",
        output.status,
        stdout(output),
        stderr(output)
    )
}

/// A delegated scope a test holds: `scope.sh` waits in `keeper/` until it is dropped.
struct Scope {
    root: PathBuf,
    keeper: Child,
}

impl Scope {
    /// A fresh delegated scope, cpu, memory and pids handed to its children, into which this
    /// test may move a process; `None` when a prerequisite is missing.
    fn start() -> Option<Self> {
        if !Path::new("/usr/bin/bwrap").is_file() {
            missing("executable /usr/bin/bwrap");
            return None;
        }
        match Command::new("systemd-run").args(SCOPE).arg("true").output() {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                missing(&format!(
                    "a user systemd manager that starts a delegated scope (systemd-run {}: {})",
                    output.status,
                    stderr(&output).trim()
                ));
                return None;
            }
            Err(error) => {
                missing(&format!("systemd-run ({error})"));
                return None;
            }
        }
        let mut keeper = Command::new("systemd-run")
            .args(SCOPE)
            .arg("sh")
            .arg(Path::new(DELEGATED).join("scope.sh"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(keeper.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let root = PathBuf::from(line.trim());
        assert!(
            root.starts_with("/sys/fs/cgroup") && root.join("keeper").is_dir(),
            "scope.sh printed no scope: {line:?}"
        );
        let scope = Self { root, keeper };
        let probe = Command::new("sh")
            .arg(Path::new(DELEGATED).join("enter.sh"))
            .arg(scope.root.join("probe"))
            .arg("true")
            .output()
            .unwrap();
        let _ = std::fs::remove_dir(scope.root.join("probe"));
        if !probe.status.success() {
            missing(&format!(
                "a test process that may move a process into a scope of the user's manager, as under systemd-run --user --scope ({})",
                stderr(&probe).trim()
            ));
            return None;
        }
        Some(scope)
    }

    /// The root as a refusal names it.
    fn named(&self) -> String {
        self.root.display().to_string()
    }

    /// `b10x-loom`, moved into the child `cgroup` of this scope first.
    fn entered(&self, cgroup: &str) -> Command {
        let mut command = Command::new("sh");
        command
            .arg(Path::new(DELEGATED).join("enter.sh"))
            .arg(self.root.join(cgroup))
            .arg(BINARY)
            .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
            .env_remove("RUST_LOG");
        command
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        drop(self.keeper.stdin.take());
        let _ = self.keeper.wait();
    }
}

/// A child the test started, killed when the test ends before it did.
struct Running(Option<Child>);

impl Running {
    fn spawn(mut command: Command) -> Self {
        Self(Some(
            command
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        ))
    }

    fn child(&mut self) -> &mut Child {
        self.0.as_mut().unwrap()
    }

    /// Waits until `path` exists; fails when the child ends first or after 30 s.
    fn wait_for(&mut self, path: &Path, what: &str) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !path.exists() {
            if self.child().try_wait().unwrap().is_some() {
                let ended = self.finish();
                panic!(
                    "{what} ended before {} existed:\n{}",
                    path.display(),
                    shown(&ended)
                );
            }
            assert!(
                Instant::now() < deadline,
                "{what}: {} does not exist after 30 s",
                path.display()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn finish(&mut self) -> Output {
        self.0.take().unwrap().wait_with_output().unwrap()
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// One test's scratch directory under `CARGO_TARGET_TMPDIR`, removed on drop.
struct Scratch {
    root: PathBuf,
    started: Instant,
}

impl Scratch {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("run-holds-{label}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        Self {
            root,
            started: Instant::now(),
        }
    }

    /// A fresh directory `name` of the scratch directory.
    fn dir(&self, name: &str) -> PathBuf {
        let dir = self.root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The test clock file every `b10x-loom` of this test shares.
    fn clock(&self) -> PathBuf {
        self.root.join("test-clock")
    }

    /// A `cell@1` tree named `label`, opened in process: its home holds `cell@1`, its workspace
    /// the stub and `target/`.
    fn cell(&self, label: &str) -> Cell {
        let root = self.dir(label);
        let home = root.join("home");
        let workspace = root.join("workspace");
        std::fs::create_dir_all(workspace.join("stubs")).unwrap();
        std::fs::create_dir_all(workspace.join("target")).unwrap();
        let installed = InstallStore::new(home.join("loom/protocols"));
        installed
            .install_file("cell@1", &Path::new(DELEGATED).join("cell.yaml"), false)
            .unwrap();
        for file in ["cell.bindings.yaml", "cell.txt", "stubs/look.sh"] {
            std::fs::copy(Path::new(DELEGATED).join(file), workspace.join(file)).unwrap();
        }
        let catalog = installed.catalog().unwrap();
        let store = root.join("store");
        open_tree(&OpenRequest {
            store: store.clone(),
            workspace: workspace.clone(),
            protocol: "cell@1".to_owned(),
            bindings: workspace.join("cell.bindings.yaml"),
            revisions: Vec::new(),
            case: "root".to_owned(),
            time: TreeTime::At(AT.to_owned()),
            catalog: &catalog,
            protocols: Vec::new(),
        })
        .unwrap_or_else(|refused| panic!("tree open of {label}: {refused}"));
        Cell {
            home,
            workspace,
            store,
        }
    }

    /// A `run` workspace named `label` whose home holds `lab-wait@1` and whose bindings file binds
    /// `wait.run` to the waiting command.
    fn waiter(&self, label: &str) -> Waiter {
        let root = self.dir(label);
        let home = root.join("home");
        let workspace = root.join("workspace");
        std::fs::create_dir_all(workspace.join("target")).unwrap();
        let source = root.join("lab-wait.yaml");
        std::fs::write(&source, PROTOCOL).unwrap();
        InstallStore::new(home.join("loom/protocols"))
            .install_file("lab-wait@1", &source, false)
            .unwrap();
        let program = root.join("bin/wait.sh");
        std::fs::create_dir_all(program.parent().unwrap()).unwrap();
        std::fs::write(&program, WAIT).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let bindings = root.join("action-bindings.json");
        std::fs::write(
            &bindings,
            json!({
                "protocol": "lab-wait@1",
                "bindings": [{"action": "wait.run", "argv": [program]}],
            })
            .to_string(),
        )
        .unwrap();
        Waiter {
            root,
            home,
            workspace,
            bindings,
        }
    }

    /// A test's end within its bound.
    fn within_bound(&self) {
        let spent = self.started.elapsed();
        assert!(spent < BOUND, "the test took {spent:?}");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// An opened `cell@1` tree.
struct Cell {
    home: PathBuf,
    workspace: PathBuf,
    store: PathBuf,
}

impl Cell {
    fn started(&self) -> PathBuf {
        self.workspace.join("target/started")
    }

    /// Lets the tree's `look.sh` end.
    fn release(&self) {
        std::fs::write(self.workspace.join("target/release"), "").unwrap();
    }

    /// `tree run --cgroup-root` the scope's root, moved into its child `cgroup` first.
    fn run(&self, scope: &Scope, cgroup: &str) -> Command {
        let mut command = scope.entered(cgroup);
        command
            .env("HOME", &self.home)
            .env("XDG_DATA_HOME", &self.home)
            .args(["tree", "run", "--at", AT, "--store"])
            .arg(&self.store)
            .arg("--cgroup-root")
            .arg(&scope.root);
        command
    }

    /// Every file of the store and its bytes.
    fn files(&self) -> Vec<(PathBuf, Vec<u8>)> {
        every_file(&self.store)
    }
}

/// `output` ended 0 with the root terminated.
fn terminated(output: &Output) {
    assert_eq!(output.status.code(), Some(0), "{}", shown(output));
    let printed: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}", shown(output)));
    assert_eq!(printed["stop"], "Terminal", "{}", shown(output));
}

/// `output` refused `ConfinementRefused` with exit 1 before any step of `cell`, saying each of
/// `named`.
fn refused_before_any_step(cell: &Cell, output: &Output, named: &[&str]) {
    let said = stderr(output);
    assert_eq!(output.status.code(), Some(1), "{}", shown(output));
    assert!(said.contains("ConfinementRefused"), "{said}");
    for name in named {
        assert!(said.contains(name), "{name}: {said}");
    }
    assert!(!cell.started().exists(), "a step started: {said}");
}

/// A `run` of `lab-wait@1` that holds its root while its bound command waits.
struct Waiter {
    root: PathBuf,
    home: PathBuf,
    workspace: PathBuf,
    bindings: PathBuf,
}

impl Waiter {
    fn started(&self) -> PathBuf {
        self.workspace.join("target/started")
    }

    fn release(&self) {
        std::fs::write(self.workspace.join("target/release"), "").unwrap();
    }

    /// `run --cgroup-root` the scope's root, moved into its child `cgroup` first, its models the
    /// catalog at `base_url`.
    fn run(&self, scope: &Scope, cgroup: &str, base_url: &str, clock: &Path) -> Command {
        let catalog = self.root.join("catalog.toml");
        std::fs::write(&catalog, CATALOG.replace("http://127.0.0.1:1/v1", base_url)).unwrap();
        let mut command = scope.entered(cgroup);
        command
            .env("HOME", &self.home)
            .env("CODEX_HOME", &self.home)
            .env("XDG_DATA_HOME", &self.home)
            .env(TEST_CLOCK, clock)
            .args(["run", "--cgroup-root"])
            .arg(&scope.root)
            .arg("--workspace")
            .arg(&self.workspace)
            .arg("--catalog")
            .arg(&catalog)
            .args([
                "--classifier-model",
                "lab-classifier",
                "--model",
                "lab-agent",
                "--action-bindings",
            ])
            .arg(&self.bindings)
            .arg(INTENT);
        command
    }
}

/// The model answers of one `run` of `lab-wait@1`: the pick, the action and its arguments.
fn waiting_script() -> Vec<Vec<u8>> {
    vec![
        tool_call(
            "example/Classifier-Model",
            "pick_protocol",
            &json!({"protocol": "lab-wait@1", "confidence": 0.99, "reasons": ["a wait"]}),
        ),
        tool_call(
            "example/Agent-Model",
            "select_action",
            &json!({"action": "wait.run"}),
        ),
        tool_call("example/Agent-Model", "action_arguments", &json!({})),
    ]
}

/// A `run --cgroup-root` the scope's root with no model, moved into its child `cgroup` first: its
/// `HOME` is the empty directory `home`, so a model call would fail on the missing route.
fn modelless_run(scope: &Scope, cgroup: &str, home: &Path, workspace: &Path) -> Command {
    let mut command = scope.entered(cgroup);
    command
        .env("HOME", home)
        .env("CODEX_HOME", home)
        .env("XDG_DATA_HOME", home)
        .args(["run", "--cgroup-root"])
        .arg(&scope.root)
        .arg("--workspace")
        .arg(workspace)
        .arg(INTENT);
    command
}

/// `output` is a `run` refused the scope's root because another process holds it.
fn run_refused_held(scope: &Scope, output: &Output) {
    let said = stderr(output);
    assert!(!output.status.success(), "{}", shown(output));
    assert!(said.contains(&scope.named()), "{}", shown(output));
    assert!(said.contains(HELD), "{}", shown(output));
}

/// Every file under `dir` and its bytes, in path order.
fn every_file(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_owned()];
    while let Some(next) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                files.push((path, bytes));
            }
        }
    }
    files.sort();
    files
}

#[test]
fn run_is_refused_a_root_a_tree_run_holds() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let scratch = Scratch::new("tree-first");
    let cell = scratch.cell("tree");
    let mut tree = Running::spawn(cell.run(&scope, "tree"));
    tree.wait_for(&cell.started(), "the tree run");

    let home = scratch.dir("run-home");
    let workspace = scratch.dir("run-workspace");
    let refused = modelless_run(&scope, "run", &home, &workspace)
        .output()
        .unwrap();
    run_refused_held(&scope, &refused);
    assert!(
        every_file(&home).is_empty(),
        "the refused run wrote into its HOME: {:?}",
        every_file(&home)
    );

    cell.release();
    terminated(&tree.finish());
    scratch.within_bound();
}

#[test]
fn tree_run_is_refused_a_root_a_run_holds() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let scratch = Scratch::new("run-first");
    let waiter = scratch.waiter("run");
    let (base_url, server) = Script::serve(waiting_script());
    let mut run = Running::spawn(waiter.run(&scope, "run", &base_url, &scratch.clock()));
    run.wait_for(&waiter.started(), "the run");

    let cell = scratch.cell("tree");
    let before = cell.files();
    let refused = cell.run(&scope, "tree").output().unwrap();
    refused_before_any_step(&cell, &refused, &[&scope.named(), HELD]);
    assert_eq!(cell.files(), before, "the refused tree run wrote");

    waiter.release();
    let ran = run.finish();
    assert_eq!(ran.status.code(), Some(0), "{}", shown(&ran));
    assert!(
        stdout(&ran).contains("stopped: Completed (done)"),
        "{}",
        shown(&ran)
    );
    assert_eq!(
        server.finish(),
        ["pick_protocol", "select_action", "action_arguments"]
    );
    scratch.within_bound();
}

#[test]
fn two_runs_on_one_root_are_refused() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let scratch = Scratch::new("two-runs");
    let waiter = scratch.waiter("first");
    let (base_url, server) = Script::serve(waiting_script());
    let mut first = Running::spawn(waiter.run(&scope, "run-1", &base_url, &scratch.clock()));
    first.wait_for(&waiter.started(), "the first run");

    let home = scratch.dir("second-home");
    let workspace = scratch.dir("second-workspace");
    let refused = modelless_run(&scope, "run-2", &home, &workspace)
        .output()
        .unwrap();
    run_refused_held(&scope, &refused);
    assert!(
        every_file(&home).is_empty(),
        "the refused run wrote into its HOME: {:?}",
        every_file(&home)
    );

    waiter.release();
    let ran = first.finish();
    assert_eq!(ran.status.code(), Some(0), "{}", shown(&ran));
    assert_eq!(server.finish().len(), 3);
    scratch.within_bound();
}

#[test]
fn the_hold_of_run_is_released_on_exit_and_on_kill() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let scratch = Scratch::new("released");

    let ended = scratch.waiter("ended");
    ended.release();
    let (base_url, server) = Script::serve(waiting_script());
    let ran = ended
        .run(&scope, "run-1", &base_url, &scratch.clock())
        .output()
        .unwrap();
    assert_eq!(ran.status.code(), Some(0), "{}", shown(&ran));
    assert_eq!(server.finish().len(), 3);
    let after_exit = scratch.cell("after-exit");
    after_exit.release();
    terminated(&after_exit.run(&scope, "tree-1").output().unwrap());

    let killed = scratch.waiter("killed");
    let (base_url, server) = Script::serve(waiting_script());
    let mut run = Running::spawn(killed.run(&scope, "run-2", &base_url, &scratch.clock()));
    run.wait_for(&killed.started(), "the run to be killed");
    run.child().kill().unwrap();
    let status = run.finish().status;
    assert_eq!(
        std::os::unix::process::ExitStatusExt::signal(&status),
        Some(9),
        "{status:?}"
    );
    server.finish();
    let after_kill = scratch.cell("after-kill");
    after_kill.release();
    terminated(&after_kill.run(&scope, "tree-2").output().unwrap());
    killed.release();
    scratch.within_bound();
}

#[test]
fn a_named_root_is_never_reexecuted() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let scratch = Scratch::new("never-reexecuted");
    let cell = scratch.cell("tree");
    let mut tree = Running::spawn(cell.run(&scope, "tree"));
    tree.wait_for(&cell.started(), "the tree run");

    let stubs = scratch.dir("stubs");
    let marker = scratch.root.join("systemd-run-started");
    let stub = stubs.join("systemd-run");
    std::fs::write(
        &stub,
        format!("#!/bin/sh\n: > '{}'\nexit 1\n", marker.display()),
    )
    .unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::var("PATH").unwrap_or_default();

    let home = scratch.dir("run-home");
    let workspace = scratch.dir("run-workspace");
    let refused = modelless_run(&scope, "run", &home, &workspace)
        .env("PATH", format!("{}:{path}", stubs.display()))
        .output()
        .unwrap();
    run_refused_held(&scope, &refused);
    assert!(!stderr(&refused).contains(RUN_HINT), "{}", shown(&refused));
    assert!(
        !marker.exists(),
        "run started systemd-run: {}",
        shown(&refused)
    );

    cell.release();
    terminated(&tree.finish());
    scratch.within_bound();
}

/// The workspace's one failing check, which the agent fixes.
const TEST_CMD: &str = "grep -qx fixed check.txt";

#[test]
fn resume_is_refused_a_root_a_tree_run_holds() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let scratch = Scratch::new("resume");
    let home = scratch.dir("home");
    let workspace = scratch.dir("workspace");
    let state = scratch.root.join("state");
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(&workspace)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {}", shown(&output));
    };
    git(&["init", "--quiet", "--initial-branch=main"]);
    git(&["config", "user.name", "Fixture Author"]);
    git(&["config", "user.email", "fixture@example.invalid"]);
    git(&["config", "commit.gpgsign", "false"]);
    std::fs::write(workspace.join("check.txt"), "broken\n").unwrap();
    git(&["add", "--all"]);
    git(&["commit", "--quiet", "--message", "a failing check"]);

    let catalog = scratch.root.join("catalog.toml");
    let command = |cgroup: Option<&str>| {
        let mut command = match cgroup {
            Some(cgroup) => scope.entered(cgroup),
            None => {
                let mut command = Command::new(BINARY);
                command
                    .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
                    .env_remove("RUST_LOG");
                command
            }
        };
        command
            .env("HOME", &home)
            .env("CODEX_HOME", &home)
            .env("XDG_DATA_HOME", &home)
            .env(TEST_CLOCK, scratch.clock());
        command
    };
    let agent = |name: &str, arguments: Value| tool_call("example/Agent-Model", name, &arguments);

    // A run on the root leaves the commission suspended at the merge's gate.
    let (base_url, server) = Script::serve(vec![
        tool_call(
            "example/Classifier-Model",
            "pick_protocol",
            &json!({"protocol": "software-change@1", "confidence": 0.99,
                "reasons": ["changes the repository"]}),
        ),
        agent("select_action", json!({"action": "implementation.edit"})),
        agent(
            "action_arguments",
            json!({"files": [{"path": "check.txt", "contents": "fixed\n"}],
                "message": "fix the check"}),
        ),
        agent("select_action", json!({"action": "tests.run"})),
        agent("action_arguments", json!({})),
        agent("select_action", json!({"action": "repository.merge"})),
        agent("action_arguments", json!({})),
    ]);
    std::fs::write(
        &catalog,
        CATALOG.replace("http://127.0.0.1:1/v1", &base_url),
    )
    .unwrap();
    let ran = command(Some("run"))
        .args(["run", "--state"])
        .arg(&state)
        .arg("--cgroup-root")
        .arg(&scope.root)
        .arg("--workspace")
        .arg(&workspace)
        .args(["--test-cmd", TEST_CMD, "--max-steps", "3", "--catalog"])
        .arg(&catalog)
        .args([
            "--classifier-model",
            "lab-classifier",
            "--model",
            "lab-agent",
            "Make the failing check pass.",
        ])
        .output()
        .unwrap();
    server.finish();
    assert_eq!(ran.status.code(), Some(0), "{}", shown(&ran));
    assert_eq!(
        stdout(&ran).lines().last(),
        Some("stopped: ApprovalRequired (repository.merge)"),
        "{}",
        shown(&ran)
    );
    let contexts = ResumeContextStore::open(&state).unwrap().list().unwrap();
    assert_eq!(contexts.len(), 1, "{contexts:?}");
    assert_eq!(
        contexts[0].cgroup_root.as_deref(),
        Some(scope.named().as_str()),
        "the kept context names the root"
    );
    let commission = contexts[0].commission_id.0.0.clone();
    let suspension = || {
        FileSuspensionStore::open(&state)
            .unwrap()
            .get(&CommissionId(Uuid(commission.clone())))
            .unwrap()
            .map(|held| held.state)
    };
    assert_eq!(suspension(), Some(SuspensionState::Pending));
    let approved = command(None)
        .args(["approve", &commission, "repository.merge", "--state"])
        .arg(&state)
        .output()
        .unwrap();
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));

    // A tree run then holds the root.
    let cell = scratch.cell("tree");
    let mut tree = Running::spawn(cell.run(&scope, "tree"));
    tree.wait_for(&cell.started(), "the tree run");

    let (base_url, server) = Script::serve(Vec::new());
    std::fs::write(
        &catalog,
        CATALOG.replace("http://127.0.0.1:1/v1", &base_url),
    )
    .unwrap();
    let refused = command(Some("resume"))
        .args(["resume", &commission, "--state"])
        .arg(&state)
        .output()
        .unwrap();
    let calls = server.finish();
    run_refused_held(&scope, &refused);
    assert!(
        calls.is_empty(),
        "the refused resume called a model: {calls:?}"
    );
    assert_eq!(
        suspension(),
        Some(SuspensionState::Pending),
        "the Suspension is still held"
    );
    let merges: Vec<_> = std::fs::read_dir(&state)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("merge-"))
        .collect();
    assert!(merges.is_empty(), "the refused resume handed off a merge");

    cell.release();
    terminated(&tree.finish());
    scratch.within_bound();
}

/// One streamed Chat Completions answer calling `tool` with `arguments`.
fn tool_call(model: &str, tool: &str, arguments: &Value) -> Vec<u8> {
    let chunk = |delta: Value, finish: Value| {
        json!({
            "id": format!("chatcmpl-call-{tool}"),
            "object": "chat.completion.chunk",
            "model": model,
            "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]
        })
    };
    let events = [
        chunk(
            json!({"role": "assistant", "tool_calls": [{"index": 0, "id": format!("call-{tool}"),
                "type": "function", "function": {"name": tool, "arguments": ""}}]}),
            Value::Null,
        ),
        chunk(
            json!({"tool_calls": [{"index": 0, "function": {"arguments": arguments.to_string()}}]}),
            Value::Null,
        ),
        chunk(json!({}), json!("tool_calls")),
        json!({
            "id": format!("chatcmpl-call-{tool}"),
            "object": "chat.completion.chunk",
            "model": model,
            "choices": [],
            "usage": {"prompt_tokens": 120, "completion_tokens": 7, "total_tokens": 127}
        }),
    ];
    let mut stream = String::new();
    for event in events {
        stream.push_str(&format!("data: {event}\n\n"));
    }
    stream.push_str("data: [DONE]\n\n");
    stream.into_bytes()
}

/// Answers one request per scripted response, in order, on a loopback socket, until told to
/// finish; records the forced tool each request named. A request past the script is answered
/// with an error status and recorded, so the case sees it.
struct Script {
    done: Arc<AtomicBool>,
    handle: JoinHandle<Vec<String>>,
}

impl Script {
    fn serve(responses: Vec<Vec<u8>>) -> (String, Self) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let done = Arc::new(AtomicBool::new(false));
        let finished = Arc::clone(&done);
        let handle = std::thread::spawn(move || {
            let mut sent = Vec::new();
            let mut responses = responses.into_iter();
            while !finished.load(Ordering::SeqCst) {
                let mut socket = match listener.accept() {
                    Ok((socket, _)) => socket,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(error) => panic!("accept: {error}"),
                };
                socket.set_nonblocking(false).unwrap();
                let request = read_request(&mut socket);
                sent.push(tool_named(&request));
                let answer = match responses.next() {
                    Some(response) => {
                        let mut answer = Vec::from(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                        );
                        answer.extend_from_slice(&response);
                        answer
                    }
                    None => Vec::from(
                        "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ),
                };
                let _ = socket.write_all(&answer);
                let _ = socket.shutdown(std::net::Shutdown::Both);
            }
            sent
        });
        (base_url, Self { done, handle })
    }

    /// Stops serving and returns the tool each request forced, in order.
    fn finish(self) -> Vec<String> {
        self.done.store(true, Ordering::SeqCst);
        self.handle.join().expect("the script finished")
    }
}

fn read_request(socket: &mut std::net::TcpStream) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = socket.read(&mut buffer).expect("read");
        request.extend_from_slice(&buffer[..read]);
        if read == 0 || complete(&request) {
            break;
        }
    }
    String::from_utf8_lossy(&request).into_owned()
}

/// Whether `request` holds its whole body, by its `Content-Length`.
fn complete(request: &[u8]) -> bool {
    let text = String::from_utf8_lossy(request);
    text.split_once("\r\n\r\n").is_some_and(|(head, rest)| {
        head.to_ascii_lowercase()
            .split_once("content-length:")
            .and_then(|(_, tail)| tail.split('\r').next())
            .and_then(|value| value.trim().parse::<usize>().ok())
            .is_some_and(|length| rest.len() >= length)
    })
}

/// The forced tool a request names.
fn tool_named(request: &str) -> String {
    let body = request.split_once("\r\n\r\n").map_or("", |(_, body)| body);
    serde_json::from_str::<Value>(body).map_or_else(
        |_| String::new(),
        |body| {
            body["tool_choice"]["function"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        },
    )
}
