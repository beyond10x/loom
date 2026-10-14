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
//! call made while that executor call runs is counted in its step. A call made while no executor
//! call runs is counted in step 0, which the report lists only when there was one, so the steps
//! always sum to the totals.
//!
//! Each turn is priced once, with the run's [`RateCard`], and rounded once; a cost is the sum of its
//! rounded turns, as [`RateCard::price`] holds. No price is reported as absent, never zero:
//! without a rate card, for a model the card does not price, after a turn that answered without
//! reporting its usage, or after a turn that failed or unwound (its tokens, and so its cost, are
//! unknown), the step's cost is absent, and so is the run's.
//!
//! The report is the generated [`RunUsageReport`] ([`RunMeter::report`]). It covers what the meter
//! saw: one process segment of the run.

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
///
/// Contract: executor calls on one meter do not overlap. The meter holds one open step, so two
/// [`MeteredExecutor`] calls running at once on it count each other's model calls in whichever
/// step opened last. `story:meter-overlapping-calls` would lift this.
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
    /// The index into `tallies` the next call is counted in: 0 while no executor call runs.
    open: usize,
}

/// One step's figures as the meter counts them.
#[derive(Debug, Clone, Copy, Default)]
struct Tally {
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
                open: 0,
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

    /// Counts one call in the open step, before its `turn` runs, so a turn that fails or unwinds
    /// is still a call. Returns the step the call's usage belongs to.
    fn call(&self) -> usize {
        let mut ledger = self.ledger();
        let open = ledger.open;
        let tally = &mut ledger.tallies[open];
        tally.calls = tally.calls.saturating_add(1);
        open
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

    /// Opens the next step and returns the one that was open, to restore when it closes.
    fn open_step(&self) -> usize {
        let mut ledger = self.ledger();
        let priced = ledger.prices.is_some();
        ledger.tallies.push(Tally::new(priced));
        let previous = ledger.open;
        ledger.open = ledger.tallies.len() - 1;
        previous
    }

    fn close_step(&self, previous: usize) {
        self.ledger().open = previous;
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

/// One metered `turn` of `port`.
fn metered_turn(
    port: &mut (impl ModelPort + ?Sized),
    meter: &RunMeter,
    request: &TurnRequest,
    sink: &mut dyn StreamSink,
) -> Result<TurnOutcome, WireError> {
    let mut call = Call {
        meter,
        step: meter.call(),
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

    fn turn(
        &mut self,
        request: &TurnRequest,
        sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        metered_turn(&mut self.inner, &self.meter, request, sink)
    }

    /// The inner port's fork, metered into the same run: a turn through a fork is a call too.
    fn fork(&self) -> Option<Box<dyn ModelPort + Send + '_>> {
        let fork = self.inner.fork()?;
        Some(Box::new(MeteredFork {
            inner: fork,
            meter: self.meter.clone(),
        }))
    }
}

/// A fork of a metered port, metered into the same run.
struct MeteredFork<'a> {
    inner: Box<dyn ModelPort + Send + 'a>,
    meter: RunMeter,
}

impl ModelPort for MeteredFork<'_> {
    fn wire(&self) -> &WireId {
        self.inner.wire()
    }

    fn turn(
        &mut self,
        request: &TurnRequest,
        sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        metered_turn(&mut *self.inner, &self.meter, request, sink)
    }

    fn fork(&self) -> Option<Box<dyn ModelPort + Send + '_>> {
        let fork = self.inner.fork()?;
        Some(Box::new(MeteredFork {
            inner: fork,
            meter: self.meter.clone(),
        }))
    }
}

/// An [`AgentExecutor`] that opens one step of a [`RunMeter`] per `run` call of `E`.
///
/// Contract: an executor is wrapped by one `MeteredExecutor`, and its calls on one meter do not
/// overlap ([`RunMeter`]). A `MeteredExecutor` over another of the same meter opens two steps for
/// one executor call. `story:meter-overlapping-calls` would lift this.
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
    previous: usize,
}

impl Drop for OpenStep<'_> {
    fn drop(&mut self) {
        self.meter.close_step(self.previous);
    }
}

impl<E: AgentExecutor> AgentExecutor for MeteredExecutor<E> {
    /// `E`'s outcome, unchanged; every model call made meanwhile is counted in a new step.
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let _step = OpenStep {
            meter: &self.meter,
            previous: self.meter.open_step(),
        };
        self.inner.run(commission, frontier)
    }
}
