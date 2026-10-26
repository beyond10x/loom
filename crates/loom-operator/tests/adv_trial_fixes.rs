//! Adversarial cases for `story:loom-agent-trial-fixes` against the operator: `spec.secretFiles`
//! on either variant and without a workspace, and a serve-variant spec without
//! `runtime.connectors`.

use std::fs;
use std::path::PathBuf;

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

fn pod(rendered: &str) -> Value {
    let deployment: Value = rendered
        .split("\n---\n")
        .map(|document| serde_yaml_ng::from_str::<Value>(document).expect("YAML"))
        .find(|document| matches!(document["kind"].as_str(), Some("Deployment" | "Job")))
        .expect("a Deployment or, for the Command variant, a Job");
    deployment["spec"]["template"]["spec"].clone()
}

fn names(list: &Value) -> Vec<String> {
    list.as_sequence()
        .map(|items| {
            items
                .iter()
                .map(|item| item["name"].as_str().expect("name").to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// The serve variant with `spec.secretFiles` and no workspace: the staging runs before the
/// check, the agent container mounts only the staged subdirectory read-only, never the Secret
/// volume, and `fsGroup` is set by `secretFiles` alone (the takt cases always carry a workspace,
/// so dropping `secret_files.is_some()` from `needs_fs_group` leaves them green).
#[test]
fn adv_serve_variant_with_secret_files_alone_stages_first_and_sets_fs_group() {
    let resource = wrapped(
        &example(),
        "  routes: {}\n  secretRef: {name: platform-operator-secrets}\n  secretFiles: {name: takt-forge, mountPath: /var/run/secrets/forge}\n",
    );
    let pod = pod(&render_document(&resource, &config()).expect("rendered"));
    assert_eq!(
        names(&pod["initContainers"]),
        ["stage-secret-files", "check"]
    );
    assert_eq!(pod["securityContext"]["fsGroup"].as_i64(), Some(10001));
    let container = &pod["containers"][0];
    let mounts = container["volumeMounts"].as_sequence().expect("mounts");
    assert!(
        mounts
            .iter()
            .all(|mount| mount["name"].as_str() != Some("secret-files-source")),
        "the agent container never sees the kubelet-owned Secret volume: {mounts:?}"
    );
    let staged = mounts
        .iter()
        .find(|mount| mount["name"].as_str() == Some("secret-files"))
        .expect("the staged mount");
    assert_eq!(staged["mountPath"].as_str(), Some("/var/run/secrets/forge"));
    assert_eq!(staged["subPath"].as_str(), Some("files"));
    assert_eq!(staged["readOnly"].as_bool(), Some(true));
}

/// The `Command` variant with `secretFiles` and no workspace still gets `fsGroup`, without
/// which the staging container (user 10001) cannot read the Secret volume's 0440 files.
#[test]
fn adv_command_variant_with_secret_files_and_no_workspace_sets_fs_group() {
    let agent = example().replace("protocol: operator-reply@1", "protocol: takt-delivery@1");
    let resource = wrapped(
        &agent,
        "  routes: {}\n  command: {args: [run]}\n  secretFiles: {name: takt-forge, mountPath: /var/run/secrets/takt}\n",
    );
    let pod = pod(&render_document(&resource, &config()).expect("rendered"));
    assert_eq!(names(&pod["initContainers"]), ["stage-secret-files"]);
    assert_eq!(pod["securityContext"]["fsGroup"].as_i64(), Some(10001));
}

/// A `secretFiles.mountPath` equal to the workspace's is refused.
#[test]
fn adv_secret_files_at_the_workspace_path_is_refused() {
    let agent = example().replace("protocol: operator-reply@1", "protocol: takt-delivery@1");
    let resource = wrapped(
        &agent,
        "  routes: {}\n  command: {args: [run]}\n  workspace: {mountPath: /workspace}\n  secretFiles: {name: takt-forge, mountPath: /workspace}\n",
    );
    match render_document(&resource, &config()) {
        Err(RenderError::Refused(refusal)) => assert!(
            refusal
                .message()
                .starts_with("refused: MountPathInvalid at spec.secretFiles.mountPath: "),
            "{}",
            refusal.message()
        ),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// A serve-variant resource whose spec has no `runtime.connectors`: before this unit the
/// decoder refused it at admission (`Malformed`); now it is admitted and rendered, the pod's
/// `check` init container passes, and `agent serve` refuses to start
/// ("`runtime` names no connectors command line"), a crash loop instead of a refusal.
#[test]
fn adv_serve_variant_without_runtime_connectors_is_refused_at_admission() {
    let agent: String = example()
        .lines()
        .filter(|line| !line.trim_start().starts_with("connectors:"))
        .map(|line| format!("{line}\n"))
        .collect();
    assert!(!agent.contains("connectors: {program"));
    let resource = wrapped(
        &agent,
        "  routes: {}\n  secretRef: {name: platform-operator-secrets}\n",
    );
    match render_document(&resource, &config()) {
        Err(RenderError::Refused(_)) => {}
        Ok(rendered) => panic!(
            "admitted and rendered a serve pod whose `agent serve` refuses to start:\n{}",
            names(&pod(&rendered)["initContainers"]).join(",")
        ),
        Err(other) => panic!("expected a refusal, got {other:?}"),
    }
}
