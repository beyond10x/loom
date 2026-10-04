//! Adversary pass 2 on `story:commission-ess-conformance`: text through the codec and the target.
//!
//! Strings with escapes, control characters, non-BMP code points, line separators and empty keys,
//! carried from a scenario's input into an event payload or a declared error.

use b10x_commission_conformance::run_suite;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    PathBuf::from(manifest).join("../..")
}

fn synthesized(tag: &str) -> Value {
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adversary2-text-{tag}-{}.json", std::process::id()));
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

/// Text that a JSON writer has to escape, or that a careless one would normalise.
const AWKWARD: &str =
    "q\"b\\s/ \n\r\t\u{8}\u{c}\u{1}\u{1f} é e\u{301} \u{2028}\u{2029} \u{feff} 😀 \u{10ffff}";

fn suspend_with_reason(tag: &str, reason: &Value) {
    let scenario = "commission.responsibility.SuspendRun/outcome/suspended";
    let mut suite = synthesized(tag);
    retarget(
        &mut suite["scenarios"][scenario],
        "reason",
        reason,
        &|value| value.get("kind") == Some(&json!("Authority")),
    );
    assert!(
        suite["scenarios"][scenario]
            .to_string()
            .contains(&reason.to_string()),
        "the edit did not land"
    );
    assert_passes(&suite, scenario);
}

/// A `Json` reason whose keys and values carry escapes and non-ASCII text, an empty key, and the
/// empty containers.
#[test]
fn adversary2_suspend_run_echoes_escaped_and_unicode_json_text() {
    suspend_with_reason(
        "json-text",
        &json!({"kind": "Authority", "value": {
            AWKWARD: AWKWARD,
            "": "",
            "nested": [[AWKWARD], {}, [], null, true, false],
            "ünï": {"😀": AWKWARD},
        }}),
    );
}

/// The two list-of-text variants of `SuspensionReason`, carrying the same text.
#[test]
fn adversary2_suspend_run_echoes_unicode_dependency_and_evidence() {
    suspend_with_reason(
        "dependency",
        &json!({"kind": "Dependency", "value": [AWKWARD, "", "case-😀"]}),
    );
    suspend_with_reason(
        "evidence",
        &json!({"kind": "Evidence", "value": [AWKWARD, ""]}),
    );
}

/// `ActionNotAdmitted.action` and `ActionNeedsAuthority.action` are the input's `action`, so an
/// awkward action name must come back unchanged through the governor and revalidation.
#[test]
fn adversary2_revalidate_echoes_an_awkward_action() {
    for outcome in ["not-admitted", "needs-authority"] {
        let scenario =
            format!("commission.responsibility.RevalidateActionRequest/outcome/{outcome}");
        let mut suite = synthesized(outcome);
        retarget(
            &mut suite["scenarios"][&scenario],
            "action",
            &json!(AWKWARD),
            &Value::is_string,
        );
        retarget(
            &mut suite["scenarios"][&scenario],
            "case_id",
            &json!(AWKWARD),
            &Value::is_string,
        );
        assert!(
            suite["scenarios"][&scenario]
                .to_string()
                .contains(&json!(AWKWARD).to_string()),
            "the edit did not land"
        );
        assert_passes(&suite, &scenario);
    }
}
