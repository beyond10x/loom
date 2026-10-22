//! Adversary cases for `story:host-stop-when-a-step-follows`.
//!
//! On a frontier only a grant moves, the host is no longer asked before the executor call; it is
//! asked right before the effect of a request the provider allowed. Under
//! `StopPolicy.on_breach: KeepStep` that call may have breached a run limit (its reported usage past
//! `max_tokens`, or its end past the deadline), and the breach is kept. `AGENTS.md` (§ Commission)
//! states that a kept breach stops the run before the host is asked, and the runtime gives a kept
//! breach as the reason over a pending answer that ends the run (`wait_reason`). These cases hold
//! the kept breach as the reason when the host stops the granted request, and nothing invoked past
//! a stop the host named.

use std::sync::atomic::{AtomicUsize, Ordering};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict, BreachStep,
    BudgetLimit, CaseId, Commission, CommissionData, CommissionId, EffectOutcome,
    EffectOutcomePerformed, ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorStep,
    ExecutorUsage, Frontier, FrontierAction, HostStop, ObservationId, PrincipalId,
    ProposedActionArguments, RunBudget, RunBudgetBreach, RunId, RunOutcome, RunOutcomeSuspended,
    StopPolicy, SuspensionReason, Unit, commission_state, frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

/// The one action the frontier lists, `ApprovalRequired`, naming itself as the capability.
const MERGE: &str = "repository.merge";

/// The trusted time before the executor call.
const BEFORE: &str = "2026-10-22T09:00:00Z";
/// The deadline, between [`BEFORE`] and [`AFTER`].
const DEADLINE: &str = "2026-10-22T09:30:00Z";
/// `DEADLINE` in whole seconds since the Unix epoch.
const DEADLINE_SECONDS: i64 = 1_792_661_400;
/// The trusted time once the executor was called.
const AFTER: &str = "2026-10-22T10:00:00Z";
/// `AFTER` in whole seconds since the Unix epoch.
const AFTER_SECONDS: i64 = 1_792_663_200;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x81)),
        agent_revision_id: AgentRevisionId(uuid(0x82)),
        case_id: case.clone(),
        principal: PrincipalId("principal-a2".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// The case open at revision 1 on every call, its frontier listing only the merge, which needs a
/// grant: the host is not asked before the executor call.
fn only_a_grant_moves(case: &CaseId) -> FakeGovernor {
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [Answer::at(1).with_items(
            Vec::new(),
            Vec::new(),
            vec![FrontierAction {
                action: MERGE.to_owned(),
                status: ActionStatus::ApprovalRequired,
                capability: Some(MERGE.to_owned()),
                reasons: vec!["the merge needs a person's approval".to_owned()],
            }],
        )],
    );
    governor
}

/// A provider that allows the merge.
fn allowing() -> StaticAuthorityProvider {
    StaticAuthorityProvider::new().answer(MERGE, AuthorityVerdict::Allow(Unit(true)))
}

/// An executor over an agent session: each call proposes the merge with arguments of its own and
/// reports `usage` as what it spent.
struct Session {
    usage: Option<ExecutorUsage>,
    calls: AtomicUsize,
}

impl Session {
    fn reporting(usage: Option<ExecutorUsage>) -> Self {
        Self {
            usage,
            calls: AtomicUsize::new(0),
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
        ExecutorStep {
            outcome: ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
                action: MERGE.to_owned(),
                arguments: ProposedActionArguments(Value::Number(call.to_string())),
            }),
            usage: self.usage.clone(),
            transcript_digest: None,
            summary: None,
        }
    }
}

/// What one executor call reports it spent: `input_tokens` in, `output_tokens` out.
fn call_usage(input_tokens: i64, output_tokens: i64) -> ExecutorUsage {
    ExecutorUsage {
        input_tokens,
        output_tokens,
        duration_ms: 500,
        cost_micro_usd: Some(3),
    }
}

/// An effect port performing every action, counting its invocations.
#[derive(Default)]
struct Effects {
    invoked: AtomicUsize,
}

impl Effects {
    fn invoked(&self) -> usize {
        self.invoked.load(Ordering::SeqCst)
    }
}

impl EffectPort for Effects {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.invoked.fetch_add(1, Ordering::SeqCst);
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Text("merged".to_owned()),
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// What the context's trusted clock reads.
enum Clock<'a> {
    /// One reading, always.
    Fixed(&'static str),
    /// [`BEFORE`] until the session was called once, [`AFTER`] from then on: the first call ends
    /// past the deadline.
    PastDeadlineAfterFirstCall(&'a Session),
}

/// New ids from counters, the clock it was given, no step budget, the run budget it was given, the
/// approval gate off and every breach kept (`KeepStep`), and a host that always stops for low
/// disk, counting each time it was asked.
struct Context<'a> {
    requests: u64,
    observations: u64,
    budget: RunBudget,
    clock: Clock<'a>,
    asked: usize,
}

impl<'a> Context<'a> {
    fn keeping(budget: RunBudget, clock: Clock<'a>) -> Self {
        Self {
            requests: 0,
            observations: 0,
            budget,
            clock,
            asked: 0,
        }
    }
}

impl LoopContext for Context<'_> {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x300 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x400 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        let reading = match &self.clock {
            Clock::Fixed(reading) => *reading,
            Clock::PastDeadlineAfterFirstCall(session) if session.calls() == 0 => BEFORE,
            Clock::PastDeadlineAfterFirstCall(_) => AFTER,
        };
        Timestamp(reading.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }

    fn run_budget(&self) -> Option<RunBudget> {
        Some(self.budget.clone())
    }

    fn stop_policy(&self) -> StopPolicy {
        StopPolicy {
            approval_gate: false,
            idle_bound: None,
            on_breach: BreachStep::KeepStep,
            breach_by_limit: Vec::new(),
        }
    }

    fn host_stop(&mut self) -> Option<HostStop> {
        self.asked += 1;
        Some(disk_low())
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

fn unlimited() -> RunBudget {
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

fn suspended_for(breach: RunBudgetBreach) -> RunOutcome {
    RunOutcome::Suspended(RunOutcomeSuspended {
        reason: SuspensionReason::RunBudget(breach),
    })
}

/// `run_until_blocked` of `session` on [`only_a_grant_moves`] under `context`, the provider
/// allowing the merge.
fn run(session: &Session, effects: &Effects, context: &mut Context<'_>) -> LoopEnd {
    let case = CaseId("case-adversary-w2-host-stop".to_owned());
    let governor = only_a_grant_moves(&case);
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x101))));
    run_until_blocked(
        &governor,
        session,
        &allowing(),
        effects,
        &commission(&case),
        &mut runs,
        context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"))
}

/// The executor call passes `max_tokens` (60 + 50 of 100) and `KeepStep` keeps it; its merge is
/// allowed and the host, asked only now, stops. A kept breach comes before the host stop
/// (`AGENTS.md` § Commission: "The step budget, a kept breach, `NoPerformableAction` and the
/// approval gate stop the run before the host is asked"), and a kept breach is the reason the
/// runtime gives over a pending answer that ends the run; the Run is suspended for the spending
/// limit, never for `HostStop`.
#[test]
fn adversary_a_kept_spending_breach_is_the_reason_when_the_host_stops_a_granted_request() {
    let session = Session::reporting(Some(call_usage(60, 50)));
    let effects = Effects::default();
    let budget = RunBudget {
        max_tokens: Some(100),
        ..unlimited()
    };
    let mut context = Context::keeping(budget, Clock::Fixed(BEFORE));

    let end = run(&session, &effects, &mut context);

    assert_eq!(session.calls(), 1, "the executor runs once, unasked");
    if context.asked > 0 {
        assert_eq!(
            effects.invoked(),
            0,
            "nothing is invoked past a stop the host named"
        );
    }
    assert_eq!(
        end.outcome,
        suspended_for(RunBudgetBreach {
            action: None,
            limit: BudgetLimit::MaxTokens,
            max: 100,
            charged: Some(110),
        }),
        "the breach the kept step passed is the reason, not the host's stop"
    );
}

/// The same with the deadline: the executor call ends past it and `KeepStep` keeps the step; the
/// Run is suspended for the deadline, never for `HostStop`.
#[test]
fn adversary_a_kept_deadline_breach_is_the_reason_when_the_host_stops_a_granted_request() {
    let session = Session::reporting(None);
    let effects = Effects::default();
    let budget = RunBudget {
        deadline: Some(Timestamp(DEADLINE.to_owned())),
        ..unlimited()
    };
    let mut context = Context::keeping(budget, Clock::PastDeadlineAfterFirstCall(&session));

    let end = run(&session, &effects, &mut context);

    assert_eq!(session.calls(), 1, "the executor runs once, unasked");
    if context.asked > 0 {
        assert_eq!(
            effects.invoked(),
            0,
            "nothing is invoked past a stop the host named"
        );
    }
    assert_eq!(
        end.outcome,
        suspended_for(RunBudgetBreach {
            action: None,
            limit: BudgetLimit::Deadline,
            max: DEADLINE_SECONDS,
            charged: Some(AFTER_SECONDS),
        }),
        "the deadline the kept step passed is the reason, not the host's stop"
    );
}
