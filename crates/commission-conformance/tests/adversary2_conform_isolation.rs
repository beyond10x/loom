//! Adversary pass 2 on `story:commission-ess-conformance`: scenario isolation and determinism.
//!
//! The crate doc says each scenario starts from an empty store, an empty event log and no forced
//! outcome, and that two runs of one suite report the same thing. These cases add scenarios to the
//! synthesized suite that would pass only if one scenario's state leaked into the next, each with a
//! control that shows the same assertions fail when the state is present in the scenario itself.

use b10x_commission_conformance::run_suite;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Command;

const ADMITTED: &str = "commission.responsibility.RevalidateActionRequest/outcome/admitted";
const STALE: &str = "commission.responsibility.RevalidateActionRequest/outcome/stale";
const STARTED: &str = "commission.responsibility.StartRun/outcome/started";
const RESUME_UNKNOWN: &str = "commission.responsibility.ResumeRun/outcome/wrong-state";
/// The id `CommissionTarget`'s counter gives the first run of a scenario.
const FIRST_RUN: &str = "00000000-0000-4000-9000-000000000001";

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    PathBuf::from(manifest).join("../..")
}

fn synthesized(tag: &str) -> Value {
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "adversary2-isolation-{tag}-{}.json",
        std::process::id()
    ));
    let output = Command::new("ess")
        .current_dir(root())
        .args(["verify", "conform", "synthesize", "--path", "ess", "--out"])
        .arg(&out)
        .output()
        .unwrap_or_else(|error| panic!("`ess` must be on PATH: {error}"));
    assert!(
        output.status.success(),
        "synthesize exited {}",
        output.status
    );
    let text = std::fs::read_to_string(&out).expect("read suite");
    let _ = std::fs::remove_file(&out);
    serde_json::from_str(&text).expect("suite is JSON")
}

fn steps(suite: &Value, scenario: &str) -> Vec<Value> {
    suite["scenarios"][scenario]["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("`{scenario}` has no steps"))
        .clone()
}

/// Adds scenario `id`, a copy of `like` with `steps`.
fn add(suite: &mut Value, id: &str, like: &str, steps: Vec<Value>) {
    let mut scenario = suite["scenarios"][like].clone();
    scenario["purpose"] = json!(format!("adversary pass 2: {id}"));
    scenario["steps"] = Value::Array(steps);
    suite["scenarios"][id] = scenario;
}

/// The report's outcomes after running `suite`, with the diagnostics for a reader of a failure.
fn run(suite: &Value) -> (Value, String) {
    let executed = run_suite(&suite.to_string()).unwrap_or_else(|e| panic!("did not run: {e}"));
    let report: Value = serde_json::from_str(&executed.report).expect("report is JSON");
    (report, executed.diagnostics)
}

fn listed(report: &Value, category: &str, id: &str) -> bool {
    report["outcomes"][category]
        .as_array()
        .is_some_and(|ids| ids.iter().any(|listed| listed == id))
}

/// A `stale` armed in one scenario and never consumed must not reach the next scenario's
/// revalidation. Control: the same arming inside the admitted scenario makes it fail.
#[test]
fn adversary2_an_armed_outcome_does_not_outlive_its_scenario() {
    let mut suite = synthesized("armed");
    let arm = steps(&suite, STALE)[0].clone();
    assert_eq!(arm["step"], "configure_external_outcome", "{arm}");
    // Sorts before `…/admitted`, so the runner reaches it first.
    let leak = "commission.responsibility.RevalidateActionRequest/outcome/a-arms-and-leaves";
    add(&mut suite, leak, STALE, vec![arm.clone()]);
    let control = "commission.responsibility.RevalidateActionRequest/outcome/zz-armed-control";
    let mut armed_admitted = vec![arm];
    armed_admitted.extend(steps(&suite, ADMITTED));
    add(&mut suite, control, ADMITTED, armed_admitted);

    let (report, diagnostics) = run(&suite);
    assert!(
        listed(&report, "failed", control),
        "control: an armed `stale` inside the admitted scenario must fail it: {}",
        report["outcomes"]
    );
    assert!(
        listed(&report, "passed", ADMITTED),
        "`{ADMITTED}` did not pass after `{leak}` armed `stale`: {}\n{diagnostics}",
        report["outcomes"]
    );
}

/// The runs and events of earlier scenarios must not be visible to a later one. Control: the same
/// assertions after a `StartRun` in the scenario itself fail.
#[test]
fn adversary2_runs_and_events_do_not_outlive_their_scenario() {
    let mut suite = synthesized("runs");
    let start = steps(&suite, STARTED)[0].clone();
    assert_eq!(start["step"], "execute_command", "{start}");
    // `expect_no_event` needs a command before it: a resume of an unknown run, which publishes nothing.
    let resume_unknown = steps(&suite, RESUME_UNKNOWN)[0].clone();
    assert_eq!(
        resume_unknown["step"], "execute_command",
        "{resume_unknown}"
    );
    let view = "commission.responsibility.RunStates";
    let assertions = vec![
        json!({"step": "query_view", "view": view}),
        json!({"step": "expect_view", "view": view, "expectation": {
            "expect": "excludes",
            "fields": {"run_id": {"kind": "literal", "value": FIRST_RUN}},
        }}),
        resume_unknown,
        json!({"step": "expect_no_event", "event": "commission.responsibility.RunStarted"}),
        json!({"step": "expect_no_event", "event": "commission.responsibility.RunSuspended"}),
    ];
    // Both sort after every scenario that starts a run.
    let control = "commission.responsibility.SuspendRun/outcome/zz-a-started-control";
    let mut started = vec![start];
    started.extend(assertions.clone());
    add(&mut suite, control, STARTED, started);
    let leak = "commission.responsibility.SuspendRun/outcome/zz-b-after-earlier-runs";
    add(&mut suite, leak, STARTED, assertions);

    let (report, diagnostics) = run(&suite);
    assert!(
        listed(&report, "failed", control),
        "control: the assertions must fail after a StartRun in the same scenario: {}",
        report["outcomes"]
    );
    assert!(
        listed(&report, "passed", leak),
        "`{leak}` saw a run or an event of an earlier scenario: {}\n{diagnostics}",
        report["outcomes"]
    );
}

/// Two runs of one suite report the same thing (crate doc, `lib.rs`).
#[test]
fn adversary2_two_runs_of_one_suite_report_the_same() {
    let suite = synthesized("twice").to_string();
    let first = run_suite(&suite).expect("first run");
    let second = run_suite(&suite).expect("second run");
    assert_eq!(first.report, second.report);
}
