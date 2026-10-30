//! Security review of unit u1 of wave 2026-10-30-w1 (`story:boundary-container-projected-token`,
//! `story:credential-free-network-policy`, design D243 § 2 and § 4): integrity checks of a
//! credential-free pod against the contract the unit states. Nothing is applied and nothing
//! reaches a cluster.

use std::fs;
use std::path::PathBuf;

use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::{RenderError, render_document};
use serde_json::json;
use serde_yaml_ng::Value;

/// The model gateway a credential-free resource declares.
const MODEL: &str = "http://llm-gateway.llm.svc:8080";
/// The Connectors instance.
const CONNECTORS: &str = "http://connectors.connectors.svc:8080";
/// Its git smart-HTTP path.
const GIT: &str = "http://connectors.connectors.svc/git";

/// The controller configuration: the three upstreams allowed, each for its relay with the
/// audiences the cases declare (its own, and `llm-gateway` for the plain-audience case), the
/// workspace claim allowed.
fn config() -> Config {
    Config::parse(&format!(
        "namespace: agents
defaultImage: {{repository: registry.example/loom-agent, tag: '0.40.0'}}
defaultResources: {{cpu: 100m, memory: 128Mi}}
modelRoute: {{url: 'http://model-route.agents.svc:8080/v1', protocol: messages, model: small-model}}
allowedClaimNames: [takt-workspace]
allowedBoundaryUpstreams:
- {{kind: model, url: '{MODEL}', audience: llm-gateway}}
- {{kind: connectors, url: '{CONNECTORS}', audience: connectors}}
- {{kind: connectors, url: '{CONNECTORS}', audience: llm-gateway}}
- {{kind: git, url: '{GIT}', audience: connectors-git}}
- {{kind: git, url: '{GIT}', audience: llm-gateway}}
"
    ))
    .expect(
        "the test configuration (story:boundary-container-projected-token: an \
         allowedBoundaryUpstreams entry is a kind, a URL and an audience)",
    )
}

/// `spec.boundary` declaring the upstream `kind` alone, its token audience `audience`.
fn one_upstream(kind: &str, audience: &str) -> serde_json::Value {
    match kind {
        "model" => json!({"model": {"upstream": MODEL, "audience": audience}}),
        "connectors" => json!({"connectors": {
            "upstream": CONNECTORS, "audience": audience, "operations": ["merge_request.get"],
        }}),
        "git" => json!({"git": {
            "upstream": GIT, "audience": audience, "repositories": ["agents/takt-target"],
        }}),
        other => panic!("no upstream kind {other}"),
    }
}

/// `spec.boundary` declaring every upstream, each with its own audience.
fn every_upstream() -> serde_json::Value {
    json!({
        "model": {"upstream": MODEL, "audience": "llm-gateway"},
        "connectors": {
            "upstream": CONNECTORS, "audience": "connectors",
            "operations": ["merge_request.create", "merge_request.get"],
        },
        "git": {
            "upstream": GIT, "audience": "connectors-git",
            "repositories": ["agents/takt-target"],
        },
    })
}

/// A credential-free `Command` resource without `spec.agent`, with `boundary`, and a workspace
/// claim when `workspace` (which gives the pod an `fsGroup`), as JSON, which reads as YAML.
fn command(boundary: &serde_json::Value, workspace: bool) -> String {
    let mut spec = json!({
        "routes": {},
        "image": {"repository": "registry.example/takt", "tag": "0.12.0"},
        "command": {"args": ["--iterations", "1"]},
        "boundary": boundary,
    });
    if workspace {
        spec["workspace"] = json!({"mountPath": "/workspace", "claimName": "takt-workspace"});
    }
    resource("takt-delivery", spec)
}

/// The operator example without its egress entries (host and namespace entries are refused
/// beside a boundary), as the `AgentServe` resource `platform-operator` with `boundary`, and a
/// workspace claim when `workspace`.
fn serve(boundary: &serde_json::Value, workspace: bool) -> String {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/agents/operator.yaml");
    let text = fs::read_to_string(path).expect("the example spec");
    let mut agent: serde_json::Value = serde_yaml_ng::from_str(&text).expect("the example is YAML");
    agent["deploy"]["kubernetes"]["egress"]
        .as_array_mut()
        .expect(
            "egress entries (story:credential-free-network-policy: beside a boundary a host or \
             namespace entry is refused, so none stays)",
        )
        .clear();
    let mut spec = json!({"agent": agent, "routes": {}, "boundary": boundary});
    if workspace {
        spec["workspace"] = json!({"mountPath": "/workspace", "claimName": "takt-workspace"});
    }
    resource("platform-operator", spec)
}

fn resource(name: &str, spec: serde_json::Value) -> String {
    json!({
        "apiVersion": "loom.beyond10x.dev/v1alpha1",
        "kind": "LoomAgent",
        "metadata": {
            "name": name,
            "namespace": "agents",
            "uid": "00000000-0000-4000-8000-000000000001",
            "generation": 1,
        },
        "spec": spec,
    })
    .to_string()
}

fn documents(rendered: &str) -> Vec<Value> {
    rendered
        .split("\n---\n")
        .map(|document| serde_yaml_ng::from_str(document).expect("each document is YAML"))
        .collect()
}

/// The pod template of the rendered Deployment or Job.
fn pod_of(docs: &[Value]) -> Value {
    docs.iter()
        .find(|doc| matches!(doc["kind"].as_str(), Some("Deployment" | "Job")))
        .expect("a workload")["spec"]["template"]["spec"]
        .clone()
}

/// Every container of `pod`, init containers (native sidecars included) first, with its name.
fn containers(pod: &Value) -> Vec<&Value> {
    ["initContainers", "containers"]
        .iter()
        .flat_map(|list| pod[*list].as_sequence().into_iter().flatten())
        .collect()
}

fn name(container: &Value) -> &str {
    container["name"].as_str().expect("a container name")
}

/// The member `key` of `container`'s security context, else the pod's.
fn effective(pod: &Value, container: &Value, key: &str) -> Option<i64> {
    container["securityContext"][key]
        .as_i64()
        .or_else(|| pod["securityContext"][key].as_i64())
}

/// The owner, group and mode the kubelet gives a projected `serviceAccountToken` file of the
/// volume `volume` in `pod`, read from Kubernetes:
///
/// * `FsUser` (`pkg/volume/util.FsUserFrom`) is the effective `runAsUser` every init container
///   and container shares, and none when two differ;
/// * the token's mode (`pkg/volume/projected`, `collectData`) is 0600 when `FsUser` or `fsGroup`
///   is set, else the volume's `defaultMode` (the API server defaults it to 0644), owned by
///   `FsUser` or else root;
/// * with `fsGroup`, `SetVolumeOwnership` (`pkg/volume/volume_linux.go`) gives the file that
///   group and adds the read-only mask 0440: 0640.
fn token_file(pod: &Value, volume: &Value) -> (i64, i64, u32) {
    let users: Vec<Option<i64>> = containers(pod)
        .into_iter()
        .map(|container| effective(pod, container, "runAsUser"))
        .collect();
    let fs_user = match users.first() {
        Some(Some(first)) if users.iter().all(|user| *user == Some(*first)) => Some(*first),
        _ => None,
    };
    let fs_group = pod["securityContext"]["fsGroup"].as_i64();
    let default_mode = volume["projected"]["defaultMode"]
        .as_u64()
        .map_or(0o644, |mode| u32::try_from(mode).expect("a mode"));
    let mut mode = if fs_user.is_some() || fs_group.is_some() {
        0o600
    } else {
        default_mode
    };
    if fs_group.is_some() {
        mode |= 0o440;
    }
    (fs_user.unwrap_or(0), fs_group.unwrap_or(0), mode)
}

/// Whether a process of `uid` in `groups` may read a file of `owner`, `group` and `mode`.
fn readable((owner, group, mode): (i64, i64, u32), uid: i64, groups: &[i64]) -> bool {
    if uid == owner {
        mode & 0o400 != 0
    } else if groups.contains(&group) {
        mode & 0o040 != 0
    } else {
        mode & 0o004 != 0
    }
}

/// Design D243 § 2: "The pod holds only a 10-minute token. That token is audience-bound ... and
/// is useless outside this cluster's network path", and decision A1 ("audience per upstream").
/// A token whose audience is the API server's is a Kubernetes API credential for the pod's
/// service account, valid wherever the API server is reachable, and the boundary sends it to the
/// upstream on every call. Admission takes the audience from the resource author and checks only
/// that it is one word (`admission.rs` `boundary`, the audience check); nothing ties it to the
/// allowed upstream. The kind and kubeadm default issuer and API audience is
/// `https://kubernetes.default.svc.cluster.local`; `https://kubernetes.default.svc` is the other
/// common one. A credential-free resource naming either must be refused at that audience.
#[test]
fn sec_an_audience_the_api_server_accepts_is_refused() {
    for kind in ["model", "connectors", "git"] {
        render_document(
            &command(&one_upstream(kind, "llm-gateway"), false),
            &config(),
        )
        .unwrap_or_else(|error| panic!("{kind} with a plain audience renders: {error}"));
    }
    for audience in [
        "https://kubernetes.default.svc.cluster.local",
        "https://kubernetes.default.svc",
    ] {
        for kind in ["model", "connectors", "git"] {
            match render_document(&command(&one_upstream(kind, audience), false), &config()) {
                Err(RenderError::Refused(refusal)) => {
                    let message = refusal.message();
                    assert!(
                        message.contains(&format!(" at spec.boundary.{kind}.audience: ")),
                        "{kind} {audience}: {message}"
                    );
                }
                Ok(rendered) => {
                    let projected: Vec<String> = documents(&rendered)
                        .iter()
                        .flat_map(|doc| {
                            doc["spec"]["template"]["spec"]["volumes"]
                                .as_sequence()
                                .into_iter()
                                .flatten()
                                .flat_map(|volume| {
                                    volume["projected"]["sources"]
                                        .as_sequence()
                                        .into_iter()
                                        .flatten()
                                })
                                .filter_map(|source| {
                                    source["serviceAccountToken"]["audience"]
                                        .as_str()
                                        .map(str::to_owned)
                                })
                                .collect::<Vec<_>>()
                        })
                        .collect();
                    panic!(
                        "admitted: spec.boundary.{kind}.audience {audience} renders the projected \
                         token audiences {projected:?}, so the boundary holds an API-server \
                         token and sends it to {kind}'s upstream"
                    );
                }
                Err(other) => panic!("{kind} {audience}: neither rendered nor refused: {other}"),
            }
        }
    }
}

/// Invariant 1 of the review, held against Kubernetes' own rules for a projected token file
/// (see [`token_file`]): on either variant, with and without a pod `fsGroup`, the token is a file
/// the boundary's user can read, in a volume only the boundary mounts, in a pod that shares no
/// process, IPC or host namespace (so no `/proc/<pid>/root` path leads into the boundary's
/// mounts), and no container reaches a Secret or the pod's own token another way.
#[test]
fn sec_the_token_is_readable_by_the_boundary_and_reachable_by_no_other_container() {
    let mut with_fs_group = Vec::new();
    for (label, resource) in [
        ("command", command(&every_upstream(), false)),
        ("command with a workspace", command(&every_upstream(), true)),
        ("serve", serve(&every_upstream(), false)),
        ("serve with a workspace", serve(&every_upstream(), true)),
        (
            "command, git only",
            command(&one_upstream("git", "connectors-git"), true),
        ),
    ] {
        let rendered = render_document(&resource, &config())
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        let docs = documents(&rendered);
        let pod = pod_of(&docs);

        for key in ["shareProcessNamespace", "hostPID", "hostIPC", "hostNetwork"] {
            assert!(
                pod[key].is_null() || pod[key] == Value::Bool(false),
                "{label}: {key} is {:?}",
                pod[key]
            );
        }
        assert_eq!(
            pod["automountServiceAccountToken"],
            Value::Bool(false),
            "{label}"
        );
        let account = docs
            .iter()
            .find(|doc| doc["kind"].as_str() == Some("ServiceAccount"))
            .expect("a ServiceAccount");
        assert_eq!(
            account["automountServiceAccountToken"],
            Value::Bool(false),
            "{label}"
        );

        let volumes = pod["volumes"].as_sequence().expect("volumes");
        let tokens = volumes
            .iter()
            .find(|volume| volume["name"].as_str() == Some("boundary-tokens"))
            .unwrap_or_else(|| panic!("{label}: the token volume"));
        assert!(
            tokens["projected"]["defaultMode"].is_null(),
            "{label}: the token mode is the kubelet's, not widened by defaultMode"
        );
        for volume in volumes {
            assert!(
                volume["secret"].is_null(),
                "{label}: a Secret volume {volume:?}"
            );
            for source in volume["projected"]["sources"]
                .as_sequence()
                .into_iter()
                .flatten()
            {
                assert!(
                    source["serviceAccountToken"].is_mapping()
                        && source.as_mapping().map(serde_yaml_ng::Mapping::len) == Some(1),
                    "{label}: a projected source other than a token {source:?}"
                );
            }
        }
        let projected: Vec<&str> = volumes
            .iter()
            .filter(|volume| volume["projected"].is_mapping())
            .filter_map(|volume| volume["name"].as_str())
            .collect();
        assert_eq!(
            projected,
            ["boundary-tokens"],
            "{label}: one projected volume"
        );

        let mounting: Vec<&str> = containers(&pod)
            .into_iter()
            .filter(|container| {
                container["volumeMounts"]
                    .as_sequence()
                    .into_iter()
                    .flatten()
                    .any(|mount| mount["name"].as_str() == Some("boundary-tokens"))
            })
            .map(name)
            .collect();
        assert_eq!(mounting, ["boundary"], "{label}: who mounts the tokens");
        for container in containers(&pod) {
            assert!(
                container["envFrom"].is_null(),
                "{label}: envFrom on {}",
                name(container)
            );
            for entry in container["env"].as_sequence().into_iter().flatten() {
                assert!(
                    entry["valueFrom"].is_null(),
                    "{label}: {} reads {:?} from elsewhere",
                    name(container),
                    entry["name"]
                );
            }
        }

        let boundary = containers(&pod)
            .into_iter()
            .find(|container| name(container) == "boundary")
            .expect("the boundary");
        let uid = effective(&pod, boundary, "runAsUser").expect("the boundary's user");
        assert_eq!(uid, 10002, "{label}");
        for other in containers(&pod) {
            if name(other) != "boundary" {
                assert_ne!(
                    effective(&pod, other, "runAsUser"),
                    Some(uid),
                    "{label}: {} shares the boundary's user",
                    name(other)
                );
            }
        }
        let fs_group = pod["securityContext"]["fsGroup"].as_i64();
        with_fs_group.push(fs_group.is_some());
        let mut groups = vec![effective(&pod, boundary, "runAsGroup").expect("its group")];
        groups.extend(fs_group);
        let file = token_file(&pod, tokens);
        assert_eq!(
            file,
            if fs_group.is_some() {
                (0, 10001, 0o640)
            } else {
                (0, 0, 0o644)
            },
            "{label}: mixed users leave FsUser unset, so root owns the token; with fsGroup the \
             agent's group 10001 may read it, and only the missing mount keeps uid 10001 out"
        );
        let (owner, group, mode) = file;
        assert!(
            readable(file, uid, &groups),
            "{label}: the boundary (uid {uid}, groups {groups:?}) cannot read its token \
             (owner {owner}, group {group}, mode {mode:o})"
        );
    }
    assert!(
        with_fs_group.contains(&true) && with_fs_group.contains(&false),
        "both ownership branches are exercised: {with_fs_group:?}"
    );
}
