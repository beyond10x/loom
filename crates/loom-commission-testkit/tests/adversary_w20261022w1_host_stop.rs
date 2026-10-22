//! Adversary pass over `story:host-stop-before-step` (wave 2026-10-22-w1).
//!
//! The story's design says a stop that takes no step (`NoAdmissibleAction`, `AwaitingApproval`,
//! `NoPerformableAction`) comes before the host's stop, and that the host is asked once per check
//! and never after the run ended. `NoAdmissibleAction` is judged only after the executor call, so
//! a passed deadline comes first there; since `story:host-stop-when-a-step-follows` the host is
//! not asked before that call when the frontier admits nothing without a grant, so the run ends
//! `NoAdmissibleAction` there, never `HostStop`. These cases hold the host stop against the other
//! stops of an iteration (a kept breach, the approval gate, a frontier admitting nothing), against
//! reads pending together, and against a resume the host stopped before the take.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use b10x_loom_commission::model::behaviour::{Generated, RunStorage};
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, BreachStep, BudgetLimit,
    CaseId, Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomePending,
    EffectOutcomePerformed, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ExecutorOutcomeProposedReads, ExecutorStep, ExecutorUsage, Frontier, FrontierAction,
    HoldSuspension, HoldSuspensionOutcome, HostStop, ObservationId, PrincipalId,
    ProposedActionArguments, RunBudget, RunBudgetBreach, RunId, RunOutcome,
    RunOutcomeAwaitingApproval, RunOutcomeSuspended, RunOutcomeWaiting, RunState, StopPolicy,
    SuspensionKey, SuspensionReason, SuspensionState, Unit, commission_state, frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{
    AdmittedRequest, EffectError, EffectPort, invoke_concurrently,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::suspension::SuspensionPort;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, resume_after_grant, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_commission_testkit::file_suspension::FileSuspensionStore;

const EVALUATE: &str = "evaluate";
const MERGE: &str = "repository.merge";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x91)),
        agent_revision_id: AgentRevisionId(uuid(0x92)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adversary-h".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn listed(name: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

/// The case open at revision 1, its frontier listing `actions` on every call.
fn listing(case: &CaseId, actions: Vec<FrontierAction>) -> FakeGovernor {
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [Answer::at(1).with_items(Vec::new(), Vec::new(), actions)],
    );
    governor
}

/// The case open at revision 1, its frontier listing `evaluate` admissible.
fn admitting_evaluate(case: &CaseId) -> FakeGovernor {
    listing(case, vec![listed(EVALUATE, ActionStatus::Admissible, None)])
}

/// What each executor call proposes.
#[derive(Clone, Copy)]
enum Proposing {
    /// `evaluate`, with the call's number as its arguments.
    One,
    /// Two reads of `evaluate` together, each with arguments of its own.
    Reads,
}

/// An executor over an agent session, each call new progress, reporting `usage` as what it spent.
struct Session {
    proposing: Proposing,
    usage: Option<ExecutorUsage>,
    calls: AtomicUsize,
}

impl Session {
    fn proposing(proposing: Proposing) -> Self {
        Self {
            proposing,
            usage: None,
            calls: AtomicUsize::new(0),
        }
    }

    fn spending(input_tokens: i64) -> Self {
        Self {
            usage: Some(ExecutorUsage {
                input_tokens,
                output_tokens: 10,
                duration_ms: 500,
                cost_micro_usd: Some(3),
            }),
            ..Self::proposing(Proposing::One)
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl AgentExecutor for Session {
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.run_with_usage(commission, frontier).outcome
    }

    fn run_with_usage(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorStep {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let outcome = match self.proposing {
            Proposing::One => ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
                action: EVALUATE.to_owned(),
                arguments: ProposedActionArguments(Value::Number(call.to_string())),
            }),
            Proposing::Reads => ExecutorOutcome::ProposedReads(ExecutorOutcomeProposedReads {
                reads: ["a", "b"]
                    .into_iter()
                    .map(|part| ExecutorOutcomeProposedAction {
                        action: EVALUATE.to_owned(),
                        arguments: ProposedActionArguments(Value::Text(format!("{call}-{part}"))),
                    })
                    .collect(),
            }),
        };
        ExecutorStep {
            outcome,
            usage: self.usage.clone(),
            transcript_digest: None,
            summary: None,
        }
    }
}

#[derive(Clone, Copy)]
enum Answering {
    Performed,
    Pending,
}

/// An effect port that performs and reads every action, answering as told, counting invocations.
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
        true
    }

    fn reads(&self, _action: &str) -> bool {
        true
    }

    fn invoke_reads(
        &self,
        commission: &Commission<commission_state::Assigned>,
        requests: &[AdmittedRequest],
    ) -> Option<Vec<Result<EffectOutcome, EffectError>>> {
        Some(invoke_concurrently(self, commission, requests))
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
            Answering::Performed => EffectOutcome::Performed(EffectOutcomePerformed {
                report: Value::Text("evaluated".to_owned()),
                attempt: None,
                audit: None,
                usage: None,
            }),
        })
    }
}

/// New ids from counters, a fixed trusted clock, the budgets and stop policy given, and a host
/// answering each `host_stop` with its answers in turn, the last one from then on, counting each
/// time it was asked.
struct Context {
    requests: u64,
    observations: u64,
    answers: VecDeque<Option<HostStop>>,
    asked: usize,
    step_budget: Option<usize>,
    run_budget: Option<RunBudget>,
    on_breach: BreachStep,
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
            on_breach: BreachStep::DiscardStep,
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
        Timestamp("2026-10-22T09:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.step_budget
    }

    fn run_budget(&self) -> Option<RunBudget> {
        self.run_budget.clone()
    }

    fn stop_policy(&self) -> StopPolicy {
        StopPolicy {
            approval_gate: true,
            idle_bound: None,
            on_breach: self.on_breach,
            breach_by_limit: Vec::new(),
        }
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

fn disk_low() -> HostStop {
    HostStop {
        name: "DiskLow".to_owned(),
        detail: Value::Object(vec![
            ("path".to_owned(), Value::Text("target".to_owned())),
            ("free_bytes".to_owned(), Value::Number("1048576".to_owned())),
        ]),
    }
}

fn suspended_for(reason: SuspensionReason) -> RunOutcome {
    RunOutcome::Suspended(RunOutcomeSuspended { reason })
}

fn no_limits() -> RunBudget {
    RunBudget {
        max_steps: None,
        max_tokens_in: None,
        max_tokens_out: None,
        max_duration_ms: None,
        max_cost_micro_usd: None,
        max_tokens: None,
        deadline: None,
        per_action: Vec::new(),
    }
}

fn run(
    case: &CaseId,
    governor: &FakeGovernor,
    session: &Session,
    effects: &Effects,
    context: &mut Context,
) -> (LoopEnd, Generated<RunStore>) {
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x101))));
    let end = run_until_blocked(
        governor,
        session,
        &StaticAuthorityProvider::new(),
        effects,
        &commission(case),
        &mut runs,
        context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"));
    (end, runs)
}

/// A frontier admitting nothing ends the run `NoAdmissibleAction` once the executor's outcome is
/// derived (`crates/loom-commission/src/runtime.rs`). No step follows the admissibility check
/// there, so the host is not asked (`story:host-stop-when-a-step-follows`): with or without a
/// stop the run ends `NoAdmissibleAction`, the executor called once and the host never asked.
#[test]
fn adversary_a_frontier_admitting_nothing_ends_the_run_before_the_host_is_asked() {
    let case = CaseId("case-adversary-h-nothing-admissible".to_owned());
    let blocked = || listing(&case, vec![listed(EVALUATE, ActionStatus::Blocked, None)]);

    let session = Session::proposing(Proposing::One);
    let effects = Effects::answering(Answering::Performed);
    let mut unstopped = Context::answering([None]);
    let (end, _) = run(&case, &blocked(), &session, &effects, &mut unstopped);
    assert_eq!(
        end.outcome,
        RunOutcome::NoAdmissibleAction(Unit(true)),
        "without a host stop the frontier ends the run as no admissible action"
    );

    let session = Session::proposing(Proposing::One);
    let effects = Effects::answering(Answering::Performed);
    let mut context = Context::answering([Some(disk_low())]);
    let (end, _) = run(&case, &blocked(), &session, &effects, &mut context);
    assert_eq!(
        (end.outcome, context.asked, session.calls()),
        (RunOutcome::NoAdmissibleAction(Unit(true)), 0, 1),
        "a frontier admitting nothing ends the run as no admissible action, the host not asked"
    );
}

/// A deadline already passed is read before the executor call, as the host's stop is, so under a
/// frontier that admits nothing it suspends the run for `RunBudget` (`Deadline`), the executor
/// never called and the host not asked.
#[test]
fn adversary_a_passed_deadline_comes_before_a_frontier_admitting_nothing() {
    let case = CaseId("case-adversary-h-nothing-admissible-deadline".to_owned());
    let governor = listing(&case, vec![listed(EVALUATE, ActionStatus::Blocked, None)]);
    let session = Session::proposing(Proposing::One);
    let effects = Effects::answering(Answering::Performed);
    let mut context = Context::answering([None]);
    context.run_budget = Some(RunBudget {
        deadline: Some(Timestamp("2026-10-22T08:30:00Z".to_owned())),
        ..no_limits()
    });

    let (end, _) = run(&case, &governor, &session, &effects, &mut context);

    assert_eq!(
        (end.outcome, session.calls(), context.asked),
        (
            suspended_for(SuspensionReason::RunBudget(RunBudgetBreach {
                action: None,
                limit: BudgetLimit::Deadline,
                max: 1_792_657_800,
                charged: Some(1_792_659_600),
            })),
            0,
            0
        ),
        "a passed deadline, read before the executor call, comes before a frontier admitting nothing"
    );
}

/// A step kept past a spending limit (`KeepStep`) stops the run before the next step for that
/// breach: the host is asked once, before the first step, and its later stop is never read.
#[test]
fn adversary_a_kept_breach_stops_the_run_before_the_host_is_asked_again() {
    let case = CaseId("case-adversary-h-kept-breach".to_owned());
    let session = Session::spending(60);
    let effects = Effects::answering(Answering::Performed);
    let mut context = Context::answering([None, Some(disk_low())]);
    context.on_breach = BreachStep::KeepStep;
    context.run_budget = Some(RunBudget {
        max_tokens_in: Some(50),
        ..no_limits()
    });

    let (end, _) = run(
        &case,
        &admitting_evaluate(&case),
        &session,
        &effects,
        &mut context,
    );

    assert_eq!(
        end.outcome,
        suspended_for(SuspensionReason::RunBudget(RunBudgetBreach {
            action: None,
            limit: BudgetLimit::MaxTokensIn,
            max: 50,
            charged: Some(60),
        }))
    );
    assert_eq!(effects.invoked(), 1, "the kept step is performed");
    assert_eq!(session.calls(), 1);
    assert_eq!(context.asked, 1, "asked before the first step only");
}

/// An unchanged frontier listing an action that needs approval ends the run `AwaitingApproval`
/// after a step, before the host is asked again.
#[test]
fn adversary_the_approval_gate_ends_the_run_before_the_host_is_asked_again() {
    let case = CaseId("case-adversary-h-gate".to_owned());
    let governor = listing(
        &case,
        vec![
            listed(MERGE, ActionStatus::ApprovalRequired, Some(MERGE)),
            listed(EVALUATE, ActionStatus::Admissible, None),
        ],
    );
    let session = Session::proposing(Proposing::One);
    let effects = Effects::answering(Answering::Performed);
    let mut context = Context::answering([None, Some(disk_low())]);

    let (end, _) = run(&case, &governor, &session, &effects, &mut context);

    assert_eq!(
        end.outcome,
        RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval {
            actions: vec![MERGE.to_owned()],
        })
    );
    assert_eq!(session.calls(), 1);
    assert_eq!(
        context.asked, 1,
        "the gate comes first; the host is not asked"
    );
}

/// Reads proposed together and answered `Pending` are not invoked again once the host stops the
/// run: it ends `Waiting` on every one of them, in the order proposed.
#[test]
fn adversary_reads_pending_together_wait_on_every_read_when_the_host_stops() {
    let case = CaseId("case-adversary-h-reads".to_owned());
    let session = Session::proposing(Proposing::Reads);
    let effects = Effects::answering(Answering::Pending);
    let mut context = Context::answering([None, Some(disk_low())]);

    let (end, runs) = run(
        &case,
        &admitting_evaluate(&case),
        &session,
        &effects,
        &mut context,
    );

    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![ActionRequestId(uuid(0x301)), ActionRequestId(uuid(0x302))],
            reason: SuspensionReason::HostStop(disk_low()),
        })
    );
    assert_eq!(
        effects.invoked(),
        2,
        "each read is invoked once, never again"
    );
    assert_eq!(session.calls(), 1);
    assert_eq!(context.asked, 2);
    assert_eq!(
        RunStorage::get(&runs.ports, &end.run_id).map(|held| held.state),
        Some(RunState::Suspended)
    );
}

/// A file suspension store under the test's own directory, holding the commission's Suspension of
/// an `evaluate` request at revision 1, the request `0x301` of the Run `0x101`.
fn holding(name: &str, commission: &Commission<commission_state::Assigned>) -> FileSuspensionStore {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary-w20261022w1-host-stop")
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

fn resume_once(
    case: &CaseId,
    suspensions: &FileSuspensionStore,
    runs: &mut RunStore,
    effects: &Effects,
    context: &mut Context,
) -> RunOutcome {
    let session = Session::proposing(Proposing::One);
    let end = resume_after_grant(
        &admitting_evaluate(case),
        &session,
        &StaticAuthorityProvider::new(),
        effects,
        &commission(case),
        runs,
        suspensions,
        context,
    )
    .unwrap_or_else(|error| panic!("the resume failed: {error:?}"));
    assert_eq!(session.calls(), 0, "a resume's own step runs no executor");
    end.outcome
}

fn stored(suspensions: &FileSuspensionStore, case: &CaseId) -> Option<SuspensionState> {
    suspensions
        .get(&commission(case).data().commission_id)
        .expect("the store reads")
        .map(|held| held.state)
}

/// A resume the host stopped before the take leaves the Suspension `Pending` and the Run
/// suspended for the stop; a later resume, the host no longer stopping, takes it and performs the
/// held request once.
#[test]
fn adversary_a_resume_the_host_stopped_before_the_take_resumes_later() {
    let case = CaseId("case-adversary-h-resume-again".to_owned());
    let suspensions = holding("resume-again", &commission(&case));
    let mut runs = RunStore::new(|| RunId(uuid(0x102)));
    let effects = Effects::answering(Answering::Performed);

    let mut stopped = Context::answering([Some(disk_low())]);
    let first = resume_once(&case, &suspensions, &mut runs, &effects, &mut stopped);
    assert_eq!(first, suspended_for(SuspensionReason::HostStop(disk_low())));
    assert_eq!(stored(&suspensions, &case), Some(SuspensionState::Pending));
    assert_eq!(effects.invoked(), 0);

    let mut going = Context::answering([None]);
    going.step_budget = Some(1);
    let second = resume_once(&case, &suspensions, &mut runs, &effects, &mut going);
    assert!(
        matches!(
            &second,
            RunOutcome::Suspended(RunOutcomeSuspended {
                reason: SuspensionReason::Budget(_)
            })
        ),
        "the resumed step is the step budget's one step: {second:?}"
    );
    assert_eq!(effects.invoked(), 1, "the held request is performed once");
    assert_eq!(
        stored(&suspensions, &case),
        None,
        "the Suspension is released"
    );
    assert_eq!(
        going.asked, 2,
        "asked before the take and before the invoke"
    );
    assert_eq!(
        RunStorage::get(&runs, &RunId(uuid(0x101))).map(|held| held.state),
        Some(RunState::Suspended)
    );
}

/// A resumed request the port answers `Pending` releases the Suspension; when the host stops the
/// run before the request is invoked again, the run ends `Waiting` on it for the host's stop,
/// nothing left `Resuming` in doubt.
#[test]
fn adversary_a_resumed_pending_request_waits_for_the_host_stop_its_suspension_released() {
    let case = CaseId("case-adversary-h-resume-pending".to_owned());
    let suspensions = holding("resume-pending", &commission(&case));
    let mut runs = RunStore::new(|| RunId(uuid(0x102)));
    let effects = Effects::answering(Answering::Pending);
    let mut context = Context::answering([None, None, Some(disk_low())]);

    let outcome = resume_once(&case, &suspensions, &mut runs, &effects, &mut context);

    assert_eq!(
        outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![ActionRequestId(uuid(0x301))],
            reason: SuspensionReason::HostStop(disk_low()),
        })
    );
    assert_eq!(effects.invoked(), 1, "invoked once, not again");
    assert_eq!(context.asked, 3);
    assert_eq!(
        stored(&suspensions, &case),
        None,
        "nothing is left in doubt"
    );
    assert_eq!(
        RunStorage::get(&runs, &RunId(uuid(0x101))).map(|held| held.state),
        Some(RunState::Suspended)
    );
}

/// On a resume the deadline is read first: a reached deadline is the reason, the host is not
/// asked, and the Suspension stays `Pending`.
#[test]
fn adversary_on_a_resume_a_reached_deadline_comes_before_the_host_stop() {
    let case = CaseId("case-adversary-h-resume-deadline".to_owned());
    let suspensions = holding("resume-deadline", &commission(&case));
    let mut runs = RunStore::new(|| RunId(uuid(0x102)));
    let effects = Effects::answering(Answering::Performed);
    let mut context = Context::answering([Some(disk_low())]);
    context.run_budget = Some(RunBudget {
        deadline: Some(Timestamp("2026-10-22T08:30:00Z".to_owned())),
        ..no_limits()
    });

    let outcome = resume_once(&case, &suspensions, &mut runs, &effects, &mut context);

    assert_eq!(
        outcome,
        suspended_for(SuspensionReason::RunBudget(RunBudgetBreach {
            action: None,
            limit: BudgetLimit::Deadline,
            max: 1_792_657_800,
            charged: Some(1_792_659_600),
        }))
    );
    assert_eq!(context.asked, 0, "the host is not asked");
    assert_eq!(stored(&suspensions, &case), Some(SuspensionState::Pending));
    assert_eq!(effects.invoked(), 0);
}
