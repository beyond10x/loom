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
//! as `skipped` (named in `ess/SKIPPED.md` or refused), and expectation 4's self-check runs for
//! both: a copy that adds an unnamed unsupported scenario `U` fails and names `U`, as `S` does. It
//! refuses `error` outright, since an execution failure hides whatever the scenario would have
//! shown, and it refuses an `ess/SKIPPED.md` entry naming a scenario the report records as passed.
//!
//! So the check passes when every scenario passed or is named in `ess/SKIPPED.md`, and only then.
//! A named `unsupported` scenario keeps it green while ESS rates the run `failed`: ESS fails the
//! execution of any run with an unsupported scenario. That is decided, not overlooked: acceptance
//! item 4 admits a named scenario, and the Rust producer has no other category for one.
//!
//! The report's own `execution_status` and `conformance_status` are read too, and each must be the
//! status its counts and coverage come to by ESS's rule ([`derived_statuses`]); a copy of the
//! report with either field changed fails the check and names the field. `run_suite` builds the
//! report with `CountReport::from_run`, which refuses statuses that contradict the counts, so on
//! this target's own report the check holds by construction. The rule itself is held to ESS's
//! ratings written out as literals: the self-check copies of expectation 4 carry the statuses ESS
//! rates them, and [`derived_statuses_rate_reports_as_ess_does`] covers each branch.
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

/// The scenario expectation 4 adds to a copy of the report as unsupported.
const UNNAMED_UNSUPPORTED: &str = "loom.run.Unnamed/outcome/unsupported-not-in-skipped-md";

/// The two statuses a report carries, each checked against what its counts and coverage come to.
const STATUSES: [&str; 2] = ["execution_status", "conformance_status"];

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

    let derived = derived_statuses(report);
    for (field, status) in STATUSES.into_iter().zip(derived) {
        if report[field] != status {
            found.push(format!(
                "the report's {field} is {}, and its counts and coverage come to `{status}`",
                report[field]
            ));
        }
    }

    found
}

/// The `execution_status` and `conformance_status` a report's counts and coverage come to, by the
/// rule ESS writes them with (ess-conformance `counts.rs`, `execution` and `qualification`).
///
/// Execution fails on a failed or unsupported scenario, is inconclusive on an errored or skipped
/// one, and passes otherwise. Conformance fails when execution fails, passes when execution passes
/// over a suite that is not empty and whose coverage is a complete inventory with no in-scope
/// refusal, and is inconclusive otherwise.
fn derived_statuses(report: &Value) -> [&'static str; 2] {
    let execution = if count(report, "failed") > 0 || count(report, "unsupported") > 0 {
        "failed"
    } else if count(report, "error") > 0 || count(report, "skipped") > 0 {
        "inconclusive"
    } else {
        "passed"
    };
    let coverage = &report["coverage"];
    let complete = coverage["knowledge"] == "complete_inventory"
        && !coverage["refused"]
            .as_array()
            .is_some_and(|refused| refused.iter().any(|one| one["scope"] == "in_scope"));
    let conformance = match execution {
        "failed" => "failed",
        "passed" if count(report, "total") > 0 && complete => "passed",
        _ => "inconclusive",
    };
    [execution, conformance]
}

/// Every category a report lists its scenarios under.
const CATEGORIES: [&str; 5] = ["passed", "failed", "error", "unsupported", "skipped"];

/// What ESS rates a run in which every scenario passed over a suite that declares no coverage
/// inventory: execution passes (`counts.rs` `execution`), and conformance is inconclusive because
/// the coverage is not a complete inventory (`counts.rs` `qualification`).
const ALL_PASSED: [&str; 2] = ["passed", "inconclusive"];

/// What ESS rates [`ALL_PASSED`] with one unsupported scenario more: any unsupported scenario fails
/// execution, and a failed execution fails conformance.
const ONE_UNSUPPORTED: [&str; 2] = ["failed", "failed"];

/// What ESS rates [`ALL_PASSED`] with one skipped scenario more, under a producer profile that has
/// the category: a skipped scenario makes execution inconclusive, and so conformance. (ESS refuses
/// a Rust-profile report with a skipped scenario outright, `counts.rs` `execution`.)
const ONE_SKIPPED: [&str; 2] = ["inconclusive", "inconclusive"];

/// A copy of `report` in which every scenario it lists passed, with the coverage of a suite that
/// declares no inventory and the statuses [`ALL_PASSED`]: the fixed base expectation 4's
/// self-checks start from, so the statuses they expect are literals, not whatever the run came to.
fn all_passed(report: &Value) -> Value {
    let mut base = report.clone();
    let mut ids: Vec<String> = CATEGORIES
        .into_iter()
        .flat_map(|category| outcome_ids(report, category))
        .collect();
    ids.sort();
    for category in CATEGORIES {
        base["outcomes"][category] = Value::Array(Vec::new());
        base["counts"][category] = Value::from(0);
    }
    let total = ids.len();
    base["outcomes"]["passed"] = Value::from(ids);
    base["counts"]["passed"] = Value::from(total);
    base["counts"]["total"] = Value::from(total);
    base["coverage"] = serde_json::json!({"knowledge": "unknown"});
    for (field, status) in STATUSES.into_iter().zip(ALL_PASSED) {
        base[field] = Value::from(status);
    }
    base
}

/// A copy of `report` that records `scenario` as one more `category` scenario and carries
/// `statuses`, the statuses ESS rates that copy.
fn with_outcome(report: &Value, category: &str, scenario: &str, statuses: [&str; 2]) -> Value {
    let mut copy = report.clone();
    copy["outcomes"][category]
        .as_array_mut()
        .unwrap_or_else(|| panic!("the report lists an `outcomes.{category}` array"))
        .push(Value::String(scenario.to_owned()));
    for counted in [category, "total"] {
        copy["counts"][counted] = Value::from(count(report, counted) + 1);
    }
    for (field, status) in STATUSES.into_iter().zip(statuses) {
        copy[field] = Value::from(status);
    }
    copy
}

/// A copy of `report` whose `field` says another status than it does.
fn with_status_changed(report: &Value, field: &str) -> Value {
    let mut copy = report.clone();
    copy[field] = Value::from(if report[field] == "passed" {
        "failed"
    } else {
        "passed"
    });
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

    // Expectation 4: the check refuses a skipped or an unsupported scenario ess/SKIPPED.md does not
    // name, and names it. The copies start from a run in which every scenario passed, so the
    // statuses ESS rates each one are literals, and the check's own rule is held to them.
    let base = all_passed(&report);
    assert_eq!(
        derived_statuses(&base),
        ALL_PASSED,
        "the check rates a run in which every scenario passed otherwise than ESS does"
    );
    for (category, scenario, rated) in [
        ("skipped", UNNAMED_SKIP, ONE_SKIPPED),
        ("unsupported", UNNAMED_UNSUPPORTED, ONE_UNSUPPORTED),
    ] {
        assert!(
            !named.contains(scenario),
            "ess/SKIPPED.md names `{scenario}`, which expectation 4 needs unnamed"
        );
        let copy = with_outcome(&base, category, scenario, rated);
        assert_eq!(
            derived_statuses(&copy),
            rated,
            "the check rates a run with one {category} scenario more otherwise than ESS does"
        );
        let added = added_violations(&base, &copy, &named);
        assert!(
            added.len() == 1 && added[0].contains(scenario),
            "expectation 4: a copy of the report with the unnamed {category} scenario `{scenario}` \
             must fail the check naming it, and only it; it added {added:?}"
        );
    }

    // The statuses: a copy whose status disagrees with its counts and coverage fails, naming it.
    for field in STATUSES {
        let added = added_violations(&report, &with_status_changed(&report, field), &named);
        assert!(
            added.len() == 1 && added[0].contains(field),
            "a copy of the report whose {field} disagrees with its counts must fail the check \
             naming {field}, and only it; it added {added:?}"
        );
    }

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

    // Expectations 1, 3 and 4, and the two statuses, on the report itself.
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

/// A report reduced to what [`derived_statuses`] reads: one scenario in each category of
/// `categories`, and `coverage`.
fn rated_report(categories: &[&str], coverage: Value) -> Value {
    let mut counts = serde_json::Map::new();
    for category in CATEGORIES {
        let listed = categories
            .iter()
            .filter(|listed| **listed == category)
            .count();
        counts.insert(category.to_owned(), Value::from(listed));
    }
    counts.insert("total".to_owned(), Value::from(categories.len()));
    serde_json::json!({"counts": counts, "coverage": coverage})
}

/// [`derived_statuses`] rates each report as ESS does (ess-conformance `counts.rs`, `execution`,
/// `qualification` and `coverage.rs` `Inventory::is_complete`), each expectation written out.
#[test]
fn derived_statuses_rate_reports_as_ess_does() {
    let complete = serde_json::json!({"knowledge": "complete_inventory", "refused": []});
    let refused_in_scope = serde_json::json!({
        "knowledge": "complete_inventory",
        "refused": [{"scope": "in_scope"}]
    });
    let refused_outside = serde_json::json!({
        "knowledge": "complete_inventory",
        "refused": [{"scope": "outside_component"}]
    });
    let unknown = serde_json::json!({"knowledge": "unknown"});
    let cases: [(&str, &[&str], &Value, [&str; 2]); 9] = [
        (
            "every scenario passed, complete inventory",
            &["passed"],
            &complete,
            ["passed", "passed"],
        ),
        (
            "every scenario passed, an in-scope refusal",
            &["passed"],
            &refused_in_scope,
            ["passed", "inconclusive"],
        ),
        (
            "every scenario passed, a refusal outside the component",
            &["passed"],
            &refused_outside,
            ["passed", "passed"],
        ),
        (
            "every scenario passed, no inventory",
            &["passed"],
            &unknown,
            ["passed", "inconclusive"],
        ),
        (
            "an empty suite, complete inventory",
            &[],
            &complete,
            ["passed", "inconclusive"],
        ),
        (
            "an unsupported scenario",
            &["passed", "unsupported"],
            &complete,
            ["failed", "failed"],
        ),
        (
            "a failed scenario",
            &["passed", "failed"],
            &complete,
            ["failed", "failed"],
        ),
        (
            "an errored scenario",
            &["passed", "error"],
            &complete,
            ["inconclusive", "inconclusive"],
        ),
        (
            "a skipped scenario, under a profile that has the category",
            &["passed", "skipped"],
            &complete,
            ["inconclusive", "inconclusive"],
        ),
    ];
    let wrong: Vec<String> = cases
        .into_iter()
        .filter_map(|(case, categories, coverage, rated)| {
            let derived = derived_statuses(&rated_report(categories, coverage.clone()));
            (derived != rated)
                .then(|| format!("{case}: ESS rates {rated:?}, the check {derived:?}"))
        })
        .collect();
    assert!(
        wrong.is_empty(),
        "the check rates reports otherwise than ESS:\n  {}",
        wrong.join("\n  ")
    );
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
