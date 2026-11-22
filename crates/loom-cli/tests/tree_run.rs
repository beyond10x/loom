//! Acceptance for `story:tree-run-command`: `b10x-loom tree` opens, verifies and prints a tree of
//! protocol cases kept in a store directory.
//!
//! The fixture is `fixtures/tree-two-level/`, written for these tests and shaped like a parent and
//! one child: the protocols `tour` (`tour@1`) and `tour.leg` (`tour-leg@1`), their bindings files
//! and stub programs that end at once. Each test installs the two protocols into its own scratch
//! user's store (in process, a few milliseconds), copies the stubs into a fresh workspace and runs
//! the binary with `HOME` and `XDG_DATA_HOME` at that scratch directory; no test reaches the
//! network, a model or a credential, and each asserts it ended within 60 s.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
use b10x_loom_intake_slice::confinement::{ConfinementAdapter, EngineConfinement, EngineRunner};
use b10x_loom_tree::model::{TreeRefusalReason, TreeStop};
use b10x_loom_tree::runlog::{RunLogEvent, RunLogLine};
use b10x_loom_tree::{
    RunSettings, STATUS_FORMAT, TreeTime, log_file_name, read_log, read_status, run_tree,
    status_json, tree_status,
};
use loom_protocols::{InstallStore, ProtocolCatalog};
use serde_json::{Value, json};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tree-two-level");

/// The bound of every test of this file.
const BOUND: Duration = Duration::from_secs(60);

/// The instant every write of a command records in these tests.
const AT: &str = "2026-11-22T08:00:00Z";

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

    fn command(&self) -> Command {
        let home = self.root.join("home");
        let mut command = Command::new(env!("CARGO_BIN_EXE_b10x-loom"));
        command
            .env("HOME", &home)
            .env("XDG_DATA_HOME", &home)
            .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
            .env_remove("RUST_LOG");
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
fn a_cancelled_run_ends_budget_spent_before_its_next_step() {
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
    assert_eq!(ran.stop, TreeStop::BudgetSpent, "{ran:?}");
    assert_eq!(ran.steps, 1, "{ran:?}");
    let written = log_lines(&tree, "root");
    let last = written.last().unwrap();
    assert!(
        matches!(&last.event, RunLogEvent::Stopped(stopped) if stopped.stop == "BudgetSpent"),
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
        let (waits, ended) = run_to_end_twice(&tree, Some("ticket.issue"));
        fs::write(root.join("waits.json"), serde_json::to_vec(&waits).unwrap()).unwrap();
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
        let store = tree.store().display().to_string();
        assert_eq!(
            decision["command"],
            json!(["b10x-loom", "tree", "run", "--store", store])
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
        .arg(tree.workspace().join("tour.bindings.yaml"))
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

/// The SHA-256 `tree status` names for the kept protocol `registration`.
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
    let status = json(&tree.tree("status"));
    let kept: Vec<(&str, &str)> = status["protocols"]
        .as_array()
        .unwrap()
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
    let at_open = kept_sha256(&tree, "tour-leg@1");
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
        .join(kept_sha256(&tree, "tour-leg@1"));
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
