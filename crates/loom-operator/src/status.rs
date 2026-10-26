//! A row of the view `loom.resource.LoomAgents` as the resource's status, and the conditions the
//! controller computes for it.
//!
//! No `loom.resource` command sets `conditions` (ESS 0.69.0 admits no list under `sets:`), so the
//! controller projects them from the row's `state`, `check`, `deployment` and `variant`:
//!
//! | state | `Checked` | `Available` |
//! |---|---|---|
//! | Pending | Unknown, `CheckPending` | Unknown, `CheckPending` |
//! | Running, `AgentServe` | True, `SpecServeable` | True, `DeploymentAvailable`; Unknown, `DeploymentNotObserved` before the Deployment is read |
//! | Running, `Command` | True, `SpecAdmitted` | Unknown, `RunNotFinished` |
//! | Degraded | True, `SpecServeable` | False, `DeploymentUnavailable` |
//! | Completed | True, `SpecAdmitted` | False, `RunSucceeded` |
//! | Failed | True, `SpecAdmitted` | False, `RunFailed` |
//! | Refused | False, `SpecRefused` | False, `SpecRefused` |
//!
//! A `Command` resource's pod is a Job that runs once, so its `Available` reads the Job's run:
//! not finished while Running, False once the run ended, since nothing runs any more and nothing
//! starts again before a new generation. Only an `AgentServe` resource reaches Degraded, since
//! only its Deployment is observed for availability. A row that carries conditions is published
//! with its own.

use b10x_loom_resource::model::{
    AgentVariant, CheckResult, Condition, ConditionStatus, ConditionType, DeploymentAvailability,
    DeploymentStatus, LoomAgentState, LoomAgents,
};

use crate::resource;

/// The state as the CRD's `status.state` spells it.
#[must_use]
pub fn state(state: LoomAgentState) -> &'static str {
    match state {
        LoomAgentState::Pending => "Pending",
        LoomAgentState::Running => "Running",
        LoomAgentState::Refused => "Refused",
        LoomAgentState::Degraded => "Degraded",
        LoomAgentState::Completed => "Completed",
        LoomAgentState::Failed => "Failed",
    }
}

/// The availability as the CRD's `status.deployment.availability` spells it.
#[must_use]
pub fn availability(availability: DeploymentAvailability) -> &'static str {
    match availability {
        DeploymentAvailability::Available => "Available",
        DeploymentAvailability::Unavailable => "Unavailable",
    }
}

/// The variant as the CRD's `status.variant` spells it.
#[must_use]
pub fn variant_name(variant: AgentVariant) -> &'static str {
    match variant {
        AgentVariant::AgentServe => "AgentServe",
        AgentVariant::Command => "Command",
    }
}

fn condition_type(kind: ConditionType) -> &'static str {
    match kind {
        ConditionType::Checked => "Checked",
        ConditionType::Available => "Available",
    }
}

fn condition_status(status: ConditionStatus) -> &'static str {
    match status {
        ConditionStatus::True => "True",
        ConditionStatus::False => "False",
        ConditionStatus::Unknown => "Unknown",
    }
}

fn condition(
    kind: ConditionType,
    status: ConditionStatus,
    reason: &str,
    message: impl Into<String>,
) -> Condition {
    Condition {
        r#type: kind,
        status,
        reason: reason.to_owned(),
        message: message.into(),
    }
}

/// The conditions `Checked` and `Available` of a resource in `state`, from its check and what
/// was last read of its Deployment and the variant its check recorded.
#[must_use]
pub fn conditions(
    state: LoomAgentState,
    check: Option<&CheckResult>,
    deployment: Option<&DeploymentStatus>,
    variant: Option<AgentVariant>,
) -> Vec<Condition> {
    let message = check.map(|check| check.message.clone()).unwrap_or_default();
    match state {
        LoomAgentState::Pending => vec![
            condition(
                ConditionType::Checked,
                ConditionStatus::Unknown,
                "CheckPending",
                "the spec of this generation is not checked yet",
            ),
            condition(
                ConditionType::Available,
                ConditionStatus::Unknown,
                "CheckPending",
                "no Deployment before the spec is checked",
            ),
        ],
        LoomAgentState::Refused => vec![
            condition(
                ConditionType::Checked,
                ConditionStatus::False,
                "SpecRefused",
                message,
            ),
            condition(
                ConditionType::Available,
                ConditionStatus::False,
                "SpecRefused",
                "a refused spec has no Deployment",
            ),
        ],
        LoomAgentState::Completed | LoomAgentState::Failed => {
            let (reason, ended) = if state == LoomAgentState::Completed {
                ("RunSucceeded", "succeeded")
            } else {
                ("RunFailed", "failed")
            };
            vec![
                condition(
                    ConditionType::Checked,
                    ConditionStatus::True,
                    "SpecAdmitted",
                    message,
                ),
                condition(
                    ConditionType::Available,
                    ConditionStatus::False,
                    reason,
                    format!(
                        "the Job's run {ended}; it is not started again before a new generation"
                    ),
                ),
            ]
        }
        LoomAgentState::Running if variant == Some(AgentVariant::Command) => vec![
            condition(
                ConditionType::Checked,
                ConditionStatus::True,
                "SpecAdmitted",
                message,
            ),
            condition(
                ConditionType::Available,
                ConditionStatus::Unknown,
                "RunNotFinished",
                "the Job's run has not finished",
            ),
        ],
        LoomAgentState::Running | LoomAgentState::Degraded => {
            let available = match deployment.map(|read| read.availability) {
                Some(DeploymentAvailability::Available) => condition(
                    ConditionType::Available,
                    ConditionStatus::True,
                    "DeploymentAvailable",
                    "the Deployment reports Available",
                ),
                Some(DeploymentAvailability::Unavailable) => condition(
                    ConditionType::Available,
                    ConditionStatus::False,
                    "DeploymentUnavailable",
                    "the Deployment reports it is not Available",
                ),
                None => condition(
                    ConditionType::Available,
                    ConditionStatus::Unknown,
                    "DeploymentNotObserved",
                    "the Deployment reports no availability yet",
                ),
            };
            vec![
                condition(
                    ConditionType::Checked,
                    ConditionStatus::True,
                    match variant {
                        Some(AgentVariant::Command) => "SpecAdmitted",
                        _ => "SpecServeable",
                    },
                    message,
                ),
                available,
            ]
        }
    }
}

/// The status a row of `LoomAgents` publishes.
#[must_use]
pub fn status(row: &LoomAgents) -> resource::LoomAgentStatus {
    let conditions = row.conditions.clone().unwrap_or_else(|| {
        conditions(
            row.state,
            row.check.as_ref(),
            row.deployment.as_ref(),
            row.variant,
        )
    });
    resource::LoomAgentStatus {
        observed_generation: row.observed_generation,
        state: state(row.state).to_owned(),
        check: row.check.as_ref().map(|check| resource::CheckStatus {
            exit_code: check.exit_code,
            message: check.message.clone(),
        }),
        deployment: row
            .deployment
            .as_ref()
            .map(|read| resource::DeploymentStatus {
                availability: availability(read.availability).to_owned(),
            }),
        variant: row.variant.map(|variant| variant_name(variant).to_owned()),
        conditions: Some(
            conditions
                .into_iter()
                .map(|condition| resource::Condition {
                    kind: condition_type(condition.r#type).to_owned(),
                    status: condition_status(condition.status).to_owned(),
                    reason: condition.reason,
                    message: condition.message,
                })
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use b10x_loom_resource::model::ResourceName;

    use super::*;

    fn triples(conditions: &[Condition]) -> Vec<(ConditionType, ConditionStatus, &str)> {
        conditions
            .iter()
            .map(|condition| {
                (
                    condition.r#type,
                    condition.status,
                    condition.reason.as_str(),
                )
            })
            .collect()
    }

    fn checked(exit_code: i64, message: &str) -> CheckResult {
        CheckResult {
            exit_code,
            message: message.to_owned(),
        }
    }

    fn read(availability: DeploymentAvailability) -> DeploymentStatus {
        DeploymentStatus { availability }
    }

    #[test]
    fn pending_is_unknown_on_both_conditions() {
        assert_eq!(
            triples(&conditions(LoomAgentState::Pending, None, None, None)),
            [
                (
                    ConditionType::Checked,
                    ConditionStatus::Unknown,
                    "CheckPending"
                ),
                (
                    ConditionType::Available,
                    ConditionStatus::Unknown,
                    "CheckPending"
                ),
            ]
        );
    }

    #[test]
    fn running_is_checked_and_available_as_the_deployment_was_read() {
        let check = checked(0, "checked a: 1 states, 1 bindings, protocol none");
        let unread = conditions(
            LoomAgentState::Running,
            Some(&check),
            None,
            Some(AgentVariant::AgentServe),
        );
        assert_eq!(
            triples(&unread),
            [
                (
                    ConditionType::Checked,
                    ConditionStatus::True,
                    "SpecServeable"
                ),
                (
                    ConditionType::Available,
                    ConditionStatus::Unknown,
                    "DeploymentNotObserved"
                ),
            ]
        );
        assert_eq!(unread[0].message, check.message);
        assert_eq!(
            triples(&conditions(
                LoomAgentState::Running,
                Some(&check),
                Some(&read(DeploymentAvailability::Available)),
                Some(AgentVariant::AgentServe),
            )),
            [
                (
                    ConditionType::Checked,
                    ConditionStatus::True,
                    "SpecServeable"
                ),
                (
                    ConditionType::Available,
                    ConditionStatus::True,
                    "DeploymentAvailable"
                ),
            ]
        );
    }

    #[test]
    fn degraded_is_checked_and_not_available() {
        let check = checked(0, "checked");
        assert_eq!(
            triples(&conditions(
                LoomAgentState::Degraded,
                Some(&check),
                Some(&read(DeploymentAvailability::Unavailable)),
                Some(AgentVariant::AgentServe),
            )),
            [
                (
                    ConditionType::Checked,
                    ConditionStatus::True,
                    "SpecServeable"
                ),
                (
                    ConditionType::Available,
                    ConditionStatus::False,
                    "DeploymentUnavailable"
                ),
            ]
        );
    }

    #[test]
    fn refused_is_neither_checked_nor_available_and_carries_the_refusal() {
        let check = checked(3, "refused: SecretLiteral at spec.agent.x: a token prefix");
        let refused = conditions(
            LoomAgentState::Refused,
            Some(&check),
            None,
            Some(AgentVariant::AgentServe),
        );
        assert_eq!(
            triples(&refused),
            [
                (
                    ConditionType::Checked,
                    ConditionStatus::False,
                    "SpecRefused"
                ),
                (
                    ConditionType::Available,
                    ConditionStatus::False,
                    "SpecRefused"
                ),
            ]
        );
        assert_eq!(refused[0].message, check.message);
    }

    #[test]
    fn a_command_resource_reads_its_run_not_a_deployment() {
        let check = checked(0, "admitted");
        let command = Some(AgentVariant::Command);
        assert_eq!(
            triples(&conditions(
                LoomAgentState::Running,
                Some(&check),
                None,
                command
            )),
            [
                (
                    ConditionType::Checked,
                    ConditionStatus::True,
                    "SpecAdmitted"
                ),
                (
                    ConditionType::Available,
                    ConditionStatus::Unknown,
                    "RunNotFinished"
                ),
            ]
        );
        for (state, reason) in [
            (LoomAgentState::Completed, "RunSucceeded"),
            (LoomAgentState::Failed, "RunFailed"),
        ] {
            let ended = conditions(state, Some(&check), None, command);
            assert_eq!(
                triples(&ended),
                [
                    (
                        ConditionType::Checked,
                        ConditionStatus::True,
                        "SpecAdmitted"
                    ),
                    (ConditionType::Available, ConditionStatus::False, reason),
                ]
            );
            assert_eq!(ended[0].message, "admitted");
        }
        assert_eq!(state(LoomAgentState::Completed), "Completed");
        assert_eq!(state(LoomAgentState::Failed), "Failed");
    }

    #[test]
    fn a_row_publishes_its_members_and_the_computed_conditions() {
        let row = LoomAgents {
            name: ResourceName("a".to_owned()),
            observed_generation: 4,
            check: Some(checked(0, "checked")),
            deployment: Some(read(DeploymentAvailability::Unavailable)),
            variant: Some(AgentVariant::Command),
            conditions: None,
            state: LoomAgentState::Degraded,
        };
        let status = status(&row);
        assert_eq!(status.observed_generation, 4);
        assert_eq!(status.state, "Degraded");
        assert_eq!(
            status.deployment.map(|read| read.availability),
            Some("Unavailable".to_owned())
        );
        let conditions = status.conditions.expect("conditions");
        assert_eq!(conditions[1].kind, "Available");
        assert_eq!(conditions[1].status, "False");
    }
}
