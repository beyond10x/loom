//! Adversarial cases for `story:loom-agent-spec-env`: what the kubelet makes of `spec.env` once
//! it expands `$(VAR)` references, and the boundaries of a variable name.
//!
//! The kubelet expands `$(NAME)` in an `env` value from the variables declared before it (the
//! `envFrom` Secret's keys, then earlier `env` entries) and in `command` and `args` from the
//! container's whole environment; `$$` is a literal `$`, and an unknown name stays as written
//! (`third_party/forked/golang/expansion` in Kubernetes). So two values the credential walk admits
//! one by one can reach the program as one credential that lies, whole, in the resource.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use b10x_loom_agent::TOKEN_PREFIXES;
use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::{RenderError, render_document};
use serde_yaml_ng::Value;

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

/// The example without its `expose` block, naming a protocol only the image's program knows.
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

/// The agent container of the rendered Deployment or Job.
fn agent_container(rendered: &str) -> Value {
    let workload: Value = rendered
        .split("\n---\n")
        .map(|document| serde_yaml_ng::from_str::<Value>(document).expect("YAML"))
        .find(|document| matches!(document["kind"].as_str(), Some("Deployment" | "Job")))
        .expect("a Deployment or a Job");
    workload["spec"]["template"]["spec"]["containers"][0].clone()
}

/// `text` as the kubelet expands it against `known`: `$$` is `$`, `$(NAME)` is `NAME`'s value
/// when it is known and stays as written otherwise, any other `$` is itself.
fn expand(text: &str, known: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        if let Some(tail) = after.strip_prefix('$') {
            out.push('$');
            rest = tail;
        } else if let Some(inner) = after.strip_prefix('(')
            && let Some(close) = inner.find(')')
        {
            let name = &inner[..close];
            match known.get(name) {
                Some(value) => out.push_str(value),
                None => out.push_str(&format!("$({name})")),
            }
            rest = &inner[close + 1..];
        } else {
            out.push('$');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// The shape of a credential `value` holds, as `b10x-loom agent check` answers it: a known token
/// prefix, or a URL carrying user information.
fn credential(value: &str) -> Option<String> {
    if let Some(prefix) = TOKEN_PREFIXES
        .iter()
        .find(|prefix| value.trim_start().starts_with(*prefix))
    {
        return Some(format!("the prefix `{prefix}`"));
    }
    let rest = value.split_once("://")?.1;
    let authority = &rest[..rest.find('/').unwrap_or(rest.len())];
    authority
        .contains('@')
        .then(|| "a URL carrying user information".to_owned())
}

/// What the program reads: each `env` value and each argument, after the kubelet's expansion.
/// The Secret of `envFrom` is not known here, so a reference to one of its keys stays as written.
fn as_the_program_reads_it(container: &Value) -> Vec<(String, String)> {
    let mut known = BTreeMap::new();
    let mut read = Vec::new();
    for entry in container["env"].as_sequence().into_iter().flatten() {
        let name = entry["name"].as_str().expect("name").to_owned();
        let value = expand(entry["value"].as_str().unwrap_or_default(), &known);
        known.insert(name.clone(), value.clone());
        read.push((format!("env {name}"), value));
    }
    for list in ["command", "args"] {
        for (index, item) in container[list]
            .as_sequence()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let value = expand(item.as_str().expect("a string"), &known);
            read.push((format!("{list}[{index}]"), value));
        }
    }
    read
}

/// Either the controller refuses `resource` without repeating `secret`, or no value the program
/// reads holds a credential.
fn refused_or_clean(resource: &str, secret: &str) {
    match render_document(resource, &config()) {
        Err(RenderError::Refused(refusal)) => {
            let message = refusal.message();
            assert!(message.contains("SecretLiteral"), "{message}");
            assert!(!message.contains(secret), "{message}");
        }
        Err(other) => panic!("neither rendered nor refused: {other:?}"),
        Ok(rendered) => {
            for (member, value) in as_the_program_reads_it(&agent_container(&rendered)) {
                assert_eq!(
                    credential(&value),
                    None,
                    "admitted, and the kubelet hands the program a credential at {member}: \
                     every byte of it is in the resource's spec.env"
                );
            }
        }
    }
}

#[test]
fn adv_the_expansion_helper_is_the_kubelets() {
    let known = BTreeMap::from([("P".to_owned(), "ghp".to_owned())]);
    assert_eq!(expand("$(P)_x", &known), "ghp_x");
    assert_eq!(expand("$$(P)_x", &known), "$(P)_x");
    assert_eq!(expand("$(Q)_x", &known), "$(Q)_x");
    assert_eq!(expand("a$b$(P", &known), "a$b$(P");
    assert_eq!(
        credential("ghp_000fake").as_deref(),
        Some("the prefix `ghp_`")
    );
    assert!(credential("https://u:p@h").is_some());
    assert_eq!(credential("/workspace/cargo"), None);
}

/// A token prefix in one value and the rest of the token in a later one that names it.
#[test]
fn adv_a_token_joined_from_two_spec_env_values_is_refused() {
    let rest = env(&[
        ("PART", "ghp"),
        ("FORGE_TOKEN", "$(PART)_000fakeNotARealToken"),
    ]);
    refused_or_clean(&serve_with(&rest), "000fakeNotARealToken");
}

/// A URL's scheme and user information in one value, its host in a later one.
#[test]
fn adv_a_url_with_a_password_joined_from_two_spec_env_values_is_refused() {
    let rest = env(&[
        ("FORGE_BASE", "https://u:not-a-real-password"),
        ("FORGE_URL", "$(FORGE_BASE)@forge.example/repo"),
    ]);
    refused_or_clean(&serve_with(&rest), "not-a-real-password");
}

/// `spec.command` names a `spec.env` value that holds the token's prefix.
#[test]
fn adv_a_token_joined_from_spec_env_into_spec_command_is_refused() {
    let rest = env(&[("PART", "ghp")]);
    let resource = command_with(&["run", "--token", "$(PART)_000fakeNotARealToken"], &rest);
    refused_or_clean(&resource, "000fakeNotARealToken");
}

/// The first byte of a variable name may be `_` (`[A-Za-z_]`): nothing else in the suite
/// admits such a name, so a rule that dropped it would stay green.
#[test]
fn adv_a_name_starting_with_an_underscore_is_admitted() {
    let rest = env(&[("_PRIVATE", "1"), ("_", "x"), ("A1_b", "y")]);
    let rendered = render_document(&serve_with(&rest), &config()).expect("admitted");
    let container = agent_container(&rendered);
    let names: Vec<&str> = container["env"]
        .as_sequence()
        .expect("env")
        .iter()
        .map(|entry| entry["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, ["_PRIVATE", "_", "A1_b"]);
}
