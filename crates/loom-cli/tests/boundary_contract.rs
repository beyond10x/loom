//! The operator renders the sidecar `boundary` (`crates/loom-operator/src/render.rs`) and this
//! command line runs it (`b10x-loom agent boundary`, `b10x-loom-boundary`). The two are built in
//! different crates, so this test holds them together through the operator's golden file
//! `credential-free.yaml`: the rendered command and probe parse as this command line, the
//! upstreams pass the relay's own declaration checks, and the loopback URLs the agent container
//! gets name the ports the relays listen on (`story:boundary-container-projected-token`,
//! `story:boundary-loopback-relays`).

use std::path::PathBuf;

use b10x_loom_boundary::{CONNECTORS_PORT, GIT_PORT, MODEL_PORT};
use b10x_loom_cli::{AgentCommand, Cli, Command};
use clap::Parser;
use serde::Deserialize;
use serde_yaml_ng::Value;

fn golden() -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../loom-operator/tests/golden/credential-free.yaml");
    let text = std::fs::read_to_string(&path).expect("the operator's credential-free golden file");
    serde_yaml_ng::Deserializer::from_str(&text)
        .map(|document| Value::deserialize(document).expect("a YAML document"))
        .collect()
}

fn pod_spec(documents: &[Value]) -> &Value {
    let job = documents
        .iter()
        .find(|document| document["kind"].as_str() == Some("Job"))
        .expect("the golden file renders a Job");
    &job["spec"]["template"]["spec"]
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_sequence()
        .expect("a list")
        .iter()
        .map(|item| item.as_str().expect("a string").to_owned())
        .collect()
}

fn boundary(spec: &Value) -> &Value {
    spec["initContainers"]
        .as_sequence()
        .expect("init containers")
        .iter()
        .find(|container| container["name"].as_str() == Some("boundary"))
        .expect("the sidecar boundary")
}

#[test]
fn the_rendered_boundary_command_parses_as_this_command_line() {
    let documents = golden();
    let container = boundary(pod_spec(&documents));
    let mut argv = strings(&container["command"]);
    argv.extend(strings(&container["args"]));
    let cli = Cli::try_parse_from(&argv).expect("the rendered command line parses");
    let Command::Agent(agent) = cli.command else {
        panic!("the rendered command is not `agent`");
    };
    let AgentCommand::Boundary(arguments) = agent.command else {
        panic!("the rendered command is not `agent boundary`");
    };
    let model = arguments.model_upstream.expect("a model upstream");
    let connectors = arguments
        .connectors_upstream
        .expect("a connectors upstream");
    let git = arguments.git_upstream.expect("a git upstream");
    for upstream in [&model, &connectors, &git] {
        b10x_loom_boundary::Upstream::parse(upstream)
            .unwrap_or_else(|_| panic!("the relay refuses a rendered upstream"));
    }
    assert!(!arguments.operations.is_empty());
    assert!(!arguments.repositories.is_empty());
}

#[test]
fn the_rendered_probe_parses_and_names_the_relay_ports() {
    let documents = golden();
    let container = boundary(pod_spec(&documents));
    let argv = strings(&container["startupProbe"]["exec"]["command"]);
    let cli = Cli::try_parse_from(&argv).expect("the rendered probe parses");
    let Command::Agent(agent) = cli.command else {
        panic!("the rendered probe is not `agent`");
    };
    let AgentCommand::BoundaryReady(arguments) = agent.command else {
        panic!("the rendered probe is not `agent boundary-ready`");
    };
    assert_eq!(arguments.ports, vec![MODEL_PORT, CONNECTORS_PORT, GIT_PORT]);
}

#[test]
fn the_agent_containers_urls_name_the_relay_ports() {
    let documents = golden();
    let spec = pod_spec(&documents);
    let agent = spec["containers"]
        .as_sequence()
        .expect("containers")
        .iter()
        .find(|container| container["name"].as_str() == Some("loom-agent"))
        .expect("the agent container");
    let env: Vec<(String, String)> = agent["env"]
        .as_sequence()
        .expect("env")
        .iter()
        .filter_map(|entry| {
            Some((
                entry["name"].as_str()?.to_owned(),
                entry["value"].as_str()?.to_owned(),
            ))
        })
        .collect();
    let value = |name: &str| {
        env.iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| panic!("{name} is set"))
    };
    assert_eq!(
        value("LOOM_BOUNDARY_MODEL_URL"),
        format!("http://127.0.0.1:{MODEL_PORT}/v1")
    );
    assert_eq!(
        value("LOOM_BOUNDARY_CONNECTORS_URL"),
        format!("http://127.0.0.1:{CONNECTORS_PORT}")
    );
    assert_eq!(
        value("LOOM_BOUNDARY_GIT_URL"),
        format!("http://127.0.0.1:{GIT_PORT}")
    );
}
