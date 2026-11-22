//! `task check` holds Loom's ESS specification to its synthesized conformance suite.
//!
//! [`ess_conformance_report`] reads the suite `ess verify conform synthesize --suite-format 5`
//! synthesized from `ess/`, committed under `generated/ess/loom/` once its input digest matched
//! the tree, runs it against `b10x-loom-executor` through [`LoomTarget`], and reads the verdict
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
//! The report's own `execution_status` and `conformance_status` are read too, and each must be the
//! status its counts and coverage come to by ESS's rule ([`derived_statuses`]); a copy of the
//! report with either field changed fails the check and names the field. `run_suite` builds the
//! report with `CountReport::from_run`, which refuses statuses that contradict the counts, so on
//! this target's own report the check holds by construction. The rule itself is held to ESS's
//! ratings written out as literals: the self-check copies of expectation 4 carry the statuses ESS
//! rates them, and [`derived_statuses_rate_reports_as_ess_does`] covers each branch.
//!
//! On top of that, the report must qualify: its `conformance_status` must be `passed`
//! ([`qualification`]), and a copy of a passing report that says `inconclusive` fails the check
//! naming the field. The suite is synthesized with declared coverage (`--suite-format 5`), so it
//! carries a complete coverage inventory and a run in which every scenario passed qualifies
//! `passed`; in the ordinary format the coverage is unknown and the same run is `inconclusive`.
//! A named `unsupported` scenario therefore no longer keeps the check green: ESS fails the
//! execution, and so the conformance, of any run with an unsupported scenario.
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
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "../../loom-executor/tests/support/ess_suite.rs"]
mod ess_suite;

/// The report format the verdict is read from.
const REPORT_FORMAT: &str = "ess-conformance-report/2";

/// The scenario expectation 4 adds to a copy of the report as skipped.
const UNNAMED_SKIP: &str = "loom.run.Unnamed/outcome/not-in-skipped-md";

/// The scenario expectation 4 adds to a copy of the report as unsupported.
const UNNAMED_UNSUPPORTED: &str = "loom.run.Unnamed/outcome/unsupported-not-in-skipped-md";

const EXPIRED: &str = "loom.decision/authored/expired-request-refuses-every-command";
const ESCALATED: &str = "loom.decision/authored/escalated-request-is-superseded";
const PRODUCED: &str = "loom.decision/authored/produced-record-is-recorded-once";
const KIND_GUARDS: &str = "loom.decision/authored/decide-kind-guards-under-a-rule";

/// The pinned cover table (AGENTS.md § ESS item 3): every refusal occurrence of the suite
/// synthesized from `ess/`, by the id its coverage inventory prints, with the authored scenario
/// that covers it. `task conform` fails when the suite's occurrence-to-cover map differs from it in
/// either direction, so a new cover is a change of this file, which a reviewer reads.
const COVER_TABLE: [(&str, &str); 17] = [
    (
        "loom.decision.DecisionRequest/state/Expired/refuses/loom.decision.Decide",
        EXPIRED,
    ),
    (
        "loom.decision.DecisionRequest/state/Expired/refuses/loom.decision.Escalate",
        EXPIRED,
    ),
    (
        "loom.decision.DecisionRequest/state/Expired/refuses/loom.decision.Expire",
        EXPIRED,
    ),
    (
        "loom.decision.DecisionRequest/state/Expired/refuses/loom.decision.Recommend",
        EXPIRED,
    ),
    (
        "loom.decision.DecisionRequest/state/Expired/refuses/loom.decision.RecordProduced",
        EXPIRED,
    ),
    (
        "loom.decision.DecisionRequest/state/Expired/refuses/loom.decision.Supersede",
        EXPIRED,
    ),
    (
        "loom.decision.DecisionRequest/state/Expired/refuses/loom.decision.TakeRecommendation",
        EXPIRED,
    ),
    (
        "loom.decision.DecisionRequest/transition/supersede/by/loom.decision.Supersede/superseded",
        ESCALATED,
    ),
    ("loom.decision.RecordProduced/outcome/recorded", PRODUCED),
    (
        "loom.decision.DecisionRequest/transition/record_produced/by/loom.decision.RecordProduced/recorded",
        PRODUCED,
    ),
    (
        "loom.decision.RecordProduced/outcome/already-recorded",
        PRODUCED,
    ),
    (
        "loom.decision.RecordProduced/outcome/produced-conflict",
        PRODUCED,
    ),
    // The conformance run checks no invariant after an authored scenario's steps: a
    // record_produced that leaves default_on_silence beside a weak recommendation passes it. The
    // invariant after `recorded` and after `already-recorded` is tested by
    // `the_stored_request_keeps_every_invariant_after_recorded_and_already_recorded` in
    // `crates/loom-decision/tests/decision_request.rs`.
    (
        "loom.decision.DecisionRequest/invariant/after/loom.decision.RecordProduced/recorded",
        PRODUCED,
    ),
    (
        "loom.decision.DecisionRequest/invariant/after/loom.decision.RecordProduced/already-recorded",
        PRODUCED,
    ),
    (
        "loom.decision.Decide/outcome/decider-under-rule",
        KIND_GUARDS,
    ),
    ("loom.decision.Decide/outcome/rule-not-holder", KIND_GUARDS),
    (
        "loom.decision.Decide/outcome/silence-takes-recommendation",
        KIND_GUARDS,
    ),
];

/// The authored scenarios the pinned table names, each once.
fn table_covers() -> BTreeSet<&'static str> {
    COVER_TABLE.iter().map(|(_, cover)| *cover).collect()
}

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

/// Runs `ess` with `args` in the repository root, asking for the release `ess/ess-inputs.yaml`
/// requires unless the caller set `ESS_TOOLCHAIN`, and returns what it printed, refusing a failed
/// run.
fn ess(args: &[&str]) -> String {
    let output = ess_suite::ess_program(&root())
        .command()
        .current_dir(root())
        .args(args)
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

/// The suite `ess/` obliges, synthesized with declared coverage (`--suite-format 5`), so the
/// report can qualify the run `passed` rather than `inconclusive`, and with the authored scenarios
/// `ess/ess-inputs.yaml` lists (`--scenarios ess`) beside the generated ones. Synthesizing `ess/`
/// takes minutes, so this reads the suite `task suite-generate` committed under
/// `generated/ess/loom/`, after the digest of its inputs matched this tree's `ess/`
/// (`crates/loom-executor/tests/support/ess_suite.rs`); `task slow` synthesizes again and compares
/// bytes.
fn committed_suite() -> String {
    let (record, suite) = ess_suite::committed(&root(), &ess_suite::ess_program(&root()))
        .unwrap_or_else(|detail| panic!("{detail}"));
    assert!(
        record.success(),
        "the committed synthesis of ess/ failed: {}",
        record.describe()
    );
    suite.unwrap_or_else(|| panic!("{} records no suite", ess_suite::RECORD))
}

/// The qualified name of every command the compiled specification declares.
fn compiled_commands() -> BTreeSet<String> {
    let compiled: Value = serde_json::from_str(&ess(&[
        "specify", "compile", "--path", "ess", "--format", "json",
    ]))
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

/// The authored scenarios the suite marks as covering a refusal: every `covered_by` of
/// `coverage.refused`, the scenarios a `covers:` list made count for a refusal.
fn covering_scenarios(suite: &Value) -> BTreeSet<String> {
    suite["coverage"]["refused"]
        .as_array()
        .unwrap_or_else(|| panic!("the suite carries no `coverage.refused` array"))
        .iter()
        .filter_map(|refusal| refusal["covered_by"].as_str())
        .map(str::to_owned)
        .collect()
}

/// What no `covered_by` reads as in [`cover_table_violations`].
const UNCOVERED: &str = "(no cover)";

/// Every way the suite's refusal occurrences and their covers differ from [`COVER_TABLE`], each
/// named: an occurrence (or its cover) the table lacks, and a table row the suite lacks. Two
/// scenarios for one occurrence differ in both directions. Empty means the map is the table.
fn cover_table_violations(suite: &Value) -> Vec<String> {
    let mut counts: std::collections::BTreeMap<(String, String), i64> =
        std::collections::BTreeMap::new();
    for refusal in suite["coverage"]["refused"]
        .as_array()
        .unwrap_or_else(|| panic!("the suite carries no `coverage.refused` array"))
    {
        let occurrence = refusal["scenario"]
            .as_str()
            .unwrap_or_else(|| panic!("a refusal names no scenario: {refusal}"))
            .to_owned();
        let cover = refusal["covered_by"]
            .as_str()
            .unwrap_or(UNCOVERED)
            .to_owned();
        *counts.entry((occurrence, cover)).or_default() += 1;
    }
    for (occurrence, cover) in COVER_TABLE {
        *counts
            .entry((occurrence.to_owned(), cover.to_owned()))
            .or_default() -= 1;
    }
    let mut found = Vec::new();
    for ((occurrence, cover), count) in counts {
        if count > 0 {
            found.push(format!(
                "covers: the suite's refusal `{occurrence}` covered by `{cover}` is not in the \
                 pinned table (conform.rs COVER_TABLE)"
            ));
        }
        if count < 0 {
            found.push(format!(
                "covers: the pinned table row `{occurrence}` covered by `{cover}` is not in the \
                 suite"
            ));
        }
    }
    found
}

/// A copy of `suite` with one more refusal occurrence, `occurrence`, covered by `cover`: the
/// form of a scenario that claims a refusal it does not test (security review F4, `sec1/e3`).
fn with_refusal(suite: &Value, occurrence: &str, cover: &str) -> Value {
    let mut copy = suite.clone();
    copy["coverage"]["refused"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("the suite carries a `coverage.refused` array"))
        .push(serde_json::json!({
            "code": "ESS-SYNTH-003",
            "scenario": occurrence,
            "covered_by": cover,
            "scope": "in_scope",
        }));
    copy
}

/// A copy of `suite` in which the refusal `occurrence` is covered by `cover` instead.
fn with_cover(suite: &Value, occurrence: &str, cover: &str) -> Value {
    let mut copy = suite.clone();
    let refusal = copy["coverage"]["refused"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("the suite carries a `coverage.refused` array"))
        .iter_mut()
        .find(|refusal| refusal["scenario"] == occurrence)
        .unwrap_or_else(|| panic!("the suite refuses `{occurrence}`"));
    refusal["covered_by"] = Value::from(cover);
    copy
}

/// Every covering scenario the report does not record as executed (passed or failed), each named
/// with the category the report lists it under. A cover counts only when the run executes it: one
/// reported skipped or unsupported, or not reported, leaves its refusal unanswered, whatever
/// `ess/SKIPPED.md` says.
fn unexecuted_covers(covers: &BTreeSet<String>, report: &Value) -> Vec<String> {
    let executed: BTreeSet<String> = ["passed", "failed"]
        .into_iter()
        .flat_map(|category| outcome_ids(report, category))
        .collect();
    covers
        .iter()
        .filter(|cover| !executed.contains(*cover))
        .map(|cover| {
            let listed = ["error", "unsupported", "skipped"]
                .into_iter()
                .find(|category| outcome_ids(report, category).contains(cover))
                .unwrap_or("not reported");
            format!(
                "covers: the covering scenario `{cover}` is not executed (report: {listed}); \
                 a refusal it covers counts only when the run executes it"
            )
        })
        .collect()
}

/// A copy of `report` in which `scenario` is no longer passed but listed under `category`, with
/// the counts moved.
fn moved_to(report: &Value, scenario: &str, category: &str) -> Value {
    let mut copy = report.clone();
    let passed = copy["outcomes"]["passed"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("the report lists an `outcomes.passed` array"));
    let before = passed.len();
    passed.retain(|id| id != scenario);
    assert_eq!(
        passed.len() + 1,
        before,
        "`{scenario}` is passed once in the report"
    );
    copy["counts"]["passed"] = Value::from(count(report, "passed") - 1);
    copy["outcomes"][category]
        .as_array_mut()
        .unwrap_or_else(|| panic!("the report lists an `outcomes.{category}` array"))
        .push(Value::String(scenario.to_owned()));
    copy["counts"][category] = Value::from(count(report, category) + 1);
    copy
}

/// The qualification `task conform` requires: the report's `conformance_status` is `passed`. A
/// status that agrees with its counts is not enough; an `inconclusive` run, such as one over a
/// suite that declares no coverage inventory, does not pass. Empty means the report qualifies.
fn qualification(report: &Value) -> Vec<String> {
    if report["conformance_status"] == "passed" {
        Vec::new()
    } else {
        vec![format!(
            "the report's conformance_status is {}; task conform requires `passed`",
            report["conformance_status"]
        )]
    }
}

/// The `execution_status` and `conformance_status` a report's counts and coverage come to, by the
/// rule ESS writes them with (ess-conformance `counts.rs`, `execution` and `qualification`).
///
/// Execution fails on a failed or unsupported scenario, is inconclusive on an errored or skipped
/// one, and passes otherwise. Conformance fails when execution fails, passes when execution passes
/// over a suite that is not empty and whose coverage is a complete inventory with no in-scope
/// refusal left uncovered, and is inconclusive otherwise. A refusal is covered when its
/// `covered_by` names an authored scenario the run selected (`coverage.rs` `Inventory::covers`):
/// one the report lists under any category.
fn derived_statuses(report: &Value) -> [&'static str; 2] {
    let execution = if count(report, "failed") > 0 || count(report, "unsupported") > 0 {
        "failed"
    } else if count(report, "error") > 0 || count(report, "skipped") > 0 {
        "inconclusive"
    } else {
        "passed"
    };
    let selected = |id: &str| {
        CATEGORIES.into_iter().any(|category| {
            report["outcomes"][category]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|listed| listed == id))
        })
    };
    let coverage = &report["coverage"];
    let complete = coverage["knowledge"] == "complete_inventory"
        && !coverage["refused"].as_array().is_some_and(|refused| {
            refused.iter().any(|one| {
                one["scope"] == "in_scope" && !one["covered_by"].as_str().is_some_and(selected)
            })
        });
    let conformance = match execution {
        "failed" => "failed",
        "passed" if count(report, "total") > 0 && complete => "passed",
        _ => "inconclusive",
    };
    [execution, conformance]
}

/// Every category a report lists its scenarios under.
const CATEGORIES: [&str; 5] = ["passed", "failed", "error", "unsupported", "skipped"];

/// What ESS rates a run in which every scenario passed over a suite that declares a complete
/// coverage inventory with no refusal: execution passes (`counts.rs` `execution`), and so does
/// conformance (`counts.rs` `qualification`).
const ALL_PASSED: [&str; 2] = ["passed", "passed"];

/// What ESS rates [`ALL_PASSED`] with one unsupported scenario more: any unsupported scenario fails
/// execution, and a failed execution fails conformance.
const ONE_UNSUPPORTED: [&str; 2] = ["failed", "failed"];

/// What ESS rates [`ALL_PASSED`] with one skipped scenario more, under a producer profile that has
/// the category: a skipped scenario makes execution inconclusive, and so conformance. (ESS refuses
/// a Rust-profile report with a skipped scenario outright, `counts.rs` `execution`.)
const ONE_SKIPPED: [&str; 2] = ["inconclusive", "inconclusive"];

/// A copy of `report` in which every scenario it lists passed, with the coverage of a suite that
/// declares a complete inventory and the statuses [`ALL_PASSED`]: the fixed base the self-checks
/// start from, so the statuses they expect are literals, not whatever the run came to.
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
    base["coverage"] = serde_json::json!({"knowledge": "complete_inventory", "refused": []});
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
    let suite_text = committed_suite();
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

    // The qualification: a copy of a passing report that says `conformance_status: inconclusive`
    // fails the check, and every violation it adds names the field.
    assert!(
        qualification(&base).is_empty(),
        "the check refuses a run in which every scenario passed over a complete inventory: {:?}",
        qualification(&base)
    );
    let inconclusive = {
        let mut copy = base.clone();
        copy["conformance_status"] = Value::from("inconclusive");
        copy
    };
    let added: Vec<String> = added_violations(&base, &inconclusive, &named)
        .into_iter()
        .chain(qualification(&inconclusive))
        .collect();
    assert!(
        added
            .iter()
            .any(|found| found.contains("conformance_status") && found.contains("inconclusive"))
            && added
                .iter()
                .all(|found| found.contains("conformance_status")),
        "a copy of a passing report with conformance_status `inconclusive` must fail the check \
         naming conformance_status; it added {added:?}"
    );

    // The pinned cover table: the suite's occurrence-to-cover map is COVER_TABLE. A copy of the
    // suite in which a fourth scenario claims a refusal, a new one or one the table gives
    // another, fails the check naming it, and a copy that drops a row names the row.
    const FOURTH: &str = "loom.decision/authored/vacuous-cover";
    const PROBE: &str = "loom.run.Probe/outcome/never";
    let claimed = cover_table_violations(&with_refusal(&suite, PROBE, FOURTH));
    assert!(
        claimed.len() == 1 && claimed[0].contains(PROBE) && claimed[0].contains(FOURTH),
        "covers: a copy of the suite in which `{FOURTH}` covers `{PROBE}` must fail the table \
         check naming it; it found {claimed:?}"
    );
    let (taken, _) = COVER_TABLE[0];
    let moved = cover_table_violations(&with_cover(&suite, taken, FOURTH));
    assert!(
        moved.len() == 2
            && moved.iter().any(|found| found.contains(FOURTH))
            && moved.iter().all(|found| found.contains(taken)),
        "covers: a copy of the suite in which `{FOURTH}` covers `{taken}` must fail the table \
         check naming both; it found {moved:?}"
    );
    found.extend(cover_table_violations(&suite));

    // Covers: every authored scenario that covers a refusal is executed. The covers the pinned
    // table names are among them, and a copy of the report in which one of them is skipped,
    // unsupported or not reported fails the check naming it.
    let covers = covering_scenarios(&suite);
    for cover in table_covers() {
        assert!(
            covers.contains(cover),
            "the suite marks no refusal covered by `{cover}`: {covers:?}"
        );
        for category in ["skipped", "unsupported"] {
            let copy = moved_to(&base, cover, category);
            let added = unexecuted_covers(&covers, &copy);
            assert!(
                added.len() == 1 && added[0].contains(cover) && added[0].contains(category),
                "covers: a copy of the report with `{cover}` {category} must fail the check \
                 naming it; it found {added:?}"
            );
        }
        let mut dropped = moved_to(&base, cover, "skipped");
        dropped["outcomes"]["skipped"]
            .as_array_mut()
            .expect("skipped array")
            .retain(|id| id != cover);
        let added = unexecuted_covers(&covers, &dropped);
        assert!(
            added.len() == 1 && added[0].contains(cover) && added[0].contains("not reported"),
            "covers: a copy of the report without `{cover}` must fail the check naming it; it \
             found {added:?}"
        );
    }
    found.extend(unexecuted_covers(&covers, &report));

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

    // Expectations 1, 3 and 4, the two statuses and the qualification, on the report itself.
    found.extend(violations(&report, &named));
    found.extend(qualification(&report));
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
/// `categories`, the `n`th listed as `<category>-<n>`, and `coverage`.
fn rated_report(categories: &[&str], coverage: Value) -> Value {
    let mut counts = serde_json::Map::new();
    let mut outcomes = serde_json::Map::new();
    for category in CATEGORIES {
        let listed: Vec<Value> = categories
            .iter()
            .filter(|listed| **listed == category)
            .enumerate()
            .map(|(n, _)| Value::from(format!("{category}-{n}")))
            .collect();
        counts.insert(category.to_owned(), Value::from(listed.len()));
        outcomes.insert(category.to_owned(), Value::from(listed));
    }
    counts.insert("total".to_owned(), Value::from(categories.len()));
    serde_json::json!({"counts": counts, "outcomes": outcomes, "coverage": coverage})
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
    let refused_covered = serde_json::json!({
        "knowledge": "complete_inventory",
        "refused": [{"scope": "in_scope", "covered_by": "passed-0"}]
    });
    let refused_covered_unselected = serde_json::json!({
        "knowledge": "complete_inventory",
        "refused": [{"scope": "in_scope", "covered_by": "not-selected"}]
    });
    let unknown = serde_json::json!({"knowledge": "unknown"});
    let cases: [(&str, &[&str], &Value, [&str; 2]); 11] = [
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
            "every scenario passed, an in-scope refusal covered by a selected authored scenario",
            &["passed"],
            &refused_covered,
            ["passed", "passed"],
        ),
        (
            "every scenario passed, an in-scope refusal covered by a scenario not selected",
            &["passed"],
            &refused_covered_unselected,
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
