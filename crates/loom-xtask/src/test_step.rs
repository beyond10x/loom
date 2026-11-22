//! The test step of `task check`, held to its budget (`~/beyond10x/AGENTS.md` § Tests stay fast):
//! the step ends within [`STEP_BUDGET`] of wall time and no test runs longer than
//! [`TEST_BUDGET`].
//!
//! It runs `cargo nextest run --workspace --locked` under the default profile of
//! `.config/nextest.toml`, which leaves out the slow lane (`slow_` tests, `task slow`), kills a test
//! at 60 s and writes a JUnit report, then `cargo test --doc --workspace --locked` (nextest runs no
//! doc tests). It reads every test's time from the JUnit report and prints the ten slowest on every
//! run, green or red, then fails when either run failed, a test took longer than
//! [`TEST_BUDGET`] or the step longer than [`STEP_BUDGET`]. The wall time is taken inside the
//! step, so a wait for a build slot before it starts does not count, and it starts once the test
//! binaries are built (`cargo test --workspace --locked --no-run`, which nextest reuses), so a
//! cold build does not count either; the doc tests' own compilation does. `-p <package>` runs the
//! same over named packages; in a build-pool job nextest runs `SLOT_TOKENS` tests at once.

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

/// One test case of nextest's JUnit report.
#[derive(Debug, Clone, PartialEq)]
pub struct Case {
    /// `<package>::<binary>`.
    pub binary: String,
    pub name: String,
    pub seconds: f64,
    pub failed: bool,
}

/// The step over the workspace, or over `packages` when any are named.
pub fn run(root: &Path, packages: &[String]) -> Result<(), String> {
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
    let build = with(&["test"], &["--locked", "--no-run"]);
    let built = cargo(root, &build)?;
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

    let started = Instant::now();
    let report = target_dir(root).join("nextest/default/junit.xml");
    let _ = fs::remove_file(&report);

    let mut nextest = with(&["nextest", "run"], &["--locked", "--no-fail-fast"]);
    if let Some(threads) = slot_tokens() {
        nextest.push(format!("--test-threads={threads}"));
    }
    let nextest = cargo(root, &nextest)?;
    let nextest_seconds = started.elapsed().as_secs_f64();
    let doc_started = Instant::now();
    let doc = cargo(root, &with(&["test", "--doc"], &["--locked"]))?;
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
    let (text, failures) = verdict(&cases, nextest, doc, wall, nextest_seconds, doc_seconds);
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
    let mut text = format!(
        "test step: {} test(s) in {} binar{} by nextest ({nextest_seconds:.1} s), doc tests \
         ({doc_seconds:.1} s); wall time {:.1} s of {} s\nslowest {SLOWEST} test(s):",
        cases.len(),
        binaries.len(),
        if binaries.len() == 1 { "y" } else { "ies" },
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

    let mut failures = Vec::new();
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

fn target_dir(root: &Path) -> PathBuf {
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
    let status = command
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|e| format!("run cargo {}: {e}", args.join(" ")))?;
    Ok(status.code())
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
        cases.push(Case {
            binary: attribute("classname"),
            name: attribute("name"),
            seconds: attribute("time").parse().unwrap_or(0.0),
            failed: body.contains("<failure") || body.contains("<error"),
        });
    }
    cases
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
                },
                Case {
                    binary: "b10x-loom-executor::ess_gate".to_owned(),
                    name: "a_slow_one".to_owned(),
                    seconds: 60.004,
                    failed: true,
                },
                Case {
                    binary: "loom-xtask::checks".to_owned(),
                    name: "a & b".to_owned(),
                    seconds: 0.01,
                    failed: false,
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
