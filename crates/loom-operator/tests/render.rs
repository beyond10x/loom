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
use b10x_loom_operator::render::{CONTROLLER_ENV, RenderError, render_document};
use serde_yaml_ng::Value;

/// The controller configuration the golden file is rendered with.
const CONFIG: &str = "namespace: agents
defaultImage: {repository: registry.example/loom-agent, tag: '0.40.0'}
defaultResources: {cpu: 100m, memory: 128Mi}
modelRoute: {url: 'http://model-route.agents.svc:8080/v1', protocol: messages, model: small-model}
allowedSecretNames: [platform-operator-secrets, takt-forge]
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
/// the image's own entrypoint (`tini -- takt run --repo /workspace --config /config/agent.yaml`
/// in takt 0.13.1) with only the arguments that follow it, since Kubernetes appends `args` to the
/// entrypoint, a workspace on a claim, one Secret as the environment and another as files.
const TAKT: &str = "  routes: {}
  image: {repository: registry.example/takt, tag: '0.12.0'}
  command: {args: [--iterations, '1']}
  workspace: {mountPath: /workspace, claimName: takt-workspace}
  secretRef: {name: platform-operator-secrets}
  secretFiles: {name: takt-forge, mountPath: /var/run/secrets/takt}
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

/// `story:takt-delivery-fixture-args`: the takt 0.13.1 image's entrypoint already runs `takt run
/// --repo /workspace`, and Kubernetes appends `args` to it, so the fixture passes only what
/// follows; a `run` or a second `--repo` in `args` would run `run` twice.
#[test]
fn the_takt_delivery_fixture_runs_takt_once() {
    let rendered = render_document(&takt_resource(), &config()).expect("rendered");
    let pod = pod_of(&rendered);
    let container = &pod["containers"][0];
    assert!(
        container["command"].is_null(),
        "the image's own entrypoint runs"
    );
    let args = strings(container, &["args"]);
    assert_eq!(args, ["--iterations", "1"]);
    assert!(!args.iter().any(|arg| arg == "run" || arg == "--repo"));

    let golden =
        fs::read_to_string(root().join("crates/loom-operator/tests/golden/takt-delivery.yaml"))
            .expect("the golden file");
    let golden_pod = pod_of(&golden);
    assert_eq!(
        strings(&golden_pod["containers"][0], &["args"]),
        ["--iterations", "1"],
        "the golden file a resource is copied from holds the same"
    );
}

/// `story:command-variant-runs-once`: a `Command` resource runs its program once, as a Job that
/// retries nothing and is never removed by a TTL, so its end stays readable; its pod template is
/// the one the `Command` Deployment had, under the same NetworkPolicy. An `AgentServe` resource
/// keeps its Deployment.
#[test]
fn a_command_resource_renders_a_job_not_a_deployment() {
    let docs = documents(&render_document(&takt_resource(), &config()).expect("rendered"));
    let kinds: Vec<&str> = docs.iter().map(|doc| text(doc, &["kind"])).collect();
    assert_eq!(
        kinds,
        ["ServiceAccount", "ConfigMap", "Job", "NetworkPolicy"]
    );
    let job = &docs[2];
    assert_eq!(text(job, &["apiVersion"]), "batch/v1");
    assert_eq!(text(job, &["metadata", "name"]), "platform-operator");
    assert_eq!(
        text(job, &["metadata", "ownerReferences", "0", "kind"]),
        "LoomAgent"
    );
    assert_eq!(at(job, &["spec", "backoffLimit"]).as_i64(), Some(0));
    assert!(at(job, &["spec", "ttlSecondsAfterFinished"]).is_null());
    for member in ["replicas", "strategy", "selector"] {
        assert!(
            at(job, &["spec", member]).is_null(),
            "a Job has no {member}"
        );
    }
    let template = at(job, &["spec", "template"]);
    assert_eq!(
        text(
            template,
            &["metadata", "annotations", "loom.beyond10x.dev/generation"]
        ),
        "1"
    );
    assert_eq!(
        text(
            template,
            &["metadata", "labels", "app.kubernetes.io/instance"]
        ),
        "platform-operator"
    );
    let pod = at(template, &["spec"]);
    assert_eq!(text(pod, &["restartPolicy"]), "Never");
    assert_eq!(
        text(pod, &["initContainers", "0", "name"]),
        "stage-secret-files"
    );
    let container = at(pod, &["containers", "0"]);
    assert_eq!(text(container, &["image"]), "registry.example/takt:0.12.0");
    assert_eq!(
        text(container, &["envFrom", "0", "secretRef", "name"]),
        "platform-operator-secrets"
    );
    assert_eq!(
        text(pod, &["volumes", "3", "persistentVolumeClaim", "claimName"]),
        "takt-workspace"
    );
    assert_eq!(
        text(pod, &["volumes", "4", "secret", "secretName"]),
        "takt-forge"
    );
    assert_eq!(
        text(pod, &["securityContext", "seccompProfile", "type"]),
        "RuntimeDefault"
    );
    assert_eq!(
        text(
            &docs[3],
            &[
                "spec",
                "podSelector",
                "matchLabels",
                "app.kubernetes.io/instance"
            ]
        ),
        "platform-operator",
        "the NetworkPolicy selects the Job's pod by the template's labels"
    );

    let serve = documents(&render_document(&default_resource(), &config()).expect("rendered"));
    let kinds: Vec<&str> = serve.iter().map(|doc| text(doc, &["kind"])).collect();
    assert_eq!(
        kinds,
        [
            "ServiceAccount",
            "ConfigMap",
            "Deployment",
            "NetworkPolicy",
            "Service"
        ],
        "an AgentServe resource still renders a Deployment"
    );
    assert!(at(&serve[2], &["spec", "template", "spec", "restartPolicy"]).is_null());
}

#[test]
fn a_command_resource_runs_the_image_program_over_config_with_its_volumes() {
    let rendered = render_document(&takt_resource(), &config()).expect("rendered");
    let docs = documents(&rendered);
    let config_map = &docs[1];
    let deployment = &docs[2];
    let pod = at(deployment, &["spec", "template", "spec"]);
    let init = at(pod, &["initContainers"]).as_sequence().expect("init");
    assert_eq!(
        init.len(),
        1,
        "only the staging, no check: the image holds no b10x-loom to check with: {pod:?}"
    );
    let stage = &init[0];
    assert_eq!(text(stage, &["name"]), "stage-secret-files");
    assert_eq!(
        text(stage, &["image"]),
        "registry.example/loom-agent:0.40.0",
        "the controller's default image, which carries b10x-loom whatever spec.image is"
    );
    assert_eq!(
        strings(stage, &["command"]),
        [
            "b10x-loom",
            "agent",
            "stage-files",
            "--from",
            "/var/run/loom/secret-files-source",
            "--to",
            "/var/run/loom/secret-files/files"
        ]
    );
    assert_eq!(
        at(stage, &["securityContext", "readOnlyRootFilesystem"]).as_bool(),
        Some(true)
    );
    assert_eq!(
        (
            text(stage, &["volumeMounts", "0", "name"]),
            text(stage, &["volumeMounts", "0", "mountPath"]),
            at(stage, &["volumeMounts", "0", "readOnly"]).as_bool()
        ),
        (
            "secret-files-source",
            "/var/run/loom/secret-files-source",
            Some(true)
        )
    );
    assert_eq!(
        (
            text(stage, &["volumeMounts", "1", "name"]),
            text(stage, &["volumeMounts", "1", "mountPath"])
        ),
        ("secret-files", "/var/run/loom/secret-files")
    );
    let container = at(pod, &["containers", "0"]);
    assert_eq!(text(container, &["image"]), "registry.example/takt:0.12.0");
    assert!(
        at(container, &["command"]).is_null(),
        "an absent entrypoint is the image's own"
    );
    assert_eq!(strings(container, &["args"]), ["--iterations", "1"]);
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
                "secret-files".to_owned(),
                "/var/run/secrets/takt".to_owned(),
                true
            ),
        ]
    );
    assert_eq!(
        text(container, &["volumeMounts", "4", "subPath"]),
        "files",
        "the directory the staging made, owned by the pod's user, mode 0700"
    );
    assert_eq!(
        text(pod, &["volumes", "3", "persistentVolumeClaim", "claimName"]),
        "takt-workspace"
    );
    assert_eq!(text(pod, &["volumes", "4", "name"]), "secret-files-source");
    assert_eq!(
        text(pod, &["volumes", "4", "secret", "secretName"]),
        "takt-forge"
    );
    assert_eq!(
        at(pod, &["volumes", "4", "secret", "defaultMode"]).as_i64(),
        Some(0o440)
    );
    assert_eq!(text(pod, &["volumes", "5", "name"]), "secret-files");
    assert_eq!(text(pod, &["volumes", "5", "emptyDir", "medium"]), "Memory");
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
            "refused: MountPathInvalid at spec.secretFiles.mountPath: ",
        ),
        (
            TAKT.replace("mountPath: /var/run/secrets/takt", "mountPath: /var"),
            "refused: MountPathInvalid at spec.secretFiles.mountPath: ",
        ),
        (
            TAKT.replace("claimName: takt-workspace", "claimName: other-claim"),
            "refused: ClaimNotAllowed at spec.workspace.claimName: ",
        ),
        (
            TAKT.replace("name: takt-forge", "name: other-forge"),
            "refused: SecretNotAllowed at spec.secretFiles.name: ",
        ),
        (
            TAKT.replace("'1']", "'1', --token, xoxb-000-fake]"),
            "refused: SecretLiteral at spec.command.args[3]",
        ),
        (
            TAKT.replace("'1']", "'1', --token=xoxb-000-fake]"),
            "refused: SecretLiteral at spec.command.args[2]",
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
        for value in [
            "xoxb-000-fake",
            "not-a-real-password",
            "other-claim",
            "other-forge",
        ] {
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

/// `spec.env` as YAML, each name and value single-quoted.
fn env(entries: &[(&str, &str)]) -> String {
    let quoted = |text: &str| format!("'{}'", text.replace('\'', "''"));
    let mut out = String::from("  env:\n");
    for (name, value) in entries {
        out.push_str(&format!(
            "  - {{name: {}, value: {}}}\n",
            quoted(name),
            quoted(value)
        ));
    }
    out
}

/// The serve variant's resource with `rest` after its routes and Secret.
fn serve_with(rest: &str) -> String {
    wrapped(
        &example(),
        &format!("  routes: {{}}\n  secretRef: {{name: platform-operator-secrets}}\n{rest}"),
    )
}

/// The `Command` variant's resource with `rest` after the takt members.
fn command_with(rest: &str) -> String {
    wrapped(&takt_agent(), &format!("{TAKT}{rest}"))
}

/// The pod template's spec of the rendered workload, a Deployment or a Job.
fn pod_of(rendered: &str) -> Value {
    let docs = documents(rendered);
    at(&docs[2], &["spec", "template", "spec"]).clone()
}

/// Each `name` and `value` of `container`'s `env`, in order; none when it has no `env`.
fn env_of(container: &Value) -> Vec<(String, String)> {
    container["env"]
        .as_sequence()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| {
                    (
                        text(entry, &["name"]).to_owned(),
                        text(entry, &["value"]).to_owned(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// What a trial run of the `Command` variant set through a wrapper script before `spec.env`.
const TRIAL_ENV: [(&str, &str); 4] = [
    ("CARGO_HOME", "/workspace/cargo"),
    ("CARGO_TARGET_DIR", "/workspace/target"),
    ("CARGO_NET_OFFLINE", "true"),
    ("B10X_GATES_POLICY", "/var/run/secrets/takt/policy.json"),
];

fn owned(entries: &[(&str, &str)]) -> Vec<(String, String)> {
    entries
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

#[test]
fn spec_env_values_reach_the_container() {
    let pod =
        pod_of(&render_document(&command_with(&env(&TRIAL_ENV)), &config()).expect("rendered"));
    let container = &pod["containers"][0];
    let mut expected = owned(&[("HOME", "/var/lib/agent")]);
    expected.extend(owned(&TRIAL_ENV));
    assert_eq!(
        env_of(container),
        expected,
        "the controller's own first, then spec.env in declaration order"
    );
    assert_eq!(
        text(container, &["envFrom", "0", "secretRef", "name"]),
        "platform-operator-secrets",
        "beside the secretRef environment"
    );
    for init in pod["initContainers"].as_sequence().expect("init") {
        assert_eq!(env_of(init), [], "an init container gets none: {init:?}");
    }

    let reversed: Vec<(&str, &str)> = TRIAL_ENV.iter().rev().copied().collect();
    let pod = pod_of(&render_document(&serve_with(&env(&reversed)), &config()).expect("rendered"));
    let container = &pod["containers"][0];
    assert_eq!(env_of(container), owned(&reversed), "the serve variant too");
    assert_eq!(
        text(container, &["envFrom", "0", "secretRef", "name"]),
        "platform-operator-secrets"
    );
    for init in pod["initContainers"].as_sequence().expect("init") {
        assert_eq!(env_of(init), [], "{init:?}");
    }

    let pod = pod_of(&render_document(&serve_with("  env: []\n"), &config()).expect("rendered"));
    assert!(
        pod["containers"][0]["env"].is_null(),
        "an empty spec.env renders no env"
    );
}

#[test]
fn a_credential_in_spec_env_is_refused_without_repeating_it() {
    let cases = [
        (
            env(&[("CARGO_HOME", "/w"), ("FORGE_TOKEN", "xoxb-000-fake")]),
            "refused: SecretLiteral at spec.env[1].value: a value with the prefix `xoxb-`",
        ),
        (
            env(&[("FLAGS", "--verbose --token ghp_000fake")]),
            "refused: SecretLiteral at spec.env[0].value: a value with the prefix `ghp_`",
        ),
        (
            env(&[("AUTH", "Authorization:Bearer 000fake")]),
            "refused: SecretLiteral at spec.env[0].value: a value with the prefix `Bearer `",
        ),
        (
            env(&[("FORGE", "https://u:not-a-real-password@forge.example")]),
            "refused: SecretLiteral at spec.env[0].value: ",
        ),
        (
            env(&[("CARGO_HOME", "/w"), ("ghp_000fake", "x")]),
            "refused: SecretLiteral at spec.env[1].name: a value with the prefix `ghp_`",
        ),
    ];
    for (rest, expected) in &cases {
        for resource in [command_with(rest), serve_with(rest)] {
            let message = refusal(&resource, &config());
            assert!(message.starts_with(expected), "{expected} <- {message}");
            for value in ["000-fake", "000fake", "not-a-real-password"] {
                assert!(!message.contains(value), "{message}");
            }
        }
    }

    let long = "a ".repeat(1024 * 1024 + 64);
    let message = refusal(&command_with(&env(&[("LONG", &long)])), &config());
    assert_eq!(
        message,
        "refused: TooLarge at spec.env: it is longer than 2097152 bytes as YAML, more than the \
         credential walk reads"
    );
    let braces = "{".repeat(4097);
    let message = refusal(&command_with(&env(&[("BRACES", &braces)])), &config());
    assert_eq!(
        message,
        "refused: TooLarge at spec.env: it holds more than 4096 `[` or `{` bytes, more than the \
         credential walk reads"
    );
}

/// [`CONTROLLER_ENV`] is every name the controller sets on any container of either variant, so a
/// name the renderer starts setting without adding it there fails here.
#[test]
fn a_reserved_variable_name_is_refused() {
    let mut set = Vec::new();
    for resource in [command_with(""), serve_with("")] {
        let pod = pod_of(&render_document(&resource, &config()).expect("rendered"));
        for list in ["containers", "initContainers"] {
            for container in pod[list].as_sequence().into_iter().flatten() {
                set.extend(env_of(container).into_iter().map(|(name, _)| name));
            }
        }
    }
    set.sort_unstable();
    set.dedup();
    let mut reserved: Vec<String> = CONTROLLER_ENV
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    reserved.sort_unstable();
    assert_eq!(set, reserved, "CONTROLLER_ENV names what render sets");

    for name in CONTROLLER_ENV {
        let rest = env(&[("CARGO_HOME", "/w"), (name, "/workspace")]);
        for resource in [command_with(&rest), serve_with(&rest)] {
            assert_eq!(
                refusal(&resource, &config()),
                "refused: EnvNameConflict at spec.env[1].name: the controller sets it on the \
                 container"
            );
        }
    }
}

#[test]
fn a_name_that_is_not_a_variable_name_is_refused_without_repeating_it() {
    for name in [
        "",
        "1PATH",
        "CARGO-HOME",
        "CARGO HOME",
        "A=B",
        "A.B",
        "ÄRGER",
    ] {
        let rest = env(&[("CARGO_HOME", "/w"), (name, "x")]);
        for resource in [command_with(&rest), serve_with(&rest)] {
            assert_eq!(
                refusal(&resource, &config()),
                "refused: NotAVariableName at spec.env[1].name: it is not an environment \
                 variable name, [A-Za-z_][A-Za-z0-9_]*",
                "{name:?}"
            );
        }
    }
}

#[test]
fn a_name_given_twice_is_refused_naming_both() {
    let rest = env(&[
        ("CARGO_HOME", "/a"),
        ("CARGO_NET_OFFLINE", "true"),
        ("CARGO_HOME", "/b"),
    ]);
    for resource in [command_with(&rest), serve_with(&rest)] {
        assert_eq!(
            refusal(&resource, &config()),
            "refused: EnvNameConflict at spec.env[2].name: it repeats spec.env[0].name"
        );
    }
    let distinct = env(&[("cargo_home", "/a"), ("CARGO_HOME", "/b")]);
    render_document(&command_with(&distinct), &config()).expect("names differ by case");

    let mut many: Vec<(String, String)> = (0..40_000)
        .map(|index| (format!("V{index}"), "x".to_owned()))
        .collect();
    many.push(("V17".to_owned(), "y".to_owned()));
    let entries: Vec<(&str, &str)> = many
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    assert_eq!(
        refusal(&serve_with(&env(&entries)), &config()),
        "refused: EnvNameConflict at spec.env[40000].name: it repeats spec.env[17].name"
    );
}

/// The kubelet expands `$(NAME)` in a value from the entries before it, and in the command from
/// the whole environment, so each is walked again as the program reads it.
#[test]
fn a_credential_joined_by_variable_expansion_is_refused_without_repeating_it() {
    let joined = TAKT.replace(
        "--iterations, '1']",
        "--iterations, '1', --token, '$(PART)_000fake']",
    );
    let cases = [
        (
            serve_with(&env(&[("PART", "ghp"), ("TOKEN", "$(PART)_000fake")])),
            "refused: SecretLiteral at spec.env[1].value: a value with the prefix `ghp_`, after \
             `$(VAR)` expansion",
        ),
        (
            serve_with(&env(&[
                ("BASE", "https://u:not-a-real-password"),
                ("URL", "$(BASE)@forge.example/repo"),
            ])),
            "refused: SecretLiteral at spec.env[1].value: ",
        ),
        (
            wrapped(
                &takt_agent(),
                &format!("{joined}{}", env(&[("PART", "ghp")])),
            ),
            "refused: SecretLiteral at spec.command.args[3]: a value with the prefix `ghp_`, \
             after `$(VAR)` expansion",
        ),
        (
            command_with(&env(&[
                ("PART", "xoxb"),
                ("TOKEN", "--token $(PART)-000fake"),
            ])),
            "refused: SecretLiteral at spec.env[1].value: a value with the prefix `xoxb-`, after \
             `$(VAR)` expansion",
        ),
    ];
    for (resource, expected) in cases {
        let message = refusal(&resource, &config());
        assert!(message.starts_with(expected), "{expected} <- {message}");
        for value in ["000fake", "not-a-real-password"] {
            assert!(!message.contains(value), "{message}");
        }
    }

    for admitted in [
        env(&[("PART", "ghp"), ("TOKEN", "$$(PART)_000fake")]),
        env(&[("TOKEN", "$(PART)_000fake"), ("PART", "ghp")]),
        env(&[("TOKEN", "$(FROM_SECRET)_000fake")]),
        env(&[("CARGO_HOME", "$(HOME)/cargo")]),
    ] {
        render_document(&command_with(&admitted), &config())
            .unwrap_or_else(|error| panic!("{admitted}: {error}"));
    }
}

/// A name the controller does not know (the service environment, a `secretRef` key) may supply
/// a `scheme://` at run time, so the walk reads such a reference as a scheme too: a URL it
/// completes with user information is refused, directly, through another entry or in the
/// command, even one standing just before `@` (the safe side: it may be `scheme://user`), and a
/// reference that completes nothing is admitted.
#[test]
fn an_unknown_reference_is_walked_as_a_possible_url_scheme() {
    let expected = "refused: SecretLiteral at ";
    for (resource, member) in [
        (
            serve_with(&env(&[(
                "FORGE_URL",
                "$(KUBERNETES_PORT)x:not-a-real-password@forge.example/repo",
            )])),
            "spec.env[0].value: ",
        ),
        (
            serve_with(&env(&[
                ("BASE", "$(FORGE_SCHEME)"),
                ("FORGE_URL", "$(BASE)u:not-a-real-password@forge.example"),
            ])),
            "spec.env[1].value: ",
        ),
        (
            wrapped(
                &takt_agent(),
                &TAKT.replace(
                    "--iterations, '1']",
                    "--iterations, '1', '$(FORGE_SCHEME)u:not-a-real-password@forge.example']",
                ),
            ),
            "spec.command.args[2]: ",
        ),
        (
            serve_with(&env(&[("NOTE", "$(USER_NAME)@forge.example")])),
            "spec.env[0].value: ",
        ),
    ] {
        let message = refusal(&resource, &config());
        assert!(
            message.starts_with(&format!("{expected}{member}")),
            "{member} <- {message}"
        );
        assert!(message.ends_with(", after `$(VAR)` expansion"), "{message}");
        for hidden in [
            "not-a-real-password",
            "KUBERNETES_PORT",
            "FORGE_SCHEME",
            "USER_NAME",
        ] {
            assert!(!message.contains(hidden), "{message}");
        }
    }
    for admitted in [
        env(&[("API", "$(KUBERNETES_PORT)/healthz")]),
        env(&[("PATHS", "$(HOME)/bin:$(CARGO_HOME)/bin")]),
    ] {
        render_document(&command_with(&admitted), &config())
            .unwrap_or_else(|error| panic!("{admitted}: {error}"));
    }
}

/// References that double a value at each step would expand beyond any memory; past the bound
/// the expanded text is refused unwalked.
#[test]
fn variable_references_expanding_beyond_the_bound_are_too_large() {
    let mut entries = vec![("V0".to_owned(), "a".repeat(1024))];
    for index in 1..=12 {
        entries.push((format!("V{index}"), format!("$(V{0})$(V{0})", index - 1)));
    }
    let entries: Vec<(&str, &str)> = entries
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    assert_eq!(
        refusal(&serve_with(&env(&entries)), &config()),
        "refused: TooLarge at spec.env[11].value: its `$(VAR)` references, expanded, add more \
         than 2097152 bytes, more than the credential walk reads"
    );
    render_document(&serve_with(&env(&entries[..10])), &config()).expect("within the bound");
}

/// The head of a `LoomAgent` without `spec.agent`, before its other `spec` members.
const HEAD: &str = "apiVersion: loom.beyond10x.dev/v1alpha1
kind: LoomAgent
metadata:
  name: platform-operator
  namespace: agents
  uid: 00000000-0000-4000-8000-000000000001
  generation: 1
spec:
";

/// A `Command` resource without `spec.agent`: the takt members, then `rest`.
fn command_without_agent(rest: &str) -> String {
    format!("{HEAD}{TAKT}{rest}")
}

/// `spec.configFiles` as YAML, each name and content a double-quoted scalar.
fn config_files(files: &[(&str, &str)]) -> String {
    let quoted = |text: &str| serde_json::to_string(text).expect("JSON text");
    let mut out = String::from("  configFiles:\n");
    for (name, content) in files {
        out.push_str(&format!(
            "  - {{name: {}, content: {}}}\n",
            quoted(name),
            quoted(content)
        ));
    }
    out
}

/// takt's endpoint configuration: a comment, a trailing space and no final newline kept.
const ENDPOINT_TOML: &str = "# the model route takt calls\nbase_url = \
     \"http://model-route.agents.svc:8080/v1\" \nprotocol = \"messages\"\ntoken_env = \
     \"TAKT_ENDPOINT_TOKEN\"";

/// takt's run configuration: not TOML on its last line, which the controller does not decode.
const RUN_TOML: &str = "iterations = 1\n\n[forge]\nremote = \"https://forge.example/agents/takt.git\"\n\
     \t  not = = toml  \n";

/// takt's own `--config` file, at the path the image's entrypoint names.
const TAKT_CONFIG: &str = "repo: /workspace\nreview: {rounds: 2}\n";

/// The data of the rendered ConfigMap, by key.
fn config_map_data(rendered: &str) -> Vec<(String, String)> {
    let docs = documents(rendered);
    assert_eq!(text(&docs[1], &["kind"]), "ConfigMap");
    docs[1]["data"]
        .as_mapping()
        .expect("data")
        .iter()
        .map(|(key, value)| {
            (
                key.as_str().expect("a key").to_owned(),
                value.as_str().expect("a value").to_owned(),
            )
        })
        .collect()
}

#[test]
fn config_files_are_written_verbatim_under_config() {
    let files = [
        ("endpoint.toml", ENDPOINT_TOML),
        ("run.toml", RUN_TOML),
        ("agent.yaml", TAKT_CONFIG),
    ];
    let rendered =
        render_document(&command_without_agent(&config_files(&files)), &config()).expect("ok");
    let data = config_map_data(&rendered);
    let keys: Vec<&str> = data.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "agent.yaml",
            "connectors.toml",
            "endpoint.toml",
            "routes.yaml",
            "run.toml"
        ],
        "without spec.agent, agent.yaml is the program's own file"
    );
    for (name, content) in files {
        let written = data.iter().find(|(key, _)| key == name).expect("written");
        assert_eq!(written.1, content, "{name} verbatim");
    }
    let routes: Value = serde_yaml_ng::from_str(
        &data
            .iter()
            .find(|(key, _)| key == "routes.yaml")
            .expect("r")
            .1,
    )
    .expect("YAML");
    assert_eq!(
        routes["routes"]
            .as_mapping()
            .map(serde_yaml_ng::Mapping::len),
        Some(0),
        "no spec.agent names no role, so the default routes route nothing: {routes:?}"
    );
    assert!(routes["endpoints"]["model-route"].is_mapping());

    let pod = pod_of(&rendered);
    let container = &pod["containers"][0];
    let mount = container["volumeMounts"]
        .as_sequence()
        .expect("mounts")
        .iter()
        .find(|mount| text(mount, &["mountPath"]) == "/config")
        .expect("the ConfigMap at /config");
    assert_eq!(text(mount, &["name"]), "spec");
    assert_eq!(mount["readOnly"].as_bool(), Some(true));
    assert!(
        mount["subPath"].is_null(),
        "the whole ConfigMap, each key a file"
    );
    let volume = pod["volumes"]
        .as_sequence()
        .expect("volumes")
        .iter()
        .find(|volume| text(volume, &["name"]) == "spec")
        .expect("the spec volume");
    assert_eq!(
        text(volume, &["configMap", "name"]),
        "platform-operator-spec"
    );
    assert!(
        volume["configMap"]["items"].is_null(),
        "every key is projected"
    );

    let beside = render_document(
        &command_with(&config_files(&[("endpoint.toml", ENDPOINT_TOML)])),
        &config(),
    )
    .expect("rendered");
    let data = config_map_data(&beside);
    let keys: Vec<&str> = data.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "agent.yaml",
            "connectors.toml",
            "endpoint.toml",
            "routes.yaml"
        ]
    );
    assert!(
        data[0].1.contains("format: loom.agent/1"),
        "with spec.agent, agent.yaml is spec.agent"
    );

    let none = render_document(&command_without_agent(""), &config()).expect("rendered");
    let data = config_map_data(&none);
    let keys: Vec<&str> = data.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(keys, ["connectors.toml", "routes.yaml"]);
}

#[test]
fn a_credential_in_a_config_file_is_refused_without_repeating_it() {
    let cases = [
        (
            vec![("endpoint.toml", "token = \"xoxb-000-fake\"\n")],
            "refused: SecretLiteral at spec.configFiles[0].content: ",
        ),
        (
            vec![
                ("endpoint.toml", ENDPOINT_TOML),
                (
                    "run.toml",
                    "iterations = 1\n\n# Authorization: Bearer 000fake\n",
                ),
            ],
            "refused: SecretLiteral at spec.configFiles[1].content: a value with the prefix \
             `Bearer ` (line 3)",
        ),
        (
            vec![("run.toml", "flags = \"--verbose --token=ghp_000fake\"\n")],
            "refused: SecretLiteral at spec.configFiles[0].content: ",
        ),
        (
            vec![("endpoint.toml", "token = \"\\u0067hp_000fake\"\n")],
            "refused: SecretLiteral at spec.configFiles[0].content: a value with the prefix \
             `ghp_`",
        ),
        (
            vec![("endpoint.json", "{\"token\": \"\\u0078oxb-000-fake\"}")],
            "refused: SecretLiteral at spec.configFiles[0].content: a value with the prefix \
             `xoxb-`, read as YAML",
        ),
        (
            vec![(
                "run.toml",
                "[forge]\nremote = \"https://u:not-a-real-password@forge.example/t.git\"\n",
            )],
            "refused: SecretLiteral at spec.configFiles[0].content: ",
        ),
        (
            vec![("ghp_000fake", "x")],
            "refused: SecretLiteral at spec.configFiles[0].name: a value with the prefix `ghp_`",
        ),
    ];
    for (files, expected) in &cases {
        for resource in [
            command_without_agent(&config_files(files)),
            command_with(&config_files(files)),
        ] {
            let message = refusal(&resource, &config());
            assert!(message.starts_with(expected), "{expected} <- {message}");
            for value in ["000-fake", "000fake", "not-a-real-password", "token"] {
                assert!(!message.contains(value), "{message}");
            }
        }
    }

    let braces = "{".repeat(4097);
    assert_eq!(
        refusal(
            &command_without_agent(&config_files(&[("run.toml", &braces)])),
            &config()
        ),
        "refused: TooLarge at spec.configFiles: it holds more than 4096 `[` or `{` bytes, more \
         than the credential walk reads"
    );
}

/// Kubernetes refuses a ConfigMap whose keys and values hold more than 1 MiB together; the
/// controller refuses the resource before it applies one.
#[test]
fn a_config_map_beyond_kubernetes_limit_is_too_large() {
    let bare = render_document(&command_without_agent(""), &config()).expect("rendered");
    let held: usize = config_map_data(&bare)
        .iter()
        .map(|(key, value)| key.len() + value.len())
        .sum();
    let name = "run.toml";
    let room = 1024 * 1024 - held - name.len();
    let line = "iterations = 1\n";
    let mut content = line.repeat(room / line.len());
    content.push_str(&"x".repeat(room - content.len()));
    render_document(
        &command_without_agent(&config_files(&[(name, &content)])),
        &config(),
    )
    .expect("exactly at the limit");
    content.push('x');
    assert_eq!(
        refusal(
            &command_without_agent(&config_files(&[(name, &content)])),
            &config()
        ),
        "refused: TooLarge at spec.configFiles: the ConfigMap it makes holds more than 1048576 \
         bytes, more than Kubernetes accepts"
    );
}

#[test]
fn an_agent_serve_resource_without_spec_agent_is_refused() {
    let serve = format!("{HEAD}  routes: {{}}\n  secretRef: {{name: platform-operator-secrets}}\n");
    assert_eq!(
        refusal(&serve, &config()),
        "refused: Malformed at spec.agent: `b10x-loom agent serve` needs a loom.agent/1 \
         document; only a Command resource may leave it out"
    );
    assert_eq!(
        refusal(
            &serve_with(&config_files(&[("endpoint.toml", ENDPOINT_TOML)])),
            &config()
        ),
        "refused: Malformed at spec.configFiles: `b10x-loom agent serve` reads no configuration \
         file; only a Command resource may give them"
    );
    render_document(&serve_with("  configFiles: []\n"), &config()).expect("an empty list");
}

#[test]
fn a_config_file_name_that_is_not_a_plain_file_name_is_refused_without_repeating_it() {
    let longest = "n".repeat(253);
    for name in [
        "",
        ".",
        "..",
        "hidden-name/x",
        "../hidden-name",
        "..hidden-name",
        "hidden name",
        "hidden-name\t",
        "hidden-n\u{e4}me",
        "hidden-name:x",
        &format!("{longest}n"),
    ] {
        let message = refusal(
            &command_without_agent(&config_files(&[("run.toml", RUN_TOML), (name, "x")])),
            &config(),
        );
        assert_eq!(
            message,
            "refused: ConfigFileInvalid at spec.configFiles[1].name: it is not a plain file \
             name, [A-Za-z0-9._-]+ other than `.` and not starting with `..`, of at most 253 bytes",
            "{name:?}"
        );
        assert!(!message.contains("hidden"), "{message}");
    }
    render_document(
        &command_without_agent(&config_files(&[
            (&longest, "x"),
            ("a-b_c.d", "y"),
            (".hidden", "z"),
        ])),
        &config(),
    )
    .expect("plain names");
}

#[test]
fn a_config_file_name_given_twice_or_held_by_the_controller_is_refused() {
    let conflict = |files: &[(&str, &str)], with_agent: bool| {
        let rest = config_files(files);
        let resource = if with_agent {
            command_with(&rest)
        } else {
            command_without_agent(&rest)
        };
        refusal(&resource, &config())
    };
    assert_eq!(
        conflict(
            &[("run.toml", "a"), ("endpoint.toml", "b"), ("run.toml", "c")],
            false
        ),
        "refused: ConfigFileConflict at spec.configFiles[2].name: it repeats \
         spec.configFiles[0].name"
    );
    for (name, with_agent) in [
        ("routes.yaml", false),
        ("connectors.toml", false),
        ("routes.yaml", true),
        ("agent.yaml", true),
    ] {
        assert_eq!(
            conflict(&[(name, "x")], with_agent),
            "refused: ConfigFileConflict at spec.configFiles[0].name: the controller writes a \
             file of that name under /config itself",
            "{name} with spec.agent {with_agent}"
        );
    }
}

/// A YAML escape inside a value of several words: the stream walk reads each value whole, so
/// each decoded string is also walked word by word, as a TOML string is.
#[test]
fn an_escaped_token_inside_a_yaml_value_of_several_words_is_refused() {
    for content in [
        "flags: \"--verbose --token \\x67hp_000fake\"\n",
        "kind: run\n---\nargs: [\"-H\", \"Authorization: \\x42earer 000fake\"]\n",
    ] {
        let message = refusal(
            &command_without_agent(&config_files(&[("run.yaml", content)])),
            &config(),
        );
        assert!(
            message.starts_with("refused: SecretLiteral at spec.configFiles[0].content: "),
            "{content:?}: {message}"
        );
        assert!(message.ends_with(", read as YAML"), "{message}");
        assert!(!message.contains("000fake"), "{message}");
    }
}

/// Each `(name, mountPath, subPath)` of `container`'s volume mounts, in order.
fn mounts_of(container: &Value) -> Vec<(String, String, Option<String>)> {
    container["volumeMounts"]
        .as_sequence()
        .expect("mounts")
        .iter()
        .map(|mount| {
            (
                text(mount, &["name"]).to_owned(),
                text(mount, &["mountPath"]).to_owned(),
                mount["subPath"].as_str().map(ToOwned::to_owned),
            )
        })
        .collect()
}

/// `story:writable-dirs-mode-0700`: Kubernetes makes an emptyDir's root mode 0777 (setgid with
/// `fsGroup`) and a non-root container cannot change it, so a `Command` pod's `/tmp` and `HOME`
/// are directories an init container makes in their emptyDirs, owned by the pod's user with mode
/// 0700, mounted by `subPath`. The mode itself is held by `crates/loom-cli/tests/prepare_dirs.rs`.
#[test]
fn the_writable_dirs_are_0700_and_owned_by_the_pod_user() {
    let bare = wrapped(
        &takt_agent(),
        "  routes: {}\n  image: {repository: registry.example/takt, tag: '0.12.0'}\n  command: \
         {args: [--iterations, '1']}\n",
    );
    for resource in [takt_resource(), bare] {
        let pod = pod_of(&render_document(&resource, &config()).expect("rendered"));
        let init = pod["initContainers"]
            .as_sequence()
            .expect("init containers");
        let prepare = init
            .iter()
            .find(|container| text(container, &["name"]) == "prepare-writable-dirs")
            .unwrap_or_else(|| panic!("an init container prepares the directories: {pod:?}"));
        assert_eq!(
            text(prepare, &["image"]),
            "registry.example/loom-agent:0.40.0",
            "the controller's default image, which carries b10x-loom whatever spec.image is"
        );
        assert_eq!(
            strings(prepare, &["command"]),
            [
                "b10x-loom",
                "agent",
                "prepare-dirs",
                "--dir",
                "/var/run/loom/tmp/tmp",
                "--dir",
                "/var/run/loom/state/home"
            ]
        );
        assert_eq!(
            at(prepare, &["securityContext", "runAsUser"]).as_i64(),
            Some(10001),
            "the directories it makes are the pod user's"
        );
        assert_eq!(
            at(prepare, &["securityContext", "readOnlyRootFilesystem"]).as_bool(),
            Some(true)
        );
        assert!(prepare["env"].is_null() && prepare["envFrom"].is_null());
        assert_eq!(
            mounts_of(prepare),
            [
                ("tmp".to_owned(), "/var/run/loom/tmp".to_owned(), None),
                ("state".to_owned(), "/var/run/loom/state".to_owned(), None),
            ]
        );

        let container = &pod["containers"][0];
        let mounts = mounts_of(container);
        for (name, path, sub) in [("tmp", "/tmp", "tmp"), ("state", "/var/lib/agent", "home")] {
            assert!(
                mounts.contains(&(name.to_owned(), path.to_owned(), Some(sub.to_owned()))),
                "{path} is the prepared directory {sub} of the emptyDir {name}: {mounts:?}"
            );
        }
        assert!(
            env_of(container).contains(&("HOME".to_owned(), "/var/lib/agent".to_owned())),
            "HOME is the prepared directory"
        );
        let volumes = pod["volumes"].as_sequence().expect("volumes");
        for name in ["tmp", "state"] {
            let volume = volumes
                .iter()
                .find(|volume| text(volume, &["name"]) == name)
                .unwrap_or_else(|| panic!("the volume {name}"));
            assert!(volume["emptyDir"].is_mapping(), "{name} is an emptyDir");
        }
    }
}
