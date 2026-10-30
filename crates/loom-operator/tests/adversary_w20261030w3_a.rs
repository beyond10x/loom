//! Adversary pass 1 on `story:command-resources-declared` (wave 2026-10-30-w3, unit A): which
//! size each container of a `Command` and an `AgentServe` pod gets, the refusal through
//! `render`, its place among the other refusals, the chart's `defaultCommandResources` and the
//! quantity spellings the configuration accepts.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::{
    BOUNDARY_CONTAINER, BOUNDARY_CPU, BOUNDARY_MEMORY, CONTAINER, RenderError, render_document,
};
use serde::Deserialize;
use serde_yaml_ng::Value;

const BIN: &str = env!("CARGO_BIN_EXE_b10x-loom-operator");

/// A configuration without `defaultCommandResources`.
const BASE: &str = "namespace: agents
defaultImage: {repository: registry.example/loom-agent, tag: '0.40.0'}
defaultResources: {cpu: 250m, memory: 256Mi}
allowedSecretNames: [takt-forge]
allowedClaimNames: [takt-workspace]
allowedBoundaryUpstreams: [{kind: model, url: 'http://llm-gateway.llm.svc:8080', audience: llm-gateway}]
";

const COMMAND_DEFAULT: &str = "defaultCommandResources: {cpu: '3', memory: 6Gi}\n";

const REQUIRED: &str = "refused: ResourcesRequired at spec.agent.deploy.kubernetes.resources: ";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn base() -> Config {
    Config::parse(BASE).expect("the base configuration")
}

fn with_default() -> Config {
    Config::parse(&format!("{BASE}{COMMAND_DEFAULT}")).expect("with defaultCommandResources")
}

/// A minimal `loom.agent/1` document, indented as `spec.agent`, with `deploy` when given.
fn agent(deploy: Option<&str>) -> String {
    let mut out = String::from(
        "  agent:
    format: loom.agent/1
    agent: {id: a, name: A, brief: answers}
    model: {routes: routes.yaml, roles: [{role: agent, model: small}]}
    runtime:
      state_dir: /var/lib/agent
      connectors: {program: connectors}
      confinement: none
",
    );
    if let Some(deploy) = deploy {
        out.push_str("    deploy:");
        out.push_str(deploy);
        out.push('\n');
    }
    out
}

fn sized_deploy(cpu: &str, memory: &str) -> String {
    format!(
        "\n      kubernetes: {{namespace: agents, service_account: a, resources: {{cpu: '{cpu}', memory: '{memory}'}}}}"
    )
}

fn resource(spec: &str) -> String {
    format!(
        "apiVersion: loom.beyond10x.dev/v1alpha1
kind: LoomAgent
metadata: {{name: smoke, namespace: agents, uid: 00000000-0000-4000-8000-0000000000aa, generation: 1}}
spec:
{spec}"
    )
}

/// A `Command` resource with every init container the controller renders for one: staged
/// Secret files, the writable directories, a workspace and the boundary sidecar.
fn full_command(agent: &str) -> String {
    resource(&format!(
        "  routes: {{}}
  image: {{repository: registry.example/smoke, tag: '1.0.0'}}
  command: {{args: [--once]}}
  workspace: {{mountPath: /workspace, claimName: takt-workspace}}
{agent}"
    ))
}

const BOUNDARY: &str = "  boundary:\n    model: {upstream: 'http://llm-gateway.llm.svc:8080', audience: llm-gateway}\n";
const SECRET_FILES: &str = "  secretFiles: {name: takt-forge, mountPath: /var/run/secrets/takt}\n";

const GIVEN_ROUTES: &str = "  routes:
    format: llm.routes/1
    endpoints:
      lab: {base_url: 'http://lab.example/v1', protocol: messages, auth: {kind: anonymous}}
    routes: {small: lab/small}
";

fn pod(rendered: &str) -> Value {
    rendered
        .split("\n---\n")
        .map(|part| serde_yaml_ng::from_str::<Value>(part).expect("a rendered document"))
        .find(|document| document["kind"] == "Job" || document["kind"] == "Deployment")
        .expect("a workload")["spec"]["template"]["spec"]
        .clone()
}

/// Every container of the pod by name, with (requests cpu, requests memory, limits cpu,
/// limits memory).
fn sizes(pod: &Value) -> Vec<(String, [String; 4])> {
    let q =
        |c: &Value, k: &str, r: &str| c["resources"][k][r].as_str().unwrap_or("<none>").to_owned();
    ["initContainers", "containers"]
        .iter()
        .flat_map(|list| pod[*list].as_sequence().cloned().unwrap_or_default())
        .map(|c| {
            (
                c["name"].as_str().expect("a name").to_owned(),
                [
                    q(&c, "requests", "cpu"),
                    q(&c, "requests", "memory"),
                    q(&c, "limits", "cpu"),
                    q(&c, "limits", "memory"),
                ],
            )
        })
        .collect()
}

/// Asserts the exact container names and that each but the boundary has `cpu`/`memory` as
/// requests and limits, the boundary its fixed size.
fn assert_sized(rendered: &str, names: &[&str], cpu: &str, memory: &str) {
    let all = sizes(&pod(rendered));
    let got: Vec<&str> = all.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(got, names, "the pod's containers");
    for (name, size) in &all {
        let expected = if name == BOUNDARY_CONTAINER {
            [BOUNDARY_CPU, BOUNDARY_MEMORY, BOUNDARY_CPU, BOUNDARY_MEMORY]
        } else {
            [cpu, memory, cpu, memory]
        };
        assert_eq!(size, &expected.map(str::to_owned), "{name}");
    }
}

fn refusal(text: &str, config: &Config) -> String {
    match render_document(text, config) {
        Err(RenderError::Refused(refusal)) => refusal.message(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary_w20261030w3_a")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

// --- which size each container gets ------------------------------------------------------

#[test]
fn every_command_container_but_the_boundary_gets_the_command_default() {
    let rendered = render_document(
        &full_command(&format!("{}{BOUNDARY}", agent(None))),
        &with_default(),
    )
    .expect("rendered");
    assert_sized(
        &rendered,
        &["prepare-writable-dirs", BOUNDARY_CONTAINER, CONTAINER],
        "3",
        "6Gi",
    );
    let rendered = render_document(&full_command(SECRET_FILES), &with_default()).expect("rendered");
    assert_sized(
        &rendered,
        &["stage-secret-files", "prepare-writable-dirs", CONTAINER],
        "3",
        "6Gi",
    );
}

#[test]
fn a_declared_command_size_reaches_every_container_but_the_boundary_under_any_configuration() {
    for config in [Config::builtin(), base(), with_default()] {
        let config = Config {
            namespace: "agents".to_owned(),
            ..config
        };
        let config = Config {
            allowed_claim_names: vec!["takt-workspace".to_owned()],
            allowed_boundary_upstreams: base().allowed_boundary_upstreams,
            ..config
        };
        let text = full_command(&format!(
            "{}{BOUNDARY}",
            agent(Some(&sized_deploy("7", "9Gi")))
        ));
        let rendered = render_document(&text, &config).expect("rendered");
        assert_sized(
            &rendered,
            &["prepare-writable-dirs", BOUNDARY_CONTAINER, CONTAINER],
            "7",
            "9Gi",
        );
    }
}

#[test]
fn an_agent_serve_pod_gets_default_resources_in_every_container_even_beside_a_command_default() {
    let text = resource(&format!("{GIVEN_ROUTES}{}{SECRET_FILES}", agent(None)));
    let rendered = render_document(&text, &with_default()).expect("rendered");
    assert_sized(
        &rendered,
        &["stage-secret-files", "check", CONTAINER],
        "250m",
        "256Mi",
    );
    let text = resource(&format!("{GIVEN_ROUTES}{}{BOUNDARY}", agent(None)));
    let rendered = render_document(&text, &with_default()).expect("rendered");
    assert_sized(
        &rendered,
        &["check", BOUNDARY_CONTAINER, CONTAINER],
        "250m",
        "256Mi",
    );
}

#[test]
fn an_agent_serve_declared_size_wins_over_default_resources() {
    let text = resource(&format!(
        "{GIVEN_ROUTES}{}",
        agent(Some(&sized_deploy("2", "3Gi")))
    ));
    let rendered = render_document(&text, &with_default()).expect("rendered");
    assert_sized(&rendered, &["check", CONTAINER], "2", "3Gi");
}

#[test]
fn a_command_with_deploy_but_no_kubernetes_is_unsized() {
    let text = resource(&format!(
        "  routes: {{}}\n  command: {{args: [--once]}}\n{}",
        agent(Some(" {}"))
    ));
    let message = refusal(&text, &Config::builtin());
    assert!(message.starts_with(REQUIRED), "{message}");
    let rendered = render_document(&text, &with_default()).expect("rendered");
    assert_sized(&rendered, &["prepare-writable-dirs", CONTAINER], "3", "6Gi");
}

// --- the refusal through render ------------------------------------------------------------

fn run_render(resource_text: &str, config_text: Option<&str>, name: &str) -> std::process::Output {
    let dir = scratch(name);
    let file = dir.join("resource.yaml");
    fs::write(&file, resource_text).expect("resource");
    let mut command = Command::new(BIN);
    command.arg("render").arg("--resource").arg(&file);
    if let Some(config_text) = config_text {
        let config = dir.join("config.yaml");
        fs::write(&config, config_text).expect("config");
        command.arg("--config").arg(config);
    }
    command.output().expect("the binary runs")
}

#[test]
fn render_with_a_config_lacking_or_nulling_the_command_default_exits_3_on_one_line() {
    let text = resource("  routes: {}\n  command: {args: [--once]}\n");
    for (case, config) in [
        ("absent", BASE.to_owned()),
        ("null", format!("{BASE}defaultCommandResources: null\n")),
        ("tilde", format!("{BASE}defaultCommandResources: ~\n")),
    ] {
        let output = run_render(&text, Some(&config), &format!("render-{case}"));
        assert_eq!(output.status.code(), Some(3), "{case}: {output:?}");
        assert!(output.stdout.is_empty(), "{case}: nothing is rendered");
        let stderr = String::from_utf8(output.stderr).expect("UTF-8");
        let lines: Vec<&str> = stderr.lines().collect();
        assert_eq!(lines.len(), 1, "{case}: one line: {stderr}");
        let detail = lines[0]
            .strip_prefix("b10x-loom-operator: ")
            .and_then(|line| line.strip_prefix(REQUIRED))
            .unwrap_or_else(|| panic!("{case}: {stderr}"));
        assert!(
            detail.contains("defaultCommandResources")
                && detail.contains("spec.agent.deploy.kubernetes.resources"),
            "{case}: {detail}"
        );
    }
    let output = run_render(
        &text,
        Some(&format!("{BASE}{COMMAND_DEFAULT}")),
        "render-set",
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

// --- the order against the other refusals ----------------------------------------------------

#[test]
fn an_unsized_command_with_an_earlier_defect_is_refused_for_that_defect() {
    let command = "  routes: {}\n  command: {args: [--once]}\n";
    for (case, extra, code) in [
        (
            "secret",
            "  secretFiles: {name: not-allowed, mountPath: /var/run/secrets/x}\n",
            "SecretNotAllowed",
        ),
        (
            "claim",
            "  workspace: {mountPath: /workspace, claimName: other}\n",
            "ClaimNotAllowed",
        ),
        (
            "mount",
            "  workspace: {mountPath: /tmp}\n",
            "MountPathInvalid",
        ),
        (
            "profile",
            "  seccompProfile: {type: Unconfined}\n",
            "ProfileNotAllowed",
        ),
        (
            "config file",
            "  configFiles: [{name: routes.yaml, content: x}]\n",
            "ConfigFileConflict",
        ),
        (
            "boundary",
            "  boundary:\n    model: {upstream: 'http://other.llm.svc:8080', audience: llm-gateway}\n",
            "BoundaryUpstreamNotAllowed",
        ),
    ] {
        let text = resource(&format!("{command}{extra}"));
        for config in [Config::builtin(), base()] {
            let message = refusal(&text, &config);
            assert!(
                message.starts_with(&format!("refused: {code} at ")),
                "{case}: {message}"
            );
        }
        // With the default the resource is sized, so the same defect is still the refusal.
        let message = refusal(&text, &with_default());
        assert!(
            message.starts_with(&format!("refused: {code} at ")),
            "{case}: {message}"
        );
    }
}

// --- the chart ------------------------------------------------------------------------------

fn helm(sets: &[&str], values: Option<&str>, name: &str) -> std::process::Output {
    let mut command = Command::new("helm");
    command
        .args(["template", "loom-agents"])
        .arg(root().join("charts/loom-agents"))
        .args(["--namespace", "agents"]);
    if let Some(values) = values {
        let file = scratch(name).join("values.yaml");
        fs::write(&file, values).expect("values");
        command.arg("--values").arg(file);
    }
    for set in sets {
        command.args(["--set", set]);
    }
    command.output().expect("helm runs")
}

fn chart_documents(output: &std::process::Output) -> Vec<Value> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout.clone()).expect("UTF-8");
    serde_yaml_ng::Deserializer::from_str(&text)
        .map(|document| Value::deserialize(document).expect("YAML"))
        .filter(|document| !document.is_null())
        .collect()
}

fn chart_config_text(documents: &[Value]) -> String {
    documents
        .iter()
        .find(|d| d["kind"] == "ConfigMap")
        .expect("ConfigMap")["data"]["config.yaml"]
        .as_str()
        .expect("config.yaml")
        .to_owned()
}

#[test]
fn the_chart_refuses_a_partial_command_default_naming_the_missing_member() {
    for (set, missing) in [
        (
            "defaultCommandResources.memory=null",
            "defaultCommandResources.memory",
        ),
        (
            "defaultCommandResources.cpu=null",
            "defaultCommandResources.cpu",
        ),
        (
            "defaultCommandResources.cpu=",
            "defaultCommandResources.cpu",
        ),
    ] {
        let output = helm(&[set], None, "partial");
        assert!(!output.status.success(), "{set}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("{missing} is required")),
            "{set}: {stderr}"
        );
    }
}

#[test]
fn a_values_file_overriding_only_cpu_keeps_the_charts_memory_and_the_controller_reads_both() {
    let documents = chart_documents(&helm(
        &[],
        Some("defaultCommandResources: {cpu: 2}\n"),
        "cpu-only",
    ));
    let config = Config::parse(&chart_config_text(&documents)).expect("the controller's config");
    let size = config.default_command_resources.expect("the member");
    assert_eq!((size.cpu.as_str(), size.memory.as_str()), ("2", "8Gi"));
    let deployment = documents
        .iter()
        .find(|d| d["kind"] == "Deployment")
        .expect("Deployment");
    let pod = &deployment["spec"]["template"]["spec"];
    assert_eq!(
        pod["containers"][0]["args"],
        serde_yaml_ng::from_str::<Value>("[run, --config, /etc/loom-operator/config.yaml]")
            .unwrap()
    );
}

#[test]
fn changing_the_command_default_rolls_the_controller() {
    let checksum = |sets: &[&str]| {
        chart_documents(&helm(sets, None, "checksum"))
            .iter()
            .find(|d| d["kind"] == "Deployment")
            .expect("Deployment")["spec"]["template"]["metadata"]["annotations"]["checksum/config"]
            .as_str()
            .expect("the checksum")
            .to_owned()
    };
    let set = checksum(&[]);
    assert_ne!(set, checksum(&["defaultCommandResources=null"]));
    assert_ne!(set, checksum(&["defaultCommandResources.cpu=2"]));
}

// --- quantity spellings ----------------------------------------------------------------------

/// Spellings Kubernetes refuses as a container's `cpu` or `memory` quantity (the API server's
/// `resource.ParseQuantity`: a signed decimal number and an optional suffix from
/// `m k M G T P E Ki Mi Gi Ti Pi Ei` or a decimal exponent, no space, no unit `B`), or as a
/// limit (negative). The controller takes `defaultCommandResources` verbatim into every
/// unsized `Command` pod, so a configuration holding one admits every such resource and the
/// Job is then rejected at apply, where no refusal names the member.
const NOT_QUANTITIES: [(&str, &str); 6] = [
    ("''", "8Gi"),
    ("' '", "8Gi"),
    ("4 cores", "8Gi"),
    ("'4'", "8GB"),
    ("'4'", "8 Gi"),
    ("'-4'", "8Gi"),
];

#[test]
fn the_configuration_refuses_a_command_default_kubernetes_cannot_parse() {
    let mut admitted = Vec::new();
    for (cpu, memory) in NOT_QUANTITIES {
        let text = format!("{BASE}defaultCommandResources: {{cpu: {cpu}, memory: {memory}}}\n");
        if Config::parse(&text).is_ok() {
            admitted.push(format!("cpu: {cpu}, memory: {memory}"));
        }
    }
    assert!(
        admitted.is_empty(),
        "config.yaml accepts defaultCommandResources Kubernetes refuses: {admitted:?}"
    );
}

#[test]
fn the_chart_does_not_hand_the_controller_a_command_default_kubernetes_cannot_parse() {
    let mut admitted = Vec::new();
    for set in [
        "defaultCommandResources.memory=8GB",
        "defaultCommandResources.cpu=4 cores",
        "defaultCommandResources.cpu= ",
    ] {
        let output = helm(&[set], None, "chart-quantity");
        if !output.status.success() {
            continue;
        }
        let documents = chart_documents(&output);
        if Config::parse(&chart_config_text(&documents)).is_ok() {
            admitted.push(set);
        }
    }
    assert!(
        admitted.is_empty(),
        "helm template and the controller's config both accept: {admitted:?}"
    );
}

#[test]
fn a_blank_size_is_refused_from_the_configuration_as_from_the_resource() {
    // The resource's own declaration refuses a blank cpu (`BlankText`) ...
    let text = resource(&format!(
        "  routes: {{}}\n  command: {{args: [--once]}}\n{}",
        agent(Some(&sized_deploy(" ", "1Gi")))
    ));
    assert!(
        render_document(&text, &with_default()).is_err(),
        "a blank declared cpu"
    );
    // ... and so must the configuration's default the controller puts in its place.
    let config = Config::parse(&format!(
        "{BASE}defaultCommandResources: {{cpu: ' ', memory: 1Gi}}\n"
    ));
    let bare = resource("  routes: {}\n  command: {args: [--once]}\n");
    let rendered = config
        .as_ref()
        .ok()
        .map(|config| render_document(&bare, config));
    assert!(
        !matches!(rendered, Some(Ok(_))),
        "a blank defaultCommandResources.cpu renders an unsized Command pod: {:?}",
        rendered.map(|r| r.map(|t| sizes(&pod(&t))))
    );
}
