//! Adversarial cases for `story:command-opaque-config-files`: `spec.configFiles` names the API
//! server refuses in a ConfigMap, and a content whose credential only a decoder sees.

use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::{RenderError, render_document};
use serde::Deserialize;
use serde_json::json;
use serde_yaml_ng::Value;

const CONFIG: &str = "namespace: agents
defaultImage: {repository: registry.example/loom-agent, tag: '0.40.0'}
defaultResources: {cpu: 100m, memory: 128Mi}
defaultCommandResources: {cpu: 100m, memory: 128Mi}
modelRoute: {url: 'http://model-route.agents.svc:8080/v1', protocol: messages, model: small-model}
allowedSecretNames: [platform-operator-secrets, takt-forge]
allowedEgressHosts: [chat.example, models.example]
allowedClaimNames: [takt-workspace]
";

fn config() -> Config {
    Config::parse(CONFIG).expect("the test configuration")
}

/// A `Command` resource without `spec.agent`, with `files` as its `spec.configFiles`, as JSON
/// (which reads as YAML).
fn command(files: &[(&str, &str)]) -> String {
    let files: Vec<serde_json::Value> = files
        .iter()
        .map(|(name, content)| json!({"name": name, "content": content}))
        .collect();
    json!({
        "apiVersion": "loom.beyond10x.dev/v1alpha1",
        "kind": "LoomAgent",
        "metadata": {
            "name": "takt-delivery",
            "namespace": "agents",
            "uid": "00000000-0000-4000-8000-000000000001",
            "generation": 1,
        },
        "spec": {
            "routes": {},
            "image": {"repository": "registry.example/takt", "tag": "0.13.1"},
            "command": {"args": ["--iterations", "1"]},
            "configFiles": files,
        },
    })
    .to_string()
}

fn outcome(resource: &str) -> Result<(), String> {
    match render_document(resource, &config()) {
        Ok(_) => Ok(()),
        Err(RenderError::Refused(refusal)) => Err(refusal.message()),
        Err(other) => panic!("neither rendered nor refused: {other:?}"),
    }
}

/// Kubernetes refuses a ConfigMap key that starts with `..` (`IsConfigMapKey`, `hasChDirPrefix`
/// in `apimachinery/pkg/util/validation`): the kubelet's atomic writer keeps `..data` and
/// `..<timestamp>` in the volume. The controller admits such a name, so the apply fails instead
/// of the resource reading Refused, which the design says the 1 MiB check exists to avoid.
#[test]
fn adv_a_name_starting_with_two_dots_is_refused() {
    for name in ["..data", "..2026_10_26", "..run.toml"] {
        let message = outcome(&command(&[("run.toml", ""), (name, "x = 1\n")]))
            .expect_err("the API server refuses this ConfigMap key");
        assert!(
            message.starts_with("refused: ConfigFileInvalid at spec.configFiles[1].name: "),
            "{name}: {message}"
        );
        assert!(!message.contains(name), "{message}");
    }
    assert_eq!(outcome(&command(&[(".hidden", "x = 1\n")])), Ok(()));
}

/// A YAML stream of two documents is not "one YAML document", so the YAML walk reads none of
/// it and returns no credential; TOML does not read it either, and the raw words keep the
/// escape. A program reading the stream gets the token.
#[test]
fn adv_an_escaped_token_in_a_second_yaml_document_is_refused() {
    let content = "kind: run\n---\ntoken: \"\\x67hp_000fakeNotARealToken\"\n";
    let decoded: Vec<Value> = serde_yaml_ng::Deserializer::from_str(content)
        .map(|document| Value::deserialize(document).expect("a YAML document"))
        .collect();
    assert_eq!(
        decoded[1]["token"].as_str(),
        Some("ghp_000fakeNotARealToken"),
        "what the program reads"
    );
    match outcome(&command(&[("run.yaml", content)])) {
        Err(message) => {
            assert!(message.contains("SecretLiteral"), "{message}");
            assert!(!message.contains("000fakeNotARealToken"), "{message}");
        }
        Ok(()) => panic!("admitted a file whose second YAML document decodes to a `ghp_` token"),
    }
}

/// One YAML document whose aliases cost the walk more than its budget (66 bytes per source byte,
/// at least 64 KiB): the walk gives up and answers "no credential", even for a token it read
/// before giving up. serde_yaml_ng, as a program would, decodes the document whole.
#[test]
fn adv_an_escaped_token_beside_aliases_beyond_the_walk_budget_is_refused() {
    let items = vec!["x"; 100].join(", ");
    let aliases = vec!["*a"; 100].join(", ");
    let content = format!(
        "token: \"\\x67hp_000fakeNotARealToken\"\nanchor: &a [{items}]\nrepeated: [{aliases}]\n"
    );
    let decoded: Value = serde_yaml_ng::from_str(&content).expect("one YAML document");
    assert_eq!(
        decoded["token"].as_str(),
        Some("ghp_000fakeNotARealToken"),
        "what the program reads"
    );
    match outcome(&command(&[("run.yaml", &content)])) {
        Err(message) => {
            assert!(message.contains("SecretLiteral"), "{message}");
            assert!(!message.contains("000fakeNotARealToken"), "{message}");
        }
        Ok(()) => panic!("admitted a YAML document that decodes to a `ghp_` token"),
    }
}
