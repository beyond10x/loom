//! Adversary pass 2 (wave 2026-10-04-w4, unit commission/frontier-admission): boundaries of the
//! capability `admission::admit` hands to the authority path.
//!
//! `story:frontier-admission` § Outcome: an `ApprovalRequired` action with no capability "is refused
//! rather than sent for authority, because no capability can be asked for and the check fails
//! toward less authority". The rule is written against `None`; an empty string names no capability
//! either. And the conflict refusal must name the conflict: two different sets of capabilities
//! must not read the same.

use b10x_commission::admission::admit;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionStatus, Admission, CaseId, Frontier, FrontierAction, FrontierData, FrontierId,
    frontier_state,
};

fn action(name: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: name.into(),
        status,
        capability: capability.map(Into::into),
        reasons: Vec::new(),
    }
}

fn frontier(actions: Vec<FrontierAction>) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(Uuid("6d8f2b3c-4e5a-4b7c-9d0e-1f2a3b4c5d6e".into())),
        case_id: CaseId("case-adversary-2".into()),
        case_revision: 5,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions,
    })
}

/// An empty capability cannot be asked for, so the action is refused, not sent for authority.
#[test]
fn approval_required_with_an_empty_capability_is_refused() {
    let issued = frontier(vec![action(
        "deploy.production",
        ActionStatus::ApprovalRequired,
        Some(""),
    )]);
    let sorted = admit(&issued, "deploy.production");
    assert!(
        matches!(sorted, Admission::Refused(ref r) if r.action == "deploy.production"),
        "ApprovalRequired with capability \"\" was sorted {sorted:?}: there is nothing to ask for"
    );
}

/// The conflict refusal joins capability names with ", ", so {"a, b", "c"} and {"a", "b, c"} give
/// one and the same refusal and the reader cannot tell which capabilities conflicted.
#[test]
fn conflict_refusals_for_different_capabilities_differ() {
    let one = admit(
        &frontier(vec![
            action("x", ActionStatus::ApprovalRequired, Some("a, b")),
            action("x", ActionStatus::ApprovalRequired, Some("c")),
        ]),
        "x",
    );
    let other = admit(
        &frontier(vec![
            action("x", ActionStatus::ApprovalRequired, Some("a")),
            action("x", ActionStatus::ApprovalRequired, Some("b, c")),
        ]),
        "x",
    );
    assert!(matches!(one, Admission::Refused(_)), "{one:?}");
    assert_ne!(
        one, other,
        "two different capability conflicts produce the same refusal"
    );
}
