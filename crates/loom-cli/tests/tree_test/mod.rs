//! Acceptance for `story:tree-suite-format`: `b10x-loom tree test` runs a `loom.tree-suite/1`
//! suite one cycle after another and reports per cycle whether it ended as the suite expects.
//!
//! The fixture is `fixtures/tree-suite/`: bindings of the `tree-two-level` protocols
//! (`fixtures/tree-two-level/protocols`, given as `--protocols`), stub programs that sleep 0.3 s,
//! and a suite of five cycles. The binary's run over it is made once per test run, kept under
//! `target/` and keyed by the SHA-256 of what it is made from (see [`ran`]); every test reads it.
//! No test reaches the network, a model or a credential; every program runs under the `local`
//! runner. Each test asserts its own time under 5 s, the shared run apart, which asserts the
//! 60 s bound of one test: the story's 10 s for it is measured, not asserted, because each tree
//! command fsyncs every write and on a loaded host that alone takes most of 10 s.
//!
//! Acceptance for `story:suite-runs-cycles-in-parallel` follows: workers, the order of results,
//! cancel and `--only`. Its suites are `fixtures/tree-suite/parallel/` (one protocol, `step`, whose
//! one action runs the program seeded as `stubs/program.sh`) and `fixtures/tree-suite/only.suite.yaml`
//! (five cycles whose one step is `open`). A test that needs the barrier copies `parallel/` beside
//! its own directory and writes the barrier directory's path into the copy (see [`Parallel`]).
//!
//! Acceptance for `story:suite-cycle-writes-a-workspace-file`, the step `write`, is in `write.rs`.
//!
//! Acceptance for `story:suite-runs-cycles-under-one-cgroup-root` is at the end: the layout
//! `<scratch>/work/<n>/` and `<scratch>/store/<n>/`, cycles side by side under `substrate` on one
//! held cgroup root, and the refusals of `tree test`'s placement. Its fixtures are
//! `fixtures/tree-suite/substrate/`; its delegated tests use `tests/common/delegated_cgroup.rs`.

mod substrate;
mod write;

use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
use b10x_loom_intake_slice::confinement::{ConfinementAdapter, EngineConfinement, EngineRunner};
use b10x_loom_tree::embed::TreeRunner;
use b10x_loom_tree::model::{CycleResult, DifferenceReason, SuiteReport};
use b10x_loom_tree::suite::{SuiteSettings, report_json, run_suite, workers_served};
use b10x_loom_tree::{TreeTime, read_protocols_directory, verify_tree};

use crate::common::delegated_cgroup::{DELEGATED, Scope, delegation_present};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tree-suite");

const PROTOCOLS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/tree-two-level/protocols"
);

/// The instant the suite's steps count their minutes from.
const AT: &str = "2026-11-22T08:00:00Z";

/// The text the route check prints and writes into `check.log`.
const CANARY: &str = "CANARY-c4f1e9";

/// The bound of each test, the shared run apart.
const TEST_BOUND: Duration = Duration::from_secs(5);

/// The bound of the shared run of the fixture suite: the bound of one test.
const RUN_BOUND: Duration = Duration::from_secs(60);

/// What the one run of the fixture suite left: its directory (`home/`, `scratch/`,
/// `report.json`), its output, its exit status and its wall time.
struct Ran {
    root: PathBuf,
    stdout: String,
    stderr: String,
    code: Option<i32>,
    seconds: f64,
}

impl Ran {
    fn report(&self) -> Value {
        serde_json::from_slice(&fs::read(self.root.join("report.json")).unwrap()).unwrap()
    }

    fn line(&self, name: &str) -> &str {
        let prefix = format!("{name}: ");
        self.stdout
            .lines()
            .find(|line| line.starts_with(&prefix))
            .unwrap_or_else(|| panic!("no line for {name}: {}{}", self.stdout, self.stderr))
    }
}

/// Every file below `dir`, in path order.
fn files(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                walk(&entry.path(), found);
            } else {
                found.push(entry.path());
            }
        }
    }
    let mut found = Vec::new();
    walk(dir, &mut found);
    found.sort();
    found
}

/// The SHA-256, as hex, of the fixture's files and the protocols (path, length, bytes), the
/// `b10x-loom` binary and this test binary.
fn digest() -> String {
    let mut hasher = Sha256::new();
    for dir in [FIXTURE, PROTOCOLS] {
        for file in files(Path::new(dir)) {
            let bytes = fs::read(&file).unwrap();
            hasher.update(file.as_os_str().as_encoded_bytes());
            hasher.update([0]);
            hasher.update((bytes.len() as u64).to_be_bytes());
            hasher.update(&bytes);
        }
    }
    hasher.update(fs::read(env!("CARGO_BIN_EXE_b10x-loom")).unwrap());
    hasher.update(fs::read(std::env::current_exe().unwrap()).unwrap());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// `b10x-loom` with `HOME` and `XDG_DATA_HOME` at `home`, and the test sync switch on, as
/// `tree_run.rs` starts it: a cycle's tree commands wait for no disk sync (about 2 s of a 2.2 s
/// cycle on a loaded host), and the durability tests of the stores keep full sync.
fn command(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_b10x-loom"));
    switched(&mut command, home);
    command
}

/// `command`, which starts `b10x-loom` itself or through `enter.sh`, with the environment
/// [`command`] gives it.
fn switched<'c>(command: &'c mut Command, home: &Path) -> &'c mut Command {
    command
        .env("B10X_LOOM_TEST_UNSYNCED_STORE", "1")
        .env("HOME", home)
        .env("XDG_DATA_HOME", home)
        .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
        .env_remove("RUST_LOG")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The fixture suite run once per test run by the built `b10x-loom tree test`, under
/// `target/` beside the test scratch directories, keyed by [`digest`]. The first test that asks
/// runs it holding a lock file and writes `ran.json` last; the others wait on the lock and read
/// what it left.
fn ran() -> Ran {
    let key = digest();
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let root = base.join(format!("tree-test-{}", &key[..16]));
    let lock = fs::File::create(base.join(format!("tree-test-{}.lock", &key[..16]))).unwrap();
    lock.lock().unwrap();
    if !root.join("ran.json").exists() {
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("home")).unwrap();
        let started = Instant::now();
        let output = command(&root.join("home"))
            .current_dir(&root)
            .args(["tree", "test", "--confinement", "local", "--at", AT])
            .arg("--suite")
            .arg(Path::new(FIXTURE).join("suite.yaml"))
            .arg("--protocols")
            .arg(PROTOCOLS)
            .arg("--scratch")
            .arg(root.join("scratch"))
            .arg("--report")
            .arg(root.join("report.json"))
            .output()
            .unwrap();
        let seconds = started.elapsed().as_secs_f64();
        let kept = json!({
            "stdout": text(&output.stdout),
            "stderr": text(&output.stderr),
            "code": output.status.code(),
            "seconds": seconds,
        });
        fs::write(root.join("ran.json"), kept.to_string()).unwrap();
    }
    lock.unlock().unwrap();
    let kept: Value = serde_json::from_slice(&fs::read(root.join("ran.json")).unwrap()).unwrap();
    let ran = Ran {
        stdout: kept["stdout"].as_str().unwrap().to_owned(),
        stderr: kept["stderr"].as_str().unwrap().to_owned(),
        code: kept["code"]
            .as_i64()
            .map(|code| i32::try_from(code).unwrap()),
        seconds: kept["seconds"].as_f64().unwrap(),
        root,
    };
    assert!(
        ran.seconds < RUN_BOUND.as_secs_f64(),
        "the fixture run took {} s: {}",
        ran.seconds,
        ran.stdout
    );
    ran
}

/// Asserts a test, the shared run apart, ended within [`TEST_BOUND`].
fn within_bound(started: Instant) {
    let spent = started.elapsed();
    assert!(spent < TEST_BOUND, "the test took {spent:?}");
}

#[test]
fn the_first_runner_reports_each_cycle() {
    let ran = ran();
    let started = Instant::now();
    let arrives = ran.line("arrives");
    assert!(
        arrives.starts_with("arrives: as expected (") && arrives.ends_with(" s)"),
        "{}{}",
        ran.stdout,
        ran.stderr
    );
    let waits = ran.line("waits");
    assert!(
        waits.starts_with("waits: as expected (") && waits.ends_with(" s)"),
        "{}",
        ran.stdout
    );
    let wrong = ran.line("wrong");
    assert!(
        wrong.starts_with(
            "wrong: FAILED: expect.outcome (outcome, OutcomeDiffers): expected arrived, found turned_back ("
        ),
        "{}",
        ran.stdout
    );
    assert_eq!(
        ran.line("allows"),
        "allows: pending, waits for story:tree-stop-record-and-labels (allow)"
    );
    assert!(
        ran.line("later").starts_with(
            "later: pending, still failing: check-log-says-hello (equals, CheckFailed)"
        ),
        "{}",
        ran.stdout
    );
    within_bound(started);
}

#[test]
fn the_summary_counts_expected_pending_and_waiting() {
    let ran = ran();
    let started = Instant::now();
    let lines: Vec<&str> = ran.stdout.lines().collect();
    // Five cycle lines, the two summary lines and the `slowest:` line.
    assert_eq!(lines.len(), 8, "{}", ran.stdout);
    assert_eq!(
        lines[5],
        "cycles: 3 expected, 2 as expected, 1 failed: wrong"
    );
    assert_eq!(
        lines[6],
        "pending: 2 (1 still failing: later; passing, move to expected: none; 1 waiting: allows on story:tree-stop-record-and-labels)"
    );
    assert_eq!(ran.code, Some(1), "{}", ran.stderr);
    let report = ran.report();
    assert_eq!(report["expected"], 3, "{report}");
    assert_eq!(report["as_expected"], 2, "{report}");
    assert_eq!(report["failed"], json!(["wrong"]), "{report}");
    assert_eq!(report["pending"], 2, "{report}");
    assert_eq!(report["pending_passing"], json!([]), "{report}");
    assert_eq!(
        report["waiting"],
        json!([{"name": "allows", "story": "story:tree-stop-record-and-labels", "kind": "allow"}]),
        "{report}"
    );
    assert_eq!(report["cycles"].as_array().unwrap().len(), 5, "{report}");
    within_bound(started);
}

#[test]
fn the_first_runner_stays_in_scratch() {
    let ran = ran();
    let started = Instant::now();
    let catalog = read_protocols_directory(Path::new(PROTOCOLS))
        .unwrap()
        .catalog;
    let mut stores = 0;
    for cycle in fs::read_dir(ran.root.join("scratch/store")).unwrap() {
        let store = cycle.unwrap().path();
        let verified = verify_tree(&store, &catalog).unwrap();
        assert!(
            verified.failure.is_none(),
            "{}: {verified:?}",
            store.display()
        );
        stores += 1;
    }
    // Four cycles ran (`allows` waits); each left its store.
    assert_eq!(stores, 4);
    // Nothing beside the scratch directory, the report and the test's own record; nothing in HOME.
    let mut left: Vec<String> = fs::read_dir(&ran.root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["home", "ran.json", "report.json", "scratch"]);
    assert_eq!(fs::read_dir(ran.root.join("home")).unwrap().count(), 0);
    // A non-empty --scratch is refused before the protocols are read: these do not exist.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("tree-test-full-{}-{nanos}", std::process::id()));
    fs::create_dir_all(dir.join("scratch")).unwrap();
    fs::write(dir.join("scratch/left.txt"), "left").unwrap();
    let refused: Output = command(&dir)
        .args(["tree", "test", "--confinement", "local", "--at", AT])
        .arg("--suite")
        .arg(Path::new(FIXTURE).join("suite.yaml"))
        .arg("--protocols")
        .arg(dir.join("no-protocols"))
        .arg("--scratch")
        .arg(dir.join("scratch"))
        .output()
        .unwrap();
    let stderr = text(&refused.stderr);
    assert_eq!(refused.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("ScratchNotEmpty"), "{stderr}");
    assert!(!stderr.contains("ProtocolsUnreadable"), "{stderr}");
    assert!(refused.stdout.is_empty());
    let _ = fs::remove_dir_all(&dir);
    within_bound(started);
}

#[test]
fn program_output_never_enters_the_summary() {
    let ran = ran();
    let started = Instant::now();
    // The route check of `later` (cycle 4) ran, and its canary is in the file its check reads.
    let kept = fs::read_to_string(ran.root.join("scratch/work/4/check.log")).unwrap();
    assert!(kept.contains(CANARY), "{kept}");
    let later = ran.line("later");
    assert!(later.contains(", found 26 bytes, sha256 "), "{later}");
    assert!(!ran.stdout.contains(CANARY), "{}", ran.stdout);
    assert!(!ran.stderr.contains(CANARY), "{}", ran.stderr);
    let report = fs::read_to_string(ran.root.join("report.json")).unwrap();
    assert!(!report.contains(CANARY), "{report}");
    within_bound(started);
}

/// The `local` runner, as `tree test --confinement local` opens it.
fn local_settings(scratch: PathBuf) -> SuiteSettings {
    let runner = EngineRunner::open(&EngineConfinement {
        adapter: ConfinementAdapter::Local,
        local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
        cgroup_root: None,
    })
    .unwrap();
    SuiteSettings::new(
        PathBuf::from(PROTOCOLS),
        scratch,
        TreeTime::At(AT.to_owned()),
        Arc::new(runner),
        Some("local".to_owned()),
    )
}

fn fresh_scratch(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("tree-test-{label}-{}-{nanos}", std::process::id()))
}

#[test]
fn run_suite_runs_a_suite_without_the_command() {
    let started = Instant::now();
    let scratch = fresh_scratch("library");
    let mut seen: Vec<String> = Vec::new();
    let mut observe = |result: &CycleResult| seen.push(result.name.clone());
    let report = run_suite(
        &Path::new(FIXTURE).join("one.suite.yaml"),
        &local_settings(scratch.clone()),
        Some(&mut observe),
        None,
    )
    .unwrap();
    assert_eq!(seen, ["opens"]);
    assert_eq!((report.expected, report.as_expected), (1, 1), "{report:?}");
    // No `run` step: no exit of one.
    assert_eq!(report.cycles[0].exit, None, "{report:?}");
    assert!(scratch.join("store/1/tree.json").is_file());
    let _ = fs::remove_dir_all(&scratch);
    within_bound(started);
}

#[test]
fn a_set_cancel_flag_runs_no_cycle() {
    let started = Instant::now();
    let scratch = fresh_scratch("cancelled");
    let cancel = AtomicBool::new(true);
    let report = run_suite(
        &Path::new(FIXTURE).join("one.suite.yaml"),
        &local_settings(scratch.clone()),
        None,
        Some(&cancel),
    )
    .unwrap();
    assert!(report.cycles.is_empty(), "{report:?}");
    assert!(!scratch.join("work/1").exists());
    assert!(!scratch.join("store/1").exists());
    let _ = fs::remove_dir_all(&scratch);
    within_bound(started);
}

#[test]
fn a_seed_that_is_a_link_is_refused_before_any_cycle() {
    let started = Instant::now();
    let dir = fresh_scratch("seed-link");
    fs::create_dir_all(dir.join("suite/stubs")).unwrap();
    fs::write(dir.join("outside.txt"), "outside the suite").unwrap();
    // A link inside a seeded directory, to a file outside the suite.
    std::os::unix::fs::symlink(dir.join("outside.txt"), dir.join("suite/stubs/leak.txt")).unwrap();
    fs::write(
        dir.join("suite/suite.yaml"),
        "format: loom.tree-suite/1\ncycles:\n  - {name: leaks, protocol: tour@1, bindings: b.yaml, seed: [{from: stubs, to: stubs}]}\n",
    )
    .unwrap();
    let refused = run_suite(
        &dir.join("suite/suite.yaml"),
        &local_settings(dir.join("scratch")),
        None,
        None,
    )
    .unwrap_err();
    assert_eq!(
        format!("{:?}", refused.reason()),
        "SeedUnusable",
        "{refused}"
    );
    assert_eq!(
        refused.member(),
        Some("cycles[0].seed[0].from"),
        "{refused}"
    );
    assert!(!dir.join("scratch").exists());
    let _ = fs::remove_dir_all(&dir);
    within_bound(started);
}

/// `tree test --help` names the one file written outside `--scratch`: the `--report` file.
#[test]
fn the_help_names_the_report_as_the_one_write_outside_scratch() {
    let started = Instant::now();
    let help = command(&fresh_scratch("help"))
        .args(["tree", "test", "--help"])
        .output()
        .unwrap();
    let said = text(&help.stdout);
    assert_eq!(help.status.code(), Some(0), "{said}");
    assert!(
        said.contains("nothing but the --report file is written outside --scratch"),
        "{said}"
    );
    within_bound(started);
}

const PARALLEL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/tree-suite/parallel"
);

/// The text of the difference a cancelled cycle ends with.
const CANCELLED: &str = "the suite was cancelled";

/// Copies the directory `from` to `to`, file by file.
fn copy_dir(from: &Path, to: &Path) {
    for file in files(from) {
        let target = to.join(file.strip_prefix(from).unwrap());
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(&file, &target).unwrap();
    }
}

/// A workers test's directories: `dir`, which holds `scratch/` (the `--scratch`) and `barrier/`
/// (the markers) alone, and `suite`, a copy of `fixtures/tree-suite/parallel` whose seeded
/// `barrier/dir.txt` names `dir/barrier`. Both are removed when it is dropped.
struct Parallel {
    dir: PathBuf,
    suite: PathBuf,
}

impl Parallel {
    fn new(label: &str) -> Self {
        let dir = fresh_scratch(label);
        let suite = fresh_scratch(&format!("{label}-suite"));
        copy_dir(Path::new(PARALLEL), &suite);
        fs::create_dir_all(dir.join("barrier")).unwrap();
        fs::write(
            suite.join("barrier/dir.txt"),
            dir.join("barrier").as_os_str().as_encoded_bytes(),
        )
        .unwrap();
        Self { dir, suite }
    }

    fn scratch(&self) -> PathBuf {
        self.dir.join("scratch")
    }

    fn barrier(&self) -> PathBuf {
        self.dir.join("barrier")
    }

    /// [`local_settings`] over the copy's protocols with `workers`.
    fn settings(&self, workers: usize) -> SuiteSettings {
        let mut settings = local_settings(self.scratch());
        settings.protocols = self.suite.join("protocols");
        settings.workers = NonZeroUsize::new(workers).unwrap();
        settings
    }

    /// The suite file `name` of the copy, run with `workers`.
    fn run(&self, name: &str, workers: usize) -> SuiteReport {
        run_suite(&self.suite.join(name), &self.settings(workers), None, None).unwrap()
    }

    /// The file `name` of cycle `n`'s workspace.
    fn work(&self, n: usize, name: &str) -> String {
        let path = self.scratch().join("work").join(n.to_string()).join(name);
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }
}

impl Drop for Parallel {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
        let _ = fs::remove_dir_all(&self.suite);
    }
}

/// The names of the entries of `dir`, sorted; each must be a directory.
fn directories(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_dir(), "{entry:?}");
            entry.file_name().to_string_lossy().into_owned()
        })
        .collect();
    names.sort();
    names
}

fn cycle_names(report: &SuiteReport) -> Vec<&str> {
    report
        .cycles
        .iter()
        .map(|cycle| cycle.name.as_str())
        .collect()
}

/// The `--report` document of `report` without any cycle's `seconds`.
fn without_seconds(report: &SuiteReport) -> Value {
    let mut document = report_json(report);
    for cycle in document["cycles"].as_array_mut().unwrap() {
        cycle.as_object_mut().unwrap().remove("seconds");
    }
    document
}

/// Test 1: three barrier cycles with three workers each see the others' markers.
#[test]
fn workers_run_cycles_at_the_same_time() {
    let started = Instant::now();
    let run = Parallel::new("workers");
    let report = run.run("barrier.suite.yaml", 3);
    assert_eq!(cycle_names(&report), ["one", "two", "three"], "{report:?}");
    for n in 1..=3 {
        assert_eq!(
            run.work(n, "barrier.txt").trim(),
            "met",
            "cycle {n}: {report:?}"
        );
    }
    assert!(report.failed.is_empty(), "{report:?}");
    within_bound(started);
}

/// Test 2: with one worker, cycle 1 waits out its barrier alone. It waits 2 s by design, so it
/// asserts the 60 s bound of one test and prints its time.
#[test]
fn one_worker_runs_cycles_one_at_a_time() {
    let started = Instant::now();
    let run = Parallel::new("one-worker");
    let report = run.run("barrier.suite.yaml", 1);
    assert_eq!(run.work(1, "barrier.txt").trim(), "timed out", "{report:?}");
    assert_eq!(cycle_names(&report), ["one", "two", "three"], "{report:?}");
    assert!(report.failed.is_empty(), "{report:?}");
    let spent = started.elapsed();
    eprintln!("one_worker_runs_cycles_one_at_a_time took {spent:?}");
    assert!(spent < RUN_BOUND, "the test took {spent:?}");
}

/// Test 3: `slow` ends after `fast`, and the observer still gets `slow` first; the report is the
/// one of a single worker but for `seconds`. The single worker's run, in directories of its own,
/// runs beside the two workers' run.
#[test]
fn results_reach_the_observer_in_suite_order() {
    let started = Instant::now();
    let run = Parallel::new("observer");
    // One worker: `fast`'s marker is there before `slow` starts, so `slow` does not wait for it.
    let serial = Parallel::new("observer-serial");
    fs::write(serial.barrier().join("fast"), "").unwrap();
    let mut seen: Vec<String> = Vec::new();
    let mut observe = |result: &CycleResult| seen.push(result.name.clone());
    let (report, one) = std::thread::scope(|scope| {
        let one = scope.spawn(|| serial.run("order.suite.yaml", 1));
        let report = run_suite(
            &run.suite.join("order.suite.yaml"),
            &run.settings(2),
            Some(&mut observe),
            None,
        )
        .unwrap();
        (report, one.join().unwrap())
    });
    assert_eq!(seen, ["slow", "fast"], "{report:?}");
    // The two ran at once: `slow`'s program saw the marker `fast`'s program wrote as its last act.
    assert_eq!(run.work(1, "waited.txt").trim(), "met", "{report:?}");
    assert_eq!(without_seconds(&report), without_seconds(&one));
    within_bound(started);
}

/// Test 4: cycle 1 fails at its first step; cycles 3 and 4 start after that and still run. One run
/// of `tree test --workers 2`, read from its output, its exit status and its `--report`.
#[test]
fn a_failing_cycle_does_not_stop_the_others_at_once() {
    let started = Instant::now();
    let dir = fresh_scratch("failing");
    let tested = tree_test_in(
        &dir,
        "scratch",
        &Path::new(PARALLEL).join("failing.suite.yaml"),
        &Path::new(PARALLEL).join("protocols"),
        &["--workers", "2"],
    );
    let said = format!("{}{}", text(&tested.stdout), text(&tested.stderr));
    assert_eq!(tested.status.code(), Some(1), "{said}");
    let report = read_report(&dir.join("scratch.json"));
    let names: Vec<&str> = report["cycles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|cycle| cycle["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["broken", "second", "third", "fourth"], "{report}");
    assert_eq!(report["failed"], json!(["broken"]), "{report}");
    assert_eq!(
        report["cycles"][0]["difference"]["reason"],
        format!("{:?}", DifferenceReason::ExitDiffers),
        "{report}"
    );
    for cycle in &report["cycles"].as_array().unwrap()[1..] {
        assert_eq!(cycle["as_expected"], true, "{cycle}");
        assert_eq!(cycle["exit"], 0, "{cycle}");
    }
    let _ = fs::remove_dir_all(&dir);
    within_bound(started);
}

/// Test 5: three cycles started together end in reverse suite order, and each leaves its own
/// name in its own `work/<n>/`.
#[test]
fn each_cycle_keeps_to_its_own_directory_at_once() {
    let started = Instant::now();
    let run = Parallel::new("own");
    let report = run.run("own.suite.yaml", 3);
    assert!(report.failed.is_empty(), "{report:?}");
    // The cycles did not end in suite order: a later cycle's marker was there when `late` ended.
    // (The sleeps make the order 3, 2, 1 when all three start together; on a loaded host `middle`
    // can start late enough to end after `late`.)
    let seen = run.work(1, "seen.txt");
    assert!(
        seen.lines().any(|line| line == "middle" || line == "early"),
        "{seen}"
    );
    for (n, name) in [(1, "late"), (2, "middle"), (3, "early")] {
        assert_eq!(run.work(n, "own.txt").trim(), name, "cycle {n}");
    }
    assert_eq!(directories(&run.scratch()), ["store", "work"]);
    for shared in ["store", "work"] {
        assert_eq!(
            directories(&run.scratch().join(shared)),
            ["1", "2", "3"],
            "{shared}"
        );
    }
    let mut left: Vec<String> = fs::read_dir(&run.dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["barrier", "scratch"]);
    within_bound(started);
}

/// Test 6: the flag is set while both workers hold a cycle at its gate; no third cycle starts.
#[test]
fn cancel_starts_no_further_cycle() {
    let started = Instant::now();
    let run = Parallel::new("cancel-workers");
    let settings = run.settings(2);
    let cancel = AtomicBool::new(false);
    let barrier = run.barrier();
    let markers = || fs::read_dir(&barrier).unwrap().count();
    let (held, report) = std::thread::scope(|scope| {
        let running = scope.spawn(|| {
            run_suite(
                &run.suite.join("cancel.suite.yaml"),
                &settings,
                None,
                Some(&cancel),
            )
        });
        let waited = Instant::now();
        while markers() < 2 && waited.elapsed() < Duration::from_secs(4) {
            std::thread::sleep(Duration::from_millis(20));
        }
        let held = markers();
        cancel.store(true, Ordering::SeqCst);
        fs::write(barrier.join("release"), "").unwrap();
        (held, running.join().unwrap().unwrap())
    });
    assert_eq!(held, 2, "{report:?}");
    assert_eq!(cycle_names(&report), ["c1", "c2"], "{report:?}");
    for cycle in &report.cycles {
        let difference = cycle.difference.as_ref().expect("a difference");
        assert_eq!(difference.found.as_deref(), Some(CANCELLED), "{cycle:?}");
        assert_eq!(difference.reason, DifferenceReason::StepFailed, "{cycle:?}");
    }
    assert!(!run.scratch().join("work/3").exists());
    assert!(!run.scratch().join("store/3").exists());
    within_bound(started);
}

/// `b10x-loom tree test --confinement local` of `suite` over `protocols` in `dir` (home
/// `dir/home`), scratch `dir/<label>` and report `dir/<label>.json`, with `extra` added.
fn tree_test_in(dir: &Path, label: &str, suite: &Path, protocols: &Path, extra: &[&str]) -> Output {
    fs::create_dir_all(dir.join("home")).unwrap();
    command(&dir.join("home"))
        .args(["tree", "test", "--confinement", "local", "--at", AT])
        .arg("--suite")
        .arg(suite)
        .arg("--protocols")
        .arg(protocols)
        .arg("--scratch")
        .arg(dir.join(label))
        .arg("--report")
        .arg(dir.join(format!("{label}.json")))
        .args(extra)
        .output()
        .unwrap()
}

/// `tree test` of `only.suite.yaml` in `dir`, with `extra` added.
fn only_test(dir: &Path, label: &str, extra: &[&str]) -> Output {
    tree_test_in(
        dir,
        label,
        &Path::new(FIXTURE).join("only.suite.yaml"),
        Path::new(PROTOCOLS),
        extra,
    )
}

fn read_report(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// Test 7: `--only b,d` runs cycles 2 and 4 alone and says it selected them.
#[test]
fn only_runs_the_named_cycles() {
    let started = Instant::now();
    let dir = fresh_scratch("only");
    let tested = only_test(&dir, "scratch", &["--only", "b,d"]);
    let stdout = text(&tested.stdout);
    assert_eq!(
        tested.status.code(),
        Some(0),
        "{stdout}{}",
        text(&tested.stderr)
    );
    let mut made: Vec<String> = fs::read_dir(dir.join("scratch/work"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    made.sort();
    assert_eq!(made, ["2", "4"]);
    let lines: Vec<&str> = stdout.lines().collect();
    // Two cycle lines, the two summary lines and the `slowest:` line.
    assert_eq!(lines.len(), 5, "{stdout}");
    assert!(lines[2].starts_with("cycles: selected 2 of 5,"), "{stdout}");
    let report = read_report(&dir.join("scratch.json"));
    assert_eq!(report["only"], json!(["b", "d"]), "{report}");
    assert_eq!(report["suite_cycles"], 5, "{report}");
    let _ = fs::remove_dir_all(&dir);
    within_bound(started);
}

/// Test 8: `--only b --only d` is the selection `--only b,d` is.
#[test]
fn only_repeated_equals_only_with_a_comma() {
    let started = Instant::now();
    let dir = fresh_scratch("only-repeated");
    let comma = only_test(&dir, "comma", &["--only", "b,d"]);
    let repeated = only_test(&dir, "repeated", &["--only", "b", "--only", "d"]);
    assert_eq!(comma.status.code(), Some(0), "{}", text(&comma.stderr));
    assert_eq!(
        repeated.status.code(),
        Some(0),
        "{}",
        text(&repeated.stderr)
    );
    // The output, each cycle's wall time apart: the `slowest:` line, whose order is that of the
    // wall times, as the names it holds in byte order.
    let lines = |output: &Output| -> Vec<String> {
        text(&output.stdout)
            .lines()
            .map(|line| {
                if let Some(slowest) = line.strip_prefix("slowest: ") {
                    let mut named: Vec<&str> = slowest
                        .split(", ")
                        .map(|part| part.split(" (").next().unwrap_or(part))
                        .collect();
                    named.sort_unstable();
                    return format!("slowest: {}", named.join(", "));
                }
                match line.rfind(" (") {
                    Some(at) if line.ends_with(" s)") => line[..at].to_owned(),
                    _ => line.to_owned(),
                }
            })
            .collect()
    };
    assert_eq!(lines(&comma), lines(&repeated));
    let report = |label: &str| {
        let mut document = read_report(&dir.join(format!("{label}.json")));
        for cycle in document["cycles"].as_array_mut().unwrap() {
            cycle.as_object_mut().unwrap().remove("seconds");
        }
        document
    };
    assert_eq!(report("comma"), report("repeated"));
    let _ = fs::remove_dir_all(&dir);
    within_bound(started);
}

/// Test 9: a full run's report names no selection and counts the suite's cycles.
#[test]
fn a_full_run_reports_no_only() {
    let ran = ran();
    let started = Instant::now();
    let report = ran.report();
    assert!(report.get("only").is_none(), "{report}");
    assert_eq!(report["suite_cycles"], 5, "{report}");
    within_bound(started);
}

/// Test 10: an `--only` name that is no cycle of the suite is refused before any cycle.
#[test]
fn only_naming_no_cycle_is_refused() {
    let started = Instant::now();
    let dir = fresh_scratch("only-unknown");
    let tested = only_test(&dir, "scratch", &["--only", "nope"]);
    let stderr = text(&tested.stderr);
    assert_eq!(tested.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("CycleUnknown"), "{stderr}");
    assert!(stderr.contains("nope"), "{stderr}");
    assert!(tested.stdout.is_empty(), "{}", text(&tested.stdout));
    assert!(!dir.join("scratch").exists());
    let _ = fs::remove_dir_all(&dir);
    within_bound(started);
}

/// Test 11: `--workers` outside 1..=64 is clap's usage error, before anything is read: the suite
/// and protocols named do not exist.
#[test]
fn workers_out_of_range_is_a_usage_error() {
    let started = Instant::now();
    let dir = fresh_scratch("workers-range");
    for workers in ["0", "65"] {
        let tested = tree_test_in(
            &dir,
            "scratch",
            &dir.join("no-suite.yaml"),
            &dir.join("no-protocols"),
            &["--workers", workers],
        );
        let stderr = text(&tested.stderr);
        assert_eq!(tested.status.code(), Some(2), "{stderr}");
        assert!(
            stderr.contains(&format!("invalid value '{workers}' for '--workers")),
            "{stderr}"
        );
        assert!(!stderr.contains("refused"), "{stderr}");
        assert!(!dir.join("scratch").exists());
    }
    let _ = fs::remove_dir_all(&dir);
    within_bound(started);
}

/// `story:suite-runs-cycles-under-one-cgroup-root`, test 7 (replacing the parallel story's test
/// 12): more than 16 workers under `substrate` are refused before any cycle, by the command before
/// it places a program (its `PATH` holds no `systemd-run`, so a start again would be refused
/// otherwise) and by `run_suite`; 16 are served.
#[test]
fn workers_above_sixteen_under_substrate_are_refused() {
    let dir = fresh_scratch("substrate-workers");
    fs::create_dir_all(dir.join("home")).unwrap();
    fs::create_dir_all(dir.join("no-programs")).unwrap();
    let seventeen = command(&dir.join("home"))
        .env("PATH", dir.join("no-programs"))
        .args(["tree", "test", "--confinement", "substrate"])
        .args(["--workers", "17", "--at", AT])
        .arg("--suite")
        .arg(Path::new(FIXTURE).join("one.suite.yaml"))
        .arg("--protocols")
        .arg(PROTOCOLS)
        .arg("--scratch")
        .arg(dir.join("scratch"))
        .output()
        .unwrap();
    let said = text(&seventeen.stderr);
    assert_eq!(seventeen.status.code(), Some(2), "{said}");
    assert!(
        said.starts_with("b10x-loom: tree test refused: SubstrateWorkersUnserved: "),
        "{said}"
    );
    assert!(said.contains(SUBSTRATE_WORKERS_DETAIL), "{said}");
    assert!(seventeen.stdout.is_empty());
    assert!(!dir.join("scratch").exists());

    let mut settings = local_settings(dir.join("scratch"));
    settings.confinement = Some("substrate".to_owned());
    settings.workers = NonZeroUsize::new(17).unwrap();
    let refused = run_suite(
        &Path::new(FIXTURE).join("one.suite.yaml"),
        &settings,
        None,
        None,
    )
    .unwrap_err();
    assert_eq!(
        format!("{:?}", refused.reason()),
        "SubstrateWorkersUnserved",
        "{refused}"
    );
    assert!(
        refused.to_string().contains(SUBSTRATE_WORKERS_DETAIL),
        "{refused}"
    );
    assert!(!dir.join("scratch").exists());

    for workers in [1, 2, 16] {
        assert!(
            workers_served(Some("substrate"), NonZeroUsize::new(workers).unwrap()).is_ok(),
            "{workers} workers under substrate"
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

/// The detail of `SubstrateWorkersUnserved` the story names.
const SUBSTRATE_WORKERS_DETAIL: &str = "a Substrate driver runs at most 16 executions at once; \
--workers above 16 under --confinement substrate is refused";

/// `tree test --help` shows the two flags, the selection line and the two refusals.
#[test]
fn the_help_names_the_workers_the_selection_and_the_refusals() {
    let started = Instant::now();
    let help = command(&fresh_scratch("help-workers"))
        .args(["tree", "test", "--help"])
        .output()
        .unwrap();
    let said = text(&help.stdout);
    assert_eq!(help.status.code(), Some(0), "{said}");
    for part in [
        "--workers <N>",
        "--only <NAME>",
        "cycles: selected <S> of <C>,",
        "CycleUnknown",
        "SubstrateWorkersUnserved",
    ] {
        assert!(said.contains(part), "{part}: {said}");
    }
    within_bound(started);
}

/// Acceptance for `story:tree-suite-stop-resume-reopen-steps`: `run_suite` over a suite of the one
/// cycle `suite`, in process, its report; the scratch directory removed.
fn ran_alone(suite: &str, label: &str) -> SuiteReport {
    let scratch = fresh_scratch(label);
    let report = run_suite(
        &Path::new(FIXTURE).join(suite),
        &local_settings(scratch.clone()),
        None,
        None,
    )
    .unwrap();
    let _ = fs::remove_dir_all(&scratch);
    report
}

/// `story:tree-suite-stop-resume-reopen-steps`, test 8: Runbook's `run-stop` cycle, `stop`,
/// `resume` refused `TreeStopped`, `reopen` and a run that goes on, passes, with one
/// `StopRecorded` and one `StopLifted` line in the root's log.
#[test]
fn the_stop_steps_run_as_runbooks_cycle() {
    let started = Instant::now();
    let suite =
        b10x_loom_tree::suite::read_suite(&Path::new(FIXTURE).join("stop.suite.yaml")).unwrap();
    let steps: Vec<String> = suite.cycles[0]
        .steps
        .iter()
        .map(|step| format!("{:?}", step.r#do))
        .collect();
    assert_eq!(steps, ["Open", "Run", "Stop", "Resume", "Reopen", "Run"]);
    let report = ran_alone("stop.suite.yaml", "stop-steps");
    let cycle = &report.cycles[0];
    assert_eq!(cycle.waits_for, None, "{report:?}");
    assert!(cycle.run && cycle.as_expected, "{report:?}");
    assert_eq!((report.expected, report.as_expected), (1, 1), "{report:?}");
    within_bound(started);
}

/// `story:tree-suite-stop-resume-reopen-steps`, test 9: `resume` on a tree no hold was recorded
/// for is an ordinary `tree run`.
#[test]
fn a_resume_of_a_running_tree_runs() {
    let started = Instant::now();
    let report = ran_alone("resume.suite.yaml", "resume-runs");
    let cycle = &report.cycles[0];
    assert_eq!(cycle.waits_for, None, "{report:?}");
    assert!(cycle.run && cycle.as_expected, "{report:?}");
    within_bound(started);
}

// Acceptance for `story:tree-suite-approve-result-and-reasons`: the `approve` step and the
// `approve` entry pass `result`, `reasons` (joined by line feeds) and `deny` to `tree approve`.
// `review.suite.yaml` has cycles over `review@1`, `approve.suite.yaml` over `tour@1`; each test runs
// `tree test --only` of its own cycles, not the shared run.

/// A protocols directory in `dir`: a copy of `tree-two-level/protocols` with
/// `tree-two-level/extra/review.yaml` added. The fixture is never changed in place.
fn review_protocols(dir: &Path) -> PathBuf {
    let protocols = dir.join("protocols");
    copy_dir(Path::new(PROTOCOLS), &protocols);
    fs::copy(
        Path::new(PROTOCOLS).join("../extra/review.yaml"),
        protocols.join("review.yaml"),
    )
    .unwrap();
    protocols
}

/// `tree test --only <only> --workers 2` of the fixture suite `suite` in a fresh directory, over
/// the review protocols for `review.suite.yaml` and the fixture's own otherwise: the directory,
/// the output and the `--report`. Two selected cycles run at once, so the test keeps its 5 s.
fn suite_cycles(label: &str, suite: &str, only: &str) -> (PathBuf, String, Value) {
    let dir = fresh_scratch(label);
    fs::create_dir_all(&dir).unwrap();
    let protocols = if suite == "review.suite.yaml" {
        review_protocols(&dir)
    } else {
        PathBuf::from(PROTOCOLS)
    };
    let tested = tree_test_in(
        &dir,
        "scratch",
        &Path::new(FIXTURE).join(suite),
        &protocols,
        &["--only", only, "--workers", "2"],
    );
    let said = format!("{}{}", text(&tested.stdout), text(&tested.stderr));
    let report_file = dir.join("scratch.json");
    assert!(report_file.is_file(), "{said}");
    (dir, said, read_report(&report_file))
}

/// The report's result of the cycle `name`.
fn cycle_of<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["cycles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cycle| cycle["name"] == name)
        .unwrap_or_else(|| panic!("no cycle {name}: {report}"))
}

/// Asserts the cycle `name` ran and ended as the suite expects.
fn as_expected(report: &Value, name: &str, said: &str) {
    let cycle = cycle_of(report, name);
    assert_eq!(cycle["pending"], false, "{said}{cycle}");
    assert_eq!(cycle["run"], true, "{said}{cycle}");
    assert_eq!(cycle["as_expected"], true, "{said}{cycle}");
}

/// Test 1: a step's named result and reasons: `[rejected, approved]` in order, and the first
/// review's person observation names the blob of the reasons, one per line. (A person's `action
/// finished` line names no blob: `PersonEffects` puts the digest in its observation alone.)
#[test]
fn the_approve_step_records_a_named_result() {
    let started = Instant::now();
    let (dir, said, report) = suite_cycles("step-result", "review.suite.yaml", "step-result");
    as_expected(&report, "step-result", &said);
    assert_eq!(cycle_of(&report, "step-result")["exit"], 0, "{report}");
    // The store keeps the reasons as written, joined by one line feed, as a blob.
    let blobs = dir.join("scratch/store/1/blobs");
    let kept = fs::read_dir(&blobs)
        .unwrap()
        .any(|blob| fs::read(blob.unwrap().path()).unwrap() == b"one\ntwo");
    assert!(kept, "no blob holds the reasons: {}", blobs.display());
    let _ = fs::remove_dir_all(&dir);
    within_bound(started);
}

/// Test 2: a scripted entry's named result, the shape of Runbook's `decider-rework`.
#[test]
fn a_scripted_entry_records_a_named_result() {
    let started = Instant::now();
    let (dir, said, report) = suite_cycles("entry-result", "review.suite.yaml", "entry-result");
    as_expected(&report, "entry-result", &said);
    let _ = fs::remove_dir_all(&dir);
    within_bound(started);
}

/// Test 3: an entry's result answering a wait on a capability alone is refused
/// `NotAPersonAction`, which ends the cycle with `StepFailed`; so is one whose person action's
/// entry lists no such result (`guide.approve`'s `route.approve` lists `performed` alone).
#[test]
fn a_result_on_a_capability_wait_is_a_difference() {
    let started = Instant::now();
    let (dir, said, report) = suite_cycles(
        "result-capability",
        "approve.suite.yaml",
        "result-on-a-capability-wait,result-on-guide-approve",
    );
    for (name, entry) in [
        ("result-on-a-capability-wait", "decide[1]"),
        ("result-on-guide-approve", "decide[0]"),
    ] {
        let cycle = cycle_of(&report, name);
        assert_eq!(cycle["pending"], false, "{said}{cycle}");
        assert_eq!(cycle["as_expected"], false, "{said}{cycle}");
        let difference = &cycle["difference"];
        assert_eq!(difference["check"], entry, "{cycle}");
        assert_eq!(
            difference["reason"],
            format!("{:?}", DifferenceReason::StepFailed),
            "{cycle}"
        );
        assert_eq!(difference["found"], "NotAPersonAction", "{cycle}");
    }
    let _ = fs::remove_dir_all(&dir);
    within_bound(started);
}

/// Test 4: `deny: true` records `authority recorded` with `Deny`, and `tree status` lists the
/// approval with `answer: Deny`; no run follows the denial.
#[test]
fn a_denial_is_recorded_and_shown() {
    let started = Instant::now();
    let (dir, said, report) = suite_cycles("denial", "approve.suite.yaml", "denial");
    as_expected(&report, "denial", &said);
    let _ = fs::remove_dir_all(&dir);
    within_bound(started);
}

/// Test 5: 60 one-line reasons are taken, 61 are refused `NotAPersonAction` by the 60-line bound.
#[test]
fn the_reasons_limit_counts_lines() {
    let started = Instant::now();
    let (dir, said, report) = suite_cycles(
        "reasons-limit",
        "review.suite.yaml",
        "reasons-at-the-limit,reasons-over-the-limit",
    );
    as_expected(&report, "reasons-at-the-limit", &said);
    as_expected(&report, "reasons-over-the-limit", &said);
    let _ = fs::remove_dir_all(&dir);
    within_bound(started);
}

// Acceptance for `story:suite-runs-cycles-under-one-cgroup-root`: every cycle works in
// `<scratch>/work/<n>/` and keeps its tree in `<scratch>/store/<n>/`, so all workspaces have one
// parent and every confined launch of every cycle shares one Substrate driver under one held,
// delegated cgroup root. The suites and POSIX sh stubs are `fixtures/tree-suite/substrate/`; a
// Substrate-confined program writes only `target/` of its workspace, so the stubs write there.
//
// The delegated tests need bubblewrap and a user systemd manager that starts a delegated scope;
// they print `skipped: missing prerequisite: …` without them unless `LOOM_REQUIRE_DELEGATION` is
// set. Each starts `tree test` through `enter.sh <scope>/run`, so the command runs in a child of
// the scope `Scope::start` holds, and gives it the scope as `--cgroup-root`. The gated stubs write
// `target/started`, then poll their own `target/release` every 0.1 s for at most 10 s; the test
// polls every cycle's `started` from the host and writes each `release` once all exist, so all
// the `started` files existing at once, before any `release`, is the overlap.

const SUBSTRATE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/tree-suite/substrate"
);

/// The `b10x-loom` these tests start.
const BINARY: &str = env!("CARGO_BIN_EXE_b10x-loom");

/// Set in the test binary [`run_suite_takes_the_runner_that_holds_the_root`] starts again in a
/// child of its scope.
const IN_SCOPE: &str = "LOOM_SUITE_IN_SCOPE";

/// The scope's directory, for the test binary started again.
const SUITE_ROOT: &str = "LOOM_SUITE_ROOT";

/// The prefix every refusal of `tree test` starts with.
const TEST_REFUSED: &str = "b10x-loom: tree test refused: ";

/// `tree test --at AT` with `HOME` at `dir/home`, started in the cgroup `cgroup` (made when absent)
/// through `enter.sh` when given.
fn tree_test_command(dir: &Path, cgroup: Option<&Path>) -> Command {
    let home = dir.join("home");
    fs::create_dir_all(&home).unwrap();
    let mut command = match cgroup {
        Some(cgroup) => {
            let mut command = Command::new("sh");
            command
                .arg(Path::new(DELEGATED).join("enter.sh"))
                .arg(cgroup)
                .arg(BINARY);
            command
        }
        None => Command::new(BINARY),
    };
    switched(&mut command, &home);
    command.args(["tree", "test", "--at", AT]);
    command
}

/// [`tree_test_command`] of the substrate suite file `suite` of `suite_dir`, its protocols, and
/// `--scratch dir/scratch`.
fn substrate_suite(dir: &Path, cgroup: Option<&Path>, suite_dir: &Path, suite: &str) -> Command {
    let mut command = tree_test_command(dir, cgroup);
    command
        .arg("--suite")
        .arg(suite_dir.join(suite))
        .arg("--protocols")
        .arg(suite_dir.join("protocols"))
        .arg("--scratch")
        .arg(dir.join("scratch"));
    command
}

/// `target/<name>` of cycle `n`'s workspace under `dir/scratch`.
fn cycle_target(dir: &Path, n: usize, name: &str) -> PathBuf {
    dir.join("scratch/work")
        .join(n.to_string())
        .join("target")
        .join(name)
}

/// A `tree test` the test started, killed when the test ends before it did.
struct Spawned(Option<std::process::Child>);

impl Spawned {
    fn spawn(mut command: Command) -> Self {
        Self(Some(
            command
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        ))
    }

    /// Waits, polling every 50 ms, until every one of `paths` exists; fails when the command ends
    /// first or after 30 s.
    fn wait_for(&mut self, paths: &[PathBuf]) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !paths.iter().all(|path| path.exists()) {
            let child = self.0.as_mut().unwrap();
            if child.try_wait().unwrap().is_some() {
                let ended = self.finish();
                panic!(
                    "tree test ended before {paths:?} existed: {:?}\n{}{}",
                    ended.status,
                    text(&ended.stdout),
                    text(&ended.stderr)
                );
            }
            assert!(
                Instant::now() < deadline,
                "{paths:?} do not all exist after 30 s"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn finish(&mut self) -> Output {
        self.0.take().unwrap().wait_with_output().unwrap()
    }
}

impl Drop for Spawned {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Asserts `output` ended 0, every cycle as expected.
fn passed(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

/// The processes in every cgroup below `root` but `keeper/`, each `(cgroup, pid)`.
fn processes_below(root: &Path) -> Vec<(PathBuf, String)> {
    fn walk(dir: &Path, root: &Path, found: &mut Vec<(PathBuf, String)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() || path == root.join("keeper") {
                continue;
            }
            for pid in fs::read_to_string(path.join("cgroup.procs"))
                .unwrap_or_default()
                .lines()
            {
                found.push((path.clone(), pid.to_owned()));
            }
            walk(&path, root, found);
        }
    }
    let mut found = Vec::new();
    walk(root, root, &mut found);
    found
}

/// Test 1: a two-cycle run leaves `work/1`, `work/2`, `store/1` and `store/2` and nothing else
/// under `--scratch`, and each cycle's program copied its own seeded name into its own workspace.
#[test]
fn cycles_work_in_one_shared_directory() {
    let scratch = fresh_scratch("shared-directory");
    let mut settings = local_settings(scratch.clone());
    settings.protocols = Path::new(SUBSTRATE).join("protocols");
    settings.workers = NonZeroUsize::new(2).unwrap();
    let report = run_suite(
        &Path::new(SUBSTRATE).join("own.suite.yaml"),
        &settings,
        None,
        None,
    )
    .unwrap();
    assert!(report.failed.is_empty(), "{report:?}");
    assert_eq!(report.as_expected, 2, "{report:?}");
    assert_eq!(directories(&scratch), ["store", "work"]);
    assert_eq!(directories(&scratch.join("work")), ["1", "2"]);
    assert_eq!(directories(&scratch.join("store")), ["1", "2"]);
    for (n, name) in [(1, "one"), (2, "two")] {
        let own = scratch.join("work").join(n.to_string()).join("own.txt");
        assert_eq!(
            fs::read_to_string(&own).unwrap().trim(),
            name,
            "{}",
            own.display()
        );
        assert!(scratch.join(format!("store/{n}/tree.json")).is_file());
    }
    let _ = fs::remove_dir_all(&scratch);
}

/// Test 2: `--cgroup-root <scope> --workers 2` runs two gated cycles at once under the scope;
/// each copies its own name, the run ends 0, and no process is left below the root but the
/// scope's keeper.
#[test]
fn substrate_runs_cycles_side_by_side_under_a_given_root() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let dir = fresh_scratch("substrate-side-by-side");
    let mut command = substrate_suite(
        &dir,
        Some(&scope.root.join("run")),
        Path::new(SUBSTRATE),
        "gated.suite.yaml",
    );
    command
        .arg("--cgroup-root")
        .arg(&scope.root)
        .args(["--workers", "2"]);
    let mut running = Spawned::spawn(command);
    running.wait_for(&[
        cycle_target(&dir, 1, "started"),
        cycle_target(&dir, 2, "started"),
    ]);
    for n in 1..=2 {
        assert!(!cycle_target(&dir, n, "release").exists());
    }
    for n in 1..=2 {
        fs::write(cycle_target(&dir, n, "release"), "").unwrap();
    }
    let output = running.finish();
    passed(&output);
    for (n, name) in [(1, "one"), (2, "two")] {
        assert_eq!(
            fs::read_to_string(cycle_target(&dir, n, "own.txt"))
                .unwrap()
                .trim(),
            name,
            "cycle {n}"
        );
    }
    // Substrate ends a launch's cgroup after its program; wait up to 10 s for it to empty.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut left = processes_below(&scope.root);
    while !left.is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        left = processes_below(&scope.root);
    }
    assert!(left.is_empty(), "processes left below the root: {left:?}");
    let _ = fs::remove_dir_all(&dir);
}

/// A copy of the substrate fixture in a fresh directory, whose `peers/one/host.txt` and
/// `peers/two/host.txt` name the host path of the other cycle's `target/marker` under
/// `dir/scratch`.
fn looking_suite(label: &str, dir: &Path) -> PathBuf {
    let suite = fresh_scratch(label);
    copy_dir(Path::new(SUBSTRATE), &suite);
    for (peer, other) in [("one", 2), ("two", 1)] {
        fs::write(
            suite.join("peers").join(peer).join("host.txt"),
            cycle_target(dir, other, "marker")
                .as_os_str()
                .as_encoded_bytes(),
        )
        .unwrap();
    }
    suite
}

/// Test 3: a confined program of one cycle reads the other cycle's marker neither by its host
/// path nor as `../<n>/target/marker`, while both markers exist on the host.
#[test]
fn a_cycle_sees_no_workspace_of_another() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let dir = fresh_scratch("substrate-no-workspace");
    let suite = looking_suite("substrate-no-workspace-suite", &dir);
    let mut command = substrate_suite(
        &dir,
        Some(&scope.root.join("run")),
        &suite,
        "look.suite.yaml",
    );
    command
        .arg("--cgroup-root")
        .arg(&scope.root)
        .args(["--workers", "2"]);
    let mut running = Spawned::spawn(command);
    running.wait_for(&[
        cycle_target(&dir, 1, "started"),
        cycle_target(&dir, 2, "started"),
    ]);
    // The positive control: each marker exists on the host before the other cycle reads it.
    for n in 1..=2 {
        assert!(cycle_target(&dir, n, "marker").is_file(), "cycle {n}");
    }
    for n in 1..=2 {
        fs::write(cycle_target(&dir, n, "release"), "").unwrap();
    }
    let output = running.finish();
    passed(&output);
    for n in 1..=2 {
        assert!(
            cycle_target(&dir, n, "unseen").is_file(),
            "cycle {n} read the other cycle's marker by: {}",
            fs::read_to_string(cycle_target(&dir, n, "seen")).unwrap_or_default()
        );
    }
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&suite);
}

/// Test 4: cycle 2's confined program sees its own process in `/proc` and not cycle 1's, which
/// runs at the same time.
#[test]
fn a_cycle_sees_no_process_of_another() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let dir = fresh_scratch("substrate-no-process");
    let mut command = substrate_suite(
        &dir,
        Some(&scope.root.join("run")),
        Path::new(SUBSTRATE),
        "procs.suite.yaml",
    );
    command
        .arg("--cgroup-root")
        .arg(&scope.root)
        .args(["--workers", "2"]);
    let mut running = Spawned::spawn(command);
    running.wait_for(&[
        cycle_target(&dir, 1, "started"),
        cycle_target(&dir, 2, "started"),
    ]);
    for n in 1..=2 {
        fs::write(cycle_target(&dir, n, "release"), "").unwrap();
    }
    running.wait_for(&[cycle_target(&dir, 2, "procs.txt")]);
    fs::write(cycle_target(&dir, 1, "done"), "").unwrap();
    let output = running.finish();
    passed(&output);
    assert_eq!(
        fs::read_to_string(cycle_target(&dir, 2, "procs.txt")).unwrap(),
        "self: seen\nother: unseen\n"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// Test 5: with no `--cgroup-root`, `tree test` under `substrate` starts itself again in a
/// delegated scope and exits with that run's status: 0 for a passing suite, 1 for a failing one.
#[test]
fn substrate_starts_itself_again() {
    if !delegation_present() {
        return;
    }
    let dir = fresh_scratch("substrate-again");
    let passing = substrate_suite(&dir, None, Path::new(SUBSTRATE), "kept.suite.yaml")
        .output()
        .unwrap();
    passed(&passing);
    assert!(
        text(&passing.stdout).contains("one: as expected ("),
        "{}",
        text(&passing.stdout)
    );
    assert_eq!(
        fs::read_to_string(cycle_target(&dir, 2, "own.txt"))
            .unwrap()
            .trim(),
        "two"
    );
    let failing_dir = fresh_scratch("substrate-again-failing");
    let failing = substrate_suite(
        &failing_dir,
        None,
        Path::new(SUBSTRATE),
        "failing.suite.yaml",
    )
    .output()
    .unwrap();
    let said = format!("{}{}", text(&failing.stdout), text(&failing.stderr));
    assert_eq!(failing.status.code(), Some(1), "{said}");
    assert!(said.contains("fails: FAILED: "), "{said}");
    assert!(said.contains("one: as expected ("), "{said}");
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&failing_dir);
}

/// Test 6: `run_suite` takes the runner `TreeRunner::on_root` builds, in a test process in a
/// child of the root: two suites, one after the other, under the one hold; once the runner and
/// the settings are dropped, the same process holds the root again. The test starts its own
/// binary again in `<scope>/run`, which does the work.
#[test]
fn run_suite_takes_the_runner_that_holds_the_root() {
    if let Some(root) =
        std::env::var_os(SUITE_ROOT).filter(|_| std::env::var_os(IN_SCOPE).is_some())
    {
        run_suite_on_the_held_root(Path::new(&root));
        return;
    }
    let Some(scope) = Scope::start() else {
        return;
    };
    let name = format!(
        "{}::run_suite_takes_the_runner_that_holds_the_root",
        module_path!()
            .split_once("::")
            .map_or(module_path!(), |(_, path)| path)
    );
    let output = Command::new("sh")
        .arg(Path::new(DELEGATED).join("enter.sh"))
        .arg(scope.root.join("run"))
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", &name, "--nocapture"])
        .env(IN_SCOPE, "1")
        .env(SUITE_ROOT, &scope.root)
        .output()
        .unwrap();
    let said = format!("{}{}", text(&output.stdout), text(&output.stderr));
    assert_eq!(output.status.code(), Some(0), "{said}");
    assert!(said.contains(&format!("test {name} ... ok")), "{said}");
    assert!(said.contains("1 passed"), "{said}");
}

/// The work of [`run_suite_takes_the_runner_that_holds_the_root`], in a child of `root`.
fn run_suite_on_the_held_root(root: &Path) {
    let held = TreeRunner::on_root(root).unwrap();
    let mut settings = SuiteSettings::new(
        Path::new(SUBSTRATE).join("protocols"),
        PathBuf::new(),
        TreeTime::At(AT.to_owned()),
        held.runner(),
        Some("substrate".to_owned()),
    );
    settings.workers = NonZeroUsize::new(2).unwrap();
    for label in ["held-first", "held-second"] {
        let scratch = fresh_scratch(label);
        settings.scratch = scratch.clone();
        let report = run_suite(
            &Path::new(SUBSTRATE).join("kept.suite.yaml"),
            &settings,
            None,
            None,
        )
        .unwrap();
        assert!(report.failed.is_empty(), "{label}: {report:?}");
        assert_eq!(report.as_expected, 2, "{label}: {report:?}");
        for (n, name) in [(1, "one"), (2, "two")] {
            let own = scratch.join(format!("work/{n}/target/own.txt"));
            assert_eq!(fs::read_to_string(&own).unwrap().trim(), name, "{label}");
        }
        let _ = fs::remove_dir_all(&scratch);
    }
    // While the runner lives the root is held: this process cannot hold it a second time.
    let twice = TreeRunner::on_root(root);
    assert!(twice.is_err(), "the root was held twice: {twice:?}");
    drop(settings);
    drop(held);
    let again = TreeRunner::on_root(root);
    assert!(
        again.is_ok(),
        "the hold outlived its runner: {:?}",
        again.err()
    );
}

/// Test 8: `--confinement local --cgroup-root <dir>` is refused `ConfinementRefused`, exit 2.
#[test]
fn local_with_a_cgroup_root_is_refused() {
    let dir = fresh_scratch("local-root");
    fs::create_dir_all(dir.join("plain")).unwrap();
    let output = substrate_suite(&dir, None, Path::new(SUBSTRATE), "own.suite.yaml")
        .args(["--confinement", "local", "--cgroup-root"])
        .arg(dir.join("plain"))
        .output()
        .unwrap();
    let said = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{said}");
    assert!(
        said.starts_with(&format!("{TEST_REFUSED}ConfinementRefused: ")),
        "{said}"
    );
    assert!(said.contains("--cgroup-root"), "{said}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    assert!(!dir.join("scratch").exists());
    let _ = fs::remove_dir_all(&dir);
}

/// Test 9: a `--cgroup-root` that is a plain directory, no cgroup, is a refused hold: exit 2,
/// naming `CgroupUndelegated` and the directory, before any cycle.
#[test]
fn a_refused_hold_exits_2() {
    let dir = fresh_scratch("refused-hold");
    let plain = dir.join("plain");
    fs::create_dir_all(&plain).unwrap();
    let output = substrate_suite(&dir, None, Path::new(SUBSTRATE), "kept.suite.yaml")
        .arg("--cgroup-root")
        .arg(&plain)
        .output()
        .unwrap();
    let said = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{said}");
    assert!(said.starts_with(TEST_REFUSED), "{said}");
    assert!(said.contains("CgroupUndelegated"), "{said}");
    assert!(said.contains(&plain.display().to_string()), "{said}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    assert!(!dir.join("scratch").exists());
    let _ = fs::remove_dir_all(&dir);
}

/// Test 10: with no `--cgroup-root` and no `systemd-run` on `PATH`, the start again fails: exit 2.
#[test]
fn a_failed_start_again_exits_2() {
    let dir = fresh_scratch("failed-start-again");
    fs::create_dir_all(dir.join("no-programs")).unwrap();
    let output = substrate_suite(&dir, None, Path::new(SUBSTRATE), "kept.suite.yaml")
        .env("PATH", dir.join("no-programs"))
        .output()
        .unwrap();
    let said = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{said}");
    assert!(said.starts_with(TEST_REFUSED), "{said}");
    assert!(
        said.contains("tree test could not start itself again"),
        "{said}"
    );
    assert!(!dir.join("scratch").exists());
    let _ = fs::remove_dir_all(&dir);
}

/// Test 11: a `tree test` marked started again (`B10X_LOOM_CONFINEMENT_REEXEC`) without the
/// handshake path (`B10X_LOOM_CONFINEMENT_READY`) is refused: exit 2.
#[test]
fn a_refused_handshake_exits_2() {
    let dir = fresh_scratch("refused-handshake");
    let output = substrate_suite(&dir, None, Path::new(SUBSTRATE), "kept.suite.yaml")
        .env("B10X_LOOM_CONFINEMENT_REEXEC", "1")
        .env_remove("B10X_LOOM_CONFINEMENT_READY")
        .output()
        .unwrap();
    let said = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{said}");
    assert!(said.starts_with(TEST_REFUSED), "{said}");
    assert!(
        said.contains("the delegation handshake path is missing"),
        "{said}"
    );
    assert!(!dir.join("scratch").exists());
    let _ = fs::remove_dir_all(&dir);
}

// Acceptance for `story:tree-test-command`: `--cycle-timeout` and the cycle kill, the `slowest:`
// line, `--list`, and the availability that no longer names this story. On a host this loaded a
// 5 s self-check fails for load alone, so these tests assert no wall time of their own but the
// 60 s bound of one test; test 1 prints its time.

/// The suite `parallel/timeout.suite.yaml`: `sleeps`, whose program writes its process id to
/// `pid.txt` and sleeps 30 s, and `brief`, whose program ends in 0.3 s.
const TIMEOUT_SUITE: &str = "timeout.suite.yaml";

/// The suite `list.suite.yaml`: five cycles, `b` pending, `c` waiting on its `decide` step.
const LIST_SUITE: &str = "list.suite.yaml";

/// Whether a process `pid` names is still there (`kill -0`).
fn process_exists(pid: &str) -> bool {
    Command::new("sh")
        .args(["-c", "kill -0 \"$1\" 2>/dev/null", "sh", pid])
        .status()
        .unwrap()
        .success()
}

/// Test 1: with `--cycle-timeout 2` the watcher sets `sleeps`' flag at its deadline while its
/// worker is blocked in `run_tree`; its program is killed, the cycle ends `CycleTimedOut` at
/// `steps[1]` and `brief` ends as expected beside it.
#[test]
fn a_cycle_over_its_timeout_is_killed() {
    let started = Instant::now();
    let dir = fresh_scratch("cycle-timeout");
    let tested = tree_test_in(
        &dir,
        "scratch",
        &Path::new(PARALLEL).join(TIMEOUT_SUITE),
        &Path::new(PARALLEL).join("protocols"),
        &["--cycle-timeout", "2", "--workers", "2"],
    );
    let spent = started.elapsed();
    eprintln!("a_cycle_over_its_timeout_is_killed: the command took {spent:?}");
    let said = format!("{}{}", text(&tested.stdout), text(&tested.stderr));
    assert_eq!(tested.status.code(), Some(1), "{said}");
    let report = read_report(&dir.join("scratch.json"));
    let sleeps = cycle_of(&report, "sleeps");
    assert_eq!(sleeps["as_expected"], false, "{said}{sleeps}");
    let difference = &sleeps["difference"];
    assert_eq!(
        difference["reason"],
        format!("{:?}", DifferenceReason::CycleTimedOut),
        "{said}{sleeps}"
    );
    assert_eq!(difference["check"], "steps[1]", "{sleeps}");
    assert_eq!(difference["kind"], "run", "{sleeps}");
    let found: f64 = difference["found"]
        .as_str()
        .and_then(|found| found.parse().ok())
        .unwrap_or_else(|| panic!("found is no number of seconds: {sleeps}"));
    assert!(found >= 2.0, "{sleeps}");
    // The cycle ended long before its program's 30 s sleep would have.
    let seconds = sleeps["seconds"].as_f64().unwrap();
    assert!(seconds < 15.0, "{sleeps}");
    assert!(
        said.lines()
            .any(|line| line.starts_with("sleeps: FAILED: steps[1] (run, CycleTimedOut)")),
        "{said}"
    );
    // No process of the program is left.
    let pid = fs::read_to_string(dir.join("scratch/work/1/pid.txt")).unwrap();
    let pid = pid.trim();
    assert!(!pid.is_empty());
    assert!(!process_exists(pid), "process {pid} is still there");
    let brief = cycle_of(&report, "brief");
    assert_eq!(brief["as_expected"], true, "{said}{brief}");
    assert_eq!(brief["exit"], 0, "{brief}");
    assert_eq!(report["failed"], json!(["sleeps"]), "{report}");
    let _ = fs::remove_dir_all(&dir);
    assert!(started.elapsed() < RUN_BOUND, "the test took {spent:?}");
}

/// Test 2: the shared run prints a `slowest:` line right after the two summary lines: the cycles
/// that ran, at most ten, slowest first, each with its seconds.
#[test]
fn the_ten_slowest_follow_the_summary() {
    let ran = ran();
    let lines: Vec<&str> = ran.stdout.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.starts_with("pending: "))
        .unwrap_or_else(|| panic!("no summary: {}", ran.stdout));
    assert!(lines[at - 1].starts_with("cycles: "), "{}", ran.stdout);
    let slowest = lines
        .get(at + 1)
        .and_then(|line| line.strip_prefix("slowest: "))
        .unwrap_or_else(|| panic!("no slowest line after the summary: {}", ran.stdout));
    let named: Vec<(&str, f64)> = slowest
        .split(", ")
        .map(|part| {
            let (name, seconds) = part
                .strip_suffix(" s)")
                .and_then(|part| part.split_once(" ("))
                .unwrap_or_else(|| panic!("{part:?} is not <name> (<s> s): {slowest}"));
            (name, seconds.parse::<f64>().unwrap())
        })
        .collect();
    assert!(named.len() <= 10, "{slowest}");
    // The four cycles that ran (`allows` waits), each named once.
    let mut names: Vec<&str> = named.iter().map(|(name, _)| *name).collect();
    names.sort_unstable();
    assert_eq!(names, ["arrives", "later", "waits", "wrong"], "{slowest}");
    assert!(
        named.windows(2).all(|pair| pair[0].1 >= pair[1].1),
        "not slowest first: {slowest}"
    );
    // Each with the seconds its own line printed.
    let report = ran.report();
    for (name, seconds) in &named {
        assert_eq!(
            cycle_of(&report, name)["seconds"].as_f64(),
            Some(*seconds),
            "{slowest}"
        );
    }
}

/// `tree test --list` of `suite` with `extra`, in `dir` as its current directory, with no
/// `--protocols`, `--scratch` or `--at`.
fn list(dir: &Path, suite: &Path, extra: &[&str]) -> Output {
    fs::create_dir_all(dir.join("home")).unwrap();
    command(&dir.join("home"))
        .current_dir(dir)
        .args(["tree", "test", "--list", "--suite"])
        .arg(suite)
        .args(extra)
        .output()
        .unwrap()
}

/// What `dir` holds, by name.
fn entries(dir: &Path) -> Vec<String> {
    let mut found: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

/// Test 3: `--list` prints the suite's five cycles in suite order and runs nothing; `--only b`
/// prints one line.
#[test]
fn list_prints_the_cycles_and_runs_nothing() {
    let started = Instant::now();
    let dir = fresh_scratch("list");
    let listed = list(&dir, &Path::new(FIXTURE).join(LIST_SUITE), &[]);
    let stdout = text(&listed.stdout);
    assert_eq!(
        listed.status.code(),
        Some(0),
        "{stdout}{}",
        text(&listed.stderr)
    );
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        [
            "a",
            "b: pending",
            "c: pending, waits for story:tree-test-decide-and-requests (decide)",
            "d",
            "e",
        ],
        "{stdout}"
    );
    // Nothing is written: the directory holds the empty home it was given.
    assert_eq!(entries(&dir), ["home"]);
    assert!(entries(&dir.join("home")).is_empty());
    let only = list(&dir, &Path::new(FIXTURE).join(LIST_SUITE), &["--only", "b"]);
    assert_eq!(only.status.code(), Some(0), "{}", text(&only.stderr));
    assert_eq!(text(&only.stdout), "b: pending\n");
    assert_eq!(entries(&dir), ["home"]);
    let _ = fs::remove_dir_all(&dir);
    assert!(started.elapsed() < RUN_BOUND);
}

/// Test 4: `--list` of a suite with an unknown step kind is refused, exit 2.
#[test]
fn a_refused_suite_is_refused_by_list() {
    let started = Instant::now();
    let dir = fresh_scratch("list-refused");
    let listed = list(
        &dir,
        &Path::new(FIXTURE).join("unknown-step.suite.yaml"),
        &[],
    );
    let stderr = text(&listed.stderr);
    assert_eq!(listed.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.starts_with("b10x-loom: tree test refused:"),
        "{stderr}"
    );
    assert!(stderr.contains("StepKindUnknown"), "{stderr}");
    assert!(listed.stdout.is_empty(), "{}", text(&listed.stdout));
    let _ = fs::remove_dir_all(&dir);
    assert!(started.elapsed() < RUN_BOUND);
}

/// Test 5: `--cycle-timeout` 61 and 0 are the command's own refusal, before the suite is read;
/// 60 with `--list` decodes the suite and runs no cycle.
#[test]
fn a_cycle_timeout_above_60_is_refused() {
    let started = Instant::now();
    let dir = fresh_scratch("cycle-timeout-range");
    let suite = Path::new(FIXTURE).join(LIST_SUITE);
    for seconds in ["61", "0"] {
        let tested = tree_test_in(
            &dir,
            "scratch",
            &suite,
            Path::new(PROTOCOLS),
            &["--cycle-timeout", seconds],
        );
        let stderr = text(&tested.stderr);
        assert_eq!(tested.status.code(), Some(2), "{seconds}: {stderr}");
        assert!(
            stderr.starts_with("b10x-loom: tree test refused:"),
            "{seconds}: {stderr}"
        );
        assert!(stderr.contains("--cycle-timeout"), "{seconds}: {stderr}");
        assert!(stderr.contains("1 to 60"), "{seconds}: {stderr}");
        assert!(!stderr.contains("invalid value"), "{seconds}: {stderr}");
        assert!(tested.stdout.is_empty(), "{}", text(&tested.stdout));
        assert!(!dir.join("scratch").exists());
    }
    let listed = list(&dir, &suite, &["--cycle-timeout", "60"]);
    assert_eq!(listed.status.code(), Some(0), "{}", text(&listed.stderr));
    assert_eq!(text(&listed.stdout).lines().count(), 5);
    assert!(!dir.join("scratch").exists());
    let _ = fs::remove_dir_all(&dir);
    assert!(started.elapsed() < RUN_BOUND);
}

/// Test 6: no part of the format waits for this story; `decide` and `requests` wait for
/// `story:tree-test-decide-and-requests`.
#[test]
fn nothing_waits_for_this_story() {
    use b10x_loom_tree::suite::{Availability, KindUse, SUITE_AVAILABILITY};
    assert!(
        !SUITE_AVAILABILITY
            .iter()
            .any(|entry| entry.availability == Availability::Waits("story:tree-test-command")),
        "{SUITE_AVAILABILITY:?}"
    );
    for (used, name) in [
        (KindUse::Step, "decide"),
        (KindUse::Entry, "decide"),
        (KindUse::Source, "requests"),
    ] {
        let entry = SUITE_AVAILABILITY
            .iter()
            .find(|entry| entry.used == used && entry.name == name)
            .unwrap_or_else(|| panic!("no entry {used:?} {name}"));
        assert_eq!(
            entry.availability,
            Availability::Waits("story:tree-test-decide-and-requests"),
            "{used:?} {name}"
        );
    }
}
