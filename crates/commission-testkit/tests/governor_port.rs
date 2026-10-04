//! Acceptance for `story:governor-port`: the `Governor` port returns, for a commissioned case, the
//! case's current revision, the frontier issued for that revision and the governor's completion
//! determination, and fails with the typed `GovernorError`. It runs against the scripted fake
//! governor, whose script answers each call in turn and can make a call fail.
//!
//! The frontiers in `governor_port_contract` carry no claims, obligations or actions:
//! `story:frontier-admission` may change those item types, and that test neither builds nor reads
//! them. `scripted_frontier_items_are_issued` does, because later stories script listed actions
//! and obligations through the fake.

use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, CompletionDetermination, CompletionDeterminationComplete, FrontierAction,
    FrontierClaim, FrontierObligation, FrontierState, GovernorError, PrincipalId, Truth, Unit,
};
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};

/// A commission of some agent revision to `case`. The port answers per case, whichever commission
/// asks, so the test reaches the case through the commission that holds it.
fn commission_to(case: &str) -> CommissionData {
    Commission::new(CommissionData {
        commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000001".to_owned())),
        agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000002".to_owned())),
        case_id: CaseId(case.to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
    .into_data()
}

#[test]
fn governor_port_contract() {
    const N: i64 = 7;

    let moving = commission_to("case-moving");
    let finished = commission_to("case-finished");
    let flaky = commission_to("case-flaky");

    let governor = FakeGovernor::new();
    governor.script(moving.case_id.clone(), [Answer::at(N), Answer::at(N + 1)]);
    governor.script(finished.case_id.clone(), [Answer::at(3).complete("X")]);
    governor.script(
        flaky.case_id.clone(),
        [
            Answer::unavailable(),
            Answer::unavailable(),
            Answer::unavailable(),
            Answer::at(5),
        ],
    );

    // Expectation 1: the script moves the case from N to N+1 between two calls, and each frontier
    // carries the case id and the revision it was issued for.
    let first = governor
        .frontier(&moving.case_id)
        .unwrap_or_else(|error| panic!("first frontier call failed: {error:?}"));
    assert_eq!(first.state(), FrontierState::Issued);
    assert_eq!(first.data().case_id, moving.case_id);
    assert_eq!(first.data().case_revision, N);

    let second = governor
        .frontier(&moving.case_id)
        .unwrap_or_else(|error| panic!("second frontier call failed: {error:?}"));
    assert_eq!(second.data().case_id, moving.case_id);
    assert_eq!(second.data().case_revision, N + 1);
    assert_eq!(governor.current_revision(&moving.case_id), Ok(N + 1));

    // Expectation 2: a case the script marks complete with outcome `X` carries `X`; a case not
    // marked complete is open.
    assert_eq!(
        governor.completion(&finished.case_id),
        Ok(CompletionDetermination::Complete(
            CompletionDeterminationComplete {
                outcome: "X".to_owned(),
            }
        ))
    );
    assert_eq!(
        governor.completion(&moving.case_id),
        Ok(CompletionDetermination::Open(Unit(true)))
    );

    // Expectation 3: a case the fake does not hold is the typed unknown-case error on every call.
    let stranger = CaseId("case-not-held".to_owned());
    assert_eq!(
        governor.current_revision(&stranger),
        Err(GovernorError::UnknownCase)
    );
    assert_eq!(
        governor.frontier(&stranger).err(),
        Some(GovernorError::UnknownCase)
    );
    assert_eq!(
        governor.completion(&stranger),
        Err(GovernorError::UnknownCase)
    );

    // Expectation 4: each call the script makes fail is the typed governor-unavailable error, and
    // the failure is that call's alone: the next answer is served.
    assert_eq!(
        governor.current_revision(&flaky.case_id),
        Err(GovernorError::GovernorUnavailable)
    );
    assert_eq!(
        governor.frontier(&flaky.case_id).err(),
        Some(GovernorError::GovernorUnavailable)
    );
    assert_eq!(
        governor.completion(&flaky.case_id),
        Err(GovernorError::GovernorUnavailable)
    );
    let recovered = governor
        .frontier(&flaky.case_id)
        .unwrap_or_else(|error| panic!("call after the scripted failures failed: {error:?}"));
    assert_eq!(recovered.data().case_revision, 5);
}

/// The script sets the frontier's contents per call: the frontier issued for an answer holds
/// exactly the claims, obligations and actions that answer carries, and an answer that carries
/// none issues an empty frontier.
#[test]
fn scripted_frontier_items_are_issued() {
    let claims = vec![FrontierClaim {
        claim: "tests.pass".to_owned(),
        value: Truth::Unknown,
    }];
    let obligations = vec![FrontierObligation {
        obligation: "verify.tests".to_owned(),
        open: true,
    }];
    let actions = vec![
        FrontierAction {
            action: "tests.run".to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        },
        FrontierAction {
            action: "repository.merge".to_owned(),
            status: ActionStatus::Blocked,
            capability: Some("repository.write".to_owned()),
            reasons: vec!["tests.pass is unknown".to_owned()],
        },
    ];

    let listed = CaseId("case-listed".to_owned());
    let down = CaseId("case-down".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        listed.clone(),
        [
            Answer::at(4).with_items(claims.clone(), obligations.clone(), actions.clone()),
            Answer::at(5),
        ],
    );
    governor.script(
        down.clone(),
        [Answer::unavailable().with_items(claims.clone(), obligations.clone(), actions.clone())],
    );

    let first = governor
        .frontier(&listed)
        .unwrap_or_else(|error| panic!("first frontier call failed: {error:?}"));
    assert_eq!(first.data().case_revision, 4);
    assert_eq!(first.data().claims, claims);
    assert_eq!(first.data().obligations, obligations);
    assert_eq!(first.data().actions, actions);

    let second = governor
        .frontier(&listed)
        .unwrap_or_else(|error| panic!("second frontier call failed: {error:?}"));
    assert_eq!(second.data().case_revision, 5);
    assert_eq!(second.data().claims, Vec::new());
    assert_eq!(second.data().obligations, Vec::new());
    assert_eq!(second.data().actions, Vec::new());

    assert_eq!(
        governor.frontier(&down).err(),
        Some(GovernorError::GovernorUnavailable)
    );
}

/// The fake logs every call it receives, in order, with its method and case — failed calls
/// included.
#[test]
fn the_fake_logs_every_call_in_order() {
    let held = CaseId("case-held".to_owned());
    let stranger = CaseId("case-not-held".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        held.clone(),
        [Answer::at(1), Answer::unavailable(), Answer::at(2)],
    );
    assert_eq!(governor.calls(), Vec::new());

    assert_eq!(governor.current_revision(&held), Ok(1));
    assert_eq!(
        governor.frontier(&held).err(),
        Some(GovernorError::GovernorUnavailable)
    );
    assert_eq!(
        governor.completion(&stranger),
        Err(GovernorError::UnknownCase)
    );
    assert_eq!(
        governor
            .frontier(&held)
            .map(|frontier| frontier.data().case_revision),
        Ok(2)
    );
    assert_eq!(
        governor.completion(&held),
        Ok(CompletionDetermination::Open(Unit(true)))
    );

    assert_eq!(
        governor.calls(),
        vec![
            GovernorCall::CurrentRevision(held.clone()),
            GovernorCall::Frontier(held.clone()),
            GovernorCall::Completion(stranger),
            GovernorCall::Frontier(held.clone()),
            GovernorCall::Completion(held),
        ]
    );
}
