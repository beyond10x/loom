//! Adversary pass 2 on `story:commission-ess-conformance`: why each forced outcome is answered.
//!
//! The synthesized `not-admitted` and `needs-authority` scenarios assert only the error's `action`
//! field, so the suite cannot tell *why* revalidation refused. These cases pin the reason each
//! forced condition is answered for: `not-admitted` by the frontier blocking the listed action (not
//! by a frontier for another case or an unlisted action), `needs-authority` by the one capability
//! the governor names, `admitted` by agreement, and `stale` by a revision other than the expected
//! one at every revision the `Integer` input admits.

use b10x_commission::action_request::revalidate;
use b10x_commission::model::json::Value as ModelJson;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionRequest, ActionRequestData, ActionRequestId, ActionStatus, CaseId,
    ProposedActionArguments, RevalidateActionRequestOutcome, RunId,
};
use b10x_commission::ports::governor::Governor;
use b10x_commission_conformance::governor::{BLOCKED, CAPABILITY, Condition, ScenarioGovernor};

fn request(
    case: &str,
    expected: i64,
    action: &str,
) -> ActionRequest<b10x_commission::model::responsibility::action_request_state::Requested> {
    ActionRequest::new(ActionRequestData {
        action_request_id: ActionRequestId(Uuid("00000000-0000-4000-8000-2736a5226db7".into())),
        run_id: RunId(Uuid("00000000-0000-4000-8000-c3741068ce62".into())),
        case_id: CaseId(case.to_owned()),
        expected_case_revision: expected,
        action: action.to_owned(),
        arguments: ProposedActionArguments(ModelJson::Null),
    })
}

fn answer(
    condition: Condition,
    case: &str,
    expected: i64,
    action: &str,
) -> RevalidateActionRequestOutcome {
    let governor = ScenarioGovernor::new(
        CaseId(case.to_owned()),
        expected,
        action.to_owned(),
        condition,
    );
    revalidate(&governor, &request(case, expected, action)).expect("the governor answers")
}

const CASES: [&str; 3] = ["case_id", "", "case \u{2028} 😀"];
const ACTIONS: [&str; 3] = ["action", "", " "];
const REVISIONS: [i64; 5] = [i64::MIN, -1, 0, 1, i64::MAX];

/// A forced `not-admitted` is refused by frontier admission on the blocked entry: the reasons are
/// exactly the governor's one blocking reason.
#[test]
fn adversary2_forced_not_admitted_is_refused_for_the_blocked_entry() {
    for case in CASES {
        for action in ACTIONS {
            for expected in REVISIONS {
                let governor = ScenarioGovernor::new(
                    CaseId(case.to_owned()),
                    expected,
                    action.to_owned(),
                    Condition::Refuses,
                );
                let frontier = governor
                    .frontier(&CaseId(case.to_owned()))
                    .expect("frontier");
                let listed: Vec<_> = frontier
                    .data()
                    .actions
                    .iter()
                    .filter(|entry| entry.action == action)
                    .map(|entry| entry.status)
                    .collect();
                assert_eq!(
                    listed,
                    vec![ActionStatus::Blocked],
                    "case {case:?}, action {action:?}"
                );
                match answer(Condition::Refuses, case, expected, action) {
                    RevalidateActionRequestOutcome::NotAdmitted { error } => {
                        assert_eq!(error.action, action);
                        assert_eq!(error.reasons, vec![BLOCKED.to_owned()]);
                    }
                    other => panic!("case {case:?}, action {action:?}, {expected}: {other:?}"),
                }
            }
        }
    }
}

/// A forced `needs-authority` names the governor's one capability.
#[test]
fn adversary2_forced_needs_authority_names_the_capability() {
    for case in CASES {
        for action in ACTIONS {
            for expected in REVISIONS {
                match answer(Condition::NeedsApproval, case, expected, action) {
                    RevalidateActionRequestOutcome::NeedsAuthority { error } => {
                        assert_eq!(error.action, action);
                        assert_eq!(error.capability, CAPABILITY);
                    }
                    other => panic!("case {case:?}, action {action:?}, {expected}: {other:?}"),
                }
            }
        }
    }
}

/// With nothing forced, the governor agrees and revalidation admits, at every revision.
#[test]
fn adversary2_unforced_revalidation_admits() {
    for case in CASES {
        for action in ACTIONS {
            for expected in REVISIONS {
                let outcome = answer(Condition::Agrees, case, expected, action);
                assert!(
                    matches!(outcome, RevalidateActionRequestOutcome::Admitted),
                    "case {case:?}, action {action:?}, {expected}: {outcome:?}"
                );
            }
        }
    }
}

/// A forced `stale` names the request's revision and a different current one, at every revision.
#[test]
fn adversary2_forced_stale_names_two_different_revisions() {
    for expected in REVISIONS {
        match answer(Condition::MovedOn, "case_id", expected, "action") {
            RevalidateActionRequestOutcome::Stale { error } => {
                assert_eq!(error.expected_case_revision, expected);
                assert_ne!(error.current_case_revision, expected);
            }
            other => panic!("{expected}: {other:?}"),
        }
    }
}

/// The synthesized `stale` scenario with `expected_case_revision = i64::MIN`, an `Integer` the
/// input admits: the run must report the scenario passed rather than stop on a panic in the target.
#[test]
fn adversary2_stale_scenario_at_the_smallest_revision_passes() {
    use serde_json::{Value, json};
    let scenario = "commission.responsibility.RevalidateActionRequest/outcome/stale";
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let out = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "adversary2-governor-min-{}.json",
        std::process::id()
    ));
    let output = std::process::Command::new("ess")
        .current_dir(std::path::PathBuf::from(manifest).join("../.."))
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
    let mut suite: Value = serde_json::from_str(&text).expect("suite is JSON");

    // The input literal and the expected error field both name the revision.
    let steps = suite["scenarios"][scenario]["steps"]
        .as_array_mut()
        .expect("steps");
    for step in steps.iter_mut() {
        if step["step"] == "execute_command" {
            step["input"]["expected_case_revision"]["value"] = json!(i64::MIN);
        }
        if step["step"] == "expect_error" {
            step["fields"]["expected_case_revision"] = json!(i64::MIN);
        }
    }
    assert!(
        suite["scenarios"][scenario]
            .to_string()
            .matches(&i64::MIN.to_string())
            .count()
            == 2,
        "the edit did not land"
    );

    let executed = b10x_commission_conformance::run_suite(&suite.to_string())
        .unwrap_or_else(|e| panic!("did not run: {e}"));
    let report: Value = serde_json::from_str(&executed.report).expect("report is JSON");
    assert!(
        report["outcomes"]["passed"]
            .as_array()
            .expect("outcomes.passed")
            .iter()
            .any(|id| id == scenario),
        "`{scenario}` did not pass: {}",
        report["outcomes"]
    );
}
