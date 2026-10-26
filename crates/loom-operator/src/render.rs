//! The children of an admitted `LoomAgent`, as design § 4 draws its pod: a ServiceAccount, a
//! ConfigMap, a Deployment, a NetworkPolicy and, with `expose.a2a`, a Service. Rendering is
//! deterministic: the same resource and configuration give the same bytes.
//!
//! Every child carries the labels `app.kubernetes.io/name: loom-agent`,
//! `app.kubernetes.io/instance: <resource>` and `app.kubernetes.io/managed-by:
//! b10x-loom-operator`, and, when the resource has a uid, the resource as its controlling owner,
//! so deleting the resource deletes them.
//!
//! * ServiceAccount `deploy.kubernetes.service_account` (else the resource's name), with no
//!   token mounted.
//! * ConfigMap `<resource>-spec`: `agent.yaml`, `routes.yaml` and `connectors.toml`.
//! * Deployment `<resource>`: replicas 1, `Recreate`; the init container `check` runs
//!   `b10x-loom agent check --spec /etc/agent/agent.yaml`, the container `loom-agent` runs
//!   `b10x-loom agent serve --spec /etc/agent/agent.yaml --state /var/lib/agent` from
//!   `spec.image` or the configured default, with the environment of `spec.secretRef`, the
//!   ConfigMap read-only at `/etc/agent`, an emptyDir at `/var/lib/agent`, and with `expose.a2a`
//!   the port `a2a` and the probes `GET /ready` and `GET /live`; both run as user 10001 on a
//!   read-only root, no privilege escalation, no capability, the runtime's default seccomp
//!   profile. The pod template names the resource's generation, so a new spec replaces the pod.
//!
//!   The `Command` variant (`spec.command` present, design § 9a) differs only in its Deployment:
//!   no `check` init container, since the image need hold no `b10x-loom`; the container runs
//!   `spec.image` (else the default) with `command` = `spec.command.entrypoint` and `args` =
//!   `spec.command.args`, each left to the image when absent; the ConfigMap is read-only at
//!   `/config`, so `spec.agent` is `/config/agent.yaml`; `HOME` is `/var/lib/agent`, and an
//!   emptyDir is at `/tmp`, the root staying read-only.
//!
//!   On either variant `spec.workspace` adds a read-write volume at its `mountPath`: the claim
//!   `claimName`, else an emptyDir. `spec.secretFiles` adds its Secret as the read-only volume
//!   `secret-files-source` (mode 0440) and a memory emptyDir `secret-files`; the init container
//!   `stage-secret-files`, run first from the configured default image, runs `b10x-loom agent
//!   stage-files` to copy each key into `secret-files/files` (directory 0700, files 0600, owned
//!   by user 10001), and the agent container mounts that subdirectory read-only at
//!   `spec.secretFiles.mountPath`, so a reader that refuses a file another user owns or others
//!   may read accepts them. With either the pod's `fsGroup` is 10001.
//! * NetworkPolicy `<resource>`: deny by default; egress to DNS in `kube-system`, to each
//!   `deploy.kubernetes.egress` entry (a namespace entry as a namespace and pod selector peer, a
//!   host entry by its port only, since a NetworkPolicy names no host) and, with the default
//!   routes, to the model route's port; ingress from each `deploy.kubernetes.ingress` namespace.
//! * Service `<resource>` (ClusterIP, port `a2a`) only with `expose.a2a`.

use std::collections::BTreeMap;
use std::fmt;

use b10x_loom_resource::model::AgentVariant;
use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec, DeploymentStrategy};
use k8s_openapi::api::core::v1::{
    Capabilities, ConfigMap, ConfigMapVolumeSource, Container, ContainerPort, EmptyDirVolumeSource,
    EnvFromSource, EnvVar, HTTPGetAction, PersistentVolumeClaimVolumeSource, PodSecurityContext,
    PodSpec, PodTemplateSpec, Probe, ResourceRequirements, SeccompProfile, SecretEnvSource,
    SecretVolumeSource, SecurityContext, Service, ServiceAccount, ServicePort, ServiceSpec, Volume,
    VolumeMount,
};
use k8s_openapi::api::networking::v1::{
    NetworkPolicy, NetworkPolicyEgressRule, NetworkPolicyIngressRule, NetworkPolicyPeer,
    NetworkPolicyPort, NetworkPolicySpec,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta, OwnerReference};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::Resource;
use loom_protocols::ProtocolCatalog;
use loom_sdk::agent::{AgentSpec, KubernetesDeploy};
use serde::Serialize;

use crate::FIELD_MANAGER;
use crate::admission::{self, Admitted, Refusal};
use crate::config::Config;
use crate::resource::{CommandSpec, LoomAgent};

/// The user every container runs as.
pub const USER: i64 = 10001;
/// Where the ConfigMap is mounted.
pub const SPEC_DIR: &str = "/etc/agent";
/// The spec file the containers read.
pub const SPEC_FILE: &str = "/etc/agent/agent.yaml";
/// The agent's state directory.
pub const STATE_DIR: &str = "/var/lib/agent";
/// Where the `Command` variant's container reads the ConfigMap: `/config/agent.yaml` is `spec.agent`.
pub const CONFIG_DIR: &str = "/config";
/// The `Command` variant's writable temporary directory, an emptyDir.
pub const TMP_DIR: &str = "/tmp";
/// The mode of the `spec.secretFiles` Secret volume's files: read by the owner and by `fsGroup`,
/// so the staging init container, running as [`USER`] in that group, reads them.
pub const SECRET_FILE_MODE: i32 = 0o440;
/// The `spec.secretFiles` Secret volume, mounted only in the staging init container.
pub const SOURCE_VOLUME: &str = "secret-files-source";
/// Where the staging init container reads the Secret volume.
pub const SOURCE_DIR: &str = "/var/run/loom/secret-files-source";
/// The memory-backed emptyDir the Secret's keys are staged in.
pub const STAGED_VOLUME: &str = "secret-files";
/// Where the staging init container writes that emptyDir.
pub const STAGED_DIR: &str = "/var/run/loom/secret-files";
/// The directory in the emptyDir holding the staged files, mode 0700, owned by [`USER`]; the
/// agent container mounts it at `spec.secretFiles.mountPath`.
pub const STAGED_SUBDIR: &str = "files";
/// The name of the a2a port.
pub const A2A_PORT: &str = "a2a";
/// The annotation of the pod template naming the resource's generation.
pub const GENERATION_ANNOTATION: &str = "loom.beyond10x.dev/generation";
/// The label that names the namespace a peer stands in, which Kubernetes sets on every namespace.
const NAMESPACE_LABEL: &str = "kubernetes.io/metadata.name";

/// The children of one resource, in the order they are applied.
#[derive(Debug, Clone, PartialEq)]
pub struct Children {
    /// The pod's ServiceAccount.
    pub service_account: ServiceAccount,
    /// `<resource>-spec`.
    pub config_map: ConfigMap,
    /// The agent's Deployment.
    pub deployment: Deployment,
    /// The pod's NetworkPolicy.
    pub network_policy: NetworkPolicy,
    /// The a2a Service, only with `expose.a2a`.
    pub service: Option<Service>,
}

/// The name of the ConfigMap of the resource `name`.
#[must_use]
pub fn config_map_name(name: &str) -> String {
    format!("{name}-spec")
}

/// The name of the ServiceAccount of the resource `name` running `spec`.
#[must_use]
pub fn service_account_name(name: &str, spec: Option<&AgentSpec>) -> String {
    spec.and_then(kubernetes)
        .map_or_else(|| name.to_owned(), |k| k.service_account.clone())
}

fn kubernetes(spec: &AgentSpec) -> Option<&KubernetesDeploy> {
    spec.deploy
        .as_ref()
        .and_then(|deploy| deploy.kubernetes.as_ref())
}

fn labels(name: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("app.kubernetes.io/instance".to_owned(), name.to_owned()),
        (
            "app.kubernetes.io/managed-by".to_owned(),
            FIELD_MANAGER.to_owned(),
        ),
        ("app.kubernetes.io/name".to_owned(), "loom-agent".to_owned()),
    ])
}

fn selector(name: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("app.kubernetes.io/instance".to_owned(), name.to_owned()),
        ("app.kubernetes.io/name".to_owned(), "loom-agent".to_owned()),
    ])
}

fn port(number: i64) -> i32 {
    i32::try_from(number).unwrap_or_default()
}

fn tcp(number: i32) -> NetworkPolicyPort {
    NetworkPolicyPort {
        port: Some(IntOrString::Int(number)),
        protocol: Some("TCP".to_owned()),
        end_port: None,
    }
}

fn in_namespace(namespace: &str) -> LabelSelector {
    LabelSelector {
        match_labels: Some(BTreeMap::from([(
            NAMESPACE_LABEL.to_owned(),
            namespace.to_owned(),
        )])),
        match_expressions: None,
    }
}

struct Renderer<'a> {
    name: &'a str,
    namespace: &'a str,
    agent: &'a LoomAgent,
    admitted: &'a Admitted,
    config: &'a Config,
}

impl Renderer<'_> {
    fn meta(&self, name: String) -> ObjectMeta {
        ObjectMeta {
            name: Some(name),
            namespace: Some(self.namespace.to_owned()),
            labels: Some(labels(self.name)),
            owner_references: self.agent.controller_owner_ref(&()).map(|owner| {
                vec![OwnerReference {
                    block_owner_deletion: Some(true),
                    ..owner
                }]
            }),
            ..ObjectMeta::default()
        }
    }

    fn kubernetes(&self) -> Option<&KubernetesDeploy> {
        kubernetes(&self.admitted.spec)
    }

    fn service_account(&self) -> ServiceAccount {
        ServiceAccount {
            metadata: self.meta(service_account_name(self.name, Some(&self.admitted.spec))),
            automount_service_account_token: Some(false),
            ..ServiceAccount::default()
        }
    }

    fn config_map(&self) -> ConfigMap {
        ConfigMap {
            metadata: self.meta(config_map_name(self.name)),
            data: Some(BTreeMap::from([
                ("agent.yaml".to_owned(), self.admitted.agent.clone()),
                (
                    "connectors.toml".to_owned(),
                    self.admitted.connectors.clone(),
                ),
                ("routes.yaml".to_owned(), self.admitted.routes.clone()),
            ])),
            ..ConfigMap::default()
        }
    }

    fn image(&self) -> String {
        self.agent.spec.image.as_ref().map_or_else(
            || self.config.default_image.to_string(),
            |image| format!("{}:{}", image.repository, image.tag),
        )
    }

    fn resources(&self) -> ResourceRequirements {
        let (cpu, memory) = self.kubernetes().map_or_else(
            || {
                (
                    self.config.default_resources.cpu.clone(),
                    self.config.default_resources.memory.clone(),
                )
            },
            |k| (k.resources.cpu.clone(), k.resources.memory.clone()),
        );
        let quantities = BTreeMap::from([
            ("cpu".to_owned(), Quantity(cpu)),
            ("memory".to_owned(), Quantity(memory)),
        ]);
        ResourceRequirements {
            limits: Some(quantities.clone()),
            requests: Some(quantities),
            ..ResourceRequirements::default()
        }
    }

    fn security(&self) -> SecurityContext {
        SecurityContext {
            allow_privilege_escalation: Some(false),
            capabilities: Some(Capabilities {
                add: None,
                drop: Some(vec!["ALL".to_owned()]),
            }),
            read_only_root_filesystem: Some(true),
            run_as_non_root: Some(true),
            run_as_user: Some(USER),
            ..SecurityContext::default()
        }
    }

    fn command(&self) -> Option<&CommandSpec> {
        self.agent.spec.command.as_ref()
    }

    fn spec_mount(&self) -> VolumeMount {
        VolumeMount {
            name: "spec".to_owned(),
            mount_path: match self.admitted.variant {
                AgentVariant::AgentServe => SPEC_DIR,
                AgentVariant::Command => CONFIG_DIR,
            }
            .to_owned(),
            read_only: Some(true),
            ..VolumeMount::default()
        }
    }

    /// The mounts and volumes both variants may add: the workspace and the Secret's files.
    fn extra_mounts(&self) -> (Vec<VolumeMount>, Vec<Volume>) {
        let mut mounts = Vec::new();
        let mut volumes = Vec::new();
        if self.admitted.variant == AgentVariant::Command {
            mounts.push(VolumeMount {
                name: "tmp".to_owned(),
                mount_path: TMP_DIR.to_owned(),
                ..VolumeMount::default()
            });
            volumes.push(Volume {
                name: "tmp".to_owned(),
                empty_dir: Some(EmptyDirVolumeSource::default()),
                ..Volume::default()
            });
        }
        if let Some(workspace) = &self.agent.spec.workspace {
            mounts.push(VolumeMount {
                name: "workspace".to_owned(),
                mount_path: workspace.mount_path.clone(),
                ..VolumeMount::default()
            });
            volumes.push(Volume {
                name: "workspace".to_owned(),
                persistent_volume_claim: workspace.claim_name.as_ref().map(|claim| {
                    PersistentVolumeClaimVolumeSource {
                        claim_name: claim.clone(),
                        read_only: None,
                    }
                }),
                empty_dir: workspace
                    .claim_name
                    .is_none()
                    .then(EmptyDirVolumeSource::default),
                ..Volume::default()
            });
        }
        if let Some(files) = &self.agent.spec.secret_files {
            mounts.push(VolumeMount {
                name: STAGED_VOLUME.to_owned(),
                mount_path: files.mount_path.clone(),
                sub_path: Some(STAGED_SUBDIR.to_owned()),
                read_only: Some(true),
                ..VolumeMount::default()
            });
            volumes.push(Volume {
                name: SOURCE_VOLUME.to_owned(),
                secret: Some(SecretVolumeSource {
                    secret_name: Some(files.name.clone()),
                    default_mode: Some(SECRET_FILE_MODE),
                    ..SecretVolumeSource::default()
                }),
                ..Volume::default()
            });
            volumes.push(Volume {
                name: STAGED_VOLUME.to_owned(),
                empty_dir: Some(EmptyDirVolumeSource {
                    medium: Some("Memory".to_owned()),
                    size_limit: None,
                }),
                ..Volume::default()
            });
        }
        (mounts, volumes)
    }

    /// The init container that copies `spec.secretFiles` out of the Secret volume, whose files
    /// the kubelet owns, into a directory the pod's user owns: `b10x-loom agent stage-files`
    /// from the controller's default image, which carries `b10x-loom` whatever `spec.image` is.
    fn stage_files(&self) -> Option<Container> {
        self.agent.spec.secret_files.as_ref().map(|_| Container {
            name: "stage-secret-files".to_owned(),
            image: Some(self.config.default_image.to_string()),
            command: Some(
                [
                    "b10x-loom",
                    "agent",
                    "stage-files",
                    "--from",
                    SOURCE_DIR,
                    "--to",
                    &format!("{STAGED_DIR}/{STAGED_SUBDIR}"),
                ]
                .map(str::to_owned)
                .to_vec(),
            ),
            resources: Some(self.resources()),
            security_context: Some(self.security()),
            volume_mounts: Some(vec![
                VolumeMount {
                    name: SOURCE_VOLUME.to_owned(),
                    mount_path: SOURCE_DIR.to_owned(),
                    read_only: Some(true),
                    ..VolumeMount::default()
                },
                VolumeMount {
                    name: STAGED_VOLUME.to_owned(),
                    mount_path: STAGED_DIR.to_owned(),
                    ..VolumeMount::default()
                },
            ]),
            ..Container::default()
        })
    }

    /// Whether the pod mounts a workspace or a Secret's files, which `fsGroup` makes the
    /// container's user able to write and read.
    fn needs_fs_group(&self) -> bool {
        self.agent.spec.workspace.is_some() || self.agent.spec.secret_files.is_some()
    }

    fn probe(path: &str) -> Probe {
        Probe {
            http_get: Some(HTTPGetAction {
                path: Some(path.to_owned()),
                port: IntOrString::String(A2A_PORT.to_owned()),
                ..HTTPGetAction::default()
            }),
            ..Probe::default()
        }
    }

    fn deployment(&self) -> Deployment {
        let a2a = self
            .admitted
            .spec
            .expose
            .as_ref()
            .and_then(|expose| expose.a2a.as_ref());
        let check = Container {
            name: "check".to_owned(),
            image: Some(self.image()),
            command: Some(
                ["b10x-loom", "agent", "check", "--spec", SPEC_FILE]
                    .map(str::to_owned)
                    .to_vec(),
            ),
            resources: Some(self.resources()),
            security_context: Some(self.security()),
            volume_mounts: Some(vec![self.spec_mount()]),
            ..Container::default()
        };
        let (extra_mounts, extra_volumes) = self.extra_mounts();
        let (command, args) = match self.command() {
            None => (
                Some(
                    [
                        "b10x-loom",
                        "agent",
                        "serve",
                        "--spec",
                        SPEC_FILE,
                        "--state",
                        STATE_DIR,
                    ]
                    .map(str::to_owned)
                    .to_vec(),
                ),
                None,
            ),
            Some(command) => (command.entrypoint.clone(), command.args.clone()),
        };
        let mut mounts = vec![
            self.spec_mount(),
            VolumeMount {
                name: "state".to_owned(),
                mount_path: STATE_DIR.to_owned(),
                ..VolumeMount::default()
            },
        ];
        mounts.extend(extra_mounts);
        let serve = Container {
            name: "loom-agent".to_owned(),
            image: Some(self.image()),
            command,
            args,
            env: (self.admitted.variant == AgentVariant::Command).then(|| {
                vec![EnvVar {
                    name: "HOME".to_owned(),
                    value: Some(STATE_DIR.to_owned()),
                    value_from: None,
                }]
            }),
            env_from: self.agent.spec.secret_ref.as_ref().map(|secret| {
                vec![EnvFromSource {
                    secret_ref: Some(SecretEnvSource {
                        name: secret.name.clone(),
                        optional: None,
                    }),
                    ..EnvFromSource::default()
                }]
            }),
            ports: a2a.map(|a2a| {
                vec![ContainerPort {
                    name: Some(A2A_PORT.to_owned()),
                    container_port: port(a2a.port),
                    protocol: Some("TCP".to_owned()),
                    ..ContainerPort::default()
                }]
            }),
            readiness_probe: a2a.map(|_| Self::probe("/ready")),
            liveness_probe: a2a.map(|_| Self::probe("/live")),
            resources: Some(self.resources()),
            security_context: Some(self.security()),
            volume_mounts: Some(mounts),
            ..Container::default()
        };
        let annotations = self.agent.metadata.generation.map(|generation| {
            BTreeMap::from([(GENERATION_ANNOTATION.to_owned(), generation.to_string())])
        });
        Deployment {
            metadata: self.meta(self.name.to_owned()),
            spec: Some(DeploymentSpec {
                replicas: Some(1),
                selector: LabelSelector {
                    match_labels: Some(selector(self.name)),
                    match_expressions: None,
                },
                strategy: Some(DeploymentStrategy {
                    type_: Some("Recreate".to_owned()),
                    rolling_update: None,
                }),
                template: PodTemplateSpec {
                    metadata: Some(ObjectMeta {
                        labels: Some(labels(self.name)),
                        annotations,
                        ..ObjectMeta::default()
                    }),
                    spec: Some(PodSpec {
                        automount_service_account_token: Some(false),
                        service_account_name: Some(service_account_name(
                            self.name,
                            Some(&self.admitted.spec),
                        )),
                        security_context: Some(PodSecurityContext {
                            run_as_non_root: Some(true),
                            run_as_user: Some(USER),
                            seccomp_profile: Some(SeccompProfile {
                                type_: "RuntimeDefault".to_owned(),
                                localhost_profile: None,
                            }),
                            fs_group: self.needs_fs_group().then_some(USER),
                            ..PodSecurityContext::default()
                        }),
                        init_containers: {
                            let init: Vec<Container> = self
                                .stage_files()
                                .into_iter()
                                .chain(
                                    (self.admitted.variant == AgentVariant::AgentServe)
                                        .then_some(check),
                                )
                                .collect();
                            (!init.is_empty()).then_some(init)
                        },
                        containers: vec![serve],
                        volumes: Some(
                            [
                                Volume {
                                    name: "spec".to_owned(),
                                    config_map: Some(ConfigMapVolumeSource {
                                        name: config_map_name(self.name),
                                        ..ConfigMapVolumeSource::default()
                                    }),
                                    ..Volume::default()
                                },
                                Volume {
                                    name: "state".to_owned(),
                                    empty_dir: Some(EmptyDirVolumeSource::default()),
                                    ..Volume::default()
                                },
                            ]
                            .into_iter()
                            .chain(extra_volumes)
                            .collect(),
                        ),
                        ..PodSpec::default()
                    }),
                },
                ..DeploymentSpec::default()
            }),
            ..Deployment::default()
        }
    }

    fn network_policy(&self) -> NetworkPolicy {
        let mut egress = vec![NetworkPolicyEgressRule {
            to: Some(vec![NetworkPolicyPeer {
                namespace_selector: Some(in_namespace("kube-system")),
                ..NetworkPolicyPeer::default()
            }]),
            ports: Some(vec![
                NetworkPolicyPort {
                    port: Some(IntOrString::Int(53)),
                    protocol: Some("UDP".to_owned()),
                    end_port: None,
                },
                tcp(53),
            ]),
        }];
        let mut ingress = Vec::new();
        if let Some(kubernetes) = self.kubernetes() {
            for entry in &kubernetes.egress {
                let to = entry.namespace.as_ref().map(|namespace| {
                    vec![NetworkPolicyPeer {
                        namespace_selector: Some(in_namespace(namespace)),
                        pod_selector: (!entry.selector.is_empty()).then(|| LabelSelector {
                            match_labels: Some(
                                entry
                                    .selector
                                    .iter()
                                    .map(|label| (label.name.clone(), label.value.clone()))
                                    .collect(),
                            ),
                            match_expressions: None,
                        }),
                        ..NetworkPolicyPeer::default()
                    }]
                });
                egress.push(NetworkPolicyEgressRule {
                    to,
                    ports: Some(vec![tcp(port(entry.port))]),
                });
            }
            for entry in &kubernetes.ingress {
                ingress.push(NetworkPolicyIngressRule {
                    from: Some(vec![NetworkPolicyPeer {
                        namespace_selector: Some(in_namespace(&entry.from_namespace)),
                        ..NetworkPolicyPeer::default()
                    }]),
                    ports: Some(vec![tcp(port(entry.port))]),
                });
            }
        }
        if self.admitted.default_routes
            && let Some(route) = &self.config.model_route
        {
            egress.push(NetworkPolicyEgressRule {
                to: None,
                ports: Some(vec![tcp(i32::from(route.port()))]),
            });
        }
        NetworkPolicy {
            metadata: self.meta(self.name.to_owned()),
            spec: Some(NetworkPolicySpec {
                pod_selector: Some(LabelSelector {
                    match_labels: Some(selector(self.name)),
                    match_expressions: None,
                }),
                policy_types: Some(vec!["Ingress".to_owned(), "Egress".to_owned()]),
                egress: Some(egress),
                ingress: Some(ingress),
            }),
        }
    }

    fn service(&self) -> Option<Service> {
        let a2a = self.admitted.spec.expose.as_ref()?.a2a.as_ref()?;
        Some(Service {
            metadata: self.meta(self.name.to_owned()),
            spec: Some(ServiceSpec {
                type_: Some("ClusterIP".to_owned()),
                selector: Some(selector(self.name)),
                ports: Some(vec![ServicePort {
                    name: Some(A2A_PORT.to_owned()),
                    port: port(a2a.port),
                    protocol: Some("TCP".to_owned()),
                    target_port: Some(IntOrString::String(A2A_PORT.to_owned())),
                    ..ServicePort::default()
                }]),
                ..ServiceSpec::default()
            }),
            ..Service::default()
        })
    }
}

/// The children of `agent`, a resource named `name` in `namespace`, admitted as `admitted`,
/// under `config`.
#[must_use]
pub fn children(
    agent: &LoomAgent,
    name: &str,
    namespace: &str,
    admitted: &Admitted,
    config: &Config,
) -> Children {
    let renderer = Renderer {
        name,
        namespace,
        agent,
        admitted,
        config,
    };
    Children {
        service_account: renderer.service_account(),
        config_map: renderer.config_map(),
        deployment: renderer.deployment(),
        network_policy: renderer.network_policy(),
        service: renderer.service(),
    }
}

fn document(out: &mut String, object: &impl Serialize) {
    if !out.is_empty() {
        out.push_str("---\n");
    }
    out.push_str(&serde_yaml_ng::to_string(object).unwrap_or_default());
}

/// `children` as one multi-document YAML, in the order they are applied.
#[must_use]
pub fn to_yaml(children: &Children) -> String {
    let mut out = String::new();
    document(&mut out, &children.service_account);
    document(&mut out, &children.config_map);
    document(&mut out, &children.deployment);
    document(&mut out, &children.network_policy);
    if let Some(service) = &children.service {
        document(&mut out, service);
    }
    out
}

/// Why a document was not rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// The document is not one `LoomAgent`, with the reason.
    NotALoomAgent(String),
    /// The plugin catalog the check reads cannot be loaded.
    Catalog(String),
    /// The resource is refused.
    Refused(Refusal),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotALoomAgent(why) => write!(f, "not a LoomAgent: {why}"),
            Self::Catalog(why) => write!(f, "the plugin catalog: {why}"),
            Self::Refused(refusal) => f.write_str(&refusal.message()),
        }
    }
}

impl std::error::Error for RenderError {}

/// Decodes `text` as one `LoomAgent` document.
///
/// # Errors
/// [`RenderError::NotALoomAgent`] when it is not YAML, names another `apiVersion` or `kind`, has
/// no `metadata.name`, or does not decode.
pub fn parse_resource(text: &str) -> Result<LoomAgent, RenderError> {
    let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(text)
        .map_err(|error| RenderError::NotALoomAgent(error.to_string()))?;
    let api_version = LoomAgent::api_version(&());
    if value["apiVersion"].as_str() != Some(api_version.as_ref()) {
        return Err(RenderError::NotALoomAgent(format!(
            "apiVersion is not {api_version}"
        )));
    }
    if value["kind"].as_str() != Some(LoomAgent::kind(&()).as_ref()) {
        return Err(RenderError::NotALoomAgent(
            "kind is not LoomAgent".to_owned(),
        ));
    }
    let agent: LoomAgent = serde_yaml_ng::from_value(value)
        .map_err(|error| RenderError::NotALoomAgent(error.to_string()))?;
    if agent.metadata.name.as_deref().is_none_or(str::is_empty) {
        return Err(RenderError::NotALoomAgent(
            "metadata.name is missing".to_owned(),
        ));
    }
    Ok(agent)
}

/// The children of the `LoomAgent` document `text` under `config`, as one multi-document YAML.
/// The resource's namespace is its `metadata.namespace`, else the configuration's.
///
/// # Errors
/// [`RenderError`]: the document is not a `LoomAgent`, the catalog cannot be loaded, or the
/// resource is refused.
pub fn render_document(text: &str, config: &Config) -> Result<String, RenderError> {
    let agent = parse_resource(text)?;
    let name = agent.metadata.name.clone().unwrap_or_default();
    let namespace = agent
        .metadata
        .namespace
        .clone()
        .unwrap_or_else(|| config.namespace.clone());
    let catalog = ProtocolCatalog::plugins().map_err(RenderError::Catalog)?;
    let actions = |protocol: &str| catalog.get(protocol).map(|entry| entry.actions());
    let admitted = admission::admit(&agent.spec, &namespace, config, &actions)
        .map_err(RenderError::Refused)?;
    Ok(to_yaml(&children(
        &agent, &name, &namespace, &admitted, config,
    )))
}
