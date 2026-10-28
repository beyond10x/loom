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
    assert_eq!(session.tools.decisions, DecisionMode::Ask);
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

/// `story:session-executor-partial-runspec`: a session names its harness overrides (effort, a
/// budget in micro-USD, strict version and context files), and a session naming none decodes as
/// before, with none of them and no context.
#[test]
fn a_session_names_its_harness_overrides() {
    let head = "format: loom.agent/1\nagent: {id: a, name: A, brief: b}\nmodel: {routes: routes.yaml, roles: []}\n";
    let session = "session:\n  kind: ClaudeCode\n  budget: {}\n  stop: {approval_gate: true, on_breach: KeepStep}\n  confinement: none\n  tools: {decisions: ask}\n";
    let named = format!(
        "{head}{session}  effort: medium\n  max_budget_micro_usd: 12500000\n  strict_version: true\n  context: [src/notes.md, docs/design.md]\n"
    );
    let spec = parse(&named).unwrap_or_else(|error| panic!("{error}"));
    let session_spec = spec.session.expect("a session");
    assert_eq!(session_spec.effort.as_deref(), Some("medium"));
    assert_eq!(session_spec.max_budget_micro_usd, Some(12_500_000));
    assert_eq!(session_spec.strict_version, Some(true));
    assert_eq!(session_spec.context, ["src/notes.md", "docs/design.md"]);

    let plain = parse(&format!("{head}{session}")).unwrap_or_else(|error| panic!("{error}"));
    let plain = plain.session.expect("a session");
    assert_eq!(plain.effort, None);
    assert_eq!(plain.max_budget_micro_usd, None);
    assert_eq!(plain.strict_version, None);
    assert!(plain.context.is_empty());

    let refused = parse(&format!("{head}{session}  strict_version: yes please\n"))
        .expect_err("a strict version that is no boolean");
    assert!(
        refused.to_string().contains("session.strict_version"),
        "{refused}"
    );
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
