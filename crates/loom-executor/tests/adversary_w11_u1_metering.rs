// SPDX-License-Identifier: Apache-2.0

//! Adversarial cases against the run meter (`story:dw-run-model-meter`,
//! `crates/loom-executor/src/metering.rs`, `ess/domains/metering.yaml`).
//!
//! Every model here is a scripted `ModelPort` that counts its own `turn` calls; nothing calls a
//! live model or the network. The executors are driven directly, one `AgentExecutor::run` call at a
//! time, so each case states exactly which calls happen inside which step.

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
use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::turn_loop::RateCard;
use b10x_loom_executor::harness::wire::{
    Item, ModelPort, Sampling, StopReason, StreamSink, ToolChoice, TurnOutcome, TurnRequest, Usage,
    VecSink, WireError, WireId,
};
use b10x_loom_executor::model::metering::RunUsageReport;
use b10x_loom_executor::{MeteredExecutor, MeteredModelPort, RunMeter};

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

fn usage(model: &str, input: u64, cached: u64, output: u64) -> Usage {
    Usage {
        model: model.to_owned(),
        input_tokens: input,
        output_tokens: output,
        cached_input_tokens: cached,
        cache_creation_input_tokens: None,
    }
}

fn answer(usage: Option<Usage>) -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::EndTurn,
        items: vec![Item::assistant("{}")],
        usage,
    }
}

fn request() -> TurnRequest {
    TurnRequest {
        model: MODEL.to_owned(),
        instructions: "Answer.".to_owned(),
        items: vec![Item::user("go")],
        tools: Vec::new(),
        max_output_tokens: None,
        sampling: Sampling::default(),
        tool_choice: ToolChoice::Auto,
    }
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

/// What a scripted port does on one turn.
enum Step {
    Answer(TurnOutcome),
    Fail,
    Panic,
}

struct Script {
    steps: VecDeque<Step>,
    seen: u64,
}

/// A scripted `ModelPort` that counts its own `turn` calls, before doing anything else. Its clones
/// and its forks share the script and the count. Once the script is spent it answers with a usage
/// of 100 input tokens, 40 of them cached, and 10 output tokens for [`MODEL`].
#[derive(Clone)]
struct Port {
    wire: WireId,
    script: Arc<Mutex<Script>>,
    forks: bool,
}

impl Port {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            wire: WireId::new(responses::WIRE).expect("a valid wire id"),
            script: Arc::new(Mutex::new(Script {
                steps: steps.into_iter().collect(),
                seen: 0,
            })),
            forks: false,
        }
    }

    fn forking(mut self) -> Self {
        self.forks = true;
        self
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

impl ModelPort for Port {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        _request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        let next = {
            let mut script = self.script.lock().unwrap_or_else(PoisonError::into_inner);
            script.seen += 1;
            script.steps.pop_front()
        };
        match next {
            Some(Step::Answer(outcome)) => Ok(outcome),
            Some(Step::Fail) => Err(WireError::transport("the connection dropped")),
            Some(Step::Panic) => panic!("the port panicked inside its turn"),
            None => Ok(answer(Some(usage(MODEL, 100, 40, 10)))),
        }
    }

    fn fork(&self) -> Option<Box<dyn ModelPort + Send + '_>> {
        self.forks
            .then(|| Box::new(self.clone()) as Box<dyn ModelPort + Send + '_>)
    }
}

fn turn(port: &mut dyn ModelPort) -> Result<TurnOutcome, WireError> {
    port.turn(&request(), &mut VecSink::new())
}

/// An executor that makes `turns` turns through its metered port per `run` call.
struct Turns {
    port: Mutex<MeteredModelPort<Port>>,
    turns: usize,
}

impl Turns {
    fn new(port: &Port, meter: &RunMeter, turns: usize) -> Self {
        Self {
            port: Mutex::new(MeteredModelPort::new(port.clone(), meter)),
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
        let mut port = self.port.lock().unwrap_or_else(PoisonError::into_inner);
        for _ in 0..self.turns {
            let _ = turn(&mut *port);
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

/// A turn that panics inside the port was still a `turn` call the port saw. The loop contains a
/// delegate's panic and carries on (`delegates_side_by_side`, `catch_unwind` around each child in
/// `crates/loom-executor/src/harness/turn_loop/mod.rs`), so the run continues and reports.
#[test]
fn a_turn_that_panics_inside_the_port_is_still_a_call() {
    let port = Port::new([Step::Panic]);
    let meter = RunMeter::new(None);
    let mut metered = MeteredModelPort::new(port.clone(), &meter);

    let contained = catch_unwind(AssertUnwindSafe(|| turn(&mut metered)));
    assert!(contained.is_err(), "the scripted turn panics");

    let report = meter.report(&run_id());
    assert_eq!(port.seen(), 1, "the port saw one turn");
    assert_eq!(
        report.model_calls,
        port.seen(),
        "the reported calls equal the turns the port saw: {report:?}"
    );
}

/// The gate of one side of [`two_executors_sharing_a_meter_each_count_their_own_calls`]: it
/// reports that its step is open, waits to be told to turn, turns `turns` times and returns.
struct Gated {
    port: Mutex<MeteredModelPort<Port>>,
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
        let mut port = self.port.lock().unwrap_or_else(PoisonError::into_inner);
        for _ in 0..self.turns {
            let _ = turn(&mut *port);
        }
        ExecutorOutcome::NoUsefulAction(Unit(true))
    }
}

fn gated(
    port: &Port,
    meter: &RunMeter,
    turns: usize,
) -> (MeteredExecutor<Gated>, Receiver<()>, Sender<()>) {
    let (opened, opened_rx) = channel();
    let (go_tx, go) = channel();
    (
        MeteredExecutor::new(
            Gated {
                port: Mutex::new(MeteredModelPort::new(port.clone(), meter)),
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
    let port = Port::new([]);
    let meter = RunMeter::new(None);
    let (a, a_opened, a_go) = gated(&port, &meter, 2);
    let (b, b_opened, b_go) = gated(&port, &meter, 1);

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
    let mut host = MeteredModelPort::new(port.clone(), &meter);
    for _ in 0..3 {
        let _ = turn(&mut host);
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
    let port = Port::new([]);
    let meter = RunMeter::new(None);
    let executor = MeteredExecutor::new(
        MeteredExecutor::new(Turns::new(&port, &meter, 2), &meter),
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
#[test]
fn a_failed_turn_states_no_cost_in_a_priced_run() {
    let port = Port::new([Step::Fail]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let _ =
        MeteredExecutor::new(Turns::new(&port, &meter, 1), &meter).run(&commission(), &frontier());

    let report = meter.report(&run_id());
    assert_eq!(report.model_calls, 1, "{report:?}");
    assert_eq!(
        report.cost_micro_usd, None,
        "a call whose cost nobody reported is not a free call: {report:?}"
    );
}

/// A turn through a fork of a metered port, and through a fork of that fork, is a call of the
/// same run, counted in the step that is open.
#[test]
fn a_turn_through_a_fork_of_a_metered_port_is_counted() {
    struct Forking {
        port: MeteredModelPort<Port>,
    }

    impl AgentExecutor for Forking {
        fn run(
            &self,
            _commission: &Commission<commission_state::Assigned>,
            _frontier: &Frontier<frontier_state::Issued>,
        ) -> ExecutorOutcome {
            let mut fork = self.port.fork().expect("the port forks");
            let _ = turn(&mut *fork);
            let mut grandchild = fork.fork().expect("the fork forks");
            let _ = turn(&mut *grandchild);
            ExecutorOutcome::NoUsefulAction(Unit(true))
        }
    }

    let port = Port::new([]).forking();
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

/// An answer that reported no usage is a call, adds no tokens, and leaves its step's cost and the
/// run's absent, while a fully priced step keeps its own cost.
#[test]
fn an_answer_without_usage_is_a_call_whose_cost_is_absent() {
    let port = Port::new([
        // Step 1: priced, then an answer without usage.
        Step::Answer(answer(Some(usage(MODEL, 100, 40, 10)))),
        Step::Answer(answer(None)),
        // Step 2: priced.
        Step::Answer(answer(Some(usage(MODEL, 100, 40, 10)))),
    ]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let executor = MeteredExecutor::new(Turns::new(&port, &meter, 2), &meter);
    let _ = executor.run(&commission(), &frontier());
    let single = MeteredExecutor::new(Turns::new(&port, &meter, 1), &meter);
    let _ = single.run(&commission(), &frontier());

    let report = meter.report(&run_id());
    assert_eq!(steps(&report), vec![(1, 2), (2, 1)], "{report:?}");
    assert_eq!(report.steps[0].input_tokens, 100, "{report:?}");
    assert_eq!(report.steps[0].cost_micro_usd, None, "{report:?}");
    assert_eq!(report.steps[1].cost_micro_usd, Some(180), "{report:?}");
    assert_eq!(report.cost_micro_usd, None, "{report:?}");
    assert_eq!(report.input_tokens, 200, "{report:?}");
}

/// A step in which one turn's model is priced and another's is not has no cost.
#[test]
fn a_step_mixing_a_priced_and_an_unlisted_model_has_no_cost() {
    let port = Port::new([
        Step::Answer(answer(Some(usage(MODEL, 100, 40, 10)))),
        Step::Answer(answer(Some(usage("unlisted", 100, 40, 10)))),
    ]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let _ =
        MeteredExecutor::new(Turns::new(&port, &meter, 2), &meter).run(&commission(), &frontier());

    let report = meter.report(&run_id());
    assert_eq!(steps(&report), vec![(1, 2)], "{report:?}");
    assert_eq!(report.steps[0].cost_micro_usd, None, "{report:?}");
    assert_eq!(report.cost_micro_usd, None, "{report:?}");
}

/// A turn made before the first executor call and one made after it returned are counted in step
/// 0, not in the step that closed, so the steps sum to the totals and the step keeps only its own.
#[test]
fn calls_outside_an_executor_call_are_counted_in_step_0() {
    let port = Port::new([]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let mut host = MeteredModelPort::new(port.clone(), &meter);
    let _ = turn(&mut host);
    let _ =
        MeteredExecutor::new(Turns::new(&port, &meter, 3), &meter).run(&commission(), &frontier());
    let _ = turn(&mut host);

    let report = meter.report(&run_id());
    assert_eq!(port.seen(), 5);
    assert_eq!(steps(&report), vec![(0, 2), (1, 3)], "{report:?}");
    assert_eq!(report.model_calls, 5, "{report:?}");
    assert_eq!(report.steps[0].cost_micro_usd, Some(2 * 180), "{report:?}");
    assert_eq!(report.cost_micro_usd, Some(5 * 180), "{report:?}");
}

/// A step closes when its executor call unwinds, so a later call is not counted in it.
#[test]
fn a_step_closes_when_its_executor_call_panics() {
    struct Panics {
        port: Mutex<MeteredModelPort<Port>>,
    }

    impl AgentExecutor for Panics {
        fn run(
            &self,
            _commission: &Commission<commission_state::Assigned>,
            _frontier: &Frontier<frontier_state::Issued>,
        ) -> ExecutorOutcome {
            let _ = turn(&mut *self.port.lock().unwrap_or_else(PoisonError::into_inner));
            panic!("the executor panicked");
        }
    }

    let port = Port::new([]);
    let meter = RunMeter::new(None);
    let executor = MeteredExecutor::new(
        Panics {
            port: Mutex::new(MeteredModelPort::new(port.clone(), &meter)),
        },
        &meter,
    );
    let unwound = catch_unwind(AssertUnwindSafe(|| {
        executor.run(&commission(), &frontier())
    }));
    assert!(unwound.is_err());
    let _ = turn(&mut MeteredModelPort::new(port.clone(), &meter));

    let report = meter.report(&run_id());
    assert_eq!(steps(&report), vec![(0, 1), (1, 1)], "{report:?}");
}

/// A usage at the edge of `u64` saturates to the largest figure rather than wrapping negative,
/// and the cost of an unpriceable amount is still stated as the saturated figure, not zero.
#[test]
fn a_usage_at_u64_max_saturates_rather_than_wrapping() {
    let port = Port::new([
        Step::Answer(answer(Some(usage(MODEL, u64::MAX, 0, u64::MAX)))),
        Step::Answer(answer(Some(usage(MODEL, u64::MAX, 0, u64::MAX)))),
    ]);
    let meter = RunMeter::new(Some(card(MODEL)));
    let _ =
        MeteredExecutor::new(Turns::new(&port, &meter, 2), &meter).run(&commission(), &frontier());

    let report = meter.report(&run_id());
    assert_eq!(report.input_tokens, i64::MAX, "{report:?}");
    assert_eq!(report.output_tokens, i64::MAX, "{report:?}");
    assert!(report.cost_micro_usd >= Some(0), "{report:?}");
    assert_eq!(report.steps[0].input_tokens, i64::MAX, "{report:?}");
}
