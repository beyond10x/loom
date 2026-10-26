//! `LoomAgentRecord` walks the `LoomAgent` lifecycle of `loom.resource` (`ess/domains/resource.yaml`)
//! through the generated behaviour: every answer below is the generated outcome, every state the
//! one the `LoomAgents` view reads back, and every event the one the record kept, each written out
//! as a literal.

use b10x_loom_resource::model::{
    AgentVariant, ApplyLoomAgent, ApplyLoomAgentOutcome, CheckResult, DeleteLoomAgent,
    DeleteLoomAgentOutcome, DeploymentAvailability, DeploymentStatus, LoomAgentApplied,
    LoomAgentCompleted, LoomAgentDegraded, LoomAgentDeleted, LoomAgentFailed, LoomAgentNotFound,
    LoomAgentObserved, LoomAgentRecovered, LoomAgentRefused, LoomAgentRunObserved,
    LoomAgentRunning, LoomAgentState, LoomAgentStateConflict, LoomAgents, ObserveDeployment,
    ObserveDeploymentOutcome, ObserveRun, ObserveRunOutcome, RecordCheck, RecordCheckOutcome,
    ResourceName, RunEnd, RunPhase, StaleGeneration,
};
use b10x_loom_resource::{LoomAgentRecord, ResourceEvent, Timestamp};

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
            variant: AgentVariant::AgentServe,
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
        variant: check.map(|_| AgentVariant::AgentServe),
        run: None,
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
                variant: Some(AgentVariant::AgentServe),
                run: None,
                conditions: None,
                state: LoomAgentState::Running,
            },
            LoomAgents {
                name: ResourceName("triage".to_owned()),
                observed_generation: 1,
                check: None,
                deployment: None,
                variant: None,
                run: None,
                conditions: None,
                state: LoomAgentState::Pending,
            },
        ]
    );
}

fn observe_run(record: &mut LoomAgentRecord, phase: RunPhase) -> ObserveRunOutcome {
    record
        .observe_run(ObserveRun {
            name: name(),
            phase,
            exit_code: None,
            finished_at: None,
            reason: None,
        })
        .expect("the generated behaviour answers every run observation")
}

fn state(record: &LoomAgentRecord) -> Option<LoomAgentState> {
    row(record).map(|row| row.state)
}

/// A `Command` resource's Job runs once (story command-variant-runs-once): an active run keeps it
/// Running, the run's end makes it Completed or Failed, a finished resource stays as it is
/// whatever is read later, and only a new generation returns it to Pending.
#[test]
fn a_command_run_ends_once_and_stays_ended_until_a_new_generation() {
    let mut record = LoomAgentRecord::default();
    assert_eq!(
        observe_run(&mut record, RunPhase::Active),
        ObserveRunOutcome::Unknown {
            error: LoomAgentNotFound { name: name() }
        }
    );
    apply(&mut record, 1);
    assert_eq!(
        observe_run(&mut record, RunPhase::Succeeded),
        ObserveRunOutcome::NoRun {
            error: LoomAgentStateConflict { name: name() }
        },
        "a Pending resource has no Job"
    );
    check(&mut record, 1, 0, "admitted");
    assert_eq!(
        observe_run(&mut record, RunPhase::Active),
        ObserveRunOutcome::StillActive {
            loom_agent_run_observed: LoomAgentRunObserved {
                name: name(),
                generation: 1
            }
        }
    );
    assert_eq!(state(&record), Some(LoomAgentState::Running));
    assert_eq!(
        observe_run(&mut record, RunPhase::Succeeded),
        ObserveRunOutcome::Completed {
            loom_agent_completed: LoomAgentCompleted {
                name: name(),
                generation: 1,
                exit_code: None
            }
        }
    );
    assert_eq!(state(&record), Some(LoomAgentState::Completed));
    for phase in [RunPhase::Active, RunPhase::Failed, RunPhase::Succeeded] {
        assert_eq!(observe_run(&mut record, phase), ObserveRunOutcome::Finished);
        assert_eq!(state(&record), Some(LoomAgentState::Completed));
    }
    assert_eq!(
        check(&mut record, 1, 0, "admitted"),
        RecordCheckOutcome::OutOfOrder {
            error: LoomAgentStateConflict { name: name() }
        }
    );
    assert_eq!(
        observe(&mut record, DeploymentAvailability::Available),
        ObserveDeploymentOutcome::NoDeployment {
            error: LoomAgentStateConflict { name: name() }
        },
        "a finished run has no Deployment"
    );

    apply(&mut record, 2);
    assert_eq!(row(&record), held(2, None, LoomAgentState::Pending));
    check(&mut record, 2, 0, "admitted");
    assert_eq!(
        observe_run(&mut record, RunPhase::Failed),
        ObserveRunOutcome::Failed {
            loom_agent_failed: LoomAgentFailed {
                name: name(),
                generation: 2,
                exit_code: None
            }
        }
    );
    assert_eq!(state(&record), Some(LoomAgentState::Failed));
    assert_eq!(
        observe_run(&mut record, RunPhase::Succeeded),
        ObserveRunOutcome::Finished
    );
    assert_eq!(state(&record), Some(LoomAgentState::Failed));
    apply(&mut record, 3);
    assert_eq!(state(&record), Some(LoomAgentState::Pending));

    let applied = |generation| {
        ResourceEvent::Applied(LoomAgentApplied {
            name: name(),
            generation,
        })
    };
    let running = |generation| {
        ResourceEvent::Running(LoomAgentRunning {
            name: name(),
            generation,
        })
    };
    assert_eq!(
        record.events(),
        [
            applied(1),
            running(1),
            ResourceEvent::RunObserved(LoomAgentRunObserved {
                name: name(),
                generation: 1
            }),
            ResourceEvent::Completed(LoomAgentCompleted {
                name: name(),
                generation: 1,
                exit_code: None
            }),
            applied(2),
            running(2),
            ResourceEvent::Failed(LoomAgentFailed {
                name: name(),
                generation: 2,
                exit_code: None
            }),
            applied(3),
        ],
        "a finished resource observed again publishes nothing"
    );
}

/// How a run ended as the Job's pod said: an exit code, an end time and a reason, each absent when
/// the pod did not say.
fn ended(exit_code: Option<i64>, finished_at: Option<&str>, reason: Option<&str>) -> RunEnd {
    RunEnd {
        exit_code,
        finished_at: finished_at.map(|at| Timestamp(at.to_owned())),
        reason: reason.map(str::to_owned),
    }
}

fn observe_end(record: &mut LoomAgentRecord, phase: RunPhase, end: &RunEnd) -> ObserveRunOutcome {
    record
        .observe_run(ObserveRun {
            name: name(),
            phase,
            exit_code: end.exit_code,
            finished_at: end.finished_at.clone(),
            reason: end.reason.clone(),
        })
        .expect("the generated behaviour answers every run observation")
}

/// `story:command-run-exit-status`: the run's end is recorded as `run` when the resource moves to
/// Completed or Failed, read back by the view, kept when the finished resource is observed again,
/// cleared by a new generation, and never recorded by an observation of a run still active.
#[test]
fn a_command_run_records_how_it_ended_until_a_new_generation() {
    let mut record = LoomAgentRecord::default();
    apply(&mut record, 1);
    check(&mut record, 1, 0, "admitted");
    let active = ended(Some(9), Some("2026-10-26T09:00:00Z"), Some("Error"));
    observe_end(&mut record, RunPhase::Active, &active);
    assert_eq!(
        row(&record).and_then(|row| row.run),
        None,
        "an active run has no end"
    );

    let succeeded = ended(Some(0), Some("2026-10-26T10:00:00Z"), Some("Completed"));
    assert_eq!(
        observe_end(&mut record, RunPhase::Succeeded, &succeeded),
        ObserveRunOutcome::Completed {
            loom_agent_completed: LoomAgentCompleted {
                name: name(),
                generation: 1,
                exit_code: Some(0),
            }
        }
    );
    let completed = row(&record).expect("the row");
    assert_eq!(completed.state, LoomAgentState::Completed);
    assert_eq!(completed.run, Some(succeeded.clone()));
    assert_eq!(
        observe_end(
            &mut record,
            RunPhase::Failed,
            &ended(Some(5), Some("2026-10-26T11:00:00Z"), Some("Error"))
        ),
        ObserveRunOutcome::Finished
    );
    assert_eq!(
        row(&record).and_then(|row| row.run),
        Some(succeeded),
        "a finished run's end is kept"
    );

    apply(&mut record, 2);
    assert_eq!(
        row(&record).and_then(|row| row.run),
        None,
        "a new generation clears it"
    );
    check(&mut record, 2, 0, "admitted");
    let exited = ended(Some(3), Some("2026-10-26T12:00:00Z"), Some("Error"));
    assert_eq!(
        observe_end(&mut record, RunPhase::Failed, &exited),
        ObserveRunOutcome::Failed {
            loom_agent_failed: LoomAgentFailed {
                name: name(),
                generation: 2,
                exit_code: Some(3),
            }
        }
    );
    assert_eq!(row(&record).and_then(|row| row.run), Some(exited));

    apply(&mut record, 3);
    check(&mut record, 3, 0, "admitted");
    let never_read = ended(None, None, Some("BackoffLimitExceeded"));
    assert_eq!(
        observe_end(&mut record, RunPhase::Failed, &never_read),
        ObserveRunOutcome::Failed {
            loom_agent_failed: LoomAgentFailed {
                name: name(),
                generation: 3,
                exit_code: None,
            }
        }
    );
    let failed = row(&record).expect("the row");
    assert_eq!(failed.state, LoomAgentState::Failed);
    assert_eq!(failed.run, Some(never_read));
}
