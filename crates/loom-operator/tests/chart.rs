//! Acceptance for `story:loom-operator`: the Helm chart `charts/loom-agents`, rendered by Helm
//! itself (`helm template`, which must be on `PATH`; CI installs it). The chart's CRD is the
//! projected one byte for byte, its versions are the workspace version, its Role grants the
//! design's verbs on the resource and the child kinds (and `update` on `loomagents/finalizers`,
//! which `blockOwnerDeletion` needs), `get` and `list` on Pods (a finished run's exit code) and
//! nothing on Secrets or Events, and the controller's image and `config.yaml` follow the values,
//! with the defaults and with the overrides `task chart-check` renders.
//!
//! `story:chart-defaults-to-published-images`: the values name the published images
//! `ghcr.io/beyond10x/loom-operator` and `ghcr.io/beyond10x/loom-agent` with empty tags, and both
//! render at the chart's `appVersion` unless a tag is set.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use std::num::NonZeroU16;

use b10x_loom_operator::config::{
    AllowedUpstream, Config, Image, ModelRoute, Resources, UpstreamKind,
};
use b10x_loom_operator::render::render_document;
use serde::Deserialize;
use serde_yaml_ng::Value;

/// The overrides `task chart-check` renders the chart with, as `--set` arguments.
const OVERRIDES: [&str; 18] = [
    "image.tag=0.0.1-test",
    "agentImage.repository=registry.example/other-agent",
    "modelRoute.url=http://model-route.agents.svc:8080/v1",
    "modelRoute.protocol=messages",
    "modelRoute.model=small-model",
    "allowedSecretNames={platform-operator-secrets}",
    "allowedEgressHosts={chat.example,models.example}",
    "allowedClaimNames={takt-workspace}",
    "allowUnconfinedProfiles=true",
    "allowedSeccompLocalhostProfiles={profiles/takt.json}",
    "allowedAppArmorLocalhostProfiles={takt-delivery}",
    "allowedBoundaryUpstreams[0].kind=model",
    "allowedBoundaryUpstreams[0].url=http://llm-gateway.llm.svc:8080",
    "allowedBoundaryUpstreams[0].audience=llm-gateway",
    "allowedBoundaryUpstreams[0].port=8000",
    "allowedBoundaryUpstreams[1].kind=git",
    "allowedBoundaryUpstreams[1].url=http://connectors.connectors.svc/git",
    "allowedBoundaryUpstreams[1].audience=connectors-git",
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
    // `story:chart-defaults-to-published-images`: the published images, their tags following
    // `appVersion`, so a release moves only `Chart.yaml`.
    assert_eq!(
        values["image"]["repository"],
        "ghcr.io/beyond10x/loom-operator"
    );
    assert_eq!(values["image"]["tag"], "");
    assert_eq!(
        values["agentImage"]["repository"],
        "ghcr.io/beyond10x/loom-agent"
    );
    assert_eq!(values["agentImage"]["tag"], "");
}

/// `story:command-schedule`: the child kinds include `cronjobs`, which a scheduled `Command`
/// resource runs as.
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
        (
            String::new(),
            "pods".to_owned(),
            vec!["get".to_owned(), "list".to_owned()],
        ),
        (String::new(), "services".to_owned(), children.clone()),
        (
            "apps".to_owned(),
            "deployments".to_owned(),
            children.clone(),
        ),
        ("batch".to_owned(), "jobs".to_owned(), children.clone()),
        ("batch".to_owned(), "cronjobs".to_owned(), children.clone()),
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
        format!("ghcr.io/beyond10x/loom-operator:{VERSION}").as_str()
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
                repository: "ghcr.io/beyond10x/loom-agent".to_owned(),
                tag: VERSION.to_owned(),
            },
            default_resources: Resources {
                cpu: "250m".to_owned(),
                memory: "256Mi".to_owned(),
            },
            default_command_resources: Some(Resources {
                cpu: "4".to_owned(),
                memory: "8Gi".to_owned(),
            }),
            model_route: None,
            allowed_secret_names: Vec::new(),
            allowed_egress_hosts: Vec::new(),
            allowed_claim_names: Vec::new(),
            allow_unconfined_profiles: false,
            allowed_seccomp_localhost_profiles: Vec::new(),
            allowed_app_armor_localhost_profiles: Vec::new(),
            allowed_boundary_upstreams: Vec::new(),
        }
    );
}

#[test]
fn overrides_reach_the_controller_image_and_config() {
    let documents = template(&OVERRIDES);
    let deployment = kind(&documents, "Deployment");
    assert_eq!(
        deployment["spec"]["template"]["spec"]["containers"][0]["image"],
        "ghcr.io/beyond10x/loom-operator:0.0.1-test"
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
            default_command_resources: Some(Resources {
                cpu: "4".to_owned(),
                memory: "8Gi".to_owned(),
            }),
            model_route: Some(ModelRoute {
                url: "http://model-route.agents.svc:8080/v1".to_owned(),
                protocol: "messages".to_owned(),
                model: "small-model".to_owned(),
            }),
            allowed_secret_names: vec!["platform-operator-secrets".to_owned()],
            allowed_egress_hosts: vec!["chat.example".to_owned(), "models.example".to_owned()],
            allowed_claim_names: vec!["takt-workspace".to_owned()],
            allow_unconfined_profiles: true,
            allowed_seccomp_localhost_profiles: vec!["profiles/takt.json".to_owned()],
            allowed_app_armor_localhost_profiles: vec!["takt-delivery".to_owned()],
            allowed_boundary_upstreams: vec![
                AllowedUpstream {
                    kind: UpstreamKind::Model,
                    url: "http://llm-gateway.llm.svc:8080".to_owned(),
                    audience: "llm-gateway".to_owned(),
                    port: NonZeroU16::new(8000),
                },
                AllowedUpstream {
                    kind: UpstreamKind::Git,
                    url: "http://connectors.connectors.svc/git".to_owned(),
                    audience: "connectors-git".to_owned(),
                    port: None,
                },
            ],
        }
    );
}

/// `story:command-resources-declared`: the chart sizes a `Command` resource that declares no
/// resources for a coding agent, `defaultCommandResources: {cpu: "4", memory: 8Gi}`, and a values
/// override that unsets it renders no such member.
#[test]
fn the_chart_sets_the_command_default_and_an_override_unsets_it() {
    let text = |documents: &[Value]| {
        let map = kind(documents, "ConfigMap");
        serde_yaml_ng::from_str::<Value>(map["data"]["config.yaml"].as_str().expect("config.yaml"))
            .expect("config.yaml is YAML")
    };
    let set = text(&template(&[]));
    assert_eq!(set["defaultCommandResources"]["cpu"], "4");
    assert_eq!(set["defaultCommandResources"]["memory"], "8Gi");
    let unset = text(&template(&["defaultCommandResources=null"]));
    assert!(
        unset
            .as_mapping()
            .expect("a mapping")
            .get("defaultCommandResources")
            .is_none(),
        "{unset:?}"
    );
    config(&template(&["defaultCommandResources=null"]));
}

/// `story:command-resources-declared`: a `Command` resource with no resources, rendered under the
/// chart's own configuration, runs with 4 CPU and 8Gi.
#[test]
fn a_command_resource_without_resources_gets_the_charts_command_default() {
    let resource = "apiVersion: loom.beyond10x.dev/v1alpha1
kind: LoomAgent
metadata: {name: smoke, namespace: agents, uid: 00000000-0000-4000-8000-000000000003}
spec:
  routes: {}
  image: {repository: registry.example/smoke, tag: '1.0.0'}
  command: {args: [--once]}
";
    let rendered = render_document(resource, &config(&template(&[]))).expect("rendered");
    let workload: Value =
        serde_yaml_ng::from_str(rendered.split("\n---\n").nth(2).expect("the workload"))
            .expect("YAML");
    let container = &workload["spec"]["template"]["spec"]["containers"][0];
    for key in ["requests", "limits"] {
        assert_eq!(container["resources"][key]["cpu"], "4", "{key}");
        assert_eq!(container["resources"][key]["memory"], "8Gi", "{key}");
    }
}

/// The version a README line names after `marker`, up to the first character that cannot be part
/// of a version.
fn version_after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    let start = line.find(marker)? + marker.len();
    let rest = &line[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
        .unwrap_or(rest.len());
    Some(&rest[..end])
}

/// `story:chart-upgrade-guide-applies-crd-first`: Helm never upgrades `crds/`, so README's upgrade
/// guide applies the CRD of the release tag with `kubectl apply --server-side` before `helm
/// upgrade`, and every apply line names the workspace version, so a release cannot ship a guide
/// that applies an older CRD.
#[test]
fn the_upgrade_guide_applies_the_crd_of_the_workspace_version() {
    let readme = fs::read_to_string(root().join("README.md")).expect("README.md");
    let lines: Vec<&str> = readme.lines().collect();
    let tagged_crd = format!(
        "kubectl apply --server-side -f https://raw.githubusercontent.com/beyond10x/loom/{VERSION}/{}",
        b10x_loom_resource::crd::FILE
    );
    let apply = lines
        .iter()
        .position(|line| line.contains(&tagged_crd))
        .unwrap_or_else(|| panic!("README.md holds `{tagged_crd}`"));
    let upgrade = lines
        .iter()
        .position(|line| line.contains("helm upgrade loom-agents"))
        .expect("README.md holds `helm upgrade loom-agents`");
    assert!(apply < upgrade, "README applies the CRD before `helm upgrade`");
    assert_eq!(
        version_after(lines[upgrade], "--version "),
        Some(VERSION),
        "README's `helm upgrade` names the workspace version"
    );

    for line in lines.iter().filter(|line| line.contains("kubectl apply")) {
        let named = [
            version_after(line, "beyond10x/loom/"),
            version_after(line, "ghcr.io/beyond10x/loom-operator:"),
        ];
        assert!(
            named.iter().any(Option::is_some),
            "README's `{line}` names the CRD of a release"
        );
        for version in named.into_iter().flatten() {
            assert_eq!(version, VERSION, "README's `{line}` names the workspace version");
        }
    }
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
