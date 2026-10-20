//! Acceptance for `story:plugin-host`, the governed turn: `turn::turn` on a case of
//! `inbound-answer@1`, Loom's chooser path on a scripted model (each step's selection and its
//! arguments answer the next entry of its script, and every request is kept), `PluginAuthority`,
//! and `DataSourceEffects` reading the fake `connectors` of `loom-connectors`.

mod support;

use b10x_loom_plugin::datasource::{ReadKind, SourceName};
use b10x_loom_plugin::project::project;
use b10x_loom_plugin::turn::{TURN_STEP_BUDGET, turn};
use b10x_loom_plugin::{Intent, RecordOutcome, SourceRead};
use serde_json::json;
use support::{Fixture, LIST, Scripted, chat, classification, item, request_text, wiki};

#[test]
fn turn_reaches_proposed_on_a_fixture_source() {
    let fixture = Fixture::new("turn", "proposed");
    let config = fixture.config(vec![chat(), wiki()]);
    let host = fixture.host(&config);
    let item = item("item-1");
    let classified = classification(Intent::Ask, &["chat"], "0.9");
    let projection = project(&config, &classified);
    let (model, seen) = Scripted::new(vec![
        (
            "source_read",
            json!({"source": "chat", "kind": "list", "input": {"channel": "C0FIXTURE1"}}),
        ),
        (
            "reply_propose",
            json!({"text": "Yes: the deploy finished."}),
        ),
    ]);

    let ended = turn(
        &host,
        model.into_turn_model(),
        &item,
        &classified,
        &projection,
    )
    .expect("the turn runs");

    assert_eq!(ended.outcome, RecordOutcome::Proposed, "{ended:?}");
    assert_eq!(ended.proposal.as_deref(), Some("Yes: the deploy finished."));
    assert_eq!(
        ended.reads,
        vec![SourceRead {
            source: SourceName("chat".to_owned()),
            kind: ReadKind::List
        }]
    );
    let invokes = fixture.runs("invoke");
    assert_eq!(invokes.len(), 1, "one read, invoked once: {invokes:?}");
    assert!(invokes[0].contains(&LIST.to_owned()), "{invokes:?}");
    let requests = seen.lock().unwrap().clone();
    assert_eq!(
        requests.len(),
        4,
        "two model requests per step: a selection, then its arguments"
    );
    let tools: Vec<String> = requests[0]
        .tools
        .iter()
        .map(|tool| tool.name.as_str().to_owned())
        .collect();
    assert_eq!(tools, ["select_action"], "a step starts with a selection");
    let mut candidates: Vec<String> =
        requests[0].tools[0].input_schema["properties"]["action"]["enum"]
            .as_array()
            .expect("the candidates")
            .iter()
            .map(|action| action.as_str().unwrap_or_default().to_owned())
            .collect();
    candidates.sort();
    assert_eq!(
        candidates,
        ["reply.decline", "reply.propose", "source.read"],
        "the frontier's catalogue, nothing else"
    );
    // `has_more` is only in the read's answer (`invoke-read.json`), never in the item.
    assert!(
        !request_text(&requests[0]).contains("has_more")
            && !request_text(&requests[1]).contains("has_more"),
        "the first step has read nothing"
    );
    let second = request_text(&requests[2]);
    assert!(
        second.contains("source.read chat list") && second.contains(r#"\"has_more\":false"#),
        "the second step is told the read's answer, as data:\n{second}"
    );
}

/// A read performed under `PluginAuthority`'s grant passes the gate of `source.read` and
/// `reply.propose`, so the turn's one Run goes on; a model that only reads still stops, at the
/// turn's step budget.
#[test]
fn a_turn_that_only_reads_stops_at_its_budget() {
    let fixture = Fixture::new("turn", "budget");
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let item = item("item-1");
    let classified = classification(Intent::Ask, &["chat"], "0.9");
    let projection = project(&config, &classified);
    let read = (
        "source_read",
        json!({"source": "chat", "kind": "list", "input": {"channel": "C0FIXTURE1"}}),
    );
    let (model, seen) = Scripted::new(vec![read; 3 * TURN_STEP_BUDGET]);

    let ended = turn(
        &host,
        model.into_turn_model(),
        &item,
        &classified,
        &projection,
    )
    .expect("the turn runs");

    assert_eq!(ended.outcome, RecordOutcome::Stopped, "{ended:?}");
    assert_eq!(ended.reads.len(), TURN_STEP_BUDGET, "{ended:?}");
    assert_eq!(fixture.runs("invoke").len(), TURN_STEP_BUDGET);
    assert_eq!(
        seen.lock().unwrap().len(),
        2 * TURN_STEP_BUDGET,
        "two model requests per step, none past the budget"
    );
}

#[test]
fn turn_context_lists_sources() {
    let fixture = Fixture::new("turn", "context");
    let config = fixture.config(vec![chat(), wiki()]);
    let host = fixture.host(&config);
    let item = item("item-1");
    let classified = classification(Intent::Find, &["chat"], "0.9");
    let projection = project(&config, &classified);
    let (model, seen) = Scripted::new(vec![(
        "reply_decline",
        json!({"reason": "nothing to look up"}),
    )]);

    let ended = turn(
        &host,
        model.into_turn_model(),
        &item,
        &classified,
        &projection,
    )
    .expect("the turn runs");

    assert_eq!(ended.outcome, RecordOutcome::Declined, "{ended:?}");
    assert_eq!(ended.detail.as_deref(), Some("nothing to look up"));
    assert!(ended.reads.is_empty());
    let requests = seen.lock().unwrap().clone();
    let first = request_text(requests.first().expect("the model was asked"));
    for listed in [
        "source `chat`",
        "list: operation `channel.history`, schema `sch-list-1`, revision `rev-1`",
        "search: operation `message.search`, schema `sch-search-1`, revision `rev-1`",
        "get: operation `message.get`, schema `sch-get-1`, revision `rev-1`",
        r#""required":["channel"]"#,
        r#""required":["query"]"#,
        r#""required":["channel","ts"]"#,
        "Is the deploy finished?",
    ] {
        assert!(
            first.contains(listed),
            "the first request lists `{listed}`:\n{first}"
        );
    }
    assert!(
        !first.contains("source `wiki`"),
        "a source outside the projection is not listed:\n{first}"
    );
    assert_eq!(
        fixture.runs("describe").len(),
        3,
        "each kind of chat described"
    );
    assert!(fixture.runs("invoke").is_empty(), "nothing was read");
}
