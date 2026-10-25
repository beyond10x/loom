//! Acceptance for `story:loom-operator`: the Helm chart `charts/loom-agents`, rendered by Helm
//! itself (`helm template`, which must be on `PATH`; CI installs it). The chart's CRD is the
//! projected one byte for byte, its versions are the workspace version, its Role grants the
//! design's verbs on the resource and the child kinds (and `update` on `loomagents/finalizers`,
//! which `blockOwnerDeletion` needs) and nothing on Secrets, Pods or Events, and the controller's
//! image and `config.yaml` follow the values, with the defaults and with the overrides
//! `task chart-check` renders.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use b10x_loom_operator::config::{Config, Image, ModelRoute, Resources};
use serde::Deserialize;
use serde_yaml_ng::Value;

/// The overrides `task chart-check` renders the chart with, as `--set` arguments.
const OVERRIDES: [&str; 7] = [
    "image.tag=0.0.1-test",
    "agentImage.repository=registry.example/other-agent",
    "modelRoute.url=http://model-route.agents.svc:8080/v1",
    "modelRoute.protocol=messages",
    "modelRoute.model=small-model",
    "allowedSecretNames={platform-operator-secrets}",
    "allowedEgressHosts={chat.example,models.example}",
];

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn chart() -> PathBuf {
    root().join("charts/loom-agents")
}

/// The documents `helm template` renders with `sets`, in namespace `agents`.
fn template(sets: &[&str]) -> Vec<Value> {
    let mut command = Command::new("helm");
    command
        .args(["template", "loom-agents"])
        .arg(chart())
        .args(["--namespace", "agents"]);
    for set in sets {
        command.args(["--set", set]);
    }
    let output = command
        .output()
        .expect("helm runs: install Helm 3 and put it on PATH");
    assert!(
        output.status.success(),
        "helm template failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("UTF-8");
    serde_yaml_ng::Deserializer::from_str(&text)
        .map(|document| Value::deserialize(document).expect("a YAML document"))
        .filter(|document| !document.is_null())
        .collect()
}

fn kind<'a>(documents: &'a [Value], kind: &str) -> &'a Value {
    let mut found = documents.iter().filter(|document| document["kind"] == kind);
    let first = found.next().unwrap_or_else(|| panic!("a {kind}"));
    assert!(found.next().is_none(), "one {kind}");
    first
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_sequence()
        .expect("a list")
        .iter()
        .map(|item| item.as_str().expect("a string").to_owned())
        .collect()
}

/// The `config.yaml` the controller reads, from the rendered ConfigMap.
fn config(documents: &[Value]) -> Config {
    let map = kind(documents, "ConfigMap");
    Config::parse(map["data"]["config.yaml"].as_str().expect("config.yaml"))
        .expect("config.yaml is the controller's configuration")
}

#[test]
fn the_crd_is_the_projected_one_byte_for_byte() {
    let copy =
        fs::read(chart().join("crds/loomagents.loom.beyond10x.dev.yaml")).expect("the chart's CRD");
    let projected = fs::read(root().join(b10x_loom_resource::crd::FILE)).expect("the CRD");
    assert!(
        copy == projected,
        "charts/loom-agents/crds/ differs from deploy/crd/: run task crd-generate"
    );
}

#[test]
fn the_chart_and_its_images_carry_the_workspace_version() {
    let text = fs::read_to_string(chart().join("Chart.yaml")).expect("Chart.yaml");
    let manifest: Value = serde_yaml_ng::from_str(&text).expect("Chart.yaml is YAML");
    assert_eq!(manifest["name"], "loom-agents");
    assert_eq!(manifest["version"], VERSION);
    assert_eq!(manifest["appVersion"], VERSION);
    assert!(
        manifest["description"]
            .as_str()
            .is_some_and(|d| !d.is_empty())
    );

    let text = fs::read_to_string(chart().join("values.yaml")).expect("values.yaml");
    assert!(
        !text.lines().any(|line| line.trim_start().starts_with('#')),
        "values.yaml carries no comments"
    );
    let values: Value = serde_yaml_ng::from_str(&text).expect("values.yaml is YAML");
    assert_eq!(values["image"]["tag"], VERSION);
    assert_eq!(values["agentImage"]["tag"], VERSION);
}

#[test]
fn the_role_grants_the_designs_verbs_and_nothing_on_secrets() {
    let documents = template(&[]);
    let role = kind(&documents, "Role");
    assert_eq!(role["metadata"]["namespace"], "agents");
    let rules = role["rules"].as_sequence().expect("rules");
    let mut granted: Vec<(String, String, Vec<String>)> = Vec::new();
    for rule in rules {
        for group in strings(&rule["apiGroups"]) {
            for resource in strings(&rule["resources"]) {
                granted.push((group.clone(), resource, strings(&rule["verbs"])));
            }
        }
    }
    granted.sort();
    let own = ["get", "list", "watch", "update", "patch"]
        .map(String::from)
        .to_vec();
    let children = [
        "create", "update", "patch", "get", "list", "watch", "delete",
    ]
    .map(String::from)
    .to_vec();
    let mut expected = vec![
        (String::new(), "configmaps".to_owned(), children.clone()),
        (
            String::new(),
            "serviceaccounts".to_owned(),
            children.clone(),
        ),
        (String::new(), "services".to_owned(), children.clone()),
        (
            "apps".to_owned(),
            "deployments".to_owned(),
            children.clone(),
        ),
        (
            "loom.beyond10x.dev".to_owned(),
            "loomagents".to_owned(),
            own.clone(),
        ),
        (
            "loom.beyond10x.dev".to_owned(),
            "loomagents/finalizers".to_owned(),
            vec!["update".to_owned()],
        ),
        (
            "loom.beyond10x.dev".to_owned(),
            "loomagents/status".to_owned(),
            own,
        ),
        (
            "networking.k8s.io".to_owned(),
            "networkpolicies".to_owned(),
            children,
        ),
    ];
    expected.sort();
    assert_eq!(granted, expected);
    assert!(
        documents
            .iter()
            .all(|document| document["kind"] != "ClusterRole")
    );

    let binding = kind(&documents, "RoleBinding");
    assert_eq!(binding["roleRef"]["kind"], "Role");
    assert_eq!(binding["roleRef"]["name"], role["metadata"]["name"]);
    let account = kind(&documents, "ServiceAccount");
    assert_eq!(binding["subjects"][0]["kind"], "ServiceAccount");
    assert_eq!(binding["subjects"][0]["name"], account["metadata"]["name"]);
}

#[test]
fn the_controller_runs_the_configured_image_with_the_values_as_its_config() {
    let documents = template(&[]);
    let deployment = kind(&documents, "Deployment");
    assert_eq!(deployment["spec"]["replicas"], 1);
    assert_eq!(deployment["spec"]["strategy"]["type"], "Recreate");
    let pod = &deployment["spec"]["template"]["spec"];
    let account = kind(&documents, "ServiceAccount");
    assert_eq!(pod["serviceAccountName"], account["metadata"]["name"]);
    let container = &pod["containers"][0];
    assert_eq!(
        container["image"],
        format!("registry.example/loom-operator:{VERSION}").as_str()
    );
    assert_eq!(
        strings(&container["args"]),
        ["run", "--config", "/etc/loom-operator/config.yaml"]
    );
    assert_eq!(
        container["volumeMounts"][0]["mountPath"],
        "/etc/loom-operator"
    );
    assert_eq!(container["volumeMounts"][0]["readOnly"], true);
    let security = &container["securityContext"];
    assert_eq!(security["runAsNonRoot"], true);
    assert_eq!(security["runAsUser"], 10001);
    assert_eq!(security["readOnlyRootFilesystem"], true);
    assert_eq!(security["allowPrivilegeEscalation"], false);
    assert_eq!(strings(&security["capabilities"]["drop"]), ["ALL"]);
    let map = kind(&documents, "ConfigMap");
    assert_eq!(
        pod["volumes"][0]["configMap"]["name"],
        map["metadata"]["name"]
    );

    assert_eq!(
        config(&documents),
        Config {
            namespace: "agents".to_owned(),
            default_image: Image {
                repository: "registry.example/loom-agent".to_owned(),
                tag: VERSION.to_owned(),
            },
            default_resources: Resources {
                cpu: "250m".to_owned(),
                memory: "256Mi".to_owned(),
            },
            model_route: None,
            allowed_secret_names: Vec::new(),
            allowed_egress_hosts: Vec::new(),
        }
    );
}

#[test]
fn overrides_reach_the_controller_image_and_config() {
    let documents = template(&OVERRIDES);
    let deployment = kind(&documents, "Deployment");
    assert_eq!(
        deployment["spec"]["template"]["spec"]["containers"][0]["image"],
        "registry.example/loom-operator:0.0.1-test"
    );
    assert_eq!(
        config(&documents),
        Config {
            namespace: "agents".to_owned(),
            default_image: Image {
                repository: "registry.example/other-agent".to_owned(),
                tag: VERSION.to_owned(),
            },
            default_resources: Resources {
                cpu: "250m".to_owned(),
                memory: "256Mi".to_owned(),
            },
            model_route: Some(ModelRoute {
                url: "http://model-route.agents.svc:8080/v1".to_owned(),
                protocol: "messages".to_owned(),
                model: "small-model".to_owned(),
            }),
            allowed_secret_names: vec!["platform-operator-secrets".to_owned()],
            allowed_egress_hosts: vec!["chat.example".to_owned(), "models.example".to_owned()],
        }
    );
}

#[test]
fn the_taskfile_renders_the_overrides_this_test_asserts() {
    let taskfile = fs::read_to_string(root().join("Taskfile.yml")).expect("Taskfile.yml");
    for set in OVERRIDES {
        assert!(
            taskfile.contains(&format!("--set '{set}'")),
            "task chart-check renders --set '{set}'"
        );
    }
}
