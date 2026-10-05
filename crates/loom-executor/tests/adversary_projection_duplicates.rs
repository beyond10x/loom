//! Adversary pass 1 on `story:frontier-projection`: a frontier that lists one action more than once.
//!
//! Commission's frontier contract (`docs/contracts/frontier.md`, commission 174bf07) lets a frontier
//! list one action more than once, and then "the least-authority entry decides, whatever their
//! order": any `Blocked` entry refuses the action, an `ApprovalRequired` entry with no capability
//! refuses it, two capabilities refuse it, and otherwise one `ApprovalRequired` entry makes it need
//! authority. Loom's executor already decides through that rule (`admit`). The acceptance says "An
//! action the frontier marks blocked is never in a projected catalogue"; a catalogue that projects
//! entry by entry puts an action the frontier blocks in front of the model, and lists one action
//! twice with two statuses.
//!
//! Each case asserts a literal catalogue. The last case states the rule for every small frontier
//! of one action and checks it against Commission's `admit`, which is not the code under test.

use b10x_loom_commission::admission::admit;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, Admission, CaseId, Frontier, FrontierAction, FrontierData, FrontierId,
    frontier_state,
};
use b10x_loom_executor::model::primitives::Uuid as LoomUuid;
use b10x_loom_executor::model::run::{CatalogueEntry, CatalogueEntryStatus, CatalogueId, TurnId};
use b10x_loom_executor::projection::project;

const MERGE: &str = "repository.merge";
const INSPECT: &str = "repository.inspect";

fn listed(action: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: action.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: vec![format!("{action} {status:?}")],
    }
}

fn frontier(actions: Vec<FrontierAction>) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(Uuid("00000000-0000-4000-8000-0000000000f1".to_owned())),
        case_id: CaseId("CHG-1842".to_owned()),
        case_revision: 7,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions,
    })
}

fn entries(actions: Vec<FrontierAction>) -> Vec<CatalogueEntry> {
    project(
        &frontier(actions),
        CatalogueId(LoomUuid("00000000-0000-4000-8000-0000000000c1".to_owned())),
        TurnId(LoomUuid("00000000-0000-4000-8000-0000000000a1".to_owned())),
    )
    .into_data()
    .entries
}

fn entry(action: &str, status: CatalogueEntryStatus) -> CatalogueEntry {
    CatalogueEntry {
        action: action.to_owned(),
        status,
    }
}

fn inspect() -> FrontierAction {
    listed(INSPECT, ActionStatus::Admissible, None)
}

/// Merge is listed `Admissible` and `Blocked`: Commission refuses it, so the frontier marks it
/// blocked, in either order. Only `inspect` is projected.
#[test]
fn adversary_projection_admissible_beside_blocked_is_not_projected() {
    for actions in [
        vec![
            inspect(),
            listed(MERGE, ActionStatus::Admissible, None),
            listed(MERGE, ActionStatus::Blocked, None),
        ],
        vec![
            inspect(),
            listed(MERGE, ActionStatus::Blocked, None),
            listed(MERGE, ActionStatus::Admissible, None),
        ],
    ] {
        assert!(
            matches!(
                admit(&frontier(actions.clone()), MERGE),
                Admission::Refused(_)
            ),
            "fixture: Commission refuses {MERGE}"
        );
        assert_eq!(
            entries(actions.clone()),
            vec![entry(INSPECT, CatalogueEntryStatus::Admissible)],
            "a blocked {MERGE} was projected from {actions:?}"
        );
    }
}

/// Merge is listed `Admissible` and `ApprovalRequired` naming a capability: Commission says it needs
/// authority. The catalogue lists it once, approval-gated.
#[test]
fn adversary_projection_admissible_beside_approval_is_one_approval_entry() {
    let actions = vec![
        inspect(),
        listed(MERGE, ActionStatus::Admissible, None),
        listed(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
        ),
    ];
    assert!(
        matches!(
            admit(&frontier(actions.clone()), MERGE),
            Admission::NeedsAuthority(_)
        ),
        "fixture: Commission asks for authority for {MERGE}"
    );
    assert_eq!(
        entries(actions),
        vec![
            entry(INSPECT, CatalogueEntryStatus::Admissible),
            entry(MERGE, CatalogueEntryStatus::ApprovalRequired),
        ]
    );
}

/// An `ApprovalRequired` entry that names no capability, or an empty or blank one, has nothing to
/// ask for: Commission refuses it as it refuses a `Blocked` one.
#[test]
fn adversary_projection_approval_without_capability_is_not_projected() {
    for capability in [None, Some(""), Some("  ")] {
        let actions = vec![
            inspect(),
            listed(MERGE, ActionStatus::ApprovalRequired, capability),
        ];
        assert!(
            matches!(
                admit(&frontier(actions.clone()), MERGE),
                Admission::Refused(_)
            ),
            "fixture: Commission refuses {MERGE} with capability {capability:?}"
        );
        assert_eq!(
            entries(actions),
            vec![entry(INSPECT, CatalogueEntryStatus::Admissible)],
            "{MERGE} with capability {capability:?} projected"
        );
    }
}

/// Two `ApprovalRequired` entries naming different capabilities: Commission refuses the action.
#[test]
fn adversary_projection_conflicting_capabilities_are_not_projected() {
    let actions = vec![
        inspect(),
        listed(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
        ),
        listed(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("repository.admin"),
        ),
    ];
    assert!(
        matches!(
            admit(&frontier(actions.clone()), MERGE),
            Admission::Refused(_)
        ),
        "fixture: Commission refuses {MERGE}"
    );
    assert_eq!(
        entries(actions),
        vec![entry(INSPECT, CatalogueEntryStatus::Admissible)]
    );
}

/// Every frontier of up to three entries for one action, each entry one of five kinds, in every
/// order: the catalogue lists the action at most once, with the status Commission's admission
/// gives it, and lists it not at all when Commission refuses it. Exhaustive, so deterministic.
#[test]
fn adversary_projection_agrees_with_commission_admission() {
    let kinds: [(ActionStatus, Option<&str>); 5] = [
        (ActionStatus::Admissible, None),
        (ActionStatus::Blocked, None),
        (ActionStatus::ApprovalRequired, Some("cap.a")),
        (ActionStatus::ApprovalRequired, Some("cap.b")),
        (ActionStatus::ApprovalRequired, None),
    ];
    let mut frontiers: Vec<Vec<usize>> = Vec::new();
    for a in 0..kinds.len() {
        frontiers.push(vec![a]);
        for b in 0..kinds.len() {
            frontiers.push(vec![a, b]);
            for c in 0..kinds.len() {
                frontiers.push(vec![a, b, c]);
            }
        }
    }

    let mut wrong = Vec::new();
    for picks in &frontiers {
        let mut actions = vec![inspect()];
        actions.extend(
            picks
                .iter()
                .map(|&kind| listed(MERGE, kinds[kind].0, kinds[kind].1)),
        );
        let expected_merge = match admit(&frontier(actions.clone()), MERGE) {
            Admission::Admissible(_) => vec![entry(MERGE, CatalogueEntryStatus::Admissible)],
            Admission::NeedsAuthority(_) => {
                vec![entry(MERGE, CatalogueEntryStatus::ApprovalRequired)]
            }
            Admission::Refused(_) => Vec::new(),
        };
        let mut expected = vec![entry(INSPECT, CatalogueEntryStatus::Admissible)];
        expected.extend(expected_merge);
        let got = entries(actions);
        if got != expected {
            wrong.push(format!(
                "{:?}: expected {:?}, projected {:?}",
                picks.iter().map(|&kind| kinds[kind]).collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(|e| (&e.action[..], e.status))
                    .collect::<Vec<_>>(),
                got.iter()
                    .map(|e| (&e.action[..], e.status))
                    .collect::<Vec<_>>(),
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} frontiers project differently from Commission's admission; first five:\n{}",
        wrong.len(),
        frontiers.len(),
        wrong.iter().take(5).cloned().collect::<Vec<_>>().join("\n")
    );
}
