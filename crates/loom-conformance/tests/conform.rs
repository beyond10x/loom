//! `task check` holds Loom's ESS specification to its synthesized conformance suite.
//!
//! [`ess_conformance_report`] synthesizes the suite from `ess/` with `ess verify conform
//! synthesize`, runs it against `b10x-loom-executor` through [`LoomTarget`], and reads the verdict
//! from the `ess-conformance-report/2` document, never from an exit code:
//!
//! 1. the report records at least one executed scenario (passed or failed);
//! 2. every command `ess specify compile --path ess --format json` lists has at least one scenario
//!    of its own in the suite (`<command>/outcome/<outcome>`);
//! 3. the report records no failed scenario;
//! 4. every skipped scenario in the report is named in `ess/SKIPPED.md`, and the same check,
//!    applied to a copy of the report that adds one skipped scenario `S` that `ess/SKIPPED.md`
//!    does not name, fails and names `S`.
//!
//! The Rust producer profile has no `skipped` category of its own for a scenario the target cannot
//! answer: it reports it `unsupported`. The check therefore holds `unsupported` to the same rule
//! as `skipped` (named in `ess/SKIPPED.md` or refused), and refuses `error` outright, since an
//! execution failure hides whatever the scenario would have shown. It also refuses an
//! `ess/SKIPPED.md` entry naming a scenario the report records as passed.
//!
//! [`taskfile_check_runs_conform`] holds the other half of the claim: `task check` runs this test,
//! through the task `conform`, as a step of its own.
//!
//! [`LoomTarget`]: b10x_loom_conformance::LoomTarget

use b10x_loom_conformance::run_suite;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// The report format the verdict is read from.
const REPORT_FORMAT: &str = "ess-conformance-report/2";

/// The scenario expectation 4 adds to a copy of the report as skipped.
const UNNAMED_SKIP: &str = "loom.run.Unnamed/outcome/not-in-skipped-md";

/// The command `task conform` runs.
const CONFORM: &str = "cargo test --locked -p b10x-loom-conformance --test conform";

/// The repository whose `ess/` is checked, read when the test runs rather than when it was built:
/// a test binary is reused across worktrees that share one `CARGO_TARGET_DIR`.
fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_else(|| {
        panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo (`task conform`)")
    });
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

/// A fresh directory under `CARGO_TARGET_TMPDIR` for this run's suite and report. A red run keeps
/// it, so the report can be read afterwards; a passing run removes it.
fn scratch() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("conform-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("create {}: {error}", dir.display()));
    dir
}

/// Runs `ess` with `args` in the repository root and returns what it printed, refusing a failed
/// run.
fn ess(args: &[&str], out: Option<&Path>) -> String {
    let mut command = Command::new("ess");
    command.current_dir(root()).args(args);
    if let Some(out) = out {
        command.arg(out);
    }
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("`ess` must be on PATH: {error}"));
    assert!(
        output.status.success(),
        "`ess {}` exited {}:\n{}{}",
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("`ess` prints UTF-8")
}

/// The suite `ess/` obliges, synthesized fresh.
fn synthesize(out: &Path) -> String {
    ess(
        &["verify", "conform", "synthesize", "--path", "ess", "--out"],
        Some(out),
    );
    fs::read_to_string(out).unwrap_or_else(|error| panic!("read {}: {error}", out.display()))
}

/// The qualified name of every command the compiled specification declares.
fn compiled_commands() -> BTreeSet<String> {
    let compiled: Value = serde_json::from_str(&ess(
        &["specify", "compile", "--path", "ess", "--format", "json"],
        None,
    ))
    .expect("`ess specify compile --format json` prints JSON");
    compiled["commands"]
        .as_object()
        .unwrap_or_else(|| panic!("the compiled specification carries no `commands` object"))
        .keys()
        .cloned()
        .collect()
}

/// The scenario ids `ess/SKIPPED.md` names, each on a line `- <scenario id>: <reason>` with a
/// reason that is not empty.
fn skipped_md_names(body: &str) -> BTreeSet<String> {
    body.lines()
        .filter_map(|line| line.strip_prefix("- "))
        .filter_map(|entry| entry.split_once(": "))
        .filter(|(_, reason)| !reason.trim().is_empty())
        .map(|(id, _)| id.trim().to_owned())
        .collect()
}

/// The scenario ids a report lists under `outcomes.<category>`.
fn outcome_ids(report: &Value, category: &str) -> Vec<String> {
    report["outcomes"][category]
        .as_array()
        .unwrap_or_else(|| panic!("the report lists no `outcomes.{category}` array: {report}"))
        .iter()
        .map(|id| {
            id.as_str()
                .unwrap_or_else(|| panic!("`outcomes.{category}` holds a non-string: {id}"))
                .to_owned()
        })
        .collect()
}

/// A count the report records under `counts.<category>`.
fn count(report: &Value, category: &str) -> u64 {
    report["counts"][category]
        .as_u64()
        .unwrap_or_else(|| panic!("the report records no `counts.{category}`: {report}"))
}

/// Every compiled command with no scenario of its own among `scenarios`.
fn uncovered<'c>(commands: &'c BTreeSet<String>, scenarios: &[String]) -> Vec<&'c String> {
    commands
        .iter()
        .filter(|command| {
            let own = format!("{command}/outcome/");
            !scenarios.iter().any(|id| id.starts_with(&own))
        })
        .collect()
}

/// Every way `report` falls short of a passing report, each naming what it found. Empty means the
/// report passes.
fn violations(report: &Value, named: &BTreeSet<String>) -> Vec<String> {
    let mut found = Vec::new();

    let passed = outcome_ids(report, "passed");
    let failed = outcome_ids(report, "failed");
    let executed = count(report, "passed") + count(report, "failed");
    if passed.len() + failed.len() == 0 || executed == 0 {
        found.push(format!(
            "expectation 1: the report records {executed} executed scenario(s) (counts.passed + \
             counts.failed); at least 1 is required"
        ));
    }

    if !failed.is_empty() || count(report, "failed") != 0 {
        found.push(format!(
            "expectation 3: the report records {} failed scenario(s) (counts.failed = {}): {failed:?}",
            failed.len(),
            count(report, "failed")
        ));
    }

    let errored = outcome_ids(report, "error");
    if !errored.is_empty() || count(report, "error") != 0 {
        found.push(format!(
            "expectation 3: the report records {} errored scenario(s) (counts.error = {}): {errored:?}",
            errored.len(),
            count(report, "error")
        ));
    }

    for category in ["skipped", "unsupported"] {
        for id in outcome_ids(report, category) {
            if !named.contains(&id) {
                found.push(format!(
                    "expectation 4: the {category} scenario `{id}` is not named in ess/SKIPPED.md"
                ));
            }
        }
    }

    // A skip is for a scenario the target cannot answer; one it answers and passes is not skipped.
    for id in named {
        if passed.contains(id) {
            found.push(format!(
                "ess/SKIPPED.md names `{id}`, which the report records as passed"
            ));
        }
    }

    found
}

/// A copy of `report` that records `scenario` as one more skipped scenario.
fn with_skipped(report: &Value, scenario: &str) -> Value {
    let mut copy = report.clone();
    copy["outcomes"]["skipped"]
        .as_array_mut()
        .expect("the report lists an `outcomes.skipped` array")
        .push(Value::String(scenario.to_owned()));
    for category in ["skipped", "total"] {
        copy["counts"][category] = Value::from(count(report, category) + 1);
    }
    copy
}

#[test]
fn ess_conformance_report() {
    let dir = scratch();
    let suite_text = synthesize(&dir.join("suite.json"));
    let suite: Value = serde_json::from_str(&suite_text).expect("the suite is JSON");
    let scenarios: Vec<String> = suite["scenarios"]
        .as_object()
        .expect("the suite carries an object of scenarios")
        .keys()
        .cloned()
        .collect();
    assert!(
        !scenarios.is_empty(),
        "the suite synthesized from ess/ holds no scenario"
    );

    let executed =
        run_suite(&suite_text).unwrap_or_else(|error| panic!("the suite did not run: {error}"));
    let report_path = dir.join("report.json");
    let diagnostics_path = dir.join("diagnostics.json");
    fs::write(&report_path, &executed.report).expect("write report.json");
    fs::write(&diagnostics_path, &executed.diagnostics).expect("write diagnostics.json");
    let report: Value = serde_json::from_str(&executed.report).expect("the report is JSON");

    assert_eq!(report["format"], REPORT_FORMAT, "report: {report}");
    assert_eq!(
        count(&report, "total"),
        u64::try_from(scenarios.len()).expect("the suite fits"),
        "the report does not cover every scenario of the suite"
    );

    let mut found = Vec::new();
    let skipped_md = root().join("ess/SKIPPED.md");
    let named = match fs::read_to_string(&skipped_md) {
        Ok(body) => skipped_md_names(&body),
        Err(error) => {
            found.push(format!("read {}: {error}", skipped_md.display()));
            BTreeSet::new()
        }
    };

    // Expectation 4: the check refuses a skip ess/SKIPPED.md does not name, and names it.
    assert!(
        !named.contains(UNNAMED_SKIP),
        "ess/SKIPPED.md names `{UNNAMED_SKIP}`, which expectation 4 needs unnamed"
    );
    let before: BTreeSet<String> = violations(&report, &named).into_iter().collect();
    let after: BTreeSet<String> = violations(&with_skipped(&report, UNNAMED_SKIP), &named)
        .into_iter()
        .collect();
    let added: Vec<&String> = after.difference(&before).collect();
    assert!(
        added.len() == 1 && added[0].contains(UNNAMED_SKIP),
        "expectation 4: a copy of the report with the unnamed skip `{UNNAMED_SKIP}` must fail the \
         check naming it, and only it; it added {added:?}"
    );

    // Expectation 2, on the suite.
    let commands = compiled_commands();
    let missing = uncovered(&commands, &scenarios);
    if commands.is_empty() || !missing.is_empty() {
        found.push(format!(
            "expectation 2: of {} compiled command(s), these have no scenario of their own in the \
             suite: {missing:?}",
            commands.len()
        ));
    }

    // Expectations 1, 3 and 4, on the report itself.
    found.extend(violations(&report, &named));
    assert!(
        found.is_empty(),
        "the conformance report does not pass ({} of {} scenario(s) passed):\n  {}\n\
         report: {}\ndiagnostics: {}",
        count(&report, "passed"),
        scenarios.len(),
        found.join("\n  "),
        report_path.display(),
        diagnostics_path.display()
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `task conform` runs [`ess_conformance_report`], and `task check` lists `conform` as a step of
/// its own.
#[test]
fn taskfile_check_runs_conform() {
    let path = root().join("Taskfile.yml");
    let taskfile = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let body = |name: &str| -> Vec<String> {
        let header = format!("  {name}:");
        let mut lines = taskfile.lines().skip_while(|line| *line != header);
        assert!(lines.next().is_some(), "Taskfile.yml has no task `{name}`");
        lines
            .take_while(|line| line.is_empty() || line.starts_with("    "))
            .map(|line| line.trim().to_owned())
            .collect()
    };
    assert!(
        body("conform")
            .iter()
            .any(|line| *line == format!("- {CONFORM}")),
        "task conform does not run `{CONFORM}`"
    );
    assert!(
        body("check").iter().any(|line| line == "- task: conform"),
        "task check does not list `conform` as its own step"
    );
}
