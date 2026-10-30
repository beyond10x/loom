//! The children of an admitted `LoomAgent`, as design § 4 draws its pod: a ServiceAccount, a
//! ConfigMap, a Deployment (a Job for the `Command` variant), a NetworkPolicy and, with
//! `expose.a2a`, a Service. Rendering is deterministic: the same resource and configuration give
//! the same bytes.
//!
//! Every child carries the labels `app.kubernetes.io/name: loom-agent`,
//! `app.kubernetes.io/instance: <resource>` and `app.kubernetes.io/managed-by:
//! b10x-loom-operator`, and, when the resource has a uid, the resource as its controlling owner,
//! so deleting the resource deletes them.
//!
//! * ServiceAccount `deploy.kubernetes.service_account` (else the resource's name), with no
//!   token mounted.
//! * ConfigMap `<resource>-spec`: `agent.yaml` (absent for a `Command` resource without
//!   `spec.agent`), `routes.yaml`, `connectors.toml` and, for a `Command` resource, each of
//!   `spec.configFiles` by its name, its content verbatim.
//! * Deployment `<resource>`: replicas 1, `Recreate`; the init container `check` runs
//!   `b10x-loom agent check --spec /etc/agent/agent.yaml`, the container `loom-agent` runs
//!   `b10x-loom agent serve --spec /etc/agent/agent.yaml --state /var/lib/agent` from
//!   `spec.image` or the configured default, with the environment of `spec.secretRef`, the
//!   ConfigMap read-only at `/etc/agent`, an emptyDir at `/var/lib/agent`, and with `expose.a2a`
//!   the port `a2a` and the probes `GET /ready` and `GET /live`; both run as user 10001 on a
//!   read-only root, no privilege escalation, no capability, the runtime's default seccomp
//!   profile. The pod template names the resource's generation, so a new spec replaces the pod.
//!
//!   The `Command` variant (`spec.command` present, design § 9a) runs its pod once, as the Job
//!   `<resource>` (`backoffLimit: 0`, pod `restartPolicy: Never`, no `ttlSecondsAfterFinished`,
//!   story command-variant-runs-once) in place of the Deployment, so a run that ends is not
//!   started again. Its pod template differs from the serve variant's only in that and in this:
//!   no `check` init container, since the image need hold no `b10x-loom`; the container runs
//!   `spec.image` (else the default) with `command` = `spec.command.entrypoint` and `args` =
//!   `spec.command.args`, each left to the image when absent; the ConfigMap is read-only at
//!   `/config`, so `spec.agent` is `/config/agent.yaml` and each of `spec.configFiles` is
//!   `/config/<name>`; `HOME` is `/var/lib/agent`, and an emptyDir is at `/tmp`, the root
//!   staying read-only. Both are directories the init container `prepare-writable-dirs`, run from
//!   the configured default image, makes with `b10x-loom agent prepare-dirs` in the emptyDirs
//!   `tmp` and `state` (`tmp/tmp`, `state/home`; owned by user 10001, mode 0700, setgid and
//!   sticky cleared), mounted by `subPath`, since Kubernetes makes an emptyDir's root 0777
//!   (setgid with `fsGroup`) and a reader that refuses a file under a directory others may write
//!   would refuse every file there.
//!
//!   A `Command` resource may declare `spec.seccompProfile` and `spec.appArmorProfile`, each
//!   `RuntimeDefault`, `Localhost` with a `localhostProfile` name, or `Unconfined`; the program's
//!   container carries them in its own `securityContext`, overriding the pod's `RuntimeDefault`
//!   seccomp profile, and the init containers keep the pod's. The controller's configuration
//!   decides which it admits ([`crate::admission`] step 16); a resource that declares none renders
//!   as before.
//!
//!   On either variant `spec.env` is the agent container's `env` after the controller's own
//!   ([`CONTROLLER_ENV`]), in declaration order, beside the `spec.secretRef` environment; no init
//!   container gets it. `spec.workspace` adds a read-write volume at its `mountPath`: the claim
//!   `claimName`, else an emptyDir. `spec.secretFiles` adds its Secret as the read-only volume
//!   `secret-files-source` (mode 0440) and a memory emptyDir `secret-files`; the init container
//!   `stage-secret-files`, run first from the configured default image, runs `b10x-loom agent
//!   stage-files` to copy each key into `secret-files/files` (directory 0700, files 0600, owned
//!   by user 10001), and the agent container mounts that subdirectory read-only at
//!   `spec.secretFiles.mountPath`, so a reader that refuses a file another user owns or others
//!   may read accepts them. With either the pod's `fsGroup` is 10001.
//!
//!   With `spec.boundary` (story boundary-container-projected-token) no container the agent runs
//!   in can read a credential. The init container `boundary`, last and a native sidecar
//!   (`restartPolicy: Always`, so it runs beside the agent and a Job still completes), runs
//!   `b10x-loom agent boundary` from the configured default image, never `spec.image`: for a
//!   declared `model`, `--model-upstream <URL> --model-token <tokens>/model`; for `connectors`,
//!   `--connectors-upstream <URL> --connectors-token <tokens>/connectors` and an `--operation`
//!   per operation id in declared order; for `git`, `--git-upstream <URL> --git-token
//!   <tokens>/git` and a `--repository` per repository, `<tokens>` being
//!   [`BOUNDARY_TOKENS_DIR`]. It runs as user and group [`BOUNDARY_USER`] on a read-only root,
//!   no privilege escalation, no capability, the runtime's default seccomp profile, its requests
//!   equal to its limits ([`BOUNDARY_CPU`], [`BOUNDARY_MEMORY`]) and no environment; its startup
//!   probe runs `b10x-loom agent boundary-ready` with a `--port` per declared relay ([`RELAYS`]:
//!   model 8401, connectors 8402, git 8403) every second, 30 times at most, so the containers
//!   after it start once every relay listens. Its only mount, read-only at
//!   [`BOUNDARY_TOKENS_DIR`], is the projected volume `boundary-tokens`: one service-account
//!   token per declared upstream, its audience the declared one, valid
//!   [`BOUNDARY_TOKEN_SECONDS`] seconds, at `model`, `connectors` or `git`; no other container
//!   mounts it and the pod's own token stays unmounted. The agent container gets each declared
//!   relay's URL (`LOOM_BOUNDARY_MODEL_URL`, `LOOM_BOUNDARY_CONNECTORS_URL`,
//!   `LOOM_BOUNDARY_GIT_URL`) as the controller's own environment, before `spec.env`, and with
//!   `spec.boundary.model` the default routes name the model relay. A resource without
//!   `spec.boundary` renders as before.
//! * NetworkPolicy `<resource>`: deny by default; egress to DNS in `kube-system`, to each
//!   `deploy.kubernetes.egress` entry (a namespace entry as a namespace and pod selector peer, a
//!   host entry by its port only, since a NetworkPolicy names no host) and, with the default
//!   routes, to the model route's port; ingress from each `deploy.kubernetes.ingress` namespace.
//!   With `spec.boundary` (story credential-free-network-policy) the egress is DNS, the
//!   `deploy.kubernetes.egress` namespace entries (admission refuses a host entry) and, per
//!   declared upstream, the namespace its host names on its TCP port (80 when it names none);
//!   no rule to the model route.
//! * Service `<resource>` (ClusterIP, port `a2a`) only with `expose.a2a`.

use std::collections::BTreeMap;
use std::fmt;

use b10x_loom_resource::model::AgentVariant;
use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec, DeploymentStrategy};
use k8s_openapi::api::batch::v1::{Job, JobSpec};
use k8s_openapi::api::core::v1::{
    AppArmorProfile, Capabilities, ConfigMap, ConfigMapVolumeSource, Container, ContainerPort,
    EmptyDirVolumeSource, EnvFromSource, EnvVar, ExecAction, HTTPGetAction,
    PersistentVolumeClaimVolumeSource, PodSecurityContext, PodSpec, PodTemplateSpec, Probe,
    ProjectedVolumeSource, ResourceRequirements, SeccompProfile, SecretEnvSource,
    SecretVolumeSource, SecurityContext, Service, ServiceAccount, ServiceAccountTokenProjection,
    ServicePort, ServiceSpec, Volume, VolumeMount, VolumeProjection,
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
use crate::resource::{Boundary, CommandSpec, LoomAgent};

/// The user every container runs as.
pub const USER: i64 = 10001;
/// The name of the container that runs the agent or, for a `Command` resource, its program.
pub const CONTAINER: &str = "loom-agent";
/// Where the ConfigMap is mounted.
pub const SPEC_DIR: &str = "/etc/agent";
/// The spec file the containers read.
pub const SPEC_FILE: &str = "/etc/agent/agent.yaml";
/// The agent's state directory.
pub const STATE_DIR: &str = "/var/lib/agent";
/// Where the `Command` variant's container reads the ConfigMap: `/config/agent.yaml` is
/// `spec.agent`, `/config/<name>` each of `spec.configFiles`.
pub const CONFIG_DIR: &str = "/config";
/// The `Command` variant's writable temporary directory: [`TMP_SUBDIR`] of the emptyDir `tmp`.
pub const TMP_DIR: &str = "/tmp";
/// The directory in the emptyDir `tmp` the `Command` variant's container mounts at [`TMP_DIR`],
/// made by the init container `prepare-writable-dirs`: mode 0700, owned by [`USER`].
pub const TMP_SUBDIR: &str = "tmp";
/// The directory in the emptyDir `state` the `Command` variant's container mounts at
/// [`STATE_DIR`], its `HOME`, made as [`TMP_SUBDIR`] is.
pub const HOME_SUBDIR: &str = "home";
/// Where `prepare-writable-dirs` mounts the emptyDir `tmp`.
pub const TMP_VOLUME_DIR: &str = "/var/run/loom/tmp";
/// Where `prepare-writable-dirs` mounts the emptyDir `state`.
pub const STATE_VOLUME_DIR: &str = "/var/run/loom/state";
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
/// The variable naming the `Command` variant's home directory, [`STATE_DIR`].
pub const HOME: &str = "HOME";
/// Every environment variable the controller sets on a container of either variant; a
/// `spec.env` entry may name none of them, whether or not the resource declares a boundary.
pub const CONTROLLER_ENV: [&str; 4] = [
    HOME,
    RELAYS[0].variable,
    RELAYS[1].variable,
    RELAYS[2].variable,
];

/// The name of the container that holds a credential-free pod's only credential: an init
/// container with `restartPolicy: Always` (a Kubernetes native sidecar), so a `Command`
/// resource's Job still completes when its program ends.
pub const BOUNDARY_CONTAINER: &str = "boundary";
/// The user and group the boundary runs as: another than the agent's [`USER`].
pub const BOUNDARY_USER: i64 = 10002;
/// The projected volume holding one service-account token per declared upstream, mounted in the
/// boundary container alone.
pub const BOUNDARY_TOKENS_VOLUME: &str = "boundary-tokens";
/// Where the boundary reads its tokens, each at `<dir>/<model|connectors|git>`.
pub const BOUNDARY_TOKENS_DIR: &str = "/var/run/loom/boundary/tokens";
/// How long a projected token is valid, in seconds; the kubelet renews it before it expires.
pub const BOUNDARY_TOKEN_SECONDS: i64 = 600;
/// The boundary container's CPU, requests and limits alike.
pub const BOUNDARY_CPU: &str = "100m";
/// The boundary container's memory, requests and limits alike.
pub const BOUNDARY_MEMORY: &str = "64Mi";
/// How often the kubelet runs the boundary's startup probe, in seconds.
pub const BOUNDARY_PROBE_PERIOD_SECONDS: i32 = 1;
/// How many failed startup probes the kubelet accepts before it restarts the boundary.
pub const BOUNDARY_PROBE_FAILURES: i32 = 30;

/// One kind of upstream the boundary relays to: its name (the token file's, the member's under
/// `spec.boundary` and the prefix of its flags), the loopback port the boundary listens on, and
/// the variable naming that URL in the agent container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relay {
    /// `model`, `connectors` or `git`.
    pub kind: &'static str,
    /// The loopback port the boundary listens on for it.
    pub port: u16,
    /// The variable the agent container reads the loopback URL from.
    pub variable: &'static str,
    /// The loopback URL the agent calls.
    pub url: &'static str,
}

/// The relays, in the order a boundary's upstreams are rendered: the model, Connectors, git.
pub const RELAYS: [Relay; 3] = [
    Relay {
        kind: "model",
        port: 8401,
        variable: "LOOM_BOUNDARY_MODEL_URL",
        url: "http://127.0.0.1:8401/v1",
    },
    Relay {
        kind: "connectors",
        port: 8402,
        variable: "LOOM_BOUNDARY_CONNECTORS_URL",
        url: "http://127.0.0.1:8402",
    },
    Relay {
        kind: "git",
        port: 8403,
        variable: "LOOM_BOUNDARY_GIT_URL",
        url: "http://127.0.0.1:8403",
    },
];

/// What one declared upstream admits besides its URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admits<'a> {
    /// The model gateway: every call.
    Everything,
    /// The Connectors instance: these operation ids, each passed as `--operation`.
    Operations(&'a [String]),
    /// The git upstream: these `<owner>/<repo>`, each passed as `--repository`.
    Repositories(&'a [String]),
}

/// One upstream a `spec.boundary` declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Upstream<'a> {
    /// Its relay in the boundary.
    pub relay: Relay,
    /// Its base URL, a cluster service.
    pub url: &'a str,
    /// The audience of its projected token.
    pub audience: &'a str,
    /// What the boundary admits on it.
    pub admits: Admits<'a>,
}

/// The upstreams `boundary` declares, in the order of [`RELAYS`].
#[must_use]
pub fn upstreams(boundary: &Boundary) -> Vec<Upstream<'_>> {
    let [model, connectors, git] = RELAYS;
    let mut out = Vec::new();
    if let Some(declared) = &boundary.model {
        out.push(Upstream {
            relay: model,
            url: &declared.upstream,
            audience: &declared.audience,
            admits: Admits::Everything,
        });
    }
    if let Some(declared) = &boundary.connectors {
        out.push(Upstream {
            relay: connectors,
            url: &declared.upstream,
            audience: &declared.audience,
            admits: Admits::Operations(&declared.operations),
        });
    }
    if let Some(declared) = &boundary.git {
        out.push(Upstream {
            relay: git,
            url: &declared.upstream,
            audience: &declared.audience,
            admits: Admits::Repositories(&declared.repositories),
        });
    }
    out
}

/// The environment the controller sets on the agent container of `variant`, each name and
/// value, in the order the container receives them, before `spec.env`: `HOME` for `Command`,
/// then the loopback URL of each upstream `boundary` declares.
#[must_use]
pub fn controller_env(
    variant: AgentVariant,
    boundary: Option<&Boundary>,
) -> Vec<(&'static str, &'static str)> {
    let mut env = match variant {
        AgentVariant::AgentServe => Vec::new(),
        AgentVariant::Command => vec![(HOME, STATE_DIR)],
    };
    env.extend(
        boundary
            .map(upstreams)
            .unwrap_or_default()
            .into_iter()
            .map(|upstream| (upstream.relay.variable, upstream.relay.url)),
    );
    env
}
/// The label that names the namespace a peer stands in, which Kubernetes sets on every namespace.
const NAMESPACE_LABEL: &str = "kubernetes.io/metadata.name";

/// What runs the pod: a Deployment for `AgentServe`, a Job that runs once for `Command`.
#[derive(Debug, Clone, PartialEq)]
pub enum Workload {
    /// The `AgentServe` variant's Deployment, which keeps its pod running.
    Deployment(Deployment),
    /// The `Command` variant's Job, which runs its pod to completion once.
    Job(Job),
}

impl Workload {
    /// The workload's name, the resource's.
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::Deployment(deployment) => deployment.metadata.name.clone(),
            Self::Job(job) => job.metadata.name.clone(),
        }
        .unwrap_or_default()
    }
}

/// The children of one resource, in the order they are applied.
#[derive(Debug, Clone, PartialEq)]
pub struct Children {
    /// The pod's ServiceAccount.
    pub service_account: ServiceAccount,
    /// `<resource>-spec`.
    pub config_map: ConfigMap,
    /// The agent's Deployment or Job.
    pub workload: Workload,
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
        self.admitted.spec.as_ref().and_then(kubernetes)
    }

    fn service_account(&self) -> ServiceAccount {
        ServiceAccount {
            metadata: self.meta(service_account_name(self.name, self.admitted.spec.as_ref())),
            automount_service_account_token: Some(false),
            ..ServiceAccount::default()
        }
    }

    fn config_map(&self) -> ConfigMap {
        ConfigMap {
            metadata: self.meta(config_map_name(self.name)),
            data: Some(self.admitted.config_map_data()),
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

    /// The security context of the container that runs the agent or the program: [`Self::security`]
    /// and, for a `Command` resource, the seccomp and AppArmor profiles it declares. The init
    /// containers keep the pod's `RuntimeDefault`, and so does every resource that declares none.
    fn program_security(&self) -> SecurityContext {
        let profile = |declared: &crate::resource::ConfinementProfile| {
            (declared.kind.clone(), declared.localhost_profile.clone())
        };
        SecurityContext {
            seccomp_profile: self.agent.spec.seccomp_profile.as_ref().map(|declared| {
                let (type_, localhost_profile) = profile(declared);
                SeccompProfile {
                    type_,
                    localhost_profile,
                }
            }),
            app_armor_profile: self.agent.spec.app_armor_profile.as_ref().map(|declared| {
                let (type_, localhost_profile) = profile(declared);
                AppArmorProfile {
                    type_,
                    localhost_profile,
                }
            }),
            ..self.security()
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
                sub_path: Some(TMP_SUBDIR.to_owned()),
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

    /// The upstreams `spec.boundary` declares; none without one.
    fn boundary_upstreams(&self) -> Vec<Upstream<'_>> {
        self.agent
            .spec
            .boundary
            .as_ref()
            .map(upstreams)
            .unwrap_or_default()
    }

    /// The container that holds a credential-free pod's only credential, when `spec.boundary`
    /// declares an upstream: a native sidecar (an init container with `restartPolicy: Always`)
    /// running `b10x-loom agent boundary` from the controller's default image as
    /// [`BOUNDARY_USER`], which relays the agent's loopback calls to each declared upstream with
    /// its projected token. It mounts the token volume alone, read-only, and gets no
    /// environment; its startup probe holds the agent back until every relay listens.
    fn boundary(&self) -> Option<Container> {
        let declared = self.boundary_upstreams();
        if declared.is_empty() {
            return None;
        }
        let mut args = vec!["agent".to_owned(), "boundary".to_owned()];
        let mut ready = ["b10x-loom", "agent", "boundary-ready"]
            .map(str::to_owned)
            .to_vec();
        for upstream in &declared {
            let kind = upstream.relay.kind;
            args.extend([
                format!("--{kind}-upstream"),
                upstream.url.to_owned(),
                format!("--{kind}-token"),
                format!("{BOUNDARY_TOKENS_DIR}/{kind}"),
            ]);
            let (flag, admitted): (&str, &[String]) = match upstream.admits {
                Admits::Everything => ("", &[]),
                Admits::Operations(operations) => ("--operation", operations),
                Admits::Repositories(repositories) => ("--repository", repositories),
            };
            for value in admitted {
                args.extend([flag.to_owned(), value.clone()]);
            }
            ready.extend(["--port".to_owned(), upstream.relay.port.to_string()]);
        }
        let quantities = BTreeMap::from([
            ("cpu".to_owned(), Quantity(BOUNDARY_CPU.to_owned())),
            ("memory".to_owned(), Quantity(BOUNDARY_MEMORY.to_owned())),
        ]);
        Some(Container {
            name: BOUNDARY_CONTAINER.to_owned(),
            image: Some(self.config.default_image.to_string()),
            command: Some(vec!["b10x-loom".to_owned()]),
            args: Some(args),
            restart_policy: Some("Always".to_owned()),
            startup_probe: Some(Probe {
                exec: Some(ExecAction {
                    command: Some(ready),
                }),
                period_seconds: Some(BOUNDARY_PROBE_PERIOD_SECONDS),
                failure_threshold: Some(BOUNDARY_PROBE_FAILURES),
                ..Probe::default()
            }),
            resources: Some(ResourceRequirements {
                limits: Some(quantities.clone()),
                requests: Some(quantities),
                ..ResourceRequirements::default()
            }),
            security_context: Some(SecurityContext {
                run_as_user: Some(BOUNDARY_USER),
                run_as_group: Some(BOUNDARY_USER),
                seccomp_profile: Some(SeccompProfile {
                    type_: "RuntimeDefault".to_owned(),
                    localhost_profile: None,
                }),
                ..self.security()
            }),
            volume_mounts: Some(vec![VolumeMount {
                name: BOUNDARY_TOKENS_VOLUME.to_owned(),
                mount_path: BOUNDARY_TOKENS_DIR.to_owned(),
                read_only: Some(true),
                ..VolumeMount::default()
            }]),
            ..Container::default()
        })
    }

    /// The projected volume of the boundary's tokens: one service-account token per declared
    /// upstream, its audience the declared one, valid [`BOUNDARY_TOKEN_SECONDS`], at the
    /// upstream's kind.
    fn boundary_volume(&self) -> Option<Volume> {
        let declared = self.boundary_upstreams();
        (!declared.is_empty()).then(|| Volume {
            name: BOUNDARY_TOKENS_VOLUME.to_owned(),
            projected: Some(ProjectedVolumeSource {
                default_mode: None,
                sources: Some(
                    declared
                        .iter()
                        .map(|upstream| VolumeProjection {
                            service_account_token: Some(ServiceAccountTokenProjection {
                                audience: Some(upstream.audience.to_owned()),
                                expiration_seconds: Some(BOUNDARY_TOKEN_SECONDS),
                                path: upstream.relay.kind.to_owned(),
                            }),
                            ..VolumeProjection::default()
                        })
                        .collect(),
                ),
            }),
            ..Volume::default()
        })
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

    /// The `Command` variant's init container that makes [`TMP_SUBDIR`] and [`HOME_SUBDIR`] in
    /// the emptyDirs `tmp` and `state` as directories the pod's user owns with mode 0700:
    /// Kubernetes makes an emptyDir's root mode 0777 (setgid with `fsGroup`), which a non-root
    /// container cannot change. `b10x-loom agent prepare-dirs` from the controller's default
    /// image, as [`Self::stage_files`] runs.
    fn prepare_dirs(&self) -> Option<Container> {
        (self.admitted.variant == AgentVariant::Command).then(|| Container {
            name: "prepare-writable-dirs".to_owned(),
            image: Some(self.config.default_image.to_string()),
            command: Some(
                [
                    "b10x-loom",
                    "agent",
                    "prepare-dirs",
                    "--dir",
                    &format!("{TMP_VOLUME_DIR}/{TMP_SUBDIR}"),
                    "--dir",
                    &format!("{STATE_VOLUME_DIR}/{HOME_SUBDIR}"),
                ]
                .map(str::to_owned)
                .to_vec(),
            ),
            resources: Some(self.resources()),
            security_context: Some(self.security()),
            volume_mounts: Some(vec![
                VolumeMount {
                    name: "tmp".to_owned(),
                    mount_path: TMP_VOLUME_DIR.to_owned(),
                    ..VolumeMount::default()
                },
                VolumeMount {
                    name: "state".to_owned(),
                    mount_path: STATE_VOLUME_DIR.to_owned(),
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

    /// The agent container's environment: the controller's own ([`CONTROLLER_ENV`]; `HOME` for
    /// the `Command` variant, then the boundary's loopback URL of each declared upstream), then
    /// `spec.env` in declaration order; none when both are empty. The init containers get none
    /// of it: they run `b10x-loom` for the controller, not the agent.
    fn env(&self) -> Option<Vec<EnvVar>> {
        let env: Vec<EnvVar> =
            controller_env(self.admitted.variant, self.agent.spec.boundary.as_ref())
                .into_iter()
                .map(|(name, value)| EnvVar {
                    name: name.to_owned(),
                    value: Some(value.to_owned()),
                    value_from: None,
                })
                .chain(self.agent.spec.env.iter().flatten().map(|entry| EnvVar {
                    name: entry.name.clone(),
                    value: Some(entry.value.clone()),
                    value_from: None,
                }))
                .collect();
        (!env.is_empty()).then_some(env)
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

    /// The pod either variant runs: the Deployment's template for `AgentServe`, the Job's for
    /// `Command`, which alone sets `restartPolicy: Never`.
    fn pod_template(&self) -> PodTemplateSpec {
        let a2a = self
            .admitted
            .spec
            .as_ref()
            .and_then(|spec| spec.expose.as_ref())
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
                sub_path: (self.admitted.variant == AgentVariant::Command)
                    .then(|| HOME_SUBDIR.to_owned()),
                ..VolumeMount::default()
            },
        ];
        mounts.extend(extra_mounts);
        let serve = Container {
            name: CONTAINER.to_owned(),
            image: Some(self.image()),
            command,
            args,
            env: self.env(),
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
            security_context: Some(self.program_security()),
            volume_mounts: Some(mounts),
            ..Container::default()
        };
        let annotations = self.agent.metadata.generation.map(|generation| {
            BTreeMap::from([(GENERATION_ANNOTATION.to_owned(), generation.to_string())])
        });
        PodTemplateSpec {
            metadata: Some(ObjectMeta {
                labels: Some(labels(self.name)),
                annotations,
                ..ObjectMeta::default()
            }),
            spec: Some(PodSpec {
                automount_service_account_token: Some(false),
                service_account_name: Some(service_account_name(
                    self.name,
                    self.admitted.spec.as_ref(),
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
                        .chain(self.prepare_dirs())
                        .chain((self.admitted.variant == AgentVariant::AgentServe).then_some(check))
                        .chain(self.boundary())
                        .collect();
                    (!init.is_empty()).then_some(init)
                },
                containers: vec![serve],
                restart_policy: (self.admitted.variant == AgentVariant::Command)
                    .then(|| "Never".to_owned()),
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
                    .chain(self.boundary_volume())
                    .collect(),
                ),
                ..PodSpec::default()
            }),
        }
    }

    /// The `AgentServe` variant's Deployment: one replica, replaced whole on a new generation.
    fn deployment(&self) -> Deployment {
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
                template: self.pod_template(),
                ..DeploymentSpec::default()
            }),
            ..Deployment::default()
        }
    }

    /// The `Command` variant's Job: its pod runs once, `backoffLimit: 0` retries no failed pod,
    /// and no `ttlSecondsAfterFinished` removes it, so the run's end stays readable. Kubernetes
    /// generates its selector.
    fn job(&self) -> Job {
        Job {
            metadata: self.meta(self.name.to_owned()),
            spec: Some(JobSpec {
                backoff_limit: Some(0),
                template: self.pod_template(),
                ..JobSpec::default()
            }),
            ..Job::default()
        }
    }

    fn workload(&self) -> Workload {
        match self.admitted.variant {
            AgentVariant::AgentServe => Workload::Deployment(self.deployment()),
            AgentVariant::Command => Workload::Job(self.job()),
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
        let upstreams = self.boundary_upstreams();
        if upstreams.is_empty() {
            if self.admitted.default_routes
                && let Some(route) = &self.config.model_route
            {
                egress.push(NetworkPolicyEgressRule {
                    to: None,
                    ports: Some(vec![tcp(i32::from(route.port()))]),
                });
            }
        } else {
            for service in upstreams
                .iter()
                .filter_map(|upstream| admission::cluster_service(upstream.url).ok())
            {
                egress.push(NetworkPolicyEgressRule {
                    to: Some(vec![NetworkPolicyPeer {
                        namespace_selector: Some(in_namespace(service.namespace)),
                        ..NetworkPolicyPeer::default()
                    }]),
                    ports: Some(vec![tcp(i32::from(service.port))]),
                });
            }
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
        let a2a = self.admitted.spec.as_ref()?.expose.as_ref()?.a2a.as_ref()?;
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
        workload: renderer.workload(),
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
    match &children.workload {
        Workload::Deployment(deployment) => document(&mut out, deployment),
        Workload::Job(job) => document(&mut out, job),
    }
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
