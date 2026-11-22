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

mod substrate;

use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
use b10x_loom_intake_slice::confinement::{ConfinementAdapter, EngineConfinement, EngineRunner};
use b10x_loom_tree::model::{CycleResult, DifferenceReason, SuiteReport};
use b10x_loom_tree::suite::{SuiteSettings, report_json, run_suite};
use b10x_loom_tree::{TreeTime, read_protocols_directory, verify_tree};
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

/// `b10x-loom` with `HOME` and `XDG_DATA_HOME` at `home`.
fn command(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_b10x-loom"));
    command
        .env("HOME", home)
        .env("XDG_DATA_HOME", home)
        .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
        .env_remove("RUST_LOG");
    command
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
        ran.line("replays"),
        "replays: pending, waits for story:tree-replay-command (replay)"
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
    assert_eq!(lines.len(), 7, "{}", ran.stdout);
    assert_eq!(
        lines[5],
        "cycles: 3 expected, 2 as expected, 1 failed: wrong"
    );
    assert_eq!(
        lines[6],
        "pending: 2 (1 still failing: later; passing, move to expected: none; 1 waiting: replays on story:tree-replay-command)"
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
        json!([{"name": "replays", "story": "story:tree-replay-command", "kind": "replay"}]),
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
    for cycle in fs::read_dir(ran.root.join("scratch")).unwrap() {
        let store = cycle.unwrap().path().join("store");
        let verified = verify_tree(&store, &catalog).unwrap();
        assert!(
            verified.failure.is_none(),
            "{}: {verified:?}",
            store.display()
        );
        stores += 1;
    }
    // Four cycles ran (`replays` waits); each left its store.
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
    let kept = fs::read_to_string(ran.root.join("scratch/4/work/check.log")).unwrap();
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
    assert!(scratch.join("1/store/tree.json").is_file());
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
    assert!(!scratch.join("1").exists());
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
        let path = self.scratch().join(n.to_string()).join("work").join(name);
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }
}

impl Drop for Parallel {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
        let _ = fs::remove_dir_all(&self.suite);
    }
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
/// name in its own `<n>/`.
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
    let mut top: Vec<String> = fs::read_dir(run.scratch())
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_dir(), "{entry:?}");
            entry.file_name().to_string_lossy().into_owned()
        })
        .collect();
    top.sort();
    assert_eq!(top, ["1", "2", "3"]);
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
    assert!(!run.scratch().join("3").exists());
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
    let mut made: Vec<String> = fs::read_dir(dir.join("scratch"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    made.sort();
    assert_eq!(made, ["2", "4"]);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 4, "{stdout}");
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
    // The output, each cycle's wall time apart.
    let lines = |output: &Output| -> Vec<String> {
        text(&output.stdout)
            .lines()
            .map(|line| match line.rfind(" (") {
                Some(at) if line.ends_with(" s)") => line[..at].to_owned(),
                _ => line.to_owned(),
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

/// Test 12: more than one worker under `substrate` is refused before any cycle, by the command
/// and by `run_suite`; one worker keeps the refusal of the missing root.
#[test]
fn workers_above_one_under_substrate_is_refused() {
    let started = Instant::now();
    let dir = fresh_scratch("substrate-workers");
    fs::create_dir_all(dir.join("home")).unwrap();
    let substrate = |workers: &str| {
        command(&dir.join("home"))
            .args(["tree", "test", "--confinement", "substrate"])
            .args(["--workers", workers, "--at", AT])
            .arg("--suite")
            .arg(Path::new(FIXTURE).join("one.suite.yaml"))
            .arg("--protocols")
            .arg(PROTOCOLS)
            .arg("--scratch")
            .arg(dir.join("scratch"))
            .output()
            .unwrap()
    };
    let two = substrate("2");
    let said = text(&two.stderr);
    assert_eq!(two.status.code(), Some(2), "{said}");
    assert!(said.contains("SubstrateWorkersUnserved"), "{said}");
    assert!(
        said.contains("story:suite-runs-cycles-under-one-cgroup-root"),
        "{said}"
    );
    assert!(two.stdout.is_empty());
    assert!(!dir.join("scratch").exists());

    let mut settings = local_settings(dir.join("scratch"));
    settings.confinement = Some("substrate".to_owned());
    settings.workers = NonZeroUsize::new(2).unwrap();
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
        refused
            .to_string()
            .contains("story:suite-runs-cycles-under-one-cgroup-root"),
        "{refused}"
    );
    assert!(!dir.join("scratch").exists());

    let one = substrate("1");
    let said = text(&one.stderr);
    assert_eq!(one.status.code(), Some(2), "{said}");
    assert!(said.contains("ConfinementRefused"), "{said}");
    assert!(!said.contains("SubstrateWorkersUnserved"), "{said}");
    assert!(!dir.join("scratch").exists());
    let _ = fs::remove_dir_all(&dir);
    within_bound(started);
}

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
