// SPDX-License-Identifier: Apache-2.0

//! A metered run reports model calls, tokens and cost per step and in total, and its call count
//! equals the `turn` calls a counting fake port saw (`story:dw-run-model-meter`,
//! `ess/domains/metering.yaml`).
//!
//! Every model here is a scripted `ModelPort` that counts its own `turn` calls; nothing calls a
//! live model or the network. The case is open at revision 1 and lists one admissible action,
//! `repository.edit`, which the effect port performs; the step budget ends the run.
//!
//! An executor that is not a model loop reports what each call spent
//! (`AgentExecutor::run_with_usage`, `story:executor-reports-usage`): the meter shows it as that
//! step's usage, the steps still sum to the run, and an executor metered at its model port reports
//! none, so nothing is counted twice.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid as CommissionUuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, CompletionDetermination, EffectOutcome, EffectOutcomePerformed,
    ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorStep, ExecutorUsage, Frontier,
    FrontierAction, FrontierData, FrontierId, FrontierObligation, GovernorError, Observation,
    ObservationId, PrincipalId, ProposedActionArguments, RunId, Unit, commission_state,
    frontier_state, observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_executor::harness::governed::{LoopExecutor, tool_name};
use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::turn_loop::{LoopConfig, RateCard};
use b10x_loom_executor::harness::wire::{
    CallId, Item, ModelPort, Sampling, StopReason, StreamSink, ToolCall, ToolChoice, ToolName,
    TurnOutcome, TurnRequest, Usage, VecSink, WireError, WireId,
};
use b10x_loom_executor::model::metering::RunUsageReport;
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{CatalogueEntry, CommissionRunId, SessionData, SessionId};
use b10x_loom_executor::{
    ArgumentContext, ArgumentGenerator, EmptyObjectArguments, FirstAdmissibleSelector, Loom,
    MeteredExecutor, MeteredModelPort, ReasoningModelSelector, RunMeter,
};
use serde_json::json;

const CASE: &str = "CASE-RUN-METERING";
const EDIT: &str = "repository.edit";
/// The model every scripted turn reports it served.
const MODEL: &str = "m";
/// What every answered turn reports: 100 input tokens, 40 of them cached, and 10 output tokens.
const INPUT: u64 = 100;
const CACHED: u64 = 40;
const OUTPUT: u64 = 10;
/// One answered turn at [`card`]'s rates for [`MODEL`]: 60 uncached input tokens at $1.25/Mtok,
/// 40 cached at $0.125/Mtok and 10 output at $10/Mtok, in micro-US-dollars.
const TURN_MICRO_USD: i64 = 75 + 5 + 100;

fn uuid(n: u64) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

fn frontier_data(case: &CaseId) -> FrontierData {
    FrontierData {
        frontier_id: FrontierId(uuid(0xf1)),
        case_id: case.clone(),
        case_revision: 1,
        claims: Vec::new(),
        obligations: vec![FrontierObligation {
            obligation: "tests-pass".to_owned(),
            open: true,
        }],
        actions: vec![FrontierAction {
            action: EDIT.to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    }
}

/// The case: open at revision 1, its frontier listing the admissible edit, whatever is performed.
struct Case;

impl Governor for Case {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(1)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        Ok(Frontier::new(frontier_data(case)))
    }

    fn completion(&self, _case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        Ok(CompletionDetermination::Open(Unit(true)))
    }
}

impl ObservationPort for Case {
    fn observe(
        &self,
        _observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        Ok(())
    }
}

/// Performs every action and changes nothing.
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
            report: CommissionValue::Null,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// Fresh ids, a fixed trusted time and a step budget.
struct Context {
    ids: u64,
    budget: usize,
}

impl Context {
    fn budget(budget: usize) -> Self {
        Self { ids: 0, budget }
    }
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids += 1;
        ActionRequestId(uuid(0x300 + self.ids))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids += 1;
        ObservationId(uuid(0x400 + self.ids))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-14T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        Some(self.budget)
    }
}

/// `run_until_blocked` on [`Case`] with `executor`, ending at `budget` steps at the latest.
fn run(executor: &(impl AgentExecutor + ?Sized), budget: usize) -> LoopEnd {
    let ids = AtomicU64::new(0x500);
    let mut runs = Generated::new(RunStore::new(move || {
        RunId(uuid(ids.fetch_add(1, Ordering::SeqCst)))
    }));
    run_until_blocked(
        &Case,
        executor,
        &StaticAuthorityProvider::new(),
        &Effects,
        &commission(),
        &mut runs,
        &mut Context::budget(budget),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

fn usage() -> Usage {
    Usage {
        model: MODEL.to_owned(),
        input_tokens: INPUT,
        output_tokens: OUTPUT,
        cached_input_tokens: CACHED,
        cache_creation_input_tokens: None,
    }
}

/// A turn that calls `tool` with `arguments`, reporting [`usage`].
fn calls(id: &str, tool: ToolName, arguments: serde_json::Value) -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::ToolCalls,
        items: vec![Item::ToolCall(ToolCall {
            call_id: CallId::new(id).expect("a valid call id"),
            name: tool,
            arguments,
        })],
        usage: Some(usage()),
    }
}

/// The loop's turn that calls the edit.
fn calls_edit(id: &str) -> TurnOutcome {
    calls(id, tool_name(EDIT).expect("a valid tool name"), json!({}))
}

/// A turn that answers in prose, reporting [`usage`].
fn prose() -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::EndTurn,
        items: vec![Item::assistant("{}")],
        usage: Some(usage()),
    }
}

/// A turn with no tools that asks [`MODEL`] about `text` under `instructions`.
fn turn_request(instructions: &str, text: &str) -> TurnRequest {
    TurnRequest {
        model: MODEL.to_owned(),
        instructions: instructions.to_owned(),
        items: vec![Item::user(text)],
        tools: Vec::new(),
        max_output_tokens: None,
        sampling: Sampling::default(),
        tool_choice: ToolChoice::Auto,
    }
}

/// What a scripted port answers once its script is spent.
type Responder = fn(&TurnRequest) -> Result<TurnOutcome, WireError>;

/// Fails every turn: a script that ran out.
fn spent(_request: &TurnRequest) -> Result<TurnOutcome, WireError> {
    Err(WireError::protocol("the script has no further turn"))
}

/// The selector's turn picks the edit; any other turn answers in prose.
fn selects_edit(request: &TurnRequest) -> Result<TurnOutcome, WireError> {
    let selecting = request
        .tools
        .iter()
        .any(|tool| tool.name.as_str() == "select_action");
    Ok(if selecting {
        calls(
            "select",
            ToolName::new("select_action").expect("a valid tool name"),
            json!({ "action": EDIT }),
        )
    } else {
        prose()
    })
}

/// What a scripted port holds, shared by its clones.
struct Script {
    answers: VecDeque<Result<TurnOutcome, WireError>>,
    then: Responder,
    /// Every `turn` call this port's clones received, failed or answered.
    seen: u64,
}

/// A scripted `ModelPort` that counts its own `turn` calls. Its clones share the script and the
/// count, so a selector and an argument generator can hold one port between them.
#[derive(Clone)]
struct Port {
    wire: WireId,
    script: Arc<Mutex<Script>>,
}

impl Port {
    fn new(
        answers: impl IntoIterator<Item = Result<TurnOutcome, WireError>>,
        then: Responder,
    ) -> Self {
        Self {
            wire: WireId::new(responses::WIRE).expect("a valid wire id"),
            script: Arc::new(Mutex::new(Script {
                answers: answers.into_iter().collect(),
                then,
                seen: 0,
            })),
        }
    }

    /// The `turn` calls the port saw.
    fn seen(&self) -> u64 {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .seen
    }
}

impl ModelPort for Port {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        let mut script = self.script.lock().unwrap_or_else(PoisonError::into_inner);
        script.seen += 1;
        match script.answers.pop_front() {
            Some(answer) => answer,
            None => (script.then)(request),
        }
    }
}

/// An argument generator that asks the model: one turn through its port per generation.
struct ModelArguments {
    port: Mutex<MeteredModelPort<Port>>,
}

impl ArgumentGenerator for ModelArguments {
    fn generate(
        &self,
        context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<CommissionValue, String> {
        let request = turn_request("Write the arguments.", &context.prompt);
        self.port
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .turn(&request, &mut VecSink::new())
            .map(|_| CommissionValue::Object(Vec::new()))
            .map_err(|error| error.to_string())
    }
}

/// Wraps an executor and keeps, for each `run` call, the `turn` calls `port` saw during it.
struct Observed<'p, E> {
    inner: E,
    port: &'p Port,
    spans: RefCell<Vec<u64>>,
}

impl<'p, E> Observed<'p, E> {
    fn new(inner: E, port: &'p Port) -> Self {
        Self {
            inner,
            port,
            spans: RefCell::new(Vec::new()),
        }
    }

    fn spans(&self) -> Vec<i64> {
        self.spans
            .borrow()
            .iter()
            .map(|&calls| i64::try_from(calls).expect("a small count"))
            .collect()
    }
}

impl<E: AgentExecutor> AgentExecutor for Observed<'_, E> {
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let before = self.port.seen();
        let outcome = self.inner.run(commission, frontier);
        self.spans.borrow_mut().push(self.port.seen() - before);
        outcome
    }
}

/// The pipeline Loom whose reasoning-model selector and model argument generator share `port`,
/// both metered into `meter`.
fn reasoning_loom(
    port: &Port,
    meter: &RunMeter,
) -> Loom<ReasoningModelSelector<MeteredModelPort<Port>>, ModelArguments> {
    Loom::new(
        ReasoningModelSelector::new(MeteredModelPort::new(port.clone(), meter), MODEL),
        ModelArguments {
            port: Mutex::new(MeteredModelPort::new(port.clone(), meter)),
        },
        "Make the change.",
    )
}

fn card(model: &str) -> RateCard {
    RateCard::parse(&format!(
        r#"{{
            "source": "a table the test declares",
            "as_of": "2026-10-14",
            "models": {{"{model}": {{
                "input_usd_per_mtok": 1.25,
                "cached_input_usd_per_mtok": 0.125,
                "output_usd_per_mtok": 10.0
            }}}}
        }}"#
    ))
    .expect("a valid card")
}

fn step_calls(report: &RunUsageReport) -> Vec<i64> {
    report.steps.iter().map(|step| step.model_calls).collect()
}

/// The pipeline Loom of [`reasoning_loom`], metered under `prices`, run for two steps.
fn reasoning_run(prices: Option<RateCard>) -> (RunUsageReport, Vec<i64>, u64) {
    let port = Port::new([], selects_edit);
    let meter = RunMeter::new(prices);
    let executor = Observed::new(
        MeteredExecutor::new(reasoning_loom(&port, &meter), &meter),
        &port,
    );
    let end = run(&executor, 2);
    (meter.report(&end.run_id), executor.spans(), port.seen())
}

#[test]
fn the_reported_calls_equal_the_turns_the_model_port_saw() {
    let port = Port::new(
        [
            Ok(calls_edit("call-1")),
            // Retried by the loop: a failed turn is a call too.
            Err(WireError::transport("the connection dropped")),
            Ok(calls_edit("call-2")),
            Ok(calls_edit("call-3")),
        ],
        spent,
    );
    let meter = RunMeter::new(None);
    let loom = Loom::new(
        FirstAdmissibleSelector,
        EmptyObjectArguments,
        "Make the change.",
    )
    .with_governor(&Case);
    let looped = LoopExecutor::new(
        &loom,
        MeteredModelPort::new(port.clone(), &meter),
        LoopConfig::new(MODEL, "INSTRUCTIONS").with_retry_backoff(Duration::from_millis(1)),
        SessionData {
            session_id: SessionId(Uuid("00000000-0000-4000-8000-00000000e001".to_owned())),
            commission_run: CommissionRunId(Uuid(
                "00000000-0000-4000-8000-00000000e0ab".to_owned(),
            )),
            wire: responses::WIRE.to_owned(),
            boundary_refusals: 0,
        },
    );
    let executor = Observed::new(MeteredExecutor::new(looped, &meter), &port);

    let end = run(&executor, 3);
    let report = meter.report(&end.run_id);

    assert!(
        port.seen() >= 2,
        "the run made more than one model call: {} after {:?}",
        port.seen(),
        end.outcome
    );
    assert_eq!(
        report.model_calls,
        i64::try_from(port.seen()).expect("a small count"),
        "the report counts every turn the port saw: {report:?}"
    );
    assert_eq!(
        report
            .steps
            .iter()
            .map(|step| step.model_calls)
            .sum::<i64>(),
        report.model_calls,
        "the steps' calls sum to the total: {report:?}"
    );
    assert_eq!(
        step_calls(&report),
        executor.spans(),
        "one step per executor call, each with the turns made during it"
    );
    assert_eq!(
        report
            .steps
            .iter()
            .map(|step| step.step)
            .collect::<Vec<_>>(),
        (1..=i64::try_from(executor.spans().len()).expect("a small count")).collect::<Vec<_>>(),
        "steps are counted from 1"
    );
    assert_eq!(
        report.commission_run.0.0, end.run_id.0.0,
        "the report names its run"
    );
}

#[test]
fn a_step_counts_the_selector_and_argument_calls_made_in_it() {
    let (report, spans, seen) = reasoning_run(None);

    assert_eq!(
        spans,
        vec![2, 2],
        "each executor call made a selection and an argument turn"
    );
    assert_eq!(
        step_calls(&report),
        spans,
        "each step's calls equal the turns made during that executor call: {report:?}"
    );
    assert_eq!(
        report.model_calls,
        i64::try_from(seen).expect("a small count")
    );
}

#[test]
fn tokens_and_cost_per_step_sum_to_the_run_total() {
    let (report, spans, _) = reasoning_run(Some(card(MODEL)));

    assert_eq!(step_calls(&report), spans);
    for step in &report.steps {
        let calls = step.model_calls;
        assert_eq!(
            step.input_tokens,
            calls * i64::try_from(INPUT).expect("small")
        );
        assert_eq!(
            step.output_tokens,
            calls * i64::try_from(OUTPUT).expect("small")
        );
        assert_eq!(
            step.cached_input_tokens,
            calls * i64::try_from(CACHED).expect("small")
        );
        assert_eq!(
            step.cost_micro_usd,
            Some(calls * TURN_MICRO_USD),
            "each turn priced and rounded once: {step:?}"
        );
    }
    let sum = |field: fn(&b10x_loom_executor::model::metering::StepUsage) -> i64| {
        report.steps.iter().map(field).sum::<i64>()
    };
    assert_eq!(report.input_tokens, sum(|step| step.input_tokens));
    assert_eq!(report.output_tokens, sum(|step| step.output_tokens));
    assert_eq!(
        report.cached_input_tokens,
        sum(|step| step.cached_input_tokens)
    );
    assert_eq!(
        report.cost_micro_usd,
        Some(sum(|step| step.cost_micro_usd.expect("priced"))),
        "the run's cost is the sum of its steps': {report:?}"
    );
    assert!(report.cost_micro_usd > Some(0));
}

#[test]
fn cost_is_absent_without_a_rate_card_and_for_an_unlisted_model() {
    for (prices, why) in [
        (None, "no rate card"),
        (
            Some(card("somebody-elses-model")),
            "a card that does not list the model",
        ),
    ] {
        let (report, _, _) = reasoning_run(prices);
        assert!(report.model_calls > 0, "{why}: the run made calls");
        assert!(report.input_tokens > 0, "{why}: tokens are still counted");
        assert_eq!(
            report.cost_micro_usd, None,
            "{why}: the run's cost is absent, never zero"
        );
        for step in &report.steps {
            assert_eq!(step.cost_micro_usd, None, "{why}: {step:?}");
        }
    }
}

/// An executor that makes one turn through its metered port per `run` call, and suspends.
struct OneTurn {
    port: Mutex<MeteredModelPort<Port>>,
}

impl AgentExecutor for OneTurn {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let request = turn_request("Answer.", "go");
        let answered = self
            .port
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .turn(&request, &mut VecSink::new());
        assert!(answered.is_err(), "the scripted turn fails");
        ExecutorOutcome::NoUsefulAction(Unit(true))
    }
}

#[test]
fn a_failed_turn_is_a_call_without_usage() {
    let port = Port::new([Err(WireError::transport("the connection dropped"))], spent);
    let meter = RunMeter::new(Some(card(MODEL)));
    let executor = MeteredExecutor::new(
        OneTurn {
            port: Mutex::new(MeteredModelPort::new(port.clone(), &meter)),
        },
        &meter,
    );
    let frontier = Frontier::new(frontier_data(&CaseId(CASE.to_owned())));
    let _ = executor.run(&commission(), &frontier);

    let report = meter.report(&RunId(uuid(0x501)));
    assert_eq!(port.seen(), 1);
    assert_eq!(
        report.model_calls, 1,
        "the failed turn is a call: {report:?}"
    );
    assert_eq!(report.steps.len(), 1);
    assert_eq!(report.steps[0].model_calls, 1);
    assert_eq!(
        (
            report.input_tokens,
            report.output_tokens,
            report.cached_input_tokens
        ),
        (0, 0, 0),
        "and it adds no tokens: {report:?}"
    );
}

/// What each call of [`Session`] reports it spent: 1,000 input and 50 output tokens over two
/// seconds, at a cost of 300 micro-US-dollars.
fn session_usage() -> ExecutorUsage {
    ExecutorUsage {
        input_tokens: 1000,
        output_tokens: 50,
        duration_ms: 2000,
        cost_micro_usd: Some(300),
    }
}

/// An executor over an agent session, not a model loop: it calls no model port of the run, proposes
/// the edit, and reports [`session_usage`] as what each call spent. With `port`, each call also
/// makes one turn through it, which the meter counts as a model call.
struct Session {
    port: Option<Mutex<MeteredModelPort<Port>>>,
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
        if let Some(port) = &self.port {
            let request = turn_request("Answer.", "go");
            let answered = port
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .turn(&request, &mut VecSink::new());
            assert!(answered.is_ok(), "the scripted turn answers");
        }
        ExecutorStep {
            outcome: ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
                action: EDIT.to_owned(),
                arguments: ProposedActionArguments(CommissionValue::Object(Vec::new())),
            }),
            usage: Some(session_usage()),
        }
    }
}

#[test]
fn a_non_model_executor_usage_is_its_step_usage() {
    // No rate card: the session priced its own calls, and nothing else needed a price.
    let meter = RunMeter::new(None);
    let executor = MeteredExecutor::new(Session { port: None }, &meter);

    let end = run(&executor, 2);
    meter.charge_effects(&end.charges);
    let report = meter.report(&end.run_id);

    assert_eq!(
        report
            .steps
            .iter()
            .map(|step| (
                step.step,
                step.model_calls,
                step.input_tokens,
                step.output_tokens,
                step.cached_input_tokens,
                step.cost_micro_usd
            ))
            .collect::<Vec<_>>(),
        vec![
            (1, 0, 1000, 50, 0, Some(300)),
            (2, 0, 1000, 50, 0, Some(300))
        ],
        "each executor call's usage is its step's usage, and no model call: {report:?}"
    );
    assert_eq!(
        (
            report.model_calls,
            report.input_tokens,
            report.output_tokens,
            report.cost_micro_usd
        ),
        (0, 2000, 100, Some(600)),
        "the steps sum to the run: {report:?}"
    );
    assert_eq!(
        end.charges.run.input_tokens, 2000,
        "the runtime charged the same usage to the run"
    );
    assert_eq!(
        (
            report.effects.input_tokens,
            report.effects.output_tokens,
            report.effects.cost_micro_usd
        ),
        (0, 0, Some(0)),
        "the report's effect usage holds what the effects reported, so the executor's usage is \
         counted once, in its steps: {report:?}"
    );
}

#[test]
fn a_step_sums_its_metered_turns_and_the_usage_its_executor_reports() {
    let port = Port::new([], |_| Ok(prose()));
    let meter = RunMeter::new(Some(card(MODEL)));
    let executor = MeteredExecutor::new(
        Session {
            port: Some(Mutex::new(MeteredModelPort::new(port.clone(), &meter))),
        },
        &meter,
    );

    let end = run(&executor, 1);
    let report = meter.report(&end.run_id);

    assert_eq!(port.seen(), 1);
    assert_eq!(report.steps.len(), 1, "{report:?}");
    let step = &report.steps[0];
    assert_eq!(step.model_calls, 1, "only the metered turn is a model call");
    assert_eq!(
        (
            step.input_tokens,
            step.output_tokens,
            step.cached_input_tokens
        ),
        (
            i64::try_from(INPUT).expect("small") + 1000,
            i64::try_from(OUTPUT).expect("small") + 50,
            i64::try_from(CACHED).expect("small")
        ),
        "the turn's tokens and the reported ones: {step:?}"
    );
    assert_eq!(
        step.cost_micro_usd,
        Some(TURN_MICRO_USD + 300),
        "the priced turn and the reported cost: {step:?}"
    );
    assert_eq!(report.cost_micro_usd, step.cost_micro_usd);
}

#[test]
fn an_executor_metered_at_its_model_port_reports_no_usage_and_is_counted_once() {
    let port = Port::new([], selects_edit);
    let meter = RunMeter::new(Some(card(MODEL)));
    let executor = MeteredExecutor::new(reasoning_loom(&port, &meter), &meter);

    let end = run(&executor, 2);
    meter.charge_effects(&end.charges);
    let report = meter.report(&end.run_id);

    let seen = i64::try_from(port.seen()).expect("a small count");
    assert_eq!(report.model_calls, seen, "{report:?}");
    assert_eq!(
        report.input_tokens,
        seen * i64::try_from(INPUT).expect("small"),
        "each turn's tokens are counted once, at the port: {report:?}"
    );
    assert_eq!(report.cost_micro_usd, Some(seen * TURN_MICRO_USD));
    assert_eq!(
        (
            end.charges.run.input_tokens,
            end.charges.run.output_tokens,
            end.charges.run.cost_micro_usd
        ),
        (0, 0, Some(0)),
        "the executor reports no usage, so its turns are charged nowhere a second time"
    );
    assert_eq!(report.effects.input_tokens, 0, "{report:?}");
}
