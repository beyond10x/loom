//! `task check` holds intake's ESS specification to its synthesized conformance suite.
//!
//! [`ess_conformance_report`] synthesizes the suite from `ess/intake/` with `ess verify conform
//! synthesize --suite-format 5` (declared coverage), runs it against `b10x-loom-intake-slice`
//! through [`IntakeTarget`], and reads the verdict from the `ess-conformance-report/2` document,
//! never from an exit code, by the rules `task commission:conform` applies to Commission:
//!
//! 1. the report records at least one passed scenario;
//! 2. the report records no failed scenario;
//! 3. every skipped or unsupported scenario in the report is named in `ess/SKIPPED.md`;
//! 4. the same check, applied to a copy of the report that adds one skipped scenario `S`, and to a
//!    copy that adds one unsupported scenario `U`, neither named in `ess/SKIPPED.md`, fails and
//!    names `S`, or `U`;
//! 5. the report's `conformance_status` is `passed`: without declared coverage ESS rates a run in
//!    which every scenario passed `inconclusive`;
//! 6. the same check, applied to a copy of a passing report whose `conformance_status` is
//!    `inconclusive`, fails and names the field.
//!
//! The Rust producer profile has no `skipped` category: a scenario the target cannot answer is
//! reported `unsupported`. The check therefore holds `unsupported` to the same rule as `skipped`
//! (named in `ess/SKIPPED.md` or refused), and refuses `error` outright, since an execution
//! failure hides whatever the scenario would have shown. It also refuses an `ess/SKIPPED.md` entry
//! naming a scenario the report records as passed.
//!
//! [`taskfile_check_runs_intake_conform`] holds the other half of the claim: `task check` runs
//! this test, through the task `intake-conform`, as a step of its own.
//!
//! [`IntakeTarget`]: b10x_loom_intake_conformance::IntakeTarget

use b10x_loom_intake_conformance::run_suite;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// The report format the verdict is read from.
const REPORT_FORMAT: &str = "ess-conformance-report/2";

/// The scenario expectation 4 adds to a copy of the report as skipped.
const UNNAMED_SKIP: &str = "intake.routing.Unnamed/outcome/not-in-skipped-md";

/// The scenario expectation 4 adds to a copy of the report as unsupported.
const UNNAMED_UNSUPPORTED: &str = "intake.routing.Unnamed/outcome/unsupported-not-in-skipped-md";

/// The command `task intake-conform` runs.
const INTAKE_CONFORM: &str = "cargo test --locked -p b10x-loom-intake-conformance --test conform";

/// The repository whose `ess/intake/` is checked, read when the test runs rather than when it was
/// built: a test binary is reused across worktrees that share one `CARGO_TARGET_DIR`.
fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_else(|| {
        panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo (`task intake-conform`)")
    });
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

/// A fresh directory under `CARGO_TARGET_TMPDIR` for this run's suite, report and the scenarios'
/// state directories. A red run keeps it, so the report can be read afterwards; a passing run
/// removes it.
fn scratch() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("intake-conform-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("create {}: {error}", dir.display()));
    dir
}

/// The suite `ess/intake/` obliges, synthesized fresh with declared coverage (`--suite-format 5`),
/// so the report can qualify the run `passed` rather than `inconclusive`.
fn synthesize(out: &Path) -> String {
    let output = Command::new("ess")
        .current_dir(root())
        .args([
            "verify",
            "conform",
            "synthesize",
            "--path",
            "ess/intake",
            "--suite-format",
            "5",
            "--out",
        ])
        .arg(out)
        .output()
        .unwrap_or_else(|error| panic!("`ess` must be on PATH: {error}"));
    assert!(
        output.status.success(),
        "`ess verify conform synthesize --path ess/intake --suite-format 5` exited {}:\n{}{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::read_to_string(out).unwrap_or_else(|error| panic!("read {}: {error}", out.display()))
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

/// Every way `report` falls short of a passing report, each naming what it found. Empty means the
/// report passes.
fn violations(report: &Value, named: &BTreeSet<String>) -> Vec<String> {
    let mut found = Vec::new();

    let passed = outcome_ids(report, "passed");
    if passed.is_empty() || count(report, "passed") == 0 {
        found.push(format!(
            "expectation 1: the report records {} passed scenario(s) (counts.passed = {}); at \
             least 1 is required",
            passed.len(),
            count(report, "passed")
        ));
    }

    let failed = outcome_ids(report, "failed");
    if !failed.is_empty() || count(report, "failed") != 0 {
        found.push(format!(
            "expectation 2: the report records {} failed scenario(s) (counts.failed = {}): {failed:?}",
            failed.len(),
            count(report, "failed")
        ));
    }

    let errored = outcome_ids(report, "error");
    if !errored.is_empty() || count(report, "error") != 0 {
        found.push(format!(
            "expectation 2: the report records {} errored scenario(s) (counts.error = {}): {errored:?}",
            errored.len(),
            count(report, "error")
        ));
    }

    for category in ["skipped", "unsupported"] {
        for id in outcome_ids(report, category) {
            if !named.contains(&id) {
                found.push(format!(
                    "expectation 3: the {category} scenario `{id}` is not named in ess/SKIPPED.md"
                ));
            }
        }
    }

    // A skip is for a scenario the target cannot answer; one it answers and passes is not skipped.
    for id in named {
        if passed.contains(id) {
            found.push(format!("skipped entry {id} names a passing scenario"));
        }
    }

    if report["conformance_status"] != "passed" {
        found.push(format!(
            "expectation 5: the report's conformance_status is {}; task intake-conform requires \
             `passed`",
            report["conformance_status"]
        ));
    }

    found
}

/// A copy of `report` whose `conformance_status` is `status`.
fn with_conformance_status(report: &Value, status: &str) -> Value {
    let mut copy = report.clone();
    copy["conformance_status"] = Value::from(status);
    copy
}

/// A copy of `report` that records `scenario` as one more `category` scenario.
fn with_outcome(report: &Value, category: &str, scenario: &str) -> Value {
    let mut copy = report.clone();
    copy["outcomes"][category]
        .as_array_mut()
        .unwrap_or_else(|| panic!("the report lists an `outcomes.{category}` array"))
        .push(Value::String(scenario.to_owned()));
    for counted in [category, "total"] {
        copy["counts"][counted] = Value::from(count(report, counted) + 1);
    }
    copy
}

/// What the check finds in `copy` that it does not find in `report`.
fn added_violations(report: &Value, copy: &Value, named: &BTreeSet<String>) -> Vec<String> {
    let before: BTreeSet<String> = violations(report, named).into_iter().collect();
    violations(copy, named)
        .into_iter()
        .filter(|found| !before.contains(found))
        .collect()
}

#[test]
fn ess_conformance_report() {
    let dir = scratch();
    let suite = synthesize(&dir.join("suite.json"));
    let scenarios = serde_json::from_str::<Value>(&suite).expect("the suite is JSON")["scenarios"]
        .as_object()
        .map(serde_json::Map::len)
        .expect("the suite carries an object of scenarios");
    assert!(
        scenarios > 0,
        "the suite synthesized from ess/intake/ holds no scenario"
    );

    let executed = run_suite(&suite, &dir.join("state"))
        .unwrap_or_else(|error| panic!("the suite did not run: {error}"));
    let report_path = dir.join("report.json");
    let diagnostics_path = dir.join("diagnostics.json");
    fs::write(&report_path, &executed.report).expect("write report.json");
    fs::write(&diagnostics_path, &executed.diagnostics).expect("write diagnostics.json");
    let report: Value = serde_json::from_str(&executed.report).expect("the report is JSON");

    assert_eq!(report["format"], REPORT_FORMAT, "report: {report}");
    assert_eq!(
        count(&report, "total"),
        u64::try_from(scenarios).expect("the suite fits"),
        "the report does not cover every scenario of the suite"
    );

    let skipped_md = root().join("ess/SKIPPED.md");
    let named = skipped_md_names(
        &fs::read_to_string(&skipped_md)
            .unwrap_or_else(|error| panic!("read {}: {error}", skipped_md.display())),
    );

    // Expectation 4: the check refuses a skipped or an unsupported scenario ess/SKIPPED.md does not
    // name, and names it.
    for (category, scenario) in [
        ("skipped", UNNAMED_SKIP),
        ("unsupported", UNNAMED_UNSUPPORTED),
    ] {
        assert!(
            !named.contains(scenario),
            "ess/SKIPPED.md names `{scenario}`, which expectation 4 needs unnamed"
        );
        let added = added_violations(&report, &with_outcome(&report, category, scenario), &named);
        assert!(
            added.len() == 1 && added[0].contains(scenario),
            "expectation 4: a copy of the report with the unnamed {category} scenario \
             `{scenario}` must fail the check naming it, and only it; it added {added:?}"
        );
    }

    // Expectation 6: a copy of a passing report that says `conformance_status: inconclusive` fails
    // the check naming the field, and only it.
    let passing = with_conformance_status(&report, "passed");
    let added = added_violations(
        &passing,
        &with_conformance_status(&passing, "inconclusive"),
        &named,
    );
    assert!(
        added.len() == 1
            && added[0].contains("conformance_status")
            && added[0].contains("inconclusive"),
        "expectation 6: a copy of a passing report with conformance_status `inconclusive` must \
         fail the check naming conformance_status, and only it; it added {added:?}"
    );

    // Expectations 1, 2, 3 and 5, on the report itself.
    let found = violations(&report, &named);
    assert!(
        found.is_empty(),
        "the conformance report does not pass ({} of {scenarios} scenario(s) passed):\n  {}\n\
         report: {}\ndiagnostics: {}",
        count(&report, "passed"),
        found.join("\n  "),
        report_path.display(),
        diagnostics_path.display()
    );
    println!(
        "intake conformance: {} of {scenarios} scenario(s) passed, conformance_status {}",
        count(&report, "passed"),
        report["conformance_status"]
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `task intake-conform` runs [`ess_conformance_report`], and `task check` lists `intake-conform`
/// as a step of its own.
#[test]
fn taskfile_check_runs_intake_conform() {
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
        body("intake-conform")
            .iter()
            .any(|line| *line == format!("- {INTAKE_CONFORM}")),
        "task intake-conform does not run `{INTAKE_CONFORM}`"
    );
    assert!(
        body("check")
            .iter()
            .any(|line| line == "- task: intake-conform"),
        "task check does not list `intake-conform` as its own step"
    );
}
