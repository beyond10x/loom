//! Adversary pass on unit U4 of wave 2026-11-05-w1, its ride-along: the upper bound of
//! `spec.schedule.startingDeadlineSeconds`, in admission and in the design's refusal table.
//! Nothing is applied and nothing reaches a cluster.

use std::fs;
use std::path::PathBuf;

use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::{RenderError, render_document};
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

const TAKT: &str = "  routes: {}
  image: {repository: registry.example/takt, tag: '0.12.0'}
  command: {args: [--iterations, '1']}
  workspace: {mountPath: /workspace, claimName: takt-workspace}
  secretRef: {name: platform-operator-secrets}
  secretFiles: {name: takt-forge, mountPath: /var/run/secrets/takt}
";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn config() -> Config {
    Config::parse(CONFIG).expect("the test configuration")
}

fn command_with(schedule: &str) -> String {
    let agent = fs::read_to_string(root().join("examples/agents/operator.yaml"))
        .expect("the example spec")
        .replace("protocol: operator-reply@1", "protocol: takt-delivery@1");
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
    let mut skipping = false;
    for line in agent.lines() {
        if line.starts_with("expose:") {
            skipping = true;
            continue;
        }
        if skipping && line.starts_with(' ') {
            continue;
        }
        skipping = false;
        if !line.is_empty() {
            out.push_str("    ");
            out.push_str(line);
        }
        out.push('\n');
    }
    out.push_str(TAKT);
    out.push_str(schedule);
    out
}

fn refusal(resource: &str) -> String {
    match render_document(resource, &config()) {
        Err(RenderError::Refused(refusal)) => refusal.message(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn cron_job(rendered: &str) -> Value {
    let docs: Vec<Value> = rendered
        .split("\n---\n")
        .map(|document| serde_yaml_ng::from_str(document).expect("YAML"))
        .collect();
    assert_eq!(docs[2]["kind"], "CronJob", "{rendered}");
    docs[2].clone()
}

/// RED. `docs/design/loom-agent-crd-deployment.md`, the schedule's refusal table, is in the
/// story's scope, and the unit's CHANGELOG, AGENTS.md, admission docs and ESS comment all state
/// the new upper bound of `startingDeadlineSeconds`; the table's row still says "is negative"
/// only, beside a `runDeadlineSeconds` row that names 9223372036.
#[test]
fn adv_u4_the_design_refusal_table_states_the_starting_deadline_bound() {
    let design = fs::read_to_string(root().join("docs/design/loom-agent-crd-deployment.md"))
        .expect("the design");
    let row = design
        .lines()
        .find(|line| line.starts_with("| `startingDeadlineSeconds`"))
        .expect("the refusal row of startingDeadlineSeconds");
    assert!(row.contains("9223372036"), "{row}");
}

/// GREEN probe. Step 18 order and bounds: the starting deadline's upper bound is refused at its
/// member before a run deadline refusal; both bounds at exactly 9223372036 are admitted together
/// and rendered as given; a value past `i64::MAX` is refused, never wrapped into the CronJob.
#[test]
fn adv_u4_the_starting_deadline_bound_keeps_its_order_and_its_edge() {
    let message = refusal(&command_with(
        "  schedule: {cron: '0 3 * * *', startingDeadlineSeconds: 9223372037, \
         runDeadlineSeconds: 0}\n",
    ));
    assert!(
        message.starts_with("refused: Malformed at spec.schedule.startingDeadlineSeconds: "),
        "{message}"
    );
    let message = refusal(&command_with(
        "  schedule: {cron: 'nope', startingDeadlineSeconds: 9223372037}\n",
    ));
    assert!(
        message.starts_with("refused: Malformed at spec.schedule.cron: "),
        "{message}"
    );
    let rendered = render_document(
        &command_with(
            "  schedule: {cron: '0 3 * * *', startingDeadlineSeconds: 9223372036, \
             runDeadlineSeconds: 9223372036}\n",
        ),
        &config(),
    )
    .expect("rendered");
    let cron = cron_job(&rendered);
    assert_eq!(
        cron["spec"]["startingDeadlineSeconds"].as_i64(),
        Some(9_223_372_036)
    );
    assert_eq!(
        cron["spec"]["jobTemplate"]["spec"]["activeDeadlineSeconds"].as_i64(),
        Some(9_223_372_036)
    );
    let past = render_document(
        &command_with("  schedule: {cron: '0 3 * * *', startingDeadlineSeconds: 9223372036854775808}\n"),
        &config(),
    );
    assert!(past.is_err(), "{past:?}");
}
