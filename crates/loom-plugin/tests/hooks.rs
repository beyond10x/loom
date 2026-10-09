//! Acceptance for `story:plugin-host`, the default hooks: classify, project, objectives, the task
//! path, and the authority provider of a turn.
//!
//! Every model here is recorded: it answers the tool its request forces with fixed arguments and
//! keeps the request. No test reaches a model, the network or a credential.

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use b10x_loom_plugin::classify::{CLASSIFY_TOOL, classify};
use b10x_loom_plugin::datasource::SourceName;
use b10x_loom_plugin::project::{DECLINE, PROPOSE, READ, project};
use b10x_loom_plugin::state::StateDir;
use b10x_loom_plugin::{
    Decimal, GRANTED, Intent, Plugin, PluginAuthority, RecordOutcome, run_plugin_on,
};
use llm_core::ToolChoice;
use loom_sdk::ProtocolCatalog;
use loom_sdk::commission::model::json::Value;
use loom_sdk::commission::model::primitives::Uuid;
use loom_sdk::commission::model::responsibility::{
    AgentRevisionId, AuthorityContext, AuthorityVerdict, CaseId, CommissionData, CommissionId,
    PrincipalId, Unit,
};
use loom_sdk::commission::ports::authority::AuthorityProvider;
use serde_json::json;
use support::{FakePlugin, Fixture, Recorded, chat, classification, item, wiki};

#[test]
fn classify_returns_intent_hints_and_confidence() {
    let fixture = Fixture::new("hooks", "classify");
    let config = fixture.config(vec![chat(), wiki()]);
    let model = Recorded::classifying("find", &["chat"], 0.87);

    let classified = classify(&model, &config, &item("item-1")).expect("a classification");

    assert_eq!(classified.intent, Intent::Find);
    assert_eq!(classified.hints, vec!["chat".to_owned()]);
    assert_eq!(classified.confidence, Decimal("0.87".to_owned()));
    let requests = model.requests();
    assert_eq!(requests.len(), 1, "one forced call, no retry");
    let request = &requests[0];
    assert_eq!(request.tools.len(), 1);
    assert_eq!(request.tools[0].name.as_str(), CLASSIFY_TOOL);
    assert!(
        matches!(&request.tool_choice, ToolChoice::Named(name) if name.as_str() == CLASSIFY_TOOL),
        "the call is forced: {:?}",
        request.tool_choice
    );
    assert_eq!(
        request.tools[0].input_schema["properties"]["intent"]["enum"],
        json!(["ask", "request", "task", "find"])
    );
    let sent = serde_json::to_string(&request.items).unwrap();
    assert!(sent.contains("Is the deploy finished?"), "{sent}");
    for source in ["chat", "wiki"] {
        assert!(
            request.instructions.contains(&format!("- {source}")),
            "the instructions name {source}: {}",
            request.instructions
        );
    }

    // An answer that is not a classification is refused, never guessed.
    for (arguments, why) in [
        (
            json!({"intent": "chat", "hints": [], "confidence": 0.9}),
            "intent",
        ),
        (
            json!({"intent": "ask", "hints": [1], "confidence": 0.9}),
            "hints",
        ),
        (
            json!({"intent": "ask", "hints": [], "confidence": 1.5}),
            "confidence",
        ),
    ] {
        let model = Recorded::answering(vec![(CLASSIFY_TOOL, arguments)]);
        let error = classify(&model, &config, &item("item-1")).expect_err(why);
        assert!(error.to_string().contains(why), "{error}");
    }
}

#[test]
fn project_admits_reads_for_ask_find_request() {
    let fixture = Fixture::new("hooks", "project_admits");
    let config = fixture.config(vec![chat(), wiki()]);
    for intent in [Intent::Ask, Intent::Find, Intent::Request] {
        let projection = project(&config, &classification(intent, &[], "0.9"));
        assert_eq!(
            projection.actions,
            vec![READ.to_owned(), PROPOSE.to_owned(), DECLINE.to_owned()],
            "{intent:?}"
        );
        assert_eq!(
            projection.sources,
            vec![SourceName("chat".to_owned()), SourceName("wiki".to_owned())],
            "{intent:?}: no hint, every configured source"
        );
    }
    let projection = project(&config, &classification(Intent::Ask, &["wiki"], "0.9"));
    assert_eq!(projection.sources, vec![SourceName("wiki".to_owned())]);
}

#[test]
fn project_stays_within_configured_sources() {
    let fixture = Fixture::new("hooks", "project_within");
    let config = fixture.config(vec![chat(), wiki()]);
    let configured: Vec<SourceName> = config.sources.iter().map(|s| s.name.clone()).collect();
    for hints in [
        vec!["elsewhere"],
        vec!["elsewhere", "chat"],
        vec!["CHAT", "wiki "],
        vec!["wiki", "chat", "wiki"],
        vec![],
    ] {
        for intent in [Intent::Ask, Intent::Find, Intent::Request, Intent::Task] {
            let projection = project(&config, &classification(intent, &hints, "0.9"));
            assert!(
                projection.sources.iter().all(|s| configured.contains(s)),
                "{intent:?} {hints:?}: {:?}",
                projection.sources
            );
            let mut unique = projection.sources.clone();
            unique.dedup();
            assert_eq!(unique, projection.sources, "{hints:?}: a source named once");
        }
    }
    let projection = project(
        &config,
        &classification(Intent::Ask, &["elsewhere", "chat"], "0.9"),
    );
    assert_eq!(projection.sources, vec![SourceName("chat".to_owned())]);
    let none = fixture.config(Vec::new());
    let projection = project(&none, &classification(Intent::Ask, &["chat"], "0.9"));
    assert!(projection.sources.is_empty(), "no source is configured");
}

#[test]
fn task_records_a_proposed_case() {
    let fixture = Fixture::new("hooks", "task");
    let config = fixture.config(vec![chat()]);
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let protocol = catalog
        .iter()
        .map(|entry| entry.name().to_owned())
        .find(|name| name != "system-query@1")
        .expect("a bundled engineering protocol");
    let classifier = Recorded::answering(vec![
        (
            CLASSIFY_TOOL,
            json!({"intent": "task", "hints": ["chat"], "confidence": 0.9}),
        ),
        (
            "pick_protocol",
            json!({"protocol": protocol, "confidence": 0.8, "reasons": ["it changes code"]}),
        ),
    ]);
    let plugin = FakePlugin::new(vec![item("item-task")], classifier);
    let task = classification(Intent::Task, &["chat"], "0.9");
    assert!(
        project(&config, &task).actions.is_empty(),
        "a task admits nothing"
    );
    assert!(project(&config, &task).sources.is_empty());

    let lines = run_plugin_on(
        &plugin,
        &fixture.host(&config),
        &fixture.plugin_state(),
        &AtomicBool::new(true),
    )
    .expect("one cycle");

    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    assert_eq!(line.outcome, RecordOutcome::ProposedCase);
    assert_eq!(line.intent, Some(Intent::Task));
    let case = line.proposed_case.as_ref().expect("a proposed case");
    assert_eq!(case.protocol, protocol);
    assert_eq!(case.confidence, Decimal("0.8".to_owned()));
    assert_eq!(case.reasons, vec!["it changes code".to_owned()]);
    assert!(line.reads.is_empty() && line.proposal.is_none());
    assert_eq!(plugin.turn_models.load(Ordering::SeqCst), 0, "no turn ran");
    assert!(fixture.argv().is_empty(), "nothing was read");
    let pick = &plugin.classifier.requests()[1];
    let offered = &pick.tools[0].input_schema["properties"]["protocol"]["enum"];
    let names: Vec<String> = catalog
        .iter()
        .map(|entry| entry.name().to_owned())
        .collect();
    assert_eq!(
        offered,
        &json!(names),
        "the router offers the bundled catalog"
    );
    assert!(
        !names.contains(&"inbound-answer@1".to_owned()),
        "the bundled catalog never offers the plugin protocol"
    );
    let record = StateDir::open(&fixture.plugin_state(), &config)
        .unwrap()
        .record()
        .unwrap();
    assert_eq!(record, lines);
}

#[test]
fn objectives_return_config_weights() {
    let fixture = Fixture::new("hooks", "objectives");
    let config = fixture.config(vec![chat()]);
    let plugin = FakePlugin::new(Vec::new(), Recorded::classifying("ask", &[], 0.9));

    assert_eq!(plugin.objectives(&config), config.objectives);
    assert_eq!(
        plugin
            .objectives(&config)
            .iter()
            .map(|objective| (objective.name.as_str(), objective.weight.0.as_str()))
            .collect::<Vec<_>>(),
        vec![("answered", "0.7"), ("fresh", "0.3")]
    );

    let stop = Arc::new(AtomicBool::new(false));
    let plugin = FakePlugin {
        stop_at: Some((2, Arc::clone(&stop))),
        ..plugin
    };
    run_plugin_on(
        &plugin,
        &fixture.host(&config),
        &fixture.plugin_state(),
        &stop,
    )
    .expect("two cycles");
    assert_eq!(
        *plugin.objectives.lock().unwrap(),
        vec![config.objectives.clone(), config.objectives.clone()],
        "each poll is weighed by the configuration's objectives"
    );
}

#[test]
fn authority_grants_only_the_two_capabilities() {
    let commission = CommissionData {
        commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000001".to_owned())),
        agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000002".to_owned())),
        case_id: CaseId("case-1".to_owned()),
        principal: PrincipalId("loom-plugin".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    };
    assert_eq!(GRANTED, ["datasource.read", "reply.propose"]);
    for capability in GRANTED {
        assert_eq!(
            PluginAuthority.decide(&commission, capability),
            Ok(AuthorityVerdict::Allow(Unit(true))),
            "{capability}"
        );
    }
    for capability in [
        "reply.send",
        "message.post",
        "repository.merge",
        "datasource.write",
        "datasource.read ",
        "DATASOURCE.READ",
        "",
    ] {
        assert!(
            matches!(
                PluginAuthority.decide(&commission, capability),
                Ok(AuthorityVerdict::Deny(_))
            ),
            "`{capability}` is denied"
        );
    }
}
