//! Adversary pass 2 for `story:governor-port`: how `Answer::with_items`, `Answer::complete` and
//! `Answer::unavailable` combine, whether every issued frontier carries the case id and revision of
//! the answer that issued it, and whether the fake can script what the stories that depend on it
//! (`story:stale-revision-action-request`, `story:run-outcomes`) say they script through it.
//! Each combination case names the mutant of `fake_governor.rs` it catches.

use b10x_commission::model::responsibility::{
    ActionStatus, CaseId, CompletionDetermination, CompletionDeterminationComplete, FrontierAction,
    FrontierClaim, FrontierObligation, GovernorError, Truth, Unit,
};
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};

fn case(id: &str) -> CaseId {
    CaseId(id.to_owned())
}

fn complete(outcome: &str) -> CompletionDetermination {
    CompletionDetermination::Complete(CompletionDeterminationComplete {
        outcome: outcome.to_owned(),
    })
}

fn open() -> CompletionDetermination {
    CompletionDetermination::Open(Unit(true))
}

fn action(name: &str, status: ActionStatus) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: None,
        reasons: Vec::new(),
    }
}

fn items() -> (
    Vec<FrontierClaim>,
    Vec<FrontierObligation>,
    Vec<FrontierAction>,
) {
    (
        vec![FrontierClaim {
            claim: "tests.pass".to_owned(),
            value: Truth::Unknown,
        }],
        vec![FrontierObligation {
            obligation: "verify.tests".to_owned(),
            open: true,
        }],
        vec![action("tests.run", ActionStatus::Admissible)],
    )
}

/// Kills: `with_items` rebuilding the reply from `Answer::at(revision)` (dropping a determination
/// set before it). `scripted_frontier_items_are_issued` never combines the two.
#[test]
fn complete_then_with_items_keeps_the_determination_and_the_items() {
    let (claims, obligations, actions) = items();
    let governor = FakeGovernor::new();
    let done = case("case-done");
    governor.script(
        done.clone(),
        [Answer::at(6).complete("X").with_items(
            claims.clone(),
            obligations.clone(),
            actions.clone(),
        )],
    );

    assert_eq!(governor.completion(&done), Ok(complete("X")));
    let frontier = governor
        .frontier(&done)
        .unwrap_or_else(|error| panic!("frontier call failed: {error:?}"));
    assert_eq!(frontier.data().case_id, done);
    assert_eq!(frontier.data().case_revision, 6);
    assert_eq!(frontier.data().claims, claims);
    assert_eq!(frontier.data().obligations, obligations);
    assert_eq!(frontier.data().actions, actions);
    assert_eq!(governor.current_revision(&done), Ok(6));
}

/// Kills: `complete` rebuilding the reply from `Answer::at(revision)` (dropping items set before
/// it).
#[test]
fn with_items_then_complete_keeps_the_items_and_the_determination() {
    let (claims, obligations, actions) = items();
    let governor = FakeGovernor::new();
    let done = case("case-done");
    governor.script(
        done.clone(),
        [Answer::at(6)
            .with_items(claims.clone(), obligations.clone(), actions.clone())
            .complete("X")],
    );

    let frontier = governor
        .frontier(&done)
        .unwrap_or_else(|error| panic!("frontier call failed: {error:?}"));
    assert_eq!(frontier.data().case_revision, 6);
    assert_eq!(frontier.data().claims, claims);
    assert_eq!(frontier.data().obligations, obligations);
    assert_eq!(frontier.data().actions, actions);
    assert_eq!(governor.completion(&done), Ok(complete("X")));
}

/// Kills: `with_items` or `complete` reviving an unavailable answer in either order, on any of the
/// three methods. Pass 1 checked `unavailable().complete(..)` on `completion` only.
#[test]
fn an_unavailable_answer_stays_unavailable_under_every_combination() {
    let (claims, obligations, actions) = items();
    let governor = FakeGovernor::new();
    let down = case("case-down");
    governor.script(
        down.clone(),
        [
            Answer::unavailable()
                .with_items(claims.clone(), obligations.clone(), actions.clone())
                .complete("X"),
            Answer::unavailable().complete("X").with_items(
                claims.clone(),
                obligations.clone(),
                actions.clone(),
            ),
            Answer::unavailable()
                .with_items(claims, obligations, actions)
                .complete("X"),
        ],
    );

    assert_eq!(
        governor.current_revision(&down),
        Err(GovernorError::GovernorUnavailable)
    );
    assert_eq!(
        governor.frontier(&down).err(),
        Some(GovernorError::GovernorUnavailable)
    );
    assert_eq!(
        governor.completion(&down),
        Err(GovernorError::GovernorUnavailable)
    );
    // The last answer keeps answering, still unavailable.
    assert_eq!(
        governor.completion(&down),
        Err(GovernorError::GovernorUnavailable)
    );
}

/// The case id and revision of every issued frontier are those of the call's own answer, across a
/// script that mixes methods, items, completion and failures; and the other methods consume the
/// script between frontier calls.
#[test]
fn every_issued_frontier_carries_its_own_answers_case_and_revision() {
    let (claims, obligations, actions) = items();
    let governor = FakeGovernor::new();
    let a = case("case-a");
    let b = case("case-b");
    governor.script(
        a.clone(),
        [
            Answer::at(10).with_items(claims.clone(), obligations.clone(), actions.clone()),
            Answer::at(11),
            Answer::unavailable(),
            Answer::at(12).complete("Y"),
            Answer::at(13).with_items(Vec::new(), Vec::new(), actions.clone()),
        ],
    );
    governor.script(b.clone(), [Answer::at(10)]);

    let f10 = governor
        .frontier(&a)
        .unwrap_or_else(|error| panic!("frontier 10 failed: {error:?}"));
    let fb = governor
        .frontier(&b)
        .unwrap_or_else(|error| panic!("frontier on b failed: {error:?}"));
    assert_eq!(governor.current_revision(&a), Ok(11));
    assert_eq!(
        governor.frontier(&a).err(),
        Some(GovernorError::GovernorUnavailable)
    );
    let f12 = governor
        .frontier(&a)
        .unwrap_or_else(|error| panic!("frontier 12 failed: {error:?}"));
    let f13 = governor
        .frontier(&a)
        .unwrap_or_else(|error| panic!("frontier 13 failed: {error:?}"));
    let f13_again = governor
        .frontier(&a)
        .unwrap_or_else(|error| panic!("frontier 13 again failed: {error:?}"));

    assert_eq!(
        (f10.data().case_id.clone(), f10.data().case_revision),
        (a.clone(), 10)
    );
    assert_eq!(f10.data().actions, actions);
    assert_eq!(
        (fb.data().case_id.clone(), fb.data().case_revision),
        (b.clone(), 10)
    );
    assert_eq!(fb.data().actions, Vec::new());
    assert_eq!(
        (f12.data().case_id.clone(), f12.data().case_revision),
        (a.clone(), 12)
    );
    assert_eq!(f12.data().actions, Vec::new());
    assert_eq!(
        (f13.data().case_id.clone(), f13.data().case_revision),
        (a.clone(), 13)
    );
    assert_eq!(f13.data().claims, Vec::new());
    assert_eq!(f13.data().actions, actions);
    assert_eq!(f13_again.data().case_revision, 13);
    assert_eq!(f13_again.data().actions, actions);
    assert_ne!(f13.data().frontier_id, f13_again.data().frontier_id);
    assert_eq!(governor.completion(&a), Ok(open()));
}

/// `story:stale-revision-action-request` acceptance 1 and 2, as the fake must be able to script
/// them: a request is built from the frontier at N, the governor moves the case to N+1 where the
/// same action is still admissible, and revalidation reads the current revision and the current
/// frontier.
#[test]
fn the_fake_scripts_a_case_moving_under_a_pending_request() {
    const N: i64 = 20;
    let listed = vec![action("tests.run", ActionStatus::Admissible)];
    let governor = FakeGovernor::new();
    let moving = case("case-moving");
    governor.script(
        moving.clone(),
        [
            Answer::at(N).with_items(Vec::new(), Vec::new(), listed.clone()),
            Answer::at(N + 1).with_items(Vec::new(), Vec::new(), listed.clone()),
        ],
    );

    let chosen_from = governor
        .frontier(&moving)
        .unwrap_or_else(|error| panic!("frontier at N failed: {error:?}"));
    assert_eq!(chosen_from.data().case_revision, N);
    assert_eq!(governor.current_revision(&moving), Ok(N + 1));
    let current = governor
        .frontier(&moving)
        .unwrap_or_else(|error| panic!("frontier at N+1 failed: {error:?}"));
    assert_eq!(current.data().case_revision, N + 1);
    assert_eq!(current.data().actions, listed);
    assert_eq!(
        governor.calls(),
        vec![
            GovernorCall::Frontier(moving.clone()),
            GovernorCall::CurrentRevision(moving.clone()),
            GovernorCall::Frontier(moving),
        ]
    );
}

/// `story:run-outcomes` rows 1, 3, 5 and 6, as the fake must be able to script them: completion
/// with an outcome, an action the frontier marks approval-required, open obligations with no
/// admissible action, and an empty frontier.
#[test]
fn the_fake_scripts_every_frontier_the_run_outcome_rows_read() {
    let governor = FakeGovernor::new();
    let completed = case("case-completed");
    let approval = case("case-approval");
    let evidence = case("case-evidence");
    let nothing = case("case-nothing");
    let gated = FrontierAction {
        action: "repository.merge".to_owned(),
        status: ActionStatus::ApprovalRequired,
        capability: Some("repository.write".to_owned()),
        reasons: vec!["needs a maintainer".to_owned()],
    };
    let obligations = vec![
        FrontierObligation {
            obligation: "verify.tests".to_owned(),
            open: true,
        },
        FrontierObligation {
            obligation: "verify.lint".to_owned(),
            open: false,
        },
    ];
    governor.script(completed.clone(), [Answer::at(1).complete("X")]);
    governor.script(
        approval.clone(),
        [Answer::at(1).with_items(Vec::new(), Vec::new(), vec![gated.clone()])],
    );
    governor.script(
        evidence.clone(),
        [Answer::at(1).with_items(
            Vec::new(),
            obligations.clone(),
            vec![action("repository.merge", ActionStatus::Blocked)],
        )],
    );
    governor.script(nothing.clone(), [Answer::at(1)]);

    assert_eq!(governor.completion(&completed), Ok(complete("X")));
    assert_eq!(
        governor
            .frontier(&approval)
            .map(|frontier| frontier.into_data().actions),
        Ok(vec![gated])
    );
    let blocked = governor
        .frontier(&evidence)
        .unwrap_or_else(|error| panic!("frontier failed: {error:?}"));
    assert_eq!(blocked.data().obligations, obligations);
    assert!(
        blocked
            .data()
            .actions
            .iter()
            .all(|listed| listed.status != ActionStatus::Admissible)
    );
    assert_eq!(blocked.data().actions.len(), 1);
    let empty = governor
        .frontier(&nothing)
        .unwrap_or_else(|error| panic!("frontier failed: {error:?}"));
    assert_eq!(
        (
            empty.data().claims.len(),
            empty.data().obligations.len(),
            empty.data().actions.len()
        ),
        (0, 0, 0)
    );
    assert_eq!(governor.completion(&nothing), Ok(open()));
}
