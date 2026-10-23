//! Acceptance for `story:loop-context-split`: `LoopContext` splits into `Ids`, `Clock`, `Limits`
//! and `HostControl`, and every type implementing the four is a `LoopContext`. A host can then
//! keep its limits per agent and its stop and event sink per process, and assemble a run's context
//! from those parts. A context assembled from four parts gives the same `LoopEnd`, the same events
//! and the same host asks as a single `LoopContext` holding the same ids, clock, limits and stop,
//! on a run the host stops, one its run budget's deadline stops and one its step budget stops.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, BreachStep, CaseId,
    Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed,
    ExecutorOutcome, ExecutorOutcomeProposedAction, Frontier, FrontierAction, HostStop,
    ObservationId, PrincipalId, ProposedActionArguments, RunBudget, RunId, RunOutcome,
    RuntimeEvent, StopPolicy, SuspensionReason, commission_state, frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::events::EventSink;
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::runtime::{
    Clock, HostControl, Ids, Limits, LoopContext, LoopEnd, run_until_blocked,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

const EVALUATE: &str = "evaluate";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x91)),
        agent_revision_id: AgentRevisionId(uuid(0x92)),
        case_id: case.clone(),
        principal: PrincipalId("principal-split".to_owned()),
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

/// Each call proposes `evaluate` with arguments of its own, so each is progress.
#[derive(Default)]
struct Session {
    calls: AtomicUsize,
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

/// Performs every request.
struct Effects;

impl EffectPort for Effects {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Text("evaluated".to_owned()),
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// What a scenario sets: the limits, the trusted time and the host's answers to `host_stop`.
#[derive(Clone)]
struct Scenario {
    name: &'static str,
    step_budget: Option<usize>,
    run_budget: Option<RunBudget>,
    stop_policy: StopPolicy,
    clock: &'static str,
    answers: Vec<Option<HostStop>>,
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

/// A stop policy other than the default, so the run reads the one its context gives.
fn policy() -> StopPolicy {
    StopPolicy {
        approval_gate: false,
        idle_bound: Some(3),
        on_breach: BreachStep::DiscardStep,
        breach_by_limit: Vec::new(),
    }
}

fn disk_low() -> HostStop {
    HostStop {
        name: "DiskLow".to_owned(),
        detail: Value::Text("target".to_owned()),
    }
}

/// The run the host stops before its second step.
fn host_stopped() -> Scenario {
    Scenario {
        name: "host-stop",
        step_budget: None,
        run_budget: Some(RunBudget {
            deadline: Some("2026-10-23T12:00:00Z".to_owned()),
            ..unlimited()
        }),
        stop_policy: policy(),
        clock: "2026-10-23T09:00:00Z",
        answers: vec![None, Some(disk_low())],
    }
}

/// The run its run budget's deadline stops before the first step.
fn deadline_stopped() -> Scenario {
    Scenario {
        name: "deadline",
        step_budget: Some(5),
        run_budget: Some(RunBudget {
            deadline: Some("2026-10-23T08:00:00Z".to_owned()),
            ..unlimited()
        }),
        stop_policy: policy(),
        clock: "2026-10-23T09:00:00Z",
        answers: vec![None],
    }
}

/// The run its step budget stops after two steps.
fn step_budget_stopped() -> Scenario {
    Scenario {
        name: "step-budget",
        step_budget: Some(2),
        run_budget: None,
        stop_policy: policy(),
        clock: "2026-10-23T09:00:00Z",
        answers: vec![None],
    }
}

/// New ids from counters.
struct Counters {
    requests: u64,
    observations: u64,
}

impl Counters {
    fn new() -> Self {
        Self {
            requests: 0,
            observations: 0,
        }
    }

    fn request(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x300 + self.requests))
    }

    fn observation(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x400 + self.observations))
    }
}

/// A host answering `host_stop` with its answers in turn, the last one from then on, counting
/// each ask, and recording every event.
struct Host {
    answers: VecDeque<Option<HostStop>>,
    asked: usize,
    events: Vec<RuntimeEvent>,
}

impl Host {
    fn new(answers: &[Option<HostStop>]) -> Self {
        Self {
            answers: answers.iter().cloned().collect(),
            asked: 0,
            events: Vec::new(),
        }
    }

    fn stop(&mut self) -> Option<HostStop> {
        self.asked += 1;
        if self.answers.len() > 1 {
            self.answers.pop_front().flatten()
        } else {
            self.answers.front().cloned().flatten()
        }
    }
}

/// The single-trait context: one value holding everything, implementing `LoopContext` itself.
struct Whole {
    ids: Counters,
    clock: &'static str,
    scenario: Scenario,
    host: Host,
}

impl LoopContext for Whole {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids.request()
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids.observation()
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(self.clock.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.scenario.step_budget
    }

    fn run_budget(&self) -> Option<RunBudget> {
        self.scenario.run_budget.clone()
    }

    fn stop_policy(&self) -> StopPolicy {
        self.scenario.stop_policy.clone()
    }

    fn host_stop(&mut self) -> Option<HostStop> {
        self.host.stop()
    }

    fn events(&mut self) -> &mut dyn EventSink {
        &mut self.host.events
    }
}

/// An agent's limits, kept per agent.
struct AgentLimits {
    step_budget: Option<usize>,
    run_budget: Option<RunBudget>,
    stop_policy: StopPolicy,
}

impl Limits for AgentLimits {
    fn step_budget(&self) -> Option<usize> {
        self.step_budget
    }

    fn run_budget(&self) -> Option<RunBudget> {
        self.run_budget.clone()
    }

    fn stop_policy(&self) -> StopPolicy {
        self.stop_policy.clone()
    }
}

/// A fixed trusted clock.
struct FixedClock(&'static str);

impl Clock for FixedClock {
    fn now(&mut self) -> Timestamp {
        Timestamp(self.0.to_owned())
    }
}

impl Ids for Counters {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.request()
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observation()
    }
}

/// The process's stop and event sink, kept per process and lent to each run.
impl HostControl for Host {
    fn host_stop(&mut self) -> Option<HostStop> {
        self.stop()
    }

    fn events(&mut self) -> &mut dyn EventSink {
        &mut self.events
    }
}

/// A context assembled from four parts: its own ids and clock, the agent's limits, borrowed, and
/// the process's host control, borrowed mutably. It implements the four traits, never
/// `LoopContext` itself.
struct Assembled<'a> {
    ids: Counters,
    clock: FixedClock,
    limits: &'a AgentLimits,
    host: &'a mut Host,
}

impl Ids for Assembled<'_> {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids.action_request_id()
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids.observation_id()
    }
}

impl Clock for Assembled<'_> {
    fn now(&mut self) -> Timestamp {
        self.clock.now()
    }
}

impl Limits for Assembled<'_> {
    fn step_budget(&self) -> Option<usize> {
        self.limits.step_budget()
    }

    fn run_budget(&self) -> Option<RunBudget> {
        self.limits.run_budget()
    }

    fn stop_policy(&self) -> StopPolicy {
        self.limits.stop_policy()
    }
}

impl HostControl for Assembled<'_> {
    fn host_stop(&mut self) -> Option<HostStop> {
        self.host.host_stop()
    }

    fn events(&mut self) -> &mut dyn EventSink {
        self.host.events()
    }
}

/// `run_until_blocked` of a fresh session under `context`, on a fresh governor and run store.
fn run<C: LoopContext>(name: &str, context: &mut C) -> LoopEnd {
    let case = CaseId(format!("case-split-{name}"));
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x101))));
    run_until_blocked(
        &governor(&case),
        &Session::default(),
        &StaticAuthorityProvider::new(),
        &Effects,
        &commission(&case),
        &mut runs,
        context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"))
}

/// The scenario run once under a single `LoopContext` and once under one assembled from four
/// parts: the two ends, events and host asks.
fn both(
    scenario: &Scenario,
) -> (
    (LoopEnd, Vec<RuntimeEvent>, usize),
    (LoopEnd, Vec<RuntimeEvent>, usize),
) {
    let mut whole = Whole {
        ids: Counters::new(),
        clock: scenario.clock,
        scenario: scenario.clone(),
        host: Host::new(&scenario.answers),
    };
    let single = run(scenario.name, &mut whole);

    let limits = AgentLimits {
        step_budget: scenario.step_budget,
        run_budget: scenario.run_budget.clone(),
        stop_policy: scenario.stop_policy.clone(),
    };
    let mut host = Host::new(&scenario.answers);
    let mut assembled = Assembled {
        ids: Counters::new(),
        clock: FixedClock(scenario.clock),
        limits: &limits,
        host: &mut host,
    };
    let parts = run(scenario.name, &mut assembled);

    (
        (single, whole.host.events, whole.host.asked),
        (parts, host.events, host.asked),
    )
}

#[test]
fn a_context_from_four_parts_runs_as_one() {
    for scenario in [host_stopped(), deadline_stopped(), step_budget_stopped()] {
        let ((single, single_events, single_asked), (parts, parts_events, parts_asked)) =
            both(&scenario);

        let reason = match &single.outcome {
            RunOutcome::Suspended(suspended) => &suspended.reason,
            other => panic!("{}: the run ends suspended, not {other:?}", scenario.name),
        };
        match (scenario.name, reason) {
            ("host-stop", SuspensionReason::HostStop(stop)) => assert_eq!(stop, &disk_low()),
            ("deadline", SuspensionReason::RunBudget(_))
            | ("step-budget", SuspensionReason::Budget(_)) => {}
            (name, reason) => panic!("{name}: the run stops for its scenario, not {reason:?}"),
        }
        assert!(
            !single_events.is_empty(),
            "{}: the sink the context gives receives the run's events",
            scenario.name
        );

        assert_eq!(
            parts, single,
            "{}: the assembled context ends the run as the single one",
            scenario.name
        );
        assert_eq!(
            parts_events, single_events,
            "{}: the assembled context's sink receives the same events",
            scenario.name
        );
        assert_eq!(
            parts_asked, single_asked,
            "{}: the host is asked as often",
            scenario.name
        );
    }
}
