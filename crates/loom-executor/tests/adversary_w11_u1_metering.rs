// SPDX-License-Identifier: Apache-2.0

//! Adversarial cases against the run meter (`story:dw-run-model-meter`,
//! `story:metering-on-llm-model`, `crates/loom-executor/src/metering.rs`,
//! `ess/domains/metering.yaml`).
//!
//! Every model here is a scripted llm `Model` that counts its own `turn` calls, except the last
//! section's, a scripted harness `ModelPort` metered by the ported loop's adapter until
//! `story:delete-harness-port` removes both; nothing calls a live model or the network. The
//! executors are driven directly, one `AgentExecutor::run` call at a time, so each case states
//! exactly which calls happen inside which step.

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId,
    FrontierObligation, PrincipalId, RunId, Unit, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_executor::harness::wire as harness;
use b10x_loom_executor::metering::RateCard;
use b10x_loom_executor::model::metering::RunUsageReport;
use b10x_loom_executor::{MeteredExecutor, MeteredModelPort, RunMeter};
use llm_core::{
    BoxFuture, Cancel, Capabilities, Error, ErrorCode, Id, Item, Model, Protocol, Provenance,
    StopReason, StreamSink, TurnObservation, TurnOutcome, TurnRequest, Usage, VecSink,
};

const MODEL: &str = "m";

fn uuid(n: u64) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: CaseId("CASE-ADVERSARY-METER".to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

fn frontier() -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf1)),
        case_id: CaseId("CASE-ADVERSARY-METER".to_owned()),
        case_revision: 1,
        claims: Vec::new(),
        obligations: vec![FrontierObligation {
            obligation: "tests-pass".to_owned(),
            open: true,
        }],
        actions: vec![FrontierAction {
            action: "repository.edit".to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    })
}

fn run_id() -> RunId {
    RunId(uuid(0x501))
}

fn provenance() -> Provenance {
    let id = |value: &str| Id::new(value).expect("a fixture id");
    Provenance {
        protocol: Protocol::Responses,
        provider: id("scripted"),
        account: id("fixture"),
        endpoint: id("in-process"),
        model: id(MODEL),
        binding_revision: id("scripted-1"),
    }
}

fn usage(input: u64, cached: u64, output: u64) -> Usage {
    Usage {
        input_tokens: Some(input),
        output_tokens: Some(output),
        cached_input_tokens: Some(cached),
        ..Usage::default()
    }
}

/// An answer in prose, served by `served` (`None`: the provider named no model, so the bound one
/// is priced), reporting `usage` as final.
fn answer(served: Option<&str>, usage: Option<Usage>) -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::EndTurn,
        items: vec![Item::assistant("{}")],
        observation: TurnObservation {
            upstream_model: served.map(|model| Id::new(model).expect("a model id")),
            usage,
            final_usage: true,
            ..TurnObservation::new(provenance())
        },
    }
}

fn request() -> TurnRequest {
    TurnRequest::new(MODEL, vec![Item::user("go")])
}

/// $1.25/Mtok input, $0.125/Mtok cached input, $10/Mtok output, for `model`.
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

/// What a scripted model does on one turn.
enum Step {
    Answer(Box<TurnOutcome>),
    Fail,
    /// Panics while its turn's future is driven.
    Panic,
}

impl Step {
    /// Answers with `outcome`.
    fn answer(outcome: TurnOutcome) -> Self {
        Self::Answer(Box::new(outcome))
    }
}

struct Script {
    steps: VecDeque<Step>,
    seen: u64,
}

/// A scripted llm `Model` that counts its own `turn` calls, before doing anything else. Its clones
/// share the script and the count. Once the script is spent it answers with a usage of 100 input
/// tokens, 40 of them cached, and 10 output tokens, served by [`MODEL`].
#[derive(Clone)]
struct Scripted {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Arc<Mutex<Script>>,
}

impl Scripted {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            provenance: provenance(),
            capabilities: Capabilities::text(128_000, 8_192),
            script: Arc::new(Mutex::new(Script {
                steps: steps.into_iter().collect(),
                seen: 0,
            })),
        }
    }

    fn seen(&self) -> i64 {
        i64::try_from(
            self.script
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .seen,
        )
        .expect("a small count")
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
        _request: &'a TurnRequest,
        _sink: &'a mut dyn StreamSink,
        _cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        let next = {
            let mut script = self.script.lock().unwrap_or_else(PoisonError::into_inner);
            script.seen += 1;
            script.steps.pop_front()
        };
        Box::pin(async move {
            match next {
                Some(Step::Answer(outcome)) => Ok(*outcome),
                // The error carries what the provider reported before the connection dropped;
                // the turn failed all the same.
                Some(Step::Fail) => Err(Error::new(ErrorCode::Transport, "the connection dropped")
                    .with_observation(answer(None, Some(usage(100, 40, 10))).observation)),
                Some(Step::Panic) => panic!("the model panicked inside its turn"),
                None => Ok(answer(None, Some(usage(100, 40, 10)))),
            }
        })
    }
}

/// A metered model holds its model as a host does, behind a pointer.
type Metered = MeteredModelPort<Arc<Scripted>>;

/// `model`, sharing its script and count, metered into `meter`.
fn metered(model: &Scripted, meter: &RunMeter) -> Metered {
    MeteredModelPort::new(Arc::new(model.clone()), meter)
}

/// `future` driven to completion on a runtime made for it.
fn drive<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime")
        .block_on(future)
}

/// One turn of `model`, driven to completion on this thread.
fn turn(model: &dyn Model) -> Result<TurnOutcome, Error> {
    let request = request();
    let mut sink = VecSink::new(64, 64 * 1024);
    let cancel = Cancel::new();
    drive(model.turn(&request, &mut sink, &cancel))
}

/// An executor that makes `turns` turns through its metered model per `run` call.
struct Turns {
    model: Metered,
    turns: usize,
}

impl Turns {
    fn new(model: &Scripted, meter: &RunMeter, turns: usize) -> Self {
        Self {
            model: metered(model, meter),
            turns,
        }
    }
}

impl AgentExecutor for Turns {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        for _ in 0..self.turns {
            let _ = turn(&self.model);
        }
        ExecutorOutcome::NoUsefulAction(Unit(true))
    }
}

fn steps(report: &RunUsageReport) -> Vec<(i64, i64)> {
    report
        .steps
        .iter()
        .map(|step| (step.step, step.model_calls))
        .collect()
}

/// A turn that panics inside the model was still a `turn` call the model saw.
#[test]
fn a_turn_that_panics_inside_the_port_is_still_a_call() {
    let model = Scripted::new([Step::Panic]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let metered = metered(&model, &meter);

    let contained = catch_unwind(AssertUnwindSafe(|| turn(&metered)));
    assert!(contained.is_err(), "the scripted turn panics");

    let report = meter.report(&run_id());
    assert_eq!(model.seen(), 1, "the model saw one turn");
    assert_eq!(
        report.model_calls,
        model.seen(),
        "the reported calls equal the turns the model saw: {report:?}"
    );
    assert_eq!(
        report.cost_micro_usd, None,
        "an unwound turn's cost is unknown: {report:?}"
    );
}

/// A turn whose future is dropped before it finished was still a call, and its cost is unknown.
#[test]
fn a_turn_dropped_unfinished_is_a_call_whose_cost_is_absent() {
    let model = Scripted::new([]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let metered = metered(&model, &meter);
    let request = request();
    let mut sink = VecSink::new(64, 64 * 1024);
    let cancel = Cancel::new();
    drop(metered.turn(&request, &mut sink, &cancel));

    let report = meter.report(&run_id());
    assert_eq!(steps(&report), vec![(0, 1)], "{report:?}");
    assert_eq!(report.input_tokens, 0, "{report:?}");
    assert_eq!(report.cost_micro_usd, None, "{report:?}");
}

/// The gate of one side of [`two_executors_sharing_a_meter_each_count_their_own_calls`]: it
/// reports that its step is open, waits to be told to turn, turns `turns` times and returns.
struct Gated {
    model: Metered,
    turns: usize,
    opened: Sender<()>,
    go: Mutex<Receiver<()>>,
}

impl AgentExecutor for Gated {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.opened.send(()).expect("the test is listening");
        self.go
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .recv()
            .expect("the test says go");
        for _ in 0..self.turns {
            let _ = turn(&self.model);
        }
        ExecutorOutcome::NoUsefulAction(Unit(true))
    }
}

fn gated(
    model: &Scripted,
    meter: &RunMeter,
    turns: usize,
) -> (MeteredExecutor<Gated>, Receiver<()>, Sender<()>) {
    let (opened, opened_rx) = channel();
    let (go_tx, go) = channel();
    (
        MeteredExecutor::new(
            Gated {
                model: metered(model, meter),
                turns,
                opened,
                go: Mutex::new(go),
            },
            meter,
        ),
        opened_rx,
        go_tx,
    )
}

/// Two executor calls on one meter that overlap in time: A opens step 1, B opens step 2, A turns
/// twice and returns, B turns once and returns, then the host turns three times outside any
/// executor call. Each executor call's turns belong to its own step, and the host's to step 0.
#[test]
fn two_executors_sharing_a_meter_each_count_their_own_calls() {
    let model = Scripted::new([]);
    let meter = RunMeter::new(None);
    let (a, a_opened, a_go) = gated(&model, &meter, 2);
    let (b, b_opened, b_go) = gated(&model, &meter, 1);

    std::thread::scope(|scope| {
        let a_done = scope.spawn(|| a.run(&commission(), &frontier()));
        a_opened.recv().expect("A opened its step");
        let b_done = scope.spawn(|| b.run(&commission(), &frontier()));
        b_opened.recv().expect("B opened its step");
        a_go.send(()).expect("A is waiting");
        a_done.join().expect("A returned");
        b_go.send(()).expect("B is waiting");
        b_done.join().expect("B returned");
    });
    let host = metered(&model, &meter);
    for _ in 0..3 {
        let _ = turn(&host);
    }

    let report = meter.report(&run_id());
    assert_eq!(report.model_calls, 6, "{report:?}");
    assert_eq!(
        steps(&report),
        vec![(0, 3), (1, 2), (2, 1)],
        "A's two turns in step 1, B's one in step 2, the host's three in step 0: {report:?}"
    );
}

/// One executor call is one step, even when the executor the run holds is a metered executor
/// over another metered executor of the same meter.
#[test]
fn nested_metered_executors_on_one_meter_open_one_step_per_executor_call() {
    let model = Scripted::new([]);
    let meter = RunMeter::new(None);
    let executor = MeteredExecutor::new(
        MeteredExecutor::new(Turns::new(&model, &meter, 2), &meter),
        &meter,
    );
    let _ = executor.run(&commission(), &frontier());

    let report = meter.report(&run_id());
    assert_eq!(
        steps(&report),
        vec![(1, 2)],
        "one executor call of the run, one step: {report:?}"
    );
}

/// A failed turn's tokens are unknown, as are an answer's that reported no usage, so no price can
/// be stated for it either: in a priced run whose only call failed the cost is absent, not zero.
/// An error that carries an observation with a usage is still a failed turn.
#[test]
fn a_failed_turn_states_no_cost_in_a_priced_run() {
    let model = Scripted::new([Step::Fail]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let _ =
        MeteredExecutor::new(Turns::new(&model, &meter, 1), &meter).run(&commission(), &frontier());

    let report = meter.report(&run_id());
    assert_eq!(report.model_calls, 1, "{report:?}");
    assert_eq!(
        report.input_tokens, 0,
        "the usage a failed turn's error carries is not taken: {report:?}"
    );
    assert_eq!(
        report.cost_micro_usd, None,
        "a call whose cost nobody reported is not a free call: {report:?}"
    );
}

/// A turn of a metered model is a call of the executor call that called `turn`, on whichever
/// thread its future is then driven: the future made in the step and driven on another thread
/// counts in that step, not in step 0.
#[test]
fn a_turn_driven_on_another_thread_counts_in_the_step_that_made_it() {
    struct Hands {
        model: Metered,
    }

    impl AgentExecutor for Hands {
        fn run(
            &self,
            _commission: &Commission<commission_state::Assigned>,
            _frontier: &Frontier<frontier_state::Issued>,
        ) -> ExecutorOutcome {
            let request = request();
            let cancel = Cancel::new();
            let (mut first, mut second) =
                (VecSink::new(64, 64 * 1024), VecSink::new(64, 64 * 1024));
            let turns = [
                self.model.turn(&request, &mut first, &cancel),
                self.model.turn(&request, &mut second, &cancel),
            ];
            std::thread::scope(|scope| {
                for future in turns {
                    let driven = scope.spawn(move || drive(future));
                    assert!(driven.join().expect("the thread returned").is_ok());
                }
            });
            ExecutorOutcome::NoUsefulAction(Unit(true))
        }
    }

    let model = Scripted::new([]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let executor = MeteredExecutor::new(
        Hands {
            model: metered(&model, &meter),
        },
        &meter,
    );
    let _ = executor.run(&commission(), &frontier());

    let report = meter.report(&run_id());
    assert_eq!(model.seen(), 2);
    assert_eq!(steps(&report), vec![(1, 2)], "{report:?}");
    assert_eq!(report.input_tokens, 200, "{report:?}");
    assert_eq!(report.cost_micro_usd, Some(2 * 180), "{report:?}");
}

/// An answer that reported no usage is a call, adds no tokens, and leaves its step's cost and the
/// run's absent, while a fully priced step keeps its own cost.
#[test]
fn an_answer_without_usage_is_a_call_whose_cost_is_absent() {
    let model = Scripted::new([
        // Step 1: priced, then an answer without usage.
        Step::answer(answer(None, Some(usage(100, 40, 10)))),
        Step::answer(answer(None, None)),
        // Step 2: priced.
        Step::answer(answer(None, Some(usage(100, 40, 10)))),
    ]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let executor = MeteredExecutor::new(Turns::new(&model, &meter, 2), &meter);
    let _ = executor.run(&commission(), &frontier());
    let single = MeteredExecutor::new(Turns::new(&model, &meter, 1), &meter);
    let _ = single.run(&commission(), &frontier());

    let report = meter.report(&run_id());
    assert_eq!(steps(&report), vec![(1, 2), (2, 1)], "{report:?}");
    assert_eq!(report.steps[0].input_tokens, 100, "{report:?}");
    assert_eq!(report.steps[0].cost_micro_usd, None, "{report:?}");
    assert_eq!(report.steps[1].cost_micro_usd, Some(180), "{report:?}");
    assert_eq!(report.cost_micro_usd, None, "{report:?}");
    assert_eq!(report.input_tokens, 200, "{report:?}");
}

/// A usage llm marks not final is a partial snapshot, never a final cost, and one that does not
/// name its input or its output tokens cannot be priced: each answer is a call whose tokens are
/// unknown, so it adds none and leaves the cost absent.
#[test]
fn an_answer_whose_usage_is_partial_is_a_call_whose_cost_is_absent() {
    let not_final = {
        let mut outcome = answer(None, Some(usage(100, 40, 10)));
        outcome.observation.final_usage = false;
        outcome
    };
    let no_output = answer(
        None,
        Some(Usage {
            output_tokens: None,
            ..usage(100, 40, 10)
        }),
    );
    for (why, outcome) in [("not final", not_final), ("no output tokens", no_output)] {
        let model = Scripted::new([Step::answer(outcome)]);
        let meter = RunMeter::new(Some(card(MODEL)));
        let _ = MeteredExecutor::new(Turns::new(&model, &meter, 1), &meter)
            .run(&commission(), &frontier());

        let report = meter.report(&run_id());
        assert_eq!(steps(&report), vec![(1, 1)], "{why}: {report:?}");
        assert_eq!(report.input_tokens, 0, "{why}: {report:?}");
        assert_eq!(report.cost_micro_usd, None, "{why}: {report:?}");
    }
}

/// A step in which one turn's model is priced and another's is not has no cost: the turn is
/// priced against the model the provider reported serving it, not the one it is bound to.
#[test]
fn a_step_mixing_a_priced_and_an_unlisted_model_has_no_cost() {
    let model = Scripted::new([
        Step::answer(answer(Some(MODEL), Some(usage(100, 40, 10)))),
        Step::answer(answer(Some("unlisted"), Some(usage(100, 40, 10)))),
    ]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let _ =
        MeteredExecutor::new(Turns::new(&model, &meter, 2), &meter).run(&commission(), &frontier());

    let report = meter.report(&run_id());
    assert_eq!(steps(&report), vec![(1, 2)], "{report:?}");
    assert_eq!(report.steps[0].cost_micro_usd, None, "{report:?}");
    assert_eq!(report.cost_micro_usd, None, "{report:?}");
}

/// A turn made before the first executor call and one made after it returned are counted in step
/// 0, not in the step that closed, so the steps sum to the totals and the step keeps only its own.
#[test]
fn calls_outside_an_executor_call_are_counted_in_step_0() {
    let model = Scripted::new([]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let host = metered(&model, &meter);
    let _ = turn(&host);
    let _ =
        MeteredExecutor::new(Turns::new(&model, &meter, 3), &meter).run(&commission(), &frontier());
    let _ = turn(&host);

    let report = meter.report(&run_id());
    assert_eq!(model.seen(), 5);
    assert_eq!(steps(&report), vec![(0, 2), (1, 3)], "{report:?}");
    assert_eq!(report.model_calls, 5, "{report:?}");
    assert_eq!(report.steps[0].cost_micro_usd, Some(2 * 180), "{report:?}");
    assert_eq!(report.cost_micro_usd, Some(5 * 180), "{report:?}");
}

/// A step closes when its executor call unwinds, so a later call is not counted in it.
#[test]
fn a_step_closes_when_its_executor_call_panics() {
    struct Panics {
        model: Metered,
    }

    impl AgentExecutor for Panics {
        fn run(
            &self,
            _commission: &Commission<commission_state::Assigned>,
            _frontier: &Frontier<frontier_state::Issued>,
        ) -> ExecutorOutcome {
            let _ = turn(&self.model);
            panic!("the executor panicked");
        }
    }

    let model = Scripted::new([]);
    let meter = RunMeter::new(None);
    let executor = MeteredExecutor::new(
        Panics {
            model: metered(&model, &meter),
        },
        &meter,
    );
    let unwound = catch_unwind(AssertUnwindSafe(|| {
        executor.run(&commission(), &frontier())
    }));
    assert!(unwound.is_err());
    let _ = turn(&metered(&model, &meter));

    let report = meter.report(&run_id());
    assert_eq!(steps(&report), vec![(0, 1), (1, 1)], "{report:?}");
}

/// A usage at the edge of `u64` saturates to the largest figure rather than wrapping negative,
/// and the cost of an unpriceable amount is still stated as the saturated figure, not zero.
#[test]
fn a_usage_at_u64_max_saturates_rather_than_wrapping() {
    let model = Scripted::new([
        Step::answer(answer(None, Some(usage(u64::MAX, 0, u64::MAX)))),
        Step::answer(answer(None, Some(usage(u64::MAX, 0, u64::MAX)))),
    ]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let _ =
        MeteredExecutor::new(Turns::new(&model, &meter, 2), &meter).run(&commission(), &frontier());

    let report = meter.report(&run_id());
    assert_eq!(report.input_tokens, i64::MAX, "{report:?}");
    assert_eq!(report.output_tokens, i64::MAX, "{report:?}");
    assert!(report.cost_micro_usd >= Some(0), "{report:?}");
    assert_eq!(report.steps[0].input_tokens, i64::MAX, "{report:?}");
}

/// A host's model is held as llm hands it out, an `Arc<dyn Model>`, and metered as it is.
#[test]
fn a_shared_model_a_host_built_is_metered_as_it_is() {
    let model = Scripted::new([]);
    let shared: Arc<dyn Model> = Arc::new(model.clone());
    let meter = RunMeter::new(Some(card(MODEL)));
    let metered = MeteredModelPort::new(shared, &meter);
    assert_eq!(
        metered.provenance(),
        &provenance(),
        "the inner model's binding"
    );
    let _ = turn(&metered);

    let report = meter.report(&run_id());
    assert_eq!(model.seen(), 1);
    assert_eq!(steps(&report), vec![(0, 1)], "{report:?}");
    assert_eq!(report.cost_micro_usd, Some(180), "{report:?}");
}

/// A host's own model over a metered one that reaches it only while its turn's future runs, as a
/// retrying wrapper does.
struct Lazy<M> {
    inner: M,
}

impl<M: Model> Model for Lazy<M> {
    fn provenance(&self) -> &Provenance {
        self.inner.provenance()
    }

    fn capabilities(&self) -> &Capabilities {
        self.inner.capabilities()
    }

    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        sink: &'a mut dyn StreamSink,
        cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        Box::pin(async move { self.inner.turn(request, sink, cancel).await })
    }
}

/// A metered model wrapped again on its meter counts each turn once, whether the outer one reaches
/// it when `turn` is called or only while the turn runs, here through a host's wrapper between
/// them.
#[test]
fn a_model_metered_again_on_one_meter_counts_each_turn_once_however_it_is_reached() {
    let model = Scripted::new([]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let at_call = MeteredModelPort::new(Arc::new(metered(&model, &meter)), &meter);
    let while_running = MeteredModelPort::new(
        Arc::new(Lazy {
            inner: metered(&model, &meter),
        }),
        &meter,
    );
    assert!(turn(&at_call).is_ok());
    assert!(turn(&while_running).is_ok());

    let report = meter.report(&run_id());
    assert_eq!(model.seen(), 2);
    assert_eq!(steps(&report), vec![(0, 2)], "{report:?}");
    assert_eq!(report.input_tokens, 200, "{report:?}");
    assert_eq!(report.cost_micro_usd, Some(2 * 180), "{report:?}");
}

/// A model metered on one meter and wrapped by a metered model of another counts its turn in
/// each: the guard is per meter, so two runs that share a model each see their call.
#[test]
fn a_model_metered_on_two_meters_counts_its_turn_in_each() {
    let model = Scripted::new([]);
    let (first, second) = (RunMeter::new(None), RunMeter::new(None));
    let outer = MeteredModelPort::new(Arc::new(metered(&model, &first)), &second);
    assert!(turn(&outer).is_ok());

    for meter in [&first, &second] {
        let report = meter.report(&run_id());
        assert_eq!(
            (report.model_calls, report.input_tokens),
            (1, 100),
            "{report:?}"
        );
    }
}

/// While executor calls on one meter overlap, a turn made on a thread that runs none of them (the
/// host's) is counted in the one that opened last: it was made while each ran, and which one it
/// serves is not known. Once both returned, the host's turn is step 0's.
#[test]
fn a_turn_on_another_thread_while_executor_calls_overlap_counts_in_the_last_opened() {
    let model = Scripted::new([]);
    let meter = RunMeter::new(None);
    let (a, a_opened, a_go) = gated(&model, &meter, 0);
    let (b, b_opened, b_go) = gated(&model, &meter, 0);
    let host = metered(&model, &meter);

    std::thread::scope(|scope| {
        let a_done = scope.spawn(|| a.run(&commission(), &frontier()));
        a_opened.recv().expect("A opened its step");
        let b_done = scope.spawn(|| b.run(&commission(), &frontier()));
        b_opened.recv().expect("B opened its step");
        let _ = turn(&host);
        a_go.send(()).expect("A is waiting");
        b_go.send(()).expect("B is waiting");
        a_done.join().expect("A returned");
        b_done.join().expect("B returned");
    });
    let _ = turn(&host);

    let report = meter.report(&run_id());
    assert_eq!(
        steps(&report),
        vec![(0, 1), (1, 0), (2, 1)],
        "the host's turn while both ran is B's, the one after both returned is step 0's: \
         {report:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// The ported loop's own model port, which a `MeteredModelPort` meters until
// `story:delete-harness-port` removes it (`crates/loom-executor/src/harness/governed.rs`).

/// A harness `ModelPort` that counts its own `turn` calls, before anything else, and answers each
/// with 100 input tokens, 40 of them cached, and 10 output tokens of [`MODEL`]. Its clones and its
/// forks share the count.
#[derive(Clone)]
struct HarnessPort {
    wire: harness::WireId,
    seen: Arc<Mutex<i64>>,
}

impl HarnessPort {
    fn new() -> Self {
        Self {
            wire: harness::WireId::new("scripted").expect("a valid wire id"),
            seen: Arc::new(Mutex::new(0)),
        }
    }

    fn seen(&self) -> i64 {
        *self.seen.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl harness::ModelPort for HarnessPort {
    fn wire(&self) -> &harness::WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        _request: &harness::TurnRequest,
        _sink: &mut dyn harness::StreamSink,
    ) -> Result<harness::TurnOutcome, harness::WireError> {
        *self.seen.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        Ok(harness::TurnOutcome {
            stop_reason: harness::StopReason::EndTurn,
            items: vec![harness::Item::assistant("{}")],
            usage: Some(harness::Usage {
                model: MODEL.to_owned(),
                input_tokens: 100,
                output_tokens: 10,
                cached_input_tokens: 40,
                cache_creation_input_tokens: None,
            }),
        })
    }

    fn fork(&self) -> Option<Box<dyn harness::ModelPort + Send + '_>> {
        Some(Box::new(self.clone()))
    }
}

/// One turn of a harness port.
fn harness_turn(
    port: &mut (impl harness::ModelPort + ?Sized),
) -> Result<harness::TurnOutcome, harness::WireError> {
    let request = harness::TurnRequest {
        model: MODEL.to_owned(),
        instructions: "Answer.".to_owned(),
        items: vec![harness::Item::user("go")],
        tools: Vec::new(),
        max_output_tokens: None,
        sampling: harness::Sampling::default(),
        tool_choice: harness::ToolChoice::Auto,
    };
    port.turn(&request, &mut harness::VecSink::new())
}

/// A turn through a fork of a metered harness port, and through a fork of that fork, is a call of
/// the same run, counted in the step that is open.
#[test]
fn a_turn_through_a_fork_of_a_metered_port_is_counted() {
    struct Forking {
        port: MeteredModelPort<HarnessPort>,
    }

    impl AgentExecutor for Forking {
        fn run(
            &self,
            _commission: &Commission<commission_state::Assigned>,
            _frontier: &Frontier<frontier_state::Issued>,
        ) -> ExecutorOutcome {
            let mut fork = harness::ModelPort::fork(&self.port).expect("the port forks");
            let _ = harness_turn(&mut *fork);
            let mut grandchild = fork.fork().expect("the fork forks");
            let _ = harness_turn(&mut *grandchild);
            ExecutorOutcome::NoUsefulAction(Unit(true))
        }
    }

    let port = HarnessPort::new();
    let meter = RunMeter::new(Some(card(MODEL)));
    let executor = MeteredExecutor::new(
        Forking {
            port: MeteredModelPort::new(port.clone(), &meter),
        },
        &meter,
    );
    let _ = executor.run(&commission(), &frontier());

    let report = meter.report(&run_id());
    assert_eq!(port.seen(), 2);
    assert_eq!(steps(&report), vec![(1, 2)], "{report:?}");
    assert_eq!(report.input_tokens, 200, "{report:?}");
    assert_eq!(report.cost_micro_usd, Some(2 * 180), "{report:?}");
}

/// A fork made in a step and turned on another thread after its executor call returned is still a
/// call of that step, not of step 0.
#[test]
fn a_fork_turned_on_another_thread_keeps_the_step_it_was_made_in() {
    let port = HarnessPort::new();
    let meter = RunMeter::new(None);
    let metered = MeteredModelPort::new(port.clone(), &meter);
    let fork = Mutex::new(None);

    struct Makes<'p, 'f> {
        port: &'p MeteredModelPort<HarnessPort>,
        fork: &'f Mutex<Option<Box<dyn harness::ModelPort + Send + 'p>>>,
    }

    impl AgentExecutor for Makes<'_, '_> {
        fn run(
            &self,
            _commission: &Commission<commission_state::Assigned>,
            _frontier: &Frontier<frontier_state::Issued>,
        ) -> ExecutorOutcome {
            *self.fork.lock().unwrap_or_else(PoisonError::into_inner) =
                harness::ModelPort::fork(self.port);
            ExecutorOutcome::NoUsefulAction(Unit(true))
        }
    }

    let _ = MeteredExecutor::new(
        Makes {
            port: &metered,
            fork: &fork,
        },
        &meter,
    )
    .run(&commission(), &frontier());
    let mut made = fork
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
        .expect("the port forked");
    std::thread::scope(|scope| {
        scope
            .spawn(|| assert!(harness_turn(&mut *made).is_ok()))
            .join()
            .expect("the thread returned");
    });

    let report = meter.report(&run_id());
    assert_eq!(port.seen(), 1);
    assert_eq!(steps(&report), vec![(1, 1)], "{report:?}");
}

/// A metered harness port wrapped again on its meter counts each turn once.
#[test]
fn a_harness_port_metered_again_on_one_meter_counts_each_turn_once() {
    let port = HarnessPort::new();
    let meter = RunMeter::new(Some(card(MODEL)));
    let mut twice = MeteredModelPort::new(MeteredModelPort::new(port.clone(), &meter), &meter);
    assert!(harness_turn(&mut twice).is_ok());

    let report = meter.report(&run_id());
    assert_eq!(port.seen(), 1);
    assert_eq!(
        (
            report.model_calls,
            report.input_tokens,
            report.cost_micro_usd
        ),
        (1, 100, Some(180)),
        "{report:?}"
    );
}
