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
//! saw: one process segment of the run, and the earlier segments it was resumed from. A meter
//! gives what it counted as a [`CarriedUsage`] ([`RunMeter::carried`]), which a workflow run
//! record keeps (`story:dw-durable-run-state`); a meter in the process that resumes the run takes
//! it back ([`RunMeter::carry`]), so its report lists the carried steps first, numbered as they
//! were, numbers its own after them, and sums the carried effect usage with its own.
//!
//! Beside the model-port usage, the report carries what the run's performed effects reported they
//! spent on their own model calls, as Commission's runtime charged it to the run and to each
//! action ([`b10x_loom_commission::runtime::LoopEnd::charges`]), once the host hands it to the
//! meter ([`RunMeter::charge_effects`]). It is never summed into the model-port figures.

use std::cell::RefCell;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use b10x_loom_commission::model::responsibility::{
    ChargedUsage, Commission, ExecutorOutcome, Frontier, RunCharges, RunId, commission_state,
    frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;

use crate::harness::turn_loop::RateCard;
use crate::harness::wire::{
    ModelPort, StreamSink, TurnOutcome, TurnRequest, Usage, WireError, WireId,
};
use crate::model::metering::{ActionUsage, CarriedUsage, EffectsUsage, RunUsageReport, StepUsage};
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
    /// What the run's effects were charged, as last handed to [`RunMeter::charge_effects`].
    effects: Option<RunCharges>,
    /// What earlier segments' effects were charged, as [`RunMeter::carry`] took it.
    carried_effects: Option<EffectsUsage>,
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
    /// The tally a carried step's usage stands for. A negative figure counts nothing.
    fn carried(usage: &StepUsage) -> Self {
        let count = |figure: i64| u64::try_from(figure).unwrap_or(0);
        Self {
            label: usage.label.clone(),
            calls: count(usage.model_calls),
            input_tokens: count(usage.input_tokens),
            output_tokens: count(usage.output_tokens),
            cached_input_tokens: count(usage.cached_input_tokens),
            cost_micro_usd: usage
                .cost_micro_usd
                .and_then(|cost| u64::try_from(cost).ok()),
        }
    }

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

/// The report's effect usage for `charges`: all zero, with no actions, when nothing was charged.
fn effects_usage(charges: Option<&RunCharges>) -> EffectsUsage {
    let line = |action: &str, charged: &ChargedUsage| ActionUsage {
        action: action.to_owned(),
        trials: charged.trials,
        input_tokens: charged.input_tokens,
        output_tokens: charged.output_tokens,
        duration_ms: charged.duration_ms,
        cost_micro_usd: charged.cost_micro_usd,
    };
    let Some(charges) = charges else {
        return EffectsUsage {
            trials: 0,
            input_tokens: 0,
            output_tokens: 0,
            duration_ms: 0,
            cost_micro_usd: Some(0),
            actions: Vec::new(),
        };
    };
    let run = line("", &charges.run);
    EffectsUsage {
        trials: run.trials,
        input_tokens: run.input_tokens,
        output_tokens: run.output_tokens,
        duration_ms: run.duration_ms,
        cost_micro_usd: run.cost_micro_usd,
        actions: charges
            .actions
            .iter()
            .map(|charge| line(&charge.action, &charge.charged))
            .collect(),
    }
}

/// `carried` and `current` summed: every figure added, the cost absent when either is, and the
/// actions of `carried` first, each with `current`'s figures for the same action added, then the
/// actions only `current` charged, in its order.
fn sum_effects(carried: &EffectsUsage, current: EffectsUsage) -> EffectsUsage {
    let cost = |a: Option<i64>, b: Option<i64>| a.zip(b).map(|(a, b)| a.saturating_add(b));
    let mut actions = carried.actions.clone();
    for action in current.actions {
        match actions
            .iter_mut()
            .find(|known| known.action == action.action)
        {
            Some(known) => {
                known.trials = known.trials.saturating_add(action.trials);
                known.input_tokens = known.input_tokens.saturating_add(action.input_tokens);
                known.output_tokens = known.output_tokens.saturating_add(action.output_tokens);
                known.duration_ms = known.duration_ms.saturating_add(action.duration_ms);
                known.cost_micro_usd = cost(known.cost_micro_usd, action.cost_micro_usd);
            }
            None => actions.push(action),
        }
    }
    EffectsUsage {
        trials: carried.trials.saturating_add(current.trials),
        input_tokens: carried.input_tokens.saturating_add(current.input_tokens),
        output_tokens: carried.output_tokens.saturating_add(current.output_tokens),
        duration_ms: carried.duration_ms.saturating_add(current.duration_ms),
        cost_micro_usd: cost(carried.cost_micro_usd, current.cost_micro_usd),
        actions,
    }
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
                effects: None,
                carried_effects: None,
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
            effects: match &ledger.carried_effects {
                Some(carried) => sum_effects(carried, effects_usage(ledger.effects.as_ref())),
                None => effects_usage(ledger.effects.as_ref()),
            },
        }
    }

    /// What the meter counted, as a run record carries it to the process that resumes the run:
    /// the steps and the effect usage of [`Self::report`].
    #[must_use]
    pub fn carried(&self) -> CarriedUsage {
        let RunUsageReport { steps, effects, .. } = self.report(&RunId(
            b10x_loom_commission::model::primitives::Uuid(String::new()),
        ));
        CarriedUsage { steps, effects }
    }

    /// Takes `carried`, what earlier segments of the run counted ([`Self::carried`]), as the start
    /// of this meter's run, for a meter that has counted nothing yet: its steps become this
    /// meter's, numbered as they were, the next executor call opening the step after the last of
    /// them, and its effect usage is summed with what this meter's run is charged. What the meter
    /// counted before is replaced.
    pub fn carry(&self, carried: &CarriedUsage) {
        let mut ledger = self.ledger();
        let priced = ledger.prices.is_some();
        let last = carried
            .steps
            .iter()
            .filter_map(|step| usize::try_from(step.step).ok())
            .max()
            .unwrap_or(0);
        let mut tallies = vec![Tally::new(priced); last + 1];
        for step in &carried.steps {
            if let Ok(at) = usize::try_from(step.step) {
                tallies[at] = Tally::carried(step);
            }
        }
        ledger.tallies = tallies;
        ledger.carried_effects = Some(carried.effects.clone());
    }

    /// Records `charges`, what Commission's runtime charged the run's effects
    /// ([`b10x_loom_commission::runtime::LoopEnd::charges`]), for the report's `effects`. A later
    /// call replaces it.
    pub fn charge_effects(&self, charges: &RunCharges) {
        self.ledger().effects = Some(charges.clone());
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
