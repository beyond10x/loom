//! Adversary pass 2 against `story:loom-agent-command-variant` (wave 2026-10-26-slicec): cases
//! that drive `render_document` and the status projection from the unit's own claims about the
//! `Command` variant, after the pass-1 fix (`words()` also walks the pieces of a word holding `:`
//! or `;`). Every credential below is a clearly fake one, never a real credential.

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

/// A takt-shaped `Command` resource whose `spec.command` is `command` (a YAML flow mapping).
fn takt_with_command(command: &str) -> String {
    wrapped(
        &takt_agent(),
        &format!(
            "  routes: {{}}\n  image: {{repository: registry.example/takt, tag: '0.12.0'}}\n  \
             command: {command}\n  secretRef: {{name: platform-operator-secrets}}\n"
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

/// `curl -H 'Authorization:Bearer <token>'`: a header with no space after its colon is one word
/// for the walk. The fix splits it into `Authorization` and `Bearer`, but the `Bearer ` prefix is
/// joined with the word after it only for a whole word, not for a piece, so the token is never
/// walked as `Bearer <token>`. The module documentation (admission.rs:20-22): "a word that is a
/// token prefix ending in a space (`Bearer`) is walked with the word after it, and a word holding
/// `:` or `;` is walked whole and then each piece between them". The story's acceptance: "a
/// credential in `spec.command` ... refused before any child".
#[test]
fn adv2_slicec_an_authorization_colon_bearer_argument_in_spec_command_is_refused() {
    let message = outcome(&takt_with_command(
        "{args: [run, -H, 'Authorization:Bearer 000fakefakefakefakefake']}",
    ));
    assert!(
        message.starts_with("refused: SecretLiteral at spec.command.args[2]"),
        "a bearer token in spec.command reached the Deployment: {message}"
    );
    assert!(!message.contains("000fakefakefakefakefake"), "{message}");
}

/// The same header in a shell script, the usual way an image's entrypoint is wrapped
/// (`sh -c '<script>'`): the script is one argument.
#[test]
fn adv2_slicec_an_authorization_colon_bearer_header_in_a_shell_script_is_refused() {
    let message = outcome(&takt_with_command(
        "{entrypoint: [sh, -c], args: ['curl -fsS -H Authorization:Bearer 000fakefakefake \
         https://forge.example/api && exec takt run']}",
    ));
    assert!(
        message.starts_with("refused: SecretLiteral at spec.command.args[0]"),
        "a bearer token in a spec.command script reached the Deployment: {message}"
    );
    assert!(!message.contains("000fakefakefake"), "{message}");
}

/// `gh auth login --with-token <<<ghp_...` in a shell script: a here-string glued to its word.
/// `<` is no separator of the walk and the library matches a prefix only at a value's start
/// (after whitespace), so the source-host token is never walked as a word starting `ghp_`.
#[test]
fn adv2_slicec_a_here_string_token_in_a_shell_script_is_refused() {
    let message = outcome(&takt_with_command(
        "{entrypoint: [sh, -c], args: ['gh auth login --with-token <<<ghp_000fakefakefakefake \
         && exec takt run']}",
    ));
    assert!(
        message.starts_with("refused: SecretLiteral at spec.command.args[0]"),
        "a source-host token in a spec.command script reached the Deployment: {message}"
    );
    assert!(!message.contains("ghp_000fakefakefakefake"), "{message}");
}
