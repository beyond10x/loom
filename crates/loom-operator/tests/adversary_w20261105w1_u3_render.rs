//! Adversary pass on unit U3 of wave 2026-11-05-w1 (`story:schedule-run-deadline`): render and
//! admission of `spec.schedule.runDeadlineSeconds`. Nothing is applied and nothing reaches a
//! cluster.

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

/// The takt `Command` resource of `tests/render.rs` with `schedule` after its members.
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

fn deadline(seconds: &str) -> String {
    command_with(&format!(
        "  schedule: {{cron: '0 3 * * *', runDeadlineSeconds: {seconds}}}\n"
    ))
}

fn refusal(resource: &str) -> String {
    match render_document(resource, &config()) {
        Err(RenderError::Refused(refusal)) => refusal.message(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn job_template_deadline(rendered: &str) -> Option<i64> {
    let docs: Vec<Value> = rendered
        .split("\n---\n")
        .map(|document| serde_yaml_ng::from_str(document).expect("YAML"))
        .collect();
    assert_eq!(docs[2]["kind"], "CronJob", "{rendered}");
    docs[2]["spec"]["jobTemplate"]["spec"]["activeDeadlineSeconds"].as_i64()
}

/// RED. The admission refuses zero because "a run would end as it starts" (`src/admission.rs`,
/// step 18). Kubernetes' Job controller compares the run's age with
/// `time.Duration(activeDeadlineSeconds) * time.Second`, an int64 count of nanoseconds, which
/// wraps negative past 9223372036 seconds: such a deadline ends every run as it starts, the same
/// harm, and it is admitted and rendered as given.
#[test]
fn adv_u3_a_run_deadline_past_what_kubernetes_holds_as_a_duration_is_refused() {
    for seconds in ["9223372037", "9223372036854775807"] {
        let message = refusal(&deadline(seconds));
        assert!(
            message.starts_with("refused: Malformed at spec.schedule.runDeadlineSeconds: "),
            "{seconds}: {message}"
        );
    }
}

/// The smallest deadline admitted is one second, rendered as given (a `<= 1` or `< 2` mutant of
/// the step 18 bound stays green in `tests/render.rs`, which admits no small value).
#[test]
fn adv_u3_a_run_deadline_of_one_second_is_admitted_and_rendered() {
    let rendered = render_document(&deadline("1"), &config()).expect("rendered");
    assert_eq!(job_template_deadline(&rendered), Some(1));
    let rendered = render_document(&deadline("9223372036"), &config()).expect("rendered");
    assert_eq!(job_template_deadline(&rendered), Some(9_223_372_036));
}

/// Step 18 order: a negative starting deadline is refused at its member before a zero run
/// deadline; a cron Kubernetes does not read before both.
#[test]
fn adv_u3_the_schedule_refusals_keep_their_order() {
    let message = refusal(&command_with(
        "  schedule: {cron: '0 3 * * *', startingDeadlineSeconds: -1, runDeadlineSeconds: 0}\n",
    ));
    assert!(
        message.starts_with("refused: Malformed at spec.schedule.startingDeadlineSeconds: "),
        "{message}"
    );
    let message = refusal(&command_with(
        "  schedule: {cron: 'nope', runDeadlineSeconds: 0}\n",
    ));
    assert!(
        message.starts_with("refused: Malformed at spec.schedule.cron: "),
        "{message}"
    );
}

/// The refusal names the member and never repeats the value.
#[test]
fn adv_u3_a_refused_run_deadline_is_not_repeated() {
    let message = refusal(&deadline("-4242"));
    assert!(
        message.starts_with("refused: Malformed at spec.schedule.runDeadlineSeconds: "),
        "{message}"
    );
    assert!(!message.contains("4242"), "{message}");
}

/// A run deadline that is no integer (a quoted `"600"`, a fraction) is not rendered: the CRD
/// declares it `integer`, and the controller renders no CronJob that guesses one.
#[test]
fn adv_u3_a_run_deadline_that_is_no_integer_is_not_rendered() {
    for seconds in ["'600'", "600.5", "true"] {
        assert!(
            render_document(&deadline(seconds), &config()).is_err(),
            "{seconds} is rendered"
        );
    }
}
