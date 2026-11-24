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
//! [`STEP_BUDGET`], or its report holds no case of a test of [`REQUIRED`] (the conformance suites and
//! the dependency guard, which `task check` runs only here: [`left_out`]), whatever way the run left
//! it out. Its summary line counts the tests that skipped for a missing
//! prerequisite (a line `skipped:` in their output), and it names each; where the delegated tests
//! can run (`loom-xtask delegation-probe`, [`crate::delegation`]) it sets [`REQUIRE_DELEGATION`]
//! on its nextest run, so a delegated test that would skip fails instead ([`require_delegation`]). It refuses to start while the test sync switch
//! (`B10X_LOOM_TEST_UNSYNCED_STORE`) is in its own environment. The wall time is taken inside the
//! step, so a wait for a build slot before it starts does not count, and it starts once the test
//! binaries are built (`cargo test --workspace --locked --no-run`, which nextest reuses), so a
//! cold build does not count either; the doc tests' own compilation does. Beside that build it
//! fills the schema cache the `crd` tests of `b10x-loom-resource` read ([`fill_schema_cache`]) and
//! the syntheses the generator tests of `loom-xtask` and `loom-commission-xtask` read
//! ([`fill_synthesis_stand_in`]) and `b10x-loom-commission-testkit` reads
//! ([`fill_plain_synthesis`]), so no test of the step makes one or waits for it. After the build it
//! builds what is missing of the trees the `tree_run` tests share (`crate::shared_trees::make`, not
//! counted) and gives its nextest run the scope as `LOOM_TEST_SCOPE`, so nextest's setup script
//! computes the same key and finds them; it prints both times, and a start of `b10x-loom` the timed
//! run counts in a build of a shared tree ([`FIXTURE_LINE`]) fails it. `-p <package>` runs the
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
/// it when `probe` ([`DELEGATION_PROBE`]: the delegated tests can run here) passes, so a
/// delegated test that skips fails the step, and leaves it unset, saying why, when the probe fails.
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
                "test step: {DELEGATION_PROBE} passed, so {REQUIRE_DELEGATION}=1: a delegated \
                 test that skips fails"
            ),
        ),
        Err(why) => (
            false,
            format!(
                "test step: {REQUIRE_DELEGATION} not set: the delegated tests cannot run here \
                 ({DELEGATION_PROBE}: {why}); they skip and are named below"
            ),
        ),
    }
}

/// The one probe of what the delegated tests need ([`crate::delegation`]), as CI runs it too.
const DELEGATION_PROBE: &str = "loom-xtask delegation-probe";

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
    bound: 66,
}];

/// The start of a line of a starts file written while a test built a shared tree, before the
/// tree's name (`fixture:ended`), in place of the test's name
/// (`story:tree-run-binary-fits-its-start-bound`).
pub const FIXTURE_LINE: &str = "fixture:";

/// The line the step prints about `budget`'s starts and the reason it fails, given the cases of
/// the run and the text of the starts file (`None` when the run wrote none). Nothing when none of
/// the budget's tests ran; a failure when they ran and counted no start, since then the counting
/// is broken; a failure when a test of the run built a shared tree ([`FIXTURE_LINE`]), which the
/// setup builds before the run, naming each tree and its starts.
pub fn start_verdict(
    budget: &StartBudget,
    cases: &[Case],
    starts: Option<&str>,
) -> (String, Option<String>) {
    let label = budget.tests.label;
    if !cases.iter().any(|case| budget.tests.holds(case)) {
        return (String::new(), None);
    }
    let lines: Vec<&str> = starts.map_or_else(Vec::new, |text| text.lines().collect());
    let count = lines.len();
    let mut built = std::collections::BTreeMap::<&str, usize>::new();
    for tree in lines
        .iter()
        .filter_map(|line| line.strip_prefix(FIXTURE_LINE))
    {
        *built.entry(tree).or_default() += 1;
    }
    let built: Vec<String> = built
        .iter()
        .map(|(tree, starts)| format!("{tree} ({starts})"))
        .collect();
    let mut text = format!(
        "{label}: {count} starts of b10x-loom, of {} ({})",
        budget.bound, budget.variable
    );
    if !built.is_empty() {
        text.push_str(&format!(
            ", in builds of shared trees: {}",
            built.join(", ")
        ));
    }
    let mut failures = Vec::new();
    if count == 0 {
        failures.push(format!(
            "{label} ran and counted no start of b10x-loom in {}: the counting is broken",
            budget.variable
        ));
    }
    if !built.is_empty() {
        failures.push(format!(
            "{label} built the shared tree(s) {} in the timed run: the setup (`loom-xtask \
             shared-trees`) builds them before it, and a test reads them",
            built.join(", ")
        ));
    }
    if count > budget.bound {
        failures.push(format!(
            "{label} made {count} starts of b10x-loom, over the {} they may make: run the \
             library in process where a test is not about the binary",
            budget.bound
        ));
    }
    let failure = (!failures.is_empty()).then(|| failures.join("; "));
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

/// A test binary `task check` runs only in the step (`story:ci-job-time-budget`), and the tests of
/// it the step's report must hold.
#[derive(Debug, Clone, Copy)]
pub struct Required {
    /// `<package>::<binary>`, as the report names it.
    pub binary: &'static str,
    /// The tests of the binary that make its claim.
    pub tests: &'static [&'static str],
}

/// The test binaries of the tasks `task check` no longer runs (`conform`, `commission:conform`,
/// `intake-conform`, `commission:deps-guard`): the step fails unless its report holds a case of
/// each test named, whatever way the run left it out (a filter, a package selection, an ignored
/// test, a target switched off).
pub const REQUIRED: &[Required] = &[
    Required {
        binary: "b10x-loom-conformance::conform",
        tests: &["ess_conformance_report"],
    },
    Required {
        binary: "b10x-loom-commission-conformance::conform",
        tests: &["ess_conformance_report"],
    },
    Required {
        binary: "b10x-loom-intake-conformance::conform",
        tests: &["ess_conformance_report"],
    },
    Required {
        binary: "b10x-loom-commission-testkit::executor_port",
        tests: &[
            "executor_port_contract",
            "commission_depends_on_no_connectors_crate",
        ],
    },
];

/// What the step's report lacks of [`REQUIRED`], one line each, for a run over the workspace
/// (`packages` empty) or over `packages`, whose binaries only are required.
pub fn left_out(cases: &[Case], packages: &[String]) -> Vec<String> {
    let mut lacking = Vec::new();
    for required in REQUIRED {
        let package = required.binary.split("::").next().unwrap_or_default();
        if !packages.is_empty() && !packages.iter().any(|named| named == package) {
            continue;
        }
        for test in required.tests {
            let ran = cases
                .iter()
                .any(|case| case.binary == required.binary && case.name == *test);
            if !ran {
                lacking.push(format!(
                    "{} {test} is not in the report: the step left it out (a filter, a package \
                     selection, an ignore or a target switched off), and `task check` runs it \
                     nowhere else",
                    required.binary
                ));
            }
        }
    }
    lacking
}

/// How many tests of [`REQUIRED`] a run over `packages` (the workspace when none) requires.
fn required_count(packages: &[String]) -> usize {
    REQUIRED
        .iter()
        .filter(|required| {
            let package = required.binary.split("::").next().unwrap_or_default();
            packages.is_empty() || packages.iter().any(|named| named == package)
        })
        .map(|required| required.tests.len())
        .sum()
}

/// The test binaries `.config/nextest.toml`'s default filter leaves out: each has a task of its
/// own in `task check`.
#[cfg(test)]
pub const OUT_OF_THE_STEP: [&str; 2] = [
    "b10x-loom-executor::ess_gate",
    "b10x-loom-commission::ess_gate",
];

/// The slow lane's test selector in the default filter.
#[cfg(test)]
const SLOW_SELECTOR: &str = "/(^|::)slow_/";

/// The predicates of a nextest filter expression other than the two it is known to hold: the slow
/// lane's `test(…)` and a `binary_id(…)` of [`OUT_OF_THE_STEP`]. Each is `name(argument)`, the
/// blanks of the text ignored. Any other predicate (`package`, `binary`, `kind`, a wildcard, a
/// regular expression, a `test` that names a required test) may leave a required binary out, so
/// the filter is not read further: the step's report decides ([`left_out`]).
#[cfg(test)]
pub fn unknown_predicates(filter: &str) -> Vec<String> {
    let chars: Vec<char> = filter.chars().collect();
    let mut unknown = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        if !chars[at].is_alphabetic() {
            at += 1;
            continue;
        }
        let start = at;
        while at < chars.len() && (chars[at].is_alphanumeric() || chars[at] == '_') {
            at += 1;
        }
        let name: String = chars[start..at].iter().collect();
        let mut open = at;
        while open < chars.len() && chars[open].is_whitespace() {
            open += 1;
        }
        if chars.get(open) != Some(&'(') || matches!(name.as_str(), "and" | "or" | "not") {
            continue;
        }
        let mut depth = 0usize;
        let mut close = open;
        while close < chars.len() {
            match chars[close] {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            close += 1;
        }
        let argument: String = chars[open + 1..close.min(chars.len())]
            .iter()
            .filter(|c| !c.is_whitespace())
            .collect();
        let known = match name.as_str() {
            "test" => argument == SLOW_SELECTOR,
            "binary_id" => OUT_OF_THE_STEP.contains(&argument.as_str()),
            _ => false,
        };
        if !known {
            unknown.push(format!("{name}({argument})"));
        }
        at = close + 1;
    }
    unknown
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
    let scope_text = scope.join(" ");
    if runs(SHARED_TREES) {
        let made =
            crate::shared_trees::make(root, &scope_text).map_err(|e| format!("test step: {e}"))?;
        println!(
            "test step: pre-step {} (not counted in the budget)",
            made.line()
        );
    }
    let setup_report = target_dir(root)
        .join("nextest")
        .join(crate::shared_trees::REPORT);
    let _ = fs::remove_file(&setup_report);
    let mut broken = None;
    let (require, said) =
        require_delegation(std::env::var_os(REQUIRE_DELEGATION).as_deref(), || {
            match crate::delegation::probe(root) {
                Ok(record) => crate::delegation::decide(&record),
                Err(error) => {
                    broken = Some(error.clone());
                    Err(error)
                }
            }
        });
    if let Some(error) = broken {
        return Err(format!(
            "test step: {DELEGATION_PROBE} could not run: {error}"
        ));
    }
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
    // The scope the setup script of the shared trees builds and keys them in, so its `cargo test`
    // finds this build current (`crates/loom-xtask/src/shared_trees.rs`).
    command.env("LOOM_TEST_SCOPE", &scope_text);
    if require {
        // A name in a literal, as for the start budgets below: `loom_governor::durable`
        // clears a variable set here only when its name is a literal.
        command.env("LOOM_REQUIRE_DELEGATION", "1");
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
    if let Ok(line) = fs::read_to_string(&setup_report) {
        println!(
            "test step: setup script {} (in the timed window)",
            line.trim()
        );
    }
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

    let (setups, cases) = match fs::read_to_string(&report) {
        Ok(xml) => setup_scripts_apart(parse_junit(&xml)),
        Err(error) => {
            return Err(format!(
                "test step: no JUnit report at {} ({error}); nextest {}",
                report.display(),
                describe(nextest)
            ));
        }
    };
    for setup in &setups {
        println!(
            "test step: setup script {} ran {:.1} s in the timed window{}",
            setup.name,
            setup.seconds,
            if setup.failed { ", FAILED" } else { "" }
        );
    }
    let (mut text, mut failures) =
        verdict(&cases, nextest, doc, wall, nextest_seconds, doc_seconds);
    let lacking = left_out(&cases, packages);
    text.push_str(&format!(
        "\ntest step: {} of {} required test(s) of the folded conformance and guard binaries ran",
        required_count(packages) - lacking.len(),
        required_count(packages)
    ));
    failures.extend(lacking);
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

/// The package whose `tree_run` tests share trees (`crates/loom-cli/tests/tree_run.rs`): the step
/// builds what is missing of them with `loom-xtask shared-trees` after its build and before its
/// timed run, so no test of the step builds one or waits on the lock of another that does, a wait
/// each waiter counted in [`BINARY_BUDGETS`]. nextest's setup script runs the same in the timed
/// window and finds them built.
const SHARED_TREES: &str = "b10x-loom-cli";

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
/// everything above it here, after `task ess-gate` built them without.
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

/// The prefix of the `classname` nextest gives a setup script in its JUnit report.
const SETUP_SCRIPT: &str = "@setup-script:";

/// The setup scripts of a report's cases (nextest writes each as a `<testcase>` of its own), and
/// the tests: a setup script is no test and no binary, and its time is printed on a line of its
/// own.
pub fn setup_scripts_apart(cases: Vec<Case>) -> (Vec<Case>, Vec<Case>) {
    cases
        .into_iter()
        .partition(|case| case.binary.starts_with(SETUP_SCRIPT))
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

    /// A start the timed run counts as `fixture:<tree>` was made by a test building a shared tree
    /// the setup should have built (`story:tree-run-binary-fits-its-start-bound`): it fails the
    /// step, naming the tree, though the count is within the bound; the same count of test lines
    /// passes.
    #[test]
    fn a_fixture_start_in_the_timed_run_fails_the_step() {
        let budget = &START_BUDGETS[0];
        let ran = parse_junit(&tree_run_report(&[1.0, 2.0]));
        let (text, failure) = start_verdict(budget, &ran, Some("case_0\nfixture:ended\ncase_1\n"));
        let failure = failure.expect("a fixture start in the timed run fails the step");
        assert!(
            failure.contains("tree_run") && failure.contains("ended"),
            "{failure}"
        );
        assert!(text.contains("3 starts"), "{text}");
        let (text, failure) = start_verdict(budget, &ran, Some("case_0\ncase_0\ncase_1\n"));
        assert_eq!(failure, None, "{text}");
    }

    /// nextest reports its setup script as a `<testcase>`: the step counts it as no test and no
    /// binary.
    #[test]
    fn a_setup_script_in_the_report_is_not_a_test() {
        let xml = tree_run_report(&[1.0]).replace(
            "</testsuites>",
            "    <testsuite name=\"@setup-script:shared-trees\" tests=\"1\">\n        \
             <testcase name=\"shared-trees\" classname=\"@setup-script:shared-trees\" \
             timestamp=\"t\" time=\"6.800\"/>\n    </testsuite>\n</testsuites>",
        );
        let (setups, cases) = setup_scripts_apart(parse_junit(&xml));
        assert_eq!(setups.len(), 1, "{setups:?}");
        assert_eq!(
            (setups[0].name.as_str(), setups[0].seconds),
            ("shared-trees", 6.8)
        );
        assert!(
            cases.iter().all(|case| !case.binary.starts_with("@setup")),
            "{cases:?}"
        );
        assert_eq!(cases.len(), 2, "{cases:?}");
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

    /// The test binaries `task check` runs inside the test step's one nextest run rather than as
    /// cargo calls of their own (`story:ci-job-time-budget`): each such call selected other
    /// packages, so cargo unified other features and built crates the step's build built again.
    const IN_THE_STEP: [&str; 4] = [
        "b10x-loom-conformance::conform",
        "b10x-loom-commission-conformance::conform",
        "b10x-loom-intake-conformance::conform",
        "b10x-loom-commission-testkit::executor_port",
    ];

    /// The test binaries that stay out of the step, each with the task of `check` that runs it:
    /// the commission one synthesizes a suite per test (D2 a of `story:ci-job-time-budget`).
    const OWN_TASKS: [(&str, &str); 2] = [
        ("b10x-loom-executor::ess_gate", "ess-gate"),
        ("b10x-loom-commission::ess_gate", "commission:ess-gate"),
    ];

    /// The tasks `task check` no longer runs: [`IN_THE_STEP`] runs their tests. Each stays for a
    /// person to run alone.
    const FOLDED: [&str; 4] = [
        "conform",
        "commission:conform",
        "intake-conform",
        "commission:deps-guard",
    ];

    /// The steps of the root Taskfile's `check`, a task as `task: <name>`, a command as written.
    fn check_steps(root: &Path) -> Vec<String> {
        let taskfile: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&fs::read_to_string(root.join("Taskfile.yml")).unwrap())
                .unwrap();
        taskfile["tasks"]["check"]["cmds"]
            .as_sequence()
            .unwrap()
            .iter()
            .map(|step| match step.as_str() {
                Some(command) => command.to_owned(),
                None => format!("task: {}", step["task"].as_str().unwrap()),
            })
            .collect()
    }

    /// `task check` builds the workspace's test targets once: the conformance suites and the
    /// dependency guard run in the test step's nextest run (its default filter does not leave
    /// them out) and none of their tasks is a step of `check`; both `ess_gate` binaries stay out
    /// of the step and keep their tasks in `check`.
    #[test]
    fn the_gate_builds_the_test_targets_once() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let steps = check_steps(&root);
        assert!(
            steps.iter().any(|step| step == "task: test-step"),
            "{steps:?}"
        );
        for task in FOLDED {
            assert!(
                !steps.iter().any(|step| *step == format!("task: {task}")),
                "task check runs `{task}`, a cargo build of its own: {steps:?}"
            );
            let defined: serde_yaml_ng::Value = serde_yaml_ng::from_str(
                &fs::read_to_string(root.join(if task.starts_with("commission:") {
                    "Taskfile.commission.yml"
                } else {
                    "Taskfile.yml"
                }))
                .unwrap(),
            )
            .unwrap();
            let name = task.trim_start_matches("commission:");
            assert!(
                defined["tasks"][name]["cmds"].as_sequence().is_some(),
                "the task `{task}` is gone; it stays for a person to run alone"
            );
        }
        let config: toml::Value =
            toml::from_str(&fs::read_to_string(root.join(".config/nextest.toml")).unwrap())
                .unwrap();
        let filter = config["profile"]["default"]["default-filter"]
            .as_str()
            .unwrap();
        // Not a search for the binaries' names: any spelling of an exclusion (`package(…)`,
        // `binary(…)`, a wildcard, a regular expression) is a predicate nobody here knows, and the
        // step's report of its run decides (`left_out`).
        assert_eq!(
            unknown_predicates(filter),
            Vec::<String>::new(),
            "the test step's filter may leave out {IN_THE_STEP:?}: {filter}"
        );
        assert_eq!(
            OWN_TASKS.map(|(binary, _)| binary),
            OUT_OF_THE_STEP,
            "the binaries the filter may leave out are the ones with a task of their own"
        );
        for (binary, task) in OWN_TASKS {
            assert!(
                filter.contains(&format!("binary_id({binary})")),
                "the test step runs {binary}, which stays out of it: {filter}"
            );
            assert!(
                steps.iter().any(|step| *step == format!("task: {task}")),
                "task check does not run `{task}`: {steps:?}"
            );
        }
    }

    /// CI's job `loom / task check` is bounded at 30 minutes (`story:ci-job-time-budget`).
    #[test]
    fn the_task_check_job_is_bounded_at_30_minutes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let workflow: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            &fs::read_to_string(root.join(".github/workflows/check.yml")).unwrap(),
        )
        .unwrap();
        let job = &workflow["jobs"]["check"];
        assert_eq!(job["name"].as_str(), Some("loom / task check"), "{job:?}");
        assert_eq!(job["timeout-minutes"].as_u64(), Some(30), "{job:?}");
    }

    /// Each test of [`IN_THE_STEP`]'s binaries the step must run, written out here on its own, so
    /// that emptying [`REQUIRED`] is a change to this list as well.
    const PINNED: [(&str, &str); 5] = [
        ("b10x-loom-conformance::conform", "ess_conformance_report"),
        (
            "b10x-loom-commission-conformance::conform",
            "ess_conformance_report",
        ),
        (
            "b10x-loom-intake-conformance::conform",
            "ess_conformance_report",
        ),
        (
            "b10x-loom-commission-testkit::executor_port",
            "executor_port_contract",
        ),
        (
            "b10x-loom-commission-testkit::executor_port",
            "commission_depends_on_no_connectors_crate",
        ),
    ];

    /// A report of the whole workspace's run: a case of every [`PINNED`] test but those of the
    /// binaries in `without`, beside tests of other binaries (as `ess_gate`'s are not in it).
    fn report_without(without: &[&str]) -> Vec<Case> {
        let case = |binary: &str, name: &str| Case {
            binary: binary.to_owned(),
            name: name.to_owned(),
            seconds: 0.1,
            failed: false,
            skipped: false,
        };
        let mut cases = vec![
            case("loom-xtask::checks", "a_check"),
            case("b10x-loom-conformance::conform", "derived_statuses_rate"),
            case("b10x-loom-cli::main", "tree_run::a_case"),
        ];
        for (binary, name) in PINNED {
            if !without.contains(&binary) {
                cases.push(case(binary, name));
            }
        }
        cases
    }

    fn named(packages: &[&str]) -> Vec<String> {
        packages.iter().map(|&p| p.to_owned()).collect()
    }

    /// The step's own report decides, whatever way a binary was left out of the run: a binary
    /// without a case of its test fails the step, naming it, and the whole report passes.
    #[test]
    fn the_step_fails_when_a_binary_of_the_folded_tasks_is_not_in_its_report() {
        assert_eq!(left_out(&report_without(&[]), &[]), Vec::<String>::new());
        for binary in IN_THE_STEP {
            let failures = left_out(&report_without(&[binary]), &[]);
            assert!(!failures.is_empty(), "{binary} left out, the step passes");
            for failure in &failures {
                assert!(failure.contains(binary), "{binary}: {failure}");
                for other in IN_THE_STEP.iter().filter(|other| **other != binary) {
                    assert!(!failure.contains(other), "{binary}: {failure}");
                }
            }
        }
        let mut one_test = report_without(&[]);
        one_test.retain(|case| case.name != "commission_depends_on_no_connectors_crate");
        let failures = left_out(&one_test, &[]);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(
            failures[0].contains("b10x-loom-commission-testkit::executor_port")
                && failures[0].contains("commission_depends_on_no_connectors_crate"),
            "{failures:?}"
        );
        let mut nothing = report_without(&[]);
        nothing.clear();
        assert_eq!(left_out(&nothing, &[]).len(), PINNED.len());
    }

    /// A binary of another package's case, or a case of its own binary under another test's name,
    /// is no case of the required test.
    #[test]
    fn a_case_of_another_binary_or_test_does_not_stand_for_a_required_one() {
        let mut cases = report_without(&["b10x-loom-intake-conformance::conform"]);
        for (binary, name) in [
            (
                "b10x-loom-intake-conformance::other",
                "ess_conformance_report",
            ),
            ("b10x-loom-intake-conformance::conform", "another_test"),
            ("b10x-loom-cli::main", "ess_conformance_report"),
        ] {
            cases.push(Case {
                binary: binary.to_owned(),
                name: name.to_owned(),
                seconds: 0.1,
                failed: false,
                skipped: false,
            });
        }
        let failures = left_out(&cases, &[]);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(
            failures[0].contains("b10x-loom-intake-conformance::conform"),
            "{failures:?}"
        );
    }

    /// Over named packages the step requires the binaries of those packages only: a package left
    /// out of `-p` is the person's choice for that run (the gate's step names none, see
    /// `the_test_step_task_runs_the_whole_workspace`), one named whose binary is missing fails.
    #[test]
    fn over_named_packages_the_step_requires_their_binaries_only() {
        let only = |binary: &str| -> Vec<Case> {
            report_without(&IN_THE_STEP)
                .into_iter()
                .chain(
                    report_without(&[])
                        .into_iter()
                        .filter(|c| c.binary == binary),
                )
                .collect()
        };
        for binary in IN_THE_STEP {
            let package = binary.split("::").next().unwrap();
            assert_eq!(
                left_out(&only(binary), &named(&[package])),
                Vec::<String>::new(),
                "{binary}"
            );
            assert!(
                !left_out(&report_without(&IN_THE_STEP), &named(&[package])).is_empty(),
                "{package} named, its binary {binary} missing, and the step passes"
            );
        }
        assert_eq!(
            left_out(&report_without(&IN_THE_STEP), &named(&["loom-xtask"])),
            Vec::<String>::new()
        );
    }

    /// [`REQUIRED`] holds exactly the binaries and tests of [`PINNED`].
    #[test]
    fn the_required_list_is_the_pinned_one() {
        let required: Vec<(&str, &str)> = REQUIRED
            .iter()
            .flat_map(|r| r.tests.iter().map(|test| (r.binary, *test)))
            .collect();
        assert_eq!(required, PINNED.to_vec());
        let binaries: Vec<&str> = REQUIRED.iter().map(|r| r.binary).collect();
        assert_eq!(binaries, IN_THE_STEP.to_vec());
    }

    /// The command of a task of `file`, as `cargo test --locked -p <package> --test <binary>`
    /// names its test binary: `<package>::<binary>`.
    fn binary_of_task(root: &Path, file: &str, task: &str) -> String {
        let taskfile: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&fs::read_to_string(root.join(file)).unwrap()).unwrap();
        let commands = taskfile["tasks"][task]["cmds"].as_sequence().unwrap();
        assert_eq!(commands.len(), 1, "{file} task {task}: {commands:?}");
        let words: Vec<&str> = commands[0].as_str().unwrap().split_whitespace().collect();
        let after = |flag: &str| {
            let at = words.iter().position(|word| *word == flag).unwrap();
            words[at + 1]
        };
        format!("{}::{}", after("-p"), after("--test"))
    }

    /// The class: every test binary a task of [`FOLDED`] runs is required of the step, derived
    /// from the Taskfiles, and each required test is a test of its file.
    #[test]
    fn every_binary_a_folded_task_runs_is_required_and_its_tests_exist() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for task in FOLDED {
            let (file, name) = match task.strip_prefix("commission:") {
                Some(name) => ("Taskfile.commission.yml", name),
                None => ("Taskfile.yml", task),
            };
            let binary = binary_of_task(&root, file, name);
            assert!(
                REQUIRED.iter().any(|required| required.binary == binary),
                "task `{task}` runs {binary}, which the step does not require: {REQUIRED:?}"
            );
        }
        for required in REQUIRED {
            let (package, binary) = required.binary.split_once("::").unwrap();
            let manifest = fs::read_dir(root.join("crates"))
                .unwrap()
                .map(|entry| entry.unwrap().path().join("Cargo.toml"))
                .find(|path| {
                    fs::read_to_string(path)
                        .is_ok_and(|text| text.contains(&format!("name = \"{package}\"")))
                })
                .unwrap_or_else(|| panic!("no crate is the package {package}"));
            let source = manifest
                .parent()
                .unwrap()
                .join(format!("tests/{binary}.rs"));
            let text = fs::read_to_string(&source)
                .unwrap_or_else(|error| panic!("read {}: {error}", source.display()));
            for test in required.tests {
                assert!(
                    text.contains(&format!("fn {test}(")),
                    "{} has no test {test}",
                    source.display()
                );
            }
        }
    }

    /// The default filter of the step names no predicate the guard does not know, so no spelling
    /// of an exclusion of a required binary (a `package(…)`, a `binary(…)`, a wildcard or a
    /// regular expression, blanks inside) goes unread; the step's report is the guard behind it.
    #[test]
    fn the_default_filter_names_no_predicate_the_guard_does_not_know() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let config: toml::Value =
            toml::from_str(&fs::read_to_string(root.join(".config/nextest.toml")).unwrap())
                .unwrap();
        let filter = config["profile"]["default"]["default-filter"]
            .as_str()
            .unwrap();
        assert_eq!(unknown_predicates(filter), Vec::<String>::new(), "{filter}");
        let slow = "not test(/(^|::)slow_/)";
        let gates =
            "binary_id(b10x-loom-executor::ess_gate) | binary_id(b10x-loom-commission::ess_gate)";
        assert_eq!(
            unknown_predicates(&format!("{slow} and not ({gates})")),
            Vec::<String>::new()
        );
        for (spelling, what) in [
            ("package(b10x-loom-conformance)", "package"),
            ("package(=b10x-loom-conformance)", "package"),
            ("binary_id(b10x-loom-conformance::*)", "binary_id"),
            ("binary_id(b10x-loom-conformance::conform)", "binary_id"),
            ("binary_id( b10x-loom-conformance::conform )", "binary_id"),
            ("binary_id\t(b10x-loom-conformance::conform)", "binary_id"),
            ("binary_id(/conform$/)", "binary_id"),
            ("binary(conform)", "binary"),
            ("binary(=executor_port)", "binary"),
            ("kind(test)", "kind"),
            ("rdeps(b10x-loom-conformance)", "rdeps"),
            ("test(/conformance/)", "test"),
            ("test(ess_conformance_report)", "test"),
            ("test(=ess_conformance_report)", "test"),
        ] {
            let unknown = unknown_predicates(&format!("{slow} and not ({gates} | {spelling})"));
            assert_eq!(unknown.len(), 1, "{spelling}: {unknown:?}");
            assert!(unknown[0].starts_with(what), "{spelling}: {unknown:?}");
        }
    }

    /// The gate's step runs the whole workspace: `task test-step` is the one command with no
    /// package named, `task check` runs that task and nothing else calls the step, and CI runs
    /// `task check`. Naming a package (`-- test-step -p …`) would require only that one's binaries.
    #[test]
    fn the_test_step_task_runs_the_whole_workspace() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut callers = Vec::new();
        for file in ["Taskfile.yml", "Taskfile.commission.yml"] {
            let taskfile: serde_yaml_ng::Value =
                serde_yaml_ng::from_str(&fs::read_to_string(root.join(file)).unwrap()).unwrap();
            for (name, task) in taskfile["tasks"].as_mapping().unwrap() {
                for command in task["cmds"].as_sequence().into_iter().flatten() {
                    if let Some(line) = command.as_str()
                        && line.contains("test-step")
                    {
                        callers.push((name.as_str().unwrap().to_owned(), line.to_owned()));
                    }
                }
            }
        }
        assert_eq!(
            callers,
            [(
                "test-step".to_owned(),
                "cargo run -q --locked -p loom-xtask -- test-step".to_owned()
            )]
        );
        let steps = check_steps(&root);
        assert_eq!(
            steps
                .iter()
                .filter(|step| *step == "task: test-step")
                .count(),
            1,
            "{steps:?}"
        );
        let workflow: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            &fs::read_to_string(root.join(".github/workflows/check.yml")).unwrap(),
        )
        .unwrap();
        let runs: Vec<&str> = workflow["jobs"]["check"]["steps"]
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(|step| step["run"].as_str())
            .collect();
        assert!(runs.contains(&"task check"), "{runs:?}");
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
