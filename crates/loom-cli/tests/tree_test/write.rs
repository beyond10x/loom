//! Acceptance for `story:suite-cycle-writes-a-workspace-file`: the step `write` copies a file of
//! the suite's directory to a path in the cycle's workspace between two of its steps.
//!
//! The fixture is `fixtures/tree-suite/write/`: the protocol `finding` (a program writes the
//! findings, a second one applies the decision they hold once `owner.decide` is granted), its
//! bindings, POSIX sh stubs, seeds and one suite per test. Every `from` holds [`CANARY`]. A test
//! that needs a path outside the suite (a directory of its own, the path of its copy) copies the
//! fixture and writes that path into a file of the copy, which its cycle seeds (see [`Copied`]).
//! Every program runs under the `local` runner; each test asserts its own time under 5 s.

use std::fs;
use std::os::unix::fs::{FileTypeExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use b10x_loom_tree::model::{CheckDifference, CycleResult, DifferenceReason, SuiteReport};
use b10x_loom_tree::suite::{SuiteSettings, cycle_line, report_json, run_suite};
use b10x_loom_tree::{log_file_name, read_protocols_directory, verify_tree};
use serde_json::Value;

use super::{copy_dir, fresh_scratch, local_settings, text, tree_test_in, within_bound};

const WRITE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/tree-suite/write"
);

/// The text every `from` of the fixture holds.
const CANARY: &str = "CANARY-w7rite";

/// [`local_settings`] over the protocols of the suite directory `suite`, with `workers`.
fn settings(suite: &Path, scratch: PathBuf, workers: usize) -> SuiteSettings {
    let mut settings = local_settings(scratch);
    settings.protocols = suite.join("protocols");
    settings.workers = std::num::NonZeroUsize::new(workers).unwrap();
    settings
}

/// The fixture's suite file `name`, run with `workers` in the fresh scratch directory `label`.
fn run_fixture(name: &str, label: &str, workers: usize) -> (SuiteReport, PathBuf) {
    let scratch = fresh_scratch(label);
    let report = run_suite(
        &Path::new(WRITE).join(name),
        &settings(Path::new(WRITE), scratch.clone(), workers),
        None,
        None,
    )
    .unwrap();
    (report, scratch)
}

/// A test's copy of the fixture: `dir` holds `scratch/` and what the test makes outside the suite,
/// `suite` is the copy, whose `paths/` the test writes. Both are removed when it is dropped.
struct Copied {
    dir: PathBuf,
    suite: PathBuf,
}

impl Copied {
    fn new(label: &str) -> Self {
        let dir = fresh_scratch(label);
        let suite = fresh_scratch(&format!("{label}-suite"));
        copy_dir(Path::new(WRITE), &suite);
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(suite.join("paths")).unwrap();
        Self { dir, suite }
    }

    /// Writes `path` into the copy's `paths/<name>`.
    fn path_file(&self, name: &str, path: &Path) {
        fs::write(
            self.suite.join("paths").join(name),
            path.as_os_str().as_encoded_bytes(),
        )
        .unwrap();
    }

    fn scratch(&self) -> PathBuf {
        self.dir.join("scratch")
    }

    fn settings(&self, workers: usize) -> SuiteSettings {
        settings(&self.suite, self.scratch(), workers)
    }

    /// The copy's suite file `name`, run with `workers`.
    fn run(&self, name: &str, workers: usize) -> SuiteReport {
        run_suite(&self.suite.join(name), &self.settings(workers), None, None).unwrap()
    }

    /// The path `name` of cycle `n`'s workspace.
    fn work(&self, n: usize, name: &str) -> PathBuf {
        self.scratch().join(n.to_string()).join("work").join(name)
    }
}

impl Drop for Copied {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
        let _ = fs::remove_dir_all(&self.suite);
    }
}

fn cycle<'r>(report: &'r SuiteReport, name: &str) -> &'r CycleResult {
    report
        .cycles
        .iter()
        .find(|cycle| cycle.name == name)
        .unwrap_or_else(|| panic!("no cycle {name}: {report:?}"))
}

fn difference<'r>(report: &'r SuiteReport, name: &str) -> &'r CheckDifference {
    cycle(report, name)
        .difference
        .as_ref()
        .unwrap_or_else(|| panic!("{name} has no difference: {report:?}"))
}

/// Asserts the cycle `name` ended at its `write` step `check` with `reason`, found `found` when
/// one is given.
fn write_failed(
    report: &SuiteReport,
    name: &str,
    check: &str,
    reason: DifferenceReason,
    found: Option<&str>,
) {
    let difference = difference(report, name);
    assert_eq!(difference.check, check, "{name}: {difference:?}");
    assert_eq!(difference.kind, "write", "{name}: {difference:?}");
    assert_eq!(difference.reason, reason, "{name}: {difference:?}");
    if let Some(found) = found {
        assert_eq!(
            difference.found.as_deref(),
            Some(found),
            "{name}: {difference:?}"
        );
    }
}

/// Asserts every cycle of `report` ended with a difference, and neither its `CycleResult` JSON nor
/// its printed line holds [`CANARY`].
fn never_shows_the_canary(report: &SuiteReport) {
    let document = report_json(report);
    for (result, json) in report
        .cycles
        .iter()
        .zip(document["cycles"].as_array().unwrap())
    {
        assert!(result.difference.is_some(), "{result:?}");
        let json = json.to_string();
        assert!(!json.contains(CANARY), "{json}");
        let line = cycle_line(result);
        assert!(!line.contains(CANARY), "{line}");
    }
}

/// The bytes of the fixture's `seeds/findings-decided.yaml`.
fn decided_bytes() -> Vec<u8> {
    fs::read(Path::new(WRITE).join("seeds/findings-decided.yaml")).unwrap()
}

/// Test 1: the first run writes an open finding as its last act after 0.5 s, the `write` replaces
/// it, and the second run reads the decision and ends exit 0.
#[test]
fn a_write_between_two_runs_is_read_by_the_second() {
    let started = Instant::now();
    let (report, scratch) = run_fixture("decided.suite.yaml", "write-decided", 1);
    let decided = cycle(&report, "decided");
    assert!(decided.as_expected, "{report:?}");
    assert_eq!(decided.exit, Some(0), "{report:?}");
    assert_eq!(
        fs::read(scratch.join("1/work/findings.yaml")).unwrap(),
        decided_bytes()
    );
    let _ = fs::remove_dir_all(&scratch);
    within_bound(started);
}

/// Test 2: a `write` makes `a/b/` and the file with mode 0644, and one over a file of mode 0600
/// leaves mode 0644.
#[test]
fn a_write_makes_a_new_file_and_its_directories() {
    let started = Instant::now();
    let (report, scratch) = run_fixture("modes.suite.yaml", "write-modes", 1);
    assert!(cycle(&report, "makes").as_expected, "{report:?}");
    for name in ["a/b/new.txt", "old.txt"] {
        let path = scratch.join("1/work").join(name);
        assert_eq!(fs::read(&path).unwrap(), decided_bytes(), "{name}");
        let mode = fs::symlink_metadata(&path).unwrap().permissions().mode() & 0o7777;
        assert_eq!(mode, 0o644, "{name}: {mode:o}");
    }
    assert!(scratch.join("1/work/a/b").is_dir());
    let _ = fs::remove_dir_all(&scratch);
    within_bound(started);
}

/// The run of `links.suite.yaml` in a copy whose cycles link `out` to `<dir>/outside/f`.
fn links_run(label: &str) -> (Copied, SuiteReport) {
    let copied = Copied::new(label);
    let outside = copied.dir.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("f"), "outside, unchanged\n").unwrap();
    copied.path_file("outside.txt", &outside);
    let report = copied.run("links.suite.yaml", 2);
    (copied, report)
}

/// Test 3: a `write` to a link to a file outside, or below a link to `..`, ends
/// `PathOutsideCycle` and writes nothing outside the workspace.
#[test]
fn a_write_never_follows_a_link() {
    let started = Instant::now();
    let (copied, report) = links_run("write-links");
    write_failed(
        &report,
        "link-file",
        "through-out",
        DifferenceReason::PathOutsideCycle,
        None,
    );
    write_failed(
        &report,
        "link-dir",
        "through-up",
        DifferenceReason::PathOutsideCycle,
        None,
    );
    assert_eq!(
        fs::read_to_string(copied.dir.join("outside/f")).unwrap(),
        "outside, unchanged\n"
    );
    // `up` is `..` of cycle 2's workspace: its cycle directory, which holds no `x`.
    assert!(fs::symlink_metadata(copied.scratch().join("2/x")).is_err());
    assert!(
        fs::symlink_metadata(copied.work(1, "out"))
            .unwrap()
            .is_symlink()
    );
    never_shows_the_canary(&report);
    within_bound(started);
}

/// The run of `no-file.suite.yaml`, whose cycles make a directory, a fifo and a directory of mode
/// 0555 where the `write` goes.
fn no_file_run(label: &str) -> (PathBuf, SuiteReport) {
    let (report, scratch) = run_fixture("no-file.suite.yaml", label, 3);
    (scratch, report)
}

/// Test 4: a directory, a fifo at `to` and a parent no file may be made in each end `StepFailed`,
/// found as the entry's kind or the error's.
#[test]
fn a_write_onto_what_is_no_file_fails_the_step() {
    let started = Instant::now();
    let (scratch, report) = no_file_run("write-no-file");
    for (name, found) in [
        ("onto-directory", "directory"),
        ("onto-fifo", "fifo"),
        ("into-read-only", "permission denied"),
    ] {
        write_failed(
            &report,
            name,
            "steps[2]",
            DifferenceReason::StepFailed,
            Some(found),
        );
    }
    assert!(scratch.join("1/work/target").is_dir());
    assert!(
        fs::symlink_metadata(scratch.join("2/work/target"))
            .unwrap()
            .file_type()
            .is_fifo()
    );
    assert_eq!(fs::read_dir(scratch.join("3/work/ro")).unwrap().count(), 0);
    never_shows_the_canary(&report);
    let _ = fs::remove_dir_all(&scratch);
    within_bound(started);
}

/// Test 6: a `from` that is a link, is behind a linked directory, is a directory, is absent or is
/// 1 MiB + 1 byte is refused `SeedUnusable` naming it, before any cycle.
#[test]
fn a_write_source_must_be_a_small_regular_file() {
    let started = Instant::now();
    let dir = fresh_scratch("write-source");
    let suite = dir.join("suite");
    fs::create_dir_all(suite.join("seeds")).unwrap();
    fs::create_dir_all(dir.join("real")).unwrap();
    fs::write(dir.join("outside.yaml"), format!("# {CANARY}\n")).unwrap();
    fs::write(dir.join("real/f.yaml"), format!("# {CANARY}\n")).unwrap();
    std::os::unix::fs::symlink(dir.join("outside.yaml"), suite.join("seeds/link.yaml")).unwrap();
    std::os::unix::fs::symlink(dir.join("real"), suite.join("linked")).unwrap();
    let mut big = format!("# {CANARY}\n").into_bytes();
    big.resize(1024 * 1024 + 1, b'x');
    fs::write(suite.join("seeds/big.yaml"), big).unwrap();
    for from in [
        "seeds/link.yaml",
        "linked/f.yaml",
        "seeds",
        "seeds/absent.yaml",
        "seeds/big.yaml",
    ] {
        fs::write(
            suite.join("suite.yaml"),
            format!(
                "format: loom.tree-suite/1\ncycles:\n  - name: writes\n    protocol: tour@1\n    bindings: b.yaml\n    steps:\n      - {{do: open}}\n      - {{do: write, from: {from}, to: findings.yaml}}\n"
            ),
        )
        .unwrap();
        let refused = run_suite(
            &suite.join("suite.yaml"),
            &local_settings(dir.join("scratch")),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(
            format!("{:?}", refused.reason()),
            "SeedUnusable",
            "{from}: {refused}"
        );
        assert_eq!(
            refused.member(),
            Some("cycles[0].steps[1].from"),
            "{from}: {refused}"
        );
        assert!(!refused.to_string().contains(CANARY), "{refused}");
        assert!(!dir.join("scratch").exists(), "{from}");
    }
    let _ = fs::remove_dir_all(&dir);
    within_bound(started);
}

/// The run of `changed.suite.yaml` in a copy, whose first runs grow one `from` past 1 MiB and
/// replace the other with a link.
fn changed_run(label: &str) -> (Copied, SuiteReport) {
    let copied = Copied::new(label);
    copied.path_file("suite.txt", &copied.suite);
    let report = copied.run("changed.suite.yaml", 2);
    (copied, report)
}

/// Test 8: a `from` that grew past 1 MiB or turned into a link after the check before the cycles
/// ends its `write` `StepFailed`, and nothing is written at `to`.
#[test]
fn a_from_that_grew_or_turned_into_a_link_fails_the_step() {
    let started = Instant::now();
    let (copied, report) = changed_run("write-changed");
    // The programs did change the copy's sources, never the committed fixture's.
    assert!(
        fs::metadata(copied.suite.join("seeds/grows.yaml"))
            .unwrap()
            .len()
            > 1024 * 1024
    );
    assert!(
        fs::symlink_metadata(copied.suite.join("seeds/relinks.yaml"))
            .unwrap()
            .is_symlink()
    );
    assert!(
        fs::metadata(Path::new(WRITE).join("seeds/grows.yaml"))
            .unwrap()
            .len()
            < 1024
    );
    for (n, name, check) in [(1, "grown", "from-grown"), (2, "relinked", "from-relinked")] {
        write_failed(&report, name, check, DifferenceReason::StepFailed, None);
        assert!(
            fs::symlink_metadata(copied.work(n, "findings.yaml")).is_err(),
            "{name}"
        );
    }
    never_shows_the_canary(&report);
    within_bound(started);
}

/// Test 9: two `compare` checks on the `write` step find the store's files and the root's run
/// log as they were after the run before it, and `tree verify` passes at the cycle's end.
#[test]
fn a_write_leaves_the_store_and_the_log_alone() {
    let started = Instant::now();
    let suite = fs::read_to_string(Path::new(WRITE).join("store.suite.yaml")).unwrap();
    assert!(suite.contains(&log_file_name("root")), "{suite}");
    let (report, scratch) = run_fixture("store.suite.yaml", "write-store", 1);
    assert!(cycle(&report, "alone").as_expected, "{report:?}");
    assert_eq!(
        fs::read(scratch.join("1/work/findings.yaml")).unwrap(),
        decided_bytes()
    );
    let catalog = read_protocols_directory(&Path::new(WRITE).join("protocols"))
        .unwrap()
        .catalog;
    let verified = verify_tree(&scratch.join("1/store"), &catalog).unwrap();
    assert!(verified.failure.is_none(), "{verified:?}");
    let _ = fs::remove_dir_all(&scratch);
    within_bound(started);
}

/// The `at` of the line after the first `Stopped` line of the root's run log in `store`: the
/// first line of the second `run` step.
fn second_run_at(store: &Path) -> String {
    let log = fs::read_to_string(store.join(log_file_name("root"))).unwrap();
    let lines: Vec<Value> = log
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let stopped = lines
        .iter()
        .position(|line| line["kind"] == "Stopped")
        .unwrap_or_else(|| panic!("no Stopped line: {log}"));
    lines
        .get(stopped + 1)
        .unwrap_or_else(|| panic!("no line after the first Stopped: {log}"))["at"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Test 10: a `write` leaves the minute count of `at` unchanged.
#[test]
fn a_write_keeps_the_instants_of_later_steps() {
    let started = Instant::now();
    let (report, scratch) = run_fixture("instants.suite.yaml", "write-instants", 2);
    assert!(report.failed.is_empty(), "{report:?}");
    let with = second_run_at(&scratch.join("1/store"));
    let without = second_run_at(&scratch.join("2/store"));
    assert_eq!(with, without);
    assert_eq!(with, "2026-11-22T08:02:00Z");
    let _ = fs::remove_dir_all(&scratch);
    within_bound(started);
}

/// Test 11: no difference of tests 3, 4 and 8 shows the bytes of `from`, in the `CycleResult`
/// JSON or the printed line. The three runs go at once.
#[test]
fn a_failed_write_never_shows_the_content() {
    let started = Instant::now();
    let (links, no_file, changed) = std::thread::scope(|scope| {
        let links = scope.spawn(|| links_run("write-canary-links"));
        let no_file = scope.spawn(|| no_file_run("write-canary-no-file"));
        let changed = scope.spawn(|| changed_run("write-canary-changed"));
        (
            links.join().unwrap(),
            no_file.join().unwrap(),
            changed.join().unwrap(),
        )
    });
    for report in [&links.1, &no_file.1, &changed.1] {
        assert!(!report.cycles.is_empty(), "{report:?}");
        never_shows_the_canary(report);
    }
    let _ = fs::remove_dir_all(&no_file.0);
    within_bound(started);
}

/// Test 12: the flag is set while the `run` before the `write` is in flight; its program is
/// killed, the cycle ends at the `run`, and `to` never gets the bytes of `from`.
#[test]
fn a_cancelled_run_before_a_write_leaves_to_alone() {
    let started = Instant::now();
    let copied = Copied::new("write-cancel");
    let markers = copied.dir.join("markers");
    fs::create_dir_all(&markers).unwrap();
    copied.path_file("markers.txt", &markers);
    let settings = copied.settings(1);
    let cancel = AtomicBool::new(false);
    let report = std::thread::scope(|scope| {
        let running = scope.spawn(|| {
            run_suite(
                &copied.suite.join("cancel.suite.yaml"),
                &settings,
                None,
                Some(&cancel),
            )
        });
        let waited = Instant::now();
        while !markers.join("started").exists() && waited.elapsed() < Duration::from_secs(4) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            markers.join("started").exists(),
            "the program never started"
        );
        cancel.store(true, Ordering::SeqCst);
        running.join().unwrap().unwrap()
    });
    let difference = difference(&report, "cancelled");
    assert_eq!(difference.check, "held", "{difference:?}");
    assert_eq!(difference.kind, "run", "{difference:?}");
    assert_eq!(
        difference.reason,
        DifferenceReason::StepFailed,
        "{difference:?}"
    );
    assert_eq!(
        difference.found.as_deref(),
        Some("the suite was cancelled"),
        "{difference:?}"
    );
    assert_ne!(
        fs::read(copied.work(1, "findings.yaml")).ok(),
        Some(decided_bytes())
    );
    assert!(!markers.join("release").exists());
    within_bound(started);
}

/// Test 13: `b10x-loom tree test --confinement local` runs test 1's suite as expected.
#[test]
fn tree_test_runs_a_write_suite() {
    let started = Instant::now();
    let dir = fresh_scratch("write-command");
    let tested = tree_test_in(
        &dir,
        "scratch",
        &Path::new(WRITE).join("decided.suite.yaml"),
        &Path::new(WRITE).join("protocols"),
        &[],
    );
    let stdout = text(&tested.stdout);
    assert_eq!(
        tested.status.code(),
        Some(0),
        "{stdout}{}",
        text(&tested.stderr)
    );
    assert!(
        stdout
            .lines()
            .any(|line| line.starts_with("decided: as expected (")),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
    within_bound(started);
}
