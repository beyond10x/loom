//! A step's `action finished` line names the moves its step wrote, whatever its end
//! (`story:tree-replay-command`, D6 a): a program step that moved its placed artifact, one refused
//! after it moved it, and a person's program that moved it and exited 1.

use std::path::Path;
use std::time::Instant;

use b10x_loom_commission::model::responsibility::CaseId;
use b10x_loom_tree::runlog::{ActionFinished, RunLogEvent};
use b10x_loom_tree::verify_case_log;

use super::BOUND;
use super::fixture::{FIRST, Moves, ROOT, governor, lines};

/// The last `action finished` line of `case`'s log in `store`.
fn last_finished(store: &Path, case: &str) -> ActionFinished {
    lines(store, case)
        .into_iter()
        .rev()
        .find_map(|line| match line.event {
            RunLogEvent::ActionFinished(finished) => Some(finished),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no action finished line in {case}"))
}

/// Asserts `finished` names one move, of `artifact`, to the revision `case` holds in `store`, from
/// another, and that the case's log verifies as `tree verify` checks it.
fn names_its_move(store: &Path, case: &str, finished: &ActionFinished, artifact: &str) {
    let held = governor(store).revisions(&CaseId(case.to_owned())).unwrap()[artifact].clone();
    assert_eq!(finished.revisions.len(), 1, "{finished:?}");
    let moved = &finished.revisions[0];
    assert_eq!(moved.artifact, artifact, "{finished:?}");
    assert_eq!(moved.to, held, "{finished:?}");
    assert!(
        moved.from.as_ref().is_some_and(|from| *from != held),
        "{finished:?}"
    );
    assert_eq!(verify_case_log(store, case).unwrap(), None, "{case}");
}

#[test]
fn a_step_that_moves_an_artifact_logs_the_move() {
    let started = Instant::now();
    let moves = Moves::made();

    // A program step: the first leg's `route.write`, read back by the run log's decoder.
    let store = moves.copy("moved", "logs-move");
    let finished = last_finished(&store, FIRST);
    assert_eq!(finished.action, "route.write", "{finished:?}");
    assert_eq!(finished.exit, Some(0), "{finished:?}");
    names_its_move(&store, FIRST, &finished, "route");

    // The same step refused after it moved the route: the line names the move and the refusal.
    let store = moves.copy("refused", "logs-refused-move");
    let finished = last_finished(&store, ROOT);
    assert_eq!(finished.action, "route.write", "{finished:?}");
    assert!(
        finished
            .refusal
            .as_deref()
            .is_some_and(|refusal| refusal.starts_with("ProgramError")),
        "{finished:?}"
    );
    names_its_move(&store, ROOT, &finished, "route");

    // A person's program that moved its subject and exited 1: its move, and no record.
    let store = moves.copy("person", "logs-person-move");
    let finished = last_finished(&store, ROOT);
    assert_eq!(finished.action, "answer.free", "{finished:?}");
    assert_eq!(finished.exit, Some(1), "{finished:?}");
    assert!(finished.evidence.is_empty(), "{finished:?}");
    names_its_move(&store, ROOT, &finished, "subject");
    assert!(started.elapsed() < BOUND);
}
