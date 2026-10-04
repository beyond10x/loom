//! story:frontier-admission: the admission check sorts a proposed action against the generated
//! `Frontier` into the generated `Admission` union.

use b10x_commission::admission::admit;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionStatus, Admission, AdmissionNeedsAuthority, AdmissionRefused, CaseId, Frontier,
    FrontierAction, FrontierData, FrontierId, Unit, frontier_state,
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
        frontier_id: FrontierId(Uuid("3d0b9a7e-2c41-4f6a-9e3b-5a8c1d2e4f60".into())),
        case_id: CaseId("case-1".into()),
        case_revision: 17,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions,
    })
}

#[test]
fn admission_sorts_proposed_actions() {
    let blocked_reasons = ["tests.pass is Unknown", "review.approved is False"];
    let issued = frontier(vec![
        action("repository.inspect", ActionStatus::Admissible, None, &[]),
        action(
            "repository.push",
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
            &[],
        ),
        action(
            "repository.merge",
            ActionStatus::Blocked,
            None,
            &blocked_reasons,
        ),
        action(
            "deploy.production",
            ActionStatus::ApprovalRequired,
            None,
            &[],
        ),
    ]);

    // 1. Listed Admissible: admissible.
    let admissible = admit(&issued, "repository.inspect");
    assert!(
        matches!(admissible, Admission::Admissible(_)),
        "an Admissible action was sorted {admissible:?}"
    );

    // 2. Listed ApprovalRequired with a capability: needs authority, naming the capability.
    assert_eq!(
        admit(&issued, "repository.push"),
        Admission::NeedsAuthority(AdmissionNeedsAuthority {
            capability: "repository.write".into(),
        }),
        "an ApprovalRequired action with a capability must name it"
    );

    // 3. Listed Blocked with reasons R: refused, naming the action and carrying R.
    assert_eq!(
        admit(&issued, "repository.merge"),
        Admission::Refused(AdmissionRefused {
            action: "repository.merge".into(),
            reasons: blocked_reasons.iter().map(|r| (*r).into()).collect(),
        }),
        "a Blocked action must be refused with its reasons"
    );

    // 4. Listed ApprovalRequired with no capability: refused, naming the action.
    match admit(&issued, "deploy.production") {
        Admission::Refused(refused) => assert_eq!(refused.action, "deploy.production"),
        other => panic!("an ApprovalRequired action with no capability was sorted {other:?}"),
    }

    // 5. Not listed: refused, naming the action.
    match admit(&issued, "repository.delete") {
        Admission::Refused(refused) => assert_eq!(refused.action, "repository.delete"),
        other => panic!("an action the frontier does not list was sorted {other:?}"),
    }

    // 6. The result is the generated union, reached through b10x-commission's re-export.
    let name = std::any::type_name_of_val(&admit(&issued, "repository.inspect"));
    assert!(
        name.starts_with("commission::"),
        "the admission result is {name}, not the generated crate's type"
    );
    assert!(
        std::any::type_name::<Admission>().starts_with("commission::"),
        "Admission is {}, not the generated crate's type",
        std::any::type_name::<Admission>()
    );
}

/// Every ordering of `entries`, by Heap's algorithm.
fn permutations(entries: &[FrontierAction]) -> Vec<Vec<FrontierAction>> {
    fn heap(k: usize, items: &mut Vec<FrontierAction>, out: &mut Vec<Vec<FrontierAction>>) {
        if k <= 1 {
            out.push(items.clone());
            return;
        }
        for i in 0..k {
            heap(k - 1, items, out);
            let swap = if k.is_multiple_of(2) { i } else { 0 };
            items.swap(swap, k - 1);
        }
    }
    let mut items = entries.to_vec();
    let mut out = Vec::new();
    heap(items.len(), &mut items, &mut out);
    out
}

fn refused(action: &str, reasons: &[&str]) -> Admission {
    Admission::Refused(AdmissionRefused {
        action: action.into(),
        reasons: reasons.iter().map(|r| (*r).into()).collect(),
    })
}

/// A frontier that lists one action more than once is sorted by its least-authority entry, and
/// the result is the same for every ordering of the entries (coordinator decision F1).
#[test]
fn admission_does_not_depend_on_entry_order() {
    let other = action("unrelated.action", ActionStatus::Blocked, None, &["other"]);
    let cases: Vec<(&str, Vec<FrontierAction>, Admission)> = vec![
        (
            "any Blocked entry refuses, carrying every Blocked entry's reasons",
            vec![
                action("x", ActionStatus::Admissible, None, &[]),
                action("x", ActionStatus::Blocked, None, &["b-reason"]),
                action(
                    "x",
                    ActionStatus::ApprovalRequired,
                    Some("cap"),
                    &["ignored"],
                ),
                action("x", ActionStatus::Blocked, None, &["a-reason", "z-reason"]),
            ],
            refused("x", &["a-reason", "z-reason", "b-reason"]),
        ),
        (
            "an ApprovalRequired entry with no capability refuses, carrying its reasons",
            vec![
                action("x", ActionStatus::Admissible, None, &[]),
                action("x", ActionStatus::ApprovalRequired, Some("cap"), &[]),
                action("x", ActionStatus::ApprovalRequired, None, &["no approver"]),
            ],
            refused("x", &["no approver"]),
        ),
        (
            "a whitespace-only capability is no capability: refused, carrying its reasons",
            vec![
                action("x", ActionStatus::Admissible, None, &[]),
                action("x", ActionStatus::ApprovalRequired, Some("cap"), &[]),
                action(
                    "x",
                    ActionStatus::ApprovalRequired,
                    Some(" \t\n"),
                    &["blank"],
                ),
            ],
            refused("x", &["blank"]),
        ),
        (
            "ApprovalRequired entries naming different capabilities refuse, quoting each",
            vec![
                action("x", ActionStatus::ApprovalRequired, Some("cap.b, c"), &[]),
                action("x", ActionStatus::Admissible, None, &[]),
                action("x", ActionStatus::ApprovalRequired, Some("cap.a\""), &[]),
            ],
            refused(
                "x",
                &[
                    r#"conflicting capabilities for an ApprovalRequired action: "cap.a\"", "cap.b, c""#,
                ],
            ),
        ),
        (
            "any ApprovalRequired entry needs authority",
            vec![
                action("x", ActionStatus::Admissible, None, &[]),
                action("x", ActionStatus::ApprovalRequired, Some("cap"), &[]),
                action("x", ActionStatus::ApprovalRequired, Some("cap"), &[]),
            ],
            Admission::NeedsAuthority(AdmissionNeedsAuthority {
                capability: "cap".into(),
            }),
        ),
        (
            "only Admissible entries admit",
            vec![
                action("x", ActionStatus::Admissible, None, &[]),
                action("x", ActionStatus::Admissible, None, &[]),
            ],
            Admission::Admissible(Unit(true)),
        ),
    ];
    for (name, entries, expected) in cases {
        let mut with_other = entries;
        with_other.push(other.clone());
        let orderings = permutations(&with_other);
        assert!(
            orderings.len() >= 6,
            "{name}: only {} orderings",
            orderings.len()
        );
        for ordering in orderings {
            let names: Vec<_> = ordering
                .iter()
                .map(|e| format!("{}:{:?}", e.action, e.status))
                .collect();
            assert_eq!(
                admit(&frontier(ordering), "x"),
                expected,
                "{name}; ordering {names:?}"
            );
        }
    }
}
