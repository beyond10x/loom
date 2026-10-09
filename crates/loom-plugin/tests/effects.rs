//! Acceptance for `story:plugin-host`, `DataSourceEffects` alone: Commission's loop on a case of
//! `inbound-answer@1`, a scripted executor proposing, `PluginAuthority` deciding, and the effect
//! port reading through the fake `connectors` of `loom-connectors`, whose argv log shows every
//! command it ran. Nothing describes the sources first here, so the log holds only the reads.

mod support;

use b10x_loom_plugin::datasource::{ReadKind, SourceName};
use b10x_loom_plugin::effects::PRODUCER;
use b10x_loom_plugin::project::project;
use b10x_loom_plugin::{DataSourceEffects, Intent, Proposal, SourceRead};
use loom_sdk::commission::model::responsibility::EffectOutcome;
use serde_json::json;
use support::{
    Fixture, LIST, Proposes, chat, chat_without_search, classification, governed, item, run_once,
    wiki,
};

/// The kinds of the evidence the case holds, in order, each checked to be the port's.
fn evidence_kinds(
    governor: &loom_sdk::CanonGovernor<loom_sdk::MemoryCaseStore>,
    case: &loom_sdk::commission::model::responsibility::CaseId,
) -> Vec<String> {
    let held = governor.evidence(case).expect("the case's evidence");
    for record in &held {
        assert_eq!(record.producer, PRODUCER);
        assert_eq!(
            record.observation_ids.len(),
            1,
            "backed by the port's observation"
        );
    }
    held.into_iter().map(|record| record.kind).collect()
}

#[test]
fn each_read_submits_evidence() {
    let fixture = Fixture::new("effects", "each_read");
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let item = item("item-1");
    let projection = project(&config, &classification(Intent::Ask, &["chat"], "0.9"));
    let (governor, case) = governed();
    let effects = DataSourceEffects::new(
        &governor,
        case.clone(),
        &item,
        host.connectors(),
        &config.sources,
        &projection,
    );

    let read = |kind: &str, input: serde_json::Value| {
        Proposes::new(vec![(
            "source.read",
            json!({"source": "chat", "kind": kind, "input": input}),
        )])
    };
    let first = run_once(
        &governor,
        &case,
        &read("list", json!({"channel": "C0FIXTURE1"})),
        &effects,
    );
    assert!(
        matches!(first.effects.as_slice(), [EffectOutcome::Performed(_)]),
        "{:?}",
        first.effects
    );
    assert_eq!(evidence_kinds(&governor, &case), ["source_read"]);
    let second = run_once(
        &governor,
        &case,
        &read(
            "get",
            json!({"channel": "C0FIXTURE1", "ts": "1700000000.000100"}),
        ),
        &effects,
    );
    assert!(matches!(
        second.effects.as_slice(),
        [EffectOutcome::Performed(_)]
    ));
    assert_eq!(
        evidence_kinds(&governor, &case),
        ["source_read", "source_read"],
        "one source_read per performed read"
    );

    // A read Connectors refuses about the item (`not_found`) is performed by nobody and submits
    // nothing. (`not_granted` is about the connection: `a_connectors_timeout_stops_no_item`.)
    std::fs::write(
        fixture.connectors_state().join(format!("invoke.{LIST}.json")),
        r#"{"ok":false,"error":{"code":"failure","data":{"code":"not_found","kind":"operational"}}}"#,
    )
    .unwrap();
    fixture.exit("invoke", 1);
    let refused = run_once(&governor, &case, &read("list", json!({})), &effects);
    assert!(
        matches!(refused.effects.as_slice(), [EffectOutcome::Refused(r)] if r.reason.contains("not_found")),
        "{:?}",
        refused.effects
    );
    assert_eq!(
        evidence_kinds(&governor, &case).len(),
        2,
        "no evidence for a refused read"
    );
    assert_eq!(fixture.runs("invoke").len(), 3, "each read invoked once");
    assert_eq!(
        effects.reads(),
        vec![
            SourceRead {
                source: SourceName("chat".to_owned()),
                kind: ReadKind::List
            },
            SourceRead {
                source: SourceName("chat".to_owned()),
                kind: ReadKind::Get
            },
        ],
        "the record carries the performed reads"
    );
    let facts = &governor.evidence(&case).unwrap()[0].facts;
    let mut text = String::new();
    loom_sdk::commission::model::json::push_value(&mut text, facts);
    let facts: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(facts["format"], "canon-evidence/1");
    assert_eq!(facts["kind"], "source_read");
    assert_eq!(facts["subject"], "item");
    assert_eq!(facts["subject_revision"], "1700000000.000100");
}

#[test]
fn effects_refuse_an_undeclared_read() {
    let fixture = Fixture::new("effects", "undeclared");
    let config = fixture.config(vec![chat_without_search(), wiki()]);
    let host = fixture.host(&config);
    let item = item("item-1");
    let projection = project(&config, &classification(Intent::Ask, &["chat"], "0.9"));
    assert_eq!(projection.sources, vec![SourceName("chat".to_owned())]);
    let (governor, case) = governed();
    let effects = DataSourceEffects::new(
        &governor,
        case.clone(),
        &item,
        host.connectors(),
        &config.sources,
        &projection,
    );

    for (source, kind, why) in [
        ("wiki", "list", "outside this turn's projection"),
        ("elsewhere", "list", "outside this turn's projection"),
        ("chat", "search", "declares no search read"),
        ("chat", "write", "names no `kind`"),
    ] {
        let end = run_once(
            &governor,
            &case,
            &Proposes::new(vec![(
                "source.read",
                json!({"source": source, "kind": kind, "input": {"query": "deploy"}}),
            )]),
            &effects,
        );
        assert!(
            matches!(end.effects.as_slice(), [EffectOutcome::Refused(r)] if r.reason.contains(why)),
            "{source}/{kind}: {:?}",
            end.effects
        );
    }
    assert!(
        fixture.argv().is_empty(),
        "no command ran: {:?}",
        fixture.argv()
    );
    assert!(governor.evidence(&case).unwrap().is_empty(), "no evidence");
    assert!(effects.reads().is_empty());
}

#[test]
fn propose_writes_the_record_only() {
    let fixture = Fixture::new("effects", "propose");
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let item = item("item-1");
    let projection = project(&config, &classification(Intent::Ask, &["chat"], "0.9"));
    let (governor, case) = governed();
    let effects = DataSourceEffects::new(
        &governor,
        case.clone(),
        &item,
        host.connectors(),
        &config.sources,
        &projection,
    );
    let before = fixture.argv().len();

    let end = run_once(
        &governor,
        &case,
        &Proposes::new(vec![(
            "reply.propose",
            json!({"text": "The deploy finished at 10:00."}),
        )]),
        &effects,
    );

    assert!(
        matches!(end.effects.as_slice(), [EffectOutcome::Performed(_)]),
        "{:?}",
        end.effects
    );
    assert_eq!(
        effects.proposal(),
        Some(Proposal {
            item: item.id.clone(),
            text: "The deploy finished at 10:00.".to_owned()
        })
    );
    assert_eq!(
        fixture.argv().len(),
        before,
        "the fake's argv log gains no line"
    );
    assert_eq!(evidence_kinds(&governor, &case), ["reply_proposed"]);

    // An empty reply is refused and recorded nowhere.
    let (governor, case) = governed();
    let effects = DataSourceEffects::new(
        &governor,
        case.clone(),
        &item,
        host.connectors(),
        &config.sources,
        &projection,
    );
    let end = run_once(
        &governor,
        &case,
        &Proposes::new(vec![("reply.propose", json!({"text": "  "}))]),
        &effects,
    );
    assert!(matches!(
        end.effects.as_slice(),
        [EffectOutcome::Refused(_)]
    ));
    assert_eq!(effects.proposal(), None);
    assert!(governor.evidence(&case).unwrap().is_empty());
    assert!(fixture.argv().is_empty());
}
