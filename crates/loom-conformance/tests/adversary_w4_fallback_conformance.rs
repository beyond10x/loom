//! Adversary pass 1, wave 2026-10-10-w1, `story:fallback-selection-recording`: the conformance
//! target's `OverruleSelection`, the `Selections` view's `replaced_by`, and the `Overruled` state as
//! a wrong-state refusal carries it.

use std::collections::BTreeMap;

use b10x_loom_conformance::LoomTarget;
use ess_conformance::target::{
    ConformanceTarget, Deadline, SemanticCommandRequest, SemanticViewRequest,
};
use ess_primitives::consistency::QueryConsistency;
use ess_primitives::facts::Number;
use ess_primitives::ids::CorrelationId;
use ess_primitives::node::Node;
use ess_primitives::time::Timestamp;

const SESSION: &str = "00000000-0000-4000-8000-000000000001";
const RUN: &str = "00000000-0000-4000-8000-000000000010";
const TURN: &str = "00000000-0000-4000-8000-000000000020";
const CATALOGUE: &str = "00000000-0000-4000-8000-000000000030";
const FAST: &str = "00000000-0000-4000-8000-000000000041";
const STRONGER: &str = "00000000-0000-4000-8000-000000000042";
const UNKNOWN: &str = "00000000-0000-4000-8000-0000000000ff";

fn text(value: &str) -> Node {
    Node::Text(value.to_owned())
}

fn int(value: i64) -> Node {
    Node::Number(Number::from(value))
}

fn correlation() -> CorrelationId {
    CorrelationId::new("adversary-w4-fallback").expect("a correlation id")
}

/// `(outcome, error name, error fields, unused)`.
type Sent = (String, Option<String>, BTreeMap<String, Node>, Vec<()>);

fn send(target: &LoomTarget, command: &str, input: &[(&str, Node)]) -> Sent {
    let result = target
        .execute_command(SemanticCommandRequest {
            command: command.parse().expect("a command reference"),
            actor: None,
            caller: None,
            input: input
                .iter()
                .map(|(name, value)| ((*name).to_owned(), value.clone()))
                .collect(),
            correlation: correlation(),
        })
        .unwrap_or_else(|error| panic!("{command}: {error}"));
    let outcome = result
        .outcome
        .as_ref()
        .map(|outcome| outcome.to_string())
        .unwrap_or_else(|| panic!("{command} reached no declared outcome"));
    let (name, fields) = match result.error {
        Some(error) => (Some(error.error.to_string()), error.fields),
        None => (None, BTreeMap::new()),
    };
    (outcome, name, fields, Vec::new())
}

fn selections(target: &LoomTarget) -> Vec<BTreeMap<String, Node>> {
    target
        .query_view(SemanticViewRequest {
            view: "loom.run.Selections".parse().expect("a view reference"),
            params: BTreeMap::new(),
            consistency: QueryConsistency::Current,
            correlation: correlation(),
            deadline: Deadline::at(Timestamp::from_epoch_millis(u64::MAX)),
        })
        .expect("the view answers")
        .rows
}

/// A target holding two `Selected` selections, FAST and STRONGER, of one catalogue.
fn two_selections() -> LoomTarget {
    let target = LoomTarget::default();
    let (outcome, ..) = send(
        &target,
        "loom.run.OpenSession",
        &[
            ("session_id", text(SESSION)),
            ("commission_run", text(RUN)),
            ("wire", text("wire-a")),
        ],
    );
    assert!(outcome.ends_with("opened"), "{outcome}");
    let (outcome, ..) = send(
        &target,
        "loom.run.RecordTurn",
        &[
            ("turn_id", text(TURN)),
            ("session_id", text(SESSION)),
            ("index", int(1)),
            ("items", Node::Seq(vec![text("item")])),
        ],
    );
    assert!(outcome.ends_with("recorded"), "{outcome}");
    let entry = |action: &str| {
        Node::Map(BTreeMap::from([
            ("action".to_owned(), text(action)),
            ("status".to_owned(), text("Admissible")),
        ]))
    };
    let (outcome, ..) = send(
        &target,
        "loom.run.ProjectCatalogue",
        &[
            ("catalogue_id", text(CATALOGUE)),
            ("turn_id", text(TURN)),
            ("frontier", text("frontier-a")),
            ("case_revision", int(7)),
            (
                "entries",
                Node::Seq(vec![entry("metrics.inspect"), entry("logs.search")]),
            ),
        ],
    );
    assert!(outcome.ends_with("projected"), "{outcome}");
    for (id, action, strategy) in [
        (FAST, "metrics.inspect", "FastTyped"),
        (STRONGER, "logs.search", "ReasoningModel"),
    ] {
        let (outcome, ..) = send(
            &target,
            "loom.run.SelectAction",
            &[
                ("selection_id", text(id)),
                ("catalogue_id", text(CATALOGUE)),
                ("action", text(action)),
                ("confidence", Node::Null),
                ("strategy", text(strategy)),
                ("case_revision", int(7)),
            ],
        );
        assert!(outcome.ends_with("selected"), "{outcome}");
    }
    target
}

fn row<'r>(rows: &'r [BTreeMap<String, Node>], id: &str) -> &'r BTreeMap<String, Node> {
    rows.iter()
        .find(|row| row.get("selection_id") == Some(&text(id)))
        .unwrap_or_else(|| panic!("no row for {id}: {rows:?}"))
}

/// Overruled through the target: the view publishes the fast selection `Overruled` with
/// `replaced_by` the replacement's id, and the replacement `Selected` with `replaced_by` null.
#[test]
fn adversary_w4_the_selections_view_publishes_replaced_by_and_overruled() {
    let target = two_selections();
    let rows = selections(&target);
    assert_eq!(row(&rows, FAST).get("replaced_by"), Some(&Node::Null));

    let (outcome, error, ..) = send(
        &target,
        "loom.run.OverruleSelection",
        &[
            ("selection_id", text(FAST)),
            ("replacement_id", text(STRONGER)),
        ],
    );
    assert!(outcome.ends_with("overruled"), "{outcome}");
    assert_eq!(error, None);

    let rows = selections(&target);
    let fast = row(&rows, FAST);
    assert_eq!(fast.get("state"), Some(&text("Overruled")), "{fast:?}");
    assert_eq!(fast.get("replaced_by"), Some(&text(STRONGER)), "{fast:?}");
    let stronger = row(&rows, STRONGER);
    assert_eq!(stronger.get("state"), Some(&text("Selected")));
    assert_eq!(stronger.get("replaced_by"), Some(&Node::Null));
}

/// After the overrule, the overruled selection answers wrong-state carrying `Overruled` to a second
/// overrule and to a revalidation, and `selection-not-selected` to an argument request.
#[test]
fn adversary_w4_an_overruled_selection_is_refused_carrying_overruled() {
    let target = two_selections();
    let overrule = || {
        send(
            &target,
            "loom.run.OverruleSelection",
            &[
                ("selection_id", text(FAST)),
                ("replacement_id", text(STRONGER)),
            ],
        )
    };
    assert!(overrule().0.ends_with("overruled"));
    let (outcome, error, fields, _) = overrule();
    assert!(outcome.ends_with("wrong-state"), "{outcome}");
    assert_eq!(error.as_deref(), Some("loom.run.SelectionStateConflict"));
    assert_eq!(
        fields,
        BTreeMap::from([("state".to_owned(), text("Overruled"))])
    );

    let (outcome, _, fields, _) = send(
        &target,
        "loom.run.RevalidateSelection",
        &[
            ("selection_id", text(FAST)),
            ("case_revision", int(7)),
            ("frontier_actions", Node::Seq(vec![text("metrics.inspect")])),
        ],
    );
    assert!(outcome.ends_with("wrong-state"), "{outcome}");
    assert_eq!(
        fields,
        BTreeMap::from([("state".to_owned(), text("Overruled"))])
    );

    let (outcome, ..) = send(
        &target,
        "loom.run.RequestArguments",
        &[
            ("argument_request_id", text(UNKNOWN)),
            ("selection_id", text(FAST)),
        ],
    );
    assert!(outcome.ends_with("selection-not-selected"), "{outcome}");
}

/// An unknown selection is wrong-state with no `state` field.
#[test]
fn adversary_w4_overruling_an_unknown_selection_is_wrong_state_without_a_state() {
    let target = two_selections();
    let (outcome, error, fields, _) = send(
        &target,
        "loom.run.OverruleSelection",
        &[
            ("selection_id", text(UNKNOWN)),
            ("replacement_id", text(STRONGER)),
        ],
    );
    assert!(outcome.ends_with("wrong-state"), "{outcome}");
    assert_eq!(error.as_deref(), Some("loom.run.SelectionStateConflict"));
    assert_eq!(fields, BTreeMap::new());
}
