//! The `loom.resource` domain, answered by `b10x-loom-resource`.
//!
//! `ApplyLoomAgent`, `RecordCheck`, `ObserveDeployment` and `DeleteLoomAgent` are answered by the
//! scenario's [`LoomAgentRecord`], which runs the generated behaviour of `loom.resource` over
//! itself, and the view `LoomAgents` by its generated query. This module decodes the suite's input
//! into the generated types, publishes the event each outcome carries and encodes each answer back;
//! it decides no outcome. The suite names members by their declared names (`observed_generation`,
//! `exit_code`), not by their wire names. The view's `conditions` is encoded as the record holds
//! it; no command sets it (ESS 0.69.0 admits no list under `sets:`), so every row reads it absent.

use std::collections::BTreeMap;

use b10x_loom_resource::LoomAgentRecord;
use b10x_loom_resource::model::{
    ApplyLoomAgent, ApplyLoomAgentOutcome, CheckResult, Condition, ConditionStatus, ConditionType,
    DeleteLoomAgent, DeleteLoomAgentOutcome, DeploymentAvailability, DeploymentStatus,
    LoomAgentObserved, LoomAgentState, LoomAgents, ObserveDeployment, ObserveDeploymentOutcome,
    RecordCheck, RecordCheckOutcome, ResourceName, StaleGeneration,
};
use ess_conformance::target::{ObservedEvent, SemanticCommandResult, ViewRow};
use ess_primitives::ids::CorrelationId;
use ess_primitives::node::Node;

use crate::codec::{self, Input};
use crate::{Live, error, event, took};

pub(crate) const APPLY_LOOM_AGENT: &str = "loom.resource.ApplyLoomAgent";
pub(crate) const RECORD_CHECK: &str = "loom.resource.RecordCheck";
pub(crate) const OBSERVE_DEPLOYMENT: &str = "loom.resource.ObserveDeployment";
pub(crate) const DELETE_LOOM_AGENT: &str = "loom.resource.DeleteLoomAgent";
pub(crate) const LOOM_AGENTS: &str = "loom.resource.LoomAgents";

const LOOM_AGENT_APPLIED: &str = "loom.resource.LoomAgentApplied";
const LOOM_AGENT_RUNNING: &str = "loom.resource.LoomAgentRunning";
const LOOM_AGENT_REFUSED: &str = "loom.resource.LoomAgentRefused";
const LOOM_AGENT_DEGRADED: &str = "loom.resource.LoomAgentDegraded";
const LOOM_AGENT_RECOVERED: &str = "loom.resource.LoomAgentRecovered";
const LOOM_AGENT_OBSERVED: &str = "loom.resource.LoomAgentObserved";
const LOOM_AGENT_DELETED: &str = "loom.resource.LoomAgentDeleted";

const STALE_GENERATION: &str = "loom.resource.StaleGeneration";
const LOOM_AGENT_NOT_FOUND: &str = "loom.resource.LoomAgentNotFound";
const LOOM_AGENT_STATE_CONFLICT: &str = "loom.resource.LoomAgentStateConflict";

fn name(input: &Input) -> Option<ResourceName> {
    codec::text(input, "name").map(ResourceName)
}

fn name_node(name: &ResourceName) -> Node {
    codec::text_node(&name.0)
}

/// `outcome` of `command`, refused with `StaleGeneration`.
fn stale(command: &str, stale: &StaleGeneration) -> SemanticCommandResult {
    took(command, "stale").with_error(
        error(STALE_GENERATION)
            .with("name", name_node(&stale.name))
            .with("generation", codec::number(stale.generation))
            .with(
                "observed_generation",
                codec::number(stale.observed_generation),
            ),
    )
}

/// `outcome` of `command`, refused with the error `declared` naming the resource.
fn refused(
    command: &str,
    outcome: &str,
    declared: &str,
    name: &ResourceName,
) -> SemanticCommandResult {
    took(command, outcome).with_error(error(declared).with("name", name_node(name)))
}

/// `ApplyLoomAgent`, by the record's generated behaviour.
pub(crate) fn apply_loom_agent(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = ApplyLoomAgent {
        name: name(input)?,
        generation: codec::integer(input, "generation")?,
    };
    Some(match live.resources.apply_loom_agent(command).ok()? {
        ApplyLoomAgentOutcome::Created { loom_agent_applied } => {
            let published = live.publish(
                event(LOOM_AGENT_APPLIED)
                    .with("name", name_node(&loom_agent_applied.name))
                    .with("generation", codec::number(loom_agent_applied.generation)),
                correlation,
            );
            took(APPLY_LOOM_AGENT, "created").emitting(published)
        }
        ApplyLoomAgentOutcome::Respecified { loom_agent_applied } => {
            let published = live.publish(
                event(LOOM_AGENT_APPLIED)
                    .with("name", name_node(&loom_agent_applied.name))
                    .with("generation", codec::number(loom_agent_applied.generation)),
                correlation,
            );
            took(APPLY_LOOM_AGENT, "respecified").emitting(published)
        }
        ApplyLoomAgentOutcome::Stale { error } => stale(APPLY_LOOM_AGENT, &error),
        ApplyLoomAgentOutcome::Unchanged => took(APPLY_LOOM_AGENT, "unchanged"),
    })
}

/// `RecordCheck`, by the record's generated behaviour.
pub(crate) fn record_check(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = RecordCheck {
        name: name(input)?,
        generation: codec::integer(input, "generation")?,
        exit_code: codec::integer(input, "exit_code")?,
        message: codec::text(input, "message")?,
    };
    Some(match live.resources.record_check(command).ok()? {
        RecordCheckOutcome::Unknown { error } => {
            refused(RECORD_CHECK, "unknown", LOOM_AGENT_NOT_FOUND, &error.name)
        }
        RecordCheckOutcome::Stale { error } => stale(RECORD_CHECK, &error),
        RecordCheckOutcome::Running { loom_agent_running } => {
            let published = live.publish(
                event(LOOM_AGENT_RUNNING)
                    .with("name", name_node(&loom_agent_running.name))
                    .with("generation", codec::number(loom_agent_running.generation)),
                correlation,
            );
            took(RECORD_CHECK, "running").emitting(published)
        }
        RecordCheckOutcome::Refused { loom_agent_refused } => {
            let published = live.publish(
                event(LOOM_AGENT_REFUSED)
                    .with("name", name_node(&loom_agent_refused.name))
                    .with("generation", codec::number(loom_agent_refused.generation))
                    .with("message", codec::text_node(&loom_agent_refused.message)),
                correlation,
            );
            took(RECORD_CHECK, "refused").emitting(published)
        }
        RecordCheckOutcome::OutOfOrder { error } => refused(
            RECORD_CHECK,
            "out-of-order",
            LOOM_AGENT_STATE_CONFLICT,
            &error.name,
        ),
    })
}

/// The `loom.resource.DeploymentAvailability` field `name`.
fn availability(input: &Input, name: &str) -> Option<DeploymentAvailability> {
    Some(match input.get(name)?.as_text()? {
        "Available" => DeploymentAvailability::Available,
        "Unavailable" => DeploymentAvailability::Unavailable,
        _ => return None,
    })
}

/// The declared name of a `loom.resource.DeploymentAvailability`.
fn availability_node(availability: DeploymentAvailability) -> Node {
    codec::text_node(match availability {
        DeploymentAvailability::Available => "Available",
        DeploymentAvailability::Unavailable => "Unavailable",
    })
}

/// Publishes `LoomAgentObserved`, the event of `still-running` and `still-degraded`.
fn observed(
    live: &mut Live,
    observed: &LoomAgentObserved,
    correlation: &CorrelationId,
) -> ObservedEvent {
    live.publish(
        event(LOOM_AGENT_OBSERVED)
            .with("name", name_node(&observed.name))
            .with("availability", availability_node(observed.availability)),
        correlation,
    )
}

/// `ObserveDeployment`, by the record's generated behaviour.
pub(crate) fn observe_deployment(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = ObserveDeployment {
        name: name(input)?,
        availability: availability(input, "availability")?,
    };
    Some(match live.resources.observe_deployment(command).ok()? {
        ObserveDeploymentOutcome::Unknown { error } => refused(
            OBSERVE_DEPLOYMENT,
            "unknown",
            LOOM_AGENT_NOT_FOUND,
            &error.name,
        ),
        ObserveDeploymentOutcome::Degraded {
            loom_agent_degraded,
        } => {
            let published = live.publish(
                event(LOOM_AGENT_DEGRADED).with("name", name_node(&loom_agent_degraded.name)),
                correlation,
            );
            took(OBSERVE_DEPLOYMENT, "degraded").emitting(published)
        }
        ObserveDeploymentOutcome::StillRunning {
            loom_agent_observed,
        } => {
            let published = observed(live, &loom_agent_observed, correlation);
            took(OBSERVE_DEPLOYMENT, "still-running").emitting(published)
        }
        ObserveDeploymentOutcome::Recovered {
            loom_agent_recovered,
        } => {
            let published = live.publish(
                event(LOOM_AGENT_RECOVERED).with("name", name_node(&loom_agent_recovered.name)),
                correlation,
            );
            took(OBSERVE_DEPLOYMENT, "recovered").emitting(published)
        }
        ObserveDeploymentOutcome::StillDegraded {
            loom_agent_observed,
        } => {
            let published = observed(live, &loom_agent_observed, correlation);
            took(OBSERVE_DEPLOYMENT, "still-degraded").emitting(published)
        }
        ObserveDeploymentOutcome::NoDeployment { error } => refused(
            OBSERVE_DEPLOYMENT,
            "no-deployment",
            LOOM_AGENT_STATE_CONFLICT,
            &error.name,
        ),
    })
}

/// `DeleteLoomAgent`, by the record's generated behaviour.
pub(crate) fn delete_loom_agent(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = DeleteLoomAgent { name: name(input)? };
    Some(match live.resources.delete_loom_agent(command).ok()? {
        DeleteLoomAgentOutcome::Deleted { loom_agent_deleted } => {
            let published = live.publish(
                event(LOOM_AGENT_DELETED).with("name", name_node(&loom_agent_deleted.name)),
                correlation,
            );
            took(DELETE_LOOM_AGENT, "deleted").emitting(published)
        }
        DeleteLoomAgentOutcome::Unknown { error } => refused(
            DELETE_LOOM_AGENT,
            "unknown",
            LOOM_AGENT_NOT_FOUND,
            &error.name,
        ),
    })
}

/// The `LoomAgents` rows, by the record's generated query; `None` when the query refuses a row.
pub(crate) fn rows(record: &LoomAgentRecord) -> Option<Vec<ViewRow>> {
    Some(record.loom_agents().ok()?.iter().map(row).collect())
}

fn row(held: &LoomAgents) -> ViewRow {
    ViewRow::from([
        ("name".to_owned(), name_node(&held.name)),
        (
            "observed_generation".to_owned(),
            codec::number(held.observed_generation),
        ),
        (
            "check".to_owned(),
            held.check.as_ref().map_or(Node::Null, check),
        ),
        (
            "deployment".to_owned(),
            held.deployment.as_ref().map_or(Node::Null, deployment),
        ),
        (
            "conditions".to_owned(),
            held.conditions.as_deref().map_or(Node::Null, |held| {
                Node::Seq(held.iter().map(condition).collect())
            }),
        ),
        ("state".to_owned(), state(held.state)),
    ])
}

/// A `loom.resource.CheckResult` as a suite value.
fn check(check: &CheckResult) -> Node {
    Node::Map(BTreeMap::from([
        ("exit_code".to_owned(), codec::number(check.exit_code)),
        ("message".to_owned(), codec::text_node(&check.message)),
    ]))
}

/// A `loom.resource.DeploymentStatus` as a suite value.
fn deployment(deployment: &DeploymentStatus) -> Node {
    Node::Map(BTreeMap::from([(
        "availability".to_owned(),
        availability_node(deployment.availability),
    )]))
}

/// A `loom.resource.Condition` as a suite value, its enums by their declared names.
fn condition(condition: &Condition) -> Node {
    Node::Map(BTreeMap::from([
        (
            "type".to_owned(),
            codec::text_node(match condition.r#type {
                ConditionType::Checked => "Checked",
                ConditionType::Available => "Available",
            }),
        ),
        (
            "status".to_owned(),
            codec::text_node(match condition.status {
                ConditionStatus::True => "True",
                ConditionStatus::False => "False",
                ConditionStatus::Unknown => "Unknown",
            }),
        ),
        ("reason".to_owned(), codec::text_node(&condition.reason)),
        ("message".to_owned(), codec::text_node(&condition.message)),
    ]))
}

/// The declared name of a `LoomAgent` state.
fn state(state: LoomAgentState) -> Node {
    codec::text_node(match state {
        LoomAgentState::Pending => "Pending",
        LoomAgentState::Running => "Running",
        LoomAgentState::Refused => "Refused",
        LoomAgentState::Degraded => "Degraded",
    })
}
