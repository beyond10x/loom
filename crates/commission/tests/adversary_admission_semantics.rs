//! Adversary pass 1 (wave 2026-10-04-w4, unit commission/frontier-admission): the admission check
//! against what `story:frontier-admission`, `docs/contracts/frontier.md` and `AGENTS.md` § Rules
//! say it does. Each case asserts a literal expected value, so a mutant of `admission::admit` that
//! the acceptance test lets through fails here.

use b10x_commission::admission::admit;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionStatus, Admission, AdmissionRefused, CaseId, Frontier, FrontierAction, FrontierData,
    FrontierId, Unit, frontier_state,
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
        frontier_id: FrontierId(Uuid("9a1e2b3c-4d5e-4f60-8a7b-1c2d3e4f5a6b".into())),
        case_id: CaseId("case-adversary".into()),
        case_revision: 3,
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

/// `Unit`'s value is always true (`ess/domains/responsibility.yaml`, `Unit`). The acceptance test
/// matches `Admissible(_)`, so `Unit(false)` passes it.
#[test]
fn admissible_carries_unit_true() {
    let issued = frontier(vec![action(
        "repository.inspect",
        ActionStatus::Admissible,
        None,
        &[],
    )]);
    assert_eq!(
        admit(&issued, "repository.inspect"),
        Admission::Admissible(Unit(true))
    );
}

/// `docs/contracts/frontier.md` § Invariant: refused "naming the action and carrying its reasons"
/// when listed `ApprovalRequired` with no capability. The acceptance test checks only the action.
#[test]
fn approval_required_without_capability_carries_its_reasons() {
    let issued = frontier(vec![action(
        "deploy.production",
        ActionStatus::ApprovalRequired,
        None,
        &["no approver is configured"],
    )]);
    assert_eq!(
        admit(&issued, "deploy.production"),
        refused("deploy.production", &["no approver is configured"])
    );
}

/// `AdmissionRefused.reasons`: "empty when it gives none". An unlisted action has no entry, so it
/// has no reasons, and none of a neighbour's reasons leak into its refusal.
#[test]
fn unlisted_action_is_refused_with_no_reasons() {
    let issued = frontier(vec![
        action(
            "repository.merge",
            ActionStatus::Blocked,
            None,
            &["tests.pass is Unknown"],
        ),
        action(
            "deploy.production",
            ActionStatus::ApprovalRequired,
            None,
            &["no approver"],
        ),
    ]);
    assert_eq!(
        admit(&issued, "repository.delete"),
        refused("repository.delete", &[])
    );
    assert_eq!(
        admit(&frontier(Vec::new()), "repository.inspect"),
        refused("repository.inspect", &[])
    );
}

/// A `Blocked` action is never admissible and never sent for authority, whatever capability the
/// entry also names.
#[test]
fn blocked_with_a_capability_is_still_refused() {
    let issued = frontier(vec![action(
        "repository.merge",
        ActionStatus::Blocked,
        Some("repository.write"),
        &["review.approved is False"],
    )]);
    assert_eq!(
        admit(&issued, "repository.merge"),
        refused("repository.merge", &["review.approved is False"])
    );
}

/// The frontier's action names are matched exactly. A proposal that differs in case, carries
/// whitespace, is a prefix or extends a listed name is an action the frontier does not list.
#[test]
fn action_names_match_exactly() {
    let issued = frontier(vec![
        action("repository.inspect", ActionStatus::Admissible, None, &[]),
        action(
            "repository.push",
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
            &[],
        ),
    ]);
    for proposed in [
        "Repository.Inspect",
        "REPOSITORY.INSPECT",
        " repository.inspect",
        "repository.inspect ",
        "repository.inspect\n",
        "repository",
        "repository.inspect.all",
        "repository.pus",
        "",
    ] {
        assert_eq!(
            admit(&issued, proposed),
            refused(proposed, &[]),
            "proposed {proposed:?}"
        );
    }
}

/// Two entries for one action that disagree are a contradictory frontier. `AGENTS.md` § Rules: fail
/// toward less authority, never silently broaden capability. Taking the first entry admits the
/// action when the `Admissible` entry happens to come first, and refuses it in the other order.
#[test]
fn conflicting_duplicate_entries_never_admit() {
    let admissible_first = frontier(vec![
        action("repository.merge", ActionStatus::Admissible, None, &[]),
        action(
            "repository.merge",
            ActionStatus::Blocked,
            None,
            &["tests.pass is Unknown"],
        ),
    ]);
    let blocked_first = frontier(vec![
        action(
            "repository.merge",
            ActionStatus::Blocked,
            None,
            &["tests.pass is Unknown"],
        ),
        action("repository.merge", ActionStatus::Admissible, None, &[]),
    ]);
    let order_a = admit(&admissible_first, "repository.merge");
    let order_b = admit(&blocked_first, "repository.merge");
    assert!(
        matches!(order_a, Admission::Refused(ref r) if r.action == "repository.merge"),
        "Admissible then Blocked was sorted {order_a:?}; Blocked then Admissible was sorted {order_b:?}"
    );
    assert!(
        matches!(order_b, Admission::Refused(ref r) if r.action == "repository.merge"),
        "Blocked then Admissible was sorted {order_b:?}"
    );

    let needs_authority_too = frontier(vec![
        action("repository.push", ActionStatus::Admissible, None, &[]),
        action(
            "repository.push",
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
            &[],
        ),
    ]);
    let sorted = admit(&needs_authority_too, "repository.push");
    assert!(
        !matches!(sorted, Admission::Admissible(_)),
        "Admissible then ApprovalRequired was sorted {sorted:?}: the approval is skipped"
    );
}
