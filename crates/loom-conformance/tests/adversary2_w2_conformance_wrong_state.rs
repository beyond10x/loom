//! Adversary pass 2, wave 2026-10-07-w2: the `state` a wrong-state refusal carries.
//!
//! `ess/domains/run.yaml` declares `loom.run.SelectionStateConflict` with a `state` field: the
//! state the subject is actually in. The synthesized suite's wrong-state scenarios check the
//! error's name only (`expect_error`, no fields), so `task conform` stays green when `LoomTarget`
//! drops or invents that field. This case drives `LoomTarget` directly and requires the field the
//! executor reported. The session cases went with `loom.run.Session` (`story:delete-harness-port`).

use std::collections::BTreeMap;

use b10x_loom_conformance::LoomTarget;
use ess_conformance::target::{ConformanceTarget, SemanticCommandRequest};
use ess_primitives::facts::Number;
use ess_primitives::ids::CorrelationId;
use ess_primitives::node::Node;

const TURN: &str = "00000000-0000-4000-8000-000000000020";
const CATALOGUE: &str = "00000000-0000-4000-8000-000000000030";
const SELECTION: &str = "00000000-0000-4000-8000-000000000040";

fn text(value: &str) -> Node {
    Node::Text(value.to_owned())
}

fn int(value: i64) -> Node {
    Node::Number(Number::from(value))
}

/// Sends `command` with `input` and returns `(outcome, error name, error fields)`.
fn send(
    target: &LoomTarget,
    command: &str,
    input: &[(&str, Node)],
) -> (String, Option<String>, BTreeMap<String, Node>) {
    let result = target
        .execute_command(SemanticCommandRequest {
            command: command.parse().expect("a command reference"),
            actor: None,
            caller: None,
            input: input
                .iter()
                .map(|(name, value)| ((*name).to_owned(), value.clone()))
                .collect(),
            correlation: CorrelationId::new("adversary2-wrong-state").expect("a correlation id"),
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
    (outcome, name, fields)
}

#[test]
fn adversary2_w2_a_selection_wrong_state_carries_the_state_the_selection_is_in() {
    let target = LoomTarget::default();
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
                Node::Seq(vec![Node::Map(BTreeMap::from([
                    ("action".to_owned(), text("repository.merge")),
                    ("status".to_owned(), text("Admissible")),
                ]))]),
            ),
        ],
    );
    assert!(outcome.ends_with("projected"), "{outcome}");
    let (outcome, ..) = send(
        &target,
        "loom.run.SelectAction",
        &[
            ("selection_id", text(SELECTION)),
            ("catalogue_id", text(CATALOGUE)),
            ("action", text("repository.merge")),
            ("confidence", Node::Null),
            ("strategy", text("Rule")),
            ("case_revision", int(7)),
        ],
    );
    assert!(outcome.ends_with("selected"), "{outcome}");
    let revalidate = || {
        send(
            &target,
            "loom.run.RevalidateSelection",
            &[
                ("selection_id", text(SELECTION)),
                ("case_revision", int(7)),
                ("frontier_actions", Node::Seq(vec![])),
            ],
        )
    };
    assert!(revalidate().0.ends_with("admitted"));
    let (outcome, error, fields) = revalidate();
    assert!(outcome.ends_with("wrong-state"), "{outcome}");
    assert_eq!(error.as_deref(), Some("loom.run.SelectionStateConflict"));
    assert_eq!(
        fields,
        BTreeMap::from([("state".to_owned(), text("Admitted"))]),
        "revalidating an Admitted selection"
    );
}
