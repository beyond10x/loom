//! Acceptance for `story:loom-operator`: `render` turns one `LoomAgent` into the children the
//! design's § 4 draws, as one multi-document YAML, and refuses before rendering what the
//! controller's admission refuses. Nothing is applied and nothing reaches a cluster.
//!
//! `examples/agents/operator.yaml` is wrapped as a `LoomAgent` here, so the golden file follows
//! the example. `LOOM_OPERATOR_BLESS=1` rewrites the golden file from the current rendering; the
//! structural assertions below hold either way.

use std::fs;
use std::path::PathBuf;

use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::{RenderError, render_document};
use serde_yaml_ng::Value;

/// The controller configuration the golden file is rendered with.
const CONFIG: &str = "namespace: agents
defaultImage: {repository: registry.example/loom-agent, tag: '0.40.0'}
defaultResources: {cpu: 100m, memory: 128Mi}
modelRoute: {url: 'http://model-route.agents.svc:8080/v1', protocol: messages, model: small-model}
allowedSecretNames: [platform-operator-secrets]
allowedEgressHosts: [chat.example, models.example]
allowedClaimNames: [takt-workspace]
";

/// A routes document holding a token: a clearly fake one, never a real credential.
const ROUTES_WITH_TOKEN: &str = "  routes:
    format: llm.routes/1
    endpoints:
      lab: {base_url: 'http://lab.example/v1', protocol: messages, auth: {kind: bearer, secret: xoxb-000-fake}}
    routes: {small: lab/small}
";

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

fn config() -> Config {
    Config::parse(CONFIG).expect("the test configuration")
}

/// `agent` wrapped as the `LoomAgent` `platform-operator` in namespace `agents`, with `rest`
/// (further `spec` members, indented two spaces) after `spec.agent`.
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

fn default_resource() -> String {
    wrapped(
        &example(),
        "  routes: {}\n  secretRef: {name: platform-operator-secrets}\n",
    )
}

/// The example without its `expose` block.
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

fn text<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    at(value, path)
        .as_str()
        .unwrap_or_else(|| panic!("{path:?} is a string in {value:?}"))
}

fn strings(value: &Value, path: &[&str]) -> Vec<String> {
    at(value, path)
        .as_sequence()
        .unwrap_or_else(|| panic!("{path:?} is a list"))
        .iter()
        .map(|item| item.as_str().expect("a string").to_owned())
        .collect()
}

fn refusal(resource: &str, config: &Config) -> String {
    match render_document(resource, config) {
        Err(RenderError::Refused(refusal)) => refusal.message(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn the_operator_example_renders_as_the_golden_file() {
    let rendered = render_document(&default_resource(), &config()).expect("rendered");
    let golden = root().join("crates/loom-operator/tests/golden/platform-operator.yaml");
    if std::env::var_os("LOOM_OPERATOR_BLESS").is_some() {
        fs::write(&golden, &rendered).expect("the golden file is written");
    }
    let expected = fs::read_to_string(&golden).expect("the golden file");
    assert_eq!(
        rendered,
        expected,
        "render differs from {}",
        golden.display()
    );
}

#[test]
fn the_children_are_the_pod_of_design_section_4() {
    let rendered = render_document(&default_resource(), &config()).expect("rendered");
    let docs = documents(&rendered);
    let kinds: Vec<&str> = docs.iter().map(|doc| text(doc, &["kind"])).collect();
    assert_eq!(
        kinds,
        [
            "ServiceAccount",
            "ConfigMap",
            "Deployment",
            "NetworkPolicy",
            "Service"
        ]
    );
    for doc in &docs {
        assert_eq!(text(doc, &["metadata", "namespace"]), "agents");
        let owner = at(doc, &["metadata", "ownerReferences", "0"]);
        assert_eq!(text(owner, &["apiVersion"]), "loom.beyond10x.dev/v1alpha1");
        assert_eq!(text(owner, &["kind"]), "LoomAgent");
        assert_eq!(text(owner, &["name"]), "platform-operator");
        assert_eq!(
            text(owner, &["uid"]),
            "00000000-0000-4000-8000-000000000001"
        );
        assert_eq!(owner["controller"], Value::Bool(true));
        assert_eq!(owner["blockOwnerDeletion"], Value::Bool(true));
        assert_eq!(
            text(doc, &["metadata", "labels", "app.kubernetes.io/managed-by"]),
            "b10x-loom-operator"
        );
    }

    let [account, map, deployment, policy, service] = &docs[..] else {
        panic!("five children");
    };
    assert_eq!(text(account, &["metadata", "name"]), "platform-operator");
    assert_eq!(account["automountServiceAccountToken"], Value::Bool(false));

    assert_eq!(text(map, &["metadata", "name"]), "platform-operator-spec");
    let data = at(map, &["data"]).as_mapping().expect("data");
    let keys: Vec<&str> = data
        .keys()
        .map(|key| key.as_str().expect("a key"))
        .collect();
    assert_eq!(keys, ["agent.yaml", "connectors.toml", "routes.yaml"]);
    let agent: Value =
        serde_yaml_ng::from_str(text(map, &["data", "agent.yaml"])).expect("agent.yaml is YAML");
    assert_eq!(text(&agent, &["agent", "id"]), "platform-operator");
    let routes: Value =
        serde_yaml_ng::from_str(text(map, &["data", "routes.yaml"])).expect("routes.yaml is YAML");
    assert_eq!(text(&routes, &["format"]), "llm.routes/1");
    assert_eq!(
        text(&routes, &["endpoints", "model-route", "base_url"]),
        "http://model-route.agents.svc:8080/v1"
    );
    assert_eq!(
        text(&routes, &["endpoints", "model-route", "auth", "kind"]),
        "anonymous"
    );
    assert_eq!(
        text(&routes, &["routes", "small"]),
        "model-route/small-model"
    );
    assert_eq!(text(map, &["data", "connectors.toml"]), "");

    let spec = at(deployment, &["spec"]);
    assert_eq!(spec["replicas"], Value::from(1));
    assert_eq!(text(spec, &["strategy", "type"]), "Recreate");
    let pod = at(spec, &["template", "spec"]);
    assert_eq!(text(pod, &["serviceAccountName"]), "platform-operator");
    assert_eq!(pod["automountServiceAccountToken"], Value::Bool(false));
    let check = at(pod, &["initContainers", "0"]);
    assert_eq!(
        strings(check, &["command"]),
        [
            "b10x-loom",
            "agent",
            "check",
            "--spec",
            "/etc/agent/agent.yaml"
        ]
    );
    let main = at(pod, &["containers", "0"]);
    assert_eq!(text(main, &["name"]), "loom-agent");
    assert_eq!(text(main, &["image"]), "registry.example/loom-agent:0.40.0");
    assert_eq!(
        strings(main, &["command"]),
        [
            "b10x-loom",
            "agent",
            "serve",
            "--spec",
            "/etc/agent/agent.yaml",
            "--state",
            "/var/lib/agent"
        ]
    );
    assert_eq!(
        text(main, &["envFrom", "0", "secretRef", "name"]),
        "platform-operator-secrets"
    );
    assert_eq!(
        text(main, &["volumeMounts", "0", "mountPath"]),
        "/etc/agent"
    );
    assert_eq!(
        at(main, &["volumeMounts", "0", "readOnly"]),
        &Value::Bool(true)
    );
    assert_eq!(
        text(main, &["volumeMounts", "1", "mountPath"]),
        "/var/lib/agent"
    );
    assert_eq!(text(main, &["ports", "0", "name"]), "a2a");
    assert_eq!(
        at(main, &["ports", "0", "containerPort"]),
        &Value::from(8080)
    );
    assert_eq!(text(main, &["readinessProbe", "httpGet", "path"]), "/ready");
    assert_eq!(text(main, &["livenessProbe", "httpGet", "path"]), "/live");
    assert_eq!(text(main, &["resources", "limits", "cpu"]), "250m");
    assert_eq!(text(main, &["resources", "limits", "memory"]), "256Mi");
    for container in [check, main] {
        let security = at(container, &["securityContext"]);
        assert_eq!(security["runAsNonRoot"], Value::Bool(true));
        assert_eq!(security["runAsUser"], Value::from(10001));
        assert_eq!(security["readOnlyRootFilesystem"], Value::Bool(true));
        assert_eq!(security["allowPrivilegeEscalation"], Value::Bool(false));
        assert_eq!(strings(security, &["capabilities", "drop"]), ["ALL"]);
    }
    assert_eq!(
        text(pod, &["volumes", "0", "configMap", "name"]),
        "platform-operator-spec"
    );
    assert!(at(pod, &["volumes", "1", "emptyDir"]).is_mapping());

    assert_eq!(
        strings(policy, &["spec", "policyTypes"]),
        ["Ingress", "Egress"]
    );
    let egress = at(policy, &["spec", "egress"])
        .as_sequence()
        .expect("egress rules");
    assert_eq!(egress.len(), 6, "DNS, four entries and the model route");
    assert_eq!(at(&egress[0], &["ports", "0", "port"]), &Value::from(53));
    assert!(
        at(&egress[1], &["to"]).is_null(),
        "a host entry by port only"
    );
    assert_eq!(at(&egress[1], &["ports", "0", "port"]), &Value::from(443));
    assert_eq!(
        text(
            &egress[3],
            &[
                "to",
                "0",
                "namespaceSelector",
                "matchLabels",
                "kubernetes.io/metadata.name"
            ]
        ),
        "monitoring"
    );
    assert_eq!(
        text(
            &egress[3],
            &["to", "0", "podSelector", "matchLabels", "app"]
        ),
        "logs"
    );
    assert_eq!(at(&egress[5], &["ports", "0", "port"]), &Value::from(8080));
    assert_eq!(
        text(
            policy,
            &[
                "spec",
                "ingress",
                "0",
                "from",
                "0",
                "namespaceSelector",
                "matchLabels",
                "kubernetes.io/metadata.name"
            ]
        ),
        "operator-clients"
    );

    assert_eq!(text(service, &["spec", "type"]), "ClusterIP");
    assert_eq!(
        at(service, &["spec", "ports", "0", "port"]),
        &Value::from(8080)
    );
    assert_eq!(text(service, &["spec", "ports", "0", "targetPort"]), "a2a");
}

#[test]
fn a_resource_without_a2a_renders_no_service_and_no_probes() {
    let resource = wrapped(&without_a2a(), "  routes: {}\n");
    let rendered = render_document(&resource, &config()).expect("rendered");
    let docs = documents(&rendered);
    let kinds: Vec<&str> = docs.iter().map(|doc| text(doc, &["kind"])).collect();
    assert_eq!(
        kinds,
        ["ServiceAccount", "ConfigMap", "Deployment", "NetworkPolicy"]
    );
    let main = at(&docs[2], &["spec", "template", "spec", "containers", "0"]);
    assert!(main["ports"].is_null());
    assert!(main["readinessProbe"].is_null());
    assert!(main["livenessProbe"].is_null());
    assert!(main["envFrom"].is_null(), "no secretRef, no envFrom");
}

#[test]
fn routes_given_are_written_as_given_and_open_no_model_route() {
    let resource = wrapped(&example(), GIVEN_ROUTES);
    let rendered = render_document(&resource, &config()).expect("rendered");
    let docs = documents(&rendered);
    let routes: Value = serde_yaml_ng::from_str(text(&docs[1], &["data", "routes.yaml"]))
        .expect("routes.yaml is YAML");
    assert_eq!(text(&routes, &["routes", "small"]), "lab/small");
    assert!(at(&routes, &["endpoints", "model-route"]).is_null());
    let egress = at(&docs[3], &["spec", "egress"])
        .as_sequence()
        .expect("egress rules");
    assert_eq!(egress.len(), 5, "DNS and four entries");
}

#[test]
fn a_secret_literal_in_the_agent_is_refused_naming_the_member_not_the_value() {
    let agent = example().replace(
        "http://logs.monitoring.svc:3100",
        "http://ops:not-a-real-password@logs.monitoring.svc:3100",
    );
    let message = refusal(&wrapped(&agent, "  routes: {}\n"), &config());
    assert!(
        message.starts_with("refused: SecretLiteral at spec.agent.bindings[1].base_url: "),
        "{message}"
    );
    assert!(!message.contains("not-a-real-password"), "{message}");
}

#[test]
fn a_secret_literal_in_the_routes_is_refused_naming_the_member_not_the_value() {
    let message = refusal(&wrapped(&example(), ROUTES_WITH_TOKEN), &config());
    assert!(
        message.starts_with("refused: SecretLiteral at spec.routes.endpoints.lab.auth.secret: "),
        "{message}"
    );
    assert!(!message.contains("xoxb-000-fake"), "{message}");
}

#[test]
fn what_the_controller_configuration_does_not_admit_is_refused_naming_the_member() {
    let mut no_route = config();
    no_route.model_route = None;
    let message = refusal(&wrapped(&example(), "  routes: {}\n"), &no_route);
    assert!(
        message.starts_with("refused: NoModelRoute at spec.routes: "),
        "{message}"
    );

    let message = refusal(
        &wrapped(
            &example(),
            "  routes: {}\n  secretRef: {name: other-secrets}\n",
        ),
        &config(),
    );
    assert!(
        message.starts_with("refused: SecretNotAllowed at spec.secretRef.name: "),
        "{message}"
    );
    assert!(!message.contains("other-secrets"), "{message}");

    let agent = example().replace("namespace: agents", "namespace: elsewhere");
    let message = refusal(&wrapped(&agent, "  routes: {}\n"), &config());
    assert!(
        message
            .starts_with("refused: NamespaceMismatch at spec.agent.deploy.kubernetes.namespace: "),
        "{message}"
    );
    assert!(!message.contains("elsewhere"), "{message}");

    let agent = example().replace("host: models.example", "host: unlisted.example");
    let message = refusal(&wrapped(&agent, "  routes: {}\n"), &config());
    assert!(
        message.starts_with(
            "refused: EgressHostNotAllowed at spec.agent.deploy.kubernetes.egress[1].host: "
        ),
        "{message}"
    );
    assert!(!message.contains("unlisted.example"), "{message}");
}

#[test]
fn a_document_that_is_not_a_loom_agent_is_not_rendered() {
    match render_document(&example(), &config()) {
        Err(RenderError::NotALoomAgent(why)) => assert!(!why.is_empty()),
        other => panic!("expected NotALoomAgent, got {other:?}"),
    }
}

/// The members a takt delivery adds after `spec.agent` (design § 9a, slice C): its own image,
/// the image's entrypoint with arguments, a workspace on a claim and the Secret as files too.
const TAKT: &str = "  routes: {}
  image: {repository: registry.example/takt, tag: '0.12.0'}
  command: {args: [run, --repo, /workspace/takt, --iterations, '1']}
  workspace: {mountPath: /workspace, claimName: takt-workspace}
  secretRef: {name: platform-operator-secrets, mountPath: /var/run/secrets/takt}
";

/// The example without `expose`, naming a protocol only the image's program knows: the serve
/// check against the plugin catalog would refuse it.
fn takt_agent() -> String {
    let agent = without_a2a().replace("protocol: operator-reply@1", "protocol: takt-delivery@1");
    assert!(agent.contains("takt-delivery@1"));
    agent
}

fn takt_resource() -> String {
    wrapped(&takt_agent(), TAKT)
}

#[test]
fn a_command_resource_renders_as_its_golden_file() {
    let rendered = render_document(&takt_resource(), &config()).expect("rendered");
    let golden = root().join("crates/loom-operator/tests/golden/takt-delivery.yaml");
    if std::env::var_os("LOOM_OPERATOR_BLESS").is_some() {
        fs::write(&golden, &rendered).expect("the golden file is written");
    }
    let expected = fs::read_to_string(&golden).expect("the golden file");
    assert_eq!(
        rendered,
        expected,
        "render differs from {}",
        golden.display()
    );
}

#[test]
fn a_command_resource_runs_the_image_program_over_config_with_its_volumes() {
    let rendered = render_document(&takt_resource(), &config()).expect("rendered");
    let docs = documents(&rendered);
    let config_map = &docs[1];
    let deployment = &docs[2];
    let pod = at(deployment, &["spec", "template", "spec"]);
    assert!(
        at(pod, &["initContainers"]).is_null(),
        "the image holds no b10x-loom to check with: {pod:?}"
    );
    let container = at(pod, &["containers", "0"]);
    assert_eq!(text(container, &["image"]), "registry.example/takt:0.12.0");
    assert!(
        at(container, &["command"]).is_null(),
        "an absent entrypoint is the image's own"
    );
    assert_eq!(
        strings(container, &["args"]),
        ["run", "--repo", "/workspace/takt", "--iterations", "1"]
    );
    assert_eq!(text(container, &["env", "0", "name"]), "HOME");
    assert_eq!(text(container, &["env", "0", "value"]), "/var/lib/agent");
    assert_eq!(
        text(container, &["envFrom", "0", "secretRef", "name"]),
        "platform-operator-secrets"
    );
    let mounts: Vec<(String, String, bool)> = at(container, &["volumeMounts"])
        .as_sequence()
        .expect("mounts")
        .iter()
        .map(|mount| {
            (
                text(mount, &["name"]).to_owned(),
                text(mount, &["mountPath"]).to_owned(),
                at(mount, &["readOnly"]).as_bool().unwrap_or(false),
            )
        })
        .collect();
    assert_eq!(
        mounts,
        [
            ("spec".to_owned(), "/config".to_owned(), true),
            ("state".to_owned(), "/var/lib/agent".to_owned(), false),
            ("tmp".to_owned(), "/tmp".to_owned(), false),
            ("workspace".to_owned(), "/workspace".to_owned(), false),
            (
                "credentials".to_owned(),
                "/var/run/secrets/takt".to_owned(),
                true
            ),
        ]
    );
    assert_eq!(
        text(pod, &["volumes", "3", "persistentVolumeClaim", "claimName"]),
        "takt-workspace"
    );
    assert_eq!(
        text(pod, &["volumes", "4", "secret", "secretName"]),
        "platform-operator-secrets"
    );
    assert_eq!(
        at(pod, &["volumes", "4", "secret", "defaultMode"]).as_i64(),
        Some(0o440)
    );
    assert_eq!(
        at(pod, &["securityContext", "fsGroup"]).as_i64(),
        Some(10001)
    );
    assert_eq!(
        at(container, &["securityContext", "readOnlyRootFilesystem"]).as_bool(),
        Some(true)
    );
    assert_eq!(
        serde_yaml_ng::from_str::<Value>(text(config_map, &["data", "agent.yaml"]))
            .expect("agent.yaml is YAML"),
        serde_yaml_ng::from_str::<Value>(&takt_agent()).expect("the agent document"),
        "/config/agent.yaml is the spec.agent document, its keys in the order the API server keeps"
    );
}

#[test]
fn a_workspace_without_a_claim_is_an_empty_dir_on_either_variant() {
    let serve = wrapped(
        &example(),
        "  routes: {}\n  workspace: {mountPath: /workspace}\n",
    );
    let docs = documents(&render_document(&serve, &config()).expect("rendered"));
    let pod = at(&docs[2], &["spec", "template", "spec"]);
    assert!(at(pod, &["volumes", "2", "emptyDir"]).is_mapping());
    assert!(at(pod, &["volumes", "2", "persistentVolumeClaim"]).is_null());
    assert_eq!(text(pod, &["initContainers", "0", "name"]), "check");
    assert_eq!(
        text(pod, &["containers", "0", "volumeMounts", "0", "mountPath"]),
        "/etc/agent",
        "the serve variant keeps its spec directory"
    );
    assert_eq!(
        at(pod, &["securityContext", "fsGroup"]).as_i64(),
        Some(10001)
    );
}

#[test]
fn a_command_resource_without_routes_or_a_model_route_gets_empty_routes() {
    let mut no_route = config();
    no_route.model_route = None;
    let rendered = render_document(&takt_resource(), &no_route).expect("rendered");
    let docs = documents(&rendered);
    assert_eq!(text(&docs[1], &["data", "routes.yaml"]), "");
}

#[test]
fn what_a_command_resource_may_not_mount_or_carry_is_refused_naming_the_member() {
    let cases = [
        (
            TAKT.replace("mountPath: /workspace,", "mountPath: workspace,"),
            "refused: MountPathInvalid at spec.workspace.mountPath: ",
        ),
        (
            TAKT.replace("mountPath: /workspace,", "mountPath: /tmp/w,"),
            "refused: MountPathInvalid at spec.workspace.mountPath: ",
        ),
        (
            TAKT.replace("mountPath: /workspace,", "mountPath: /,"),
            "refused: MountPathInvalid at spec.workspace.mountPath: ",
        ),
        (
            TAKT.replace("mountPath: /workspace,", "mountPath: /work/../config,"),
            "refused: MountPathInvalid at spec.workspace.mountPath: ",
        ),
        (
            TAKT.replace(
                "mountPath: /var/run/secrets/takt",
                "mountPath: /workspace/keys",
            ),
            "refused: MountPathInvalid at spec.secretRef.mountPath: ",
        ),
        (
            TAKT.replace("mountPath: /var/run/secrets/takt", "mountPath: /var"),
            "refused: MountPathInvalid at spec.secretRef.mountPath: ",
        ),
        (
            TAKT.replace("claimName: takt-workspace", "claimName: other-claim"),
            "refused: ClaimNotAllowed at spec.workspace.claimName: ",
        ),
        (
            TAKT.replace("'1']", "'1', --token, xoxb-000-fake]"),
            "refused: SecretLiteral at spec.command.args[6]",
        ),
        (
            TAKT.replace("'1']", "'1', --token=xoxb-000-fake]"),
            "refused: SecretLiteral at spec.command.args[5]",
        ),
        (
            TAKT.replace(
                "command: {args:",
                "command: {entrypoint: ['https://u:not-a-real-password@forge.example'], args:",
            ),
            "refused: SecretLiteral at spec.command",
        ),
    ];
    for (rest, expected) in cases {
        let message = refusal(&wrapped(&takt_agent(), &rest), &config());
        assert!(message.starts_with(expected), "{expected} <- {message}");
        for value in ["xoxb-000-fake", "not-a-real-password", "other-claim"] {
            assert!(!message.contains(value), "{message}");
        }
    }
    let agent = takt_agent().replace(
        "http://logs.monitoring.svc:3100",
        "http://ops:not-a-real-password@logs.monitoring.svc:3100",
    );
    let message = refusal(&wrapped(&agent, TAKT), &config());
    assert!(
        message.starts_with("refused: SecretLiteral at spec.agent.bindings[1].base_url"),
        "a Command spec is still walked for a credential: {message}"
    );
}

#[test]
fn the_serve_variant_still_refuses_a_protocol_outside_the_catalog() {
    let message = refusal(&wrapped(&takt_agent(), "  routes: {}\n"), &config());
    assert!(
        message.starts_with("refused: UnknownProtocol at spec.agent."),
        "{message}"
    );
}
