//! Adversary pass 1 against `story:loom-agent-command-variant` (wave 2026-10-26-slicec): cases
//! that drive `render_document` from the unit's own claims about the `Command` variant.
//! Every credential below is a clearly fake one, never a real credential.

use std::fs;
use std::path::PathBuf;

use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::{RenderError, render_document};

const CONFIG: &str = "namespace: agents
defaultImage: {repository: registry.example/loom-agent, tag: '0.40.0'}
defaultResources: {cpu: 100m, memory: 128Mi}
modelRoute: {url: 'http://model-route.agents.svc:8080/v1', protocol: messages, model: small-model}
allowedSecretNames: [platform-operator-secrets]
allowedEgressHosts: [chat.example, models.example]
allowedClaimNames: [takt-workspace]
";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn example() -> String {
    fs::read_to_string(root().join("examples/agents/operator.yaml")).expect("the example spec")
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

fn without_a2a() -> String {
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
    out
}

fn takt_agent() -> String {
    without_a2a().replace("protocol: operator-reply@1", "protocol: takt-delivery@1")
}

/// A takt-shaped `Command` resource whose `spec.command.args` is `args` (YAML flow items).
fn takt_with_args(args: &str) -> String {
    wrapped(
        &takt_agent(),
        &format!(
            "  routes: {{}}\n  image: {{repository: registry.example/takt, tag: '0.12.0'}}\n  \
             command: {{args: [{args}]}}\n  secretRef: {{name: platform-operator-secrets}}\n"
        ),
    )
}

/// The refusal line, or a description of what was rendered instead.
fn outcome(resource: &str) -> String {
    match render_document(resource, &config()) {
        Err(RenderError::Refused(refusal)) => refusal.message(),
        Err(other) => format!("not refused but failed: {other:?}"),
        Ok(rendered) => format!(
            "ADMITTED and rendered ({} bytes of children)",
            rendered.len()
        ),
    }
}

/// `curl -u user:token` / `git -c credential...`: the user-and-token pair is one argument, joined
/// by `:`. The story's acceptance: "a credential in `spec.command` ... refused before any child".
#[test]
fn adv_slicec_a_user_colon_token_argument_in_spec_command_is_refused() {
    let message = outcome(&takt_with_args(
        "run, -u, 'x-access-token:ghp_000fakefakefakefakefake'",
    ));
    assert!(
        message.starts_with("refused: SecretLiteral at spec.command.args[2]"),
        "a source-host token in spec.command reached the Deployment: {message}"
    );
}

/// `curl -H x-api-key:sk-...` (a header with no space after its colon) is one argument.
#[test]
fn adv_slicec_a_header_colon_key_argument_in_spec_command_is_refused() {
    let message = outcome(&takt_with_args(
        "run, -H, 'x-api-key:sk-ant-000fakefakefakefakefake'",
    ));
    assert!(
        message.starts_with("refused: SecretLiteral at spec.command.args[2]"),
        "a model-provider key in spec.command reached the Deployment: {message}"
    );
}

/// The admission module documentation: a `Command` resource's `spec.agent` passes "only the
/// credential walk of that library (`SecretLiteral`) and the `loom.agent/1` decoder
/// (`Malformed`)"; the serve variant refuses the same document `Malformed`.
#[test]
fn adv_slicec_a_command_spec_agent_beyond_the_decoder_flow_bound_is_refused_as_the_decoder_does() {
    let braces = "{".repeat(4097);
    let agent = takt_agent().replace(
        "Never claim a fix you did not observe.",
        &format!("Never claim a fix you did not observe. {braces}"),
    );
    assert!(agent.contains(&braces));
    let serve = outcome(&wrapped(&agent, "  routes: {}\n"));
    assert!(
        serve.starts_with("refused: Malformed at spec.agent"),
        "{serve}"
    );
    let command = outcome(&wrapped(&agent, "  routes: {}\n  command: {}\n"));
    assert!(
        command.starts_with("refused: Malformed at spec.agent"),
        "the Command variant answers a reason its documentation does not list: {command}"
    );
}
