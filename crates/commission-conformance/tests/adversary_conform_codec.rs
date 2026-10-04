//! Adversary pass 1 on `story:commission-ess-conformance`.
//!
//! Each case drives `CommissionTarget` from a scenario of the suite synthesized from `ess/`, edited
//! only in the literal values it carries, and asserts the scenario still passes. The edits stay
//! inside what the specification declares: `Integer` is an `i64`, `Json` is any JSON value.

use b10x_commission::action_request::revalidate;
use b10x_commission::model::json::Value as ModelJson;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionRequest, ActionRequestData, ActionRequestId, CaseId, ProposedActionArguments,
    RevalidateActionRequestOutcome, RunId,
};
use b10x_commission_conformance::governor::{Condition, ScenarioGovernor};
use b10x_commission_conformance::run_suite;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Command;

/// `2^53 + 1`: an `i64` that no `f64` holds.
const BEYOND_F64: i64 = 9_007_199_254_740_993;

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    PathBuf::from(manifest).join("../..")
}

fn synthesized(tag: &str) -> Value {
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "adversary-conform-{tag}-{}.json",
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

/// Every value under `key` in `node`: a `{kind: literal, value}` input gets `value` replaced, and
/// a bare payload value accepted by `bare` is replaced whole.
fn retarget(node: &mut Value, key: &str, new: &Value, bare: &dyn Fn(&Value) -> bool) {
    match node {
        Value::Object(members) => {
            for (name, value) in members.iter_mut() {
                if name == key {
                    if value.get("kind") == Some(&json!("literal")) {
                        value["value"] = new.clone();
                        continue;
                    }
                    if bare(value) {
                        *value = new.clone();
                        continue;
                    }
                }
                retarget(value, key, new, bare);
            }
        }
        Value::Array(items) => {
            for item in items {
                retarget(item, key, new, bare);
            }
        }
        _ => {}
    }
}

/// Runs `suite` and asserts `scenario` passed, quoting the runner's diagnostics when it did not.
fn assert_passes(suite: &Value, scenario: &str) {
    let executed = run_suite(&suite.to_string()).unwrap_or_else(|e| panic!("did not run: {e}"));
    let report: Value = serde_json::from_str(&executed.report).expect("report is JSON");
    let passed = report["outcomes"]["passed"]
        .as_array()
        .expect("outcomes.passed")
        .iter()
        .any(|id| id == scenario);
    if !passed {
        let at = executed.diagnostics.find(scenario).unwrap_or(0);
        let end = (at + 2500).min(executed.diagnostics.len());
        panic!(
            "`{scenario}` did not pass; report outcomes: {}\ndiagnostics: {}",
            report["outcomes"],
            &executed.diagnostics[at..end]
        );
    }
}

/// `StartRun` with a `case_revision` above `2^53`: `RunStarted` and `RunStates` must carry the
/// revision the command was given. The input decodes exactly (`Number::as_i64`); the encoding back
/// (`codec::number`, `value as f64`) does not.
#[test]
fn adversary_start_run_echoes_an_integer_above_2_pow_53() {
    let scenario = "commission.responsibility.StartRun/outcome/started";
    let mut suite = synthesized("start");
    let big = json!(BEYOND_F64);
    retarget(
        &mut suite["scenarios"][scenario],
        "case_revision",
        &big,
        &Value::is_number,
    );
    assert!(
        suite["scenarios"][scenario]
            .to_string()
            .contains("9007199254740993"),
        "the edit did not land"
    );
    assert_passes(&suite, scenario);
}

/// `SuspendRun` with a `Json` reason holding an integer above `2^53`: `RunSuspended.reason` is
/// `input.reason`. `codec::to_json` keeps it exactly; `codec::from_json` reparses it as an `f64`.
#[test]
fn adversary_suspend_run_echoes_a_json_reason_exactly() {
    let scenario = "commission.responsibility.SuspendRun/outcome/suspended";
    let mut suite = synthesized("suspend");
    let reason = json!({"kind": "Authority", "value": {"limit": BEYOND_F64}});
    retarget(
        &mut suite["scenarios"][scenario],
        "reason",
        &reason,
        &|value| value.get("kind") == Some(&json!("Authority")),
    );
    assert!(
        suite["scenarios"][scenario]
            .to_string()
            .contains("9007199254740993"),
        "the edit did not land"
    );
    assert_passes(&suite, scenario);
}

/// A forced `stale` must answer `stale` for every expected revision the `Integer` input admits.
/// `ScenarioGovernor::new` moves the revision on with `saturating_add(1)`, which at `i64::MAX` is
/// `i64::MAX`: the governor then agrees with the request and revalidation admits it.
#[test]
fn adversary_forced_stale_at_the_largest_revision_answers_stale() {
    let case = CaseId("case_id".to_owned());
    let governor = ScenarioGovernor::new(
        case.clone(),
        i64::MAX,
        "action".to_owned(),
        Condition::MovedOn,
    );
    let request = ActionRequest::new(ActionRequestData {
        action_request_id: ActionRequestId(Uuid("00000000-0000-4000-8000-2736a5226db7".into())),
        run_id: RunId(Uuid("00000000-0000-4000-8000-c3741068ce62".into())),
        case_id: case,
        expected_case_revision: i64::MAX,
        action: "action".to_owned(),
        arguments: ProposedActionArguments(ModelJson::Null),
    });
    let outcome = revalidate(&governor, &request).expect("the governor answers");
    assert!(
        matches!(outcome, RevalidateActionRequestOutcome::Stale { .. }),
        "a forced `stale` at expected_case_revision = i64::MAX answered {outcome:?}"
    );
}
