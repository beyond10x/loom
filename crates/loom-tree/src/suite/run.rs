//! The results of a suite's cycles and what is printed of them: one line per cycle, the two
//! summary lines of `~/beyond10x/sdc/scripts/run-finding-cycles.sh` with the waiting clause added,
//! and the `loom.tree.SuiteReport` document `--report` writes. A pending cycle is never counted as
//! expected, failed or skipped, and a cycle waiting for a story is never run.

use std::path::Path;

use loom::tree::{CheckDifference, CycleCondition, CycleResult, SuiteCycle, SuiteReport};
use loom_protocols::ProtocolCatalog;
use serde_json::{Map, Value, json};

use super::check::holds;
use super::select::{CycleView, select_values};
use loom::primitives::Decimal;
use loom::tree::{SelectItem, WaitingCycle};

/// The result of a cycle that waits for a story: pending, not run.
pub fn waiting_result(cycle: &SuiteCycle, waiting: WaitingCycle) -> CycleResult {
    CycleResult {
        name: cycle.name.clone(),
        pending: true,
        waits_for: Some(waiting),
        run: false,
        not_run_when: None,
        as_expected: false,
        difference: None,
        exit: None,
        seconds: Decimal("0.0".into()),
    }
}

/// The result of a cycle whose `when[index]` does not hold: not run, counted in neither
/// `expected` nor `pending`.
pub fn not_run_result(cycle: &SuiteCycle, index: usize) -> CycleResult {
    CycleResult {
        name: cycle.name.clone(),
        pending: cycle.pending,
        waits_for: None,
        run: false,
        not_run_when: i64::try_from(index).ok(),
        as_expected: false,
        difference: None,
        exit: None,
        seconds: Decimal("0.0".into()),
    }
}

/// Whether `condition` holds: a protocol of `catalog` (by registration or protocol id) declares
/// the action or outcome it names, or the file of `suite` it names holds its test. A protocol the
/// catalog does not hold declares nothing.
pub fn when_holds(condition: &CycleCondition, catalog: &ProtocolCatalog, suite: &Path) -> bool {
    if let (Some(protocol), Some(declares)) = (&condition.protocol, &condition.declares) {
        let Some(entry) = catalog.iter().find(|entry| {
            entry.name() == protocol.as_str()
                || entry.model.protocol.id.as_str() == protocol.as_str()
        }) else {
            return false;
        };
        let Ok(document) = serde_yaml_ng::from_str::<Value>(&entry.definition.yaml) else {
            return false;
        };
        return match (&declares.action, &declares.outcome) {
            (Some(action), _) => document["actions"].get(action.as_str()).is_some(),
            (None, Some(outcome)) => document["outcomes"].get(outcome.as_str()).is_some(),
            (None, None) => false,
        };
    }
    let (Some(file), Some(test)) = (&condition.suite, &condition.test) else {
        return false;
    };
    let item = SelectItem {
        log: None,
        records: None,
        runs: None,
        cases: None,
        doc: None,
        files: None,
        step: None,
        suite: Some(file.clone()),
        requests: None,
        calls: None,
        r#where: None,
        pick: None,
        path: None,
        follow: None,
        r#as: None,
    };
    let steps = std::collections::BTreeMap::new();
    let view = CycleView {
        work: suite,
        store: suite,
        suite,
        steps: &steps,
        catalog,
    };
    let Ok(values) = select_values(&[item], &view) else {
        return false;
    };
    let conditions = loom::tree::Conditions {
        all: vec![test.clone()],
        any_of: Vec::new(),
    };
    values.len() == 1 && holds(&values[0].value, &conditions).unwrap_or(false)
}

/// The counts of `cycles` in suite order.
pub fn summarize(cycles: Vec<CycleResult>) -> SuiteReport {
    let ran = |result: &&CycleResult| result.run;
    let expected: Vec<&CycleResult> = cycles.iter().filter(ran).filter(|r| !r.pending).collect();
    let pending = cycles
        .iter()
        .filter(|r| r.pending && (r.run || r.waits_for.is_some()))
        .count();
    SuiteReport {
        expected: i64::try_from(expected.len()).unwrap_or(i64::MAX),
        as_expected: i64::try_from(expected.iter().filter(|r| r.as_expected).count())
            .unwrap_or(i64::MAX),
        failed: expected
            .iter()
            .filter(|r| !r.as_expected)
            .map(|r| r.name.clone())
            .collect(),
        pending: i64::try_from(pending).unwrap_or(i64::MAX),
        pending_passing: cycles
            .iter()
            .filter(|r| r.pending && r.run && r.as_expected)
            .map(|r| r.name.clone())
            .collect(),
        waiting: cycles.iter().filter_map(|r| r.waits_for.clone()).collect(),
        not_run: cycles
            .iter()
            .filter(|r| r.not_run_when.is_some())
            .map(|r| r.name.clone())
            .collect(),
        cycles,
    }
}

fn names(list: &[String]) -> String {
    if list.is_empty() {
        "none".into()
    } else {
        list.join(", ")
    }
}

fn difference_text(difference: &CheckDifference) -> String {
    let mut text = format!(
        "{} ({}, {:?}): expected {}",
        difference.check, difference.kind, difference.reason, difference.expected
    );
    if let Some(found) = &difference.found {
        text.push_str(&format!(", found {found}"));
    }
    text
}

/// The line one cycle's result prints.
pub fn cycle_line(result: &CycleResult) -> String {
    let name = &result.name;
    let seconds = &result.seconds.0;
    if let Some(waiting) = &result.waits_for {
        return format!(
            "{name}: pending, waits for {} ({})",
            waiting.story, waiting.kind
        );
    }
    if let Some(index) = result.not_run_when {
        return format!("{name}: not run (when[{index}])");
    }
    match (result.pending, result.as_expected, &result.difference) {
        (false, true, _) => format!("{name}: as expected ({seconds} s)"),
        (false, false, Some(difference)) => {
            format!(
                "{name}: FAILED: {} ({seconds} s)",
                difference_text(difference)
            )
        }
        (false, false, None) => format!("{name}: FAILED ({seconds} s)"),
        (true, true, _) => format!("{name}: pending, passing ({seconds} s)"),
        (true, false, Some(difference)) => format!(
            "{name}: pending, still failing: {} ({seconds} s)",
            difference_text(difference)
        ),
        (true, false, None) => format!("{name}: pending, still failing ({seconds} s)"),
    }
}

/// The two summary lines of `report`.
pub fn summary_lines(report: &SuiteReport) -> [String; 2] {
    let still_failing: Vec<String> = report
        .cycles
        .iter()
        .filter(|r| r.pending && r.run && !r.as_expected)
        .map(|r| r.name.clone())
        .collect();
    let waiting = if report.waiting.is_empty() {
        "none".to_owned()
    } else {
        report
            .waiting
            .iter()
            .map(|w| format!("{} on {}", w.name, w.story))
            .collect::<Vec<_>>()
            .join(", ")
    };
    [
        format!(
            "cycles: {} expected, {} as expected, {} failed: {}",
            report.expected,
            report.as_expected,
            report.failed.len(),
            names(&report.failed)
        ),
        format!(
            "pending: {} ({} still failing: {}; passing, move to expected: {}; {} waiting: {})",
            report.pending,
            still_failing.len(),
            names(&still_failing),
            names(&report.pending_passing),
            report.waiting.len(),
            waiting
        ),
    ]
}

fn put(object: &mut Map<String, Value>, name: &str, value: Option<Value>) {
    if let Some(value) = value {
        object.insert(name.to_owned(), value);
    }
}

fn waiting_json(waiting: &WaitingCycle) -> Value {
    json!({"name": waiting.name, "story": waiting.story, "kind": waiting.kind})
}

fn difference_json(difference: &CheckDifference) -> Value {
    let mut object = Map::new();
    object.insert("check".into(), json!(difference.check));
    object.insert("kind".into(), json!(difference.kind));
    object.insert("reason".into(), json!(format!("{:?}", difference.reason)));
    object.insert("expected".into(), json!(difference.expected));
    put(
        &mut object,
        "found",
        difference.found.clone().map(Value::from),
    );
    Value::Object(object)
}

fn result_json(result: &CycleResult) -> Value {
    let mut object = Map::new();
    object.insert("name".into(), json!(result.name));
    object.insert("pending".into(), json!(result.pending));
    put(
        &mut object,
        "waits_for",
        result.waits_for.as_ref().map(waiting_json),
    );
    object.insert("run".into(), json!(result.run));
    put(
        &mut object,
        "not_run_when",
        result.not_run_when.map(Value::from),
    );
    object.insert("as_expected".into(), json!(result.as_expected));
    put(
        &mut object,
        "difference",
        result.difference.as_ref().map(difference_json),
    );
    put(&mut object, "exit", result.exit.map(Value::from));
    object.insert(
        "seconds".into(),
        serde_json::from_str::<Value>(&result.seconds.0).unwrap_or(json!(result.seconds.0)),
    );
    Value::Object(object)
}

/// `report` as the `loom.tree.SuiteReport` document `--report` writes, member for member.
pub fn report_json(report: &SuiteReport) -> Value {
    json!({
        "cycles": report.cycles.iter().map(result_json).collect::<Vec<_>>(),
        "expected": report.expected,
        "as_expected": report.as_expected,
        "failed": report.failed,
        "pending": report.pending,
        "pending_passing": report.pending_passing,
        "waiting": report.waiting.iter().map(waiting_json).collect::<Vec<_>>(),
        "not_run": report.not_run,
    })
}
