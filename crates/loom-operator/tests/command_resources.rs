//! Acceptance for `story:command-resources-declared`: a `Command` `LoomAgent` runs with the CPU
//! and memory its `spec.agent.deploy.kubernetes.resources` declares, else with the controller
//! configuration's `defaultCommandResources`; with neither it is refused `ResourcesRequired` at
//! `spec.agent.deploy.kubernetes.resources` before anything is rendered. `defaultResources` stays
//! the size of an `AgentServe` pod, and the boundary sidecar keeps its own fixed size.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::{
    BOUNDARY_CONTAINER, BOUNDARY_CPU, BOUNDARY_MEMORY, RenderError, render_document,
};
use serde_yaml_ng::Value;

/// A configuration without `defaultCommandResources`, allowing what the fixtures name.
const CONFIG: &str = "namespace: agents
defaultImage: {repository: registry.example/loom-agent, tag: '0.40.0'}
defaultResources: {cpu: 250m, memory: 256Mi}
allowedSecretNames: [takt-forge]
allowedEgressHosts: [chat.example, models.example]
allowedBoundaryUpstreams: [{kind: model, url: 'http://llm-gateway.llm.svc:8080', audience: llm-gateway}]
";

/// The member `CONFIG` gains to size a `Command` resource that declares nothing.
const COMMAND_DEFAULT: &str = "defaultCommandResources: {cpu: '4', memory: 8Gi}\n";

/// The refusal of a `Command` resource no one sized.
const REQUIRED: &str = "refused: ResourcesRequired at spec.agent.deploy.kubernetes.resources: ";

const HEAD: &str = "apiVersion: loom.beyond10x.dev/v1alpha1
kind: LoomAgent
metadata:
  name: smoke
  namespace: agents
  uid: 00000000-0000-4000-8000-000000000002
  generation: 1
spec:
";

/// The `Command` members after `spec.agent`: an image and its program's arguments, nothing any
/// configuration must allow.
const BARE: &str = "  routes: {}
  image: {repository: registry.example/smoke, tag: '1.0.0'}
  command: {args: [--once]}
";

/// [`BARE`] and a Secret as files, so the pod has an init container besides the program's.
const COMMAND: &str = "  routes: {}
  image: {repository: registry.example/smoke, tag: '1.0.0'}
  command: {args: [--once]}
  secretFiles: {name: takt-forge, mountPath: /var/run/secrets/takt}
";

/// The `spec.routes` an `AgentServe` resource is given, so no model route is needed.
const GIVEN_ROUTES: &str = "  routes:
    format: llm.routes/1
    endpoints:
      lab: {base_url: 'http://lab.example/v1', protocol: messages, auth: {kind: anonymous}}
    routes: {small: lab/small}
";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn example() -> String {
    fs::read_to_string(root().join("examples/agents/operator.yaml")).expect("the example spec")
}

/// The example without its top-level `blocks` (each key and its indented lines).
fn without(blocks: &[&str]) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in example().lines() {
        if blocks
            .iter()
            .any(|block| line.starts_with(&format!("{block}:")))
        {
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

/// A `LoomAgent` holding `agent` as `spec.agent`, if any, then `rest`.
fn resource(agent: Option<&str>, rest: &str) -> String {
    let mut out = String::from(HEAD);
    if let Some(agent) = agent {
        out.push_str("  agent:\n");
        for line in agent.lines() {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str(rest);
    out
}

/// A `Command` resource without `spec.agent`, followed by `rest`.
fn command_without_agent(rest: &str) -> String {
    resource(None, rest)
}

/// A `Command` resource whose `spec.agent` has no `deploy`, followed by `rest`.
fn command_without_deploy(rest: &str) -> String {
    resource(Some(&without(&["expose", "deploy"])), rest)
}

/// A `Command` resource whose `spec.agent.deploy.kubernetes.resources` is `cpu` and `memory`.
fn command_declaring(cpu: &str, memory: &str) -> String {
    let agent = without(&["expose"]).replace(
        "resources: {cpu: 250m, memory: 256Mi}",
        &format!("resources: {{cpu: '{cpu}', memory: {memory}}}"),
    );
    assert!(agent.contains(&format!("memory: {memory}")));
    resource(Some(&agent), COMMAND)
}

/// An `AgentServe` resource whose `spec.agent` has no `deploy`.
fn serve_without_deploy() -> String {
    resource(Some(&without(&["deploy"])), GIVEN_ROUTES)
}

fn config() -> Config {
    Config::parse(CONFIG).expect("the test configuration")
}

fn with_command_default() -> Config {
    Config::parse(&format!("{CONFIG}{COMMAND_DEFAULT}"))
        .expect("the test configuration with defaultCommandResources")
}

fn refusal(resource: &str, config: &Config) -> String {
    match render_document(resource, config) {
        Err(RenderError::Refused(refusal)) => refusal.message(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// The pod template's spec of the workload, the third child.
fn pod(rendered: &str) -> Value {
    let workload = rendered
        .split("\n---\n")
        .nth(2)
        .expect("a third child, the workload");
    let workload: Value = serde_yaml_ng::from_str(workload).expect("YAML");
    workload["spec"]["template"]["spec"].clone()
}

/// CPU and memory, as Kubernetes quantities.
type Size = (String, String);

/// Each container of `pod`, init containers first, by name with its requests and limits.
fn sizes(pod: &Value) -> Vec<(String, Size, Size)> {
    let quantity = |resources: &Value, key: &str| {
        (
            resources[key]["cpu"].as_str().expect("cpu").to_owned(),
            resources[key]["memory"]
                .as_str()
                .expect("memory")
                .to_owned(),
        )
    };
    ["initContainers", "containers"]
        .iter()
        .flat_map(|list| pod[*list].as_sequence().cloned().unwrap_or_default())
        .map(|container| {
            let resources = &container["resources"];
            (
                container["name"].as_str().expect("a name").to_owned(),
                quantity(resources, "requests"),
                quantity(resources, "limits"),
            )
        })
        .collect()
}

/// Asserts every container of `rendered`'s pod but the boundary sidecar requests and is limited
/// to `cpu` and `memory`, and that there is at least one init container among them.
fn sized(rendered: &str, cpu: &str, memory: &str) {
    let pod = pod(rendered);
    let init = pod["initContainers"]
        .as_sequence()
        .map_or(0, |init| init.len());
    assert!(init >= 1, "the fixture renders an init container: {pod:?}");
    let expected = (cpu.to_owned(), memory.to_owned());
    for (name, requests, limits) in sizes(&pod) {
        if name == BOUNDARY_CONTAINER {
            continue;
        }
        assert_eq!(requests, expected, "{name} requests");
        assert_eq!(limits, expected, "{name} limits");
    }
}

#[test]
fn a_command_resource_without_resources_is_refused_resources_required() {
    for (case, resource) in [
        ("without spec.agent", command_without_agent(BARE)),
        ("without deploy", command_without_deploy(BARE)),
    ] {
        for (config_name, config) in [("built-in", Config::builtin()), ("config.yaml", config())] {
            let message = refusal(&resource, &config);
            assert!(
                message.starts_with(REQUIRED),
                "{case}, {config_name}: {message}"
            );
            let detail = &message[REQUIRED.len()..];
            assert!(
                detail.contains(
                    "declare its cpu and memory in spec.agent.deploy.kubernetes.resources"
                ) && detail
                    .contains("set defaultCommandResources in the controller's configuration",),
                "{case}, {config_name}: the detail names the member to set and the \
                 configuration's default: {message}"
            );
        }
    }
}

/// A directory under cargo's per-target test directory, emptied first.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("the test directory");
    dir
}

#[test]
fn render_without_config_refuses_an_unsized_command_resource_with_exit_3() {
    let dir = scratch("command-resources-render");
    let file = dir.join("smoke.yaml");
    fs::write(&file, command_without_agent(BARE)).expect("the resource is written");
    let output = Command::new(env!("CARGO_BIN_EXE_b10x-loom-operator"))
        .arg("render")
        .arg("--resource")
        .arg(&file)
        .output()
        .expect("the binary runs");
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    assert!(output.stdout.is_empty(), "nothing is rendered: {output:?}");
    let stderr = String::from_utf8(output.stderr).expect("UTF-8");
    assert!(
        stderr.lines().any(|line| line
            .strip_prefix("b10x-loom-operator: ")
            .is_some_and(|line| line.starts_with(REQUIRED))),
        "{stderr}"
    );
}

#[test]
fn a_command_resource_gets_the_configured_command_default() {
    for resource in [
        command_without_agent(COMMAND),
        command_without_deploy(COMMAND),
    ] {
        let rendered = render_document(&resource, &with_command_default()).expect("rendered");
        sized(&rendered, "4", "8Gi");
    }
}

#[test]
fn a_declared_size_wins_over_every_default() {
    for config in [config(), with_command_default(), Config::builtin()] {
        let config = Config {
            allowed_secret_names: vec!["takt-forge".to_owned()],
            allowed_egress_hosts: vec!["chat.example".to_owned(), "models.example".to_owned()],
            namespace: "agents".to_owned(),
            ..config
        };
        let rendered = render_document(&command_declaring("8", "16Gi"), &config).expect("rendered");
        sized(&rendered, "8", "16Gi");
    }
}

#[test]
fn an_agent_serve_resource_keeps_the_default_resources() {
    let rendered = render_document(&serve_without_deploy(), &Config::builtin()).expect("rendered");
    sized(&rendered, "250m", "256Mi");
    let rendered =
        render_document(&serve_without_deploy(), &with_command_default()).expect("rendered");
    sized(&rendered, "250m", "256Mi");
}

#[test]
fn the_boundary_sidecar_keeps_its_own_size() {
    let boundary = "  boundary:\n    model: {upstream: 'http://llm-gateway.llm.svc:8080', audience: llm-gateway}\n";
    let rest = COMMAND.replace(
        "  secretFiles: {name: takt-forge, mountPath: /var/run/secrets/takt}\n",
        boundary,
    );
    let rendered =
        render_document(&resource(None, &rest), &with_command_default()).expect("rendered");
    sized(&rendered, "4", "8Gi");
    let pod = pod(&rendered);
    let sidecar = sizes(&pod)
        .into_iter()
        .find(|(name, _, _)| name == BOUNDARY_CONTAINER)
        .expect("the boundary sidecar");
    let fixed = (BOUNDARY_CPU.to_owned(), BOUNDARY_MEMORY.to_owned());
    assert_eq!((sidecar.1, sidecar.2), (fixed.clone(), fixed));
}

#[test]
fn default_command_resources_is_optional_and_a_misspelt_member_is_refused() {
    assert_eq!(
        Config::parse(CONFIG).expect("absent is a configuration"),
        config()
    );
    for (text, named) in [
        (
            "defaultCommandResource: {cpu: '4', memory: 8Gi}\n",
            "defaultCommandResource",
        ),
        ("defaultCommandResources: {cpu: '4', mem: 8Gi}\n", "mem"),
        ("defaultCommandResources: {cpu: '4'}\n", "memory"),
    ] {
        let error = Config::parse(&format!("{CONFIG}{text}")).expect_err(text);
        assert!(error.0.contains(named), "{text}: {error}");
    }
}

/// A configuration whose `defaultResources` is `serve` and whose `defaultCommandResources` is
/// `command`, each `(cpu, memory)` written as a quoted YAML string.
fn sized_config(serve: (&str, &str), command: (&str, &str)) -> String {
    let quoted = |text: &str| serde_json::to_string(text).expect("JSON text");
    format!(
        "namespace: agents\ndefaultImage: {{repository: registry.example/loom-agent, tag: \
         '0.40.0'}}\ndefaultResources: {{cpu: {}, memory: {}}}\ndefaultCommandResources: {{cpu: \
         {}, memory: {}}}\n",
        quoted(serve.0),
        quoted(serve.1),
        quoted(command.0),
        quoted(command.1)
    )
}

/// Spellings Kubernetes does not read as a non-negative quantity.
const NOT_QUANTITIES: [&str; 12] = [
    "", " ", "4 cores", "8GB", "8 Gi", "-4", "4m ", "Gi", ".", "1e", "1.2.3", "8gi",
];

/// Spellings it does.
const QUANTITIES: [&str; 14] = [
    "250m", "4", "0", "8Gi", "0.5", "1.5Gi", ".5", "2.", "1e3", "2E-3", "100M", "2k", "1Ei", "500u",
];

#[test]
fn a_size_in_the_configuration_that_is_no_kubernetes_quantity_is_refused_naming_member_and_value() {
    let good = ("1", "1Gi");
    for value in NOT_QUANTITIES {
        for (member, serve, command) in [
            ("defaultResources.cpu", (value, "1Gi"), good),
            ("defaultResources.memory", ("1", value), good),
            ("defaultCommandResources.cpu", good, (value, "1Gi")),
            ("defaultCommandResources.memory", good, ("1", value)),
        ] {
            let error = Config::parse(&sized_config(serve, command))
                .expect_err(&format!("{member}: {value:?}"));
            assert_eq!(
                error.0,
                format!("{member}: {value:?} is not a Kubernetes quantity"),
                "{member}"
            );
        }
    }
    for value in QUANTITIES {
        let config = Config::parse(&sized_config((value, value), (value, value)))
            .unwrap_or_else(|error| panic!("{value:?} is a quantity: {error}"));
        assert_eq!(
            config.default_command_resources.map(|size| size.memory),
            Some(value.to_owned())
        );
    }
}

#[test]
fn render_and_run_refuse_a_configuration_holding_no_quantity() {
    let dir = scratch("command-resources-bad-config");
    let config = dir.join("config.yaml");
    fs::write(&config, sized_config(("1", "1Gi"), ("4", "8GB"))).expect("the config");
    let resource = dir.join("smoke.yaml");
    fs::write(&resource, command_without_agent(BARE)).expect("the resource");
    let expected = format!(
        "b10x-loom-operator: --config {}: defaultCommandResources.memory: \"8GB\" is not a \
         Kubernetes quantity\n",
        config.display()
    );
    for (command, args) in [
        (
            "render",
            vec!["--resource", resource.to_str().expect("UTF-8")],
        ),
        ("run", Vec::new()),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_b10x-loom-operator"))
            .arg(command)
            .args(args)
            .arg("--config")
            .arg(&config)
            .output()
            .expect("the binary runs");
        assert_eq!(output.status.code(), Some(1), "{command}: {output:?}");
        assert!(output.stdout.is_empty(), "{command}: {output:?}");
        assert_eq!(
            String::from_utf8(output.stderr).expect("UTF-8"),
            expected,
            "{command}"
        );
    }
}
