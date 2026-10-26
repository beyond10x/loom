//! The `LoomAgent` resource as the controller reads and writes it on the wire: the kube type
//! over the CRD `loom-xtask crd-generate` projects from `ess/domains/resource.yaml`.
//!
//! The type carries no schema of its own (`schema = "disabled"`): the CRD's schema is the one
//! projected from the specification, and the cluster validates against it. `spec.agent` and
//! `spec.routes` are kept as JSON, since the controller hands both to the check and writes them
//! into the ConfigMap as given. The status members are the row of the view `LoomAgents`, spelt
//! as the CRD spells them.

use b10x_loom_resource::model::AgentVariant;
use kube::CustomResource;
use serde::{Deserialize, Serialize};

/// The `spec` of a `LoomAgent`.
#[derive(CustomResource, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[kube(
    group = "loom.beyond10x.dev",
    version = "v1alpha1",
    kind = "LoomAgent",
    plural = "loomagents",
    namespaced,
    status = "LoomAgentStatus",
    schema = "disabled",
    derive = "PartialEq"
)]
#[serde(rename_all = "camelCase")]
pub struct LoomAgentSpec {
    /// The `loom.agent/1` document, verbatim.
    pub agent: serde_json::Value,
    /// The `llm.routes/1` document, inline; an empty object asks for the controller's model
    /// route.
    pub routes: serde_json::Value,
    /// The connectors-local TOML, only for connectors bindings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connectors_config: Option<String>,
    /// The Secret whose keys become the agent container's environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<SecretRef>,
    /// The Secret whose keys become files in the agent container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_files: Option<SecretFiles>,
    /// The agent image, overriding the controller's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageRef>,
    /// The program the pod runs instead of `b10x-loom agent serve`; present, the resource is
    /// the `Command` variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<CommandSpec>,
    /// A writable volume for a repository checkout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<Workspace>,
}

impl LoomAgentSpec {
    /// What the pod runs: `Command` when `spec.command` is present, `AgentServe` otherwise.
    #[must_use]
    pub fn variant(&self) -> AgentVariant {
        if self.command.is_some() {
            AgentVariant::Command
        } else {
            AgentVariant::AgentServe
        }
    }
}

/// `spec.command`: the container's entrypoint and arguments, each left to the image when absent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    /// The entrypoint, Kubernetes's `command`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,
    /// The arguments, Kubernetes's `args`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
}

/// `spec.workspace`: an emptyDir, or the PersistentVolumeClaim `claimName`, at `mountPath`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    /// Where the volume is mounted, read-write.
    pub mount_path: String,
    /// The PersistentVolumeClaim, in the resource's namespace; absent, an emptyDir.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_name: Option<String>,
}

/// `spec.secretRef`: a Secret, by name, in the resource's namespace, whose keys become the
/// container's environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRef {
    /// The Secret's name.
    pub name: String,
}

/// `spec.secretFiles`: a Secret, by name, in the resource's namespace, whose keys are files in
/// `mountPath`, owned by the pod's user, the directory 0700 and each file 0600.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretFiles {
    /// The Secret's name.
    pub name: String,
    /// Where the Secret's keys are files, read-only.
    pub mount_path: String,
}

/// `spec.image`: an image repository and tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRef {
    /// The repository, registry host included.
    pub repository: String,
    /// The tag.
    pub tag: String,
}

/// The `status` of a `LoomAgent`: the controller's record of it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoomAgentStatus {
    /// The generation the controller last applied.
    pub observed_generation: i64,
    /// The lifecycle state: `Pending`, `Running`, `Refused`, `Degraded`, `Completed` or `Failed`.
    pub state: String,
    /// The spec check's result for `observedGeneration`, once it ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<CheckStatus>,
    /// What the controller last read of the child Deployment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment: Option<DeploymentStatus>,
    /// What the pod of the checked generation runs: `AgentServe` or `Command`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// How a `Command` resource's run ended, once it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunStatus>,
    /// The conditions `Checked` and `Available`, and `Completed` for a `Command` resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<Condition>>,
}

/// `status.check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckStatus {
    /// 0 serveable, 3 refused, as `b10x-loom agent check` exits.
    pub exit_code: i64,
    /// The check's line: what was checked, or the refusal naming its member.
    pub message: String,
}

/// `status.run`: the program container's exit code, when it ended and why, each absent when the
/// Job's pod did not say.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStatus {
    /// The program's exit code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    /// When the program ended, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    /// The terminated container's reason, or the Job's `Failed` reason when no container ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// `status.deployment`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentStatus {
    /// `Available` or `Unavailable`.
    pub availability: String,
}

/// One entry of `status.conditions`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    /// `Checked`, `Available` or `Completed`.
    #[serde(rename = "type")]
    pub kind: String,
    /// `True`, `False` or `Unknown`.
    pub status: String,
    /// Why, in one CamelCase word.
    pub reason: String,
    /// Why, as a person reads it.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use b10x_loom_resource::crd;
    use kube::Resource;

    use super::*;

    #[test]
    fn the_kube_type_is_the_projected_crds_group_version_kind_and_plural() {
        assert_eq!(LoomAgent::group(&()), crd::GROUP);
        assert_eq!(LoomAgent::version(&()), crd::VERSION);
        assert_eq!(LoomAgent::kind(&()), crd::KIND);
        assert_eq!(LoomAgent::plural(&()), crd::PLURAL);
        assert_eq!(
            LoomAgent::api_version(&()),
            format!("{}/{}", crd::GROUP, crd::VERSION)
        );
    }

    #[test]
    fn the_status_is_spelt_as_the_crd_spells_it() {
        let status = LoomAgentStatus {
            observed_generation: 2,
            state: "Running".to_owned(),
            check: Some(CheckStatus {
                exit_code: 0,
                message: "checked".to_owned(),
            }),
            deployment: Some(DeploymentStatus {
                availability: "Available".to_owned(),
            }),
            variant: Some("Command".to_owned()),
            run: Some(RunStatus {
                exit_code: Some(3),
                finished_at: Some("2026-10-26T12:00:00Z".to_owned()),
                reason: None,
            }),
            conditions: Some(vec![Condition {
                kind: "Checked".to_owned(),
                status: "True".to_owned(),
                reason: "SpecServeable".to_owned(),
                message: "checked".to_owned(),
            }]),
        };
        assert_eq!(
            serde_json::to_value(&status).expect("JSON"),
            serde_json::json!({
                "observedGeneration": 2,
                "state": "Running",
                "check": {"exitCode": 0, "message": "checked"},
                "deployment": {"availability": "Available"},
                "variant": "Command",
                "run": {"exitCode": 3, "finishedAt": "2026-10-26T12:00:00Z"},
                "conditions": [{
                    "type": "Checked", "status": "True", "reason": "SpecServeable",
                    "message": "checked",
                }],
            })
        );
    }
}
