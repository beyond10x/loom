//! Adversary pass 2 on `story:stale-revision-action-request`: the two revision comparisons of
//! `revalidate`, and the order of its stale and other-case checks.
//!
//! `crates/commission-testkit/tests/action_request.rs` only ever moves a case forward, and always
//! lets the revision call and the frontier call agree on staleness. So `current > expected` in
//! place of `current != expected` (or the same on the frontier's revision) survives it. It also
//! drives a frontier for another case only at the request's own revision, so it never sees which of
//! the stale check and the other-case check wins.

use b10x_commission::action_request::{request, revalidate};
use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionRequest, ActionRequestId, ActionRequestStale, ActionStatus, CaseId,
    CompletionDetermination, ExecutorOutcomeProposedAction, Frontier, FrontierAction,
    GovernorError, ProposedActionArguments, RevalidateActionRequestOutcome, RunId,
    action_request_state, frontier_state,
};
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};

const CASE: &str = "case-adversary2-revalidation";
const OTHER: &str = "case-adversary2-other";
const N: i64 = 20;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn at(revision: i64) -> Answer {
    Answer::at(revision).with_items(
        Vec::new(),
        Vec::new(),
        vec![FrontierAction {
            action: "merge".to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    )
}

/// A request for `merge` built from the frontier `governor` issues now for this case.
fn request_from<G: Governor>(governor: &G) -> ActionRequest<action_request_state::Requested> {
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

fn stale(expected: i64, current: i64) -> RevalidateActionRequestOutcome {
    RevalidateActionRequestOutcome::Stale {
        error: ActionRequestStale {
            expected_case_revision: expected,
            current_case_revision: current,
        },
    }
}

/// The request was made at N+1; the governor now reports N (the case was reset, or a replica is
/// behind) while its frontier is still issued for N+1. Not current means stale, in either
/// direction, and it is decided from the revision call alone. Kills `current > expected`.
#[test]
fn adversary2_request_revision_behind_the_request_is_stale() {
    let governor = FakeGovernor::new();
    // Built from the first answer; revalidation reads the second (revision) and would read the
    // third (frontier).
    governor.script(case(), [at(N + 1), at(N), at(N + 1)]);
    let request = request_from(&governor);
    assert_eq!(request.data().expected_case_revision, N + 1, "precondition");

    let before = governor.calls().len();
    let outcome = revalidate(&governor, &request)
        .unwrap_or_else(|error| panic!("revalidation failed at the governor: {error:?}"));
    assert_eq!(outcome, stale(N + 1, N), "a revision behind the request's");
    assert_eq!(
        governor.calls()[before..],
        [GovernorCall::CurrentRevision(case())],
        "stale is decided from the current revision, before any frontier is read"
    );
}

/// The revision call agrees with the request (N+1), but the frontier is issued for N. The request
/// is stale against that frontier. Kills `issued.case_revision > expected`.
#[test]
fn adversary2_request_frontier_behind_the_request_is_stale() {
    let governor = FakeGovernor::new();
    governor.script(case(), [at(N + 1), at(N + 1), at(N)]);
    let request = request_from(&governor);

    let outcome = revalidate(&governor, &request)
        .unwrap_or_else(|error| panic!("revalidation failed at the governor: {error:?}"));
    assert_eq!(
        outcome,
        stale(N + 1, N),
        "a frontier issued for an older revision"
    );
}

/// A governor that issues every frontier for `OTHER`, whatever case it is asked about.
struct OtherCaseGovernor(FakeGovernor);

impl Governor for OtherCaseGovernor {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.0.current_revision(case)
    }

    fn frontier(&self, _case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        self.0.frontier(&CaseId(OTHER.to_owned()))
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.0.completion(case)
    }
}

/// `crates/commission/src/action_request.rs:15-16`: "A frontier issued for another case is refused
/// the same way [not admitted], with a reason saying so." The other case is at revision 3, this one
/// at N. The answer must be not admitted naming both cases — not stale, and above all not a stale
/// refusal whose `current_case_revision` is another case's revision, which reads as a fact about
/// this case that the governor never reported.
#[test]
fn adversary2_request_other_case_frontier_at_another_revision_is_not_admitted() {
    let fake = FakeGovernor::new();
    fake.script(case(), [at(N)]);
    fake.script(CaseId(OTHER.to_owned()), [at(3)]);
    let request = request_from(&fake);
    assert_eq!(request.data().expected_case_revision, N, "precondition");

    let outcome = revalidate(&OtherCaseGovernor(fake), &request)
        .unwrap_or_else(|error| panic!("revalidation failed at the governor: {error:?}"));
    match outcome {
        RevalidateActionRequestOutcome::NotAdmitted { error } => {
            assert_eq!(error.action, "merge", "refused action");
            assert!(
                error.reasons.len() == 1
                    && error.reasons[0].contains(OTHER)
                    && error.reasons[0].contains(CASE),
                "the reason does not name both cases: {:?}",
                error.reasons
            );
        }
        other => panic!(
            "a frontier for another case, at that case's revision 3, must be refused as not \
             admitted (action_request.rs:15-16); got {other:?}"
        ),
    }
}
