//! Adversary cases for `story:handoff-holds-no-suspension` (wave 2026-10-13-w3, U1): the
//! conformance scenario forcing `HoldSuspension` `settled`.
//!
//! The specification says a settled hold leaves nothing held: `ess/commission/domains/
//! responsibility.yaml`, on `commission.responsibility.Suspension` ("no Suspension is stored, none
//! is left for an approval to name") and on `HoldSuspension` ("then nothing is stored"). The
//! scenario ESS synthesizes for the outcome checks the error's fields and that no event is
//! published, and never reads the store: a target that answers `settled` and stores the Suspension
//! anyway, silently, passes it. These cases show the gap and that the target, read, meets what the
//! scenario leaves unsaid.

use b10x_loom_commission_conformance::run_suite;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Command;

const SETTLED: &str = "commission.responsibility.HoldSuspension/outcome/settled";
const HELD: &str = "commission.responsibility.HoldSuspension/outcome/held";
const VIEW: &str = "commission.responsibility.SuspensionStates";

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    PathBuf::from(manifest).join("../..")
}

fn synthesized(tag: &str) -> Value {
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adversary-w8-u1-{tag}-{}.json", std::process::id()));
    let output = Command::new("ess")
        .current_dir(root())
        .args([
            "verify",
            "conform",
            "synthesize",
            "--path",
            "ess/commission",
            "--suite-format",
            "5",
            "--out",
        ])
        .arg(&out)
        .output()
        .unwrap_or_else(|error| panic!("`ess` must be on PATH: {error}"));
    assert!(
        output.status.success(),
        "synthesize exited {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
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

/// The `action_request_id` the scenario's `HoldSuspension` sends.
fn held_request(steps: &[Value]) -> Value {
    steps
        .iter()
        .find(|step| step["step"] == "execute_command")
        .map(|step| step["input"]["action_request_id"]["value"].clone())
        .filter(Value::is_string)
        .unwrap_or_else(|| panic!("no HoldSuspension with a literal request in {steps:?}"))
}

/// The steps that read the store after the hold and expect it not to hold the request.
fn nothing_held(request: &Value) -> Vec<Value> {
    vec![
        json!({"step": "query_view", "view": VIEW}),
        json!({"step": "expect_view", "view": VIEW, "expectation": {
            "expect": "excludes",
            "fields": {"action_request_id": {"kind": "literal", "value": request}},
        }}),
    ]
}

/// Adds scenario `id` with `steps`, a copy of the `held` scenario (whose sources name the view)
/// with the `settled` scenario's sources added.
fn add(suite: &mut Value, id: &str, steps: Vec<Value>) {
    let mut scenario = suite["scenarios"][HELD].clone();
    let mut sources = scenario["source"].as_array().cloned().unwrap_or_default();
    sources.extend(
        suite["scenarios"][SETTLED]["source"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
    );
    scenario["source"] = Value::Array(sources);
    scenario["purpose"] = json!(format!("adversary w8 U1: {id}"));
    scenario["steps"] = Value::Array(steps);
    suite["scenarios"][id] = scenario;
}

fn listed(report: &Value, category: &str, id: &str) -> bool {
    report["outcomes"][category]
        .as_array()
        .is_some_and(|ids| ids.iter().any(|listed| listed == id))
}

/// The synthesized `settled` scenario holds the target to the specification's "nothing is
/// stored": after the hold it reads `SuspensionStates` and expects the request absent. It does
/// not: it checks the error and the absence of events only, so a target that stores the
/// Suspension and answers `settled` passes. Red until a scenario (authored in `ess/commission/`,
/// or synthesized by a newer ESS) reads the store after a settled hold.
#[test]
fn the_settled_scenario_reads_the_store_and_finds_nothing_held() {
    let suite = synthesized("shape");
    let settled = steps(&suite, SETTLED);
    let reads_the_store = settled.windows(2).any(|pair| {
        pair[0]["step"] == "query_view"
            && pair[0]["view"] == VIEW
            && pair[1]["step"] == "expect_view"
            && pair[1]["view"] == VIEW
            && pair[1]["expectation"]["expect"] == "excludes"
    });
    let authored = suite["coverage"]["authored"]
        .as_array()
        .is_some_and(|ids| !ids.is_empty());
    assert!(
        reads_the_store,
        "`{SETTLED}` never reads {VIEW} after the hold (authored scenarios: {authored}); its \
         steps:\n{}",
        serde_json::to_string_pretty(&settled).unwrap_or_default()
    );
}

/// What the scenario leaves unsaid, checked: the `settled` scenario with a read of the store
/// added passes against `CommissionTarget`, and the same read after a `held` hold fails it, so the
/// read can fail.
#[test]
fn a_settled_hold_leaves_the_conformance_store_empty() {
    let mut suite = synthesized("store");
    let settled = steps(&suite, SETTLED);
    let held = steps(&suite, HELD);

    let checked = "commission.responsibility.HoldSuspension/outcome/zz-settled-holds-nothing";
    let mut settled_and_read = settled.clone();
    settled_and_read.extend(nothing_held(&held_request(&settled)));
    add(&mut suite, checked, settled_and_read);

    let control = "commission.responsibility.HoldSuspension/outcome/zz-held-control";
    let mut held_and_read = held.clone();
    held_and_read.extend(nothing_held(&held_request(&held)));
    add(&mut suite, control, held_and_read);

    let executed = run_suite(&suite.to_string()).unwrap_or_else(|e| panic!("did not run: {e}"));
    let report: Value = serde_json::from_str(&executed.report).expect("report is JSON");
    assert!(
        listed(&report, "failed", control),
        "control: a held Suspension must fail the read: {}",
        report["outcomes"]
    );
    assert!(
        listed(&report, "passed", checked),
        "a settled hold left the request held: {}\n{}",
        report["outcomes"],
        executed.diagnostics
    );
}
