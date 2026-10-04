//! Adversary pass 1 on `story:stale-revision-action-request`: revalidation over frontiers that list
//! one action more than once, or list a near-miss of its name, and the request's arguments carried
//! byte for byte.
//!
//! `crates/commission-testkit/tests/action_request.rs` lists every action at most once, so a
//! revalidation that read only the first entry for the action, or any `Admissible` entry, passed
//! it. Its arguments fixture is compared with a second parse of the same text, which cannot tell a
//! normalizing copy from the executor's own value.

use b10x_commission::action_request::{request, revalidate};
use b10x_commission::model::json::{self, Value};
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionNeedsAuthority, ActionNotAdmitted, ActionRequestId, ActionStatus, CaseId,
    ExecutorOutcomeProposedAction, FrontierAction, ProposedActionArguments,
    RevalidateActionRequestOutcome, RunId,
};
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};

const CASE: &str = "case-adversary-listing";
const N: i64 = 5;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn entry(
    action: &str,
    status: ActionStatus,
    capability: Option<&str>,
    reasons: &[&str],
) -> FrontierAction {
    FrontierAction {
        action: action.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: reasons.iter().map(|reason| (*reason).to_owned()).collect(),
    }
}

/// Revalidates a request for `proposed`, made at N, against a frontier at N listing `actions`.
fn revalidated(actions: Vec<FrontierAction>, proposed: &str) -> RevalidateActionRequestOutcome {
    let governor = FakeGovernor::new();
    governor.script(
        case(),
        [Answer::at(N).with_items(Vec::new(), Vec::new(), actions)],
    );
    let frontier = governor
        .frontier(&case())
        .unwrap_or_else(|error| panic!("frontier call failed: {error:?}"));
    let at_n = request(
        ActionRequestId(uuid(1)),
        RunId(uuid(2)),
        &frontier,
        ExecutorOutcomeProposedAction {
            action: proposed.to_owned(),
            arguments: ProposedActionArguments(Value::Null),
        },
    );
    revalidate(&governor, &at_n)
        .unwrap_or_else(|error| panic!("revalidation failed at the governor: {error:?}"))
}

fn not_admitted(action: &str, reasons: &[&str]) -> RevalidateActionRequestOutcome {
    RevalidateActionRequestOutcome::NotAdmitted {
        error: ActionNotAdmitted {
            action: action.to_owned(),
            reasons: reasons.iter().map(|reason| (*reason).to_owned()).collect(),
        },
    }
}

/// `merge` listed `Admissible` and `Blocked`, in both orders: not admitted, with the block's
/// reason.
#[test]
fn adversary_request_admissible_and_blocked_is_not_admitted() {
    let admissible = entry("merge", ActionStatus::Admissible, None, &[]);
    let blocked = entry("merge", ActionStatus::Blocked, None, &["checks are red"]);
    for actions in [
        vec![admissible.clone(), blocked.clone()],
        vec![blocked.clone(), admissible.clone()],
    ] {
        assert_eq!(
            revalidated(actions.clone(), "merge"),
            not_admitted("merge", &["checks are red"]),
            "listed {actions:?}"
        );
    }
}

/// `merge` listed `Admissible` and `ApprovalRequired`, in both orders: it needs authority, never
/// admitted.
#[test]
fn adversary_request_admissible_and_approval_required_needs_authority() {
    let admissible = entry("merge", ActionStatus::Admissible, None, &[]);
    let approval = entry(
        "merge",
        ActionStatus::ApprovalRequired,
        Some("repo.merge"),
        &[],
    );
    for actions in [
        vec![admissible.clone(), approval.clone()],
        vec![approval.clone(), admissible.clone()],
    ] {
        assert_eq!(
            revalidated(actions.clone(), "merge"),
            RevalidateActionRequestOutcome::NeedsAuthority {
                error: ActionNeedsAuthority {
                    action: "merge".to_owned(),
                    capability: "repo.merge".to_owned(),
                },
            },
            "listed {actions:?}"
        );
    }
}

/// `ApprovalRequired` with no capability to ask for, or with two: not admitted.
#[test]
fn adversary_request_approval_without_one_capability_is_not_admitted() {
    let blank = entry(
        "merge",
        ActionStatus::ApprovalRequired,
        Some("  "),
        &["no approver"],
    );
    assert_eq!(
        revalidated(vec![blank], "merge"),
        not_admitted("merge", &["no approver"]),
        "a whitespace-only capability"
    );

    let two = vec![
        entry("merge", ActionStatus::ApprovalRequired, Some("b"), &[]),
        entry("merge", ActionStatus::ApprovalRequired, Some("a"), &[]),
    ];
    assert_eq!(
        revalidated(two, "merge"),
        not_admitted(
            "merge",
            &[r#"conflicting capabilities for an ApprovalRequired action: "a", "b""#]
        ),
        "two capabilities"
    );
}

/// A proposed name that differs from the listed one by case or a trailing space is a different,
/// unlisted action.
#[test]
fn adversary_request_near_miss_name_is_not_admitted() {
    for proposed in ["Merge", "merge ", ""] {
        assert_eq!(
            revalidated(
                vec![entry("merge", ActionStatus::Admissible, None, &[])],
                proposed
            ),
            not_admitted(proposed, &[]),
            "proposed {proposed:?}"
        );
    }
}

/// The request carries the executor's arguments as the executor wrote them: member order,
/// duplicate keys and number spellings kept, compared with a value written out by hand.
#[test]
fn adversary_request_arguments_are_carried_byte_for_byte() {
    let text = r#"{"z":10.50,"a":[1e3,-0,null],"z":"dup"}"#;
    let parsed = json::parse(text).unwrap_or_else(|error| panic!("fixture is not JSON: {error:?}"));
    let governor = FakeGovernor::new();
    governor.script(case(), [Answer::at(N)]);
    let frontier = governor
        .frontier(&case())
        .unwrap_or_else(|error| panic!("frontier call failed: {error:?}"));
    let made = request(
        ActionRequestId(uuid(1)),
        RunId(uuid(2)),
        &frontier,
        ExecutorOutcomeProposedAction {
            action: "merge".to_owned(),
            arguments: ProposedActionArguments(parsed),
        },
    );

    let expected = Value::Object(vec![
        ("z".to_owned(), Value::Number("10.50".to_owned())),
        (
            "a".to_owned(),
            Value::Array(vec![
                Value::Number("1e3".to_owned()),
                Value::Number("-0".to_owned()),
                Value::Null,
            ]),
        ),
        ("z".to_owned(), Value::Text("dup".to_owned())),
    ]);
    assert_eq!(made.data().arguments.0, expected, "the request's arguments");

    let mut rendered = String::new();
    json::push_value(&mut rendered, &made.data().arguments.0);
    assert_eq!(rendered, text, "the request's arguments, rendered");
}
