//! `LoomAgentRecord` walks the `LoomAgent` lifecycle of `loom.resource` (`ess/domains/resource.yaml`)
//! through the generated behaviour: every answer below is the generated outcome, every state the
//! one the `LoomAgents` view reads back, and every event the one the record kept, each written out
//! as a literal.

use b10x_loom_resource::model::{
    ApplyLoomAgent, ApplyLoomAgentOutcome, CheckResult, DeleteLoomAgent, DeleteLoomAgentOutcome,
    DeploymentAvailability, DeploymentStatus, LoomAgentApplied, LoomAgentDegraded,
    LoomAgentDeleted, LoomAgentNotFound, LoomAgentObserved, LoomAgentRecovered, LoomAgentRefused,
    LoomAgentRunning, LoomAgentState, LoomAgentStateConflict, LoomAgents, ObserveDeployment,
    ObserveDeploymentOutcome, RecordCheck, RecordCheckOutcome, ResourceName, StaleGeneration,
};
use b10x_loom_resource::{LoomAgentRecord, ResourceEvent};

fn name() -> ResourceName {
    ResourceName("platform-operator".to_owned())
}

fn apply(record: &mut LoomAgentRecord, generation: i64) -> ApplyLoomAgentOutcome {
    record
        .apply_loom_agent(ApplyLoomAgent {
            name: name(),
            generation,
        })
        .expect("the generated behaviour answers every apply")
}

fn check(
    record: &mut LoomAgentRecord,
    generation: i64,
    exit_code: i64,
    message: &str,
) -> RecordCheckOutcome {
    record
        .record_check(RecordCheck {
            name: name(),
            generation,
            exit_code,
            message: message.to_owned(),
        })
        .expect("the generated behaviour answers every check")
}

fn observe(
    record: &mut LoomAgentRecord,
    availability: DeploymentAvailability,
) -> ObserveDeploymentOutcome {
    record
        .observe_deployment(ObserveDeployment {
            name: name(),
            availability,
        })
        .expect("the generated behaviour answers every observation")
}

fn delete(record: &mut LoomAgentRecord) -> DeleteLoomAgentOutcome {
    record
        .delete_loom_agent(DeleteLoomAgent { name: name() })
        .expect("the generated behaviour answers every delete")
}

/// The one row the view holds, or `None` when it holds none; more than one row is a failure.
fn row(record: &LoomAgentRecord) -> Option<LoomAgents> {
    let mut rows = record
        .loom_agents()
        .expect("the generated query answers the view");
    assert!(rows.len() <= 1, "one resource, one row: {rows:?}");
    rows.pop()
}

/// The row of a resource no Deployment observation has reached since its generation was applied.
fn held(generation: i64, check: Option<(i64, &str)>, state: LoomAgentState) -> Option<LoomAgents> {
    observed(generation, check, None, state)
}

/// The row of a resource whose last Deployment observation read `availability`. No command sets
/// `conditions`: the controller computes them when it publishes the status.
fn observed(
    generation: i64,
    check: Option<(i64, &str)>,
    availability: Option<DeploymentAvailability>,
    state: LoomAgentState,
) -> Option<LoomAgents> {
    Some(LoomAgents {
        name: name(),
        observed_generation: generation,
        check: check.map(|(exit_code, message)| CheckResult {
            exit_code,
            message: message.to_owned(),
        }),
        deployment: availability.map(|availability| DeploymentStatus { availability }),
        conditions: None,
        state,
    })
}

#[test]
fn the_record_walks_the_lifecycle_from_apply_to_delete() {
    let mut record = LoomAgentRecord::default();
    assert_eq!(row(&record), None);

    assert_eq!(
        apply(&mut record, 1),
        ApplyLoomAgentOutcome::Created {
            loom_agent_applied: LoomAgentApplied {
                name: name(),
                generation: 1
            }
        }
    );
    assert_eq!(row(&record), held(1, None, LoomAgentState::Pending));

    assert_eq!(apply(&mut record, 1), ApplyLoomAgentOutcome::Unchanged);
    assert_eq!(row(&record), held(1, None, LoomAgentState::Pending));

    assert_eq!(
        check(&mut record, 1, 0, ""),
        RecordCheckOutcome::Running {
            loom_agent_running: LoomAgentRunning {
                name: name(),
                generation: 1
            }
        }
    );
    assert_eq!(
        row(&record),
        held(1, Some((0, "")), LoomAgentState::Running)
    );

    assert_eq!(
        observe(&mut record, DeploymentAvailability::Available),
        ObserveDeploymentOutcome::StillRunning {
            loom_agent_observed: LoomAgentObserved {
                name: name(),
                availability: DeploymentAvailability::Available
            }
        }
    );
    assert_eq!(
        row(&record),
        observed(
            1,
            Some((0, "")),
            Some(DeploymentAvailability::Available),
            LoomAgentState::Running
        ),
        "a Running resource whose Deployment is available shows it"
    );
    assert_eq!(
        observe(&mut record, DeploymentAvailability::Unavailable),
        ObserveDeploymentOutcome::Degraded {
            loom_agent_degraded: LoomAgentDegraded { name: name() }
        }
    );
    assert_eq!(
        row(&record),
        observed(
            1,
            Some((0, "")),
            Some(DeploymentAvailability::Unavailable),
            LoomAgentState::Degraded
        )
    );
    assert_eq!(
        observe(&mut record, DeploymentAvailability::Unavailable),
        ObserveDeploymentOutcome::StillDegraded {
            loom_agent_observed: LoomAgentObserved {
                name: name(),
                availability: DeploymentAvailability::Unavailable
            }
        }
    );
    assert_eq!(
        observe(&mut record, DeploymentAvailability::Available),
        ObserveDeploymentOutcome::Recovered {
            loom_agent_recovered: LoomAgentRecovered { name: name() }
        }
    );
    assert_eq!(
        row(&record),
        observed(
            1,
            Some((0, "")),
            Some(DeploymentAvailability::Available),
            LoomAgentState::Running
        )
    );

    assert_eq!(
        apply(&mut record, 2),
        ApplyLoomAgentOutcome::Respecified {
            loom_agent_applied: LoomAgentApplied {
                name: name(),
                generation: 2
            }
        }
    );
    assert_eq!(
        row(&record),
        held(2, None, LoomAgentState::Pending),
        "a new generation clears the check and the Deployment observation"
    );

    assert_eq!(
        check(&mut record, 2, 3, "bindings[1].base_url is not a URL"),
        RecordCheckOutcome::Refused {
            loom_agent_refused: LoomAgentRefused {
                name: name(),
                generation: 2,
                message: "bindings[1].base_url is not a URL".to_owned()
            }
        }
    );
    assert_eq!(
        row(&record),
        held(
            2,
            Some((3, "bindings[1].base_url is not a URL")),
            LoomAgentState::Refused
        )
    );

    assert_eq!(
        apply(&mut record, 1),
        ApplyLoomAgentOutcome::Stale {
            error: StaleGeneration {
                name: name(),
                generation: 1,
                observed_generation: 2
            }
        }
    );
    assert_eq!(
        check(&mut record, 1, 0, ""),
        RecordCheckOutcome::Stale {
            error: StaleGeneration {
                name: name(),
                generation: 1,
                observed_generation: 2
            }
        }
    );
    assert_eq!(
        row(&record),
        held(
            2,
            Some((3, "bindings[1].base_url is not a URL")),
            LoomAgentState::Refused
        ),
        "a stale apply or check changes nothing"
    );

    assert_eq!(
        delete(&mut record),
        DeleteLoomAgentOutcome::Deleted {
            loom_agent_deleted: LoomAgentDeleted { name: name() }
        }
    );
    assert_eq!(row(&record), None);
    assert_eq!(
        delete(&mut record),
        DeleteLoomAgentOutcome::Unknown {
            error: LoomAgentNotFound { name: name() }
        }
    );

    assert_eq!(
        record.events(),
        [
            ResourceEvent::Applied(LoomAgentApplied {
                name: name(),
                generation: 1
            }),
            ResourceEvent::Running(LoomAgentRunning {
                name: name(),
                generation: 1
            }),
            ResourceEvent::Observed(LoomAgentObserved {
                name: name(),
                availability: DeploymentAvailability::Available
            }),
            ResourceEvent::Degraded(LoomAgentDegraded { name: name() }),
            ResourceEvent::Observed(LoomAgentObserved {
                name: name(),
                availability: DeploymentAvailability::Unavailable
            }),
            ResourceEvent::Recovered(LoomAgentRecovered { name: name() }),
            ResourceEvent::Applied(LoomAgentApplied {
                name: name(),
                generation: 2
            }),
            ResourceEvent::Refused(LoomAgentRefused {
                name: name(),
                generation: 2,
                message: "bindings[1].base_url is not a URL".to_owned()
            }),
            ResourceEvent::Deleted(LoomAgentDeleted { name: name() }),
        ]
        .as_slice(),
        "the record keeps exactly the events its outcomes published, in order"
    );
}

#[test]
fn a_newer_generation_returns_every_state_to_pending_with_the_check_cleared() {
    let mut record = LoomAgentRecord::default();
    apply(&mut record, 1);
    assert_eq!(
        apply(&mut record, 2),
        ApplyLoomAgentOutcome::Respecified {
            loom_agent_applied: LoomAgentApplied {
                name: name(),
                generation: 2
            }
        },
        "Pending is respecified too"
    );
    assert_eq!(row(&record), held(2, None, LoomAgentState::Pending));

    check(&mut record, 2, 0, "");
    observe(&mut record, DeploymentAvailability::Unavailable);
    assert_eq!(
        row(&record),
        observed(
            2,
            Some((0, "")),
            Some(DeploymentAvailability::Unavailable),
            LoomAgentState::Degraded
        )
    );
    apply(&mut record, 5);
    assert_eq!(row(&record), held(5, None, LoomAgentState::Pending));
}

#[test]
fn a_check_or_an_observation_out_of_order_changes_nothing() {
    let mut record = LoomAgentRecord::default();
    apply(&mut record, 1);
    assert_eq!(
        observe(&mut record, DeploymentAvailability::Available),
        ObserveDeploymentOutcome::NoDeployment {
            error: LoomAgentStateConflict { name: name() }
        },
        "a Pending resource has no Deployment to observe"
    );
    check(&mut record, 1, 0, "");
    assert_eq!(
        check(&mut record, 1, 0, ""),
        RecordCheckOutcome::OutOfOrder {
            error: LoomAgentStateConflict { name: name() }
        },
        "a Running resource takes no second check"
    );
    assert_eq!(
        check(&mut record, 2, 0, ""),
        RecordCheckOutcome::Stale {
            error: StaleGeneration {
                name: name(),
                generation: 2,
                observed_generation: 1
            }
        },
        "a check for a generation the record has not applied is stale"
    );
    assert_eq!(
        row(&record),
        held(1, Some((0, "")), LoomAgentState::Running)
    );

    let mut refused = LoomAgentRecord::default();
    apply(&mut refused, 1);
    check(&mut refused, 1, 1, "refused");
    assert_eq!(
        observe(&mut refused, DeploymentAvailability::Unavailable),
        ObserveDeploymentOutcome::NoDeployment {
            error: LoomAgentStateConflict { name: name() }
        },
        "a Refused resource has no Deployment to observe"
    );
    assert_eq!(
        row(&refused),
        held(1, Some((1, "refused")), LoomAgentState::Refused)
    );
}

#[test]
fn a_resource_the_record_does_not_hold_is_not_found() {
    let mut record = LoomAgentRecord::default();
    assert_eq!(
        check(&mut record, 1, 0, ""),
        RecordCheckOutcome::Unknown {
            error: LoomAgentNotFound { name: name() }
        }
    );
    assert_eq!(
        observe(&mut record, DeploymentAvailability::Available),
        ObserveDeploymentOutcome::Unknown {
            error: LoomAgentNotFound { name: name() }
        }
    );
    assert_eq!(
        delete(&mut record),
        DeleteLoomAgentOutcome::Unknown {
            error: LoomAgentNotFound { name: name() }
        }
    );
    assert_eq!(record.events(), [].as_slice());
    assert_eq!(row(&record), None);
}

#[test]
fn two_resources_are_held_apart() {
    let mut record = LoomAgentRecord::default();
    apply(&mut record, 3);
    record
        .apply_loom_agent(ApplyLoomAgent {
            name: ResourceName("triage".to_owned()),
            generation: 1,
        })
        .expect("apply");
    check(&mut record, 3, 0, "");
    let rows = record.loom_agents().expect("the view");
    assert_eq!(
        rows,
        vec![
            LoomAgents {
                name: name(),
                observed_generation: 3,
                check: Some(CheckResult {
                    exit_code: 0,
                    message: String::new()
                }),
                deployment: None,
                conditions: None,
                state: LoomAgentState::Running,
            },
            LoomAgents {
                name: ResourceName("triage".to_owned()),
                observed_generation: 1,
                check: None,
                deployment: None,
                conditions: None,
                state: LoomAgentState::Pending,
            },
        ]
    );
}
