//! `inbound-answer@1`: a read-only protocol a plugin turn answers an inbound item under. It is in
//! the plugin catalog only, never in the bundled catalog the router and `protocols list` read.
use b10x_canon::eval::{Supplied, evaluate_with, read_case, read_evidence};
use b10x_canon::ir::Ir;
use loom_protocols::{ProtocolCatalog, SourceKind};
use serde_json::{Value, json};

const YAML: &str = include_str!("../../../protocols/inbound-answer/1.yaml");
const NAME: &str = "inbound-answer@1";

fn ir() -> Ir {
    let model = b10x_canon::model::parse(YAML).expect("inbound-answer parses");
    b10x_canon::validate::validate(&model).expect("inbound-answer validates");
    b10x_canon::ir::compile(&model).expect("inbound-answer compiles")
}

/// Canon's decision on a fresh case about item revision `r1`, with `kinds` as its evidence.
fn decide(kinds: &[&str]) -> (Value, Value) {
    let case = read_case(
        &json!({
            "format": "canon-case/1",
            "id": "case-1",
            "protocol": "inbound.answer",
            "artifacts": {"item": {"revision": "r1"}},
        })
        .to_string(),
    )
    .expect("case reads");
    let evidence: Vec<_> = kinds
        .iter()
        .enumerate()
        .map(|(n, kind)| {
            read_evidence(
                &json!({
                    "format": "canon-evidence/1",
                    "id": format!("e{n}"),
                    "kind": kind,
                    "subject": "item",
                    "subject_revision": "r1",
                })
                .to_string(),
            )
            .expect("evidence reads")
        })
        .collect();
    let decision =
        evaluate_with(&ir(), &case, &evidence, Supplied::default()).expect("Canon decides");
    (
        decision.actions.expect("actions section"),
        decision.outcomes.expect("outcomes section"),
    )
}

fn status<'a>(section: &'a Value, id: &str) -> &'a str {
    section[id]["status"]
        .as_str()
        .unwrap_or_else(|| panic!("no status for `{id}` in {section}"))
}

#[test]
fn inbound_answer_compiles() {
    let model = b10x_canon::model::parse(YAML).expect("parses");
    b10x_canon::validate::validate(&model).expect("validates");
    let ir = b10x_canon::ir::compile(&model).expect("compiles");
    assert_eq!(ir.protocol.id.as_str(), "inbound.answer");
    assert_eq!(ir.protocol.revision, 1);
}

/// An action as declared: its id, effect, required capabilities and the evidence it may produce.
type Declared = (String, Option<String>, Vec<String>, Vec<String>);

/// Every name the story declares, read from the compiled IR: the artifact, the evidence kinds, and
/// each action's effect, capability and evidence, and the outcomes. Nothing more is declared.
#[test]
fn protocol_declares_its_actions_and_outcomes() {
    let ir = ir();
    let names = |ids: Vec<&str>| ids.into_iter().map(str::to_owned).collect::<Vec<_>>();
    let artifacts: Vec<String> = ir.artifacts.keys().map(|id| id.as_str().into()).collect();
    assert_eq!(artifacts, names(vec!["item"]));
    let kinds: Vec<String> = ir
        .evidence_kinds
        .keys()
        .map(|id| id.as_str().into())
        .collect();
    assert_eq!(
        kinds,
        names(vec!["reply_declined", "reply_proposed", "source_read"])
    );
    let declared: Vec<Declared> = ir
        .actions
        .iter()
        .map(|(id, action)| {
            (
                id.as_str().to_owned(),
                action.effect.as_ref().map(|e| e.as_str().to_owned()),
                action
                    .requires
                    .iter()
                    .map(|c| c.as_str().to_owned())
                    .collect(),
                action
                    .may_produce
                    .iter()
                    .map(|k| k.as_str().to_owned())
                    .collect(),
            )
        })
        .collect();
    let expected = |id: &str, effect: &str, capability: Vec<&str>, produces: &str| {
        (
            id.to_owned(),
            Some(effect.to_owned()),
            names(capability),
            names(vec![produces]),
        )
    };
    assert_eq!(
        declared,
        vec![
            expected("reply.decline", "none", vec![], "reply_declined"),
            expected(
                "reply.propose",
                "none",
                vec!["reply.propose"],
                "reply_proposed"
            ),
            expected(
                "source.read",
                "read",
                vec!["datasource.read"],
                "source_read"
            ),
        ],
        "a proposal and a decline are recorded in the case and never sent"
    );
    let outcomes: Vec<String> = ir.outcomes.keys().map(|id| id.as_str().into()).collect();
    assert_eq!(outcomes, names(vec!["declined", "proposed"]));
}

#[test]
fn plugin_catalog_lists_inbound_answer() {
    let catalog = ProtocolCatalog::plugins().expect("the plugin catalog");
    let entry = catalog
        .get(NAME)
        .expect("inbound-answer@1 in the plugin catalog");
    assert_eq!(entry.name(), NAME);
    assert_eq!(entry.definition.yaml, YAML);
    assert_eq!(entry.definition.source.kind, SourceKind::Loom);
    assert_eq!(
        entry.definition.source.path,
        "protocols/inbound-answer/1.yaml"
    );
    assert_eq!(entry.model.protocol.id.as_str(), "inbound.answer");
}

#[test]
fn bundled_catalog_excludes_inbound_answer() {
    for (label, catalog) in [
        ("bundled", ProtocolCatalog::bundled().expect("bundled")),
        (
            "engineering",
            ProtocolCatalog::engineering().expect("engineering"),
        ),
    ] {
        assert!(catalog.get(NAME).is_none(), "{label} lists {NAME}");
        assert!(
            catalog
                .iter()
                .all(|entry| entry.model.protocol.id.as_str() != "inbound.answer"),
            "{label} lists inbound.answer under another registration"
        );
    }
}

#[test]
fn frontier_offers_no_write() {
    let ir = ir();
    let (actions, _) = decide(&[]);
    for offered in ["source.read", "reply.propose", "reply.decline"] {
        assert_ne!(status(&actions, offered), "blocked", "{offered}: {actions}");
    }
    for (id, action) in &ir.actions {
        if status(&actions, id.as_str()) == "blocked" {
            continue;
        }
        let effect = action.effect.as_ref().map(|e| e.as_str());
        assert!(
            matches!(effect, Some("read") | Some("none")),
            "offered action `{id}` has effect {effect:?}"
        );
    }
}

#[test]
fn proposed_needs_a_source_read() {
    let (_, outcomes) = decide(&["reply_proposed"]);
    assert_eq!(status(&outcomes, "proposed"), "blocked", "{outcomes}");
    let (_, outcomes) = decide(&["source_read"]);
    assert_eq!(status(&outcomes, "proposed"), "blocked", "{outcomes}");
}

#[test]
fn proposed_with_a_read_and_a_proposal() {
    let (_, outcomes) = decide(&["source_read", "reply_proposed"]);
    assert_eq!(status(&outcomes, "proposed"), "legitimate", "{outcomes}");
    assert_eq!(status(&outcomes, "declined"), "blocked", "{outcomes}");
}

#[test]
fn declined_with_a_decline() {
    let (actions, outcomes) = decide(&["reply_declined"]);
    assert_eq!(status(&outcomes, "declined"), "legitimate", "{outcomes}");
    assert_eq!(status(&outcomes, "proposed"), "blocked", "{outcomes}");
    assert_eq!(
        status(&actions, "reply.decline"),
        "admissible",
        "a turn declines without a person deciding: {actions}"
    );
    let (fresh, _) = decide(&[]);
    assert_eq!(status(&fresh, "reply.decline"), "admissible", "{fresh}");
}

#[test]
fn propose_and_decline_exclude_each_other() {
    let (fresh, _) = decide(&[]);
    for action in ["reply.propose", "reply.decline"] {
        assert_ne!(status(&fresh, action), "blocked", "{action}: {fresh}");
    }
    let (actions, _) = decide(&["reply_declined"]);
    assert_eq!(
        status(&actions, "reply.propose"),
        "blocked",
        "a declined item gets no proposal: {actions}"
    );
    let (actions, _) = decide(&["source_read", "reply_proposed"]);
    assert_eq!(
        status(&actions, "reply.decline"),
        "blocked",
        "a proposed reply is not declined after: {actions}"
    );
}
