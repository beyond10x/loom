//! Acceptance for `story:tree-run-command`: `b10x-loom tree` opens, verifies and prints a tree of
//! protocol cases kept in a store directory.
//!
//! The fixture is `fixtures/tree-two-level/`, written for these tests and shaped like a parent and
//! one child: the protocols `tour` (`tour@1`) and `tour.leg` (`tour-leg@1`), their bindings files
//! and stub programs that end at once. Each test installs the two protocols into its own scratch
//! user's store (in process, a few milliseconds), copies the stubs into a fresh workspace and runs
//! the binary with `HOME` and `XDG_DATA_HOME` at that scratch directory; no test reaches the
//! network, a model or a credential, and each asserts it ended within 60 s.

use std::collections::BTreeMap;
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
    RunSettings, STATUS_FORMAT, TreeTime, decision_json, log_file_name, read_log, read_status,
    run_tree, status_json, tree_status,
};
use loom_protocols::{InstallStore, ProtocolCatalog};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

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
}

impl Tree {
    fn new(label: &str) -> Self {
        let started = Instant::now();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("tree-run-{label}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(root.join("home")).unwrap();
        fs::create_dir_all(root.join("workspace/stubs")).unwrap();
        let installed = InstallStore::new(root.join("home/loom/protocols"));
        for (name, file) in [("tour@1", "tour.yaml"), ("tour-leg@1", "tour-leg.yaml")] {
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
        Self { root, started }
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
        let _ = fs::remove_dir_all(&self.root);
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
/// until a run exits other than 3: per wait the run's JSON and the status read at it, and the last
/// run's JSON.
fn run_to_end(tree: &Tree) -> (Vec<(Value, Value)>, Value) {
    run_to_end_twice(tree, None)
}

/// [`run_to_end`], taking the decision on `twice` two times in a row.
fn run_to_end_twice(tree: &Tree, twice: Option<&str>) -> (Vec<(Value, Value)>, Value) {
    let opened = tree.open();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
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
    let tree = Tree::new("status-link");
    let (waits, ended) = run_to_end(&tree);
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
    let tree = Tree::new("status-records");
    run_to_end(&tree);
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
    let tree = Tree::new("status-runs");
    run_to_end(&tree);
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
    let tree = Tree::new("status-approvals");
    // The grant of the ticket is recorded twice at one revision: two entries, one decision in
    // the approvals file.
    run_to_end_twice(&tree, Some("ticket.issue"));
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
    let tree = Tree::new("status-copies");
    run_to_end(&tree);
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
    let tree = Tree::new("status-value");
    run_to_end(&tree);
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

/// A `child opened` or `child closed` line an observer was given: the case whose log holds it,
/// the line, and the digests of the `case_outcome` records that case's file held right then.
struct ChildLine {
    case: String,
    line: RunLogLine,
    held: BTreeMap<String, String>,
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
/// line the observer was given, in order.
fn child_lines_to_end(tree: &Tree) -> Vec<ChildLine> {
    let opened = tree.open();
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let catalog = tree.catalog();
    let store = tree.store();
    let mut seen: Vec<ChildLine> = Vec::new();
    for _ in 0..8 {
        let mut observer = |case: &str, line: &RunLogLine| {
            if record_of(line).is_some() {
                seen.push(ChildLine {
                    case: case.to_owned(),
                    line: line.clone(),
                    held: record_digests(&store, case),
                });
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

#[test]
fn each_write_of_a_childs_record_appears_in_exactly_one_parent_line() {
    let tree = Tree::new("child-record-lines");
    let seen = child_lines_to_end(&tree);
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
                .find(|given| given.case == "root" && given.line == *line)
                .unwrap_or_else(|| panic!("{child}: the observer was not given {line:?}"));
            let (_, digest) = record_of(line).unwrap();
            assert_eq!(
                digest,
                given.held.get(record).map(String::as_str),
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
    let tree = Tree::new("child-record-digests");
    let (_, ended) = run_to_end(&tree);
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
    let tree = Tree::new("child-record-observer");
    let seen = child_lines_to_end(&tree);
    let written: Vec<RunLogLine> = log_lines(&tree, "root")
        .into_iter()
        .filter(|line| record_of(line).is_some())
        .collect();
    let observed: Vec<RunLogLine> = seen
        .iter()
        .filter(|given| given.case == "root")
        .map(|given| given.line.clone())
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
        let (record, digest) = record_of(line).unwrap();
        assert!(
            record.is_some() && digest.is_some(),
            "the observer is given the record of {line:?}"
        );
    }
}
