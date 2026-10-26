#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! `b10x-loom-operator`: the Kubernetes controller of the `LoomAgent` resource.
//!
//! It watches `LoomAgent` in one namespace and reconciles each resource into one pod: a
//! ServiceAccount, a ConfigMap (`agent.yaml`, `routes.yaml`, `connectors.toml`), a Deployment
//! running `b10x-loom agent serve` behind the init container `b10x-loom agent check`, a
//! NetworkPolicy and, with `expose.a2a`, a Service, each owned by the resource. A resource with
//! `spec.command` is the `Command` variant (design § 9a): its pod runs the image's own program,
//! with no init container, the spec at `/config/agent.yaml`, and, on either variant, a workspace
//! volume and the Secret as files when the resource asks for them. Before any child
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
            agent: serde_json::json!({}),
            routes: serde_json::json!({}),
            connectors_config: Some(String::new()),
            secret_ref: Some(resource::SecretRef {
                name: "s".to_owned(),
            }),
            secret_files: Some(resource::SecretFiles {
                name: "f".to_owned(),
                mount_path: "/m".to_owned(),
            }),
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
                "state",
                "variant"
            ]
        );
    }
}
