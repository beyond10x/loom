// SPDX-License-Identifier: Apache-2.0

//! What one Commission run spent on its model, counted at the [`ModelPort`] boundary
//! (`ess/domains/metering.yaml`, `loom.metering`).
//!
//! A [`RunMeter`] is shared by the [`MeteredModelPort`]s and the [`MeteredExecutor`] of one run.
//! Each `turn` through a metered port is one model call, whoever made it: the loop, the
//! reasoning-model selector, an argument generator or compaction. The call is counted before the
//! turn runs, so a turn that failed or unwound is still a call; a turn that answered adds the
//! tokens the provider reported for it, and one that failed or unwound adds none.
//!
//! The [`MeteredExecutor`] opens one step per [`AgentExecutor::run`] call, counted from 1, and every
//! call that executor call makes is counted in its step: a call on the thread the executor call
//! runs on, and a call through a fork of a metered port made in it, on whichever thread the fork
//! turns (delegates side by side). The step is carried by the executor call, not held by the
//! meter, so executor calls on one meter may overlap on several threads, each counting its own
//! calls, and a [`MeteredExecutor`] inside an executor call of the same meter opens no second step.
//! A call made while no executor call runs is counted in step 0, which the report lists only when
//! there was one, so the steps always sum to the totals. A step carries the label its executor gave the call
//! ([`RunMeter::label_step`]): a [`crate::workflow::WorkflowExecutor`] metered with
//! [`crate::workflow::WorkflowExecutor::with_meter`] labels each call with the workflow step it
//! took. Step 0 and a call that took no named step carry none.
//!
//! Each turn is priced once, with the run's [`RateCard`], and rounded once; a cost is the sum of its
//! rounded turns, as [`RateCard::price`] holds. No price is reported as absent, never zero:
//! without a rate card, for a model the card does not price, after a turn that answered without
//! reporting its usage, or after a turn that failed or unwound (its tokens, and so its cost, are
//! unknown), the step's cost is absent, and so is the run's.
//!
//! The report is the generated [`RunUsageReport`] ([`RunMeter::report`]). It covers what the meter
//! saw: one process segment of the run.

use std::cell::RefCell;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use b10x_loom_commission::model::responsibility::{
    Commission, ExecutorOutcome, Frontier, RunId, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;

use crate::harness::turn_loop::RateCard;
use crate::harness::wire::{
    ModelPort, StreamSink, TurnOutcome, TurnRequest, Usage, WireError, WireId,
};
use crate::model::metering::{RunUsageReport, StepUsage};
use crate::model::primitives::Uuid;
use crate::model::run::CommissionRunId;

/// The usage of one run, shared by every metered port and the metered executor of the run.
///
/// Cloning it is cheap and every clone counts into the same run.
#[derive(Debug, Clone)]
pub struct RunMeter {
    shared: Arc<Mutex<Ledger>>,
}

/// What the meter has counted so far.
#[derive(Debug)]
struct Ledger {
    prices: Option<RateCard>,
    /// Step 0 first, then one tally per executor call, in order.
    tallies: Vec<Tally>,
}

thread_local! {
    /// The steps open on this thread, innermost last: the meter's identity ([`RunMeter::id`]) and
    /// the index into its `tallies`. An executor call pushes its step when it begins and removes
    /// it when it returns or unwinds, so a step is open exactly while its executor call runs.
    static OPEN: RefCell<Vec<(usize, usize)>> = const { RefCell::new(Vec::new()) };
}

/// One step's figures as the meter counts them.
#[derive(Debug, Clone, Default)]
struct Tally {
    /// The name the executor gave its call, when it gave one.
    label: Option<String>,
    calls: u64,
    input_tokens: u64,
    output_tokens: u64,
    cached_input_tokens: u64,
    /// The sum of the priced turns; `None` once a turn could not be priced, or without a card.
    cost_micro_usd: Option<u64>,
}

impl Tally {
    fn new(priced: bool) -> Self {
        Self {
            cost_micro_usd: priced.then_some(0),
            ..Self::default()
        }
    }

    fn usage(&self, step: usize) -> StepUsage {
        StepUsage {
            step: figure(u64::try_from(step).unwrap_or(u64::MAX)),
            label: self.label.clone(),
            model_calls: figure(self.calls),
            input_tokens: figure(self.input_tokens),
            output_tokens: figure(self.output_tokens),
            cached_input_tokens: figure(self.cached_input_tokens),
            cost_micro_usd: self.cost_micro_usd.map(figure),
        }
    }
}

/// A count as the generated model's `Integer`, saturating: no run reaches `i64::MAX` of anything.
fn figure(count: u64) -> i64 {
    i64::try_from(count).unwrap_or(i64::MAX)
}

impl RunMeter {
    /// A meter for one run, pricing each turn with `prices` when there is a card.
    #[must_use]
    pub fn new(prices: Option<RateCard>) -> Self {
        let priced = prices.is_some();
        Self {
            shared: Arc::new(Mutex::new(Ledger {
                prices,
                tallies: vec![Tally::new(priced)],
            })),
        }
    }

    /// The run's usage so far: the totals and the steps in order, step 0 only when a call was
    /// made while no executor call ran. `run` is the Commission run the report covers
    /// ([`b10x_loom_commission::runtime::LoopEnd::run_id`]).
    #[must_use]
    pub fn report(&self, run: &RunId) -> RunUsageReport {
        let ledger = self.ledger();
        let steps: Vec<StepUsage> = ledger
            .tallies
            .iter()
            .enumerate()
            .filter(|(step, tally)| *step > 0 || tally.calls > 0)
            .map(|(step, tally)| tally.usage(step))
            .collect();
        let mut total = Tally::new(ledger.prices.is_some());
        for tally in &ledger.tallies {
            total.calls = total.calls.saturating_add(tally.calls);
            total.input_tokens = total.input_tokens.saturating_add(tally.input_tokens);
            total.output_tokens = total.output_tokens.saturating_add(tally.output_tokens);
            total.cached_input_tokens = total
                .cached_input_tokens
                .saturating_add(tally.cached_input_tokens);
            total.cost_micro_usd = match (total.cost_micro_usd, tally.cost_micro_usd) {
                (Some(sum), Some(cost)) => Some(sum.saturating_add(cost)),
                _ => None,
            };
        }
        let total = total.usage(0);
        RunUsageReport {
            commission_run: CommissionRunId(Uuid(run.0.0.clone())),
            model_calls: total.model_calls,
            input_tokens: total.input_tokens,
            output_tokens: total.output_tokens,
            cached_input_tokens: total.cached_input_tokens,
            cost_micro_usd: total.cost_micro_usd,
            steps,
        }
    }

    /// Labels the step of the executor call running on this thread with `label`, the name its
    /// executor gave the call (a workflow step's name); a later label of the same step replaces
    /// it. Nothing is labelled while no executor call runs: step 0 carries no label.
    pub fn label_step(&self, label: &str) {
        if let Some(step) = self.open_here() {
            self.ledger().tallies[step].label = Some(label.to_owned());
        }
    }

    /// This meter's identity among the steps open on a thread: the address of the ledger every
    /// clone shares, alive while a step of it is open, since its executor call holds the meter.
    fn id(&self) -> usize {
        Arc::as_ptr(&self.shared).addr()
    }

    /// The step of the innermost executor call of this meter running on this thread, if any.
    fn open_here(&self) -> Option<usize> {
        let id = self.id();
        OPEN.with_borrow(|open| {
            open.iter()
                .rev()
                .find(|(meter, _)| *meter == id)
                .map(|(_, step)| *step)
        })
    }

    /// The step a call made now on this thread is counted in: its executor call's step, else
    /// `outside`, the step of the executor call a fork was made in (0 for a port of its own).
    fn step_here(&self, outside: usize) -> usize {
        self.open_here().unwrap_or(outside)
    }

    /// Counts one call in `step`, before its `turn` runs, so a turn that fails or unwinds is
    /// still a call. Returns the step the call's usage belongs to.
    fn call(&self, step: usize) -> usize {
        let mut ledger = self.ledger();
        let tally = &mut ledger.tallies[step];
        tally.calls = tally.calls.saturating_add(1);
        step
    }

    /// Adds the usage a counted call reported to its `step`. Without one (a turn that failed or
    /// unwound, or an answer without usage) the tokens are unknown, so no price can be stated.
    fn record(&self, step: usize, reported: Option<&Usage>) {
        let mut guard = self.ledger();
        let ledger = &mut *guard;
        let card = ledger.prices.as_ref();
        let tally = &mut ledger.tallies[step];
        let Some(usage) = reported else {
            tally.cost_micro_usd = None;
            return;
        };
        tally.input_tokens = tally.input_tokens.saturating_add(usage.input_tokens);
        tally.output_tokens = tally.output_tokens.saturating_add(usage.output_tokens);
        tally.cached_input_tokens = tally
            .cached_input_tokens
            .saturating_add(usage.cached_input_tokens);
        tally.cost_micro_usd = match (tally.cost_micro_usd, card.and_then(|c| c.price(usage))) {
            (Some(sum), Some(cost)) => Some(sum.saturating_add(cost)),
            _ => None,
        };
    }

    /// Opens the next step for the executor call beginning on this thread, unless an executor call
    /// of this meter already runs here (a metered executor over another of the same meter): one
    /// executor call is one step. Returns the step it opened, to close when the call ends.
    fn open_step(&self) -> Option<usize> {
        if self.open_here().is_some() {
            return None;
        }
        let step = {
            let mut ledger = self.ledger();
            let priced = ledger.prices.is_some();
            ledger.tallies.push(Tally::new(priced));
            ledger.tallies.len() - 1
        };
        OPEN.with_borrow_mut(|open| open.push((self.id(), step)));
        Some(step)
    }

    /// Closes `step` on this thread: calls made here later are no longer counted in it.
    fn close_step(&self, step: usize) {
        let id = self.id();
        OPEN.with_borrow_mut(|open| {
            if let Some(at) = open.iter().rposition(|entry| *entry == (id, step)) {
                open.remove(at);
            }
        });
    }

    fn ledger(&self) -> MutexGuard<'_, Ledger> {
        self.shared.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A [`ModelPort`] that counts every `turn` of `M` into a [`RunMeter`].
#[derive(Debug)]
pub struct MeteredModelPort<M> {
    inner: M,
    meter: RunMeter,
}

impl<M: ModelPort> MeteredModelPort<M> {
    /// `inner`, its turns counted into `meter`.
    pub fn new(inner: M, meter: &RunMeter) -> Self {
        Self {
            inner,
            meter: meter.clone(),
        }
    }

    /// The port this one meters.
    pub fn into_inner(self) -> M {
        self.inner
    }
}

/// One metered `turn` of `port`, counted in `step`.
fn metered_turn(
    port: &mut (impl ModelPort + ?Sized),
    meter: &RunMeter,
    step: usize,
    request: &TurnRequest,
    sink: &mut dyn StreamSink,
) -> Result<TurnOutcome, WireError> {
    let mut call = Call {
        meter,
        step: meter.call(step),
        settled: false,
    };
    let outcome = port.turn(request, sink);
    call.settled = true;
    meter.record(
        call.step,
        outcome
            .as_ref()
            .ok()
            .and_then(|answer| answer.usage.as_ref()),
    );
    outcome
}

/// A call counted before its `turn`: when the turn unwinds, its cost is unknown.
struct Call<'m> {
    meter: &'m RunMeter,
    step: usize,
    settled: bool,
}

impl Drop for Call<'_> {
    fn drop(&mut self) {
        if !self.settled {
            self.meter.record(self.step, None);
        }
    }
}

impl<M: ModelPort> ModelPort for MeteredModelPort<M> {
    fn wire(&self) -> &WireId {
        self.inner.wire()
    }

    /// A call of the executor call running on this thread, or of step 0 when none runs.
    fn turn(
        &mut self,
        request: &TurnRequest,
        sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        let step = self.meter.step_here(0);
        metered_turn(&mut self.inner, &self.meter, step, request, sink)
    }

    /// The inner port's fork, metered into the same run: a turn through a fork is a call too, of
    /// the executor call the fork was made in, on whichever thread it turns.
    fn fork(&self) -> Option<Box<dyn ModelPort + Send + '_>> {
        let fork = self.inner.fork()?;
        Some(Box::new(MeteredFork {
            inner: fork,
            meter: self.meter.clone(),
            step: self.meter.step_here(0),
        }))
    }
}

/// A fork of a metered port, metered into the same run.
struct MeteredFork<'a> {
    inner: Box<dyn ModelPort + Send + 'a>,
    meter: RunMeter,
    /// The step of the executor call the fork was made in: 0 when none ran.
    step: usize,
}

impl ModelPort for MeteredFork<'_> {
    fn wire(&self) -> &WireId {
        self.inner.wire()
    }

    /// A call of the executor call running on this thread, else of the one the fork was made in.
    fn turn(
        &mut self,
        request: &TurnRequest,
        sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        let step = self.meter.step_here(self.step);
        metered_turn(&mut *self.inner, &self.meter, step, request, sink)
    }

    fn fork(&self) -> Option<Box<dyn ModelPort + Send + '_>> {
        let fork = self.inner.fork()?;
        Some(Box::new(MeteredFork {
            inner: fork,
            meter: self.meter.clone(),
            step: self.meter.step_here(self.step),
        }))
    }
}

/// An [`AgentExecutor`] that opens one step of a [`RunMeter`] per `run` call of `E`.
///
/// Calls of several metered executors on one meter may overlap, each on its own thread, and each
/// counts in its own step. Inside an executor call of the same meter (a `MeteredExecutor` over
/// another) it opens no step of its own: one executor call is one step.
#[derive(Debug)]
pub struct MeteredExecutor<E> {
    inner: E,
    meter: RunMeter,
}

impl<E: AgentExecutor> MeteredExecutor<E> {
    /// `inner`, each of its `run` calls one step of `meter`.
    pub fn new(inner: E, meter: &RunMeter) -> Self {
        Self {
            inner,
            meter: meter.clone(),
        }
    }

    /// The executor this one meters.
    pub fn into_inner(self) -> E {
        self.inner
    }
}

/// Closes a step when its executor call returns or unwinds.
struct OpenStep<'m> {
    meter: &'m RunMeter,
    step: usize,
}

impl Drop for OpenStep<'_> {
    fn drop(&mut self) {
        self.meter.close_step(self.step);
    }
}

impl<E: AgentExecutor> AgentExecutor for MeteredExecutor<E> {
    /// `E`'s outcome, unchanged; every model call it makes is counted in a new step, or in the
    /// step already open on this thread for an executor call of the same meter.
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let _step = self.meter.open_step().map(|step| OpenStep {
            meter: &self.meter,
            step,
        });
        self.inner.run(commission, frontier)
    }
}
