//! The `LoomAgent` resource as the controller reads and writes it on the wire: the kube type
//! over the CRD `loom-xtask crd-generate` projects from `ess/domains/resource.yaml`.
//!
//! The type carries no schema of its own (`schema = "disabled"`): the CRD's schema is the one
//! projected from the specification, and the cluster validates against it. `spec.agent` and
//! `spec.routes` are kept as JSON, since the controller hands both to the check and writes them
//! into the ConfigMap as given. The status members are the row of the view `LoomAgents`, spelt
//! as the CRD spells them.

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
    /// The agent image, overriding the controller's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageRef>,
}

/// `spec.secretRef`: a Secret, by name, in the resource's namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRef {
    /// The Secret's name.
    pub name: String,
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
    /// The lifecycle state: `Pending`, `Running`, `Refused` or `Degraded`.
    pub state: String,
    /// The spec check's result for `observedGeneration`, once it ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<CheckStatus>,
    /// What the controller last read of the child Deployment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment: Option<DeploymentStatus>,
    /// The conditions `Checked` and `Available`.
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

/// `status.deployment`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentStatus {
    /// `Available` or `Unavailable`.
    pub availability: String,
}

/// One entry of `status.conditions`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    /// `Checked` or `Available`.
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
                "conditions": [{
                    "type": "Checked", "status": "True", "reason": "SpecServeable",
                    "message": "checked",
                }],
            })
        );
    }
}
