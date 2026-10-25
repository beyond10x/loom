//! Acceptance for `story:loom-operator`: the command line `b10x-loom-operator`, run as a process.
//! Nothing here reaches a cluster: `crd` and `render` read only their arguments.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn operator() -> Command {
    Command::new(env!("CARGO_BIN_EXE_b10x-loom-operator"))
}

#[test]
fn crd_prints_the_projected_definition_byte_for_byte() {
    let output = operator().arg("crd").output().expect("the binary runs");
    assert!(output.status.success(), "{output:?}");
    let file = fs::read(root().join(b10x_loom_resource::crd::FILE)).expect("the CRD file");
    assert_eq!(output.stdout, file);
    assert!(output.stderr.is_empty());
}

#[test]
fn render_of_a_document_that_is_not_a_loom_agent_exits_2_naming_the_file() {
    let spec = root().join("examples/agents/operator.yaml");
    let output = operator()
        .arg("render")
        .arg("--resource")
        .arg(&spec)
        .output()
        .expect("the binary runs");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8");
    assert!(
        stderr.contains(&format!("--resource {}", spec.display())),
        "{stderr}"
    );
    assert!(stderr.contains("not a LoomAgent"), "{stderr}");
}

/// A directory under cargo's per-target test directory, emptied first.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("the test directory");
    dir
}

/// The example spec wrapped as a `LoomAgent`, as `tests/render.rs` wraps it for the golden file.
fn resource(rest: &str) -> String {
    let agent =
        fs::read_to_string(root().join("examples/agents/operator.yaml")).expect("the example");
    let mut out = String::from(
        "apiVersion: loom.beyond10x.dev/v1alpha1\nkind: LoomAgent\nmetadata:\n  name: \
         platform-operator\n  namespace: agents\n  uid: \
         00000000-0000-4000-8000-000000000001\n  generation: 1\nspec:\n  agent:\n",
    );
    for line in agent.lines() {
        out.push_str("    ");
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(rest);
    out
}

const CONFIG: &str = "namespace: agents
defaultImage: {repository: registry.example/loom-agent, tag: '0.40.0'}
defaultResources: {cpu: 100m, memory: 128Mi}
modelRoute: {url: 'http://model-route.agents.svc:8080/v1', protocol: messages, model: small-model}
allowedSecretNames: [platform-operator-secrets]
allowedEgressHosts: [chat.example, models.example]
";

#[test]
fn render_prints_the_children_of_the_golden_file() {
    let dir = scratch("render-golden");
    fs::write(dir.join("config.yaml"), CONFIG).expect("config written");
    fs::write(
        dir.join("agent.yaml"),
        resource("  routes: {}\n  secretRef: {name: platform-operator-secrets}\n"),
    )
    .expect("resource written");
    let output = operator()
        .args(["render", "--resource"])
        .arg(dir.join("agent.yaml"))
        .arg("--config")
        .arg(dir.join("config.yaml"))
        .output()
        .expect("the binary runs");
    assert!(output.status.success(), "{output:?}");
    let golden = fs::read(root().join("crates/loom-operator/tests/golden/platform-operator.yaml"))
        .expect("the golden file");
    assert_eq!(output.stdout, golden);
}

#[test]
fn render_of_a_refused_resource_exits_3_naming_the_member() {
    let dir = scratch("render-refused");
    fs::write(
        dir.join("agent.yaml"),
        resource("  routes: {}\n  secretRef: {name: platform-operator-secrets}\n"),
    )
    .expect("resource written");
    let output = operator()
        .args(["render", "--resource"])
        .arg(dir.join("agent.yaml"))
        .output()
        .expect("the binary runs");
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8");
    assert!(
        stderr.contains("refused: NoModelRoute at spec.routes: "),
        "{stderr}"
    );
}

#[test]
fn render_of_a_missing_file_exits_1_naming_it() {
    let missing = root().join("crates/loom-operator/tests/no-such-resource.yaml");
    let output = operator()
        .args(["render", "--resource"])
        .arg(&missing)
        .output()
        .expect("the binary runs");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8(output.stderr).expect("UTF-8");
    assert!(stderr.contains("no-such-resource.yaml"), "{stderr}");
}

#[test]
fn an_unknown_command_exits_2() {
    let output = operator().arg("apply").output().expect("the binary runs");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
}
