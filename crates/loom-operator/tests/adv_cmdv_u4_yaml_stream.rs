//! Adversarial cases, pass 2, for `story:command-opaque-config-files`: the YAML stream walk of a
//! `spec.configFiles` content, and the word walk of its decoded strings (`yaml_words`), against
//! what a program reading the file decodes.

use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::{RenderError, render_document};
use serde::Deserialize;
use serde_json::json;

const CONFIG: &str = "namespace: agents
defaultImage: {repository: registry.example/loom-agent, tag: '0.40.0'}
defaultResources: {cpu: 100m, memory: 128Mi}
modelRoute: {url: 'http://model-route.agents.svc:8080/v1', protocol: messages, model: small-model}
allowedSecretNames: [platform-operator-secrets, takt-forge]
allowedEgressHosts: [chat.example, models.example]
allowedClaimNames: [takt-workspace]
";

fn config() -> Config {
    Config::parse(CONFIG).expect("the test configuration")
}

fn command(name: &str, content: &str) -> String {
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
            "configFiles": [{"name": name, "content": content}],
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

fn refused_without_repeating(content: &str, secret: &str) {
    match outcome(&command("run.yaml", content)) {
        Err(message) => {
            assert!(
                message.contains("SecretLiteral") || message.contains("TooLarge"),
                "{message}"
            );
            assert!(!message.contains(secret), "{message}");
        }
        Ok(()) => panic!("admitted a file a YAML reader decodes to a token: {content:?}"),
    }
}

/// What a program reads: the struct it wants, unknown members ignored.
#[derive(Deserialize)]
struct Run {
    token: String,
}

/// A duplicate key is legal to the walk of `check_text` (it tracks no keys) and to a typed
/// reader that ignores the member, but `serde_yaml_ng::Value` refuses it, so `yaml_words` stops
/// at that document and never splits the decoded strings into words. A token inside a value
/// (`--auth ghp_…`, which the walk refuses written out) then hides behind its escape.
#[test]
fn adv_an_escaped_token_inside_a_value_beside_a_duplicate_key_is_refused() {
    let content = "a: 1\na: 2\ntoken: \"--auth \\x67hp_000fakeNotARealToken\"\n";
    let run: Run = serde_yaml_ng::from_str(content).expect("a typed reader reads it");
    assert_eq!(run.token, "--auth ghp_000fakeNotARealToken");
    let written = "token: \"--auth ghp_000fakeNotARealToken\"\n";
    assert!(
        outcome(&command("run.yaml", written)).is_err(),
        "written out"
    );
    refused_without_repeating(content, "000fakeNotARealToken");
}

/// The same in a second document after a first holding a duplicate key.
#[test]
fn adv_an_escaped_token_in_a_document_after_a_duplicate_key_is_refused() {
    let content = "a: 1\na: 2\n---\ntoken: \"--auth \\x67hp_000fakeNotARealToken\"\n";
    let second = serde_yaml_ng::Deserializer::from_str(content)
        .nth(1)
        .expect("a second document");
    let run = Run::deserialize(second).expect("a typed reader reads it");
    assert_eq!(run.token, "--auth ghp_000fakeNotARealToken");
    refused_without_repeating(content, "000fakeNotARealToken");
}

/// D-u4.11: a scan error ends the stream. A reader that splits the stream at `---` lines before
/// parsing (as Kubernetes's own YAML reader does) still decodes the second document.
#[test]
fn adv_an_escaped_token_after_a_document_with_a_scan_error_is_refused() {
    let content = "a: [1, 2\n---\ntoken: \"\\x67hp_000fakeNotARealToken\"\n";
    assert!(
        serde_yaml_ng::Deserializer::from_str(content)
            .next()
            .map(serde_yaml_ng::Value::deserialize)
            .expect("a first document")
            .is_err(),
        "the first document does not scan"
    );
    let second = content.split("\n---\n").nth(1).expect("a second document");
    let run: Run = serde_yaml_ng::from_str(second).expect("read alone, it decodes");
    assert_eq!(run.token, "ghp_000fakeNotARealToken");
    refused_without_repeating(content, "000fakeNotARealToken");
}

/// A token inside a value, then nesting past the parser's recursion limit: the walk must not
/// read "clean" because the error message changed.
#[test]
fn adv_a_token_before_nesting_past_the_recursion_limit_is_refused() {
    let nest = format!("{}{}", "[".repeat(200), "]".repeat(200));
    let content = format!("token: \"--auth \\x67hp_000fakeNotARealToken\"\nnest: {nest}\n");
    refused_without_repeating(&content, "000fakeNotARealToken");
}
