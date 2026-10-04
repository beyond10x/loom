//! Adversary pass 2 on `story:frontier-projection`: where a listed-twice action lands.
//!
//! `crates/loom/src/projection.rs` promises "each distinct action once, at its first position in
//! the frontier, with the status Commission's admission gives it". Every earlier case puts the
//! repeated action after every other action, so the first and the last position of the repeated
//! action are the same catalogue index there and a projection that keeps the last position passes
//! them all. These cases interleave the repeats with other actions, so the two rules differ.
//!
//! Every expected catalogue is a literal.

use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionStatus, CaseId, Frontier, FrontierAction, FrontierData, FrontierId, frontier_state,
};
use b10x_loom::model::primitives::Uuid as LoomUuid;
use b10x_loom::model::run::{CatalogueEntry, CatalogueEntryStatus, CatalogueId, TurnId};
use b10x_loom::projection::project;

const MERGE: &str = "repository.merge";
const INSPECT: &str = "repository.inspect";
const EDIT: &str = "repository.edit";
const WRITE: &str = "repository.write";

fn listed(action: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: action.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

fn frontier(actions: Vec<FrontierAction>) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(Uuid("00000000-0000-4000-8000-0000000000f2".to_owned())),
        case_id: CaseId("CHG-1842".to_owned()),
        case_revision: 4,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions,
    })
}

fn entries(actions: Vec<FrontierAction>) -> Vec<(String, CatalogueEntryStatus)> {
    project(
        &frontier(actions),
        CatalogueId(LoomUuid("00000000-0000-4000-8000-0000000000c2".to_owned())),
        TurnId(LoomUuid("00000000-0000-4000-8000-0000000000a2".to_owned())),
    )
    .into_data()
    .entries
    .into_iter()
    .map(|CatalogueEntry { action, status }| (action, status))
    .collect()
}

fn e(action: &str, status: CatalogueEntryStatus) -> (String, CatalogueEntryStatus) {
    (action.to_owned(), status)
}

/// Merge is listed first `Admissible`, then after `inspect` `ApprovalRequired`: Commission asks for
/// authority, and merge keeps its first position, ahead of `inspect`.
#[test]
fn adversary2_projection_repeat_keeps_first_position() {
    assert_eq!(
        entries(vec![
            listed(MERGE, ActionStatus::Admissible, None),
            listed(INSPECT, ActionStatus::Admissible, None),
            listed(MERGE, ActionStatus::ApprovalRequired, Some(WRITE)),
        ]),
        vec![
            e(MERGE, CatalogueEntryStatus::ApprovalRequired),
            e(INSPECT, CatalogueEntryStatus::Admissible),
        ]
    );
}

/// Two actions each listed twice, interleaved: each lands where it is first listed.
#[test]
fn adversary2_projection_interleaved_repeats_keep_first_positions() {
    assert_eq!(
        entries(vec![
            listed(INSPECT, ActionStatus::Admissible, None),
            listed(MERGE, ActionStatus::ApprovalRequired, Some(WRITE)),
            listed(EDIT, ActionStatus::Admissible, None),
            listed(INSPECT, ActionStatus::Admissible, None),
            listed(MERGE, ActionStatus::Admissible, None),
        ]),
        vec![
            e(INSPECT, CatalogueEntryStatus::Admissible),
            e(MERGE, CatalogueEntryStatus::ApprovalRequired),
            e(EDIT, CatalogueEntryStatus::Admissible),
        ]
    );
}

/// Merge is listed first `Blocked` and later `Admissible`: Commission refuses it, so it is in
/// neither position, and the actions around it keep their order.
#[test]
fn adversary2_projection_blocked_first_admissible_later_is_nowhere() {
    assert_eq!(
        entries(vec![
            listed(MERGE, ActionStatus::Blocked, None),
            listed(INSPECT, ActionStatus::Admissible, None),
            listed(MERGE, ActionStatus::Admissible, None),
            listed(EDIT, ActionStatus::Admissible, None),
        ]),
        vec![
            e(INSPECT, CatalogueEntryStatus::Admissible),
            e(EDIT, CatalogueEntryStatus::Admissible),
        ]
    );
}

/// Merge is listed first `Admissible` and later `Blocked`: refused, so not at its first position
/// either, although that entry alone would be admissible.
#[test]
fn adversary2_projection_admissible_first_blocked_later_is_nowhere() {
    assert_eq!(
        entries(vec![
            listed(MERGE, ActionStatus::Admissible, None),
            listed(INSPECT, ActionStatus::Admissible, None),
            listed(MERGE, ActionStatus::Blocked, None),
        ]),
        vec![e(INSPECT, CatalogueEntryStatus::Admissible)]
    );
}
