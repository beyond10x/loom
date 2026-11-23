//! Acceptance for `story:tree-replay-command`, library half: `replay_tree` decides every case of a
//! tree again as of its last `evaluated` line and says whether its state and Canon's decision
//! equal what that line recorded; a step's `action finished` line names the moves it wrote.
//!
//! The trees are the two-level tree of `crates/loom-cli/tests/fixtures/tree-two-level/`, opened
//! and run through the library once per test run ([`fixture`]); each test copies the store it
//! reads into a directory of its own and asserts it ended within 60 s. A local runner and POSIX sh
//! stubs; no network, no model, no credential.

pub mod fixture;
mod moves;

use std::time::{Duration, Instant};

use b10x_loom_commission::model::responsibility::CaseId;
use b10x_loom_tree::model::TreeReplay;
use b10x_loom_tree::runlog::{RunLogEvent, RunLogLine};
use b10x_loom_tree::{rebuild_case, replay_difference, replay_json, replay_tree, tree_status};
use serde_json::json;

use fixture::{
    FIRST, Moves, NORTH, ROOT, SOUTH, Trees, append_a_late_child_closed, catalog, governor,
    last_evaluated, lines, move_outside_the_log, receive_outside_the_log, snapshot,
};

/// The bound of every test of this module.
const BOUND: Duration = Duration::from_secs(60);

/// The entry of `case` in `replayed`.
fn entry<'a>(replayed: &'a TreeReplay, case: &str) -> &'a b10x_loom_tree::model::CaseReplay {
    replayed
        .cases
        .iter()
        .find(|entry| entry.case == case)
        .unwrap_or_else(|| panic!("no case {case}: {replayed:?}"))
}

/// Asserts every case of `replayed` but `except` was decided again and matches its log.
fn others_match(replayed: &TreeReplay, except: &str) {
    for case in replayed.cases.iter().filter(|case| case.case != except) {
        assert!(case.evaluated, "{case:?}");
        assert_eq!(case.state_matches_log, Some(true), "{case:?}");
        assert_eq!(case.decision_matches_log, Some(true), "{case:?}");
        assert_eq!(case.unmatched_line, None, "{case:?}");
    }
}

/// The lines of `lines` after the last `evaluated` one.
fn after_evaluation(lines: &[RunLogLine]) -> &[RunLogLine] {
    &lines[last_evaluated(lines) + 1..]
}

/// The legs `legs.open` opened whose log holds a `terminated` line, and the one whose log does
/// not, at the wait.
fn closed_and_open(store: &std::path::Path) -> (&'static str, &'static str) {
    let terminated = |case: &str| {
        lines(store, case)
            .iter()
            .any(|line| matches!(line.event, RunLogEvent::Terminated(_)))
    };
    match (terminated(NORTH), terminated(SOUTH)) {
        (false, true) => (SOUTH, NORTH),
        (true, false) => (NORTH, SOUTH),
        other => panic!("at the wait one leg is closed and its sibling open: {other:?}"),
    }
}

#[test]
fn a_tree_with_closed_cases_replays_unchanged() {
    let started = Instant::now();
    let trees = Trees::made();
    for which in ["wait", "ended"] {
        let store = trees.copy(which, &format!("unchanged-{which}"));
        let catalog = catalog(&store);
        if which == "wait" {
            // The premise: after the root's last `evaluated` line its log opens both legs and
            // closes one, the order `tree run` keeps.
            let (closed, _) = closed_and_open(&store);
            let root = lines(&store, ROOT);
            let after = after_evaluation(&root);
            for child in [NORTH, SOUTH] {
                assert!(
                    after.iter().any(|line| matches!(&line.event,
                        RunLogEvent::ChildOpened(opened) if opened.child == child)),
                    "{child}: {after:?}"
                );
            }
            let closed_lines: Vec<&str> = after
                .iter()
                .filter_map(|line| match &line.event {
                    RunLogEvent::ChildClosed(closed) => Some(closed.child.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(closed_lines, [closed], "{after:?}");
            // The removal: the rebuilt root holds neither leg's link nor record, the closed one's
            // included, and every line's undo applied.
            let held = governor(&store)
                .case_state(&CaseId(ROOT.to_owned()))
                .unwrap();
            let rebuilt = rebuild_case(&held, &root).unwrap();
            for child in [NORTH, SOUTH] {
                let record = &held
                    .children
                    .iter()
                    .find(|link| link.child.0 == child)
                    .unwrap_or_else(|| panic!("the root links {child}"))
                    .outcome_evidence
                    .0;
                assert!(
                    held.evidence
                        .iter()
                        .any(|held| held.data.evidence_id.0.0 == *record),
                    "{child}"
                );
                assert!(
                    rebuilt
                        .state
                        .children
                        .iter()
                        .all(|link| link.child.0 != child),
                    "{child}"
                );
                assert!(
                    rebuilt
                        .state
                        .evidence
                        .iter()
                        .all(|held| held.data.evidence_id.0.0 != *record),
                    "{child}"
                );
            }
            assert_eq!(rebuilt.unmatched_line, None);
        }
        let before = snapshot(store.parent().unwrap());
        let replayed = replay_tree(&store, &catalog, None).unwrap();
        assert_eq!(snapshot(store.parent().unwrap()), before, "{which}");
        let order: Vec<String> = tree_status(&store, &catalog)
            .unwrap()
            .cases
            .into_iter()
            .map(|case| case.case)
            .collect();
        let replayed_order: Vec<&str> = replayed
            .cases
            .iter()
            .map(|case| case.case.as_str())
            .collect();
        assert_eq!(replayed_order, order, "{which}");
        others_match(&replayed, "");
        assert_eq!(replay_difference(&replayed), None, "{which}");
    }
    assert!(started.elapsed() < BOUND);
}

#[test]
fn a_revision_moved_outside_the_log_is_a_state_difference() {
    let started = Instant::now();
    let store = Trees::made().copy("wait", "moved-outside");
    let (_, open) = closed_and_open(&store);
    move_outside_the_log(&store, open, "route");
    let replayed = replay_tree(&store, &catalog(&store), None).unwrap();
    let moved = entry(&replayed, open);
    assert_eq!(moved.state_matches_log, Some(false), "{moved:?}");
    assert_eq!(
        moved.case_revision.map(|revision| revision - 1),
        moved.logged_case_revision,
        "{moved:?}"
    );
    assert_eq!(moved.unmatched_line, None, "{moved:?}");
    others_match(&replayed, open);
    assert_eq!(replay_difference(&replayed), Some((open, "state", None)));
    assert!(started.elapsed() < BOUND);
}

#[test]
fn a_record_received_outside_the_log_is_a_decision_difference() {
    let started = Instant::now();
    let store = Trees::made().copy("wait", "received-outside");
    let (_, open) = closed_and_open(&store);
    receive_outside_the_log(&store, open);
    let replayed = replay_tree(&store, &catalog(&store), None).unwrap();
    let received = entry(&replayed, open);
    assert_eq!(received.state_matches_log, Some(true), "{received:?}");
    assert_eq!(received.decision_matches_log, Some(false), "{received:?}");
    others_match(&replayed, open);
    assert_eq!(replay_difference(&replayed), Some((open, "decision", None)));
    assert!(started.elapsed() < BOUND);
}

#[test]
fn a_case_not_yet_decided_is_listed_and_not_a_difference() {
    let started = Instant::now();
    let store = Trees::made().copy("opened", "not-decided");
    assert!(
        !lines(&store, ROOT)
            .iter()
            .any(|line| matches!(line.event, RunLogEvent::Evaluated(_))),
        "tree open writes no evaluated line"
    );
    let replayed = replay_tree(&store, &catalog(&store), None).unwrap();
    assert_eq!(
        replay_json(&replayed),
        json!({"cases": [{"case": ROOT, "evaluated": false}]})
    );
    assert_eq!(replay_difference(&replayed), None);
    assert!(started.elapsed() < BOUND);
}

#[test]
fn a_replay_of_one_case_lists_it_and_its_descendants() {
    let started = Instant::now();
    let store = Trees::made().copy("ended", "one-case");
    let catalog = catalog(&store);
    let order: Vec<String> = tree_status(&store, &catalog)
        .unwrap()
        .cases
        .into_iter()
        .map(|case| case.case)
        .collect();
    assert_eq!(order.len(), 4, "{order:?}");
    let listed = |case: &str| -> Vec<String> {
        replay_tree(&store, &catalog, Some(case))
            .unwrap()
            .cases
            .into_iter()
            .map(|case| case.case)
            .collect()
    };
    assert_eq!(listed(FIRST), [FIRST]);
    assert_eq!(listed(ROOT), order);
    let refused = replay_tree(&store, &catalog, Some("root.nowhere")).unwrap_err();
    assert!(fixture::refused_as(&refused, "CaseNotInTree"), "{refused}");
    assert!(started.elapsed() < BOUND);
}

#[test]
fn the_undo_of_a_logged_move_matches_and_a_foreign_one_differs() {
    let started = Instant::now();
    let moves = Moves::made();
    for (which, case) in [("moved", FIRST), ("refused", ROOT)] {
        let store = moves.copy(which, &format!("undo-{which}"));
        let log = lines(&store, case);
        let after = after_evaluation(&log);
        let finished = after
            .iter()
            .rev()
            .find_map(|line| match &line.event {
                RunLogEvent::ActionFinished(finished) => Some(finished),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{which}: an action finished line after the evaluation"));
        assert_eq!(finished.action, "route.write", "{which}");
        assert_eq!(finished.revisions.len(), 1, "{which}: {finished:?}");
        let replayed = replay_tree(&store, &catalog(&store), None).unwrap();
        others_match(&replayed, "");
    }
    let store = moves.copy("moved", "undo-foreign");
    let log = lines(&store, FIRST);
    let finished = log
        .iter()
        .rev()
        .find(|line| matches!(line.event, RunLogEvent::ActionFinished(_)))
        .unwrap()
        .seq;
    move_outside_the_log(&store, FIRST, "route");
    let replayed = replay_tree(&store, &catalog(&store), None).unwrap();
    let leg = entry(&replayed, FIRST);
    assert_eq!(leg.state_matches_log, Some(false), "{leg:?}");
    assert_eq!(leg.unmatched_line, Some(finished), "{leg:?}");
    assert_eq!(
        replay_difference(&replayed),
        Some((FIRST, "state", Some(finished)))
    );
    assert!(started.elapsed() < BOUND);
}

#[test]
fn a_child_closed_whose_open_precedes_the_last_evaluation_is_named() {
    let started = Instant::now();
    let store = Trees::made().copy("wait", "late-child-closed");
    let (closed, _) = closed_and_open(&store);
    let seq = append_a_late_child_closed(&store, ROOT, closed);
    let root = lines(&store, ROOT);
    // The log verifies and its last `evaluated` line now follows the closed leg's `child opened`.
    let opened_at = root
        .iter()
        .position(|line| {
            matches!(&line.event,
            RunLogEvent::ChildOpened(opened) if opened.child == closed)
        })
        .unwrap();
    assert!(last_evaluated(&root) > opened_at);
    let replayed = replay_tree(&store, &catalog(&store), None).unwrap();
    let parent = entry(&replayed, ROOT);
    assert_eq!(parent.state_matches_log, Some(false), "{parent:?}");
    assert_eq!(parent.unmatched_line, Some(seq), "{parent:?}");
    others_match(&replayed, ROOT);
    assert_eq!(
        replay_difference(&replayed),
        Some((ROOT, "state", Some(seq)))
    );
    assert!(started.elapsed() < BOUND);
}
