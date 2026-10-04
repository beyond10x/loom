//! Adversary pass 1 on `story:local-runtime-loop` (wave 2026-10-04-w7).
//!
//! Red cases:
//!
//! * `request_is_made_against_its_runs_case_revision`: `ess/domains/responsibility.yaml` says of
//!   `Run.case_revision` "a proposal made on another revision is stale" and of `Run.requests` "a
//!   request is made inside one run, against that run's case revision". The loop keeps one Run
//!   across a case revision change and admits a request at the new revision inside it.
//! * `governor_failure_mid_iteration_leaves_no_unnamed_running_run`: a governor failure after the
//!   Run started and an observation was delivered returns an error that names no run, and the Run
//!   stays `Running` in the store.
//! * `bound_counts_the_frontier_the_executor_saw`: the bound is keyed on the loaded revision, so a
//!   case that moves between the load and the frontier read lets the executor run three times on
//!   frontiers of one unchanged revision.
//!
//! Green probes (the implementor's untested choices, pinned): authority allow, deny and provider
//! failure; completion found at the bound; `NeedsHumanJudgment`.

use std::sync::{Mutex, PoisonError};

use b10x_commission::model::behaviour::{Generated, RunStorage};
use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::{Timestamp, Uuid};
use b10x_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictDeny, CaseId, Commission, CommissionData, CommissionId, ExecutorOutcome,
    ExecutorOutcomeNeedsHumanJudgment, ExecutorOutcomeProposedAction, Frontier, FrontierAction,
    HumanDecisionRequest, ObservationId, PrincipalId, ProposedActionArguments,
    RevalidateActionRequestOutcome, RunId, RunOutcome, RunOutcomeCompleted,
    RunOutcomeNeedsHumanJudgment, RunState, Unit, commission_state, frontier_state,
};
use b10x_commission::outcome::RunStore;
use b10x_commission::ports::executor::AgentExecutor;
use b10x_commission::runtime::{LoopContext, LoopEnd, LoopError, run_until_blocked};
use b10x_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x51)),
        agent_revision_id: AgentRevisionId(uuid(0x52)),
        case_id: case.clone(),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn action(name: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

fn listing(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

fn inspect_only(revision: i64) -> Answer {
    listing(
        revision,
        vec![action("inspect", ActionStatus::Admissible, None)],
    )
}

fn deploy_needs(revision: i64, others: Vec<FrontierAction>) -> Answer {
    let mut actions = vec![action(
        "deploy",
        ActionStatus::ApprovalRequired,
        Some("prod.deploy"),
    )];
    actions.extend(others);
    listing(revision, actions)
}

fn proposal(name: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    })
}

#[derive(Debug, Default)]
struct Context {
    requests: u64,
    observations: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x300 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x400 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-04T12:00:00Z".to_owned())
    }
}

fn store() -> Generated<RunStore> {
    let mut issued = 0u64;
    Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }))
}

fn run<E: AgentExecutor>(
    governor: &FakeGovernor,
    executor: &E,
    authority: &StaticAuthorityProvider,
    commission: &Commission<commission_state::Assigned>,
    runs: &mut Generated<RunStore>,
) -> Result<LoopEnd, LoopError> {
    run_until_blocked(
        governor,
        executor,
        authority,
        commission,
        runs,
        &mut Context::default(),
    )
}

/// RED. The case is at 7 when the loop starts its Run; the proposal made on revision 7 is stale
/// because the case moved to 8; the loop reads again and admits `inspect` at 8 inside the Run that
/// started against 7. The specification says a request is made against its run's case revision.
#[test]
fn request_is_made_against_its_runs_case_revision() {
    let case = CaseId("case-moves".to_owned());
    let commission = commission(&case);
    let governor = FakeGovernor::new();
    // load, completion, frontier at 7; then 8 for revalidation and the next iteration with its
    // revalidation (7 calls); then complete.
    governor.script(
        case.clone(),
        std::iter::repeat_n(inspect_only(7), 3)
            .chain(std::iter::repeat_n(inspect_only(8), 7))
            .chain([inspect_only(8).complete("Y")]),
    );
    let executor = ScriptedExecutor::new([proposal("inspect"), proposal("inspect")]);
    let authority = StaticAuthorityProvider::new();
    let mut runs = store();

    let end = run(&governor, &executor, &authority, &commission, &mut runs)
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    // Coordinator decision F1 (adversary pass 1): when the governor's current revision differs
    // from the Run's case_revision, the loop admits nothing more and ends with
    // NoAdmissibleAction, since the Run's revision has no admissible action left. It ends at the
    // revision change, before the scripted completion is reached.
    assert_eq!(
        end.outcome,
        RunOutcome::NoAdmissibleAction(Unit(true)),
        "decision F1: the loop ends at the revision change"
    );
    let held = RunStorage::get(&runs.ports, &end.run_id)
        .unwrap_or_else(|| panic!("the loop's run is not stored"));
    assert_eq!(held.data.case_revision, 7, "fixture: the Run started at 7");
    for admitted in &end.admitted {
        assert_eq!(
            admitted.expected_case_revision,
            held.data.case_revision,
            "ess Run.case_revision: a proposal made on another revision than the run's is stale; \
             request {} was admitted at revision {} inside run {} started at {}",
            admitted.action_request_id.0.0,
            admitted.expected_case_revision,
            end.run_id.0.0,
            held.data.case_revision
        );
    }
}

/// RED. The governor answers the iteration's reads, the executor proposes, the observation is
/// delivered, and the governor then fails the revalidation. The loop returns an error. Either the
/// error names the Run the loop started, or that Run is not left `Running`: otherwise a Run stays
/// `Running` in the store that no return value identifies, and a retry starts a second one.
#[test]
fn governor_failure_mid_iteration_leaves_no_unnamed_running_run() {
    let case = CaseId("case-fails".to_owned());
    let commission = commission(&case);
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        std::iter::repeat_n(inspect_only(5), 3).chain([Answer::unavailable()]),
    );
    let executor = ScriptedExecutor::new([proposal("inspect")]);
    let authority = StaticAuthorityProvider::new();
    let mut runs = store();

    let error = match run(&governor, &executor, &authority, &commission, &mut runs) {
        Ok(end) => panic!("fixture: the governor fails revalidation, got {end:?}"),
        Err(error) => error,
    };

    assert_eq!(
        governor.observations().len(),
        1,
        "partial effect: the step's observation was delivered before the failure"
    );
    let stored = RunStorage::list(&runs.ports);
    assert_eq!(stored.len(), 1, "the loop started one Run");
    let started = &stored[0];
    let named = format!("{error:?}").contains(&started.data.run_id.0.0);
    assert!(
        named || started.state != RunState::Running,
        "after {error:?} the Run {} is left {:?} and the error does not name it",
        started.data.run_id.0.0,
        started.state
    );
}

/// An executor that answers `NoUsefulAction` to every call and notes the revision of each frontier
/// it was given.
#[derive(Default)]
struct Idle {
    seen: Mutex<Vec<i64>>,
}

impl AgentExecutor for Idle {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        seen.push(frontier.data().case_revision);
        assert!(seen.len() <= 10, "unbounded: {seen:?}");
        ExecutorOutcome::NoUsefulAction(Unit(true))
    }
}

/// RED (note). The case moves from 5 to 6 between the first iteration's load and its frontier
/// read, then stays at 6. Every frontier the executor sees is of revision 6, yet it is asked three
/// times: the bound compares loaded revisions, not the frontier the executor saw. The module says
/// the loop is bounded "where the frontier does not change", after two idle iterations.
#[test]
fn bound_counts_the_frontier_the_executor_saw() {
    let case = CaseId("case-shifts".to_owned());
    let commission = commission(&case);
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [inspect_only(5), inspect_only(5), inspect_only(6)],
    );
    let executor = Idle::default();
    let authority = StaticAuthorityProvider::new();
    let mut runs = store();

    let end = run(&governor, &executor, &authority, &commission, &mut runs)
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    assert_eq!(end.outcome, RunOutcome::NoAdmissibleAction(Unit(true)));
    let seen = executor
        .seen
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    assert!(
        seen.iter().filter(|&&revision| revision == 6).count() <= 2,
        "the executor ran {} times on frontiers of unchanged revision 6: {seen:?}",
        seen.len()
    );
}

/// GREEN probe. Allow: the request is recorded as admitted with its `NeedsAuthority`
/// revalidation, the provider is asked once, and the case then completes.
#[test]
fn authority_allow_admits_the_request() {
    let case = CaseId("case-allow".to_owned());
    let commission = commission(&case);
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        std::iter::repeat_n(deploy_needs(4, Vec::new()), 6)
            .chain([deploy_needs(4, Vec::new()).complete("X")]),
    );
    let executor = ScriptedExecutor::new([proposal("deploy")]);
    let authority =
        StaticAuthorityProvider::new().answer("prod.deploy", AuthorityVerdict::Allow(Unit(true)));
    let mut runs = store();

    let end = run(&governor, &executor, &authority, &commission, &mut runs)
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    assert_eq!(authority.asked().len(), 1);
    assert_eq!(end.requests.len(), 1);
    assert!(matches!(
        end.requests[0].outcome,
        RevalidateActionRequestOutcome::NeedsAuthority { .. }
    ));
    assert_eq!(end.admitted, vec![end.requests[0].request.clone()]);
    assert_eq!(
        end.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "X".to_owned()
        })
    );
}

/// GREEN probe. Deny, with another action admissible: nothing admitted, the provider asked per
/// proposal, and the bound ends the run after two denied iterations.
#[test]
fn authority_deny_admits_nothing_and_is_bounded() {
    let case = CaseId("case-deny".to_owned());
    let commission = commission(&case);
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [deploy_needs(
            4,
            vec![action("inspect", ActionStatus::Admissible, None)],
        )],
    );
    let executor = ScriptedExecutor::new([proposal("deploy"), proposal("deploy")]);
    let authority = StaticAuthorityProvider::new().answer(
        "prod.deploy",
        AuthorityVerdict::Deny(AuthorityVerdictDeny {
            reason: "no".to_owned(),
        }),
    );
    let mut runs = store();

    let end = run(&governor, &executor, &authority, &commission, &mut runs)
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    assert_eq!(authority.asked().len(), 2);
    assert!(end.admitted.is_empty(), "denied yet admitted: {end:?}");
    assert_eq!(end.outcome, RunOutcome::NoAdmissibleAction(Unit(true)));
}

/// GREEN probe. Provider failure, the action needing authority being the frontier's only one:
/// nothing admitted, never an allow, the run ends from the frontier.
#[test]
fn authority_failure_admits_nothing() {
    let case = CaseId("case-provider-down".to_owned());
    let commission = commission(&case);
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [deploy_needs(4, Vec::new())]);
    let executor = ScriptedExecutor::new([proposal("deploy")]);
    let authority = StaticAuthorityProvider::new().fail("prod.deploy", "down");
    let mut runs = store();

    let end = run(&governor, &executor, &authority, &commission, &mut runs)
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    assert_eq!(authority.asked().len(), 1);
    assert!(end.admitted.is_empty(), "admitted on a failure: {end:?}");
    assert_eq!(end.outcome, RunOutcome::NoAdmissibleAction(Unit(true)));
}

/// GREEN probe. One idle iteration at 6, then the governor holds the case complete: the run ends
/// completed, not with no admissible action.
#[test]
fn completion_at_the_bound_wins() {
    let case = CaseId("case-done-at-bound".to_owned());
    let commission = commission(&case);
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        std::iter::repeat_n(inspect_only(6), 3).chain([inspect_only(6).complete("Z")]),
    );
    let executor = ScriptedExecutor::new([ExecutorOutcome::NoUsefulAction(Unit(true))]);
    let authority = StaticAuthorityProvider::new();
    let mut runs = store();

    let end = run(&governor, &executor, &authority, &commission, &mut runs)
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    assert_eq!(
        end.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "Z".to_owned()
        })
    );
}

/// GREEN probe. `NeedsHumanJudgment` ends the run carrying its request, and the observation
/// carries it too.
#[test]
fn needs_human_judgment_ends_the_run() {
    let case = CaseId("case-human".to_owned());
    let commission = commission(&case);
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [inspect_only(3)]);
    let request = HumanDecisionRequest(Value::Text("which?".to_owned()));
    let executor = ScriptedExecutor::new([ExecutorOutcome::NeedsHumanJudgment(
        ExecutorOutcomeNeedsHumanJudgment {
            request: request.clone(),
        },
    )]);
    let authority = StaticAuthorityProvider::new();
    let mut runs = store();

    let end = run(&governor, &executor, &authority, &commission, &mut runs)
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    assert_eq!(
        end.outcome,
        RunOutcome::NeedsHumanJudgment(RunOutcomeNeedsHumanJudgment {
            request: request.clone()
        })
    );
    assert_eq!(
        governor.observations()[0].payload.member("request"),
        Some(&request.0)
    );
}
