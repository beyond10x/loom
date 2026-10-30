#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! `b10x-loom-operator`: the Kubernetes controller of the `LoomAgent` resource.
//!
//! It watches `LoomAgent` in one namespace and reconciles each resource into one pod: a
//! ServiceAccount, a ConfigMap (`agent.yaml`, `routes.yaml`, `connectors.toml`), a Deployment
//! running `b10x-loom agent serve` behind the init container `b10x-loom agent check`, a
//! NetworkPolicy and, with `expose.a2a`, a Service, each owned by the resource. A resource with
//! `spec.command` is the `Command` variant (design § 9a): its pod is a `batch/v1` Job, run once,
//! running the image's own program with no init container and the spec at `/config/agent.yaml`;
//! a finished Job is not started again before a new generation, and its status reports how the
//! run ended (`status.run`: the program's exit code and end time, read from the Job's pod, and
//! the condition `Completed`). On either variant the pod has a
//! workspace volume and the Secret as files when the resource asks for them; with `spec.boundary`
//! it holds no Secret, and the native sidecar `boundary` alone mounts its credentials, projected
//! service-account tokens, and relays the agent's calls to the declared upstreams. Before any child
//! is rendered the spec is checked in-process (`b10x_loom_agent::check_text`, the library behind
//! `b10x-loom agent check`), `spec.routes` is walked for a credential and read by llm's strict
//! routes parser, and `spec.connectorsConfig` is decoded as TOML and walked, word by word and
//! over its raw text too; a refusal is the resource's status and no child exists for it. The
//! lifecycle is the generated behaviour of `loom.resource` over
//! `b10x_loom_resource::LoomAgentRecord`; its row is the resource's status.
//!
//! * [`cli`]: the command line, `run`, `render` and `crd`.
//! * [`config`]: the controller's `config.yaml`.
//! * [`resource`]: the `LoomAgent` kube type.
//! * [`admission`]: the checks before any child.
//! * [`render`]: the children.
//! * [`status`]: a row of `LoomAgents` as the status, conditions included.
//! * [`reconcile`]: one reconcile, and the controller loop.

pub mod admission;
pub mod cli;
pub mod config;
pub mod reconcile;
pub mod render;
pub mod resource;
pub mod status;

/// The field manager of every server-side apply, and the `app.kubernetes.io/managed-by` label of
/// every child.
pub const FIELD_MANAGER: &str = "b10x-loom-operator";

/// The `LoomAgent` CustomResourceDefinition, the bytes of
/// `deploy/crd/loomagents.loom.beyond10x.dev.yaml`, which `loom-xtask crd-generate` projects
/// from `ess/`.
pub const CRD: &str = include_str!("../../../deploy/crd/loomagents.loom.beyond10x.dev.yaml");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crd_names_the_spec_members_the_kube_type_reads() {
        let crd: serde_yaml_ng::Value = serde_yaml_ng::from_str(CRD).expect("the CRD is YAML");
        let spec = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]["properties"];
        let mut members: Vec<&str> = spec
            .as_mapping()
            .expect("spec properties")
            .keys()
            .map(|key| key.as_str().expect("a name"))
            .collect();
        members.sort_unstable();
        let full = resource::LoomAgentSpec {
            agent: Some(serde_json::json!({})),
            routes: serde_json::json!({}),
            connectors_config: Some(String::new()),
            secret_ref: Some(resource::SecretRef {
                name: "s".to_owned(),
            }),
            secret_files: Some(resource::SecretFiles {
                name: "f".to_owned(),
                mount_path: "/m".to_owned(),
            }),
            env: Some(vec![resource::EnvVar {
                name: "N".to_owned(),
                value: "v".to_owned(),
            }]),
            image: Some(resource::ImageRef {
                repository: "r".to_owned(),
                tag: "t".to_owned(),
            }),
            command: Some(resource::CommandSpec {
                entrypoint: Some(Vec::new()),
                args: Some(Vec::new()),
            }),
            workspace: Some(resource::Workspace {
                mount_path: "/w".to_owned(),
                claim_name: Some("c".to_owned()),
            }),
            config_files: Some(vec![resource::ConfigFile {
                name: "n".to_owned(),
                content: String::new(),
            }]),
            seccomp_profile: Some(resource::ConfinementProfile {
                kind: "Localhost".to_owned(),
                localhost_profile: Some("p".to_owned()),
            }),
            app_armor_profile: Some(resource::ConfinementProfile {
                kind: "RuntimeDefault".to_owned(),
                localhost_profile: None,
            }),
            boundary: Some(full_boundary()),
        };
        let written = serde_json::to_value(full).expect("JSON");
        let mut keys: Vec<&str> = written
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, members);
    }

    /// A `spec.boundary` declaring every upstream.
    fn full_boundary() -> resource::Boundary {
        resource::Boundary {
            model: Some(resource::BoundaryUpstream {
                upstream: "http://m.n.svc".to_owned(),
                audience: "m".to_owned(),
            }),
            connectors: Some(resource::ConnectorsBoundary {
                upstream: "http://c.n.svc".to_owned(),
                audience: "c".to_owned(),
                operations: vec!["o".to_owned()],
            }),
            git: Some(resource::GitBoundary {
                upstream: "http://g.n.svc".to_owned(),
                audience: "g".to_owned(),
                repositories: vec!["o/r".to_owned()],
            }),
        }
    }

    /// The members of the object schema `schema`, sorted.
    fn properties(schema: &serde_yaml_ng::Value) -> Vec<String> {
        let mut members: Vec<String> = schema["properties"]
            .as_mapping()
            .expect("properties")
            .keys()
            .map(|key| key.as_str().expect("a name").to_owned())
            .collect();
        members.sort_unstable();
        members
    }

    /// The members of the JSON object `value`, sorted.
    fn keys(value: &serde_json::Value) -> Vec<String> {
        let mut keys: Vec<String> = value
            .as_object()
            .expect("an object")
            .keys()
            .cloned()
            .collect();
        keys.sort_unstable();
        keys
    }

    #[test]
    fn the_crd_names_the_boundary_members_the_kube_type_reads() {
        let crd: serde_yaml_ng::Value = serde_yaml_ng::from_str(CRD).expect("the CRD is YAML");
        let schema = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
            ["properties"]["boundary"];
        let written = serde_json::to_value(full_boundary()).expect("JSON");
        assert_eq!(keys(&written), properties(schema));
        for upstream in ["model", "connectors", "git"] {
            assert_eq!(
                keys(&written[upstream]),
                properties(&schema["properties"][upstream]),
                "spec.boundary.{upstream}"
            );
        }
        assert_eq!(
            serde_json::from_value::<resource::Boundary>(serde_json::json!({}))
                .expect("every upstream is optional"),
            resource::Boundary::default()
        );
    }

    #[test]
    fn the_crd_names_the_status_members_the_controller_writes() {
        let crd: serde_yaml_ng::Value = serde_yaml_ng::from_str(CRD).expect("the CRD is YAML");
        let status = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["status"]
            ["properties"];
        let mut members: Vec<&str> = status
            .as_mapping()
            .expect("status properties")
            .keys()
            .map(|key| key.as_str().expect("a name"))
            .collect();
        members.sort_unstable();
        assert_eq!(
            members,
            [
                "check",
                "conditions",
                "deployment",
                "observedGeneration",
                "run",
                "state",
                "variant"
            ]
        );
    }
}
