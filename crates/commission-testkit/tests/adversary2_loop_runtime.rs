//! Adversary pass 2 on `story:local-runtime-loop` (wave 2026-10-04-w7).
//!
//! Red cases:
//!
//! * `stale_after_an_idle_iteration_ends_completed`: `runtime.rs` says of a stale proposal "the
//!   next iteration loads the case, which has moved, and ends the run unless the case is
//!   complete", and decision F1 says the same. After one idle iteration the stall bound ends the
//!   run in the stale iteration itself, so a case completed by the move ends `NoAdmissibleAction`.
//!   The same move after an admitted iteration ends `Completed` (the green control below).
//! * `admitted_proposal_on_an_unchanged_frontier_is_bounded`: no effect executes, so an admitted
//!   request never moves the case; an executor that proposes the same admitted action on the same
//!   frontier is asked again without bound (story note from wave w5, run-outcomes F4).
//! * `executor_suspension_reason_survives_an_observation_failure`: the executor returns
//!   `Suspended` with reason `S`, the observation port then fails, and the Run is suspended with
//!   `ExternalAvailability` instead; `S` reaches neither `SuspendRun` nor the returned error.
//!
//! Green probes: the frontier revision check (loaded revision equal, frontier not), an observation
//! failure suspending and naming the Run, and a failed suspension carried beside the failure.

use std::cell::Cell;
use std::sync::{Mutex, PoisonError};

use b10x_commission::model::behaviour::{Generated, RunStorage};
use b10x_commission::model::json::Value;
use b10x_commission::model::obligation::UnmetObligation;
use b10x_commission::model::primitives::{Timestamp, Uuid};
use b10x_commission::model::responsibility::obligations::{StartRunBehavior, SuspendRunBehavior};
use b10x_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, CompletionDetermination, ExecutorOutcome,
    ExecutorOutcomeProposedAction, ExecutorOutcomeSuspended, Frontier, FrontierAction,
    GovernorError, Observation, ObservationId, PrincipalId, ProposedActionArguments, RunId,
    RunOutcome, RunOutcomeCompleted, RunState, StartRun, StartRunOutcome, SuspendRun,
    SuspendRunOutcome, SuspensionReason, Unit, commission_state, frontier_state, observation_state,
};
use b10x_commission::outcome::RunStore;
use b10x_commission::ports::evidence::ObservationPort;
use b10x_commission::ports::executor::AgentExecutor;
use b10x_commission::ports::governor::Governor;
use b10x_commission::runtime::{LoopContext, LoopEnd, LoopError, LoopFailure, run_until_blocked};
use b10x_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x61)),
        agent_revision_id: AgentRevisionId(uuid(0x62)),
        case_id: case.clone(),
        principal: PrincipalId("principal-b".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn inspect_only(revision: i64) -> Answer {
    Answer::at(revision).with_items(
        Vec::new(),
        Vec::new(),
        vec![FrontierAction {
            action: "inspect".to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    )
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
        ActionRequestId(uuid(0x500 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x600 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-04T12:00:00Z".to_owned())
    }
}

fn store() -> Generated<RunStore> {
    let mut issued = 0u64;
    Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x700 + issued))
    }))
}

/// The run store, recording every `SuspendRun` it receives; with `refuse`, it refuses each one
/// with an unmet obligation and leaves the Run as it was.
struct Runs {
    inner: Generated<RunStore>,
    suspends: Vec<SuspendRun>,
    refuse: bool,
}

impl Runs {
    fn new(refuse: bool) -> Self {
        Self {
            inner: store(),
            suspends: Vec::new(),
            refuse,
        }
    }

    fn only(&self) -> (RunId, RunState) {
        let held = RunStorage::list(&self.inner.ports);
        assert_eq!(held.len(), 1, "fixture: the loop starts one Run: {held:?}");
        (held[0].data.run_id.clone(), held[0].state)
    }
}

impl StartRunBehavior for Runs {
    fn start_run(&mut self, input: StartRun) -> Result<StartRunOutcome, UnmetObligation> {
        self.inner.start_run(input)
    }
}

impl SuspendRunBehavior for Runs {
    fn suspend_run(&mut self, input: SuspendRun) -> Result<SuspendRunOutcome, UnmetObligation> {
        self.suspends.push(input.clone());
        if self.refuse {
            return Err(UnmetObligation {
                capability: "probe",
                source: "adversary2 refuses SuspendRun",
            });
        }
        self.inner.suspend_run(input)
    }
}

/// The fake governor, whose observation port fails every delivery.
struct ObserveFails {
    inner: FakeGovernor,
}

impl Governor for ObserveFails {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.inner.current_revision(case)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        self.inner.frontier(case)
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.inner.completion(case)
    }
}

impl ObservationPort for ObserveFails {
    fn observe(
        &self,
        _observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        Err(GovernorError::GovernorUnavailable)
    }
}

fn drive<G, E>(governor: &G, executor: &E, case: &CaseId) -> Result<LoopEnd, LoopError>
where
    G: Governor + ObservationPort,
    E: AgentExecutor,
{
    let mut runs = store();
    run_until_blocked(
        governor,
        executor,
        &StaticAuthorityProvider::new(),
        &commission(case),
        &mut runs,
        &mut Context::default(),
    )
}

/// RED. Revision 5, `inspect` admissible. Iteration 1: `NoUsefulAction` (the frontier admits
/// `inspect`, so the run continues, idle). Iteration 2: `inspect` is proposed on revision 5, and by
/// revalidation the case is at 6 and complete with `Done`. The module doc and decision F1: the
/// next iteration loads the moved case and, since it is complete, ends the run completed.
#[test]
fn stale_after_an_idle_iteration_ends_completed() {
    let case = CaseId("case-done-while-proposing".to_owned());
    let governor = FakeGovernor::new();
    // calls 1-6: start load, completion, frontier; load, completion, frontier (all 5, open);
    // call 7 on (revalidation and after): 6, complete.
    governor.script(
        case.clone(),
        std::iter::repeat_n(inspect_only(5), 6).chain([inspect_only(6).complete("Done")]),
    );
    let executor = ScriptedExecutor::new([
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        proposal("inspect"),
    ]);

    let end = drive(&governor, &executor, &case)
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    assert!(
        end.admitted.is_empty(),
        "fixture: nothing admitted: {end:?}"
    );
    assert_eq!(
        end.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "Done".to_owned()
        }),
        "runtime.rs: a stale proposal is followed by a load of the moved case, which ends the run \
         completed when the case is complete (decision F1); governor calls: {:?}",
        governor.calls()
    );
}

/// GREEN control. The same move, after an iteration that admitted `inspect`: the run ends
/// completed.
#[test]
fn stale_after_an_admitted_iteration_ends_completed() {
    let case = CaseId("case-done-after-admit".to_owned());
    let governor = FakeGovernor::new();
    // calls 1-8: start load, completion, frontier, revalidation (2); load, completion, frontier
    // (all 5, open); call 9 on (revalidation and after): 6, complete.
    governor.script(
        case.clone(),
        std::iter::repeat_n(inspect_only(5), 8).chain([inspect_only(6).complete("Done")]),
    );
    let executor = ScriptedExecutor::new([proposal("inspect"), proposal("inspect")]);

    let end = drive(&governor, &executor, &case)
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    assert_eq!(end.admitted.len(), 1, "fixture: one admitted: {end:?}");
    assert_eq!(
        end.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "Done".to_owned()
        })
    );
}

/// How often the test lets the loop ask an executor before stopping it from outside.
const GUARD: usize = 25;

/// An executor that proposes `inspect` on every frontier, as one deterministic in its input does
/// when the frontier does not change, until the test's guard suspends it.
#[derive(Default)]
struct Repeats {
    calls: Mutex<Vec<i64>>,
}

impl AgentExecutor for Repeats {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let mut calls = self.calls.lock().unwrap_or_else(PoisonError::into_inner);
        calls.push(frontier.data().case_revision);
        if calls.len() >= GUARD {
            return ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
                reason: SuspensionReason::Time(Value::Text("test guard".to_owned())),
            });
        }
        proposal("inspect")
    }
}

/// RED. The case stays at 5, open, its frontier admitting `inspect`, and the executor proposes
/// `inspect` every time. No effect executes, so nothing moves the case: the loop asks the same
/// executor on the same frontier until the test stops it.
#[test]
fn admitted_proposal_on_an_unchanged_frontier_is_bounded() {
    let case = CaseId("case-never-moves".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [inspect_only(5)]);
    let executor = Repeats::default();

    let end = drive(&governor, &executor, &case)
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    let calls = executor
        .calls
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    assert!(
        calls.len() < GUARD,
        "story note (wave w5, run-outcomes F4): the loop must not re-ask the same executor on an \
         unchanged frontier without bound; it asked {} times on revisions {:?}, admitted {} \
         requests, and ended only through the test's guard with {:?}",
        calls.len(),
        calls,
        end.admitted.len(),
        end.outcome
    );
}

/// An executor that notes each call and proposes `inspect`.
#[derive(Default)]
struct Counts {
    calls: Cell<usize>,
}

impl AgentExecutor for Counts {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.calls.set(self.calls.get() + 1);
        proposal("inspect")
    }
}

/// GREEN guard. The Run starts at 5 and completion answers open at 5, but the frontier is issued
/// for 6 (the case moved between the two reads), and stays 6. The executor never sees a frontier
/// of another revision than the Run's, and no request is made: without the frontier check the
/// request would be bound to 6 and admitted inside a Run of 5.
#[test]
fn frontier_of_another_revision_never_reaches_the_executor() {
    let case = CaseId("case-frontier-ahead".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [inspect_only(5), inspect_only(5), inspect_only(6)],
    );
    let executor = Counts::default();

    let end = drive(&governor, &executor, &case)
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    assert_eq!(
        executor.calls.get(),
        0,
        "the executor ran on a frontier of 6"
    );
    assert!(end.requests.is_empty(), "requests: {:?}", end.requests);
    assert_eq!(end.outcome, RunOutcome::NoAdmissibleAction(Unit(true)));
}

/// GREEN probe. The observation port fails after the first executor step: the Run is suspended
/// with `ExternalAvailability`, and the error names it.
#[test]
fn observation_failure_suspends_and_names_the_run() {
    let case = CaseId("case-observe-down".to_owned());
    let inner = FakeGovernor::new();
    inner.script(case.clone(), [inspect_only(4)]);
    let governor = ObserveFails { inner };
    let executor = ScriptedExecutor::new([ExecutorOutcome::NoUsefulAction(Unit(true))]);
    let mut runs = Runs::new(false);

    let error = run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &commission(&case),
        &mut runs,
        &mut Context::default(),
    )
    .err()
    .unwrap_or_else(|| panic!("fixture: the observation port fails"));

    let (run_id, state) = runs.only();
    assert_eq!(error.run_id, Some(run_id.clone()));
    assert_eq!(
        error.failure,
        LoopFailure::Governor(GovernorError::GovernorUnavailable)
    );
    assert_eq!(error.suspension, None);
    assert_eq!(state, RunState::Suspended);
    assert_eq!(runs.suspends.len(), 1);
    assert_eq!(runs.suspends[0].run_id, run_id);
    assert!(matches!(
        runs.suspends[0].reason,
        SuspensionReason::ExternalAvailability(_)
    ));
}

/// GREEN probe. The governor fails mid-iteration, and the store refuses the suspension: the error
/// names the Run and carries both failures, and its message says both.
#[test]
fn refused_suspension_is_carried_beside_the_failure() {
    let case = CaseId("case-store-refuses".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        std::iter::repeat_n(inspect_only(4), 3).chain([Answer::unavailable()]),
    );
    let executor = ScriptedExecutor::new([proposal("inspect")]);
    let mut runs = Runs::new(true);

    let error = run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &commission(&case),
        &mut runs,
        &mut Context::default(),
    )
    .err()
    .unwrap_or_else(|| panic!("fixture: the governor fails revalidation"));

    let (run_id, state) = runs.only();
    assert_eq!(error.run_id, Some(run_id.clone()));
    assert_eq!(
        error.failure,
        LoopFailure::Governor(GovernorError::GovernorUnavailable)
    );
    assert!(
        matches!(
            error.suspension.as_deref(),
            Some(LoopFailure::Obligation(UnmetObligation {
                capability: "probe",
                ..
            }))
        ),
        "suspension: {:?}",
        error.suspension
    );
    assert_eq!(state, RunState::Running, "the store refused the suspension");
    let shown = error.to_string();
    assert!(
        shown.contains(&run_id.0.0)
            && shown.contains("GovernorUnavailable")
            && shown.contains("probe"),
        "{shown}"
    );
}

/// RED (note). The executor returns `Suspended` with reason `S`, and delivering the step's
/// observation fails. The Run is suspended once, with `ExternalAvailability`, and the error
/// carries the governor failure: `S` is in neither, nor in the observation, so the reason the
/// executor stopped for is lost.
#[test]
fn executor_suspension_reason_survives_an_observation_failure() {
    let case = CaseId("case-suspend-observe-down".to_owned());
    let inner = FakeGovernor::new();
    inner.script(case.clone(), [inspect_only(4)]);
    let governor = ObserveFails { inner };
    let reason = SuspensionReason::Dependency(vec![CaseId("case-upstream".to_owned())]);
    let executor = ScriptedExecutor::new([ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
        reason: reason.clone(),
    })]);
    let mut runs = Runs::new(false);

    let result = run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &commission(&case),
        &mut runs,
        &mut Context::default(),
    );

    let reasons: Vec<&SuspensionReason> = runs.suspends.iter().map(|s| &s.reason).collect();
    let in_error = match &result {
        Ok(end) => format!("{:?}", end.outcome).contains("case-upstream"),
        Err(error) => format!("{error:?}").contains("case-upstream"),
    };
    assert!(
        reasons.contains(&&reason) || in_error,
        "the executor suspended for {reason:?}; SuspendRun received {reasons:?} and the loop \
         returned {result:?}"
    );
}
