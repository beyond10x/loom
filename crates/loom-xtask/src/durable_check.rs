//! `durable-check`, the lane `task durable-check` runs in `task check`, outside the test step
//! (`story:tree-run-tests-fit-the-budget`): the test sync switch of `b10x-loom-governor` reaches no
//! build but a test build of `b10x-loom-cli`, and without it every durable write syncs.
//!
//! 1. `cargo tree` over the four packages of the count tests, with their dev-dependencies, names no
//!    `test-unsynced-store`: their tests build without the feature.
//! 2. The count tests run over those packages ([`COUNT_TESTS`]), each once and passing, read from
//!    nextest's JUnit report: a filter that selects none of them fails the lane, never passes it.
//! 3. `cargo check --release` of `b10x-loom-governor` with the feature fails with its
//!    `compile_error!` text ([`REFUSAL`]).
//! 4. `cargo tree` of `b10x-loom-cli`'s normal dependencies names no `test-unsynced-store`.

use std::fs;
use std::path::Path;
use std::process::Output;
use std::time::Instant;

use crate::test_step::{cargo_command, parse_junit, target_dir};

/// The packages of the count tests; none has `b10x-loom-cli` among its dev-dependencies.
pub const PACKAGES: [&str; 4] = [
    "b10x-loom-governor",
    "b10x-loom-runlog",
    "b10x-loom-tree",
    "b10x-loom-bindings",
];

/// The count tests: every durable write of the tree path syncs.
pub const COUNT_TESTS: [&str; 4] = [
    "each_governor_write_syncs_its_file_and_directory",
    "a_run_log_append_syncs_its_file_and_directory",
    "a_tree_store_write_syncs_its_file_and_directory",
    "a_kept_blob_is_synced",
];

/// The feature a release build must not hold.
pub const FEATURE: &str = "test-unsynced-store";

/// The text of the `compile_error!` a release build with the feature fails with.
pub const REFUSAL: &str = "is for test builds only";

/// Runs the lane in `root`.
pub fn durable_check(root: &Path) -> Result<(), String> {
    let started = Instant::now();

    let mut tree = vec![
        "tree",
        "--locked",
        "-e",
        "normal,dev,features",
        "--target",
        "all",
    ];
    for package in PACKAGES {
        tree.extend(["-p", package]);
    }
    no_feature(&captured(root, &tree)?, "the count tests' packages")?;

    let report = target_dir(root).join("nextest/default/junit.xml");
    let _ = fs::remove_file(&report);
    let filter = format!("test(/(^|::)({})$/)", COUNT_TESTS.join("|"));
    let mut run = vec![
        "nextest",
        "run",
        "--locked",
        "--no-fail-fast",
        "-E",
        &filter,
    ];
    for package in PACKAGES {
        run.extend(["-p", package]);
    }
    let ran = cargo_command(root)
        .args(&run)
        .status()
        .map_err(|e| format!("durable-check: run cargo {}: {e}", run.join(" ")))?;
    let xml = fs::read_to_string(&report).map_err(|e| {
        format!(
            "durable-check: no JUnit report at {}: {e}",
            report.display()
        )
    })?;
    counted_once(&parse_junit(&xml), ran.success())?;

    let release = [
        "check",
        "--release",
        "--locked",
        "-p",
        "b10x-loom-governor",
        "--features",
        FEATURE,
    ];
    refused(&captured(root, &release)?)?;

    let cli = [
        "tree",
        "--locked",
        "-p",
        "b10x-loom-cli",
        "-e",
        "normal,features",
        "--target",
        "all",
    ];
    no_feature(
        &captured(root, &cli)?,
        "b10x-loom-cli's normal dependencies",
    )?;

    println!(
        "durable-check: the count tests synced every write without the feature, a release build \
         with it is refused, and b10x-loom-cli does not hold it ({:.1} s)",
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

/// Runs `cargo <args>` in `root`, its output captured; an error when it cannot start.
fn captured(root: &Path, args: &[&str]) -> Result<Output, String> {
    cargo_command(root)
        .args(args)
        .output()
        .map_err(|e| format!("durable-check: run cargo {}: {e}", args.join(" ")))
}

/// Fails unless `cargo tree` exited 0 and names no [`FEATURE`].
pub fn no_feature(output: &Output, what: &str) -> Result<(), String> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        return Err(format!(
            "durable-check: cargo tree of {what} exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if stdout.contains(FEATURE) {
        return Err(format!(
            "durable-check: {what} hold the feature `{FEATURE}` of b10x-loom-governor, which only \
             b10x-loom-cli's dev-dependencies may enable"
        ));
    }
    println!("durable-check: {what} do not hold `{FEATURE}`");
    Ok(())
}

/// Fails unless each count test ran exactly once and passed, and nextest exited 0.
pub fn counted_once(cases: &[crate::test_step::Case], exited_0: bool) -> Result<(), String> {
    let mut problems = Vec::new();
    for test in COUNT_TESTS {
        let runs: Vec<_> = cases
            .iter()
            .filter(|case| case.name == test || case.name.ends_with(&format!("::{test}")))
            .collect();
        match runs.as_slice() {
            [case] if !case.failed => {}
            [case] => problems.push(format!("{} {} failed", case.binary, case.name)),
            [] => problems.push(format!("{test} did not run")),
            more => problems.push(format!("{test} ran {} times", more.len())),
        }
    }
    if cases.len() != COUNT_TESTS.len() {
        problems.push(format!(
            "{} test(s) ran, not the {} count tests",
            cases.len(),
            COUNT_TESTS.len()
        ));
    }
    if !exited_0 {
        problems.push("cargo nextest run did not exit 0".to_owned());
    }
    if problems.is_empty() {
        println!(
            "durable-check: {} count tests ran without the feature and passed",
            COUNT_TESTS.len()
        );
        Ok(())
    } else {
        Err(format!("durable-check:\n  {}", problems.join("\n  ")))
    }
}

/// Fails unless the release build failed with [`REFUSAL`].
pub fn refused(output: &Output) -> Result<(), String> {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() && stderr.contains(REFUSAL) {
        println!("durable-check: a release build with `{FEATURE}` is refused");
        Ok(())
    } else {
        Err(format!(
            "durable-check: `cargo check --release -p b10x-loom-governor --features {FEATURE}` \
             exited {} without the refusal `{REFUSAL}`: a release build can hold the test sync \
             switch\n{}",
            output.status,
            stderr.trim()
        ))
    }
}
