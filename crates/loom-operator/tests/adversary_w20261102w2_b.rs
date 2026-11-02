//! Adversary pass 1 on unit B of wave 2026-11-02-w2 (`story:command-schedule`): the rendered
//! CronJob and step 18 of admission, driven through `render_document` as `tests/render.rs` does.
//! Nothing reaches a cluster.

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

fn config() -> Config {
    Config::parse(CONFIG).expect("the test configuration")
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn example() -> String {
    fs::read_to_string(root().join("examples/agents/operator.yaml")).expect("the example spec")
}

/// The example without `expose`, naming a protocol only the image's program knows.
fn takt_agent() -> String {
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

/// `agent` wrapped as the `LoomAgent` `platform-operator`, with `rest` after `spec.agent`.
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

/// A `Command` resource running `b10x-loom agent run` over its spec, with `schedule` after it.
fn agent_run_with(schedule: &str) -> String {
    wrapped(
        &takt_agent(),
        &format!("  routes: {{}}\n  command: {{}}\n{schedule}"),
    )
}

fn documents(rendered: &str) -> Vec<Value> {
    rendered
        .split("\n---\n")
        .map(|document| serde_yaml_ng::from_str(document).expect("each document is YAML"))
        .collect()
}

fn at<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    path.iter()
        .fold(value, |value, key| match key.parse::<usize>() {
            Ok(index) => &value[index],
            Err(_) => &value[*key],
        })
}

fn refusal(resource: &str) -> String {
    match render_document(resource, &config()) {
        Err(RenderError::Refused(refusal)) => refusal.message(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// The document of `kind` among the rendered ones.
fn of_kind(rendered: &str, kind: &str) -> Value {
    documents(rendered)
        .into_iter()
        .find(|doc| doc["kind"].as_str() == Some(kind))
        .unwrap_or_else(|| panic!("a {kind} in {rendered}"))
}

/// Kubernetes reads a schedule with robfig/cron's standard parser on the string as given: a
/// descriptor is matched exactly (`@hourly ` is no descriptor) and `strings.Fields` of
/// ` @daily` is one field, not five. Admission trims before it checks, and the CronJob carries
/// the untrimmed text, so the API server refuses at apply what admission admitted. Either
/// admission refuses it at `spec.schedule.cron`, or the CronJob carries what was checked.
#[test]
fn adv_b_a_descriptor_with_surrounding_whitespace_is_refused_or_rendered_as_checked() {
    for cron in ["@hourly ", " @daily", "@weekly\n"] {
        let resource = agent_run_with(&format!(
            "  schedule: {{cron: {}}}\n",
            serde_json::to_string(cron).expect("JSON")
        ));
        match render_document(&resource, &config()) {
            Err(RenderError::Refused(refusal)) => {
                let message = refusal.message();
                assert!(
                    message.starts_with("refused: Malformed at spec.schedule.cron: "),
                    "{cron:?}: {message}"
                );
            }
            Ok(rendered) => {
                let schedule = at(&of_kind(&rendered, "CronJob"), &["spec", "schedule"])
                    .as_str()
                    .expect("a schedule")
                    .to_owned();
                assert_eq!(
                    schedule,
                    schedule.trim(),
                    "{cron:?} was admitted and the CronJob carries {schedule:?}, which \
                     Kubernetes's cron parser refuses"
                );
            }
            Err(other) => panic!("{cron:?}: {other:?}"),
        }
    }
}

/// Kubernetes refuses a CronJob whose name is longer than 52 characters (the Job it starts
/// appends 11), while a Job may be named up to 63. A `Command` resource of a 53-character name
/// runs as a Job; with `spec.schedule` it must be refused at admission or render a CronJob of a
/// name Kubernetes admits, never one the API server refuses at every reconcile.
#[test]
fn adv_b_a_resource_name_a_cronjob_cannot_carry_is_refused_or_not_rendered_as_the_name() {
    let long = format!("platform-operator-{}", "x".repeat(35));
    assert_eq!(long.len(), 53);
    let rename = |resource: String| {
        let renamed = resource.replace(
            "metadata:\n  name: platform-operator\n",
            &format!("metadata:\n  name: {long}\n"),
        );
        assert_ne!(renamed, resource);
        renamed
    };
    let once = render_document(&rename(agent_run_with("")), &config())
        .expect("the Job of a 53-character name renders");
    assert_eq!(
        at(&of_kind(&once, "Job"), &["metadata", "name"]).as_str(),
        Some(long.as_str())
    );
    match render_document(
        &rename(agent_run_with("  schedule: {cron: '*/20 * * * *'}\n")),
        &config(),
    ) {
        Err(RenderError::Refused(_)) => {}
        Ok(rendered) => {
            let cron = of_kind(&rendered, "CronJob");
            let name = at(&cron, &["metadata", "name"]).as_str().expect("a name");
            assert!(
                name.len() <= 52,
                "the CronJob is named {name:?}, {} characters; Kubernetes admits 52",
                name.len()
            );
        }
        Err(other) => panic!("{other:?}"),
    }
}

/// A text of whitespace only is blank: refused at `spec.schedule.input`.
#[test]
fn adv_b_a_whitespace_text_is_refused_at_the_input() {
    for text in ["   ", "\n\t \n", "\u{a0}"] {
        let message = refusal(&agent_run_with(&format!(
            "  schedule:\n    cron: '0 3 * * *'\n    input: {{text: {}}}\n",
            serde_json::to_string(text).expect("JSON")
        )));
        assert!(
            message.starts_with("refused: Malformed at spec.schedule.input: "),
            "{text:?}: {message}"
        );
    }
}

/// A key naming a path, a name that is no ConfigMap name, and both padded with whitespace are
/// refused at `spec.schedule.input`, never repeating the value; a 253-byte DNS subdomain name
/// and a 253-byte key are admitted.
#[test]
fn adv_b_a_config_map_key_ref_that_names_a_path_or_no_config_map_is_refused() {
    let too_long = format!("{}.{}", "a".repeat(63), "b".repeat(190));
    for (name, key) in [
        ("tasks", "../x"),
        ("tasks", "a/b"),
        ("tasks", "/etc/passwd"),
        ("tasks", " nightly"),
        (" tasks", "nightly"),
        ("tasks.", "nightly"),
        ("-tasks", "nightly"),
        ("Tasks", "nightly"),
        ("tasks/x", "nightly"),
        (too_long.as_str(), "nightly"),
        ("tasks", &"k".repeat(254)),
    ] {
        let message = refusal(&agent_run_with(&format!(
            "  schedule:\n    cron: '0 3 * * *'\n    input: {{configMapKeyRef: {{name: {}, key: {}}}}}\n",
            serde_json::to_string(name).expect("JSON"),
            serde_json::to_string(key).expect("JSON"),
        )));
        assert!(
            message.starts_with("refused: Malformed at spec.schedule.input: "),
            "{name:?} {key:?}: {message}"
        );
        for value in ["../x", "a/b", "/etc/passwd", "tasks/x"] {
            assert!(!message.contains(value), "{message}");
        }
    }
    let longest = format!(
        "{}.{}.{}.{}",
        "a".repeat(63),
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(61)
    );
    assert_eq!(longest.len(), 253);
    let key = "k".repeat(253);
    let rendered = render_document(
        &agent_run_with(&format!(
            "  schedule:\n    cron: '0 3 * * *'\n    input: {{configMapKeyRef: {{name: {longest}, key: {key}}}}}\n"
        )),
        &config(),
    )
    .expect("the longest names Kubernetes admits");
    let cron = of_kind(&rendered, "CronJob");
    let volumes = at(
        &cron,
        &["spec", "jobTemplate", "spec", "template", "spec", "volumes"],
    )
    .as_sequence()
    .expect("volumes");
    let volume = volumes
        .iter()
        .find(|volume| volume["name"].as_str() == Some("schedule-input"))
        .expect("the input's volume");
    assert_eq!(volume["configMap"]["name"].as_str(), Some(longest.as_str()));
    assert_eq!(
        volume["configMap"]["items"][0]["key"].as_str(),
        Some(key.as_str())
    );
}

/// A credential on a later line of a multi-line text, or behind `Authorization: Bearer`, is
/// refused `SecretLiteral` at `spec.schedule.input.text`, never repeating it.
#[test]
fn adv_b_a_credential_anywhere_in_a_multi_line_text_is_refused() {
    for text in [
        "check the queue\nthen push with ghp_000fake\n",
        "check the queue\n\n  token=xoxb-000-fake",
        "call it with user:ghp_000fake please",
    ] {
        let message = refusal(&agent_run_with(&format!(
            "  schedule:\n    cron: '0 3 * * *'\n    input: {{text: {}}}\n",
            serde_json::to_string(text).expect("JSON")
        )));
        assert!(
            message.starts_with("refused: SecretLiteral at spec.schedule.input.text: "),
            "{text:?}: {message}"
        );
        for value in ["000fake", "000-fake"] {
            assert!(!message.contains(value), "{message}");
        }
    }
}

/// `resource-schedule/input-inline`: the text lands in the resource's ConfigMap byte for byte,
/// leading and trailing whitespace and newlines kept; the program container mounts that whole
/// ConfigMap at `/config` (no `items` that would leave the key out), and `--input` names the key
/// there.
#[test]
fn adv_b_the_inline_text_reaches_the_file_agent_run_reads_byte_for_byte() {
    let text = "  check the queue\n\treport what changed  \n";
    let rendered = render_document(
        &agent_run_with(&format!(
            "  schedule:\n    cron: '0 3 * * *'\n    input: {{text: {}}}\n",
            serde_json::to_string(text).expect("JSON")
        )),
        &config(),
    )
    .expect("rendered");
    let config_map = of_kind(&rendered, "ConfigMap");
    assert_eq!(
        config_map["data"]["schedule-input.txt"].as_str(),
        Some(text)
    );
    let cron = of_kind(&rendered, "CronJob");
    let pod = at(&cron, &["spec", "jobTemplate", "spec", "template", "spec"]);
    let container = &pod["containers"][0];
    let command: Vec<&str> = container["command"]
        .as_sequence()
        .expect("a command")
        .iter()
        .map(|item| item.as_str().expect("a string"))
        .collect();
    let input = command
        .iter()
        .position(|item| *item == "--input")
        .and_then(|index| command.get(index + 1))
        .expect("--input <file>");
    let mount = container["volumeMounts"]
        .as_sequence()
        .expect("mounts")
        .iter()
        .find(|mount| mount["name"].as_str() == Some("spec"))
        .expect("the spec mount");
    let dir = mount["mountPath"].as_str().expect("a path");
    assert_eq!(*input, format!("{dir}/schedule-input.txt"));
    let volume = pod["volumes"]
        .as_sequence()
        .expect("volumes")
        .iter()
        .find(|volume| volume["name"].as_str() == Some("spec"))
        .expect("the spec volume");
    assert_eq!(
        volume["configMap"]["name"].as_str(),
        config_map["metadata"]["name"].as_str()
    );
    assert!(volume["configMap"]["items"].is_null(), "{volume:?}");
    assert!(mount["subPath"].is_null(), "{mount:?}");
}

/// The text counts toward the ConfigMap's 1 MiB: a text that fills it is `TooLarge`, before
/// anything is rendered.
#[test]
fn adv_b_a_text_beyond_the_config_maps_limit_is_too_large() {
    let text = "check ".repeat(1024 * 1024 / 6 + 1);
    let message = refusal(&agent_run_with(&format!(
        "  schedule:\n    cron: '0 3 * * *'\n    input: {{text: '{text}'}}\n"
    )));
    assert!(
        message.starts_with("refused: TooLarge at "),
        "{}",
        &message[..message.len().min(200)]
    );
}

/// Bounds of each field, by number and by name: the first and last value admitted, one past
/// either refused, a name in a field that has none refused.
#[test]
fn adv_b_cron_field_bounds_and_names() {
    for cron in [
        "0-59 0-23 1-31 1-12 0-6",
        "0 0 1 jan-dec sun-sat",
        "0 0 1 DEC SAT",
        "*/59 */23 */31 */12 */6",
        "0 0 ? * ?",
    ] {
        render_document(
            &agent_run_with(&format!("  schedule: {{cron: '{cron}'}}\n")),
            &config(),
        )
        .unwrap_or_else(|error| panic!("{cron:?}: {error:?}"));
    }
    for cron in [
        "0-60 * * * *",
        "* 0-24 * * *",
        "* * 0-31 * *",
        "* * 1-32 * *",
        "* * * 0-12 *",
        "* * * 1-13 *",
        "* * * * 0-7",
        "MON * * * *",
        "* * * MON *",
        "* * * * JAN",
        "* * * * SUNDAY",
        "1-2-3 * * * *",
        "*/1/2 * * * *",
        "*/-1 * * * *",
        "-1 * * * *",
        "0 0 L * *",
        "0 0 * * 1#2",
    ] {
        let message = refusal(&agent_run_with(&format!(
            "  schedule: {{cron: '{cron}'}}\n"
        )));
        assert!(
            message.starts_with("refused: Malformed at spec.schedule.cron: "),
            "{cron:?}: {message}"
        );
    }
}

/// A starting deadline of 0 is admitted and reaches the CronJob as 0; a time zone with a sign
/// in it is admitted.
#[test]
fn adv_b_a_zero_starting_deadline_and_a_signed_zone_reach_the_cronjob() {
    let rendered = render_document(
        &agent_run_with(
            "  schedule: {cron: '0 3 * * *', startingDeadlineSeconds: 0, timeZone: Etc/GMT+5}\n",
        ),
        &config(),
    )
    .expect("rendered");
    let cron = of_kind(&rendered, "CronJob");
    assert_eq!(
        at(&cron, &["spec", "startingDeadlineSeconds"]).as_i64(),
        Some(0)
    );
    assert_eq!(at(&cron, &["spec", "timeZone"]).as_str(), Some("Etc/GMT+5"));
}

/// An `AgentServe` resource with a schedule carrying an input is refused at `spec.schedule`, the
/// variant's rule, whatever the input holds.
#[test]
fn adv_b_a_schedule_with_an_input_on_agent_serve_is_refused_at_the_schedule() {
    let resource = wrapped(
        &example(),
        "  routes: {}\n  secretRef: {name: platform-operator-secrets}\n  schedule:\n    cron: '0 3 * * *'\n    input: {text: check the queue}\n",
    );
    let message = refusal(&resource);
    assert!(
        message.starts_with("refused: Malformed at spec.schedule: "),
        "{message}"
    );
}
