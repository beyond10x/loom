//! Acceptance for `story:tree-run-command`: `b10x-loom tree` opens, verifies and prints a tree of
//! protocol cases kept in a store directory.
//!
//! The fixture is `fixtures/tree-two-level/`, written for these tests and shaped like a parent and
//! one child: the protocols `tour` (`tour@1`) and `tour.leg` (`tour-leg@1`), their bindings files
//! and stub programs that end at once. Each test installs the two protocols into its own scratch
//! user's store (in process, a few milliseconds), copies the stubs into a fresh workspace and runs
//! the binary with `HOME` and `XDG_DATA_HOME` at that scratch directory; no test reaches the
//! network, a model or a credential, and each asserts it ended within 60 s.
//!
//! Each `b10x-loom` runs with the test sync switch on (`B10X_LOOM_TEST_UNSYNCED_STORE=1`, read only
//! by a build with the feature `test-unsynced-store` of `b10x-loom-governor`), so no store write
//! waits for the disk (`story:tree-run-tests-fit-the-budget`): what these tests prove is the binary's
//! arguments, exit codes and decisions, and the durability tests of the stores keep full sync.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
use b10x_loom_intake_slice::confinement::{ConfinementAdapter, EngineConfinement, EngineRunner};
use b10x_loom_tree::model::{SpentBudget, TreeRefusalReason, TreeStop};
use b10x_loom_tree::runlog::{RunLogEvent, RunLogLine};
use b10x_loom_tree::{
    RunSettings, STATUS_FORMAT, TreeTime, decision_json, log_file_name, read_log, read_status,
    run_tree, status_json, tree_status,
};
use loom_protocols::{InstallStore, ProtocolCatalog};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

#[path = "tree_run/delegated_cgroup.rs"]
mod delegated_cgroup;
#[path = "tree_run/handoff_when.rs"]
mod handoff_when;

#[path = "tree_run/principal.rs"]
mod principal;
#[path = "tree_run/signals.rs"]
mod signals;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tree-two-level");

/// The bound of every test of this file.
const BOUND: Duration = Duration::from_secs(60);

/// The instant every write of a command records in these tests.
const AT: &str = "2026-11-22T08:00:00Z";

/// The file a `b10x-loom` built for tests appends its sync counts to (`loom_governor::durable`).
const SYNC_COUNTS: &str = "B10X_LOOM_TEST_SYNC_COUNTS";

thread_local! {
    /// The value of the test sync switch on each `b10x-loom` the test starts: `1`, so no store
    /// write of the binary syncs (`story:tree-run-tests-fit-the-budget`), unless [`counted`] sets
    /// another, or `None` for none.
    static SWITCH_VALUE: RefCell<Option<String>> = RefCell::new(Some("1".to_owned()));
    /// The file each `b10x-loom` the test starts appends its sync counts to, while [`counted`] runs.
    static COUNTS_FILE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Gives `command`, a start of `b10x-loom` by this binary, the test sync switch and the counts
/// file of this thread. Every start of `b10x-loom` in the `tree_run` tests goes through here.
fn with_the_switch(command: &mut Command) -> &mut Command {
    match SWITCH_VALUE.with_borrow(Clone::clone) {
        Some(value) => command.env("B10X_LOOM_TEST_UNSYNCED_STORE", value),
        None => command.env_remove("B10X_LOOM_TEST_UNSYNCED_STORE"),
    };
    match COUNTS_FILE.with_borrow(Clone::clone) {
        Some(file) => command.env(SYNC_COUNTS, file),
        None => command.env_remove(SYNC_COUNTS),
    };
    command
}

/// One test's scratch directory: the user's home with the fixture protocols installed, the
/// workspace with the stubs and the bindings files, and the path of the store.
struct Tree {
    root: PathBuf,
    started: Instant,
    /// A shared fixture other tests read: never removed.
    shared: bool,
}

impl Tree {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self::at(
            PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
                .join(format!("tree-run-{label}-{}-{nanos}", std::process::id())),
        )
    }

    /// A fresh scratch directory at `root`.
    fn at(root: PathBuf) -> Self {
        let started = Instant::now();
        fs::create_dir_all(root.join("home")).unwrap();
        fs::create_dir_all(root.join("workspace/stubs")).unwrap();
        let installed = InstallStore::new(root.join("home/loom/protocols"));
        for (name, file) in [
            ("tour@1", "tour.yaml"),
            ("tour-leg@1", "tour-leg.yaml"),
            ("fork@1", "../extra/fork.yaml"),
            ("sign@1", "../extra/sign.yaml"),
            ("review@1", "../extra/review.yaml"),
        ] {
            installed
                .install_file(
                    name,
                    &Path::new(FIXTURE).join("protocols").join(file),
                    false,
                )
                .unwrap();
        }
        for entry in fs::read_dir(Path::new(FIXTURE).join("stubs")).unwrap() {
            let entry = entry.unwrap();
            fs::copy(
                entry.path(),
                root.join("workspace/stubs").join(entry.file_name()),
            )
            .unwrap();
        }
        for entry in fs::read_dir(FIXTURE).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                fs::copy(entry.path(), root.join("workspace").join(entry.file_name())).unwrap();
            }
        }
        Self {
            root,
            started,
            shared: false,
        }
    }

    /// The catalog of the test's user: the bundled protocols and the two installed.
    fn catalog(&self) -> ProtocolCatalog {
        InstallStore::new(self.root.join("home/loom/protocols"))
            .catalog()
            .unwrap()
    }

    fn store(&self) -> PathBuf {
        self.root.join("store")
    }

    fn workspace(&self) -> PathBuf {
        self.root.join("workspace")
    }

    /// `b10x-loom` for this test: its home and data directory the test's, the test sync switch on
    /// unless [`counted`] says otherwise, and its sync counts written where [`counted`] reads them.
    fn command(&self) -> Command {
        let home = self.root.join("home");
        let mut command = Command::new(env!("CARGO_BIN_EXE_b10x-loom"));
        command
            .env("HOME", &home)
            .env("XDG_DATA_HOME", &home)
            .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
            .env_remove("RUST_LOG");
        with_the_switch(&mut command);
        command
    }

    /// `tree open` of `tour@1` with the bindings file `bindings` of the workspace into `store`.
    fn open_into(&self, store: &Path, bindings: &str) -> Output {
        self.command()
            .args([
                "tree",
                "open",
                "--protocol",
                "tour@1",
                "--at",
                AT,
                "--store",
            ])
            .arg(store)
            .arg("--bindings")
            .arg(self.workspace().join(bindings))
            .arg("--workspace")
            .arg(self.workspace())
            .output()
            .unwrap()
    }

    fn open(&self) -> Output {
        self.open_into(&self.store(), "tour.bindings.yaml")
    }

    /// `tree open` of the child protocol `tour-leg@1` alone, as a tree of one case.
    fn open_leg(&self) -> Output {
        self.open_leg_with("tour-leg.bindings.yaml")
    }

    /// `tree open` of `tour-leg@1` alone with the bindings file `bindings` of the workspace.
    fn open_leg_with(&self, bindings: &str) -> Output {
        self.command()
            .args(["tree", "open", "--protocol", "tour-leg@1", "--at", AT])
            .arg("--store")
            .arg(self.store())
            .arg("--bindings")
            .arg(self.workspace().join(bindings))
            .arg("--workspace")
            .arg(self.workspace())
            .output()
            .unwrap()
    }

    /// `tree run` on the store, every program through the `local` runner.
    fn run(&self) -> Output {
        self.command()
            .args([
                "tree",
                "run",
                "--confinement",
                "local",
                "--at",
                AT,
                "--store",
            ])
            .arg(self.store())
            .output()
            .unwrap()
    }

    /// Runs the command a waiting `tree run` printed, from its JSON.
    fn decided(&self, decision: &Value) -> Output {
        let words: Vec<String> = decision["command"]
            .as_array()
            .unwrap()
            .iter()
            .map(|word| word.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(words[0], "b10x-loom", "{decision}");
        self.command()
            .args(&words[1..])
            .args(["--at", AT])
            .output()
            .unwrap()
    }

    /// Runs the command a waiting `tree run` printed exactly as printed: no flag added.
    fn printed(&self, decision: &Value) -> Output {
        let words: Vec<String> = decision["command"]
            .as_array()
            .unwrap()
            .iter()
            .map(|word| word.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(words[0], "b10x-loom", "{decision}");
        self.command().args(&words[1..]).output().unwrap()
    }

    fn tree(&self, verb: &str) -> Output {
        self.command()
            .args(["tree", verb, "--store"])
            .arg(self.store())
            .output()
            .unwrap()
    }

    /// Every file of the store and its bytes, by name.
    fn snapshot(&self) -> Vec<(String, Vec<u8>)> {
        snapshot(&self.store())
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let took = self.started.elapsed();
        if !std::thread::panicking() {
            assert!(took < BOUND, "the test took {took:?}, past its {BOUND:?}");
        }
        if !self.shared {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn snapshot(store: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<(String, Vec<u8>)> = match fs::read_dir(store) {
        Ok(entries) => entries
            .map(|entry| {
                let entry = entry.unwrap();
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    fs::read(entry.path()).unwrap_or_default(),
                )
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    files.sort();
    files
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}{}", stdout(output), stderr(output)))
}

#[test]
fn an_agent_binding_is_refused_before_anything_runs() {
    let tree = Tree::new("agent");
    fs::create_dir_all(tree.store()).unwrap();
    let output = tree.open_into(&tree.store(), "tour-agent.bindings.yaml");
    let said = stderr(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(said.contains("AgentNotRunYet"), "{said}");
    assert!(said.contains("tour-leg-agent.bindings.yaml"), "{said}");
    assert!(said.contains("route.write"), "{said}");
    assert!(said.contains("story:bound-agent-node"), "{said}");
    assert_eq!(
        fs::read_dir(tree.store()).unwrap().count(),
        0,
        "the store is not empty"
    );
}

#[test]
fn tree_open_keeps_the_root_case_and_tree_status_names_its_files() {
    let tree = Tree::new("status");
    let opened = tree.open();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let opened = json(&opened);
    assert_eq!(opened["root"], "root");
    assert_eq!(opened["protocol"], "tour@1");

    let status = tree.tree("status");
    assert_eq!(status.status.code(), Some(0), "{}", stderr(&status));
    let status = json(&status);
    let cases = status["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 1, "{status}");
    assert_eq!(cases[0]["case"], "root");
    assert_eq!(cases[0]["protocol"], "tour@1");
    assert_eq!(cases[0]["revision"], 1);
    for file in ["case_file", "run_log"] {
        let path = PathBuf::from(cases[0][file].as_str().unwrap());
        assert!(path.is_file(), "{file} {} does not exist", path.display());
        assert!(path.starts_with(fs::canonicalize(tree.store()).unwrap()));
    }
    let log = fs::read_to_string(cases[0]["run_log"].as_str().unwrap()).unwrap();
    assert_eq!(log.lines().count(), 1, "{log}");
    assert!(log.contains(r#""kind":"CaseOpened""#), "{log}");
    assert!(log.contains(AT), "{log}");
}

#[test]
fn tree_verify_names_the_case_and_line_of_a_changed_byte() {
    let tree = Tree::new("verify");
    let opened = tree.open();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let verified = tree.tree("verify");
    assert_eq!(verified.status.code(), Some(0), "{}", stderr(&verified));
    assert_eq!(json(&verified)["cases"], 1);

    // The log's only line is its last: the chain names no digest of it, the head does.
    let status = json(&tree.tree("status"));
    let log = PathBuf::from(status["cases"][0]["run_log"].as_str().unwrap());
    let mut bytes = fs::read(&log).unwrap();
    let at = bytes
        .windows(4)
        .position(|window| window == b"tour")
        .unwrap();
    bytes[at] = b'T';
    fs::write(&log, bytes).unwrap();
    let verified = tree.tree("verify");
    let said = stderr(&verified);
    assert_eq!(verified.status.code(), Some(3), "{said}");
    assert!(said.contains("case root line 0"), "{said}");
    assert_eq!(json(&verified)["failure"]["line"], 0);
}

#[test]
fn a_second_writer_is_refused_store_in_use_naming_the_holder() {
    let tree = Tree::new("in-use");
    fs::create_dir_all(tree.store()).unwrap();
    fs::write(tree.store().join(b10x_loom_tree::LOCK_FILE), b"").unwrap();
    let held = b10x_loom_tree::StoreLock::acquire(&tree.store()).unwrap();
    let before = tree.snapshot();
    let output = tree.open();
    let said = stderr(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(said.contains("StoreInUse"), "{said}");
    assert!(
        said.contains(&format!("held by process {}", std::process::id())),
        "{said}"
    );
    assert_eq!(
        tree.snapshot(),
        before,
        "the refused writer wrote into the store"
    );
    drop(held);
}

#[test]
fn an_approval_is_recorded_for_one_case_one_capability_and_its_revision() {
    let tree = Tree::new("approve");
    let opened = tree.open();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let approve = |case: &str| {
        tree.command()
            .args(["tree", "approve", "--at", AT, "--capability", "guide.issue"])
            .args(["--principal", "guide", "--case", case, "--store"])
            .arg(tree.store())
            .output()
            .unwrap()
    };
    let before = tree.snapshot();
    let refused = approve("root.elsewhere");
    assert_eq!(refused.status.code(), Some(1), "{}", stderr(&refused));
    assert!(
        stderr(&refused).contains("CaseNotInTree"),
        "{}",
        stderr(&refused)
    );
    assert_eq!(
        tree.snapshot(),
        before,
        "a refused approval wrote into the store"
    );

    let approved = approve("root");
    assert_eq!(approved.status.code(), Some(0), "{}", stderr(&approved));
    let approval = json(&approved);
    assert_eq!(approval["case"], "root");
    assert_eq!(approval["capability"], "guide.issue");
    assert_eq!(approval["decision"], "Allow");
    assert_eq!(approval["principal"], "guide");
    assert_eq!(approval["case_revision"], 1);
    let approvals = b10x_loom_tree::read_approvals(&tree.store(), "root").unwrap();
    assert_eq!(approvals.len(), 1, "{approvals:?}");
    let status = json(&tree.tree("status"));
    let log = fs::read_to_string(status["cases"][0]["run_log"].as_str().unwrap()).unwrap();
    assert!(
        log.lines()
            .nth(1)
            .unwrap()
            .contains(r#""kind":"AuthorityRecorded""#),
        "{log}"
    );
    let verified = tree.tree("verify");
    assert_eq!(verified.status.code(), Some(0), "{}", stderr(&verified));
}

#[test]
fn two_opens_with_the_same_at_write_the_same_bytes() {
    let tree = Tree::new("same-bytes");
    let first = tree.root.join("first");
    let second = tree.root.join("second");
    for store in [&first, &second] {
        let opened = tree.open_into(store, "tour.bindings.yaml");
        assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    }
    let names: Vec<String> = snapshot(&first).into_iter().map(|(name, _)| name).collect();
    assert!(
        names.iter().any(|name| name.starts_with("runlog-")),
        "{names:?}"
    );
    assert_eq!(snapshot(&first), snapshot(&second));
}

#[test]
fn a_one_case_tree_runs_through_its_waits_to_its_outcome() {
    let tree = Tree::new("one-case");
    let opened = tree.open_leg();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));

    // The writer runs until the check is valid; the approval of the route is a person's.
    let first = tree.run();
    assert_eq!(
        first.status.code(),
        Some(3),
        "{}{}",
        stdout(&first),
        stderr(&first)
    );
    let waited = json(&first);
    assert_eq!(waited["stop"], "AwaitingPerson", "{waited}");
    let decision = &waited["decisions"][0];
    assert_eq!(decision["action"], "route.approve", "{waited}");
    assert_eq!(decision["kind"], "ApproveAction", "{waited}");
    assert_eq!(
        fs::read_to_string(tree.workspace().join("route.txt")).unwrap(),
        "stop\nstop\n"
    );
    let approved = tree.decided(decision);
    assert_eq!(approved.status.code(), Some(0), "{}", stderr(&approved));

    // The ticket waits on its capability, then is issued and the case ends through `passed`.
    let second = tree.run();
    assert_eq!(
        second.status.code(),
        Some(3),
        "{}{}",
        stdout(&second),
        stderr(&second)
    );
    let waited = json(&second);
    assert_eq!(waited["stop"], "AwaitingAuthority", "{waited}");
    let decision = &waited["decisions"][0];
    assert_eq!(decision["capability"], "guide.issue", "{waited}");
    let approved = tree.decided(decision);
    assert_eq!(approved.status.code(), Some(0), "{}", stderr(&approved));

    let last = tree.run();
    assert_eq!(
        last.status.code(),
        Some(0),
        "{}{}",
        stdout(&last),
        stderr(&last)
    );
    let ended = json(&last);
    assert_eq!(ended["stop"], "Terminal", "{ended}");
    assert_eq!(ended["detail"], "passed", "{ended}");
    let verified = tree.tree("verify");
    assert_eq!(verified.status.code(), Some(0), "{}", stderr(&verified));
}

#[test]
fn a_childs_wait_stops_the_run_naming_the_child_case() {
    let tree = Tree::new("child-wait");
    let opened = tree.open();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));

    // The root opens its first leg, which runs until its route waits on the guide.
    let first = tree.run();
    assert_eq!(
        first.status.code(),
        Some(3),
        "{}{}",
        stdout(&first),
        stderr(&first)
    );
    let waited = json(&first);
    assert_eq!(waited["stop"], "AwaitingPerson", "{waited}");
    let child = waited["case"].as_str().unwrap().to_owned();
    assert_ne!(child, "root", "{waited}");
    let decision = &waited["decisions"][0];
    assert_eq!(decision["case"], child.as_str(), "{waited}");
    let command: Vec<&str> = decision["command"]
        .as_array()
        .unwrap()
        .iter()
        .map(|word| word.as_str().unwrap())
        .collect();
    assert_eq!(&command[..3], ["b10x-loom", "tree", "approve"], "{waited}");
    let at = command.iter().position(|word| *word == "--case").unwrap();
    assert_eq!(command[at + 1], child, "{waited}");

    // The status lists the child below the root, and the root logs its opening.
    let status = tree.tree("status");
    assert_eq!(status.status.code(), Some(0), "{}", stderr(&status));
    let status = json(&status);
    let cases = status["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 2, "{status}");
    assert_eq!(cases[1]["case"], child.as_str(), "{status}");
    assert_eq!(cases[1]["parent"], "root", "{status}");
    assert_eq!(cases[1]["protocol"], "tour-leg@1", "{status}");
    let root_log = fs::read_to_string(cases[0]["run_log"].as_str().unwrap()).unwrap();
    assert!(root_log.contains("ChildOpened"), "{root_log}");
    let verified = tree.tree("verify");
    assert_eq!(verified.status.code(), Some(0), "{}", stderr(&verified));
}

/// The settings of a run through the library: the `local` runner, the instant `AT`, `max_steps`.
fn settings(max_steps: Option<i64>) -> RunSettings {
    let runner = EngineRunner::open(&EngineConfinement {
        adapter: ConfinementAdapter::Local,
        local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
        cgroup_root: None,
    })
    .unwrap();
    RunSettings {
        max_steps,
        max_wall_seconds: None,
        time: TreeTime::At(AT.to_owned()),
        runner: Arc::new(runner),
        confinement: Some("local".to_owned()),
        cgroup_root: None,
    }
}

/// The lines of `case`'s log in the store of `tree`.
fn log_lines(tree: &Tree, case: &str) -> Vec<RunLogLine> {
    let bytes = fs::read(tree.store().join(log_file_name(case))).unwrap();
    read_log(&bytes).unwrap()
}

#[test]
fn an_observer_is_given_each_run_log_line_right_after_it_is_written() {
    let tree = Tree::new("observer");
    let opened = tree.open_leg();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let catalog = tree.catalog();
    let store = tree.store();
    let mut seen: Vec<(String, RunLogLine)> = Vec::new();
    let mut observer = |case: &str, line: &RunLogLine| {
        // The line is in the log when the observer is given it.
        let written = log_lines(&tree, case);
        assert_eq!(written.last(), Some(line));
        seen.push((case.to_owned(), line.clone()));
    };
    let ran = run_tree(
        &store,
        &catalog,
        &settings(Some(3)),
        Some(&mut observer),
        None,
    )
    .unwrap();
    assert_eq!(ran.stop, TreeStop::BudgetSpent, "{ran:?}");
    let written = log_lines(&tree, "root");
    // Every line the run wrote, in order: all but the `case opened` line `tree open` wrote.
    let observed: Vec<RunLogLine> = seen.iter().map(|(_, line)| line.clone()).collect();
    assert_eq!(observed, written[1..].to_vec());
    assert!(seen.iter().all(|(case, _)| case == "root"));
}

#[test]
fn a_cancelled_run_ends_cancelled_before_its_next_step() {
    let tree = Tree::new("cancel");
    let opened = tree.open_leg();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let catalog = tree.catalog();
    let cancel = AtomicBool::new(false);
    // The handle is set once the first step finished; the run asks it before the next.
    let mut observer = |_: &str, line: &RunLogLine| {
        if matches!(line.event, RunLogEvent::ActionFinished(_)) {
            cancel.store(true, Ordering::SeqCst);
        }
    };
    let ran = run_tree(
        &tree.store(),
        &catalog,
        &settings(None),
        Some(&mut observer),
        Some(&cancel),
    )
    .unwrap();
    assert_eq!(ran.stop, TreeStop::Cancelled, "{ran:?}");
    assert_eq!(ran.steps, 1, "{ran:?}");
    let written = log_lines(&tree, "root");
    let last = written.last().unwrap();
    assert!(
        matches!(&last.event, RunLogEvent::Stopped(stopped) if stopped.stop == "Cancelled" && stopped.budget.is_none()),
        "{last:?}"
    );
    let finished = written
        .iter()
        .filter(|line| matches!(line.event, RunLogEvent::ActionFinished(_)))
        .count();
    assert_eq!(finished, 1);
    // The first step checked the route; the writer, the next step, never ran.
    assert!(!tree.workspace().join("route.txt").exists());
}

/// `tree open` of the two-level tree, then `tree run` and each first printed decision in turn
/// until a run exits other than 3, taking the decision on `twice` two times in a row: per wait the
/// run's JSON and the status read at it, and the last run's JSON.
fn run_to_end_twice(tree: &Tree, twice: Option<&str>) -> (Vec<(Value, Value)>, Value) {
    let opened = tree.open();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    run_opened(tree, twice)
}

/// `tree run` and each first printed decision in turn on a tree already open, as [`run_to_end_twice`].
fn run_opened(tree: &Tree, twice: Option<&str>) -> (Vec<(Value, Value)>, Value) {
    let mut waits = Vec::new();
    for _ in 0..8 {
        let ran = tree.run();
        if ran.status.code() != Some(3) {
            assert_eq!(
                ran.status.code(),
                Some(0),
                "{}{}",
                stdout(&ran),
                stderr(&ran)
            );
            return (waits, json(&ran));
        }
        let waited = json(&ran);
        let status = json(&tree.tree("status"));
        let decided = tree.decided(&waited["decisions"][0]);
        assert_eq!(decided.status.code(), Some(0), "{}", stderr(&decided));
        if twice.is_some_and(|action| waited["decisions"][0]["action"] == action) {
            let again = tree.decided(&waited["decisions"][0]);
            assert_eq!(again.status.code(), Some(0), "{}", stderr(&again));
        }
        waits.push((waited, status));
    }
    panic!("the tree did not end within 8 runs: {waits:?}");
}

/// The two-level tree run to its end, shared by the status tests: its store and workspace, each
/// wait with the status read at it, and the last run's JSON.
struct Ended {
    tree: Tree,
    waits: Vec<(Value, Value)>,
    ended: Value,
}

/// The SHA-256, as hex, of what the shared ended tree is made from: every file of the fixture
/// directory (its path, its length and its bytes, in path order), the `b10x-loom` binary and this
/// test binary, whose helpers drive the run.
fn ended_digest() -> String {
    use sha2::{Digest, Sha256};
    fn files(dir: &Path, found: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                files(&entry.path(), found);
            } else {
                found.push(entry.path());
            }
        }
    }
    let mut found = Vec::new();
    files(Path::new(FIXTURE), &mut found);
    found.sort();
    let mut hasher = Sha256::new();
    for file in &found {
        let bytes = fs::read(file).unwrap();
        hasher.update(
            file.strip_prefix(FIXTURE)
                .unwrap()
                .as_os_str()
                .as_encoded_bytes(),
        );
        hasher.update([0]);
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(&bytes);
    }
    hasher.update(fs::read(env!("CARGO_BIN_EXE_b10x-loom")).unwrap());
    hasher.update(fs::read(std::env::current_exe().unwrap()).unwrap());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The file of the shared ended tree holding the syncs its commands made and skipped, all with the
/// switch on.
const SYNCED_FILE: &str = "synced.json";

/// The two-level tree run to its end once per test run (the grant of `ticket.issue` taken twice),
/// kept under `target/` beside the test scratch directories and keyed by [`ended_digest`]: an
/// open, eight runs and their decisions cost seconds each, so each status test reads a fresh copy of
/// this one tree. The first test that asks builds it holding a lock file; the others wait
/// on the lock and read what it left.
fn ended_tree() -> Ended {
    let key = ended_digest();
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let root = base.join(format!("tree-run-ended-{}", &key[..16]));
    let lock = fs::File::create(base.join(format!("tree-run-ended-{}.lock", &key[..16]))).unwrap();
    lock.lock().unwrap();
    if !root.join("ended.json").exists() {
        let _ = fs::remove_dir_all(&root);
        let mut tree = Tree::at(root.clone());
        tree.shared = true;
        let ((waits, ended), synced) =
            counted(Some("1"), || run_to_end_twice(&tree, Some("ticket.issue")));
        fs::write(root.join("waits.json"), serde_json::to_vec(&waits).unwrap()).unwrap();
        fs::write(
            root.join(SYNCED_FILE),
            serde_json::to_vec(&json!({"synced": synced.synced, "skipped": synced.skipped}))
                .unwrap(),
        )
        .unwrap();
        fs::write(root.join("ended.json"), serde_json::to_vec(&ended).unwrap()).unwrap();
    }
    lock.unlock().unwrap();
    let waits = serde_json::from_slice(&fs::read(root.join("waits.json")).unwrap()).unwrap();
    let ended = serde_json::from_slice(&fs::read(root.join("ended.json")).unwrap()).unwrap();
    // Each test reads a fresh copy; the store names the shared workspace, which nothing writes.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let copy = base.join(format!(
        "tree-run-ended-copy-{}-{nanos}",
        std::process::id()
    ));
    copy_dir(&root, &copy);
    Ended {
        tree: Tree {
            root: copy,
            started: Instant::now(),
            shared: false,
        },
        waits,
        ended,
    }
}

/// Copies the directory `from` to `to`, a link as a link.
fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &target);
        } else if kind.is_symlink() {
            std::os::unix::fs::symlink(fs::read_link(entry.path()).unwrap(), &target).unwrap();
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// The entry of `case` in `status`.
fn entry<'a>(status: &'a Value, case: &str) -> &'a Value {
    status["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["case"] == case)
        .unwrap_or_else(|| panic!("no case {case}: {status}"))
}

#[test]
fn a_status_names_each_cases_link_protocol_and_stop() {
    let Ended { tree, waits, ended } = ended_tree();
    assert_eq!(ended["stop"], "Terminal", "{ended}");
    let status = json(&tree.tree("status"));
    let root = entry(&status, "root");
    assert!(
        root.get("name").is_none() && root.get("parent").is_none(),
        "{root}"
    );
    assert_eq!(root["protocol_id"], "tour", "{root}");
    assert_eq!(root["outcome"], "arrived", "{root}");
    let first = entry(&status, "root.first");
    assert_eq!(first["name"], "first", "{first}");
    assert_eq!(first["parent"], "root", "{first}");
    assert_eq!(first["protocol"], "tour-leg@1", "{first}");
    assert_eq!(first["protocol_id"], "tour-leg", "{first}");
    assert_eq!(first["protocol_revision"], "1", "{first}");
    assert_eq!(first["outcome"], "passed", "{first}");
    // Read at the wait on `ticket.issue`, the waiting case names the decision the run printed.
    let (waited, at_wait) = waits
        .iter()
        .find(|(waited, _)| waited["decisions"][0]["action"] == "ticket.issue")
        .unwrap();
    let waiting = entry(at_wait, waited["case"].as_str().unwrap());
    assert_eq!(waiting["stop"], "AwaitingAuthority", "{waiting}");
    assert_eq!(
        waiting["stop_next"],
        json!([waited["decisions"][0]["decides"]]),
        "{waiting}"
    );
}

#[test]
fn a_status_lists_each_record_in_the_order_received() {
    let Ended { tree, .. } = ended_tree();
    let status = json(&tree.tree("status"));
    let root = entry(&status, "root");
    let records = root["evidence"].as_array().unwrap();
    for (index, record) in records.iter().enumerate() {
        assert_eq!(record["index"], index, "{root}");
    }
    let kinds: Vec<&str> = records
        .iter()
        .map(|record| record["kind"].as_str().unwrap())
        .collect();
    // The first leg's opening, the copied approval, then the opening of one leg per name.
    assert_eq!(
        kinds,
        [
            "case_outcome",
            "first_leg_opened",
            "route_approval",
            "case_outcome",
            "case_outcome",
            "legs_opened"
        ],
        "{root}"
    );
    // The governor writes the child's `case_outcome` record with the opening, before the step's own.
    assert!(records[0].get("action").is_none(), "{root}");
    assert_eq!(records[1]["action"], "first.open", "{root}");
    let copied = &records[2];
    assert_eq!(copied["action"], "approval.copy", "{root}");
    // The copy keeps the source record's members.
    let first = entry(&status, "root.first");
    let source = first["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["kind"] == "route_approval")
        .unwrap();
    for member in ["result", "producer", "subject", "subject_revision"] {
        assert_eq!(copied[member], source[member], "{member}: {root}");
    }
    assert_eq!(source["action"], "route.approve", "{first}");
    assert!(copied["upstream_revisions"].is_object(), "{root}");
}

#[test]
fn a_status_counts_the_attempts_of_a_retried_writer() {
    let Ended { tree, .. } = ended_tree();
    let status = json(&tree.tree("status"));
    let first = entry(&status, "root.first");
    let writes: Vec<&Value> = first["runs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|run| run["action"] == "route.write")
        .collect();
    assert_eq!(writes.len(), 2, "{first}");
    assert_eq!(writes[0]["attempt"], 1, "{first}");
    assert_eq!(writes[0]["retried"], true, "{first}");
    assert_eq!(writes[0]["retry_of"], json!([]), "{first}");
    assert_eq!(writes[1]["attempt"], 2, "{first}");
    assert_eq!(writes[1]["retried"], false, "{first}");
    let retry_of = writes[1]["retry_of"].as_array().unwrap();
    assert!(!retry_of.is_empty(), "{first}");
    // Each record the second run was given is a check record of the leg that found it invalid.
    for id in retry_of {
        let record = first["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .find(|record| record["evidence_id"] == *id)
            .unwrap_or_else(|| panic!("{id}: {first}"));
        assert_eq!(record["action"], "route.check", "{first}");
        assert_eq!(record["result"], "invalid", "{first}");
    }
}

#[test]
fn a_status_lists_each_approval_in_the_order_recorded() {
    // The grant of the ticket is recorded twice at one revision (the shared tree takes it twice):
    // two entries, one decision in the approvals file.
    let Ended { tree, .. } = ended_tree();
    let status = json(&tree.tree("status"));
    let first = entry(&status, "root.first");
    let approvals = first["approvals"].as_array().unwrap();
    assert_eq!(approvals.len(), 3, "{first}");
    assert_eq!(approvals[0]["capability"], "guide.approve", "{first}");
    assert_eq!(approvals[0]["action"], "route.approve", "{first}");
    assert_eq!(approvals[0]["actor"], "guide", "{first}");
    assert_eq!(approvals[0]["at"], AT, "{first}");
    for grant in &approvals[1..] {
        assert_eq!(grant["capability"], "guide.issue", "{first}");
        assert_eq!(grant["answer"], "Allow", "{first}");
        assert!(grant.get("action").is_none(), "{first}");
    }
    let file = tree
        .store()
        .join(b10x_loom_tree::approvals_file_name("root.first"));
    let kept: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
    let issued = kept.to_string().matches("guide.issue").count();
    assert_eq!(issued, 1, "{kept}");
    assert!(
        entry(&status, "root")["approvals"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_status_names_the_copy_into_the_root() {
    let Ended { tree, .. } = ended_tree();
    let status = json(&tree.tree("status"));
    assert_eq!(
        entry(&status, "root")["copies"],
        json!([{"source_case": "root.first", "target_case": "root", "record_kind": "route_approval"}]),
        "{status}"
    );
    assert_eq!(
        entry(&status, "root.first")["copies"],
        json!([]),
        "{status}"
    );
}

#[test]
fn the_printed_status_is_the_value_of_tree_status() {
    let Ended { tree, .. } = ended_tree();
    let printed = tree.tree("status");
    assert_eq!(printed.status.code(), Some(0), "{}", stderr(&printed));
    let value = tree_status(&tree.store(), &tree.catalog()).unwrap();
    let document = json(&printed);
    assert_eq!(document, status_json(&value));
    assert_eq!(document["format"], STATUS_FORMAT);
    assert_eq!(STATUS_FORMAT, "loom.tree-status/1");
    assert_eq!(document["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(read_status(&printed.stdout).unwrap(), value);
}

/// The two-level tree after one `tree run --max-steps 1` once per test run, kept as
/// [`ended_tree`] keeps its tree: under `target/`, keyed by [`ended_digest`], built by the first
/// test that asks holding a lock file. The run opened the first leg and stopped `BudgetSpent` in
/// it before its next step. A fresh copy of the tree and the run's JSON; a test reads the copy with
/// `tree status` and never runs it, since its store names the shared workspace.
fn stepped_tree() -> (Tree, Value) {
    let key = ended_digest();
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let root = base.join(format!("tree-run-stepped-{}", &key[..16]));
    let lock =
        fs::File::create(base.join(format!("tree-run-stepped-{}.lock", &key[..16]))).unwrap();
    lock.lock().unwrap();
    if !root.join("stepped.json").exists() {
        let _ = fs::remove_dir_all(&root);
        let mut tree = Tree::at(root.clone());
        tree.shared = true;
        let opened = tree.open();
        assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
        let ran = tree
            .command()
            .args([
                "tree",
                "run",
                "--confinement",
                "local",
                "--max-steps",
                "1",
                "--at",
                AT,
                "--store",
            ])
            .arg(tree.store())
            .output()
            .unwrap();
        assert_eq!(
            ran.status.code(),
            Some(5),
            "{}{}",
            stdout(&ran),
            stderr(&ran)
        );
        fs::write(root.join("stepped.json"), &ran.stdout).unwrap();
    }
    lock.unlock().unwrap();
    let ran = serde_json::from_slice(&fs::read(root.join("stepped.json")).unwrap()).unwrap();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let copy = base.join(format!(
        "tree-run-stepped-copy-{}-{nanos}",
        std::process::id()
    ));
    copy_dir(&root, &copy);
    let tree = Tree {
        root: copy,
        started: Instant::now(),
        shared: false,
    };
    (tree, ran)
}

#[test]
fn a_status_names_each_protocol_its_cases_use_with_its_digest() {
    let Ended { tree, .. } = ended_tree();
    let status = json(&tree.tree("status"));
    let cases = status["cases"].as_array().unwrap();
    let legs = cases
        .iter()
        .filter(|case| case["protocol"] == "tour-leg@1")
        .count();
    assert!(legs >= 2, "more than one case uses tour-leg@1: {status}");
    let catalog = tree.catalog();
    let listed = status["protocols"].as_array().unwrap();
    let registrations: Vec<&str> = listed
        .iter()
        .map(|protocol| protocol["registration"].as_str().unwrap())
        .collect();
    // One entry per protocol the cases use, once, in the order of its first case.
    assert_eq!(registrations, ["tour@1", "tour-leg@1"], "{status}");
    for (protocol, (id, file)) in listed
        .iter()
        .zip([("tour", "tour.yaml"), ("tour.leg", "tour-leg.yaml")])
    {
        let registration = protocol["registration"].as_str().unwrap();
        let entry = catalog.get(registration).unwrap();
        assert_eq!(protocol["protocol"], id, "{protocol}");
        assert_eq!(protocol["revision"], 1, "{protocol}");
        assert_eq!(protocol["file"], entry.definition.source.path, "{protocol}");
        assert_eq!(protocol["sha256"], entry.definition.sha256, "{protocol}");
        // The digest is the one of the protocol's own bytes, as the fixture holds them.
        let bytes = fs::read(fixture_protocols().join(file)).unwrap();
        assert_eq!(protocol["sha256"], sha256_hex(&bytes), "{protocol}");
    }
}

#[test]
fn a_status_names_a_log_that_does_not_verify() {
    let (tree, ran) = stepped_tree();
    let child = ran["case"].as_str().unwrap().to_owned();
    assert_ne!(child, "root", "{ran}");
    let status = json(&tree.tree("status"));
    for case in status["cases"].as_array().unwrap() {
        assert_eq!(case["log"], json!({"verified": true}), "{case}");
    }
    // One changed byte in the child's first line: its second line no longer names it.
    let path = tree.store().join(log_file_name(&child));
    let mut bytes = fs::read(&path).unwrap();
    let first_line = bytes.iter().position(|byte| *byte == b'\n').unwrap();
    let at = bytes[..first_line]
        .windows(8)
        .position(|window| window == b"tour-leg")
        .unwrap();
    bytes[at] = b'T';
    fs::write(&path, &bytes).unwrap();
    let expected = read_log(&bytes).unwrap_err();
    let printed = tree.tree("status");
    assert_eq!(printed.status.code(), Some(0), "{}", stderr(&printed));
    let status = json(&printed);
    let entry_of_child = entry(&status, &child);
    assert_eq!(
        entry_of_child["log"],
        json!({
            "verified": false,
            "refusal": "BrokenChain",
            "line": 1,
            "detail": expected.detail,
        }),
        "{entry_of_child}"
    );
    // Nothing is read from a log that does not verify.
    for member in ["stop", "budget"] {
        assert!(entry_of_child.get(member).is_none(), "{entry_of_child}");
    }
    assert_eq!(entry_of_child["stop_next"], json!([]), "{entry_of_child}");
    assert_eq!(entry_of_child["approvals"], json!([]), "{entry_of_child}");
    assert_eq!(
        entry(&status, "root")["log"],
        json!({"verified": true}),
        "{status}"
    );
}

#[test]
fn a_status_reads_a_missing_log_as_verified_and_empty() {
    let (tree, ran) = stepped_tree();
    let child = ran["case"].as_str().unwrap().to_owned();
    fs::remove_file(tree.store().join(log_file_name(&child))).unwrap();
    let printed = tree.tree("status");
    assert_eq!(printed.status.code(), Some(0), "{}", stderr(&printed));
    let status = json(&printed);
    let entry_of_child = entry(&status, &child);
    assert_eq!(
        entry_of_child["log"],
        json!({"verified": true}),
        "{entry_of_child}"
    );
    for member in ["stop", "budget"] {
        assert!(entry_of_child.get(member).is_none(), "{entry_of_child}");
    }
}

#[test]
fn a_step_budget_stop_names_max_steps_in_the_log_and_the_status() {
    let (tree, ran) = stepped_tree();
    assert_eq!(ran["stop"], "BudgetSpent", "{ran}");
    assert_eq!(ran["steps"], 1, "{ran}");
    assert_eq!(ran["detail"], "the run took its 1 steps", "{ran}");
    let child = ran["case"].as_str().unwrap().to_owned();
    let lines = log_lines(&tree, &child);
    let last = lines.last().unwrap();
    assert!(
        matches!(
            &last.event,
            RunLogEvent::Stopped(stopped)
                if stopped.stop == "BudgetSpent" && stopped.budget.as_deref() == Some("MaxSteps")
        ),
        "{last:?}"
    );
    let printed = tree.tree("status");
    assert_eq!(printed.status.code(), Some(0), "{}", stderr(&printed));
    let status = json(&printed);
    let entry_of_child = entry(&status, &child);
    assert_eq!(entry_of_child["stop"], "BudgetSpent", "{entry_of_child}");
    assert_eq!(entry_of_child["budget"], "MaxSteps", "{entry_of_child}");
    assert!(entry(&status, "root").get("budget").is_none(), "{status}");
    // The printed status reads back as the value `tree_status` returns, budget and logs included.
    let value = tree_status(&tree.store(), &tree.catalog()).unwrap();
    assert_eq!(read_status(&printed.stdout).unwrap(), value);
    let held = value.cases.iter().find(|case| case.case == child).unwrap();
    assert_eq!(held.budget, Some(SpentBudget::MaxSteps));
}

/// A cancel is no budget (`story:cancel-a-running-bound-program`): a run cancelled before its
/// first step stops `Cancelled`, its `stopped` line naming no budget and, as `next`, the action
/// the next run takes first.
#[test]
fn a_cancelled_run_stops_cancelled_naming_its_next_action() {
    let tree = Tree::new("cancel-first");
    let opened = tree.open_leg();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let cancel = AtomicBool::new(true);
    let ran = run_tree(
        &tree.store(),
        &tree.catalog(),
        &settings(None),
        None,
        Some(&cancel),
    )
    .unwrap();
    assert_eq!(ran.stop, TreeStop::Cancelled, "{ran:?}");
    assert_eq!(ran.steps, 0, "{ran:?}");
    assert_eq!(ran.detail.as_deref(), Some("the run was cancelled"));
    let lines = log_lines(&tree, "root");
    // The `case opened` line `tree open` wrote, then the stop: no step was taken.
    assert_eq!(lines.len(), 2, "{lines:?}");
    let RunLogEvent::Stopped(stopped) = &lines[1].event else {
        panic!("no stop: {lines:?}");
    };
    assert_eq!(stopped.stop, "Cancelled", "{stopped:?}");
    assert_eq!(stopped.budget, None, "{stopped:?}");
    assert_eq!(stopped.next.len(), 1, "{stopped:?}");
    let status = json(&tree.tree("status"));
    assert_eq!(entry(&status, "root")["stop"], "Cancelled", "{status}");
    assert!(entry(&status, "root").get("budget").is_none(), "{status}");
    // The next run, not cancelled, takes that action first.
    run_tree(
        &tree.store(),
        &tree.catalog(),
        &settings(Some(1)),
        None,
        None,
    )
    .unwrap();
    let first = log_lines(&tree, "root")
        .into_iter()
        .find_map(|line| match line.event {
            RunLogEvent::ActionFinished(finished) => Some(finished.action),
            _ => None,
        });
    assert_eq!(first.as_ref(), stopped.next.first(), "{stopped:?}");
}

#[test]
fn a_wall_time_stop_names_max_wall_seconds_in_the_stopped_line() {
    let tree = Tree::new("wall-time");
    let opened = tree.open_leg();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let started = Instant::now();
    let mut settings = settings(None);
    settings.max_wall_seconds = Some(1);
    // The first step takes a second: the run's wall time is spent before the next.
    let mut slept = false;
    let mut observer = |_: &str, line: &RunLogLine| {
        if !slept && matches!(line.event, RunLogEvent::ActionFinished(_)) {
            slept = true;
            std::thread::sleep(Duration::from_millis(1100));
        }
    };
    let ran = run_tree(
        &tree.store(),
        &tree.catalog(),
        &settings,
        Some(&mut observer),
        None,
    )
    .unwrap();
    assert_eq!(ran.stop, TreeStop::BudgetSpent, "{ran:?}");
    assert_eq!(ran.steps, 1, "{ran:?}");
    assert_eq!(
        ran.detail.as_deref(),
        Some("the run's 1 s of wall time are spent")
    );
    let lines = log_lines(&tree, "root");
    let last = lines.last().unwrap();
    assert!(
        matches!(
            &last.event,
            RunLogEvent::Stopped(stopped)
                if stopped.stop == "BudgetSpent"
                    && stopped.budget.as_deref() == Some("MaxWallSeconds")
        ),
        "{last:?}"
    );
    let took = started.elapsed();
    assert!(took < Duration::from_secs(5), "the run took {took:?}");
}

#[test]
fn a_status_of_an_unknown_format_is_refused() {
    let tree = Tree::new("status-format");
    let opened = tree.open();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let mut document = json(&tree.tree("status"));
    document["format"] = json!("loom.tree-status/2");
    let refused = read_status(document.to_string().as_bytes()).unwrap_err();
    assert_eq!(refused.0.reason, TreeRefusalReason::UnknownStatusFormat);
    assert!(
        refused.0.detail.contains("loom.tree-status/2"),
        "{}",
        refused.0.detail
    );
    document.as_object_mut().unwrap().remove("format");
    document.as_object_mut().unwrap().remove("version");
    let refused = read_status(document.to_string().as_bytes()).unwrap_err();
    assert_eq!(refused.0.reason, TreeRefusalReason::UnknownStatusFormat);
    assert!(
        refused.0.detail.contains("no format"),
        "{}",
        refused.0.detail
    );
}

#[test]
fn a_spent_run_budget_ends_the_retries_of_a_step() {
    // A writer that changes nothing, sent back by a check that stays invalid, under a budget of
    // three runs: the run takes it three times, is refused once, and is stuck. Without the bound
    // a refused step stayed due through its retry records (844 refusals in one run of the stub
    // cycle); `--max-steps 100` keeps a regression from running on.
    let tree = Tree::new("spent-budget");
    // One stop already: the check finds the route short, about a revision the case holds.
    fs::write(tree.workspace().join("route.txt"), "start\n").unwrap();
    let opened = tree.open_leg_with("tour-leg-idle.bindings.yaml");
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let ran = tree
        .command()
        .args([
            "tree",
            "run",
            "--confinement",
            "local",
            "--max-steps",
            "100",
            "--at",
            AT,
        ])
        .arg("--store")
        .arg(tree.store())
        .output()
        .unwrap();
    let lines = log_lines(&tree, "root");
    let writes = lines
        .iter()
        .filter(|line| matches!(&line.event, RunLogEvent::ActionFinished(finished) if finished.action == "route.write"))
        .count();
    assert!(
        writes <= 20,
        "route.write finished {writes} times in one run"
    );
    assert_eq!(writes, 4, "three runs and one refusal");
    assert_eq!(
        ran.status.code(),
        Some(4),
        "{}{}",
        stdout(&ran),
        stderr(&ran)
    );
    assert_eq!(json(&ran)["stop"], "Stuck");
}

/// The `route_approval` record of a one-case leg opened with `bindings`, `notes.txt` holding `abc`,
/// once its person approved the route.
fn approval_record(name: &str, bindings: &str) -> Value {
    let tree = Tree::new(name);
    fs::write(tree.workspace().join("notes.txt"), "abc").unwrap();
    let opened = tree.open_leg_with(bindings);
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let waited = tree.run();
    assert_eq!(
        waited.status.code(),
        Some(3),
        "{}{}",
        stdout(&waited),
        stderr(&waited)
    );
    let approved = tree.decided(&json(&waited)["decisions"][0]);
    assert_eq!(approved.status.code(), Some(0), "{}", stderr(&approved));
    let status = json(&tree.tree("status"));
    entry(&status, "root")["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["kind"] == "route_approval")
        .cloned()
        .unwrap_or_else(|| panic!("no route_approval: {status}"))
}

#[test]
fn a_person_record_names_the_revisions_of_its_declared_upstream() {
    // `notes.txt` holding `abc` is at revision `t` and the first 16 hex digits of its SHA-256.
    let declared = approval_record("person-upstream", "tour-leg-upstream.bindings.yaml");
    assert_eq!(
        declared["upstream_revisions"],
        json!({"notes": "tba7816bf8f01cfea"}),
        "{declared}"
    );
    let undeclared = approval_record("person-no-upstream", "tour-leg.bindings.yaml");
    assert_eq!(undeclared["upstream_revisions"], json!({}), "{undeclared}");
}

// Story `child-record-in-the-run-log`: each write of a child's `case_outcome` record in its parent
// appears in one line of the parent's log, `child opened` or `child closed`, naming the record and
// the SHA-256 of its facts.

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The case file of `case` in `store`, as JSON.
fn case_file(store: &Path, case: &str) -> Value {
    let path = store.join(format!("{}.json", loom_governor::case_file_stem(case)));
    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap()
}

/// The SHA-256 of the facts of each `case_outcome` record `case`'s case file holds now, by
/// evidence id: the facts' JSON as `serde_json` writes it, members in name order.
fn record_digests(store: &Path, case: &str) -> BTreeMap<String, String> {
    case_file(store, case)["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|held| held["data"]["kind"] == "case_outcome")
        .map(|held| {
            (
                held["data"]["evidence_id"].as_str().unwrap().to_owned(),
                sha256_hex(&serde_json::to_vec(&held["data"]["facts"]).unwrap()),
            )
        })
        .collect()
}

/// A `child opened` or `child closed` line as JSON, every member of the line; `null` for any other
/// line.
fn child_line_json(line: &RunLogLine) -> Value {
    let (kind, child, protocol, outcome, record, digest) = match &line.event {
        RunLogEvent::ChildOpened(opened) => (
            "ChildOpened",
            &opened.child,
            Some(&opened.protocol),
            None,
            &opened.record,
            &opened.record_digest,
        ),
        RunLogEvent::ChildClosed(closed) => (
            "ChildClosed",
            &closed.child,
            None,
            closed.outcome.as_ref(),
            &closed.record,
            &closed.record_digest,
        ),
        _ => return Value::Null,
    };
    json!({
        "seq": line.seq,
        "prev": line.prev,
        "at": line.at.0,
        "kind": kind,
        "child": child,
        "protocol": protocol,
        "outcome": outcome,
        "record": record,
        "record_digest": digest,
    })
}

/// The `record` and `record_digest` of a child line; `None` for any other line.
fn record_of(line: &RunLogLine) -> Option<(Option<&str>, Option<&str>)> {
    match &line.event {
        RunLogEvent::ChildOpened(opened) => {
            Some((opened.record.as_deref(), opened.record_digest.as_deref()))
        }
        RunLogEvent::ChildClosed(closed) => {
            Some((closed.record.as_deref(), closed.record_digest.as_deref()))
        }
        _ => None,
    }
}

/// The two-level tree run in process to its end: `tree open`, then `run_tree` with an observer,
/// each first decision taken through the command line, until a run ends `Terminal`. Each child
/// line the observer was given, in order: the case whose log holds it, the line as JSON and the
/// digests of the `case_outcome` records the case's file held right then.
fn child_lines_to_end(tree: &Tree) -> Vec<Value> {
    let opened = tree.open();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let catalog = tree.catalog();
    let store = tree.store();
    let mut seen: Vec<Value> = Vec::new();
    for _ in 0..8 {
        let mut observer = |case: &str, line: &RunLogLine| {
            if record_of(line).is_some() {
                seen.push(json!({
                    "case": case,
                    "line": child_line_json(line),
                    "held": record_digests(&store, case),
                }));
            }
        };
        let ran = run_tree(&store, &catalog, &settings(None), Some(&mut observer), None).unwrap();
        if ran.stop == TreeStop::Terminal {
            return seen;
        }
        let decided = tree.decided(&decision_json(&ran.decisions[0]));
        assert_eq!(decided.status.code(), Some(0), "{}", stderr(&decided));
    }
    panic!("the tree did not end within 8 runs");
}

/// The two-level tree run in process to its end once per test run, as [`ended_tree`] keeps its
/// tree: under `target/`, keyed by [`ended_digest`], built by the first test that asks holding a
/// lock file. A fresh copy of the tree and each child line the observer was given, as
/// [`child_lines_to_end`] returns them.
fn observed_tree() -> (Tree, Vec<Value>) {
    let key = ended_digest();
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let root = base.join(format!("tree-run-observed-{}", &key[..16]));
    let lock =
        fs::File::create(base.join(format!("tree-run-observed-{}.lock", &key[..16]))).unwrap();
    lock.lock().unwrap();
    if !root.join("seen.json").exists() {
        let _ = fs::remove_dir_all(&root);
        // `run_tree` writes the store in process: in a process of its own with the switch on, so
        // those writes do not wait for the disk as the binary's do not.
        let built = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "the_observed_tree_is_built_with_the_switch_on",
                "--ignored",
                "--nocapture",
            ])
            .env("B10X_LOOM_TEST_UNSYNCED_STORE", "1")
            .env(OBSERVED_ROOT, &root)
            .output()
            .unwrap();
        assert!(
            built.status.success() && root.join("seen.json").exists(),
            "{}{}",
            stdout(&built),
            stderr(&built)
        );
    }
    lock.unlock().unwrap();
    let seen = serde_json::from_slice(&fs::read(root.join("seen.json")).unwrap()).unwrap();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let copy = base.join(format!(
        "tree-run-observed-copy-{}-{nanos}",
        std::process::id()
    ));
    copy_dir(&root, &copy);
    let tree = Tree {
        root: copy,
        started: Instant::now(),
        shared: false,
    };
    (tree, seen)
}

/// Not a test of its own: the test step (`crates/loom-xtask/src/test_step.rs`) runs it after its
/// build and before its timed run, so the shared trees ([`ended_tree`], [`stepped_tree`],
/// [`observed_tree`]) are made once and no test of the timed run makes one or waits on the lock of
/// another that does: a waiter's wait counted in the binary's summed time, once per waiter.
#[test]
#[ignore = "run by the test step before its timed run"]
fn the_shared_trees_are_made() {
    drop(ended_tree());
    drop(stepped_tree());
    drop(observed_tree());
}

/// The variable naming the directory [`observed_tree`] has its child process build the tree in.
const OBSERVED_ROOT: &str = "TREE_RUN_OBSERVED_ROOT";

/// Not a test of its own: the child process [`observed_tree`] starts, with the switch on in its
/// own environment, builds the observed tree at [`OBSERVED_ROOT`].
#[test]
#[ignore = "run by observed_tree in a process of its own"]
fn the_observed_tree_is_built_with_the_switch_on() {
    let root = PathBuf::from(std::env::var_os(OBSERVED_ROOT).expect("started by observed_tree"));
    let mut tree = Tree::at(root.clone());
    tree.shared = true;
    let seen = child_lines_to_end(&tree);
    fs::write(root.join("seen.json"), serde_json::to_vec(&seen).unwrap()).unwrap();
}

#[test]
fn each_write_of_a_childs_record_appears_in_exactly_one_parent_line() {
    let (tree, seen) = observed_tree();
    let store = tree.store();
    let root = case_file(&store, "root");
    let links = root["children"].as_array().unwrap();
    assert_eq!(links.len(), 3, "the first leg and two legs: {root}");
    let lines = log_lines(&tree, "root");
    let names = |line: &RunLogLine, record: &str| match &line.event {
        RunLogEvent::ActionFinished(finished) => finished.evidence.iter().any(|id| id == record),
        _ => record_of(line).is_some_and(|(named, _)| named == Some(record)),
    };
    for link in links {
        let child = link["child"].as_str().unwrap();
        let record = link["outcome_evidence"].as_str().unwrap();
        // The record is written at the opening and replaced at the termination: two lines.
        let naming: Vec<&RunLogLine> = lines.iter().filter(|line| names(line, record)).collect();
        assert_eq!(naming.len(), 2, "{child}: {naming:#?}");
        assert!(
            matches!(&naming[0].event, RunLogEvent::ChildOpened(opened) if opened.child == child),
            "{child}: the opening's write is in `child opened`: {naming:#?}"
        );
        assert!(
            matches!(&naming[1].event, RunLogEvent::ChildClosed(closed) if closed.child == child),
            "{child}: the termination's write is in `child closed`: {naming:#?}"
        );
        for line in naming {
            let given = seen
                .iter()
                .find(|given| given["case"] == "root" && given["line"] == child_line_json(line))
                .unwrap_or_else(|| panic!("{child}: the observer was not given {line:?}"));
            let (_, digest) = record_of(line).unwrap();
            assert_eq!(
                digest,
                given["held"][record].as_str(),
                "{child}: the digest is that of the facts right after the write: {line:?}"
            );
        }
        // The child's own log names no record of its parent.
        let own = log_lines(&tree, child);
        assert!(
            !own.iter().any(|line| names(line, record)),
            "{child}: {own:#?}"
        );
    }
}

#[test]
fn the_closing_digest_differs_from_the_opening_digest() {
    let Ended { tree, ended, .. } = ended_tree();
    assert_eq!(ended["stop"], "Terminal", "{ended}");
    let mut opened: BTreeMap<String, String> = BTreeMap::new();
    let mut closed: BTreeMap<String, String> = BTreeMap::new();
    for line in log_lines(&tree, "root") {
        let Some((Some(record), Some(digest))) = record_of(&line) else {
            continue;
        };
        let into = match line.event {
            RunLogEvent::ChildOpened(_) => &mut opened,
            _ => &mut closed,
        };
        assert!(
            into.insert(record.to_owned(), digest.to_owned()).is_none(),
            "{record} named twice by one kind of line"
        );
    }
    assert_eq!(opened.len(), 3, "{opened:?}");
    assert_eq!(
        opened.keys().collect::<Vec<_>>(),
        closed.keys().collect::<Vec<_>>()
    );
    for (record, digest) in &opened {
        let closing = &closed[record];
        for held in [digest, closing] {
            assert!(
                held.len() == 64
                    && held
                        .bytes()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
                "{record}: {held} is no SHA-256"
            );
        }
        assert_ne!(
            digest, closing,
            "{record}: the record is replaced, not repeated"
        );
    }
}

#[test]
fn a_status_names_the_child_of_each_case_outcome_record() {
    let tree = Tree::new("child-record-status");
    let opened = tree.open();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    // The first run opens the first leg, which waits on its person.
    let ran = tree.run();
    assert_eq!(
        ran.status.code(),
        Some(3),
        "{}{}",
        stdout(&ran),
        stderr(&ran)
    );
    let printed = tree.tree("status");
    assert_eq!(printed.status.code(), Some(0), "{}", stderr(&printed));
    let status = json(&printed);
    let root = case_file(&tree.store(), "root");
    let names: BTreeMap<&str, &str> = root["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|link| {
            (
                link["outcome_evidence"].as_str().unwrap(),
                link["name"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(names.values().collect::<Vec<_>>(), [&"first"], "{root}");
    let mut named = 0;
    for case in status["cases"].as_array().unwrap() {
        for record in case["evidence"].as_array().unwrap() {
            if record["kind"] == "case_outcome" {
                let id = record["evidence_id"].as_str().unwrap();
                assert_eq!(record["name"], names[id], "{record}");
                named += 1;
            } else {
                assert!(record.get("name").is_none(), "{record}");
            }
        }
    }
    assert_eq!(named, 1, "{status}");
    // The printed status reads back as the value it printed, `name` included.
    let value = tree_status(&tree.store(), &tree.catalog()).unwrap();
    assert_eq!(read_status(&printed.stdout).unwrap(), value);
    assert_eq!(status, status_json(&value));
}

#[test]
fn an_observer_is_given_the_record_of_each_child_line() {
    let (tree, seen) = observed_tree();
    let written: Vec<Value> = log_lines(&tree, "root")
        .iter()
        .filter(|line| record_of(line).is_some())
        .map(child_line_json)
        .collect();
    let observed: Vec<Value> = seen
        .iter()
        .filter(|given| given["case"] == "root")
        .map(|given| given["line"].clone())
        .collect();
    assert_eq!(
        observed, written,
        "the observer is given each child line as written"
    );
    assert_eq!(
        observed.len(),
        6,
        "three opened, three closed: {observed:#?}"
    );
    for line in &observed {
        assert!(
            line["record"].is_string() && line["record_digest"].is_string(),
            "the observer is given the record of {line}"
        );
    }
}

#[test]
fn a_case_with_two_legitimate_outcomes_waits_for_tree_close() {
    let tree = Tree::new("close");
    let opened = tree
        .command()
        .args([
            "tree",
            "open",
            "--protocol",
            "fork@1",
            "--at",
            AT,
            "--store",
        ])
        .arg(tree.store())
        .arg("--bindings")
        .arg(tree.workspace().join("fork.bindings.yaml"))
        .arg("--workspace")
        .arg(tree.workspace())
        .output()
        .unwrap();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));

    // Once the road is seen both outcomes are legitimate: the run waits and prints two closes.
    let waited = tree.run();
    assert_eq!(
        waited.status.code(),
        Some(3),
        "{}{}",
        stdout(&waited),
        stderr(&waited)
    );
    let waited = json(&waited);
    assert_eq!(waited["stop"], "AwaitingPerson", "{waited}");
    let decisions = waited["decisions"].as_array().unwrap();
    let outcomes: Vec<&str> = decisions
        .iter()
        .map(|decision| decision["outcome"].as_str().unwrap())
        .collect();
    assert_eq!(outcomes, ["kept", "left"], "{waited}");
    for decision in decisions {
        assert_eq!(decision["kind"], "Close", "{waited}");
        assert_eq!(decision["command"][2], "close", "{waited}");
    }

    // An outcome that is not legitimate is refused and the store keeps every byte.
    let before = tree.snapshot();
    let refused = tree
        .command()
        .args([
            "tree",
            "close",
            "--case",
            "root",
            "--outcome",
            "lost",
            "--at",
            AT,
            "--store",
        ])
        .arg(tree.store())
        .output()
        .unwrap();
    assert_ne!(refused.status.code(), Some(0), "{}", stdout(&refused));
    assert!(
        stderr(&refused).contains("OutcomeNotLegitimate"),
        "{}",
        stderr(&refused)
    );
    assert_eq!(tree.snapshot(), before);

    // The printed command closes the case through `left`; the tree is then terminal through it.
    let closed = tree.printed(&decisions[1]);
    assert_eq!(closed.status.code(), Some(0), "{}", stderr(&closed));
    assert_eq!(json(&closed)["outcome"], "left");
    let status = json(&tree.tree("status"));
    assert_eq!(entry(&status, "root")["outcome"], "left", "{status}");
    let lines = log_lines(&tree, "root");
    assert!(
        matches!(&lines.last().unwrap().event, RunLogEvent::Terminated(ended) if ended.reason == "left")
    );
    let verified = tree.tree("verify");
    assert_eq!(verified.status.code(), Some(0), "{}", stderr(&verified));
    let again = tree
        .command()
        .args([
            "tree",
            "close",
            "--case",
            "root",
            "--outcome",
            "kept",
            "--at",
            AT,
            "--store",
        ])
        .arg(tree.store())
        .output()
        .unwrap();
    assert!(
        stderr(&again).contains("CaseTerminated"),
        "{}",
        stderr(&again)
    );
}

#[test]
fn a_writer_with_a_handoff_waits_on_a_person_with_its_text() {
    // The writer writes nothing and nothing sends it back: once it ran and the check found the
    // route short, nothing is useful, and its handoff is the decision.
    let tree = Tree::new("handoff");
    let opened = tree.open_leg_with("tour-leg-handoff.bindings.yaml");
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    for _ in 0..2 {
        let waited = tree.run();
        assert_eq!(
            waited.status.code(),
            Some(3),
            "{}{}",
            stdout(&waited),
            stderr(&waited)
        );
        let waited = json(&waited);
        assert_eq!(waited["stop"], "AwaitingPerson", "{waited}");
        let decision = &waited["decisions"][0];
        assert_eq!(decision["kind"], "Handoff", "{waited}");
        assert_eq!(decision["action"], "route.write", "{waited}");
        assert_eq!(
            decision["decides"],
            "Add a second stop to route.txt, then run the tree again."
        );
        // The next run is started as this one was: under the same confinement.
        let store = tree.store().display().to_string();
        assert_eq!(
            decision["command"],
            json!([
                "b10x-loom",
                "tree",
                "run",
                "--store",
                store,
                "--confinement",
                "local"
            ])
        );
    }
    let writes = log_lines(&tree, "root")
        .iter()
        .filter(|line| matches!(&line.event, RunLogEvent::ActionFinished(finished) if finished.action == "route.write"))
        .count();
    assert_eq!(writes, 1, "the writer runs once at its revision");
}

/// The one decision a run that waits prints, of the kind `kind`.
fn one_decision(ran: &Output, kind: &str) -> Value {
    assert_eq!(ran.status.code(), Some(3), "{}{}", stdout(ran), stderr(ran));
    let waited = json(ran);
    let decisions = waited["decisions"].as_array().unwrap();
    assert_eq!(decisions.len(), 1, "{waited}");
    assert_eq!(decisions[0]["kind"], kind, "{waited}");
    decisions[0].clone()
}

#[test]
fn every_command_a_leg_prints_runs_as_printed_to_its_outcome() {
    // A handoff, a person action under a capability and a capability alone, each answered by
    // the command the run printed, with no flag added, until the case passes.
    let tree = Tree::new("printed");
    let opened = tree.open_leg_with("tour-leg-handoff.bindings.yaml");
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let handoff = one_decision(&tree.run(), "Handoff");
    // The person does what the handoff text asks.
    fs::write(tree.workspace().join("route.txt"), "start\nend\n").unwrap();
    let approve_action = one_decision(&tree.printed(&handoff), "ApproveAction");
    assert_eq!(
        approve_action["capability"], "guide.approve",
        "{approve_action}"
    );
    let taken = tree.printed(&approve_action);
    assert_eq!(
        taken.status.code(),
        Some(0),
        "{}{}",
        stdout(&taken),
        stderr(&taken)
    );
    let approve = one_decision(&tree.run(), "Approve");
    let granted = tree.printed(&approve);
    assert_eq!(
        granted.status.code(),
        Some(0),
        "{}{}",
        stdout(&granted),
        stderr(&granted)
    );
    let ended = tree.run();
    assert_eq!(
        ended.status.code(),
        Some(0),
        "{}{}",
        stdout(&ended),
        stderr(&ended)
    );
    assert_eq!(json(&ended)["detail"], "passed");
}

/// `tree open` of `protocol` with the bindings file `bindings` of the workspace.
fn open_with(tree: &Tree, protocol: &str, bindings: &str) {
    let opened = tree
        .command()
        .args([
            "tree",
            "open",
            "--protocol",
            protocol,
            "--at",
            AT,
            "--store",
        ])
        .arg(tree.store())
        .arg("--bindings")
        .arg(tree.workspace().join(bindings))
        .arg("--workspace")
        .arg(tree.workspace())
        .output()
        .unwrap();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
}

/// A `sign@1` case brought to its first wait, and the decisions the run printed: signing (a
/// person, no capability), sealing (a person under `clerk.seal`) and stamping (a program under
/// `clerk.stamp`) all wait at once.
fn signpost(label: &str) -> (Tree, Vec<Value>) {
    let tree = Tree::new(label);
    open_with(&tree, "sign@1", "sign.bindings.yaml");
    let ran = tree.run();
    assert_eq!(
        ran.status.code(),
        Some(3),
        "{}{}",
        stdout(&ran),
        stderr(&ran)
    );
    let waited = json(&ran);
    assert_eq!(waited["stop"], "AwaitingPerson", "{waited}");
    let decisions = waited["decisions"].as_array().unwrap().clone();
    assert_eq!(decisions.len(), 3, "{waited}");
    (tree, decisions)
}

/// The decision of `decisions` about `action`.
fn about(decisions: &[Value], action: &str) -> Value {
    decisions
        .iter()
        .find(|decision| decision["action"] == action)
        .cloned()
        .unwrap_or_else(|| panic!("no decision about {action}: {decisions:?}"))
}

/// The lines of `case`'s log written since it held `before` lines.
fn lines_since(tree: &Tree, case: &str, before: usize) -> Vec<RunLogLine> {
    log_lines(tree, case)[before..].to_vec()
}

/// The records of `kind` in the status of `case`.
fn records_of(tree: &Tree, case: &str, kind: &str) -> Vec<Value> {
    let status = json(&tree.tree("status"));
    entry(&status, case)["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|record| record["kind"] == kind)
        .cloned()
        .collect()
}

fn is_authority(line: &RunLogLine) -> bool {
    matches!(line.event, RunLogEvent::AuthorityRecorded(_))
}

fn finished(line: &RunLogLine) -> Option<&str> {
    match &line.event {
        RunLogEvent::ActionFinished(finished) => Some(finished.action.as_str()),
        _ => None,
    }
}

#[test]
fn the_printed_approve_command_runs() {
    let (tree, decisions) = signpost("printed-approve");
    let decision = about(&decisions, "post.stamp");
    assert_eq!(decision["kind"], "Approve", "{decision}");
    let before = log_lines(&tree, "root").len();
    let granted = tree.printed(&decision);
    assert_eq!(granted.status.code(), Some(0), "{}", stderr(&granted));
    let written = lines_since(&tree, "root", before);
    assert_eq!(written.len(), 1, "{written:?}");
    assert!(is_authority(&written[0]), "{written:?}");

    // The next run takes the action the grant allowed.
    let before = log_lines(&tree, "root").len();
    let ran = tree.run();
    assert_eq!(
        ran.status.code(),
        Some(3),
        "{}{}",
        stdout(&ran),
        stderr(&ran)
    );
    let written = lines_since(&tree, "root", before);
    assert!(
        written
            .iter()
            .any(|line| finished(line) == Some("post.stamp")),
        "{written:?}"
    );
    let stamped = records_of(&tree, "root", "post_stamped");
    assert_eq!(stamped.len(), 1, "{stamped:?}");
    assert_eq!(stamped[0]["result"], "stamped", "{stamped:?}");
}

#[test]
fn the_printed_approve_action_command_runs() {
    let (tree, decisions) = signpost("printed-approve-action");
    let decision = about(&decisions, "post.seal");
    assert_eq!(decision["kind"], "ApproveAction", "{decision}");
    assert_eq!(decision["capability"], "clerk.seal", "{decision}");
    let before = log_lines(&tree, "root").len();
    let taken = tree.printed(&decision);
    assert_eq!(taken.status.code(), Some(0), "{}", stderr(&taken));
    let written = lines_since(&tree, "root", before);
    assert_eq!(written.len(), 2, "{written:?}");
    assert!(is_authority(&written[0]), "{written:?}");
    assert_eq!(finished(&written[1]), Some("post.seal"), "{written:?}");
    let sealed = records_of(&tree, "root", "post_sealed");
    assert_eq!(sealed.len(), 1, "{sealed:?}");
    assert_eq!(sealed[0]["result"], "sealed", "{sealed:?}");
    assert_eq!(sealed[0]["producer"], "clerk", "{sealed:?}");
}

#[test]
fn the_printed_person_action_without_a_capability_runs() {
    // `post.sign` needs no capability: the printed `tree approve --action` names none and runs.
    let (tree, decisions) = signpost("printed-no-capability");
    let decision = about(&decisions, "post.sign");
    assert_eq!(decision["kind"], "ApproveAction", "{decision}");
    assert!(decision["capability"].is_null(), "{decision}");
    assert!(
        !decision["command"]
            .as_array()
            .unwrap()
            .iter()
            .any(|word| word == "--capability"),
        "{decision}"
    );
    let before = log_lines(&tree, "root").len();
    let taken = tree.printed(&decision);
    assert_eq!(
        taken.status.code(),
        Some(0),
        "{}{}",
        stdout(&taken),
        stderr(&taken)
    );
    // One step, no authority: the action is taken and no approval is kept.
    let written = lines_since(&tree, "root", before);
    assert_eq!(written.len(), 1, "{written:?}");
    assert_eq!(finished(&written[0]), Some("post.sign"), "{written:?}");
    assert!(!written.iter().any(is_authority), "{written:?}");
    assert!(
        b10x_loom_tree::read_approvals(&tree.store(), "root")
            .unwrap()
            .is_empty()
    );
    let signed = records_of(&tree, "root", "post_signed");
    assert_eq!(signed.len(), 1, "{signed:?}");
    assert_eq!(signed[0]["result"], "signed", "{signed:?}");
    assert_eq!(signed[0]["producer"], "walker", "{signed:?}");
    let verified = tree.tree("verify");
    assert_eq!(verified.status.code(), Some(0), "{}", stderr(&verified));
}

#[test]
fn a_person_action_that_needs_a_capability_is_refused_without_one() {
    let (tree, _) = signpost("needs-capability");
    let before = tree.snapshot();
    let refused = tree
        .command()
        .args(["tree", "approve", "--store"])
        .arg(tree.store())
        .args([
            "--case",
            "root",
            "--principal",
            "clerk",
            "--action",
            "post.seal",
        ])
        .output()
        .unwrap();
    let said = stderr(&refused);
    assert_eq!(refused.status.code(), Some(1), "{said}");
    assert!(said.contains("NotAPersonAction"), "{said}");
    assert!(said.contains("clerk.seal"), "{said}");
    assert_eq!(
        tree.snapshot(),
        before,
        "a refused action wrote into the store"
    );
}

#[test]
fn approve_without_an_action_still_requires_a_capability() {
    let tree = Tree::new("no-action-no-capability");
    let refused = tree
        .command()
        .args(["tree", "approve", "--store"])
        .arg(tree.store())
        .args(["--case", "root", "--principal", "guide"])
        .output()
        .unwrap();
    let said = stderr(&refused);
    assert_eq!(refused.status.code(), Some(2), "{said}");
    assert!(said.contains("--capability"), "{said}");
    assert!(!tree.store().exists(), "the refused command made the store");
}

#[test]
fn the_printed_close_command_runs() {
    let tree = Tree::new("printed-close");
    open_with(&tree, "fork@1", "fork.bindings.yaml");
    let waited = tree.run();
    assert_eq!(waited.status.code(), Some(3), "{}", stderr(&waited));
    let waited = json(&waited);
    let left = waited["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|decision| decision["outcome"] == "left")
        .cloned()
        .unwrap_or_else(|| panic!("no close through left: {waited}"));
    assert_eq!(left["kind"], "Close", "{left}");
    let closed = tree.printed(&left);
    assert_eq!(closed.status.code(), Some(0), "{}", stderr(&closed));
    assert_eq!(json(&closed)["outcome"], "left");
    let lines = log_lines(&tree, "root");
    assert!(
        matches!(&lines.last().unwrap().event, RunLogEvent::Terminated(ended) if ended.reason == "left"),
        "{lines:?}"
    );
    // The tree is terminal through `left`: the next run ends at once.
    let ended = tree.run();
    assert_eq!(ended.status.code(), Some(0), "{}", stderr(&ended));
    assert_eq!(json(&ended)["stop"], "Terminal");
    let status = json(&tree.tree("status"));
    assert_eq!(entry(&status, "root")["outcome"], "left", "{status}");
}

#[test]
fn the_printed_handoff_command_runs() {
    // The writer writes nothing: run as printed, the next run waits on the same handoff.
    let tree = Tree::new("printed-handoff");
    let opened = tree.open_leg_with("tour-leg-handoff.bindings.yaml");
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let handoff = one_decision(&tree.run(), "Handoff");
    let words: Vec<&str> = handoff["command"]
        .as_array()
        .unwrap()
        .iter()
        .map(|word| word.as_str().unwrap())
        .collect();
    assert_eq!(&words[..3], ["b10x-loom", "tree", "run"], "{handoff}");
    // Run unchanged, it starts its programs as this run did, not under the default confinement.
    assert!(
        words
            .windows(2)
            .any(|pair| pair == ["--confinement", "local"]),
        "{handoff}"
    );
    let before = log_lines(&tree, "root").len();
    let again = tree.printed(&handoff);
    assert_eq!(handoff, one_decision(&again, "Handoff"));
    let written = lines_since(&tree, "root", before);
    assert!(
        matches!(&written.last().unwrap().event, RunLogEvent::Stopped(stopped) if stopped.stop == "AwaitingPerson"),
        "{written:?}"
    );
}

/// A `review@1` case brought to its first wait, `draft.txt` written: the review and the
/// judgement, both a person's with no capability, wait; the writer waits for a rejection.
fn reviewed(label: &str) -> Tree {
    let tree = Tree::new(label);
    // The draft is there before the review: a record about it names its revision.
    fs::write(tree.workspace().join("draft.txt"), "draft\n").unwrap();
    open_with(&tree, "review@1", "review.bindings.yaml");
    let ran = tree.run();
    assert_eq!(
        ran.status.code(),
        Some(3),
        "{}{}",
        stdout(&ran),
        stderr(&ran)
    );
    let waited = json(&ran);
    assert_eq!(waited["stop"], "AwaitingPerson", "{waited}");
    assert_eq!(waited["decisions"].as_array().unwrap().len(), 2, "{waited}");
    tree
}

/// `tree approve` of the person action `action` of the root as `reviewer`, with `flags` after.
fn review(tree: &Tree, action: &str, flags: &[&str]) -> Output {
    tree.command()
        .args(["tree", "approve", "--at", AT, "--store"])
        .arg(tree.store())
        .args([
            "--case",
            "root",
            "--principal",
            "reviewer",
            "--action",
            action,
        ])
        .args(flags)
        .output()
        .unwrap()
}

/// Every file below `store` and its bytes, by path.
fn every_file(store: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    let mut directories = vec![store.to_owned()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                directories.push(entry.path());
            } else {
                files.push((entry.path(), fs::read(entry.path()).unwrap_or_default()));
            }
        }
    }
    files.sort();
    files
}

/// `output` refused `NotAPersonAction` with exit 1, saying each of `named`.
fn refused_naming(output: &Output, named: &[&str]) {
    let said = stderr(output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(said.contains("NotAPersonAction"), "{said}");
    for name in named {
        assert!(said.contains(name), "{name}: {said}");
    }
}

#[test]
fn a_person_records_the_named_result() {
    let tree = reviewed("named-result");
    let before = log_lines(&tree, "root").len();
    let taken = review(&tree, "draft.review", &["--result", "rejected"]);
    assert_eq!(taken.status.code(), Some(0), "{}", stderr(&taken));
    let reviews = records_of(&tree, "root", "draft_review");
    assert_eq!(reviews.len(), 1, "{reviews:?}");
    assert_eq!(reviews[0]["result"], "rejected", "{reviews:?}");
    assert_eq!(reviews[0]["producer"], "reviewer", "{reviews:?}");
    let written = lines_since(&tree, "root", before);
    assert_eq!(written.len(), 1, "{written:?}");
    let RunLogEvent::ActionFinished(ended) = &written[0].event else {
        panic!("{written:?}");
    };
    assert_eq!(ended.action, "draft.review");
    assert_eq!(
        ended.evidence,
        [reviews[0]["evidence_id"].as_str().unwrap()]
    );
}

#[test]
fn an_undeclared_result_is_refused_and_writes_nothing() {
    let tree = reviewed("undeclared-result");
    let before = every_file(&tree.store());
    let refused = review(&tree, "draft.review", &["--result", "lost"]);
    refused_naming(&refused, &["draft.review", "lost", "performed", "rejected"]);
    assert_eq!(every_file(&tree.store()), before);
}

#[test]
fn a_result_one_entry_does_not_list_writes_no_record() {
    // The judgement's first entry lists `rejected`, its second (`draft_note`) only `performed`.
    let tree = reviewed("one-entry-unlisted");
    let before = every_file(&tree.store());
    let refused = review(&tree, "draft.judge", &["--result", "rejected"]);
    refused_naming(
        &refused,
        &["draft.judge", "rejected", "draft_note", "performed"],
    );
    assert_eq!(every_file(&tree.store()), before);
}

#[test]
fn without_a_result_the_person_performed() {
    let tree = reviewed("performed");
    let taken = review(&tree, "draft.review", &[]);
    assert_eq!(taken.status.code(), Some(0), "{}", stderr(&taken));
    let reviews = records_of(&tree, "root", "draft_review");
    assert_eq!(reviews.len(), 1, "{reviews:?}");
    assert_eq!(reviews[0]["result"], "approved", "{reviews:?}");
}

/// `tree approve` with `flags` on a tree opened in `label`: refused by the command line with exit
/// 2, the store unchanged.
fn refused_by_the_command_line(label: &str, flags: &[&str]) {
    let tree = Tree::new(label);
    open_with(&tree, "review@1", "review.bindings.yaml");
    let before = every_file(&tree.store());
    let refused = tree
        .command()
        .args(["tree", "approve", "--store"])
        .arg(tree.store())
        .args(["--case", "root", "--principal", "reviewer"])
        .args(flags)
        .output()
        .unwrap();
    assert_eq!(
        refused.status.code(),
        Some(2),
        "{flags:?}: {}",
        stderr(&refused)
    );
    assert_eq!(every_file(&tree.store()), before, "{flags:?}");
}

#[test]
fn a_result_with_deny_is_refused() {
    refused_by_the_command_line(
        "result-deny",
        &["--capability", "c", "--deny", "--result", "rejected"],
    );
}

#[test]
fn a_result_without_an_action_is_refused() {
    refused_by_the_command_line(
        "result-no-action",
        &["--capability", "c", "--result", "rejected"],
    );
}

#[test]
fn reasons_and_a_reasons_file_together_are_refused() {
    refused_by_the_command_line(
        "reasons-both",
        &[
            "--action",
            "draft.review",
            "--result",
            "rejected",
            "--reasons",
            "short",
            "--reasons-file",
            "reasons.txt",
        ],
    );
}

#[test]
fn reasons_without_a_named_result_are_refused() {
    refused_by_the_command_line(
        "reasons-no-result",
        &["--action", "draft.review", "--reasons", "short"],
    );
}

#[test]
fn a_rejected_result_sends_the_writer_back_with_the_reasons() {
    let tree = reviewed("rejected-feedback");
    let reasons = "The draft has one line.\nAdd a second, then ask again.\n";
    let taken = review(
        &tree,
        "draft.review",
        &["--result", "rejected", "--reasons", reasons],
    );
    assert_eq!(taken.status.code(), Some(0), "{}", stderr(&taken));
    let ran = tree.run();
    assert_eq!(
        ran.status.code(),
        Some(3),
        "{}{}",
        stdout(&ran),
        stderr(&ran)
    );
    let writes = log_lines(&tree, "root")
        .iter()
        .filter(|line| finished(line) == Some("draft.write"))
        .count();
    assert_eq!(writes, 1, "the rejected draft is written again");
    let feedback: Value =
        serde_json::from_slice(&fs::read(tree.workspace().join("feedback.json")).unwrap()).unwrap();
    let records = feedback["records"].as_array().unwrap();
    assert_eq!(records.len(), 1, "{feedback}");
    let record = &records[0];
    assert_eq!(record["result"], "rejected", "{feedback}");
    assert_eq!(record["action"], "draft.review", "{feedback}");
    assert!(record["exit"].is_null(), "{feedback}");
    let digest = record["blob_sha256"].as_str().unwrap();
    let log = feedback["logs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|log| log["blob_sha256"] == digest)
        .unwrap_or_else(|| panic!("no log of {digest}: {feedback}"));
    assert!(
        log["tail"].as_str().unwrap().ends_with(reasons),
        "{feedback}"
    );
    assert_eq!(log["tail"], reasons, "{feedback}");
}

#[test]
fn reasons_out_of_bounds_or_outside_the_workspace_are_refused() {
    let tree = reviewed("reasons-bounds");
    let workspace = tree.workspace();
    fs::write(workspace.join("long.txt"), "x".repeat(65537)).unwrap();
    fs::write(workspace.join("lines.txt"), "line\n".repeat(61)).unwrap();
    fs::write(workspace.join("fits.txt"), "fits\n").unwrap();
    std::os::unix::fs::symlink(workspace.join("fits.txt"), workspace.join("link.txt")).unwrap();
    let outside = tree.root.join("outside.txt");
    fs::write(&outside, "outside\n").unwrap();
    let before = every_file(&tree.store());
    for file in [
        workspace.join("long.txt"),
        workspace.join("lines.txt"),
        workspace.join("link.txt"),
        outside,
        workspace.join("absent.txt"),
        workspace.join("stubs"),
    ] {
        let path = file.display().to_string();
        let refused = review(
            &tree,
            "draft.review",
            &["--result", "rejected", "--reasons-file", &path],
        );
        refused_naming(&refused, &[]);
        assert_eq!(every_file(&tree.store()), before, "{path}");
    }
    let refused = review(
        &tree,
        "draft.review",
        &["--result", "rejected", "--reasons", &"y".repeat(65537)],
    );
    refused_naming(&refused, &[]);
    assert_eq!(every_file(&tree.store()), before);
    // At the bounds the reasons are kept whole.
    fs::write(workspace.join("edge.txt"), "z\n".repeat(60)).unwrap();
    let path = workspace.join("edge.txt").display().to_string();
    let taken = review(
        &tree,
        "draft.review",
        &["--result", "rejected", "--reasons-file", &path],
    );
    assert_eq!(taken.status.code(), Some(0), "{}", stderr(&taken));
}

#[test]
fn the_reasons_are_kept_as_a_blob_not_in_the_log() {
    let tree = reviewed("reasons-blob");
    let reasons = "Marker 7f3a: the second paragraph repeats the first.";
    let taken = review(
        &tree,
        "draft.review",
        &["--result", "rejected", "--reasons", reasons],
    );
    assert_eq!(taken.status.code(), Some(0), "{}", stderr(&taken));
    assert!(!stdout(&taken).contains("7f3a"), "{}", stdout(&taken));
    let files = every_file(&tree.store());
    let holding: Vec<&PathBuf> = files
        .iter()
        .filter(|(_, bytes)| String::from_utf8_lossy(bytes).contains("7f3a"))
        .map(|(path, _)| path)
        .collect();
    assert_eq!(holding.len(), 1, "{holding:?}");
    assert_eq!(
        holding[0].parent().unwrap().file_name().unwrap(),
        "blobs",
        "{holding:?}"
    );
    assert_eq!(fs::read(holding[0]).unwrap(), reasons.as_bytes());
    // An approval with reasons keeps them too: `performed` is a named result like any other.
    let tree = reviewed("reasons-performed");
    let taken = review(
        &tree,
        "draft.review",
        &["--result", "performed", "--reasons", reasons],
    );
    assert_eq!(taken.status.code(), Some(0), "{}", stderr(&taken));
    assert!(
        every_file(&tree.store())
            .iter()
            .any(|(path, bytes)| bytes == reasons.as_bytes()
                && path.parent().unwrap().ends_with("blobs"))
    );
}

/// The fixture's protocols directory: `tour.yaml`, `tour-leg.yaml` and the subdirectory `old/`.
fn fixture_protocols() -> PathBuf {
    Path::new(FIXTURE).join("protocols")
}

/// The text of the fixture protocol file `name`.
fn fixture_protocol(name: &str) -> String {
    fs::read_to_string(fixture_protocols().join(name)).unwrap()
}

/// A protocols directory of the test holding `files`, by name.
fn protocols_dir(tree: &Tree, files: &[(&str, &str)]) -> PathBuf {
    let dir = tree.root.join("protocols");
    fs::create_dir_all(&dir).unwrap();
    for (name, text) in files {
        fs::write(dir.join(name), text).unwrap();
    }
    dir
}

/// The fixture's two protocols as a directory of the test.
fn tour_protocols(tree: &Tree) -> PathBuf {
    protocols_dir(
        tree,
        &[
            ("tour.yaml", &fixture_protocol("tour.yaml")),
            ("tour-leg.yaml", &fixture_protocol("tour-leg.yaml")),
        ],
    )
}

/// `tree open --protocols dir` of the root `protocol` with the bindings file `tour.bindings.yaml`.
fn open_from(tree: &Tree, dir: &Path, protocol: &str) -> Output {
    open_from_with(tree, dir, protocol, "tour.bindings.yaml")
}

/// `tree open --protocols dir` of the root `protocol` with the bindings file `bindings`.
fn open_from_with(tree: &Tree, dir: &Path, protocol: &str, bindings: &str) -> Output {
    tree.command()
        .args([
            "tree",
            "open",
            "--protocol",
            protocol,
            "--at",
            AT,
            "--store",
        ])
        .arg(tree.store())
        .arg("--protocols")
        .arg(dir)
        .arg("--bindings")
        .arg(tree.workspace().join(bindings))
        .arg("--workspace")
        .arg(tree.workspace())
        .output()
        .unwrap()
}

/// Asserts `output` is a refusal naming `reason` and each of `named`, and the store holds nothing.
fn refused_with_an_empty_store(tree: &Tree, output: &Output, reason: &str, named: &[&str]) {
    assert_ne!(output.status.code(), Some(0), "{}", stdout(output));
    let text = stderr(output);
    assert!(text.contains(reason), "{text}");
    for name in named {
        assert!(text.contains(name), "{name}: {text}");
    }
    let empty = fs::read_dir(tree.store()).map_or(true, |mut entries| entries.next().is_none());
    assert!(empty, "the store holds {:?}", tree.snapshot());
}

/// The SHA-256 `tree status` names for the protocol `registration`, which a case of the tree uses:
/// the status lists only those (`kept_entry` reads what `tree open` kept).
fn kept_sha256(tree: &Tree, registration: &str) -> String {
    let status = json(&tree.tree("status"));
    status["protocols"]
        .as_array()
        .unwrap()
        .iter()
        .find(|kept| kept["registration"] == registration)
        .and_then(|kept| kept["sha256"].as_str())
        .unwrap_or_else(|| panic!("no kept {registration}: {status}"))
        .to_owned()
}

/// The entries of the store's `tree-protocols.json`: what `tree open --protocols` kept.
fn kept_list(tree: &Tree) -> Vec<Value> {
    let path = tree.store().join(b10x_loom_tree::PROTOCOLS_FILE);
    let kept: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    kept["protocols"].as_array().unwrap().clone()
}

/// The entry `tree-protocols.json` keeps for `registration`.
fn kept_entry(tree: &Tree, registration: &str) -> Value {
    kept_list(tree)
        .into_iter()
        .find(|kept| kept["registration"] == registration)
        .unwrap_or_else(|| panic!("tree-protocols.json keeps no {registration}"))
}

#[test]
fn a_protocols_directory_registers_what_its_files_declare() {
    let tree = Tree::new("protocols-registered");
    let opened = open_from(&tree, &fixture_protocols(), "tour@1");
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let catalog = b10x_loom_tree::stored_catalog(&tree.store())
        .unwrap()
        .expect("a kept catalog");
    let names: Vec<&str> = catalog.iter().map(|entry| entry.name()).collect();
    assert_eq!(names, ["tour-leg@1", "tour@1"]);
    for entry in catalog.iter() {
        assert_eq!(
            entry.definition.source.kind,
            loom_protocols::SourceKind::Directory
        );
    }
    let kept = kept_list(&tree);
    let kept: Vec<(&str, &str)> = kept
        .iter()
        .map(|kept| {
            (
                kept["registration"].as_str().unwrap(),
                kept["file"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        kept,
        [("tour-leg@1", "tour-leg.yaml"), ("tour@1", "tour.yaml")]
    );
    // The status lists the protocols the tree's cases use, from the kept catalog: the root's alone
    // until a leg is opened, the entry `tree-protocols.json` keeps for it.
    let status = json(&tree.tree("status"));
    assert_eq!(
        status["protocols"],
        json!([kept_entry(&tree, "tour@1")]),
        "{status}"
    );
}

#[test]
fn a_tree_from_a_protocols_directory_runs_with_the_directory_deleted() {
    let tree = Tree::new("protocols-deleted");
    let dir = tour_protocols(&tree);
    let opened = open_from(&tree, &dir, "tour@1");
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    fs::remove_dir_all(&dir).unwrap();
    let (_, ended) = run_opened(&tree, None);
    assert_eq!(ended["detail"], "arrived", "{ended}");
}

#[test]
fn an_edit_after_tree_open_changes_nothing_in_the_open_tree() {
    let tree = Tree::new("protocols-edited");
    let dir = tour_protocols(&tree);
    let opened = open_from(&tree, &dir, "tour@1");
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let leg = fixture_protocol("tour-leg.yaml");
    let at_open = kept_entry(&tree, "tour-leg@1")["sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    // Under the edited definition no leg could pass, so the root could not arrive.
    fs::write(
        dir.join("tour-leg.yaml"),
        leg.replace("  passed:\n", "  finished:\n"),
    )
    .unwrap();
    let (_, ended) = run_opened(&tree, None);
    assert_eq!(ended["detail"], "arrived", "{ended}");
    // The status names the SHA-256 of the file as it was at `tree open`: the kept blob's bytes.
    assert_eq!(kept_sha256(&tree, "tour-leg@1"), at_open);
    let kept = fs::read_to_string(tree.store().join("blobs").join(&at_open)).unwrap();
    assert_eq!(kept, leg);
}

#[test]
fn nothing_in_a_run_selects_or_replaces_a_protocol() {
    let tree = Tree::new("protocols-unselected");
    let dir = tour_protocols(&tree);
    let leg = fixture_protocol("tour-leg.yaml");
    let workspace = tree.workspace();
    // Each leg's writer writes a `tour-leg` document of its own over the directory's file and into
    // the workspace, and prints it as its report.
    let replacing = fs::read_to_string(workspace.join("tour-leg.bindings.yaml"))
        .unwrap()
        .replace(
            "argv: [sh, stubs/route-write.sh]",
            &format!("argv: [sh, stubs/route-replace.sh, {}]", dir.display()),
        );
    fs::write(workspace.join("tour-leg-replace.bindings.yaml"), &replacing).unwrap();
    fs::write(
        workspace.join("tour-replace.bindings.yaml"),
        fs::read_to_string(workspace.join("tour.bindings.yaml"))
            .unwrap()
            .replace("tour-leg.bindings.yaml", "tour-leg-replace.bindings.yaml"),
    )
    .unwrap();
    let opened = open_from_with(&tree, &dir, "tour@1", "tour-replace.bindings.yaml");
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let at_open = json(&tree.tree("status"));
    // A child bindings file naming `tour@1` does not select the protocol of its case: the run is
    // refused and records nothing.
    fs::write(
        workspace.join("tour-leg-replace.bindings.yaml"),
        replacing.replace("protocol: tour-leg@1", "protocol: tour@1"),
    )
    .unwrap();
    let refused = tree.run();
    assert_ne!(refused.status.code(), Some(0), "{}", stdout(&refused));
    let text = stderr(&refused);
    assert!(
        text.contains("BindingsRefused") && text.contains("tour@1"),
        "{text}"
    );
    fs::write(workspace.join("tour-leg-replace.bindings.yaml"), &replacing).unwrap();
    assert_eq!(json(&tree.tree("status")), at_open);
    let (_, ended) = run_opened(&tree, None);
    assert_eq!(ended["detail"], "arrived", "{ended}");
    // The writers ran: the directory's file and the workspace hold their document.
    let written = fs::read_to_string(dir.join("tour-leg.yaml")).unwrap();
    assert!(
        written.contains("  finished:\n") && written != leg,
        "{written}"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("tour-leg.yaml")).unwrap(),
        written
    );
    // Every case keeps the registration and the SHA-256 of `tree open`: the status lists each
    // protocol the cases use as `tree-protocols.json` kept it at `tree open`.
    let status = json(&tree.tree("status"));
    assert_eq!(
        status["protocols"],
        json!([kept_entry(&tree, "tour@1"), kept_entry(&tree, "tour-leg@1")]),
        "{status}"
    );
    assert_eq!(
        at_open["protocols"],
        json!([kept_entry(&tree, "tour@1")]),
        "{at_open}"
    );
    let cases = status["cases"].as_array().unwrap();
    assert!(cases.len() > 2, "{status}");
    for case in cases {
        let expected = if case.get("parent").is_none() {
            "tour@1"
        } else {
            "tour-leg@1"
        };
        assert_eq!(case["protocol"], expected, "{case}");
    }
    let kept = kept_sha256(&tree, "tour-leg@1");
    assert_eq!(
        fs::read_to_string(tree.store().join("blobs").join(&kept)).unwrap(),
        leg
    );
}

#[test]
fn two_files_declaring_one_protocol_and_revision_are_refused() {
    let tree = Tree::new("protocols-twice");
    let tour = fixture_protocol("tour.yaml");
    let dir = tour_protocols(&tree);
    fs::write(dir.join("tour-copy.yaml"), &tour).unwrap();
    let opened = open_from(&tree, &dir, "tour@1");
    refused_with_an_empty_store(
        &tree,
        &opened,
        "DuplicateProtocol",
        &["tour-copy.yaml", "tour.yaml"],
    );
}

#[test]
fn two_files_declaring_one_protocol_id_are_refused() {
    let tree = Tree::new("protocols-two-revisions");
    let tour = fixture_protocol("tour.yaml");
    let dir = tour_protocols(&tree);
    fs::write(
        dir.join("tour-two.yaml"),
        tour.replacen("  revision: 1\n", "  revision: 2\n", 1),
    )
    .unwrap();
    let opened = open_from(&tree, &dir, "tour@1");
    refused_with_an_empty_store(
        &tree,
        &opened,
        "DuplicateProtocol",
        &["tour-two.yaml", "tour.yaml"],
    );
}

#[test]
fn a_protocol_canon_refuses_is_refused_naming_the_file() {
    let tree = Tree::new("protocols-invalid");
    let dir = tour_protocols(&tree);
    let invalid = fixture_protocol("tour-leg.yaml")
        .replace("  id: tour.leg\n", "  id: tour.broken\n")
        .replace("{claim: ticket.issued}", "{claim: no.such.claim}");
    fs::write(dir.join("broken.yaml"), invalid).unwrap();
    let opened = open_from(&tree, &dir, "tour@1");
    refused_with_an_empty_store(&tree, &opened, "ProtocolRefused", &["broken.yaml"]);
    let detail = &json_line(&opened)["detail"];
    assert!(detail.as_str().unwrap().len() <= 1024 + 6, "{detail}");
}

/// The JSON object of the refusal's second line on standard error.
fn json_line(output: &Output) -> Value {
    let text = stderr(output);
    let line = text.lines().find(|line| line.starts_with('{')).unwrap();
    serde_json::from_str(line).unwrap()
}

#[test]
fn a_link_leaving_the_protocols_directory_is_refused() {
    let tree = Tree::new("protocols-link");
    let dir = tour_protocols(&tree);
    let outside = tree.root.join("outside.yaml");
    fs::write(
        &outside,
        fixture_protocol("tour-leg.yaml").replace("  id: tour.leg\n", "  id: tour.outside\n"),
    )
    .unwrap();
    std::os::unix::fs::symlink(&outside, dir.join("elsewhere.yaml")).unwrap();
    let opened = open_from(&tree, &dir, "tour@1");
    refused_with_an_empty_store(
        &tree,
        &opened,
        "ProtocolLinkLeavesDirectory",
        &["elsewhere.yaml", "outside.yaml"],
    );
}

#[test]
fn the_root_protocol_must_be_in_the_protocols_directory() {
    let tree = Tree::new("protocols-root");
    let opened = open_from(&tree, &fixture_protocols(), "system-query@1");
    refused_with_an_empty_store(
        &tree,
        &opened,
        "ProtocolNotInDirectory",
        &["system-query@1"],
    );
}

#[test]
fn a_child_protocol_the_directory_lacks_is_refused() {
    let tree = Tree::new("protocols-child");
    let tour = fixture_protocol("tour.yaml").replacen(
        "case_open: {protocol: tour.leg,",
        "case_open: {protocol: tour.detour,",
        1,
    );
    let dir = protocols_dir(
        &tree,
        &[
            ("tour.yaml", &tour),
            ("tour-leg.yaml", &fixture_protocol("tour-leg.yaml")),
        ],
    );
    let opened = open_from(&tree, &dir, "tour@1");
    refused_with_an_empty_store(
        &tree,
        &opened,
        "ProtocolNotInDirectory",
        &["tour.detour", "tour.yaml"],
    );
}

#[test]
fn a_directory_with_no_protocol_file_is_refused() {
    let tree = Tree::new("protocols-none");
    let dir = protocols_dir(&tree, &[("notes.txt", "not a protocol\n")]);
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("sub/tour.yaml"), fixture_protocol("tour.yaml")).unwrap();
    let opened = open_from(&tree, &dir, "tour@1");
    refused_with_an_empty_store(&tree, &opened, "NoProtocolFiles", &[]);
}

#[test]
fn a_changed_stored_protocol_is_refused() {
    let tree = Tree::new("protocols-changed");
    let dir = tour_protocols(&tree);
    let opened = open_from(&tree, &dir, "tour@1");
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let blob = tree
        .store()
        .join("blobs")
        .join(kept_entry(&tree, "tour-leg@1")["sha256"].as_str().unwrap());
    let mut bytes = fs::read(&blob).unwrap();
    bytes[0] ^= 1;
    fs::write(&blob, &bytes).unwrap();
    let before = tree.snapshot();
    let ran = tree.run();
    assert_ne!(ran.status.code(), Some(0), "{}", stdout(&ran));
    let text = stderr(&ran);
    assert!(text.contains("StoredProtocolChanged"), "{text}");
    assert!(text.contains("tour-leg@1"), "{text}");
    assert_eq!(tree.snapshot(), before);
}

// ---- the test sync switch (story:tree-run-tests-fit-the-budget) ----------------------------------

/// The syncs every `b10x-loom` a closure of [`counted`] started made and skipped, summed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Synced {
    synced: u64,
    skipped: u64,
}

/// Runs `run` with the switch at `switch` (`None`: not in the environment) on every command it
/// starts, each appending its counts to one fresh file, and returns what `run` answered and the
/// counts summed. Fails when no command wrote its counts.
fn counted<T>(switch: Option<&str>, run: impl FnOnce() -> T) -> (T, Synced) {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let file = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("sync-counts-{}-{nanos}", std::process::id()));
    let before = SWITCH_VALUE.replace(switch.map(str::to_owned));
    COUNTS_FILE.set(Some(file.clone()));
    let answered = run();
    COUNTS_FILE.set(None);
    SWITCH_VALUE.set(before);
    let text = fs::read_to_string(&file).unwrap_or_default();
    let _ = fs::remove_file(&file);
    let mut sum = Synced {
        synced: 0,
        skipped: 0,
    };
    for line in text.lines() {
        let counts: Value = serde_json::from_str(line).unwrap();
        sum.synced += counts["synced"].as_u64().unwrap();
        sum.skipped += counts["skipped"].as_u64().unwrap();
    }
    assert!(
        text.lines().count() > 0,
        "no b10x-loom wrote its sync counts"
    );
    (answered, sum)
}

#[test]
fn the_switch_is_off_unless_its_variable_is_1() {
    for value in [None, Some("0"), Some(" 1")] {
        let tree = Tree::new("switch-off");
        let (opened, counts) = counted(value, || tree.open_leg());
        assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
        assert!(
            counts.synced > 0 && counts.skipped == 0,
            "{value:?}: {counts:?}"
        );
    }
    let tree = Tree::new("switch-on");
    let (opened, counts) = counted(Some("1"), || tree.open_leg());
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    assert!(counts.synced == 0 && counts.skipped > 0, "{counts:?}");
}

/// Read from the shared ended tree, whose every command ran with the switch on and counted.
#[test]
fn with_the_switch_a_two_level_tree_makes_no_sync() {
    let Ended { tree, ended, .. } = ended_tree();
    assert_eq!(ended["stop"], "Terminal", "{ended}");
    let counts: Value =
        serde_json::from_slice(&fs::read(tree.root.join(SYNCED_FILE)).unwrap()).unwrap();
    assert_eq!(counts["synced"], 0, "{counts}");
    assert!(counts["skipped"].as_u64().unwrap() > 0, "{counts}");
}

/// The same tree with the switch off syncs every write the switch skipped: about a thousand syncs,
/// so in the slow lane (`task slow`).
#[test]
fn slow_without_the_switch_the_two_level_tree_syncs_what_the_switch_skips() {
    let tree = Tree::new("proxy-on-again");
    let ((_, ended), on) = counted(Some("1"), || run_to_end_twice(&tree, None));
    assert_eq!(ended["stop"], "Terminal", "{ended}");
    let tree = Tree::new("proxy-off");
    let ((_, ended), off) = counted(None, || run_to_end_twice(&tree, None));
    assert_eq!(ended["stop"], "Terminal", "{ended}");
    assert_eq!(
        off,
        Synced {
            synced: on.skipped,
            skipped: 0
        },
        "with the switch: {on:?}"
    );
}

/// A shell program that prints its environment, then checks the route as `route-check.sh` does.
const ENV_CHECK: &str =
    "#!/bin/sh\nenv\ntest -f route.txt || exit 1\ntest \"$(wc -l < route.txt)\" -ge 2\n";

#[test]
fn a_bound_program_never_sees_the_switch() {
    let tree = Tree::new("program-env");
    fs::write(tree.workspace().join("stubs/env-check.sh"), ENV_CHECK).unwrap();
    let bindings = fs::read_to_string(tree.workspace().join("tour-leg.bindings.yaml"))
        .unwrap()
        .replace("stubs/route-check.sh", "stubs/env-check.sh");
    fs::write(tree.workspace().join("env.bindings.yaml"), bindings).unwrap();
    let opened = tree.open_leg_with("env.bindings.yaml");
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let (ran, counts) = counted(Some("1"), || tree.run());
    assert_eq!(
        ran.status.code(),
        Some(3),
        "{}{}",
        stdout(&ran),
        stderr(&ran)
    );
    assert!(counts.skipped > 0, "the switch was on: {counts:?}");

    let mut printed = 0;
    for entry in fs::read_dir(tree.store().join("blobs")).unwrap() {
        let blob = fs::read_to_string(entry.unwrap().path()).unwrap_or_default();
        if blob.contains("LOOM_CASE=") {
            printed += 1;
        }
        assert!(
            !blob.contains("B10X_LOOM_TEST_UNSYNCED_STORE"),
            "a bound program saw the switch:\n{blob}"
        );
    }
    assert!(printed > 0, "no blob holds a program's environment");
}

#[test]
fn a_bindings_or_protocol_member_cannot_set_the_switch() {
    let leg = fs::read_to_string(Path::new(FIXTURE).join("tour-leg.bindings.yaml")).unwrap();
    for (label, member) in [
        (
            "top",
            leg.replace(
                "actions:\n",
                "B10X_LOOM_TEST_UNSYNCED_STORE: \"1\"\nactions:\n",
            ),
        ),
        (
            "executor",
            leg.replacen(
                "      kind: program\n",
                "      kind: program\n      B10X_LOOM_TEST_UNSYNCED_STORE: \"1\"\n",
                1,
            ),
        ),
    ] {
        assert_ne!(member, leg, "{label}");
        let tree = Tree::new("switch-member");
        fs::write(tree.workspace().join("member.bindings.yaml"), member).unwrap();
        let (opened, _) = counted(None, || tree.open_leg_with("member.bindings.yaml"));
        refused_with_an_empty_store(
            &tree,
            &opened,
            "UndeclaredMember",
            &["B10X_LOOM_TEST_UNSYNCED_STORE"],
        );
    }

    // A program argv and a protocol description naming the switch, run twice: with the switch on,
    // then off, where the run must sync every write the first skipped. The writer adds both stops
    // at once, so the run checks the route once before it waits: half the writes of the fixture's.
    let bindings = leg.replace(
        "argv: [sh, stubs/route-write.sh]",
        "argv: [env, B10X_LOOM_TEST_UNSYNCED_STORE=1, sh, stubs/route-write-both.sh]",
    );
    assert_ne!(bindings, leg);
    let protocol = fixture_protocol("tour-leg.yaml").replace(
        "One leg of a walking tour.",
        "B10X_LOOM_TEST_UNSYNCED_STORE=1 One leg of a walking tour.",
    );
    let open_and_run = |label: &str, switch: Option<&str>| {
        let tree = Tree::new(label);
        fs::write(tree.workspace().join("switch.bindings.yaml"), &bindings).unwrap();
        fs::write(
            tree.workspace().join("stubs/route-write-both.sh"),
            "#!/bin/sh\necho stop >> route.txt\necho stop >> route.txt\n",
        )
        .unwrap();
        let dir = protocols_dir(&tree, &[("tour-leg.yaml", &protocol)]);
        let ((opened, ran), counts) = counted(switch, || {
            (
                open_from_with(&tree, &dir, "tour-leg@1", "switch.bindings.yaml"),
                tree.run(),
            )
        });
        assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
        assert_eq!(
            ran.status.code(),
            Some(3),
            "{}{}",
            stdout(&ran),
            stderr(&ran)
        );
        counts
    };
    let on = open_and_run("switch-documents-on", Some("1"));
    assert!(on.synced == 0 && on.skipped > 0, "{on:?}");
    let off = open_and_run("switch-documents-off", None);
    assert_eq!(
        off,
        Synced {
            synced: on.skipped,
            skipped: 0
        },
        "with the switch: {on:?}"
    );
}

#[test]
fn a_person_step_syncs_in_full() {
    let step = |label: &str, switch: Option<&str>| {
        let (tree, decisions) = signpost(label);
        let decision = about(&decisions, "post.sign");
        let (taken, counts) = counted(switch, || tree.printed(&decision));
        assert_eq!(
            taken.status.code(),
            Some(0),
            "{}{}",
            stdout(&taken),
            stderr(&taken)
        );
        assert_eq!(records_of(&tree, "root", "post_signed").len(), 1);
        counts
    };
    let on = step("person-on", Some("1"));
    assert!(on.synced == 0 && on.skipped > 0, "{on:?}");
    let off = step("person-off", None);
    assert_eq!(
        off,
        Synced {
            synced: on.skipped,
            skipped: 0
        },
        "with the switch: {on:?}"
    );
}

#[test]
fn the_tree_run_binary_has_the_20_s_override() {
    let id = concat!(env!("CARGO_PKG_NAME"), "::", env!("CARGO_CRATE_NAME"));
    assert_eq!(id, "b10x-loom-cli::tree_run");
    if let Some(running) = std::env::var_os("NEXTEST_BINARY_ID") {
        assert_eq!(running, id, "nextest names this binary otherwise");
    }
    let config = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.config/nextest.toml"),
    )
    .unwrap();
    let held = config.split("[[").any(|table| {
        table.starts_with("profile.default.overrides]]")
            && table.contains(&format!("binary_id({id})"))
            && table.contains(r#"slow-timeout = { period = "20s", terminate-after = 1 }"#)
    });
    assert!(held, "no override kills a test of {id} at 20 s:\n{config}");
}
