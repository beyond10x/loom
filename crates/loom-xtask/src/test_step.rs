//! The test step of `task check`, held to its budget (`~/beyond10x/AGENTS.md` § Tests stay fast):
//! the step ends within [`STEP_BUDGET`] of wall time and no test runs longer than
//! [`TEST_BUDGET`].
//!
//! It runs `cargo nextest run --workspace --locked` under the default profile of
//! `.config/nextest.toml`, which leaves out the slow lane (`slow_` tests, `task slow`), kills a test
//! at 60 s and writes a JUnit report, then `cargo test --doc --workspace --locked` (nextest runs no
//! doc tests). It reads every test's time from the JUnit report and prints the ten slowest on every
//! run, green or red, then fails when either run failed, a test took longer than
//! [`TEST_BUDGET`], the tests of [`BINARY_BUDGETS`] longer than their summed budget, the tests of
//! [`START_BUDGETS`] started `b10x-loom` more often than their bound (each start a line of the file
//! the budget's variable names, which the step sets on its nextest run), or the step longer than
//! [`STEP_BUDGET`]. Its summary line counts the tests that skipped for a missing
//! prerequisite (a line `skipped:` in their output), and it names each; where a delegated user
//! scope starts it sets [`REQUIRE_DELEGATION`] on its nextest run, so a delegated test that would
//! skip fails instead ([`require_delegation`]). It refuses to start while the test sync switch
//! (`B10X_LOOM_TEST_UNSYNCED_STORE`) is in its own environment. The wall time is taken inside the
//! step, so a wait for a build slot before it starts does not count, and it starts once the test
//! binaries are built (`cargo test --workspace --locked --no-run`, which nextest reuses), so a
//! cold build does not count either; the doc tests' own compilation does. Beside that build it
//! fills the schema cache the `crd` tests of `b10x-loom-resource` read ([`fill_schema_cache`]) and
//! the syntheses the generator tests of `loom-xtask` and `loom-commission-xtask` read
//! ([`fill_synthesis_stand_in`]) and `b10x-loom-commission-testkit` reads
//! ([`fill_plain_synthesis`]), so no test of the step makes one or waits for it. `-p <package>` runs the
//! same over named packages, the doc tests over those of them with a library ([`with_library`]);
//! in a build-pool job nextest runs `SLOT_TOKENS` tests at once.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// The whole step's wall time, doc tests and the warm build included.
pub const STEP_BUDGET: Duration = Duration::from_secs(120);
/// One test's time.
pub const TEST_BUDGET: Duration = Duration::from_secs(60);
/// How many of the slowest tests every run prints.
const SLOWEST: usize = 10;

/// The variable of the test sync switch (`loom_governor::durable`), which the step refuses to
/// start under: only the tests that start `b10x-loom` set it, on that child alone.
const TEST_SWITCH: &str = "B10X_LOOM_TEST_UNSYNCED_STORE";

/// The step's answer to its own environment's value of [`TEST_SWITCH`]: an error naming it when
/// the variable is there, whatever its value.
pub fn refuse_the_switch(value: Option<&std::ffi::OsStr>) -> Result<(), String> {
    match value {
        None => Ok(()),
        Some(value) => Err(format!(
            "test step: {TEST_SWITCH} is set in the step's environment (to {value:?}), so the \
             durability tests would not sync; unset it and start the step again"
        )),
    }
}

/// The variable the tests that need a delegated user scope read: set, a missing prerequisite fails
/// them instead of skipping them (`crates/loom-cli/tests/tree_run/delegated_cgroup.rs`).
pub const REQUIRE_DELEGATION: &str = "LOOM_REQUIRE_DELEGATION";

/// Whether the step sets [`REQUIRE_DELEGATION`] to `1` on its nextest run, and the line it prints
/// about it. A value the caller set stays as it is and `probe` is not run; otherwise the step sets
/// it when `probe` (a delegated user scope starts here) passes, so a delegated test that skips
/// fails the step, and leaves it unset, saying why, when the probe fails.
pub fn require_delegation(
    set: Option<&std::ffi::OsStr>,
    probe: impl FnOnce() -> Result<(), String>,
) -> (bool, String) {
    if let Some(value) = set {
        return (
            false,
            format!("test step: {REQUIRE_DELEGATION}={value:?} as the caller set it"),
        );
    }
    match probe() {
        Ok(()) => (
            true,
            format!(
                "test step: a delegated user scope starts here, so {REQUIRE_DELEGATION}=1: a \
                 delegated test that skips fails"
            ),
        ),
        Err(why) => (
            false,
            format!(
                "test step: {REQUIRE_DELEGATION} not set: no delegated user scope starts here \
                 ({DELEGATION_PROBE}: {why}); the tests that need one skip and are named below"
            ),
        ),
    }
}

/// The probe of a delegated user scope, as the tests that need one start it.
const DELEGATION_PROBE: &str = "systemd-run --user --scope --quiet -p Delegate=yes true";

/// Runs [`DELEGATION_PROBE`]: `Ok` when it exits 0, else what it said.
fn probe_delegation() -> Result<(), String> {
    let output = Command::new("systemd-run")
        .args(["--user", "--scope", "--quiet", "-p", "Delegate=yes", "true"])
        .output()
        .map_err(|e| format!("cannot start: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// The tests of one test module of a package, wherever they run: in a test binary of their own, or
/// as a module of the package's one test binary `main` (`story:one-test-binary-per-crate`), their
/// names then prefixed with the module's.
#[derive(Debug, Clone, Copy)]
pub struct Tests {
    /// The name the step prints for them: `<package>::<module>`.
    pub label: &'static str,
    /// Each `(binary id, name prefix)` that selects them.
    pub layouts: [(&'static str, &'static str); 2],
}

impl Tests {
    /// Whether `case` is one of these tests.
    pub fn holds(&self, case: &Case) -> bool {
        self.layouts
            .iter()
            .any(|(binary, prefix)| case.binary == *binary && case.name.starts_with(prefix))
    }
}

/// The tests of `crates/loom-cli/tests/tree_run.rs`, in either layout.
pub const TREE_RUN: Tests = Tests {
    label: "b10x-loom-cli::tree_run",
    layouts: [
        ("b10x-loom-cli::tree_run", ""),
        ("b10x-loom-cli::main", "tree_run::"),
    ],
};

/// Tests held to a summed time of their own, beside [`TEST_BUDGET`] and [`STEP_BUDGET`]: those of
/// [`TREE_RUN`] (`story:tree-run-tests-fit-the-budget`).
pub const BINARY_BUDGETS: [(Tests, Duration); 1] = [(TREE_RUN, Duration::from_secs(60))];

/// How many processes of `b10x-loom` the tests of a module may start in one run of the step
/// (`story:tree-run-tests-fit-the-test-budget`): a count that does not move with the host's load,
/// as seconds do. Each start appends one line to the file `variable` names, which the step sets
/// for its nextest run.
#[derive(Debug, Clone, Copy)]
pub struct StartBudget {
    pub tests: Tests,
    pub variable: &'static str,
    pub bound: usize,
}

/// The start budgets: [`TREE_RUN`]'s starts, counted by `with_the_switch` in
/// `crates/loom-cli/tests/tree_run.rs`, bounded at the count the story left them at.
pub const START_BUDGETS: [StartBudget; 1] = [StartBudget {
    tests: TREE_RUN,
    variable: "LOOM_TREE_RUN_STARTS",
    bound: 59,
}];

/// The line the step prints about `budget`'s starts and the reason it fails, given the cases of
/// the run and the text of the starts file (`None` when the run wrote none). Nothing when none of
/// the budget's tests ran; a failure when they ran and counted no start, since then the counting
/// is broken.
pub fn start_verdict(
    budget: &StartBudget,
    cases: &[Case],
    starts: Option<&str>,
) -> (String, Option<String>) {
    let label = budget.tests.label;
    if !cases.iter().any(|case| budget.tests.holds(case)) {
        return (String::new(), None);
    }
    let count = starts.map_or(0, |text| text.lines().count());
    let text = format!(
        "{label}: {count} starts of b10x-loom, of {} ({})",
        budget.bound, budget.variable
    );
    let failure = if count == 0 {
        Some(format!(
            "{label} ran and counted no start of b10x-loom in {}: the counting is broken",
            budget.variable
        ))
    } else if count > budget.bound {
        Some(format!(
            "{label} made {count} starts of b10x-loom, over the {} they may make: run the \
             library in process where a test is not about the binary",
            budget.bound
        ))
    } else {
        None
    };
    (text, failure)
}

/// One test case of nextest's JUnit report.
#[derive(Debug, Clone, PartialEq)]
pub struct Case {
    /// `<package>::<binary>`.
    pub binary: String,
    pub name: String,
    pub seconds: f64,
    pub failed: bool,
    /// The test printed a line starting `skipped:` (a missing prerequisite) and passed.
    pub skipped: bool,
}

/// The step over the workspace, or over `packages` when any are named.
pub fn run(root: &Path, packages: &[String]) -> Result<(), String> {
    refuse_the_switch(std::env::var_os(TEST_SWITCH).as_deref())?;
    let mut scope: Vec<&str> = Vec::new();
    if packages.is_empty() {
        scope.push("--workspace");
    }
    for package in packages {
        scope.extend(["-p", package.as_str()]);
    }
    let with = |head: &[&'static str], tail: &[&'static str]| -> Vec<String> {
        head.iter()
            .copied()
            .chain(scope.iter().copied())
            .chain(tail.iter().copied())
            .map(str::to_owned)
            .collect()
    };

    let build_started = Instant::now();
    let runs = |package: &str| packages.is_empty() || packages.iter().any(|p| p == package);
    let mut fills: Vec<(&str, Fill<'_>)> = Vec::new();
    if runs(CRD_PACKAGE) {
        fills.push(("schema cache", Box::new(|| fill_schema_cache(root))));
    }
    if runs(PLAIN_SYNTHESIS_PACKAGE) {
        fills.push(("plain synthesis", Box::new(|| fill_plain_synthesis(root))));
    }
    for (package, specification) in GENERATOR_PACKAGES {
        if runs(package) {
            fills.push((
                "synthesis stand-in",
                Box::new(move || fill_synthesis_stand_in(root, specification)),
            ));
        }
    }
    let (build, built, filled) = std::thread::scope(|scope| {
        let handles: Vec<_> = fills
            .into_iter()
            .map(|(what, fill)| (what, scope.spawn(fill)))
            .collect();
        let build = with(&["test"], &["--locked", "--no-run"]);
        let built = cargo(root, &build);
        let filled: Vec<_> = handles
            .into_iter()
            .map(|(what, handle)| {
                let result = handle
                    .join()
                    .unwrap_or_else(|_| Err(format!("filling the {what} panicked")));
                (what, result)
            })
            .collect();
        (build, built, filled)
    });
    for (what, result) in filled {
        let (path, seconds) = result.map_err(|e| format!("test step: {e}"))?;
        println!(
            "test step: {what} {} ready in {seconds:.1} s, beside the build",
            path.display()
        );
    }
    let built = built?;
    if built != Some(0) {
        return Err(format!(
            "test step: building the test binaries (cargo {}) exited {}",
            build.join(" "),
            describe(built)
        ));
    }
    println!(
        "test step: test binaries built in {:.1} s (not counted in the budget)",
        build_started.elapsed().as_secs_f64()
    );
    if runs(SHARED_TREES.0) {
        let fill_started = Instant::now();
        let fill = with(
            &["test"],
            &[
                "--locked",
                "--test",
                SHARED_TREES.1,
                "--",
                "--ignored",
                "--exact",
                SHARED_TREES.2,
            ],
        );
        let filled = cargo(root, &fill)?;
        if filled != Some(0) {
            return Err(format!(
                "test step: making the shared trees (cargo {}) exited {}",
                fill.join(" "),
                describe(filled)
            ));
        }
        println!(
            "test step: the shared trees of {}::{} made in {:.1} s (not counted in the budget)",
            SHARED_TREES.0,
            SHARED_TREES.1,
            fill_started.elapsed().as_secs_f64()
        );
    }
    let (require, said) = require_delegation(
        std::env::var_os(REQUIRE_DELEGATION).as_deref(),
        probe_delegation,
    );
    println!("{said}");

    let started = Instant::now();
    let report = target_dir(root).join("nextest/default/junit.xml");
    let _ = fs::remove_file(&report);

    let mut nextest = with(&["nextest", "run"], &["--locked", "--no-fail-fast"]);
    let cpus = std::thread::available_parallelism().map_or(0, |n| n.get());
    match slot_tokens() {
        Some(threads) => {
            nextest.push(format!("--test-threads={threads}"));
            println!("test step: {threads} test threads (SLOT_TOKENS) on {cpus} CPUs");
        }
        None => println!("test step: nextest's default test threads on {cpus} CPUs"),
    }
    let mut command = cargo_command(root);
    if require {
        command.env(REQUIRE_DELEGATION, "1");
    }
    let starts: Vec<(StartBudget, PathBuf)> = START_BUDGETS
        .iter()
        .map(|budget| {
            let file = starts_file(root, budget);
            let _ = fs::remove_file(&file);
            if let Some(directory) = file.parent() {
                let _ = fs::create_dir_all(directory);
            }
            (*budget, file)
        })
        .collect();
    for (budget, file) in &starts {
        // A name in a literal, which `only_the_allowed_files_name_or_set_the_switch` (in
        // `crates/loom-governor/src/durable.rs`) reads as not the test sync switch.
        match budget.variable {
            "LOOM_TREE_RUN_STARTS" => command.env("LOOM_TREE_RUN_STARTS", file),
            other => return Err(format!("test step: no start budget counts in {other}")),
        };
    }
    let nextest = command
        .args(&nextest)
        .status()
        .map_err(|e| format!("run cargo {}: {e}", nextest.join(" ")))?
        .code();
    let nextest_seconds = started.elapsed().as_secs_f64();
    let doc_started = Instant::now();
    let doc = if packages.is_empty() {
        cargo(root, &with(&["test", "--doc"], &["--locked"]))?
    } else {
        let libraries = with_library(&metadata(root)?, packages);
        if libraries.is_empty() {
            println!("test step: no named package has a library target, so no doc tests");
            Some(0)
        } else {
            let mut doc: Vec<String> = ["test", "--doc"].map(str::to_owned).to_vec();
            for package in libraries {
                doc.extend(["-p".to_owned(), package]);
            }
            doc.push("--locked".to_owned());
            cargo(root, &doc)?
        }
    };
    let doc_seconds = doc_started.elapsed().as_secs_f64();
    let wall = started.elapsed();

    let cases = match fs::read_to_string(&report) {
        Ok(xml) => parse_junit(&xml),
        Err(error) => {
            return Err(format!(
                "test step: no JUnit report at {} ({error}); nextest {}",
                report.display(),
                describe(nextest)
            ));
        }
    };
    let (mut text, mut failures) =
        verdict(&cases, nextest, doc, wall, nextest_seconds, doc_seconds);
    for (budget, file) in &starts {
        let (line, failure) =
            start_verdict(budget, &cases, fs::read_to_string(file).ok().as_deref());
        if !line.is_empty() {
            text.push('\n');
            text.push_str(&line);
        }
        failures.extend(failure);
    }
    println!("{text}");
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("test step failed:\n  {}", failures.join("\n  ")))
    }
}

/// The summary every run prints, and the reasons the step fails (none when it passes).
pub fn verdict(
    cases: &[Case],
    nextest: Option<i32>,
    doc: Option<i32>,
    wall: Duration,
    nextest_seconds: f64,
    doc_seconds: f64,
) -> (String, Vec<String>) {
    let mut sorted: Vec<&Case> = cases.iter().collect();
    sorted.sort_by(|a, b| b.seconds.total_cmp(&a.seconds));
    let binaries: std::collections::BTreeSet<&str> =
        cases.iter().map(|case| case.binary.as_str()).collect();
    let skipped: Vec<&Case> = cases.iter().filter(|case| case.skipped).collect();
    let mut text = format!(
        "test step: {} test(s) in {} binar{} by nextest ({nextest_seconds:.1} s), {} skipped, \
         doc tests ({doc_seconds:.1} s); wall time {:.1} s of {} s\nslowest {SLOWEST} test(s):",
        cases.len(),
        binaries.len(),
        if binaries.len() == 1 { "y" } else { "ies" },
        skipped.len(),
        wall.as_secs_f64(),
        STEP_BUDGET.as_secs(),
    );
    for case in sorted.iter().take(SLOWEST) {
        text.push_str(&format!(
            "\n  {:>7.2} s  {} {}{}",
            case.seconds,
            case.binary,
            case.name,
            if case.failed { "  FAILED" } else { "" }
        ));
    }
    if !skipped.is_empty() {
        text.push_str(&format!(
            "\nskipped {} test(s), a prerequisite missing:",
            skipped.len()
        ));
        for case in &skipped {
            text.push_str(&format!("\n  {} {}", case.binary, case.name));
        }
    }

    let mut failures = Vec::new();
    for (tests, budget) in BINARY_BUDGETS {
        let held: Vec<&Case> = cases.iter().filter(|case| tests.holds(case)).collect();
        if held.is_empty() {
            continue;
        }
        let label = tests.label;
        let summed: f64 = held.iter().map(|case| case.seconds).sum();
        text.push_str(&format!(
            "\n{label}: {summed:.1} s summed over {} test(s), of {} s",
            held.len(),
            budget.as_secs()
        ));
        if summed > budget.as_secs_f64() {
            failures.push(format!(
                "{label} ran {summed:.1} s summed over its {} test(s), over the {} s they may \
                 take together",
                held.len(),
                budget.as_secs()
            ));
        }
    }
    if nextest != Some(0) {
        failures.push(format!("cargo nextest run exited {}", describe(nextest)));
    }
    if doc != Some(0) {
        failures.push(format!("cargo test --doc exited {}", describe(doc)));
    }
    for case in &sorted {
        if case.seconds > TEST_BUDGET.as_secs_f64() {
            failures.push(format!(
                "{} {} ran {:.1} s, over the {} s a test may take: make it fast, or move it to the \
                 slow lane as `slow_{}`",
                case.binary,
                case.name,
                case.seconds,
                TEST_BUDGET.as_secs(),
                case.name
            ));
        }
    }
    if wall > STEP_BUDGET {
        failures.push(format!(
            "the step ran {:.1} s, over its budget of {} s",
            wall.as_secs_f64(),
            STEP_BUDGET.as_secs()
        ));
    }
    (text, failures)
}

/// The package, test binary and ignored test that make the trees the tests of
/// `b10x-loom-cli::tree_run` share (`crates/loom-cli/tests/tree_run.rs`): run after the build and
/// before the timed run, so no test of the step builds one or waits on the lock of another that
/// does, a wait each waiter counted in [`BINARY_BUDGETS`].
const SHARED_TREES: (&str, &str, &str) = ("b10x-loom-cli", "tree_run", "the_shared_trees_are_made");

/// The package whose `crd` tests read the schema cache.
const CRD_PACKAGE: &str = "b10x-loom-resource";

/// The package whose tests read the plain synthesis of `ess/commission`.
const PLAIN_SYNTHESIS_PACKAGE: &str = "b10x-loom-commission-testkit";

/// A fixture filled beside the build: the entry it made and the seconds it took.
type Fill<'a> = Box<dyn FnOnce() -> Result<(PathBuf, f64), String> + Send + 'a>;

/// The generators whose tests run their synthesis on a stand-in `ess`, and the specification
/// each synthesizes (`ess_suite::GENERATED_SPECIFICATIONS`).
const GENERATOR_PACKAGES: [(&str, &str); 2] = [
    ("loom-xtask", crate::ess_suite::GENERATED_SPECIFICATIONS[0]),
    (
        "loom-commission-xtask",
        crate::ess_suite::GENERATED_SPECIFICATIONS[1],
    ),
];

/// Fills the synthesis the generator tests' stand-in `ess` answers with
/// (`ess_suite::synthesis_stand_in` over `<root>/<specification>`, seconds each), under the
/// directory and key those tests use, so that no test synthesizes or waits on one that does.
fn fill_synthesis_stand_in(root: &Path, specification: &str) -> Result<(PathBuf, f64), String> {
    let started = Instant::now();
    let made = crate::ess_suite::synthesis_stand_in(
        &crate::ess_suite::ess_program(root),
        &root.join(specification),
        &crate::ess_suite::RUST_CRATE,
        &target_dir(root)
            .join("tmp")
            .join(crate::ess_suite::STAND_IN_CACHE),
    )?;
    Ok((made, started.elapsed().as_secs_f64()))
}

/// Fills the plain synthesis of `ess/commission` that `b10x-loom-commission-testkit`'s tests read
/// (`ess verify conform synthesize` with no other option, seconds each time) under the same
/// directory and key, so no test of the step makes it or waits for it.
fn fill_plain_synthesis(root: &Path) -> Result<(PathBuf, f64), String> {
    let started = Instant::now();
    let made = crate::ess_suite::plain_synthesis_once(
        &crate::ess_suite::ess_program(root),
        root,
        &root.join(crate::ess_suite::PLAIN_SYNTHESIS_SPECIFICATION),
        &target_dir(root)
            .join("tmp")
            .join(crate::ess_suite::PLAIN_SYNTHESIS_CACHE),
    )?;
    Ok((made, started.elapsed().as_secs_f64()))
}

/// Fills the cache `crates/loom-resource/tests/crd.rs` reads (`ess generate --kind schema` over
/// `ess/`, about 16 s) under the same directory and key, so that no test process of the step
/// generates it or waits on another that does: the test finds it made. Returns the entry and the
/// seconds it took (none to speak of when it was there).
fn fill_schema_cache(root: &Path) -> Result<(PathBuf, f64), String> {
    let started = Instant::now();
    let program = crate::ess_suite::ess_program(root);
    let version = crate::ess_suite::ess_version(&program, Some(root))?;
    let inputs = crate::ess_suite::inputs_digest(&root.join("ess"), &version)?;
    let key = format!("schema-{}", inputs.trim_start_matches("sha256:"));
    let cache = target_dir(root).join("tmp/ess-schema-cache");
    let made = crate::ess_suite::made_once(&cache, &key, |out| {
        let output = program
            .command()
            .current_dir(root)
            .args(["generate", "--path", "ess", "--kind", "schema", "--out"])
            .arg(out)
            .output()
            .map_err(|e| format!("run `ess generate --kind schema` (is ess on PATH?): {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "`ess generate --kind schema` exited {}:\n{}{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    })?;
    Ok((made, started.elapsed().as_secs_f64()))
}

/// The file `budget`'s starts are counted in, under the build directory beside nextest's report.
fn starts_file(root: &Path, budget: &StartBudget) -> PathBuf {
    target_dir(root)
        .join("nextest")
        .join(format!("{}.starts", budget.variable.to_lowercase()))
}

fn describe(code: Option<i32>) -> String {
    code.map_or_else(|| "by a signal".to_owned(), |code| code.to_string())
}

/// The slots the build pool granted this job (`SLOT_TOKENS`), so nextest runs as many tests at
/// once as the pool counts; `None` leaves nextest's default, one a CPU.
fn slot_tokens() -> Option<u32> {
    std::env::var("SLOT_TOKENS")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .filter(|&tokens| tokens > 0)
}

pub(crate) fn target_dir(root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"))
}

/// Runs the `cargo` on `PATH` in `root`, its output passed through, and returns its exit code.
///
/// With `ESS_TOOLCHAIN` the release `ess/ess-inputs.yaml` requires when the caller set none, as
/// the Taskfiles do, so a test that starts `ess` runs the pin under a newer dispatcher.
/// Without the variables `cargo run` set for this program: a build script that reruns when one
/// changes (ring's on `CARGO_MANIFEST_DIR` and `CARGO_PKG_*`) would otherwise rebuild its crate and
/// everything above it here, after `task ess-gate` and `task conform` built them without.
fn cargo(root: &Path, args: &[String]) -> Result<Option<i32>, String> {
    let status = cargo_command(root)
        .args(args)
        .status()
        .map_err(|e| format!("run cargo {}: {e}", args.join(" ")))?;
    Ok(status.code())
}

/// The `cargo` on `PATH`, to run in `root`, as [`cargo`] starts it.
pub(crate) fn cargo_command(root: &Path) -> Command {
    let mut command = Command::new("cargo");
    if std::env::var_os("ESS_TOOLCHAIN").is_none_or(|value| value.is_empty())
        && let Some(release) = crate::ess_suite::pinned_release(root)
    {
        command.env("ESS_TOOLCHAIN", release);
    }
    for (name, _) in std::env::vars_os() {
        let Some(name) = name.to_str() else { continue };
        if name.starts_with("CARGO_PKG_")
            || matches!(
                name,
                "CARGO_MANIFEST_DIR"
                    | "CARGO_MANIFEST_PATH"
                    | "CARGO_CRATE_NAME"
                    | "CARGO_BIN_NAME"
                    | "CARGO_PRIMARY_PACKAGE"
            )
        {
            command.env_remove(name);
        }
    }
    command.current_dir(root);
    command
}

/// The workspace's own packages, as `cargo metadata --no-deps` gives them.
fn metadata(root: &Path) -> Result<serde_json::Value, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1", "--locked"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("run cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "test step: cargo metadata exited {}: {}",
            describe(output.status.code()),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| format!("test step: cargo metadata: {e}"))
}

/// The packages of `packages` with a target `cargo test --doc` tests (a library or a
/// proc-macro), in the order named: `cargo test --doc` over a package with binaries only fails
/// with "no library targets found".
fn with_library(metadata: &serde_json::Value, packages: &[String]) -> Vec<String> {
    let documented = |name: &str| {
        metadata["packages"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|package| {
                package["name"] == name
                    && package["targets"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|target| {
                            target["kind"].as_array().into_iter().flatten().any(|kind| {
                                matches!(kind.as_str(), Some("lib" | "rlib" | "proc-macro"))
                            })
                        })
            })
    };
    packages
        .iter()
        .filter(|package| documented(package))
        .cloned()
        .collect()
}

/// Every `<testcase>` of a JUnit report: its `classname` (the binary), `name`, `time`, and whether
/// it holds a `<failure>` or `<error>`.
pub fn parse_junit(xml: &str) -> Vec<Case> {
    let mut cases = Vec::new();
    let mut rest = xml;
    while let Some(at) = rest.find("<testcase ") {
        rest = &rest[at + "<testcase ".len()..];
        let Some(end) = rest.find('>') else { break };
        let head = &rest[..end];
        let closed = head.ends_with('/');
        let body = if closed {
            ""
        } else {
            rest.find("</testcase>")
                .map_or("", |close| &rest[end..close])
        };
        let attribute = |key: &str| -> String {
            let needle = format!("{key}=\"");
            head.find(&needle)
                .map(|start| {
                    let value = &head[start + needle.len()..];
                    unescape(&value[..value.find('"').unwrap_or(value.len())])
                })
                .unwrap_or_default()
        };
        let failed = body.contains("<failure") || body.contains("<error");
        cases.push(Case {
            binary: attribute("classname"),
            name: attribute("name"),
            seconds: attribute("time").parse().unwrap_or(0.0),
            failed,
            skipped: !failed && printed_a_skip(body),
        });
    }
    cases
}

/// Whether the output a `<testcase>` holds (its `<system-out>` and `<system-err>`, stored for a
/// passing test by `store-success-output` in `.config/nextest.toml`) has a line that starts with
/// `skipped:`, as a test that skips for a missing prerequisite prints.
fn printed_a_skip(body: &str) -> bool {
    let text = unescape(body)
        .replace("<system-out>", "\n")
        .replace("<system-err>", "\n")
        .replace("<![CDATA[", "\n");
    text.lines()
        .any(|line| line.trim_start().starts_with("skipped:"))
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="3" failures="1" errors="0" uuid="u" timestamp="t" time="70.2">
    <testsuite name="b10x-loom-executor::ess_gate" tests="2" disabled="0" errors="0" failures="1">
        <testcase name="gate_holds_on_the_specification" classname="b10x-loom-executor::ess_gate" timestamp="t" time="1.250">
        </testcase>
        <testcase name="a_slow_one" classname="b10x-loom-executor::ess_gate" timestamp="t" time="60.004">
            <failure type="test failure">timed out</failure>
        </testcase>
    </testsuite>
    <testsuite name="loom-xtask::checks" tests="1" disabled="0" errors="0" failures="0">
        <testcase name="a &amp; b" classname="loom-xtask::checks" timestamp="t" time="0.010"/>
    </testsuite>
</testsuites>
"#;

    #[test]
    fn the_doc_tests_run_over_the_named_packages_with_a_library_only() {
        let metadata = serde_json::json!({"packages": [
            {"name": "a-lib", "targets": [{"kind": ["lib"]}, {"kind": ["test"]}]},
            {"name": "a-tool", "targets": [{"kind": ["bin"]}, {"kind": ["test"]}]},
            {"name": "a-macro", "targets": [{"kind": ["proc-macro"]}]},
            {"name": "both", "targets": [{"kind": ["bin"]}, {"kind": ["rlib"]}]},
        ]});
        let named = |names: &[&str]| names.iter().map(|&n| n.to_owned()).collect::<Vec<_>>();
        assert_eq!(
            with_library(&metadata, &named(&["a-tool", "both", "a-lib", "a-macro"])),
            named(&["both", "a-lib", "a-macro"])
        );
        assert!(with_library(&metadata, &named(&["a-tool"])).is_empty());
        assert!(with_library(&metadata, &named(&["not-a-member"])).is_empty());
    }

    #[test]
    fn the_report_gives_each_test_its_binary_time_and_failure() {
        let cases = parse_junit(REPORT);
        assert_eq!(
            cases,
            vec![
                Case {
                    binary: "b10x-loom-executor::ess_gate".to_owned(),
                    name: "gate_holds_on_the_specification".to_owned(),
                    seconds: 1.25,
                    failed: false,
                    skipped: false,
                },
                Case {
                    binary: "b10x-loom-executor::ess_gate".to_owned(),
                    name: "a_slow_one".to_owned(),
                    seconds: 60.004,
                    failed: true,
                    skipped: false,
                },
                Case {
                    binary: "loom-xtask::checks".to_owned(),
                    name: "a & b".to_owned(),
                    seconds: 0.01,
                    failed: false,
                    skipped: false,
                },
            ]
        );
    }

    #[test]
    fn a_test_over_60_s_fails_the_step_and_the_slowest_are_printed() {
        let cases = parse_junit(REPORT);
        let (text, failures) = verdict(
            &cases,
            Some(0),
            Some(0),
            Duration::from_secs(80),
            70.0,
            10.0,
        );
        assert!(
            text.contains("3 test(s) in 2 binaries")
                && text.contains("slowest 10 test(s):\n    60.00 s  b10x-loom-executor::ess_gate a_slow_one  FAILED\n     1.25 s"),
            "{text}"
        );
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(
            failures[0].contains("a_slow_one ran 60.0 s"),
            "{failures:?}"
        );
    }

    #[test]
    fn the_test_step_refuses_the_switch_in_its_environment() {
        assert_eq!(refuse_the_switch(None), Ok(()));
        for value in ["1", "0", ""] {
            let refused = refuse_the_switch(Some(std::ffi::OsStr::new(value)))
                .expect_err("the step refuses to start under the switch");
            assert!(
                refused.contains("B10X_LOOM_TEST_UNSYNCED_STORE"),
                "{value:?}: {refused}"
            );
        }
    }

    /// A report whose `tree_run` tests take `seconds` each, beside one test of another binary.
    fn tree_run_report(seconds: &[f64]) -> String {
        tree_run_report_in("b10x-loom-cli::tree_run", "", seconds)
    }

    /// A report whose `tree_run` tests take `seconds` each in the test binary `binary`, each named
    /// `<prefix>case_<n>`, beside one test of another binary and, in `binary`, one test of another
    /// module that takes 50 s.
    fn tree_run_report_in(binary: &str, prefix: &str, seconds: &[f64]) -> String {
        let mut xml = format!(
            r#"<testsuites name="nextest-run">
    <testsuite name="{binary}" tests="2">
"#
        );
        for (n, time) in seconds.iter().enumerate() {
            xml.push_str(&format!(
                "        <testcase name=\"{prefix}case_{n}\" classname=\"{binary}\" \
                 timestamp=\"t\" time=\"{time:.3}\"/>\n"
            ));
        }
        if !prefix.is_empty() {
            xml.push_str(&format!(
                "        <testcase name=\"run_events::other\" classname=\"{binary}\" \
                 timestamp=\"t\" time=\"50.000\"/>\n"
            ));
        }
        xml.push_str(
            r#"    </testsuite>
    <testsuite name="loom-xtask::checks" tests="1">
        <testcase name="other" classname="loom-xtask::checks" timestamp="t" time="50.000"/>
    </testsuite>
</testsuites>
"#,
        );
        xml
    }

    /// A starts file of one line over the bound fails the step, naming `tree_run`, the count and
    /// the bound; one at the bound passes; and tests of the module that ran with no starts file
    /// fail it, since then nothing was counted.
    #[test]
    fn a_module_over_its_start_budget_fails_the_step() {
        let budget = &START_BUDGETS[0];
        assert_eq!(budget.variable, "LOOM_TREE_RUN_STARTS");
        let ran = parse_junit(&tree_run_report(&[1.0, 2.0]));
        let lines =
            |count: usize| -> String { (0..count).map(|n| format!("case_{}\n", n % 2)).collect() };
        let over = budget.bound + 1;
        let (text, failure) = start_verdict(budget, &ran, Some(&lines(over)));
        let failure = failure.expect("a start over the bound fails the step");
        assert!(
            failure.contains("tree_run")
                && failure.contains(&format!("{over} starts"))
                && failure.contains(&format!("{} ", budget.bound)),
            "{failure}"
        );
        assert!(
            text.contains("tree_run") && text.contains(&format!("{over} starts")),
            "{text}"
        );

        let (text, failure) = start_verdict(budget, &ran, Some(&lines(budget.bound)));
        assert_eq!(failure, None, "{text}");
        assert!(text.contains(&format!("{} starts", budget.bound)), "{text}");

        let (_, failure) = start_verdict(budget, &ran, None);
        assert!(
            failure.is_some_and(|failure| failure.contains("no start")),
            "tests that ran counting nothing pass"
        );
        let other = parse_junit(REPORT);
        let (text, failure) = start_verdict(budget, &other, None);
        assert_eq!((text.as_str(), failure), ("", None));
    }

    /// The `tree_run` tests are held to their summed budget in both layouts of
    /// `story:one-test-binary-per-crate`: as the binary `b10x-loom-cli::tree_run`, and as the
    /// module `tree_run` of `b10x-loom-cli::main`, its tests named `tree_run::…`. Another module of
    /// `main` is not counted with them.
    #[test]
    fn the_budgets_select_tree_run_in_its_own_binary_and_in_main() {
        for (binary, prefix) in [
            ("b10x-loom-cli::tree_run", ""),
            ("b10x-loom-cli::main", "tree_run::"),
        ] {
            let over = parse_junit(&tree_run_report_in(binary, prefix, &[30.5, 30.5]));
            let (text, failures) =
                verdict(&over, Some(0), Some(0), Duration::from_secs(70), 1.0, 1.0);
            assert_eq!(failures.len(), 1, "{binary}: {failures:?}");
            assert!(
                failures[0].contains("tree_run")
                    && failures[0].contains("61.0 s")
                    && failures[0].contains("60 s"),
                "{binary}: {failures:?}"
            );
            assert!(
                text.contains("61.0 s summed over 2 test(s)"),
                "{binary}: {text}"
            );

            let within = parse_junit(&tree_run_report_in(binary, prefix, &[30.0, 29.0]));
            let (text, failures) =
                verdict(&within, Some(0), Some(0), Duration::from_secs(70), 1.0, 1.0);
            assert!(failures.is_empty(), "{binary}: {failures:?}");
            assert!(
                text.contains("59.0 s summed over 2 test(s)"),
                "{binary}: {text}"
            );
        }
    }

    /// The filter of the override that kills a `tree_run` test at 20 s: the binary while it is its
    /// own, or its module in `b10x-loom-cli::main`; never a `slow_` test, since the default
    /// profile's overrides apply to the slow profile too.
    const TREE_RUN_FILTER: &str = "(binary_id(b10x-loom-cli::tree_run) or \
        (binary_id(b10x-loom-cli::main) and test(/^tree_run::/))) and not test(/(^|::)slow_/)";

    #[test]
    fn the_tree_run_override_holds_in_both_layouts() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let config: toml::Value =
            toml::from_str(&fs::read_to_string(root.join(".config/nextest.toml")).unwrap())
                .unwrap();
        let overrides = config["profile"]["default"]["overrides"]
            .as_array()
            .unwrap();
        let held: Vec<&toml::Value> = overrides
            .iter()
            .filter(|table| table["filter"].as_str() == Some(TREE_RUN_FILTER))
            .collect();
        assert_eq!(
            held.len(),
            1,
            "one override has the filter {TREE_RUN_FILTER:?}: {overrides:?}"
        );
        let limit = &held[0]["slow-timeout"];
        assert_eq!(limit["period"].as_str(), Some("20s"), "{limit:?}");
        assert_eq!(limit["terminate-after"].as_integer(), Some(1), "{limit:?}");
    }

    #[test]
    fn a_binary_over_its_budget_fails_the_step() {
        let over = parse_junit(&tree_run_report(&[30.5, 30.5]));
        let (text, failures) = verdict(&over, Some(0), Some(0), Duration::from_secs(70), 1.0, 1.0);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(
            failures[0].contains("b10x-loom-cli::tree_run")
                && failures[0].contains("61.0 s")
                && failures[0].contains("60 s"),
            "{failures:?}"
        );
        assert!(
            text.contains("b10x-loom-cli::tree_run: 61.0 s summed over 2 test(s)"),
            "{text}"
        );

        let within = parse_junit(&tree_run_report(&[30.0, 29.0]));
        let (_, failures) = verdict(&within, Some(0), Some(0), Duration::from_secs(70), 1.0, 1.0);
        assert!(failures.is_empty(), "{failures:?}");
    }

    /// A report holding one passing test that printed a skip, one that did not, and one whose
    /// output only mentions the word.
    const SKIPPING: &str = r#"<testsuites name="nextest-run">
    <testsuite name="b10x-loom-cli::tree_run" tests="3">
        <testcase name="delegated_cgroup::a_held_root" classname="b10x-loom-cli::tree_run" timestamp="t" time="0.010">
            <system-out></system-out>
            <system-err>skipped: missing prerequisite: a user systemd manager that starts a delegated scope
</system-err>
        </testcase>
        <testcase name="a_one_case_tree" classname="b10x-loom-cli::tree_run" timestamp="t" time="1.500">
            <system-out>
running 1 test
</system-out>
        </testcase>
        <testcase name="mentions" classname="b10x-loom-cli::tree_run" timestamp="t" time="0.200">
            <system-err>the word skipped: inside a line is no skip
</system-err>
        </testcase>
    </testsuite>
</testsuites>
"#;

    #[test]
    fn the_summary_line_counts_the_skipped_tests_and_names_each() {
        let cases = parse_junit(SKIPPING);
        let (text, failures) = verdict(&cases, Some(0), Some(0), Duration::from_secs(5), 1.0, 1.0);
        assert!(failures.is_empty(), "{failures:?}");
        let summary = text.lines().next().unwrap();
        assert!(
            summary.contains("3 test(s) in 1 binary") && summary.contains("1 skipped"),
            "{text}"
        );
        assert!(
            text.contains(
                "skipped 1 test(s), a prerequisite missing:\n  b10x-loom-cli::tree_run delegated_cgroup::a_held_root"
            ),
            "{text}"
        );
        let skipped: Vec<&str> = cases
            .iter()
            .filter(|case| case.skipped)
            .map(|case| case.name.as_str())
            .collect();
        assert_eq!(skipped, ["delegated_cgroup::a_held_root"], "{text}");

        let (text, _) = verdict(
            &parse_junit(REPORT),
            Some(0),
            Some(0),
            Duration::from_secs(5),
            1.0,
            1.0,
        );
        assert!(
            text.lines().next().unwrap().contains("0 skipped") && !text.contains("skipped 0"),
            "{text}"
        );
    }

    /// The commands of a task that take minutes in one build-pool job: the synthesis of `ess/`
    /// (`suite-generate`, `suite-check`, 534.5 s on 2026-11-21) and the slow lane's tests.
    const LONG_COMMANDS: [&str; 3] = ["suite-generate", "suite-check", "--profile slow"];

    /// The longest a build-pool job may run when it names its time (`slot --timeout`,
    /// `SLOT_TIMEOUT`); without a name it is ended at 10 minutes.
    const POOL_MOST: Duration = Duration::from_secs(30 * 60);

    /// Every task that runs a command of [`LONG_COMMANDS`] names the pool time of 30 minutes, and
    /// the slow profile kills one test at least 10 minutes before the pool would end its job.
    #[test]
    fn a_task_that_takes_minutes_names_its_pool_time_and_a_slow_test_ends_before_it() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let taskfile: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&fs::read_to_string(root.join("Taskfile.yml")).unwrap())
                .unwrap();
        let tasks = taskfile["tasks"].as_mapping().unwrap();
        let mut long = Vec::new();
        for (name, task) in tasks {
            let name = name.as_str().unwrap();
            let commands = task["cmds"].as_sequence().cloned().unwrap_or_default();
            let runs_long = commands.iter().any(|command| {
                command
                    .as_str()
                    .is_some_and(|line| LONG_COMMANDS.iter().any(|long| line.contains(long)))
            });
            if runs_long {
                long.push(name.to_owned());
                assert_eq!(
                    task["env"]["SLOT_TIMEOUT"].as_str(),
                    Some("30m"),
                    "task {name} runs a command that takes minutes and names no pool time of 30m"
                );
            }
        }
        assert_eq!(long, ["suite-generate", "slow"], "{long:?}");

        let config: toml::Value =
            toml::from_str(&fs::read_to_string(root.join(".config/nextest.toml")).unwrap())
                .unwrap();
        let limit = &config["profile"]["slow"]["slow-timeout"];
        let period: u64 = limit["period"]
            .as_str()
            .and_then(|period| period.strip_suffix('s'))
            .and_then(|seconds| seconds.parse().ok())
            .unwrap();
        let terminate = u64::try_from(limit["terminate-after"].as_integer().unwrap()).unwrap();
        let per_test = Duration::from_secs(period * terminate);
        assert!(
            per_test + Duration::from_secs(10 * 60) <= POOL_MOST,
            "the slow profile lets one test run {per_test:?}, too close to the pool's {POOL_MOST:?}"
        );
    }

    #[test]
    fn the_step_requires_delegation_where_a_delegated_scope_starts() {
        let passes = || Ok(());
        let fails = || Err("Failed to connect to user scope bus".to_owned());
        let (set, said) = require_delegation(None, passes);
        assert!(set, "{said}");
        assert!(said.contains("LOOM_REQUIRE_DELEGATION=1"), "{said}");
        let (set, said) = require_delegation(None, fails);
        assert!(!set, "{said}");
        assert!(
            said.contains("not set") && said.contains("Failed to connect to user scope bus"),
            "{said}"
        );
        let (set, said) = require_delegation(Some(std::ffi::OsStr::new("1")), || {
            panic!("the probe runs only when the caller set nothing")
        });
        assert!(!set, "the caller's value stays: {said}");
        assert!(said.contains("as the caller set it"), "{said}");
    }

    #[test]
    fn a_step_over_120_s_fails_and_a_red_run_fails() {
        let fast: Vec<Case> = parse_junit(REPORT)
            .into_iter()
            .filter(|case| case.seconds < 60.0)
            .collect();
        let (_, failures) = verdict(&fast, Some(0), Some(0), Duration::from_secs(119), 1.0, 1.0);
        assert!(failures.is_empty(), "{failures:?}");
        let (_, failures) = verdict(&fast, Some(0), Some(0), Duration::from_secs(121), 1.0, 1.0);
        assert!(
            failures.len() == 1 && failures[0].contains("over its budget of 120 s"),
            "{failures:?}"
        );
        let (text, failures) = verdict(
            &fast,
            Some(100),
            Some(101),
            Duration::from_secs(1),
            1.0,
            1.0,
        );
        assert!(text.contains("slowest 10 test(s):"), "{text}");
        assert_eq!(
            failures,
            vec![
                "cargo nextest run exited 100".to_owned(),
                "cargo test --doc exited 101".to_owned()
            ]
        );
    }
}
