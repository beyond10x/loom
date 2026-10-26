//! Adversarial cases, pass 2, for `story:loom-agent-spec-env`: the walk after `$(VAR)`
//! expansion against what the kubelet expands.
//!
//! The kubelet expands a value against the `envFrom` keys, the `env` entries before it and the
//! service environment (`MappingFuncFor(tmpEnv, serviceEnv)` in `kubelet_pods.go`); the service
//! environment always holds the `kubernetes` service of `default`, so `KUBERNETES_PORT` is
//! `tcp://<cluster IP>:443` in every pod, whatever `enableServiceLinks` says.

use std::fs;
use std::path::PathBuf;

use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::{RenderError, render_document};

const CONFIG: &str = "namespace: agents
defaultImage: {repository: registry.example/loom-agent, tag: '0.40.0'}
defaultResources: {cpu: 100m, memory: 128Mi}
modelRoute: {url: 'http://model-route.agents.svc:8080/v1', protocol: messages, model: small-model}
allowedSecretNames: [platform-operator-secrets, takt-forge]
allowedEgressHosts: [chat.example, models.example]
allowedClaimNames: [takt-workspace]
";

fn example() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read_to_string(root.join("examples/agents/operator.yaml")).expect("the example spec")
}

fn config() -> Config {
    Config::parse(CONFIG).expect("the test configuration")
}

fn wrapped(agent: &str, rest: &str) -> String {
    let mut out = String::from(
        "apiVersion: loom.beyond10x.dev/v1alpha1
kind: LoomAgent
metadata:
  name: platform-operator
  namespace: agents
  uid: 00000000-0000-4000-8000-000000000001
  generation: 1
spec:
  agent:
",
    );
    for line in agent.lines() {
        if !line.is_empty() {
            out.push_str("    ");
            out.push_str(line);
        }
        out.push('\n');
    }
    out.push_str(rest);
    out
}

fn command_agent() -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in example().lines() {
        if line.starts_with("expose:") {
            skipping = true;
            continue;
        }
        if skipping && line.starts_with(' ') {
            continue;
        }
        skipping = false;
        out.push_str(line);
        out.push('\n');
    }
    out.replace("protocol: operator-reply@1", "protocol: takt-delivery@1")
}

fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

fn env(entries: &[(&str, &str)]) -> String {
    let mut out = String::from("  env:\n");
    for (name, value) in entries {
        out.push_str(&format!(
            "  - {{name: {}, value: {}}}\n",
            quoted(name),
            quoted(value)
        ));
    }
    out
}

fn serve_with(rest: &str) -> String {
    wrapped(
        &example(),
        &format!("  routes: {{}}\n  secretRef: {{name: platform-operator-secrets}}\n{rest}"),
    )
}

fn command_with(args: &[&str], rest: &str) -> String {
    let args: Vec<String> = args.iter().map(|arg| quoted(arg)).collect();
    wrapped(
        &command_agent(),
        &format!(
            "  routes: {{}}\n  image: {{repository: registry.example/takt, tag: '0.12.0'}}\n  \
             command: {{args: [{}]}}\n  secretRef: {{name: platform-operator-secrets}}\n{rest}",
            args.join(", ")
        ),
    )
}

fn outcome(resource: &str) -> Result<(), String> {
    match render_document(resource, &config()) {
        Ok(_) => Ok(()),
        Err(RenderError::Refused(refusal)) => Err(refusal.message()),
        Err(other) => panic!("neither rendered nor refused: {other:?}"),
    }
}

/// `$(KUBERNETES_PORT)` is `tcp://<cluster IP>:443` at run time, so the value below reaches the
/// program as a URL carrying a user name and password, the shape the walk refuses as written;
/// the controller leaves the service environment unexpanded and admits it.
#[test]
fn adv_a_url_joined_with_the_kubernetes_service_link_is_refused() {
    let rest = env(&[(
        "FORGE_URL",
        "$(KUBERNETES_PORT)x:not-a-real-password@forge.example/repo",
    )]);
    let as_run = "tcp://10.96.0.1:443x:not-a-real-password@forge.example/repo";
    let written = env(&[("FORGE_URL", as_run)]);
    let refused = outcome(&serve_with(&written)).expect_err("the expanded text, written out");
    assert!(refused.contains("SecretLiteral"), "{refused}");
    match outcome(&serve_with(&rest)) {
        Err(message) => assert!(
            message.contains("SecretLiteral") && !message.contains("not-a-real-password"),
            "{message}"
        ),
        Ok(()) => panic!(
            "admitted; the kubelet expands it to a URL carrying a user name and password, \
             which the walk refuses when written out: {refused}"
        ),
    }
}

/// What the kubelet leaves as written is not refused: a reference to a later entry, a
/// self-reference, and a command naming a value that held only such a reference.
#[test]
fn adv_a_reference_the_kubelet_does_not_expand_is_admitted() {
    let rest = env(&[
        ("LATER", "$(PART)_000fakeNotARealToken"),
        ("PART", "ghp"),
        ("SELF", "$(SELF)_000fakeNotARealToken"),
    ]);
    assert_eq!(outcome(&serve_with(&rest)), Ok(()));
    let resource = command_with(&["run", "--token", "$(LATER)"], &rest);
    assert_eq!(outcome(&resource), Ok(()), "no second expansion");
}

/// Two references in one argument, and a token built over three hops, are refused without
/// repeating any of it.
#[test]
fn adv_tokens_joined_over_several_references_are_refused() {
    let rest = env(&[("ONE", "gh"), ("TWO", "p_000fakeNotARealToken")]);
    let message = outcome(&command_with(&["run", "$(ONE)$(TWO)"], &rest)).expect_err("refused");
    assert_eq!(
        message,
        "refused: SecretLiteral at spec.command.args[1]: a value with the prefix `ghp_`, after \
         `$(VAR)` expansion"
    );
    let rest = env(&[
        ("A", "g"),
        ("B", "$(A)h"),
        ("C", "$(B)p"),
        ("TOKEN", "$(C)_000fakeNotARealToken"),
    ]);
    let message = outcome(&serve_with(&rest)).expect_err("refused");
    assert_eq!(
        message,
        "refused: SecretLiteral at spec.env[3].value: a value with the prefix `ghp_`, after \
         `$(VAR)` expansion"
    );
}
