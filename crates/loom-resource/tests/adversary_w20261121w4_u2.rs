//! Adversary, wave 2026-11-21-w4, unit U2 (story ess-format-31): the raise of `ess/system.yaml`
//! to `ess/31` adds an up-front existence check to five `loom.resource` commands. These cases pin
//! the answers, events and rows for every input class where the order of refusals could differ:
//! an absent row beside every value of every other guard, a deleted row, a name that is a prefix of
//! a held one, and a stale generation on a row in a state the selected branch cannot move from.
//! Every expected value is a literal read from `ess/domains/resource.yaml`.

use b10x_loom_resource::model::{
    AgentVariant, ApplyLoomAgent, ApplyLoomAgentOutcome, DeleteLoomAgent, DeleteLoomAgentOutcome,
    DeploymentAvailability, LoomAgentApplied, LoomAgentDeleted, LoomAgentNotFound,
    LoomAgentRefused, LoomAgentRunning, LoomAgentState, LoomAgentStateConflict, ObserveDeployment,
    ObserveDeploymentOutcome, ObserveRun, ObserveRunOutcome, ObserveScheduledRun,
    ObserveScheduledRunOutcome, RecordCheck, RecordCheckOutcome, ResourceName, RunPhase,
    StaleGeneration,
};
use b10x_loom_resource::{LoomAgentRecord, ResourceEvent, Timestamp};

fn target() -> ResourceName {
    ResourceName("platform-operator".to_owned())
}

fn not_found() -> LoomAgentNotFound {
    LoomAgentNotFound { name: target() }
}

fn apply(record: &mut LoomAgentRecord, name: &ResourceName, generation: i64) {
    let outcome = record
        .apply_loom_agent(ApplyLoomAgent {
            name: name.clone(),
            generation,
        })
        .expect("apply");
    assert_eq!(
        outcome,
        ApplyLoomAgentOutcome::Created {
            loom_agent_applied: LoomAgentApplied {
                name: name.clone(),
                generation,
            }
        }
    );
}

fn run_ends() -> [(Option<i64>, Option<Timestamp>, Option<String>); 2] {
    [
        (None, None, None),
        (
            Some(i64::MAX),
            Some(Timestamp("2026-11-21T08:00:00Z".to_owned())),
            Some(String::new()),
        ),
    ]
}

/// Sends every command but `DeleteLoomAgent` for `target()` with every value of every other guard
/// the command reads, and asserts each answers `unknown` naming `target()`.
fn assert_every_observation_is_unknown(record: &mut LoomAgentRecord) {
    for generation in [i64::MIN, -1, 0, 1, 2, i64::MAX] {
        for exit_code in [i64::MIN, -1, 0, 1, i64::MAX] {
            for variant in [AgentVariant::AgentServe, AgentVariant::Command] {
                assert_eq!(
                    record
                        .record_check(RecordCheck {
                            name: target(),
                            generation,
                            exit_code,
                            message: String::new(),
                            variant,
                        })
                        .expect("check"),
                    RecordCheckOutcome::Unknown { error: not_found() },
                    "RecordCheck generation {generation} exit {exit_code}"
                );
            }
        }
    }
    for availability in [
        DeploymentAvailability::Available,
        DeploymentAvailability::Unavailable,
    ] {
        assert_eq!(
            record
                .observe_deployment(ObserveDeployment {
                    name: target(),
                    availability,
                })
                .expect("observe deployment"),
            ObserveDeploymentOutcome::Unknown { error: not_found() }
        );
    }
    for phase in [RunPhase::Active, RunPhase::Succeeded, RunPhase::Failed] {
        for (exit_code, finished_at, reason) in run_ends() {
            assert_eq!(
                record
                    .observe_run(ObserveRun {
                        name: target(),
                        phase,
                        exit_code,
                        finished_at: finished_at.clone(),
                        reason: reason.clone(),
                    })
                    .expect("observe run"),
                ObserveRunOutcome::Unknown { error: not_found() },
                "ObserveRun {phase:?}"
            );
            assert_eq!(
                record
                    .observe_scheduled_run(ObserveScheduledRun {
                        name: target(),
                        phase,
                        exit_code,
                        finished_at,
                        reason,
                    })
                    .expect("observe scheduled run"),
                ObserveScheduledRunOutcome::Unknown { error: not_found() },
                "ObserveScheduledRun {phase:?}"
            );
        }
    }
}

fn delete(record: &mut LoomAgentRecord) -> DeleteLoomAgentOutcome {
    record
        .delete_loom_agent(DeleteLoomAgent { name: target() })
        .expect("delete")
}

#[test]
fn adversary_an_absent_row_is_unknown_beside_every_other_guard_and_writes_nothing() {
    // An empty record, and one holding names that differ from `target()` only by a prefix, a
    // suffix and case: none of them is the addressed row.
    let mut empty = LoomAgentRecord::default();
    assert_every_observation_is_unknown(&mut empty);
    assert_eq!(
        delete(&mut empty),
        DeleteLoomAgentOutcome::Unknown { error: not_found() }
    );
    assert_eq!(empty, LoomAgentRecord::default());

    let mut others = LoomAgentRecord::default();
    let neighbours = [
        ResourceName("platform".to_owned()),
        ResourceName("platform-operator-2".to_owned()),
        ResourceName("Platform-Operator".to_owned()),
    ];
    for neighbour in &neighbours {
        apply(&mut others, neighbour, 7);
    }
    let before = others.clone();
    assert_every_observation_is_unknown(&mut others);
    assert_eq!(
        delete(&mut others),
        DeleteLoomAgentOutcome::Unknown { error: not_found() }
    );
    assert_eq!(others, before, "no row and no event changed");
    let rows = others.loom_agents().expect("view");
    assert_eq!(
        rows.iter().map(|row| row.name.clone()).collect::<Vec<_>>(),
        neighbours.to_vec()
    );
    assert!(rows.iter().all(|row| row.state == LoomAgentState::Pending));
}

#[test]
fn adversary_a_deleted_row_is_unknown_to_every_command() {
    let mut record = LoomAgentRecord::default();
    apply(&mut record, &target(), 3);
    assert_eq!(
        record
            .record_check(RecordCheck {
                name: target(),
                generation: 3,
                exit_code: 0,
                message: "ok".to_owned(),
                variant: AgentVariant::Command,
            })
            .expect("check"),
        RecordCheckOutcome::Running {
            loom_agent_running: LoomAgentRunning {
                name: target(),
                generation: 3,
            }
        }
    );
    assert_eq!(
        delete(&mut record),
        DeleteLoomAgentOutcome::Deleted {
            loom_agent_deleted: LoomAgentDeleted { name: target() }
        }
    );
    let events = vec![
        ResourceEvent::Applied(LoomAgentApplied {
            name: target(),
            generation: 3,
        }),
        ResourceEvent::Running(LoomAgentRunning {
            name: target(),
            generation: 3,
        }),
        ResourceEvent::Deleted(LoomAgentDeleted { name: target() }),
    ];
    assert_eq!(record.events(), events.as_slice());

    assert_every_observation_is_unknown(&mut record);
    assert_eq!(
        delete(&mut record),
        DeleteLoomAgentOutcome::Unknown { error: not_found() },
        "a second delete finds no row"
    );
    assert_eq!(record.events(), events.as_slice());
    assert_eq!(record.agent(&target()), None);
    assert_eq!(record.loom_agents().expect("view"), Vec::new());
}

#[test]
fn adversary_a_stale_generation_answers_before_the_state_conflict_and_writes_nothing() {
    // A Refused row: `running` and `refused` both move from Pending only, so a current generation
    // answers `out-of-order`; a stale one answers `stale` first, whatever the exit code.
    let mut record = LoomAgentRecord::default();
    apply(&mut record, &target(), 1);
    assert_eq!(
        record
            .record_check(RecordCheck {
                name: target(),
                generation: 1,
                exit_code: 2,
                message: "refused".to_owned(),
                variant: AgentVariant::AgentServe,
            })
            .expect("check"),
        RecordCheckOutcome::Refused {
            loom_agent_refused: LoomAgentRefused {
                name: target(),
                generation: 1,
                message: "refused".to_owned(),
            }
        }
    );
    let before = record.clone();
    for exit_code in [0, 2] {
        for generation in [0, 2, i64::MAX] {
            assert_eq!(
                record
                    .record_check(RecordCheck {
                        name: target(),
                        generation,
                        exit_code,
                        message: "again".to_owned(),
                        variant: AgentVariant::Command,
                    })
                    .expect("check"),
                RecordCheckOutcome::Stale {
                    error: StaleGeneration {
                        name: target(),
                        generation,
                        observed_generation: 1,
                    }
                },
                "generation {generation} exit {exit_code}"
            );
        }
        assert_eq!(
            record
                .record_check(RecordCheck {
                    name: target(),
                    generation: 1,
                    exit_code,
                    message: "again".to_owned(),
                    variant: AgentVariant::Command,
                })
                .expect("check"),
            RecordCheckOutcome::OutOfOrder {
                error: LoomAgentStateConflict { name: target() }
            },
            "exit {exit_code}"
        );
    }
    assert_eq!(record, before, "no row and no event changed");
}
