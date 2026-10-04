//! Adversary pass 2 (wave 2026-10-04-w4, unit commission/frontier-admission): the precedence among
//! the three refusal rules of `admission::admit`, and how it merges reasons, as its module doc
//! (`crates/commission/src/admission.rs:7-18`) and `docs/contracts/frontier.md` § Invariant state
//! them. All three rules refuse, so only the reasons tell them apart; the acceptance and pass-1
//! cases never put two refusal rules on one action, so swapping their order went unseen.

use b10x_commission::admission::admit;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionStatus, Admission, AdmissionRefused, CaseId, Frontier, FrontierAction, FrontierData,
    FrontierId, frontier_state,
};

fn action(
    name: &str,
    status: ActionStatus,
    capability: Option<&str>,
    reasons: &[&str],
) -> FrontierAction {
    FrontierAction {
        action: name.into(),
        status,
        capability: capability.map(Into::into),
        reasons: reasons.iter().map(|r| (*r).into()).collect(),
    }
}

fn frontier(actions: Vec<FrontierAction>) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(Uuid("5c7e1a2b-3d4f-4a6b-8c9d-0e1f2a3b4c5d".into())),
        case_id: CaseId("case-adversary-2".into()),
        case_revision: 5,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions,
    })
}

fn refused(action: &str, reasons: &[&str]) -> Admission {
    Admission::Refused(AdmissionRefused {
        action: action.into(),
        reasons: reasons.iter().map(|r| (*r).into()).collect(),
    })
}

/// Sorts `entries` forwards and backwards and asserts both give `expected`.
fn assert_both_orders(entries: Vec<FrontierAction>, expected: &Admission, rule: &str) {
    let mut reversed = entries.clone();
    reversed.reverse();
    for ordering in [entries, reversed] {
        let shown: Vec<_> = ordering
            .iter()
            .map(|e| format!("{:?}/{:?}/{:?}", e.status, e.capability, e.reasons))
            .collect();
        assert_eq!(
            &admit(&frontier(ordering), "x"),
            expected,
            "{rule}; {shown:?}"
        );
    }
}

/// Rule 1 before rule 2: "any Blocked entry: refused, carrying the reasons of every Blocked entry".
#[test]
fn blocked_outranks_an_approval_with_no_capability() {
    assert_both_orders(
        vec![
            action("x", ActionStatus::ApprovalRequired, None, &["no approver"]),
            action("x", ActionStatus::Blocked, None, &["tests.pass is Unknown"]),
        ],
        &refused("x", &["tests.pass is Unknown"]),
        "rule 1 decides before rule 2",
    );
}

/// Rule 1 before rule 3: a Blocked entry decides, not the capability conflict.
#[test]
fn blocked_outranks_a_capability_conflict() {
    assert_both_orders(
        vec![
            action("x", ActionStatus::ApprovalRequired, Some("cap.a"), &[]),
            action("x", ActionStatus::Blocked, None, &["tests.pass is Unknown"]),
            action("x", ActionStatus::ApprovalRequired, Some("cap.b"), &[]),
        ],
        &refused("x", &["tests.pass is Unknown"]),
        "rule 1 decides before rule 3",
    );
}

/// Rule 2 before rule 3: an ApprovalRequired entry with no capability decides, not the conflict.
#[test]
fn approval_with_no_capability_outranks_a_capability_conflict() {
    assert_both_orders(
        vec![
            action("x", ActionStatus::ApprovalRequired, Some("cap.a"), &[]),
            action("x", ActionStatus::ApprovalRequired, None, &["no approver"]),
            action("x", ActionStatus::ApprovalRequired, Some("cap.b"), &[]),
        ],
        &refused("x", &["no approver"]),
        "rule 2 decides before rule 3",
    );
}

/// Rule 2: "carrying the reasons of every such entry" — two of them, not the first one found.
#[test]
fn every_approval_with_no_capability_gives_its_reasons() {
    assert_both_orders(
        vec![
            action("x", ActionStatus::ApprovalRequired, None, &["no approver"]),
            action(
                "x",
                ActionStatus::ApprovalRequired,
                None,
                &["approver offline"],
            ),
        ],
        &refused("x", &["approver offline", "no approver"]),
        "rule 2 carries every such entry's reasons",
    );
}

/// Module doc: "with identical lists taken once". No other case repeats a reason list.
#[test]
fn identical_reason_lists_are_taken_once() {
    assert_both_orders(
        vec![
            action("x", ActionStatus::Blocked, None, &["tests.pass is Unknown"]),
            action("x", ActionStatus::Blocked, None, &["tests.pass is Unknown"]),
        ],
        &refused("x", &["tests.pass is Unknown"]),
        "identical Blocked reason lists are taken once",
    );
}
