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

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
use b10x_loom_intake_slice::confinement::{ConfinementAdapter, EngineConfinement, EngineRunner};
use b10x_loom_tree::model::CycleResult;
use b10x_loom_tree::suite::{SuiteSettings, run_suite};
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
