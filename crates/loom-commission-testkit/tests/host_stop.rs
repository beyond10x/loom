//! Acceptance for `story:host-stop-before-step`: a host stops a run before a step for a reason
//! only it can read (`LoopContext::host_stop`), and the run ends suspended for
//! `SuspensionReason::HostStop`, named as the host named it, or `Waiting` while a request is
//! pending. The runtime asks the host where it reads the run's deadline before a step, right after
//! the deadline: before each executor call and before a pending request is invoked again, and on a
//! resume before the Suspension is taken and right before the held request is invoked. It asks
//! once per check and never after the run ended; a host that supplies no stop is never stopped.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use b10x_loom_commission::model::behaviour::{Generated, RunStorage};
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, BudgetLimit, CaseId,
    Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomePending,
    EffectOutcomePerformed, ExecutorOutcome, ExecutorOutcomeProposedAction, Frontier,
    FrontierAction, HoldSuspension, HoldSuspensionOutcome, HostStop, ObservationId, PrincipalId,
    ProposedActionArguments, RunBudget, RunBudgetBreach, RunId, RunOutcome, RunOutcomeSuspended,
    RunOutcomeWaiting, RunState, SuspensionKey, SuspensionReason, SuspensionState,
    commission_state, frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::suspension::SuspensionPort;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, resume_after_grant, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_commission_testkit::file_suspension::FileSuspensionStore;

const EVALUATE: &str = "evaluate";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x81)),
        agent_revision_id: AgentRevisionId(uuid(0x82)),
        case_id: case.clone(),
        principal: PrincipalId("principal-h".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// The case open at revision 1, its frontier listing `evaluate` admissible, on every call.
fn governor(case: &CaseId) -> FakeGovernor {
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [Answer::at(1).with_items(
            Vec::new(),
            Vec::new(),
            vec![FrontierAction {
                action: EVALUATE.to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            }],
        )],
    );
    governor
}

/// An executor over an agent session: each call proposes `evaluate` with arguments of its own, so
/// each is progress.
#[derive(Default)]
struct Session {
    calls: AtomicUsize,
}

impl Session {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl AgentExecutor for Session {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: EVALUATE.to_owned(),
            arguments: ProposedActionArguments(Value::Number(call.to_string())),
        })
    }
}

/// What the effect port answers every invocation.
#[derive(Clone, Copy)]
enum Answering {
    Performed,
    Pending,
    /// The port performs no action: the run ends `NoPerformableAction` before any step.
    PerformsNothing,
}

/// An effect port answering as told, counting its invocations.
struct Effects {
    answering: Answering,
    invoked: AtomicUsize,
}

impl Effects {
    fn answering(answering: Answering) -> Self {
        Self {
            answering,
            invoked: AtomicUsize::new(0),
        }
    }

    fn invoked(&self) -> usize {
        self.invoked.load(Ordering::SeqCst)
    }
}

impl EffectPort for Effects {
    fn performs(&self, _action: &str) -> bool {
        !matches!(self.answering, Answering::PerformsNothing)
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.invoked.fetch_add(1, Ordering::SeqCst);
        Ok(match self.answering {
            Answering::Pending => EffectOutcome::Pending(EffectOutcomePending {
                report: Value::Text("still evaluating".to_owned()),
                ends_run: false,
            }),
            Answering::Performed | Answering::PerformsNothing => {
                EffectOutcome::Performed(EffectOutcomePerformed {
                    report: Value::Text("evaluated".to_owned()),
                    attempt: None,
                    audit: None,
                    usage: None,
                })
            }
        })
    }
}

/// New ids from counters, a fixed trusted clock, the step and run budgets it was given, and a host
/// answering each `host_stop` with its answers in turn, the last one from then on, counting each
/// time it was asked.
struct Context {
    requests: u64,
    observations: u64,
    answers: VecDeque<Option<HostStop>>,
    asked: usize,
    step_budget: Option<usize>,
    run_budget: Option<RunBudget>,
    clock: &'static str,
}

impl Context {
    fn answering(answers: impl IntoIterator<Item = Option<HostStop>>) -> Self {
        Self {
            requests: 0,
            observations: 0,
            answers: answers.into_iter().collect(),
            asked: 0,
            step_budget: None,
            run_budget: None,
            clock: "2026-10-22T09:00:00Z",
        }
    }
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
        Timestamp(self.clock.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.step_budget
    }

    fn run_budget(&self) -> Option<RunBudget> {
        self.run_budget.clone()
    }

    fn host_stop(&mut self) -> Option<HostStop> {
        self.asked += 1;
        if self.answers.len() > 1 {
            self.answers.pop_front().flatten()
        } else {
            self.answers.front().cloned().flatten()
        }
    }
}

/// The free space of the build directory under the host's threshold, with the host's own detail.
fn disk_low() -> HostStop {
    HostStop {
        name: "DiskLow".to_owned(),
        detail: Value::Object(vec![
            ("path".to_owned(), Value::Text("target".to_owned())),
            ("free_bytes".to_owned(), Value::Number("1048576".to_owned())),
            (
                "threshold_bytes".to_owned(),
                Value::Number("21474836480".to_owned()),
            ),
        ]),
    }
}

/// The host's active time used up: its wall time less its own pauses inside an effect.
fn active_time() -> HostStop {
    HostStop {
        name: "ActiveTime".to_owned(),
        detail: Value::Array(vec![
            Value::Number("5400000".to_owned()),
            Value::Text("paused \"on the forge\"\n".to_owned()),
            Value::Bool(true),
            Value::Null,
        ]),
    }
}

fn suspended_for(stop: HostStop) -> RunOutcome {
    RunOutcome::Suspended(RunOutcomeSuspended {
        reason: SuspensionReason::HostStop(stop),
    })
}

/// `run_until_blocked` of `session` on [`governor`] under `context`, with a run store.
fn run(
    name: &str,
    session: &Session,
    effects: &Effects,
    context: &mut Context,
) -> (LoopEnd, Generated<RunStore>) {
    let case = CaseId(format!("case-host-stop-{name}"));
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x101))));
    let end = run_until_blocked(
        &governor(&case),
        session,
        &StaticAuthorityProvider::new(),
        effects,
        &commission(&case),
        &mut runs,
        context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"));
    (end, runs)
}

fn stored_state(runs: &Generated<RunStore>, run_id: &RunId) -> Option<RunState> {
    RunStorage::get(&runs.ports, run_id).map(|held| held.state)
}

#[test]
fn a_host_stop_before_the_first_step_calls_no_executor() {
    let session = Session::default();
    let effects = Effects::answering(Answering::Performed);
    let mut context = Context::answering([Some(disk_low())]);

    let (end, runs) = run("first", &session, &effects, &mut context);

    assert_eq!(end.outcome, suspended_for(disk_low()));
    assert_eq!(session.calls(), 0, "no executor is called");
    assert_eq!(effects.invoked(), 0, "nothing is performed");
    assert_eq!(context.asked, 1, "the host is asked once, before the step");
    assert_eq!(end.steps, 0);
    assert_eq!(
        stored_state(&runs, &end.run_id),
        Some(RunState::Suspended),
        "the Run is suspended for the host's stop"
    );
}

#[test]
fn a_host_stop_after_one_step_stops_before_the_next() {
    let session = Session::default();
    let effects = Effects::answering(Answering::Performed);
    let mut context = Context::answering([None, Some(active_time())]);

    let (end, runs) = run("after-one", &session, &effects, &mut context);

    assert_eq!(
        end.outcome,
        suspended_for(active_time()),
        "suspended for the stop, its detail as the host gave it"
    );
    assert_eq!(session.calls(), 1, "the executor is called once");
    assert_eq!(effects.invoked(), 1, "the first step is performed");
    assert_eq!(context.asked, 2, "once before each executor call");
    assert_eq!(stored_state(&runs, &end.run_id), Some(RunState::Suspended));
}

#[test]
fn a_host_stop_while_a_request_is_pending_ends_the_run_waiting() {
    let session = Session::default();
    let effects = Effects::answering(Answering::Pending);
    let mut context = Context::answering([None, Some(disk_low())]);

    let (end, runs) = run("pending", &session, &effects, &mut context);

    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![ActionRequestId(uuid(0x301))],
            reason: SuspensionReason::HostStop(disk_low()),
        })
    );
    assert_eq!(
        effects.invoked(),
        1,
        "the pending request is not invoked again"
    );
    assert_eq!(session.calls(), 1);
    assert_eq!(context.asked, 2);
    assert_eq!(stored_state(&runs, &end.run_id), Some(RunState::Suspended));
}

/// The host is asked once before each step and never once the run ended: a run its step budget
/// ends after two steps asks the host twice, and the stop it would give then is never read.
#[test]
fn a_host_stop_is_asked_once_before_each_step_and_never_after_the_run_ended() {
    let session = Session::default();
    let effects = Effects::answering(Answering::Performed);
    let mut context = Context::answering([None, None, Some(disk_low())]);
    context.step_budget = Some(2);

    let (end, _) = run("budgeted", &session, &effects, &mut context);

    assert!(
        matches!(
            &end.outcome,
            RunOutcome::Suspended(RunOutcomeSuspended {
                reason: SuspensionReason::Budget(_)
            })
        ),
        "the step budget ends the run: {:?}",
        end.outcome
    );
    assert_eq!(session.calls(), 2);
    assert_eq!(
        context.asked, 2,
        "asked before each step, not after the end"
    );
}

/// The deadline is read first: a run past its deadline is suspended for it and the host is not
/// asked.
#[test]
fn a_deadline_reached_first_is_the_reason_and_the_host_is_not_asked() {
    let session = Session::default();
    let effects = Effects::answering(Answering::Performed);
    let mut context = Context::answering([Some(disk_low())]);
    context.run_budget = Some(RunBudget {
        max_steps: None,
        max_tokens_in: None,
        max_tokens_out: None,
        max_duration_ms: None,
        max_cost_micro_usd: None,
        max_tokens: None,
        deadline: Some(Timestamp("2026-10-22T08:30:00Z".to_owned())),
        per_action: Vec::new(),
    });

    let (end, _) = run("deadline-first", &session, &effects, &mut context);

    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended {
            reason: SuspensionReason::RunBudget(RunBudgetBreach {
                action: None,
                limit: BudgetLimit::Deadline,
                max: 1_792_657_800,
                charged: Some(1_792_659_600),
            }),
        })
    );
    assert_eq!(
        context.asked, 0,
        "the host is not asked once the deadline stopped the run"
    );
    assert_eq!(session.calls(), 0);
}

/// A stop that takes no step comes first: a frontier the effect port performs nothing of ends
/// `NoPerformableAction`, and the host is not asked.
#[test]
fn a_stop_that_takes_no_step_comes_before_the_host_stop() {
    let session = Session::default();
    let effects = Effects::answering(Answering::PerformsNothing);
    let mut context = Context::answering([Some(disk_low())]);

    let (end, _) = run("unperformable", &session, &effects, &mut context);

    assert!(
        matches!(end.outcome, RunOutcome::NoPerformableAction(_)),
        "the run ends as no performable action: {:?}",
        end.outcome
    );
    assert_eq!(context.asked, 0, "the host is not asked");
    assert_eq!(session.calls(), 0);
}

/// A suspension store under the test's own directory `name`, holding `commission`'s Suspension of
/// an `evaluate` request at revision 1, the request `0x301` of the Run `0x101`.
fn holding(name: &str, commission: &Commission<commission_state::Assigned>) -> FileSuspensionStore {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("host-stop")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    let suspensions = FileSuspensionStore::open(&dir).expect("the suspension store opens");
    let commission_id = commission.data().commission_id.clone();
    let held = suspensions.hold(HoldSuspension {
        suspension: SuspensionKey {
            commission_id: commission_id.clone(),
        },
        commission_id,
        run_id: RunId(uuid(0x101)),
        run_case_revision: 1,
        action_request_id: ActionRequestId(uuid(0x301)),
        case_id: commission.data().case_id.clone(),
        expected_case_revision: 1,
        action: EVALUATE.to_owned(),
        arguments: ProposedActionArguments(Value::Null),
        reason: SuspensionReason::Authority(Value::Null),
    });
    assert!(matches!(held, HoldSuspensionOutcome::Held { .. }));
    suspensions
}

/// What a resume of a held Suspension ended with, under a host answering `answers`.
struct Resumed {
    outcome: RunOutcome,
    stored: Option<SuspensionState>,
    run: Option<RunState>,
    invoked: usize,
    calls: usize,
    asked: usize,
}

fn resume(name: &str, answers: Vec<Option<HostStop>>) -> Resumed {
    let case = CaseId(format!("case-host-stop-{name}"));
    let commission = commission(&case);
    let suspensions = holding(name, &commission);
    let session = Session::default();
    let effects = Effects::answering(Answering::Performed);
    let mut context = Context::answering(answers);
    let mut runs = RunStore::new(|| RunId(uuid(0x102)));
    let end = resume_after_grant(
        &governor(&case),
        &session,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission,
        &mut runs,
        &suspensions,
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the resume failed: {error:?}"));
    let stored = suspensions
        .get(&commission.data().commission_id)
        .expect("the store reads")
        .map(|held| held.state);
    Resumed {
        outcome: end.outcome,
        stored,
        run: RunStorage::get(&runs, &RunId(uuid(0x101))).map(|held| held.state),
        invoked: effects.invoked(),
        calls: session.calls(),
        asked: context.asked,
    }
}

#[test]
fn a_resume_the_host_stops_takes_nothing() {
    let resumed = resume("resume-before-take", vec![Some(disk_low())]);

    assert_eq!(resumed.outcome, suspended_for(disk_low()));
    assert_eq!(
        resumed.stored,
        Some(SuspensionState::Pending),
        "nothing was taken: the Suspension stays pending"
    );
    assert_eq!(resumed.invoked, 0, "nothing is performed");
    assert_eq!(resumed.calls, 0, "no executor runs");
    assert_eq!(resumed.run, Some(RunState::Suspended));
    assert_eq!(resumed.asked, 1, "asked once, before the take");
}

/// A host that stops the resume right before the held request is invoked releases the Suspension
/// the resume took, and nothing is performed.
#[test]
fn a_resume_the_host_stops_before_the_invoke_releases_the_suspension() {
    let resumed = resume("resume-before-invoke", vec![None, Some(active_time())]);

    assert_eq!(resumed.outcome, suspended_for(active_time()));
    assert_eq!(
        resumed.stored, None,
        "the Suspension the resume took is released"
    );
    assert_eq!(resumed.invoked, 0, "nothing is performed");
    assert_eq!(resumed.calls, 0, "no executor runs");
    assert_eq!(resumed.run, Some(RunState::Suspended));
    assert_eq!(
        resumed.asked, 2,
        "asked before the take and before the invoke"
    );
}
