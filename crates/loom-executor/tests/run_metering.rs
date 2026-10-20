// SPDX-License-Identifier: Apache-2.0

//! A metered run reports model calls, tokens and cost per step and in total, and its call count
//! equals the `turn` calls a counting fake model saw (`story:dw-run-model-meter`,
//! `story:metering-on-llm-model`, `ess/domains/metering.yaml`).
//!
//! Every model here is a scripted llm `Model` that counts its own `turn` calls; nothing calls a
//! live model or the network. The executor is Loom's chooser path: the model selector and argument
//! generator of `b10x_loom_executor::model_selector` over one metered model, so each executor call
//! asks the model to select and then for the selected action's arguments. The case is open at
//! revision 1 and lists one admissible action, `repository.edit`, which the effect port performs;
//! the step budget ends the run.
//!
//! An executor that is not a model loop reports what each call spent
//! (`AgentExecutor::run_with_usage`, `story:executor-reports-usage`): the meter shows it as that
//! step's usage, the steps still sum to the run, and an executor metered at its model reports
//! none, so nothing is counted twice.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

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
use b10x_loom_executor::metering::RateCard;
use b10x_loom_executor::model::metering::RunUsageReport;
use b10x_loom_executor::model_selector::{Briefing, ModelArguments, ModelSelector};
use b10x_loom_executor::{Loom, MeteredExecutor, MeteredModelPort, RunMeter};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, ErrorCode, Id, Item, Model, Protocol,
    Provenance, StopReason, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest, Usage,
};
use serde_json::{Value, json};

const CASE: &str = "CASE-RUN-METERING";
const EDIT: &str = "repository.edit";
/// The model every scripted turn is bound to and reports it served.
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

/// What every answered turn reports it spent.
fn usage() -> Usage {
    Usage {
        input_tokens: Some(INPUT),
        output_tokens: Some(OUTPUT),
        cached_input_tokens: Some(CACHED),
        ..Usage::default()
    }
}

/// What a scripted model holds: the turns it fails, by number from 1, and the turns it saw.
struct Script {
    fails: BTreeSet<u64>,
    seen: u64,
}

/// A scripted llm `Model` bound to [`MODEL`] that counts its own `turn` calls, before anything
/// else. A turn held to a named tool calls it: `select_action` with the edit, any other with no
/// arguments; any other turn answers in prose. Every answer reports [`usage`] as final; a turn the
/// script fails answers with a transport error.
struct Scripted {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<Script>,
}

impl Scripted {
    /// A model that fails its turns numbered `fails`, counted from 1, and answers every other.
    fn new(fails: impl IntoIterator<Item = u64>) -> Self {
        let id = |value: &str| Id::new(value).expect("a fixture id");
        Self {
            provenance: Provenance {
                protocol: Protocol::Responses,
                provider: id("scripted"),
                account: id("fixture"),
                endpoint: id("in-process"),
                model: id(MODEL),
                binding_revision: id("scripted-1"),
            },
            capabilities: Capabilities {
                tools: true,
                tool_choice: true,
                ..Capabilities::text(128_000, 8_192)
            },
            script: Mutex::new(Script {
                fails: fails.into_iter().collect(),
                seen: 0,
            }),
        }
    }

    /// The `turn` calls the model saw, failed or answered.
    fn seen(&self) -> u64 {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .seen
    }

    /// The answer to `request`.
    fn answer(&self, request: &TurnRequest) -> TurnOutcome {
        let observation = TurnObservation {
            usage: Some(usage()),
            final_usage: true,
            ..TurnObservation::new(self.provenance.clone())
        };
        match &request.tool_choice {
            ToolChoice::Named(tool) => TurnOutcome {
                stop_reason: StopReason::ToolCalls,
                items: vec![Item::ToolCall(ToolCall {
                    call_id: CallId::new("call-1").expect("a call id"),
                    name: tool.clone(),
                    arguments: if tool.as_str() == "select_action" {
                        json!({ "action": EDIT })
                    } else {
                        json!({})
                    },
                })],
                observation,
            },
            _ => TurnOutcome {
                stop_reason: StopReason::EndTurn,
                items: vec![Item::assistant("{}")],
                observation,
            },
        }
    }
}

impl Model for Scripted {
    fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        _sink: &'a mut dyn StreamSink,
        _cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        let failed = {
            let mut script = self.script.lock().unwrap_or_else(PoisonError::into_inner);
            script.seen += 1;
            script.fails.contains(&script.seen)
        };
        let answered = if failed {
            Err(Error::new(ErrorCode::Transport, "the connection dropped"))
        } else {
            Ok(self.answer(request))
        };
        Box::pin(async move { answered })
    }
}

/// What the chooser tells the model: the run's instruction, and an empty object schema for any
/// action's arguments. A request that fails is sent once more, as a host that retries does: the
/// failed attempt is a call too.
struct Brief;

impl Briefing for Brief {
    fn text(&self) -> String {
        "Make the change.\n".to_owned()
    }

    fn selection_instructions(&self) -> String {
        "Choose the next action.".to_owned()
    }

    fn argument_instructions(&self) -> String {
        "Write the arguments.".to_owned()
    }

    fn arguments_schema(&self, _action: &str) -> Value {
        json!({"type": "object"})
    }

    fn send<'a>(
        &'a self,
        model: &'a dyn Model,
        request: &'a TurnRequest,
        sink: &'a mut dyn StreamSink,
        cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        Box::pin(async move {
            match model.turn(request, &mut *sink, cancel).await {
                Err(_) => model.turn(request, sink, cancel).await,
                answered => answered,
            }
        })
    }
}

/// One turn of `model` with no tools, driven to completion here.
fn turn(model: &dyn Model) -> Result<TurnOutcome, Error> {
    let request = TurnRequest::new(MODEL, vec![Item::user("go")]);
    let mut sink = llm_core::VecSink::new(64, 64 * 1024);
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime")
        .block_on(model.turn(&request, &mut sink, &Cancel::new()))
}

/// Wraps an executor and keeps, for each `run` call, the `turn` calls `model` saw during it.
struct Observed<'m, E> {
    inner: E,
    model: &'m Scripted,
    spans: RefCell<Vec<u64>>,
}

impl<'m, E> Observed<'m, E> {
    fn new(inner: E, model: &'m Scripted) -> Self {
        Self {
            inner,
            model,
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
        let before = self.model.seen();
        let outcome = self.inner.run(commission, frontier);
        self.spans.borrow_mut().push(self.model.seen() - before);
        outcome
    }
}

/// The chooser Loom whose model selector and argument generator both ask `model`, revalidating
/// against [`Case`].
fn chooser(model: &dyn Model) -> Loom<ModelSelector<'_, Brief>, ModelArguments<'_, Brief>, &Case> {
    Loom::new(
        ModelSelector::new(model, Brief),
        ModelArguments::new(model, Brief),
        "Make the change.",
    )
    .with_governor(&Case)
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

/// The chooser of [`chooser`] over a metered model, metered under `prices`, run for two steps.
fn chooser_run(prices: Option<RateCard>) -> (RunUsageReport, Vec<i64>, u64) {
    let model = Scripted::new([]);
    let meter = RunMeter::new(prices);
    let metered = MeteredModelPort::new(&model, &meter);
    let executor = Observed::new(MeteredExecutor::new(chooser(&metered), &meter), &model);
    let end = run(&executor, 2);
    (meter.report(&end.run_id), executor.spans(), model.seen())
}

#[test]
fn the_reported_calls_equal_the_turns_the_model_port_saw() {
    // The third turn, the second step's selection, fails and is sent again: a failed turn is a
    // call too.
    let model = Scripted::new([3]);
    let meter = RunMeter::new(None);
    let metered = MeteredModelPort::new(&model, &meter);
    let executor = Observed::new(MeteredExecutor::new(chooser(&metered), &meter), &model);

    let end = run(&executor, 3);
    let report = meter.report(&end.run_id);

    assert!(
        model.seen() >= 3,
        "the run made the failed call and went on: {} after {:?}",
        model.seen(),
        end.outcome
    );
    assert_eq!(
        report.model_calls,
        i64::try_from(model.seen()).expect("a small count"),
        "the report counts every turn the model saw: {report:?}"
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
    let (report, spans, seen) = chooser_run(None);

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
    let (report, spans, _) = chooser_run(Some(card(MODEL)));

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
        let (report, _, _) = chooser_run(prices);
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

/// An executor that makes one turn through its metered model per `run` call, and suspends.
struct OneTurn<'m> {
    model: &'m dyn Model,
}

impl AgentExecutor for OneTurn<'_> {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        assert!(turn(self.model).is_err(), "the scripted turn fails");
        ExecutorOutcome::NoUsefulAction(Unit(true))
    }
}

#[test]
fn a_failed_turn_is_a_call_without_usage() {
    let model = Scripted::new([1]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let metered = MeteredModelPort::new(&model, &meter);
    let executor = MeteredExecutor::new(OneTurn { model: &metered }, &meter);
    let frontier = Frontier::new(frontier_data(&CaseId(CASE.to_owned())));
    let _ = executor.run(&commission(), &frontier);

    let report = meter.report(&RunId(uuid(0x501)));
    assert_eq!(model.seen(), 1);
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
    assert_eq!(report.cost_micro_usd, None, "nor a cost: {report:?}");
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

/// An executor over an agent session, not a model loop: it calls no model of the run, proposes
/// the edit, and reports [`session_usage`] as what each call spent. With `model`, each call also
/// makes one turn of it, which the meter counts as a model call.
struct Session<'m> {
    model: Option<&'m dyn Model>,
}

impl AgentExecutor for Session<'_> {
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
        if let Some(model) = self.model {
            assert!(turn(model).is_ok(), "the scripted turn answers");
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
    let executor = MeteredExecutor::new(Session { model: None }, &meter);

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
    let model = Scripted::new([]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let metered = MeteredModelPort::new(&model, &meter);
    let executor = MeteredExecutor::new(
        Session {
            model: Some(&metered),
        },
        &meter,
    );

    let end = run(&executor, 1);
    let report = meter.report(&end.run_id);

    assert_eq!(model.seen(), 1);
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
    let model = Scripted::new([]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let metered = MeteredModelPort::new(&model, &meter);
    let executor = MeteredExecutor::new(chooser(&metered), &meter);

    let end = run(&executor, 2);
    meter.charge_effects(&end.charges);
    let report = meter.report(&end.run_id);

    let seen = i64::try_from(model.seen()).expect("a small count");
    assert_eq!(report.model_calls, seen, "{report:?}");
    assert_eq!(
        report.input_tokens,
        seen * i64::try_from(INPUT).expect("small"),
        "each turn's tokens are counted once, at the model: {report:?}"
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
