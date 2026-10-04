//! Acceptance for `story:local-runtime-loop`: `runtime::run_until_blocked` drives one commission
//! over the scripted fake governor, the scripted fake executor and the static fake authority
//! provider until its run ends, and executes no effect.
//!
//! One call is one loop, and one loop starts one Run of the commission (`StartRun`) at the case
//! revision its first iteration loaded. Each iteration makes these calls, in this order:
//!
//! 1. `Governor::current_revision`: the case is loaded;
//! 2. `Governor::completion`: a case the governor holds complete ends the run completed, carrying
//!    the governor's outcome, and the iteration makes no further call;
//! 3. `Governor::frontier`: the frontier is obtained;
//! 4. `AgentExecutor::run` on that frontier. The step is reported to the observation port as one
//!    `Observation`: its id and time come from the loop's context, its source is `executor`, its
//!    subject is `<case>@<frontier revision>`, and a proposal's payload carries its `action` and
//!    `arguments`. It is never evidence;
//! 5. on `Suspended`, the Run is suspended through `SuspendRun` and the run ends suspended,
//!    carrying the executor's reason;
//! 6. on `ProposedAction`, the proposal becomes an action request bound to the frontier's revision
//!    (`action_request::request`, its id from the context) and is revalidated
//!    (`action_request::revalidate`: `current_revision`, then `frontier` unless already stale).
//!    Admitted: the request is recorded as admitted, and the loop reads the frontier again. Stale:
//!    the loop reads the frontier again. Needs authority: the provider is asked for the capability
//!    the frontier names. Otherwise the run outcome is derived (`outcome::derive`);
//! 7. otherwise the run outcome is derived.
//!
//! Every request is recorded with its revalidation outcome, admitted or not; no request is
//! executed. Commission has no effect port yet (commission `story:effect-invocation`).
//!
//! The loop is bounded where the frontier does not change (wave 2026-10-04-w5, run-outcomes F4):
//! two iterations in a row that admit no request end the run with no admissible action, and an
//! admitted request starts the count again, unless the same action and arguments were already
//! admitted in this Run, which counts as admitting nothing. A stale revalidation does not count;
//! the next iteration's load ends the run (adversary pass 2, F-A and F-B). It covers
//! `NoUsefulAction`, `CompletedLocalReasoning` and a refused proposal alike. A Run is bound to the case revision it started against
//! (`ess/domains/responsibility.yaml:390`): when the case moves to another revision, the loop
//! admits nothing more and ends with no admissible action, unless the governor holds the case
//! complete (coordinator decision F1, adversary pass 1).
//!
//! The fake governor takes one scripted answer per call, whichever port method is called, so each
//! script below counts calls in the order above.

use std::iter::repeat_n;
use std::sync::{Mutex, PoisonError};

use b10x_commission::model::behaviour::{Generated, RunStorage};
use b10x_commission::model::json::{self, Value};
use b10x_commission::model::primitives::{Timestamp, Uuid};
use b10x_commission::model::responsibility::obligations::RunStatesQuery;
use b10x_commission::model::responsibility::{
    ActionNeedsAuthority, ActionNotAdmitted, ActionRequestData, ActionRequestId,
    ActionRequestStale, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData, CommissionId,
    ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorOutcomeSuspended, Frontier,
    FrontierAction, FrontierId, ObservationId, PrincipalId, ProposedActionArguments,
    RevalidateActionRequestOutcome, RunId, RunOutcome, RunOutcomeCompleted,
    RunOutcomeNeedsAuthority, RunOutcomeSuspended, RunState, RunStates, SuspensionReason, Unit,
    commission_state, frontier_state,
};
use b10x_commission::outcome::RunStore;
use b10x_commission::ports::executor::AgentExecutor;
use b10x_commission::runtime::{LoopContext, LoopEnd, Revalidated, run_until_blocked};
use b10x_commission_testkit::fake_authority::{AuthorityQuery, StaticAuthorityProvider};
use b10x_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};

/// The trusted time the loop's context gives.
const NOW: &str = "2026-10-04T12:00:00Z";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(n: u64, case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(n)),
        agent_revision_id: AgentRevisionId(uuid(0x20 + n)),
        case_id: case.clone(),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn admissible(name: &str) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status: ActionStatus::Admissible,
        capability: None,
        reasons: Vec::new(),
    }
}

/// The case open at `revision`, its frontier listing exactly `actions`.
fn listing(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

fn proposal(name: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    })
}

/// The request the loop makes for a proposal of `action` with no arguments: the `n`-th request id
/// of its context, its run, the case and the revision of the frontier it was chosen from.
fn request(n: u64, run: &RunId, case: &CaseId, revision: i64, action: &str) -> ActionRequestData {
    ActionRequestData {
        action_request_id: ActionRequestId(uuid(0x300 + n)),
        run_id: run.clone(),
        case_id: case.clone(),
        expected_case_revision: revision,
        action: action.to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    }
}

/// The governor calls an iteration makes before it runs the executor: load, completion, frontier.
fn iteration(case: &CaseId) -> Vec<GovernorCall> {
    vec![
        GovernorCall::CurrentRevision(case.clone()),
        GovernorCall::Completion(case.clone()),
        GovernorCall::Frontier(case.clone()),
    ]
}

/// The governor calls of a revalidation that is not stale.
fn revalidation(case: &CaseId) -> Vec<GovernorCall> {
    vec![
        GovernorCall::CurrentRevision(case.clone()),
        GovernorCall::Frontier(case.clone()),
    ]
}

/// The governor calls of an iteration that finds the case complete.
fn completed(case: &CaseId) -> Vec<GovernorCall> {
    vec![
        GovernorCall::CurrentRevision(case.clone()),
        GovernorCall::Completion(case.clone()),
    ]
}

/// The scripted fake executor, noting the length of the fake governor's call log at each of its
/// calls: the order across the two fakes, which neither log holds alone.
struct Sequenced<'g> {
    governor: &'g FakeGovernor,
    script: ScriptedExecutor,
    at: Mutex<Vec<usize>>,
}

impl<'g> Sequenced<'g> {
    fn new(governor: &'g FakeGovernor, script: impl IntoIterator<Item = ExecutorOutcome>) -> Self {
        Self {
            governor,
            script: ScriptedExecutor::new(script),
            at: Mutex::new(Vec::new()),
        }
    }

    /// The governor log's length at each executor call, in call order.
    fn at(&self) -> Vec<usize> {
        self.at
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl AgentExecutor for Sequenced<'_> {
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.at
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(self.governor.calls().len());
        self.script.run(commission, frontier)
    }
}

/// Expectation 1 for every executor call of a loop: the last governor calls before it are the
/// case loaded and the frontier obtained, in that order.
fn reads_before_each_step(name: &str, case: &CaseId, governor: &FakeGovernor, at: &[usize]) {
    let calls = governor.calls();
    let reads = iteration(case);
    for (step, &len) in at.iter().enumerate() {
        assert!(
            len >= reads.len() && len <= calls.len(),
            "{name}: executor call {step} came after {len} governor calls of {}",
            calls.len()
        );
        assert_eq!(
            calls[len - reads.len()..len],
            reads[..],
            "{name}: 1. executor call {step} is not preceded by load, completion and frontier"
        );
    }
}

/// New ids from counters, one per kind, and the one trusted time [`NOW`].
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
        Timestamp(NOW.to_owned())
    }
}

fn rows(runs: &Generated<RunStore>) -> Vec<RunStates> {
    runs.run_states()
        .unwrap_or_else(|unmet| panic!("RunStates: {unmet}"))
}

/// Runs one loop with a fresh context, and checks expectation 8 for it: the loop starts exactly
/// one Run, of this commission, at `started_at`, and leaves every earlier Run as it was.
fn drive(
    runs: &mut Generated<RunStore>,
    name: &str,
    commission: &Commission<commission_state::Assigned>,
    governor: &FakeGovernor,
    executor: &Sequenced<'_>,
    authority: &StaticAuthorityProvider,
    started_at: i64,
) -> LoopEnd {
    let before = rows(runs);
    let mut context = Context::default();
    let end = run_until_blocked(
        governor,
        executor,
        authority,
        commission,
        runs,
        &mut context,
    )
    .unwrap_or_else(|error| panic!("{name}: the loop failed: {error:?}"));
    let after = rows(runs);

    assert!(
        before.iter().all(|row| after.contains(row)),
        "{name}: the loop changed an earlier Run: {before:?} -> {after:?}"
    );
    let new: Vec<&RunStates> = after
        .iter()
        .filter(|row| !before.iter().any(|old| old.run_id == row.run_id))
        .collect();
    assert_eq!(new.len(), 1, "{name}: the loop starts exactly one Run");
    let run = new[0];
    assert_eq!(run.run_id, end.run_id, "{name}: the run the loop reports");
    assert_eq!(
        run.commission_id,
        commission.data().commission_id,
        "{name}: the Run's commission"
    );
    assert_eq!(
        run.case_revision, started_at,
        "{name}: 8. the Run carries the case revision its loop started against"
    );
    end
}

/// Expectations 1, 2, 4 and 5 in one loop, at revision 5, on a frontier that admits `inspect`:
/// the executor proposes the unlisted `deploy`, then `inspect`, and the governor then reports the
/// case complete with `X`.
fn order_refusal_admission_completion(runs: &mut Generated<RunStore>) {
    let name = "order, refusal, admission, completion";
    let case = CaseId("case-order".to_owned());
    let commission = commission(1, &case);
    let governor = FakeGovernor::new();
    let open = listing(5, vec![admissible("inspect")]);
    governor.script(
        case.clone(),
        repeat_n(open.clone(), 11).chain([open.complete("X")]),
    );
    let executor = Sequenced::new(&governor, [proposal("deploy"), proposal("inspect")]);
    let authority = StaticAuthorityProvider::new();

    let end = drive(runs, name, &commission, &governor, &executor, &authority, 5);

    // 1. Per iteration, load and frontier come before the executor.
    assert_eq!(
        governor.calls(),
        [
            iteration(&case),
            revalidation(&case),
            iteration(&case),
            revalidation(&case),
            completed(&case),
        ]
        .concat(),
        "{name}: 1. governor calls"
    );
    assert_eq!(executor.at(), [3, 8], "{name}: 1. executor calls");
    reads_before_each_step(name, &case, &governor, &executor.at());

    // 2. The unlisted `deploy` is refused and not admitted, and the loop reads the frontier again:
    //    the second executor call runs on the frontier issued after the refusal (the fake numbers
    //    its frontiers 1, 2, 3, … in the order issued; 2 is the revalidation's).
    let deploy = request(1, &end.run_id, &case, 5, "deploy");
    let inspect = request(2, &end.run_id, &case, 5, "inspect");
    assert_eq!(
        end.requests,
        vec![
            Revalidated {
                request: deploy,
                outcome: RevalidateActionRequestOutcome::NotAdmitted {
                    error: ActionNotAdmitted {
                        action: "deploy".to_owned(),
                        reasons: Vec::new(),
                    },
                },
            },
            Revalidated {
                request: inspect.clone(),
                outcome: RevalidateActionRequestOutcome::Admitted,
            },
        ],
        "{name}: 2. requests and their revalidation"
    );
    let ran_on: Vec<(CommissionId, FrontierId)> = executor
        .script
        .calls()
        .into_iter()
        .map(|call| (call.commission_id, call.frontier_id))
        .collect();
    assert_eq!(
        ran_on,
        [
            (commission.data().commission_id.clone(), FrontierId(uuid(1))),
            (commission.data().commission_id.clone(), FrontierId(uuid(3))),
        ],
        "{name}: 2. the executor runs on the frontier read again"
    );

    // 4. `inspect` is admitted and recorded, and nothing executes it: the loop holds no effect
    //    port, the governor received only the reads above and no evidence, and the provider was
    //    not asked.
    assert_eq!(end.admitted, vec![inspect], "{name}: 4. admitted");
    assert!(
        governor.evidence().is_empty(),
        "{name}: 4. evidence reached the governor: {:?}",
        governor.evidence()
    );
    assert!(
        authority.asked().is_empty(),
        "{name}: 4. the provider was asked: {:?}",
        authority.asked()
    );
    assert_eq!(
        governor.observations().len(),
        2,
        "{name}: one observation per executor step"
    );

    // 5. Completed, carrying the governor's outcome.
    assert_eq!(
        end.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "X".to_owned(),
        }),
        "{name}: 5. outcome"
    );
}

/// Expectation 3: `inspect` is proposed on the frontier of revision 7, and by revalidation the
/// governor has moved the case to 8.
fn stale_proposal(runs: &mut Generated<RunStore>) {
    let name = "stale proposal";
    let case = CaseId("case-stale".to_owned());
    let commission = commission(2, &case);
    let governor = FakeGovernor::new();
    let at7 = listing(7, vec![admissible("inspect")]);
    let at8 = listing(8, vec![admissible("inspect")]);
    governor.script(
        case.clone(),
        repeat_n(at7, 3)
            .chain(repeat_n(at8.clone(), 2))
            .chain([at8.complete("Y")]),
    );
    let executor = Sequenced::new(&governor, [proposal("inspect")]);
    let authority = StaticAuthorityProvider::new();

    let end = drive(runs, name, &commission, &governor, &executor, &authority, 7);

    assert_eq!(
        governor.calls(),
        [
            iteration(&case),
            vec![GovernorCall::CurrentRevision(case.clone())],
            completed(&case),
        ]
        .concat(),
        "{name}: governor calls"
    );
    reads_before_each_step(name, &case, &governor, &executor.at());
    assert_eq!(
        end.requests,
        vec![Revalidated {
            request: request(1, &end.run_id, &case, 7, "inspect"),
            outcome: RevalidateActionRequestOutcome::Stale {
                error: ActionRequestStale {
                    expected_case_revision: 7,
                    current_case_revision: 8,
                },
            },
        }],
        "{name}: 3. refused as stale"
    );
    assert!(
        end.admitted.is_empty(),
        "{name}: 3. a stale request was admitted: {:?}",
        end.admitted
    );
    assert_eq!(
        end.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "Y".to_owned(),
        }),
        "{name}: outcome"
    );
}

/// Expectation 6: the executor returns `Suspended` with reason `S`.
fn suspended(runs: &mut Generated<RunStore>) {
    let name = "suspended";
    let case = CaseId("case-suspended".to_owned());
    let commission = commission(3, &case);
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(2, vec![admissible("inspect")])]);
    let reason = SuspensionReason::Dependency(vec![CaseId("case-upstream".to_owned())]);
    let executor = Sequenced::new(
        &governor,
        [ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
            reason: reason.clone(),
        })],
    );
    let authority = StaticAuthorityProvider::new();

    let end = drive(runs, name, &commission, &governor, &executor, &authority, 2);

    assert_eq!(governor.calls(), iteration(&case), "{name}: governor calls");
    reads_before_each_step(name, &case, &governor, &executor.at());
    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended {
            reason: reason.clone(),
        }),
        "{name}: 6. outcome"
    );
    let held = RunStorage::get(&runs.ports, &end.run_id)
        .unwrap_or_else(|| panic!("{name}: 6. the suspended run is not stored"));
    assert_eq!(held.state, RunState::Suspended, "{name}: 6. Run state");
    assert_eq!(held.data.run_id, end.run_id, "{name}: 6. Run id");
    assert_eq!(held.data.case_revision, 2, "{name}: 6. Run case revision");
    assert!(
        end.requests.is_empty(),
        "{name}: requests: {:?}",
        end.requests
    );
}

/// Expectation 7: the frontier admits no action and lists no obligation.
fn no_admissible_action(runs: &mut Generated<RunStore>) {
    let name = "no admissible action";
    let case = CaseId("case-empty".to_owned());
    let commission = commission(4, &case);
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(9, Vec::new())]);
    let executor = Sequenced::new(&governor, [ExecutorOutcome::NoUsefulAction(Unit(true))]);
    let authority = StaticAuthorityProvider::new();

    let end = drive(runs, name, &commission, &governor, &executor, &authority, 9);

    assert_eq!(governor.calls(), iteration(&case), "{name}: governor calls");
    assert_eq!(executor.at(), [3], "{name}: executor calls");
    assert_eq!(
        end.outcome,
        RunOutcome::NoAdmissibleAction(Unit(true)),
        "{name}: 7. outcome"
    );
}

/// Expectation 9: the executor proposes `deploy`, which the frontier marks `ApprovalRequired`
/// under `prod.deploy`, and the provider answers approval required with `Q`.
fn authority_consulted(runs: &mut Generated<RunStore>) {
    let name = "authority consulted";
    let case = CaseId("case-authority".to_owned());
    let commission = commission(5, &case);
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            4,
            vec![FrontierAction {
                action: "deploy".to_owned(),
                status: ActionStatus::ApprovalRequired,
                capability: Some("prod.deploy".to_owned()),
                reasons: Vec::new(),
            }],
        )],
    );
    let executor = Sequenced::new(&governor, [proposal("deploy")]);
    let authority = StaticAuthorityProvider::new().answer(
        "prod.deploy",
        AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
            request: "Q".to_owned(),
        }),
    );

    let end = drive(runs, name, &commission, &governor, &executor, &authority, 4);

    assert_eq!(
        governor.calls(),
        [iteration(&case), revalidation(&case)].concat(),
        "{name}: governor calls"
    );
    reads_before_each_step(name, &case, &governor, &executor.at());
    assert_eq!(
        authority.asked(),
        [AuthorityQuery {
            principal: PrincipalId("principal-a".to_owned()),
            authority_context: AuthorityContext(Value::Null),
            capability: "prod.deploy".to_owned(),
        }],
        "{name}: 9. the provider is asked for the action's capability"
    );
    assert_eq!(
        end.requests,
        vec![Revalidated {
            request: request(1, &end.run_id, &case, 4, "deploy"),
            outcome: RevalidateActionRequestOutcome::NeedsAuthority {
                error: ActionNeedsAuthority {
                    action: "deploy".to_owned(),
                    capability: "prod.deploy".to_owned(),
                },
            },
        }],
        "{name}: 9. requests"
    );
    assert!(
        end.admitted.is_empty(),
        "{name}: 9. admitted without authority: {:?}",
        end.admitted
    );
    assert_eq!(
        end.outcome,
        RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
            request: "Q".to_owned(),
        }),
        "{name}: 9. outcome"
    );
}

/// Expectation 10: the one executor step proposes `report_result` with arguments saying tests
/// passed; the governor then reports the case complete.
fn observation_delivered(runs: &mut Generated<RunStore>) {
    let name = "observation delivered";
    let case = CaseId("case-observed".to_owned());
    let commission = commission(6, &case);
    let governor = FakeGovernor::new();
    let open = listing(3, vec![admissible("report_result")]);
    governor.script(
        case.clone(),
        repeat_n(open.clone(), 6).chain([open.complete("Z")]),
    );
    let said = json::parse(r#"{"tests": "passed"}"#)
        .unwrap_or_else(|error| panic!("{name}: fixture is not JSON: {error}"));
    let executor = Sequenced::new(
        &governor,
        [ExecutorOutcome::ProposedAction(
            ExecutorOutcomeProposedAction {
                action: "report_result".to_owned(),
                arguments: ProposedActionArguments(said.clone()),
            },
        )],
    );
    let authority = StaticAuthorityProvider::new();

    let end = drive(runs, name, &commission, &governor, &executor, &authority, 3);

    let observations = governor.observations();
    assert_eq!(
        observations.len(),
        1,
        "{name}: 10. one observation from the one step: {observations:?}"
    );
    let observed = &observations[0];
    assert_eq!(
        observed.observation_id,
        ObservationId(uuid(0x401)),
        "{name}: 10. observation id from the context"
    );
    assert_eq!(
        observed.observed_at,
        Timestamp(NOW.to_owned()),
        "{name}: 10. observation time from the context"
    );
    assert_eq!(observed.source, "executor", "{name}: 10. source");
    assert_eq!(observed.subject, "case-observed@3", "{name}: 10. subject");
    assert_eq!(
        observed.payload.member("action"),
        Some(&Value::Text("report_result".to_owned())),
        "{name}: 10. payload action"
    );
    assert_eq!(
        observed.payload.member("arguments"),
        Some(&said),
        "{name}: 10. payload arguments, unchanged"
    );
    assert!(
        governor.evidence().is_empty(),
        "{name}: 10. an executor output became evidence: {:?}",
        governor.evidence()
    );
    assert_eq!(end.admitted.len(), 1, "{name}: admitted");
    assert_eq!(
        end.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "Z".to_owned(),
        }),
        "{name}: outcome"
    );
}

/// The bound where the frontier does not change: the executor is never asked a third time at a
/// revision where two iterations in a row admitted nothing. A third executor call past the bound
/// would exhaust its script and panic.
fn unchanged_frontier_is_bounded(runs: &mut Generated<RunStore>) {
    let inspect = || vec![admissible("inspect")];

    // `NoUsefulAction` twice at revision 6, on a frontier that admits `inspect`.
    let name = "bound: NoUsefulAction twice";
    let case = CaseId("case-idle".to_owned());
    let commission_a = commission(7, &case);
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(6, inspect())]);
    let executor = Sequenced::new(
        &governor,
        [
            ExecutorOutcome::NoUsefulAction(Unit(true)),
            ExecutorOutcome::NoUsefulAction(Unit(true)),
        ],
    );
    let authority = StaticAuthorityProvider::new();
    let end = drive(
        runs,
        name,
        &commission_a,
        &governor,
        &executor,
        &authority,
        6,
    );
    assert_eq!(executor.at(), [3, 6], "{name}: executor calls");
    assert_eq!(
        end.outcome,
        RunOutcome::NoAdmissibleAction(Unit(true)),
        "{name}: outcome"
    );

    // A refused proposal, then `CompletedLocalReasoning`: both admit nothing.
    let name = "bound: refused, then CompletedLocalReasoning";
    let case = CaseId("case-wander".to_owned());
    let commission_b = commission(8, &case);
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(6, inspect())]);
    let executor = Sequenced::new(
        &governor,
        [
            proposal("deploy"),
            ExecutorOutcome::CompletedLocalReasoning(Unit(true)),
        ],
    );
    let end = drive(
        runs,
        name,
        &commission_b,
        &governor,
        &executor,
        &authority,
        6,
    );
    assert_eq!(executor.at(), [3, 8], "{name}: executor calls");
    assert!(end.admitted.is_empty(), "{name}: admitted");
    assert_eq!(
        end.outcome,
        RunOutcome::NoAdmissibleAction(Unit(true)),
        "{name}: outcome"
    );

    // A new case revision ends the run: revision 6, then 7. A Run is bound to the case revision it
    // started against, and a proposal made on another revision is stale
    // (ess/domains/responsibility.yaml:390), so the Run's revision has no admissible action left:
    // the loop admits nothing more and ends with no admissible action (coordinator decision F1,
    // adversary pass 1).
    let name = "bound: the loop ends at the revision change";
    let case = CaseId("case-moving".to_owned());
    let commission_c = commission(9, &case);
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        repeat_n(listing(6, inspect()), 3).chain([listing(7, inspect())]),
    );
    let executor = Sequenced::new(
        &governor,
        [
            ExecutorOutcome::NoUsefulAction(Unit(true)),
            ExecutorOutcome::NoUsefulAction(Unit(true)),
            ExecutorOutcome::NoUsefulAction(Unit(true)),
        ],
    );
    let end = drive(
        runs,
        name,
        &commission_c,
        &governor,
        &executor,
        &authority,
        6,
    );
    assert_eq!(
        governor.calls(),
        [iteration(&case), completed(&case)].concat(),
        "{name}: governor calls"
    );
    assert_eq!(executor.at(), [3], "{name}: executor calls");
    assert_eq!(
        end.outcome,
        RunOutcome::NoAdmissibleAction(Unit(true)),
        "{name}: outcome"
    );
}

#[test]
fn run_until_blocked_over_fakes() {
    let mut issued = 0u64;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));

    order_refusal_admission_completion(&mut runs);
    stale_proposal(&mut runs);
    suspended(&mut runs);
    no_admissible_action(&mut runs);
    authority_consulted(&mut runs);
    observation_delivered(&mut runs);
    unchanged_frontier_is_bounded(&mut runs);

    // 8. One Run per loop, each at the revision its loop started against (checked per loop).
    assert_eq!(rows(&runs).len(), 9, "one Run per loop");
}
