//! `task check` holds Commission's ESS specification to its synthesized conformance suite.
//!
//! [`ess_conformance_report`] synthesizes the suite from `ess/` with `ess verify conform
//! synthesize`, runs it against `b10x-commission` through [`CommissionTarget`], and reads the
//! verdict from the `ess-conformance-report/2` document, never from an exit code:
//!
//! 1. the report records at least one passed scenario;
//! 2. the report records no failed scenario;
//! 3. every skipped scenario in the report is named in `ess/SKIPPED.md`;
//! 4. the same check, applied to a copy of the report that adds one skipped scenario `S` that
//!    `ess/SKIPPED.md` does not name, fails and names `S`.
//!
//! The Rust producer profile has no `skipped` category: a scenario the target cannot answer is
//! reported `unsupported`. The check therefore holds `unsupported` to the same rule as `skipped`
//! (named in `ess/SKIPPED.md` or refused), and refuses `error` outright, since an execution
//! failure hides whatever the scenario would have shown. It also refuses an `ess/SKIPPED.md` entry
//! naming a scenario the report records as passed.
//!
//! [`CommissionTarget`]: b10x_commission_conformance::CommissionTarget

use b10x_commission_conformance::run_suite;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// The report format the verdict is read from.
const REPORT_FORMAT: &str = "ess-conformance-report/2";

/// The scenario expectation 4 adds to a copy of the report as skipped.
const UNNAMED_SKIP: &str = "commission.responsibility.Unnamed/outcome/not-in-skipped-md";

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

/// The suite `ess/` obliges, synthesized fresh.
fn synthesize(out: &Path) -> String {
    let output = Command::new("ess")
        .current_dir(root())
        .args(["verify", "conform", "synthesize", "--path", "ess", "--out"])
        .arg(out)
        .output()
        .unwrap_or_else(|error| panic!("`ess` must be on PATH: {error}"));
    assert!(
        output.status.success(),
        "`ess verify conform synthesize --path ess` exited {}:\n{}{}",
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
    let suite = synthesize(&dir.join("suite.json"));
    let scenarios = serde_json::from_str::<Value>(&suite).expect("the suite is JSON")["scenarios"]
        .as_object()
        .map(serde_json::Map::len)
        .expect("the suite carries an object of scenarios");
    assert!(
        scenarios > 0,
        "the suite synthesized from ess/ holds no scenario"
    );

    let executed =
        run_suite(&suite).unwrap_or_else(|error| panic!("the suite did not run: {error}"));
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

    // Expectations 1, 2 and 3, on the report itself.
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
    let _ = fs::remove_dir_all(&dir);
}
