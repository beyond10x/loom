//! Acceptance for `story:agent-domain`: `examples/agents/operator.yaml` decodes into a
//! `loom_sdk::agent::AgentSpec` through `loom_sdk::agent::parse`, every section of it read.

use std::path::PathBuf;

use loom_sdk::agent::{Binding, DecisionMode, WriteMode, parse};

#[test]
fn the_operator_example_decodes() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/agents/operator.yaml");
    let text = std::fs::read_to_string(&path).expect("the example is readable");
    let spec = parse(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));

    assert_eq!(spec.format, "loom.agent/1");
    assert_eq!(spec.agent.id.0, "platform-operator");
    assert_eq!(spec.model.roles.len(), 2);
    let session = spec
        .session
        .as_ref()
        .expect("the example declares a session");
    assert_eq!(session.tools.decisions, DecisionMode::Frame);
    assert_eq!(session.tools.egress.len(), 1);
    assert_eq!(spec.states.len(), 3);
    assert!(matches!(spec.bindings[0], Binding::Connectors(_)));
    assert!(matches!(spec.bindings[1], Binding::Http(_)));
    assert_eq!(
        spec.writes.as_ref().map(|writes| writes.mode),
        Some(WriteMode::Auto)
    );
    let kubernetes = spec
        .deploy
        .as_ref()
        .and_then(|deploy| deploy.kubernetes.as_ref())
        .expect("the example declares a Kubernetes deployment");
    assert_eq!(kubernetes.egress.len(), 4);
    assert_eq!(kubernetes.egress[2].selector[0].value, "logs");
}

#[test]
fn a_document_of_another_format_is_refused() {
    let text = "format: loom.agent/2\nagent: {id: a, name: A, brief: b}\nmodel: {routes: routes.yaml, roles: []}\n";
    let refused = parse(text).expect_err("another format decodes");
    assert_eq!(
        refused.to_string(),
        "`format` is `loom.agent/2`, not `loom.agent/1`"
    );
}
