//! `operator-reply@1`: `inbound-answer@1` plus a reply that is sent. A plugin turn answers an
//! inbound item under it: it reads, proposes, and sends only a reply already proposed, under the
//! `reply.send` capability. It is in the plugin catalog only, never in the bundled catalog the
//! router and `protocols list` read.
use b10x_canon::eval::{Supplied, evaluate_with, read_case, read_evidence};
use b10x_canon::ir::Ir;
use loom_protocols::{ProtocolCatalog, SourceKind};
use serde_json::{Value, json};

const YAML: &str = include_str!("../../../protocols/operator-reply/1.yaml");
const NAME: &str = "operator-reply@1";

fn ir() -> Ir {
    let model = b10x_canon::model::parse(YAML).expect("operator-reply parses");
    b10x_canon::validate::validate(&model).expect("operator-reply validates");
    b10x_canon::ir::compile(&model).expect("operator-reply compiles")
}

/// Canon's decision on a fresh case about item revision `r1`, with `kinds` as its evidence and no
/// authority supplied.
fn decide(kinds: &[&str]) -> (Value, Value) {
    let case = read_case(
        &json!({
            "format": "canon-case/1",
            "id": "case-1",
            "protocol": "operator.reply",
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
fn operator_reply_compiles() {
    let model = b10x_canon::model::parse(YAML).expect("parses");
    b10x_canon::validate::validate(&model).expect("validates");
    let ir = b10x_canon::ir::compile(&model).expect("compiles");
    assert_eq!(ir.protocol.id.as_str(), "operator.reply");
    assert_eq!(ir.protocol.revision, 1);
}

/// An action as declared: its id, effect, required capabilities and the evidence it may produce.
type Declared = (String, Option<String>, Vec<String>, Vec<String>);

/// Every name the story declares, read from the compiled IR: the artifact, the evidence kinds,
/// the claims, each action's effect, capability and evidence, and the outcomes. Nothing more is
/// declared.
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
        names(vec![
            "reply_declined",
            "reply_proposed",
            "reply_sent",
            "source_read"
        ])
    );
    let claims: Vec<String> = ir.claims.keys().map(|id| id.as_str().into()).collect();
    assert_eq!(
        claims,
        names(vec![
            "item.sourced",
            "reply.declined",
            "reply.proposed",
            "reply.sent"
        ])
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
            expected("reply.send", "write", vec!["reply.send"], "reply_sent"),
            expected(
                "source.read",
                "read",
                vec!["datasource.read"],
                "source_read"
            ),
        ],
        "only reply.send writes, and it needs the reply.send capability"
    );
    let outcomes: Vec<String> = ir.outcomes.keys().map(|id| id.as_str().into()).collect();
    assert_eq!(outcomes, names(vec!["answered", "declined"]));
}

#[test]
fn plugin_catalog_lists_operator_reply() {
    let catalog = ProtocolCatalog::plugins().expect("the plugin catalog");
    let entry = catalog
        .get(NAME)
        .expect("operator-reply@1 in the plugin catalog");
    assert_eq!(entry.name(), NAME);
    assert_eq!(entry.definition.yaml, YAML);
    assert_eq!(entry.definition.source.kind, SourceKind::Loom);
    assert_eq!(
        entry.definition.source.path,
        "protocols/operator-reply/1.yaml"
    );
    assert_eq!(entry.model.protocol.id.as_str(), "operator.reply");
    assert_eq!(
        entry.actions(),
        vec![
            "source.read",
            "reply.propose",
            "reply.send",
            "reply.decline"
        ],
        "what a spec's states may name under operator-reply@1, in declaration order"
    );
}

#[test]
fn bundled_catalog_excludes_operator_reply() {
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
                .all(|entry| entry.model.protocol.id.as_str() != "operator.reply"),
            "{label} lists operator.reply under another registration"
        );
    }
}

/// Under Canon, `reply.send` is blocked until a reply is proposed for the item, and once one is,
/// it still needs the `reply.send` capability: a decision supplied no authority does not admit it.
#[test]
fn operator_reply_send_needs_a_proposal() {
    for kinds in [&[][..], &["source_read"][..]] {
        let (actions, _) = decide(kinds);
        assert_eq!(
            status(&actions, "reply.send"),
            "blocked",
            "with {kinds:?} nothing is proposed, so nothing is sent: {actions}"
        );
    }
    let (actions, _) = decide(&["source_read", "reply_proposed"]);
    let send = status(&actions, "reply.send");
    assert_ne!(send, "blocked", "a proposed reply may be sent: {actions}");
    assert_ne!(
        send, "admissible",
        "sending needs the reply.send capability, which no authority supplied grants: {actions}"
    );
}

#[test]
fn answered_needs_a_read_a_proposal_and_a_sent_reply() {
    for kinds in [
        &["source_read", "reply_proposed"][..],
        &["reply_proposed", "reply_sent"][..],
        &["source_read", "reply_sent"][..],
        &["reply_sent"][..],
    ] {
        let (_, outcomes) = decide(kinds);
        assert_ne!(
            status(&outcomes, "answered"),
            "legitimate",
            "{kinds:?}: {outcomes}"
        );
    }
    let (_, outcomes) = decide(&["source_read", "reply_proposed", "reply_sent"]);
    assert_eq!(status(&outcomes, "answered"), "legitimate", "{outcomes}");
    assert_eq!(status(&outcomes, "declined"), "blocked", "{outcomes}");
}

#[test]
fn declined_with_a_decline() {
    let (actions, outcomes) = decide(&["reply_declined"]);
    assert_eq!(status(&outcomes, "declined"), "legitimate", "{outcomes}");
    assert_eq!(status(&outcomes, "answered"), "blocked", "{outcomes}");
    assert_eq!(
        status(&actions, "reply.decline"),
        "admissible",
        "a turn declines without a person deciding: {actions}"
    );
    assert_eq!(
        status(&actions, "reply.propose"),
        "blocked",
        "a declined item gets no proposal: {actions}"
    );
    let (fresh, _) = decide(&[]);
    assert_eq!(status(&fresh, "reply.decline"), "admissible", "{fresh}");
    let (actions, _) = decide(&["source_read", "reply_proposed"]);
    assert_eq!(
        status(&actions, "reply.decline"),
        "blocked",
        "a proposed reply is not declined after: {actions}"
    );
}
