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
        2,
        "the staging and the writable dirs, no check: the image holds no b10x-loom to check with: {pod:?}"
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
/// name the renderer starts setting without adding it there fails here. A credential-free
/// resource of either variant, declaring every upstream and no `spec.env`, is rendered too.
#[test]
fn a_reserved_variable_name_is_refused() {
    let mut set = Vec::new();
    let every = boundary(true, true, true);
    for (resource, config) in [
        (command_with(""), config()),
        (serve_with(""), config()),
        (
            format!("{HEAD}  routes: {{}}\n  command: {{args: [--iterations, '1']}}\n{every}"),
            boundary_config(),
        ),
        (credential_free_serve(&every), boundary_config()),
    ] {
        let pod = pod_of(&render_document(&resource, &config).expect("rendered"));
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

/// The model gateway a credential-free resource declares: a cluster service with a port and a
/// path.
const MODEL_UPSTREAM: &str = "http://llm-gateway.llm.svc:8080";
/// The Connectors instance, by its fully qualified cluster name.
const CONNECTORS_UPSTREAM: &str = "http://connectors.connectors.svc.cluster.local:8080";
/// Its git smart-HTTP path, on the default port.
const GIT_UPSTREAM: &str = "http://connectors.connectors.svc/git";

/// `text` as a YAML flow scalar: JSON text, which YAML reads as written.
fn quoted(text: &str) -> String {
    serde_json::to_string(text).expect("JSON text")
}

/// The test configuration, allowing `upstreams` as the boundary upstreams.
fn allowing(upstreams: &[&str]) -> Config {
    Config::parse(&format!(
        "{CONFIG}allowedBoundaryUpstreams: {}\n",
        serde_json::to_string(upstreams).expect("JSON text")
    ))
    .expect("the test configuration with boundary upstreams")
}

/// The test configuration with the three upstreams the credential-free resources declare.
fn boundary_config() -> Config {
    allowing(&[MODEL_UPSTREAM, CONNECTORS_UPSTREAM, GIT_UPSTREAM])
}

/// `spec.boundary` declaring `model`, `connectors` and `git` where asked, as YAML.
fn boundary(model: bool, connectors: bool, git: bool) -> String {
    let mut out = String::from("  boundary:\n");
    if model {
        out.push_str(&format!(
            "    model: {{upstream: '{MODEL_UPSTREAM}', audience: llm-gateway}}\n"
        ));
    }
    if connectors {
        out.push_str(&format!(
            "    connectors: {{upstream: '{CONNECTORS_UPSTREAM}', audience: connectors, \
             operations: [merge_request.create, merge_request.get]}}\n"
        ));
    }
    if git {
        out.push_str(&format!(
            "    git: {{upstream: '{GIT_UPSTREAM}', audience: connectors-git, repositories: \
             [agents/takt-target, agents/takt-docs]}}\n"
        ));
    }
    out
}

/// A `spec.boundary` declaring only `model`, at `upstream` with `audience`.
fn model_boundary(upstream: &str, audience: &str) -> String {
    format!(
        "  boundary:\n    model: {{upstream: {}, audience: {}}}\n",
        quoted(upstream),
        quoted(audience)
    )
}

/// A `spec.boundary` declaring only `connectors`, admitting `operations`.
fn connectors_boundary(operations: &[&str]) -> String {
    format!(
        "  boundary:\n    connectors: {{upstream: '{CONNECTORS_UPSTREAM}', audience: connectors, \
         operations: {}}}\n",
        serde_json::to_string(operations).expect("JSON text")
    )
}

/// A `spec.boundary` declaring only `git`, admitting `repositories`.
fn git_boundary(repositories: &[&str]) -> String {
    format!(
        "  boundary:\n    git: {{upstream: '{GIT_UPSTREAM}', audience: connectors-git, \
         repositories: {}}}\n",
        serde_json::to_string(repositories).expect("JSON text")
    )
}

/// A takt delivery with no credential in its pod: its own image and arguments, a workspace on a
/// claim, plain environment and its run configuration as a file, then `rest` (a boundary, as a
/// rule); no `spec.agent`, no `secretRef`, no `secretFiles`.
fn credential_free_command(rest: &str) -> String {
    format!(
        "{HEAD}  routes: {{}}\n  image: {{repository: registry.example/takt, tag: '0.12.0'}}\n  \
         command: {{args: [--iterations, '1']}}\n  workspace: {{mountPath: /workspace, claimName: \
         takt-workspace}}\n{}{}{rest}",
        env(&[("CARGO_NET_OFFLINE", "true")]),
        config_files(&[("run.toml", "iterations = 1\n")]),
    )
}

/// The example without its two host egress entries, which a boundary refuses; its namespace
/// entries (logs and metrics in `monitoring`) and its ingress stay.
fn credential_free_agent() -> String {
    let agent: String = example()
        .lines()
        .filter(|line| !line.contains("{name: chat-api,") && !line.contains("{name: model-api,"))
        .map(|line| format!("{line}\n"))
        .collect();
    assert_eq!(
        agent.lines().count() + 2,
        example().lines().count(),
        "two host entries removed"
    );
    agent
}

/// The serve variant of a credential-free agent: the example without host egress, the default
/// routes, then `rest`.
fn credential_free_serve(rest: &str) -> String {
    wrapped(&credential_free_agent(), &format!("  routes: {{}}\n{rest}"))
}

/// The container named `name` among `pod`'s init containers.
fn init_container<'a>(pod: &'a Value, name: &str) -> Option<&'a Value> {
    pod["initContainers"]
        .as_sequence()
        .into_iter()
        .flatten()
        .find(|container| text(container, &["name"]) == name)
}

/// An egress rule as the namespace its peer names (none for a rule without a peer) and its
/// ports as `(protocol, port)`.
type EgressRule = (Option<String>, Vec<(String, i64)>);

/// Each egress rule of the rendered NetworkPolicy.
fn egress_of(rendered: &str) -> Vec<EgressRule> {
    let docs = documents(rendered);
    let policy = docs
        .iter()
        .find(|doc| text(doc, &["kind"]) == "NetworkPolicy")
        .expect("a NetworkPolicy");
    at(policy, &["spec", "egress"])
        .as_sequence()
        .expect("egress rules")
        .iter()
        .map(|rule| {
            let namespace = rule["to"].as_sequence().map(|peers| {
                assert_eq!(peers.len(), 1, "one peer: {rule:?}");
                text(
                    &peers[0],
                    &[
                        "namespaceSelector",
                        "matchLabels",
                        "kubernetes.io/metadata.name",
                    ],
                )
                .to_owned()
            });
            let ports = rule["ports"]
                .as_sequence()
                .expect("ports")
                .iter()
                .map(|port| {
                    (
                        text(port, &["protocol"]).to_owned(),
                        port["port"].as_i64().expect("a port number"),
                    )
                })
                .collect();
            (namespace, ports)
        })
        .collect()
}

/// A rule to `namespace` on one port.
fn rule(namespace: &str, protocol: &str, port: i64) -> EgressRule {
    (
        Some(namespace.to_owned()),
        vec![(protocol.to_owned(), port)],
    )
}

/// The DNS rule every NetworkPolicy opens first.
fn dns() -> EgressRule {
    (
        Some("kube-system".to_owned()),
        vec![("UDP".to_owned(), 53), ("TCP".to_owned(), 53)],
    )
}

/// The values a refusal of a boundary member must never repeat.
const BOUNDARY_VALUES: [&str; 9] = [
    "llm-gateway",
    "LLM-Gateway",
    "connectors.connectors",
    "not-a-real-password",
    "000fake",
    "000-fake",
    "merge",
    "takt-",
    "10.0.0.1",
];

fn repeats_no_boundary_value(message: &str) {
    for value in BOUNDARY_VALUES {
        assert!(!message.contains(value), "{value} repeated: {message}");
    }
}

/// `story:boundary-container-projected-token` (design D243 § 2, § 4, decision A1): a resource
/// that declares `spec.boundary` gets a second container, `boundary`, as a native sidecar (an
/// init container with `restartPolicy: Always`, last, so a Job still completes when its program
/// ends) running `b10x-loom agent boundary` from the controller's default image as user 10002.
/// It alone mounts the projected volume that holds one service-account token per declared
/// upstream (its audience the declared one, 600 s); it mounts nothing else and gets no
/// environment, and the pod's own token stays unmounted.
#[test]
fn a_boundary_renders_a_second_container_holding_the_only_token() {
    let every = boundary(true, true, true);
    for resource in [
        credential_free_command(&every),
        credential_free_serve(&every),
    ] {
        let pod = pod_of(&render_document(&resource, &boundary_config()).expect("rendered"));
        let init = pod["initContainers"]
            .as_sequence()
            .expect("init containers");
        let sidecar = init.last().expect("an init container");
        assert_eq!(
            text(sidecar, &["name"]),
            "boundary",
            "the last init container"
        );
        assert_eq!(
            init.iter()
                .filter(|container| text(container, &["name"]) == "boundary")
                .count(),
            1
        );
        assert_eq!(
            text(sidecar, &["restartPolicy"]),
            "Always",
            "a native sidecar"
        );
        assert_eq!(
            text(sidecar, &["image"]),
            "registry.example/loom-agent:0.40.0",
            "the controller's default image, never spec.image"
        );
        assert_eq!(strings(sidecar, &["command"]), ["b10x-loom"]);
        assert_eq!(
            strings(sidecar, &["args"]),
            [
                "agent",
                "boundary",
                "--model-upstream",
                MODEL_UPSTREAM,
                "--model-token",
                "/var/run/loom/boundary/tokens/model",
                "--connectors-upstream",
                CONNECTORS_UPSTREAM,
                "--connectors-token",
                "/var/run/loom/boundary/tokens/connectors",
                "--operation",
                "merge_request.create",
                "--operation",
                "merge_request.get",
                "--git-upstream",
                GIT_UPSTREAM,
                "--git-token",
                "/var/run/loom/boundary/tokens/git",
                "--repository",
                "agents/takt-target",
                "--repository",
                "agents/takt-docs",
            ]
        );
        let probe = &sidecar["startupProbe"];
        assert_eq!(
            strings(probe, &["exec", "command"]),
            [
                "b10x-loom",
                "agent",
                "boundary-ready",
                "--port",
                "8401",
                "--port",
                "8402",
                "--port",
                "8403"
            ]
        );
        assert_eq!(probe["periodSeconds"], Value::from(1));
        assert_eq!(probe["failureThreshold"], Value::from(30));

        let security = &sidecar["securityContext"];
        assert_eq!(security["runAsUser"], Value::from(10002));
        assert_eq!(security["runAsGroup"], Value::from(10002));
        assert_eq!(security["runAsNonRoot"], Value::Bool(true));
        assert_eq!(security["readOnlyRootFilesystem"], Value::Bool(true));
        assert_eq!(security["allowPrivilegeEscalation"], Value::Bool(false));
        assert_eq!(strings(security, &["capabilities", "drop"]), ["ALL"]);
        assert!(security["capabilities"]["add"].is_null(), "no capability");
        assert_eq!(
            text(security, &["seccompProfile", "type"]),
            "RuntimeDefault"
        );
        let resources = &sidecar["resources"];
        assert!(
            resources["limits"]["cpu"].is_string() && resources["limits"]["memory"].is_string()
        );
        assert_eq!(resources["requests"], resources["limits"]);
        assert!(
            sidecar["env"].is_null() && sidecar["envFrom"].is_null(),
            "no environment on the boundary"
        );
        assert_eq!(
            mounts_of(sidecar),
            [(
                "boundary-tokens".to_owned(),
                "/var/run/loom/boundary/tokens".to_owned(),
                None
            )],
            "its only mount: no workspace, ConfigMap, emptyDir or Secret"
        );
        assert_eq!(sidecar["volumeMounts"][0]["readOnly"], Value::Bool(true));

        let volumes = pod["volumes"].as_sequence().expect("volumes");
        let tokens = volumes
            .iter()
            .find(|volume| text(volume, &["name"]) == "boundary-tokens")
            .expect("the token volume");
        let sources = tokens["projected"]["sources"]
            .as_sequence()
            .expect("projected sources");
        let declared: Vec<(String, i64, String)> = sources
            .iter()
            .map(|source| {
                assert_eq!(
                    source.as_mapping().map(serde_yaml_ng::Mapping::len),
                    Some(1),
                    "a service-account token and nothing else: {source:?}"
                );
                let token = &source["serviceAccountToken"];
                (
                    text(token, &["audience"]).to_owned(),
                    token["expirationSeconds"].as_i64().expect("an expiry"),
                    text(token, &["path"]).to_owned(),
                )
            })
            .collect();
        assert_eq!(
            declared,
            [
                ("llm-gateway".to_owned(), 600, "model".to_owned()),
                ("connectors".to_owned(), 600, "connectors".to_owned()),
                ("connectors-git".to_owned(), 600, "git".to_owned()),
            ]
        );
        for list in ["containers", "initContainers"] {
            for container in pod[list].as_sequence().into_iter().flatten() {
                if text(container, &["name"]) != "boundary" {
                    assert!(
                        !mounts_of(container)
                            .iter()
                            .any(|(name, _, _)| name == "boundary-tokens"),
                        "only the boundary mounts the tokens: {container:?}"
                    );
                }
                assert!(container["envFrom"].is_null(), "no Secret environment");
            }
        }
        assert!(
            volumes.iter().all(|volume| volume["secret"].is_null()),
            "no Secret volume"
        );
        assert_eq!(pod["automountServiceAccountToken"], Value::Bool(false));
    }

    let pod = pod_of(
        &render_document(
            &credential_free_command(&boundary(true, false, false)),
            &boundary_config(),
        )
        .expect("rendered"),
    );
    let sidecar = init_container(&pod, "boundary").expect("the boundary");
    assert_eq!(
        strings(sidecar, &["args"]),
        [
            "agent",
            "boundary",
            "--model-upstream",
            MODEL_UPSTREAM,
            "--model-token",
            "/var/run/loom/boundary/tokens/model"
        ],
        "only the declared upstreams"
    );
    assert_eq!(
        strings(&sidecar["startupProbe"], &["exec", "command"]),
        ["b10x-loom", "agent", "boundary-ready", "--port", "8401"]
    );
    let tokens = pod["volumes"]
        .as_sequence()
        .expect("volumes")
        .iter()
        .find(|volume| text(volume, &["name"]) == "boundary-tokens")
        .expect("the token volume");
    assert_eq!(
        tokens["projected"]["sources"].as_sequence().map(Vec::len),
        Some(1)
    );

    for resource in [takt_resource(), default_resource()] {
        let pod = pod_of(&render_document(&resource, &config()).expect("rendered"));
        assert!(
            init_container(&pod, "boundary").is_none(),
            "no boundary without one"
        );
        assert!(
            !pod["volumes"]
                .as_sequence()
                .expect("volumes")
                .iter()
                .any(|volume| text(volume, &["name"]) == "boundary-tokens")
        );
    }
}

/// The agent container reaches each declared upstream through the boundary on loopback: it gets
/// the boundary's URL for each, after the controller's own environment and before `spec.env`,
/// mounts no token and has no Secret. With the default routes and `boundary.model`, the routes
/// document's endpoint is the boundary, with the model route's protocol and model.
#[test]
fn the_agent_container_gets_the_boundary_urls_and_no_token() {
    let urls = [
        ("LOOM_BOUNDARY_MODEL_URL", "http://127.0.0.1:8401/v1"),
        ("LOOM_BOUNDARY_CONNECTORS_URL", "http://127.0.0.1:8402"),
        ("LOOM_BOUNDARY_GIT_URL", "http://127.0.0.1:8403"),
    ];
    let rendered = render_document(
        &credential_free_command(&boundary(true, true, true)),
        &boundary_config(),
    )
    .expect("rendered");
    let pod = pod_of(&rendered);
    assert_eq!(
        pod["containers"].as_sequence().map(Vec::len),
        Some(1),
        "the boundary is an init container, not a second program"
    );
    let agent = &pod["containers"][0];
    assert_eq!(text(agent, &["name"]), "loom-agent");
    let mut expected = owned(&[("HOME", "/var/lib/agent")]);
    expected.extend(owned(&urls));
    expected.extend(owned(&[("CARGO_NET_OFFLINE", "true")]));
    assert_eq!(
        env_of(agent),
        expected,
        "the controller's own, the boundary's URLs, then spec.env"
    );
    assert!(agent["envFrom"].is_null(), "no Secret as its environment");
    assert!(
        !mounts_of(agent)
            .iter()
            .any(|(name, _, _)| name == "boundary-tokens"),
        "no token"
    );
    let data = config_map_data(&rendered);
    let routes: Value = serde_yaml_ng::from_str(
        &data
            .iter()
            .find(|(key, _)| key == "routes.yaml")
            .expect("routes.yaml")
            .1,
    )
    .expect("routes.yaml is YAML");
    assert_eq!(
        text(&routes, &["endpoints", "model-route", "base_url"]),
        "http://127.0.0.1:8401/v1"
    );
    assert_eq!(
        text(&routes, &["endpoints", "model-route", "protocol"]),
        "messages"
    );
    assert_eq!(
        text(&routes, &["endpoints", "model-route", "auth", "kind"]),
        "anonymous"
    );

    for declared in [
        [true, false, false],
        [false, true, false],
        [false, false, true],
        [false, true, true],
    ] {
        let [model, connectors, git] = declared;
        let rendered = render_document(
            &credential_free_serve(&boundary(model, connectors, git)),
            &boundary_config(),
        )
        .expect("rendered");
        let given: Vec<(&str, &str)> = urls
            .iter()
            .zip(declared)
            .filter(|(_, declared)| *declared)
            .map(|(url, _)| *url)
            .collect();
        let pod = pod_of(&rendered);
        assert_eq!(
            env_of(&pod["containers"][0]),
            owned(&given),
            "each URL only for a declared upstream, on the serve variant too"
        );
        if model {
            let routes: Value = serde_yaml_ng::from_str(
                &config_map_data(&rendered)
                    .iter()
                    .find(|(key, _)| key == "routes.yaml")
                    .expect("routes.yaml")
                    .1,
            )
            .expect("YAML");
            assert_eq!(
                text(&routes, &["endpoints", "model-route", "base_url"]),
                "http://127.0.0.1:8401/v1"
            );
            assert_eq!(
                text(&routes, &["routes", "small"]),
                "model-route/small-model"
            );
        }
    }
}

/// A credential-free agent holds no Secret: `spec.boundary` beside `spec.secretRef` or
/// `spec.secretFiles` is refused at that member before any child exists, even when the
/// configuration allows the Secret.
#[test]
fn a_boundary_beside_a_secret_is_refused() {
    let every = boundary(true, true, true);
    for (secret, member) in [
        (
            "  secretRef: {name: platform-operator-secrets}\n",
            "spec.secretRef",
        ),
        (
            "  secretFiles: {name: takt-forge, mountPath: /var/run/secrets/takt}\n",
            "spec.secretFiles",
        ),
    ] {
        let rest = format!("{secret}{every}");
        for resource in [credential_free_command(&rest), credential_free_serve(&rest)] {
            let message = refusal(&resource, &boundary_config());
            assert!(
                message.starts_with(&format!("refused: BoundaryWithSecret at {member}: ")),
                "{message}"
            );
            assert!(
                !message.contains("platform-operator-secrets") && !message.contains("takt-forge"),
                "{message}"
            );
        }
        render_document(&credential_free_command(secret), &boundary_config())
            .expect("the Secret alone is admitted");
    }
}

/// The controller's `allowedBoundaryUpstreams` lists the upstreams a resource may name, exactly;
/// empty, as built in, it admits none.
#[test]
fn a_boundary_upstream_the_controller_does_not_allow_is_refused() {
    for (member, declared) in [
        ("model", boundary(true, false, false)),
        ("connectors", boundary(false, true, false)),
        ("git", boundary(false, false, true)),
    ] {
        let expected = format!(
            "refused: BoundaryUpstreamNotAllowed at spec.boundary.{member}.upstream: the \
             controller's allowedBoundaryUpstreams does not hold it"
        );
        assert_eq!(
            refusal(&credential_free_command(&declared), &config()),
            expected
        );
        assert_eq!(
            refusal(&credential_free_serve(&declared), &allowing(&[])),
            expected
        );
    }
    for upstream in [
        "http://llm-gateway.llm.svc:8080/v2",
        "http://llm-gateway.llm.svc:8081/v1",
        "http://llm-gateway.llm.svc.cluster.local:8080/v1",
        "http://llm-gateway.llm.svc:8080/v1",
    ] {
        let message = refusal(
            &credential_free_command(&model_boundary(upstream, "llm-gateway")),
            &boundary_config(),
        );
        assert!(
            message.starts_with(
                "refused: BoundaryUpstreamNotAllowed at spec.boundary.model.upstream: "
            ),
            "{upstream}: {message}"
        );
        repeats_no_boundary_value(&message);
    }
}

/// `story:credential-free-network-policy`: a credential-free agent's NetworkPolicy opens DNS,
/// each declared upstream's namespace on its port (80 when the URL names none) and the spec's
/// namespace egress entries, and nothing else: no rule without a peer, so no host entry and no
/// model-route rule.
#[test]
fn a_credential_free_agent_reaches_dns_and_its_upstream_namespaces_only() {
    let every = boundary(true, true, true);
    let serve =
        render_document(&credential_free_serve(&every), &boundary_config()).expect("rendered");
    assert_eq!(
        egress_of(&serve),
        [
            dns(),
            rule("monitoring", "TCP", 3100),
            rule("monitoring", "TCP", 9090),
            rule("llm", "TCP", 8080),
            rule("connectors", "TCP", 8080),
            rule("connectors", "TCP", 80),
        ]
    );
    let docs = documents(&serve);
    let policy = docs
        .iter()
        .find(|doc| text(doc, &["kind"]) == "NetworkPolicy")
        .expect("a NetworkPolicy");
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
        "operator-clients",
        "ingress as declared"
    );

    let command =
        render_document(&credential_free_command(&every), &boundary_config()).expect("rendered");
    assert_eq!(
        egress_of(&command),
        [
            dns(),
            rule("llm", "TCP", 8080),
            rule("connectors", "TCP", 8080),
            rule("connectors", "TCP", 80),
        ]
    );
    let only_git = render_document(
        &credential_free_command(&boundary(false, false, true)),
        &boundary_config(),
    )
    .expect("rendered");
    assert_eq!(
        egress_of(&only_git),
        [dns(), rule("connectors", "TCP", 80)],
        "no model-route rule beside a boundary, even with the default routes"
    );

    let without = render_document(&command_without_agent(""), &config()).expect("rendered");
    assert_eq!(
        egress_of(&without),
        [dns(), (None, vec![("TCP".to_owned(), 8080)])],
        "without a boundary the default routes still open the model route's port"
    );
}

/// A NetworkPolicy names no host, so a host entry's rule opens its port to every destination;
/// beside a boundary such an entry is refused at its host, even one the configuration allows.
#[test]
fn a_host_egress_entry_beside_a_boundary_is_refused() {
    let first = wrapped(
        &example(),
        &format!("  routes: {{}}\n{}", boundary(true, false, false)),
    );
    let message = refusal(&first, &boundary_config());
    assert!(
        message.starts_with(
            "refused: BoundaryEgressHost at spec.agent.deploy.kubernetes.egress[0].host: "
        ),
        "{message}"
    );
    assert!(!message.contains("chat.example"), "{message}");

    let agent = credential_free_agent().replace(
        "      - {name: metrics,",
        "      - {name: chat-api, host: chat.example, port: 443}\n      - {name: metrics,",
    );
    let message = refusal(
        &wrapped(
            &agent,
            &format!("  routes: {{}}\n{}", boundary(false, true, true)),
        ),
        &boundary_config(),
    );
    assert!(
        message.starts_with(
            "refused: BoundaryEgressHost at spec.agent.deploy.kubernetes.egress[1].host: "
        ),
        "{message}"
    );
    assert!(!message.contains("chat.example"), "{message}");
    render_document(&wrapped(&agent, "  routes: {}\n"), &boundary_config())
        .expect("the same host is admitted without a boundary");
}

/// An upstream is a plain `http://` URL of a cluster service, `<service>.<namespace>.svc` with
/// an optional `.cluster.local`, port and path, so the NetworkPolicy can name its namespace;
/// anything else is refused at its member, even when the configuration lists it, and the refusal
/// never repeats it. An `https` upstream is refused in this wave (approval record wave-d243-l).
#[test]
fn an_upstream_that_is_no_cluster_service_is_refused() {
    let long_label = format!("http://{}.llm.svc/v1", "a".repeat(64));
    let invalid = [
        "https://llm-gateway.llm.svc:8443/v1",
        "HTTP://llm-gateway.llm.svc/v1",
        "ftp://llm-gateway.llm.svc/v1",
        "llm-gateway.llm.svc:8080",
        "//llm-gateway.llm.svc:8080",
        "http://user:not-a-real-password@llm-gateway.llm.svc/v1",
        "http://user@llm-gateway.llm.svc/v1",
        "http://llm-gateway.llm.svc/v1?key=not-a-real-password",
        "http://llm-gateway.llm.svc?key",
        "http://llm-gateway.llm.svc/v1#part",
        "http://llm-gateway.example/v1",
        "http://llm-gateway.svc/v1",
        "http://llm-gateway.llm.svc.cluster/v1",
        "http://llm-gateway.llm.svc.cluster.local.example/v1",
        "http://llm-gateway.llm.other.svc/v1",
        "http://llm-gateway.llm.svc./v1",
        "http://LLM-Gateway.llm.svc/v1",
        "http://-llm-gateway.llm.svc/v1",
        "http://llm-gateway-.llm.svc/v1",
        "http://llm_gateway.llm.svc/v1",
        "http://.llm.svc/v1",
        &long_label,
        "http://10.0.0.1:8080/v1",
        "http://[fd00::1]:8080/v1",
        "http://llm-gateway.llm.svc:0/v1",
        "http://llm-gateway.llm.svc:65536/v1",
        "http://llm-gateway.llm.svc:/v1",
        "http://llm-gateway.llm.svc:80a/v1",
        "http://llm-gateway.llm.svc:+80/v1",
        "http://llm-gateway.llm.svc/v1//chat",
        "http://llm-gateway.llm.svc/v1/../admin",
        "http://llm-gateway.llm.svc/v1/./chat",
        "http://llm-gateway.llm.svc/v1/",
        "http://llm-gateway.llm.svc/",
        "http://llm-gateway.llm.svc/v 1",
        "http://llm-gateway.llm.svc/v1%2Fadmin",
        "http://llm-gateway.llm.svc/v1\\admin",
        "http://llm-gateway.llm.svc /v1",
        "http://llm-gateway.llm.svc/v1\n",
        "http://",
        "",
    ];
    for upstream in invalid {
        let message = refusal(
            &credential_free_command(&model_boundary(upstream, "llm-gateway")),
            &allowing(&[upstream]),
        );
        assert!(
            message
                .starts_with("refused: BoundaryUpstreamInvalid at spec.boundary.model.upstream: "),
            "{upstream:?}: {message}"
        );
        repeats_no_boundary_value(&message);
        assert!(
            !message.contains("fd00") && !message.contains("aaaa"),
            "{message}"
        );
    }
    let connectors = "  boundary:\n    connectors: {upstream: 'https://connectors.connectors.svc', \
                      audience: connectors, operations: [merge_request.create]}\n";
    let git = "  boundary:\n    git: {upstream: 'http://connectors.svc/git', audience: \
               connectors-git, repositories: [agents/takt-target]}\n";
    for (declared, member) in [(connectors, "connectors"), (git, "git")] {
        let message = refusal(
            &credential_free_command(declared),
            &allowing(&[
                "https://connectors.connectors.svc",
                "http://connectors.svc/git",
            ]),
        );
        assert!(
            message.starts_with(&format!(
                "refused: BoundaryUpstreamInvalid at spec.boundary.{member}.upstream: "
            )),
            "{message}"
        );
        repeats_no_boundary_value(&message);
    }

    for (upstream, namespace, port) in [
        ("http://llm-gateway.llm.svc", "llm", 80),
        (
            "http://llm-gateway.llm.svc.cluster.local:8080/v1",
            "llm",
            8080,
        ),
        ("http://a.b-1.svc:65535/x/y.z_~-", "b-1", 65535),
        ("http://0gateway.llm.svc:1/v1/chat", "llm", 1),
    ] {
        let rendered = render_document(
            &credential_free_command(&model_boundary(upstream, "llm-gateway")),
            &allowing(&[upstream]),
        )
        .unwrap_or_else(|error| panic!("{upstream}: {error}"));
        assert_eq!(
            egress_of(&rendered),
            [dns(), rule(namespace, "TCP", port)],
            "{upstream}"
        );
    }
}

/// A boundary that declares none of `model`, `connectors` and `git` holds nothing to relay.
#[test]
fn a_boundary_that_declares_no_upstream_is_malformed() {
    for resource in [
        credential_free_command("  boundary: {}\n"),
        credential_free_serve("  boundary: {}\n"),
    ] {
        let message = refusal(&resource, &boundary_config());
        assert!(
            message.starts_with("refused: Malformed at spec.boundary: "),
            "{message}"
        );
    }
}

/// An audience is the token's audience, a plain word: empty, or holding whitespace or a control
/// character, it is `Malformed`, never repeated.
#[test]
fn a_boundary_audience_that_is_empty_or_not_one_word_is_malformed() {
    for audience in [
        "",
        "llm gateway",
        "llm\tgateway",
        " llm-gateway",
        "llm-gateway\n",
        "llm\u{7}gateway",
        "llm\u{a0}gateway",
    ] {
        let message = refusal(
            &credential_free_command(&model_boundary(MODEL_UPSTREAM, audience)),
            &boundary_config(),
        );
        assert!(
            message.starts_with("refused: Malformed at spec.boundary.model.audience: "),
            "{audience:?}: {message}"
        );
        repeats_no_boundary_value(&message);
        assert!(!message.contains("gateway"), "{message}");
    }
}

/// The operations a boundary admits are Connectors operation ids and its repositories are
/// `<owner>/<repo>`, each given once: an empty list, an entry out of form (one starting with `-`
/// would read as a flag of the boundary's command line) or given twice is `Malformed` at it.
#[test]
fn boundary_operations_and_repositories_out_of_form_are_malformed() {
    let operations: [(&[&str], &str); 8] = [
        (&[], "spec.boundary.connectors.operations"),
        (
            &["merge_request.create", "merge request"],
            "spec.boundary.connectors.operations[1]",
        ),
        (
            &["merge_request/create"],
            "spec.boundary.connectors.operations[0]",
        ),
        (
            &["merge_request:create"],
            "spec.boundary.connectors.operations[0]",
        ),
        (&[""], "spec.boundary.connectors.operations[0]"),
        (&["--model-token"], "spec.boundary.connectors.operations[0]"),
        (&["-merge"], "spec.boundary.connectors.operations[0]"),
        (
            &[
                "merge_request.create",
                "merge_request.get",
                "merge_request.create",
            ],
            "spec.boundary.connectors.operations[2]",
        ),
    ];
    for (listed, member) in operations {
        let message = refusal(
            &credential_free_command(&connectors_boundary(listed)),
            &boundary_config(),
        );
        assert!(
            message.starts_with(&format!("refused: Malformed at {member}: ")),
            "{listed:?}: {message}"
        );
        repeats_no_boundary_value(&message);
    }
    let repeated = refusal(
        &credential_free_command(&connectors_boundary(&[
            "merge_request.create",
            "merge_request.get",
            "merge_request.create",
        ])),
        &boundary_config(),
    );
    assert!(
        repeated.ends_with("it repeats spec.boundary.connectors.operations[0]"),
        "{repeated}"
    );

    let repositories: [(&[&str], &str); 16] = [
        (&[], "spec.boundary.git.repositories"),
        (&["takt-target"], "spec.boundary.git.repositories[0]"),
        (
            &["agents/takt-target/extra"],
            "spec.boundary.git.repositories[0]",
        ),
        (&["agents/"], "spec.boundary.git.repositories[0]"),
        (&["/takt-target"], "spec.boundary.git.repositories[0]"),
        (&["./takt-target"], "spec.boundary.git.repositories[0]"),
        (&["../takt-target"], "spec.boundary.git.repositories[0]"),
        (&["agents/.."], "spec.boundary.git.repositories[0]"),
        (&["agents/."], "spec.boundary.git.repositories[0]"),
        (
            &["agents/takt-target.git"],
            "spec.boundary.git.repositories[0]",
        ),
        (
            &["agents/takt-target.GIT"],
            "spec.boundary.git.repositories[0]",
        ),
        (&["agents/takt target"], "spec.boundary.git.repositories[0]"),
        (&["agents/takt:target"], "spec.boundary.git.repositories[0]"),
        (
            &["-agents/takt-target"],
            "spec.boundary.git.repositories[0]",
        ),
        (&[""], "spec.boundary.git.repositories[0]"),
        (
            &[
                "agents/takt-target",
                "agents/takt-docs",
                "agents/takt-target",
            ],
            "spec.boundary.git.repositories[2]",
        ),
    ];
    for (listed, member) in repositories {
        let message = refusal(
            &credential_free_command(&git_boundary(listed)),
            &boundary_config(),
        );
        assert!(
            message.starts_with(&format!("refused: Malformed at {member}: ")),
            "{listed:?}: {message}"
        );
        repeats_no_boundary_value(&message);
    }

    let admitted = render_document(
        &credential_free_command(&format!(
            "{}{}",
            connectors_boundary(&["merge_request.create", "Issue-2.get_all"]),
            git_boundary(&["agents/takt-target", "Agents.Two/repo_name-2.x"])
                .replace("  boundary:\n", "")
        )),
        &boundary_config(),
    )
    .expect("operation ids and repositories in form are admitted");
    let pod = pod_of(&admitted);
    let args = strings(
        init_container(&pod, "boundary").expect("the boundary"),
        &["args"],
    );
    assert!(
        args.ends_with(&[
            "--repository".to_owned(),
            "agents/takt-target".to_owned(),
            "--repository".to_owned(),
            "Agents.Two/repo_name-2.x".to_owned(),
        ]),
        "{args:?}"
    );
}

/// The boundary's strings reach the pod spec, so they are walked for a credential as
/// `spec.command` is, and a refusal never repeats one.
#[test]
fn a_credential_in_the_boundary_is_refused_without_repeating_it() {
    for (declared, expected) in [
        (
            model_boundary(MODEL_UPSTREAM, "ghp_000fake"),
            "refused: SecretLiteral at spec.boundary.model.audience: a value with the prefix \
             `ghp_`",
        ),
        (
            connectors_boundary(&["merge_request.create", "xoxb-000-fake"]),
            "refused: SecretLiteral at spec.boundary.connectors.operations[1]: a value with the \
             prefix `xoxb-`",
        ),
        (
            git_boundary(&["ghp_000fake/takt-target"]),
            "refused: SecretLiteral at spec.boundary.git.repositories[0]: a value with the prefix \
             `ghp_`",
        ),
    ] {
        let message = refusal(&credential_free_command(&declared), &boundary_config());
        assert!(message.starts_with(expected), "{expected} <- {message}");
        repeats_no_boundary_value(&message);
    }
}

/// The boundary's URLs are the controller's own environment: a `spec.env` entry may not take
/// their names (on either variant, with or without a boundary), and the `$(VAR)` walk expands a
/// reference to one the boundary declares to its URL, which completes no user information, while
/// a reference to one it does not declare is read as a possible URL scheme.
#[test]
fn the_boundary_urls_are_the_controllers_environment() {
    for name in [
        "LOOM_BOUNDARY_MODEL_URL",
        "LOOM_BOUNDARY_CONNECTORS_URL",
        "LOOM_BOUNDARY_GIT_URL",
    ] {
        let rest = env(&[("CARGO_HOME", "/w"), (name, "http://127.0.0.1:9/v1")]);
        for (resource, config) in [
            (command_with(&rest), config()),
            (serve_with(&rest), config()),
            (
                credential_free_serve(&format!("{rest}{}", boundary(true, true, true))),
                boundary_config(),
            ),
        ] {
            assert_eq!(
                refusal(&resource, &config),
                "refused: EnvNameConflict at spec.env[1].name: the controller sets it on the \
                 container"
            );
        }
    }

    let note = env(&[(
        "NOTE",
        "$(LOOM_BOUNDARY_MODEL_URL)u:not-a-real-password@forge.example",
    )]);
    render_document(
        &credential_free_serve(&format!("{note}{}", boundary(true, false, false))),
        &boundary_config(),
    )
    .expect("the model URL it expands to completes no user information");
    let message = refusal(
        &credential_free_serve(&format!("{note}{}", boundary(false, true, false))),
        &boundary_config(),
    );
    assert!(
        message.starts_with("refused: SecretLiteral at spec.env[0].value: ")
            && message.ends_with(", after `$(VAR)` expansion"),
        "an undeclared upstream's name is unknown: {message}"
    );
    repeats_no_boundary_value(&message);
}

#[test]
fn a_credential_free_command_resource_renders_as_its_golden_file() {
    let rendered = render_document(
        &credential_free_command(&boundary(true, true, true)),
        &boundary_config(),
    )
    .expect("rendered");
    let golden = root().join("crates/loom-operator/tests/golden/credential-free.yaml");
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
