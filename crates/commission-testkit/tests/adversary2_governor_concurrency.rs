//! Adversary pass 2 for `story:governor-port`: the fake's call log under concurrent calls.
//!
//! The module documentation says every port call "takes the next answer" and that
//! `FakeGovernor::calls` lists every call "in the order received". `answer` logs a call under the
//! `calls` lock, releases it, and only then takes the `scripts` lock to pop the answer. Two threads
//! can therefore be logged in one order and answered in the other, so the k-th logged call need not
//! be the call that received the k-th answer. This case asserts that it is.

use std::sync::{Arc, Barrier};
use std::thread;

use b10x_commission::model::responsibility::CaseId;
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};

const CALLS_PER_THREAD: i64 = 50_000;

#[test]
fn the_kth_logged_call_received_the_kth_answer() {
    let case = CaseId("case-contended".to_owned());
    let governor = Arc::new(FakeGovernor::new());
    // Answer k (0-based) carries revision k; the script outlasts every call, so no answer repeats.
    governor.script(case.clone(), (0..=2 * CALLS_PER_THREAD).map(Answer::at));

    let start = Arc::new(Barrier::new(2));
    let revisions = {
        let (governor, case, start) = (Arc::clone(&governor), case.clone(), Arc::clone(&start));
        thread::spawn(move || {
            start.wait();
            (0..CALLS_PER_THREAD)
                .map(|_| {
                    governor
                        .current_revision(&case)
                        .unwrap_or_else(|error| panic!("current_revision failed: {error:?}"))
                })
                .collect::<Vec<i64>>()
        })
    };
    let frontiers = {
        let (governor, case, start) = (Arc::clone(&governor), case.clone(), Arc::clone(&start));
        thread::spawn(move || {
            start.wait();
            (0..CALLS_PER_THREAD)
                .map(|_| {
                    governor
                        .frontier(&case)
                        .unwrap_or_else(|error| panic!("frontier failed: {error:?}"))
                        .data()
                        .case_revision
                })
                .collect::<Vec<i64>>()
        })
    };
    let revisions = revisions
        .join()
        .unwrap_or_else(|_| panic!("current_revision thread panicked"));
    let frontiers = frontiers
        .join()
        .unwrap_or_else(|_| panic!("frontier thread panicked"));

    let log = governor.calls();
    assert_eq!(log.len(), (2 * CALLS_PER_THREAD) as usize);
    let (mut next_revision, mut next_frontier) = (revisions.iter(), frontiers.iter());
    let mut mismatches = Vec::new();
    for (position, call) in log.iter().enumerate() {
        let received = match call {
            GovernorCall::CurrentRevision(_) => next_revision.next(),
            GovernorCall::Frontier(_) => next_frontier.next(),
            GovernorCall::Completion(_) => None,
        };
        let received = *received.unwrap_or_else(|| panic!("log entry {position} has no call"));
        if received != position as i64 {
            mismatches.push((position, call.clone(), received));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} logged calls did not receive the answer at their log position; first: {:?}",
        mismatches.len(),
        log.len(),
        mismatches.first()
    );
}
