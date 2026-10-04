//! Adversary pass 1 on `story:stale-revision-action-request`: a governor that cannot answer fails
//! revalidation with its own typed error, at whichever of the two calls it fails, and never reaches
//! an outcome — least of all `Admitted`.
//!
//! `crates/commission-testkit/tests/action_request.rs` never drives a failing governor, so a
//! revalidation that read a failed revision as the expected one, or mapped every error to one
//! variant, passed it.

use b10x_commission::action_request::{request, revalidate};
use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionRequest, ActionRequestData, ActionRequestId, ActionStatus, CaseId,
    CompletionDetermination, ExecutorOutcomeProposedAction, Frontier, FrontierAction,
    GovernorError, ProposedActionArguments, RunId, action_request_state, frontier_state,
};
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};

const CASE: &str = "case-adversary-governor-errors";
const N: i64 = 11;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn merge_admissible() -> Vec<FrontierAction> {
    vec![FrontierAction {
        action: "merge".to_owned(),
        status: ActionStatus::Admissible,
        capability: None,
        reasons: Vec::new(),
    }]
}

fn at(revision: i64) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), merge_admissible())
}

/// A request for `merge` built from the frontier the governor issues now.
fn request_from(governor: &FakeGovernor) -> ActionRequest<action_request_state::Requested> {
    let frontier = governor
        .frontier(&case())
        .unwrap_or_else(|error| panic!("frontier call failed: {error:?}"));
    request(
        ActionRequestId(uuid(1)),
        RunId(uuid(2)),
        &frontier,
        ExecutorOutcomeProposedAction {
            action: "merge".to_owned(),
            arguments: ProposedActionArguments(Value::Null),
        },
    )
}

/// A case the governor does not hold fails as `UnknownCase`, from the revision call, and no
/// frontier is asked for.
#[test]
fn adversary_request_unknown_case_fails_with_unknown_case() {
    let governor = FakeGovernor::new();
    let unheld = ActionRequest::new(ActionRequestData {
        action_request_id: ActionRequestId(uuid(3)),
        run_id: RunId(uuid(2)),
        case_id: case(),
        expected_case_revision: N,
        action: "merge".to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    });
    assert_eq!(
        revalidate(&governor, &unheld),
        Err(GovernorError::UnknownCase),
        "a case the governor does not hold"
    );
    assert_eq!(
        governor.calls(),
        [GovernorCall::CurrentRevision(case())],
        "a failed revision call is the end of revalidation"
    );
}

/// The revision call fails; the frontier the governor would issue next admits the action at the
/// request's revision. Revalidation fails with `GovernorUnavailable` — it does not go on to the
/// frontier and admit.
#[test]
fn adversary_request_unavailable_revision_never_admits() {
    let governor = FakeGovernor::new();
    // Built from the first answer; revalidation's revision call takes the second (unavailable),
    // and a frontier call, if one were made, would take the third: admissible at N.
    governor.script(case(), [at(N), Answer::unavailable(), at(N)]);
    let at_n = request_from(&governor);

    let calls_before = governor.calls().len();
    assert_eq!(
        revalidate(&governor, &at_n),
        Err(GovernorError::GovernorUnavailable),
        "the governor could not say the case's current revision"
    );
    assert_eq!(
        governor.calls()[calls_before..],
        [GovernorCall::CurrentRevision(case())],
        "no frontier is read after the revision call failed"
    );
}

/// The revision call answers N, the frontier call fails: `GovernorUnavailable`, not an outcome.
#[test]
fn adversary_request_unavailable_frontier_fails_with_unavailable() {
    let governor = FakeGovernor::new();
    governor.script(case(), [at(N), at(N), Answer::unavailable()]);
    let at_n = request_from(&governor);

    let calls_before = governor.calls().len();
    assert_eq!(
        revalidate(&governor, &at_n),
        Err(GovernorError::GovernorUnavailable),
        "the governor could not issue the current frontier"
    );
    assert_eq!(
        governor.calls()[calls_before..],
        [
            GovernorCall::CurrentRevision(case()),
            GovernorCall::Frontier(case())
        ],
        "revision first, then the frontier that failed"
    );
}

/// A governor that knows the case's revision and then, at the frontier call, no longer holds the
/// case: its `UnknownCase` reaches the caller as `UnknownCase`, not as another variant.
struct ForgetsAtFrontier;

impl Governor for ForgetsAtFrontier {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(N)
    }

    fn frontier(&self, _case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        Err(GovernorError::UnknownCase)
    }

    fn completion(&self, _case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        Err(GovernorError::UnknownCase)
    }
}

#[test]
fn adversary_request_unknown_case_at_frontier_keeps_its_variant() {
    let at_n = ActionRequest::new(ActionRequestData {
        action_request_id: ActionRequestId(uuid(4)),
        run_id: RunId(uuid(2)),
        case_id: case(),
        expected_case_revision: N,
        action: "merge".to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    });
    assert_eq!(
        revalidate(&ForgetsAtFrontier, &at_n),
        Err(GovernorError::UnknownCase),
        "the frontier call's own error"
    );
}
