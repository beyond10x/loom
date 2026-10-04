//! Adversary cases for `story:governor-port`: behaviour of the scripted fake governor that its
//! module documentation promises and `governor_port_contract` does not observe. Each case names the
//! mutant of `fake_governor.rs` or `ports/governor.rs` it catches.

use std::collections::BTreeSet;
use std::sync::Arc;

use b10x_commission::model::responsibility::{
    CaseId, CompletionDetermination, CompletionDeterminationComplete, FrontierId, GovernorError,
};
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};

fn case(id: &str) -> CaseId {
    CaseId(id.to_owned())
}

fn frontier_id(governor: &dyn Governor, case: &CaseId) -> FrontierId {
    governor
        .frontier(case)
        .unwrap_or_else(|error| panic!("frontier call on {case:?} failed: {error:?}"))
        .data()
        .frontier_id
        .clone()
}

/// Kills: `next_frontier_id` without `*issued += 1` (every frontier carries one id).
/// A frontier is an entity whose identity is `frontier_id`; two issued frontiers sharing it are one
/// entity with two contents.
#[test]
fn every_issued_frontier_has_its_own_id() {
    let governor = FakeGovernor::new();
    let a = case("case-a");
    let b = case("case-b");
    governor.script(a.clone(), [Answer::at(1), Answer::at(2)]);
    governor.script(b.clone(), [Answer::at(1)]);

    let ids = [
        frontier_id(&governor, &a),
        frontier_id(&governor, &a),
        frontier_id(&governor, &a),
        frontier_id(&governor, &b),
        frontier_id(&governor, &b),
    ];
    let distinct: BTreeSet<&str> = ids.iter().map(|id| id.0.0.as_str()).collect();
    assert_eq!(distinct.len(), ids.len(), "frontier ids repeat: {ids:?}");
}

/// Kills: `script` keeping the old script (`entry(..).or_insert(..)` instead of `insert`).
/// The fake documents "replacing any script it had".
#[test]
fn scripting_a_case_again_replaces_its_script() {
    let governor = FakeGovernor::new();
    let a = case("case-a");
    governor.script(a.clone(), [Answer::at(1), Answer::at(2)]);
    assert_eq!(governor.current_revision(&a), Ok(1));

    governor.script(a.clone(), [Answer::at(9)]);
    assert_eq!(governor.current_revision(&a), Ok(9));
}

/// Kills: `Answer::complete` dropping the revision it was given (`(0, Complete(..))`).
/// `governor_port_contract` scripts `Answer::at(3).complete("X")` and never reads the 3.
#[test]
fn a_complete_answer_keeps_its_revision() {
    let governor = FakeGovernor::new();
    let done = case("case-done");
    governor.script(done.clone(), [Answer::at(3).complete("X")]);

    assert_eq!(governor.current_revision(&done), Ok(3));
    let frontier = governor
        .frontier(&done)
        .unwrap_or_else(|error| panic!("frontier call failed: {error:?}"));
    assert_eq!(frontier.data().case_revision, 3);
    assert_eq!(
        governor.completion(&done),
        Ok(CompletionDetermination::Complete(
            CompletionDeterminationComplete {
                outcome: "X".to_owned(),
            }
        ))
    );
}

/// Kills: `Answer::complete` turning an unavailable answer into a complete one.
/// The fake documents "An unavailable answer stays unavailable".
#[test]
fn complete_does_not_revive_an_unavailable_answer() {
    let governor = FakeGovernor::new();
    let down = case("case-down");
    governor.script(down.clone(), [Answer::unavailable().complete("X")]);

    assert_eq!(
        governor.completion(&down),
        Err(GovernorError::GovernorUnavailable)
    );
}

/// Kills: `script` without its emptiness assertion (the documented `# Panics`).
#[test]
#[should_panic(expected = "has no answer")]
fn an_empty_script_is_refused() {
    let governor = FakeGovernor::new();
    governor.script(case("case-empty"), []);
}

/// Kills: a trait change that makes `Governor` unusable as `dyn Governor + Send + Sync`, and a
/// fake that stops being shareable across threads. `story:local-runtime-loop` drives the loop with
/// this fake; a shared fake is what lets a test read the fake after the loop has used it.
#[test]
fn the_fake_is_a_shareable_trait_object() {
    let governor: Arc<dyn Governor + Send + Sync> = Arc::new(FakeGovernor::new());
    let worker = Arc::clone(&governor);
    let answer = std::thread::spawn(move || worker.current_revision(&case("case-none")))
        .join()
        .unwrap_or_else(|_| panic!("worker panicked"));
    assert_eq!(answer, Err(GovernorError::UnknownCase));
    assert_eq!(
        governor.completion(&case("case-none")),
        Err(GovernorError::UnknownCase)
    );
}
