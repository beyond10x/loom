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
//! cold build does not count either; the doc tests' own compilation does. Beside that build it
//! fills the schema cache the `crd` tests of `b10x-loom-resource` read ([`fill_schema_cache`]) and
//! the syntheses the generator tests of `loom-xtask` and `loom-commission-xtask` read
//! ([`fill_synthesis_stand_in`]), so no test of the step makes one or waits for it. `-p <package>` runs the
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
    let runs = |package: &str| packages.is_empty() || packages.iter().any(|p| p == package);
    let mut fills: Vec<(&str, Fill<'_>)> = Vec::new();
    if runs(CRD_PACKAGE) {
        fills.push(("schema cache", Box::new(|| fill_schema_cache(root))));
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
    let nextest = cargo(root, &nextest)?;
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

/// The package whose `crd` tests read the schema cache.
const CRD_PACKAGE: &str = "b10x-loom-resource";

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
