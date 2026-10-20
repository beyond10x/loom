// SPDX-License-Identifier: Apache-2.0

//! What one Commission run spent on its model, counted at llm's [`Model`] boundary
//! (`ess/domains/metering.yaml`, `loom.metering`).
//!
//! A [`RunMeter`] is shared by the [`MeteredModelPort`]s and the [`MeteredExecutor`] of one run.
//! A [`MeteredModelPort`] is an llm [`Model`] over the model a host built, and each `turn` of it is
//! one model call, whoever made it: the model selector, an argument generator, a host's own
//! request. The call is counted when `turn` is called, before the turn runs, so a turn that failed,
//! unwound or was dropped unfinished is still a call; a turn that answered adds the tokens the
//! provider reported for it, and one that failed, unwound or was dropped adds none.
//!
//! The [`MeteredExecutor`] opens one step per executor call ([`AgentExecutor::run_with_usage`],
//! which the runtime makes, or [`AgentExecutor::run`]), counted from 1, and every call that
//! executor call makes is counted in its step: a `turn` called on the thread the executor call runs
//! on, on whichever thread the turn's future is then driven. The step is carried by the executor call, not
//! held by the meter, so executor calls on one meter may overlap on several threads, each counting
//! its own calls, and a [`MeteredExecutor`] inside an executor call of the same meter opens no
//! second step. A call made while no executor call runs on its thread is counted in step 0, which
//! the report lists only when there was one, so the steps always sum to the totals. A step carries
//! the label its executor gave the call ([`RunMeter::label_step`]): a
//! [`crate::workflow::WorkflowExecutor`] metered with
//! [`crate::workflow::WorkflowExecutor::with_meter`] labels each call with the workflow step it
//! took. Step 0 and a call that took no named step carry none.
//!
//! A turn's tokens are its usage as llm reports it ([`TurnObservation`]), taken only when the
//! usage is final and names both its input and its output tokens; cached input tokens it does not
//! name count as none. Each turn is priced once, with the run's [`RateCard`], against the model the
//! provider reported serving it (the observation's `upstream_model`, else the model it is bound
//! to), and rounded once; a cost is the sum of its rounded turns, as [`RateCard::price`] holds. No
//! price is reported as absent, never zero: after a turn without a rate card, for a model the card
//! does not price, after a turn that answered without a usage the meter takes (none, one that is
//! not final, or one that does not name its input or output tokens), or after a turn that failed,
//! unwound or was dropped (its tokens, and so its cost, are unknown, whatever usage an error
//! carries), the step's cost is absent, and so is the run's. A step that made no model call needs
//! no price: its cost is the cost its executor reported, and zero when it reported no usage, card
//! or none.
//!
//! The ported model loop's own model port is metered by a [`MeteredModelPort`] too, by the same
//! rules, until that port is deleted; that implementation lives beside the loop.
//!
//! The report is the generated [`RunUsageReport`] ([`RunMeter::report`]). It covers what the meter
//! saw: one process segment of the run, and the earlier segments it was resumed from. A meter
//! gives what it counted as a [`CarriedUsage`] ([`RunMeter::carried`]), which a workflow run
//! record keeps (`story:dw-durable-run-state`); a meter in the process that resumes the run takes
//! it back ([`RunMeter::carry`]), so its report lists the carried steps first, numbered as they
//! were, numbers its own after them, and sums the carried effect usage with its own.
//!
//! An executor that is not a model loop (an agent session, a remote executor) reports what each
//! call spent ([`AgentExecutor::run_with_usage`], `story:executor-reports-usage`). The
//! [`MeteredExecutor`] adds that usage to the call's step when it is called through
//! `run_with_usage`, which hands the usage on to the runtime, and never through `run`, which hands
//! on the outcome alone, so the meter shows only usage the run is charged: its input and output
//! tokens, and its cost to the cost of the step's priced calls, so a step that made no model call
//! takes the reported cost, with or without a rate card, and a usage with no cost leaves the
//! step's cost absent. It is no model call, and the steps still sum to the totals. An executor
//! whose model calls go through a metered model reports none of them, so nothing is counted twice.
//!
//! Beside the model usage, the report carries what the run's performed effects reported they
//! spent on their own model calls, as Commission's runtime charged it to the run and to each
//! action ([`b10x_loom_commission::runtime::LoopEnd::charges`]), once the host hands it to the
//! meter ([`RunMeter::charge_effects`]). It is never summed into the model figures. Its totals are
//! the actions' sums: the run's charge also holds the usage its executor calls reported, which the
//! report holds in their steps instead.
//!
//! # The rate card
//!
//! Rates are not compiled in: the operator points the run at a [`RateCard`], a small JSON
//! document that names its own source and the day it was read, so every figure the run reports
//! can be traced back to it.
//!
//! ```json
//! {
//!   "source": "openai.com/api/pricing, read by hand",
//!   "as_of": "2026-08-24",
//!   "models": {
//!     "gpt-5.6-sol": {
//!       "input_usd_per_mtok": 1.25,
//!       "cached_input_usd_per_mtok": 0.125,
//!       "output_usd_per_mtok": 10.0
//!     }
//!   }
//! }
//! ```
//!
//! Every figure is an integer count of micro-US-dollars. Arithmetic happens in pico-dollars per
//! token, so no rate is rounded before it is multiplied, and each turn is rounded to a micro-dollar
//! exactly once; the run total is the sum of the rounded turns, so a reader's addition of the
//! per-step figures agrees with the total printed beside them.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use b10x_loom_commission::model::responsibility::{
    ChargedUsage, Commission, ExecutorOutcome, ExecutorStep, ExecutorUsage, Frontier, RunCharges,
    RunId, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use llm_core::{
    BoxFuture, Cancel, Capabilities, Error, Model, Provenance, StreamSink, TurnObservation,
    TurnOutcome, TurnRequest, Usage,
};
use serde::{Deserialize, Serialize};

use crate::model::metering::{ActionUsage, CarriedUsage, EffectsUsage, RunUsageReport, StepUsage};
use crate::model::primitives::Uuid;
use crate::model::run::CommissionRunId;

/// Pico-dollars in one micro-dollar.
const PICO_PER_MICRO: u128 = 1_000_000;
/// Pico-dollars per token, for a rate written as dollars per million tokens.
const PICO_PER_USD_PER_MTOK: f64 = 1_000_000.0;

/// A rate as an operator wrote it: US dollars per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRates {
    /// Input tokens the provider did **not** serve from its cache: llm's cache counts partition a
    /// turn's input tokens, so the uncached count this rate applies to is the difference (see
    /// [`RateCard::price`]).
    pub input_usd_per_mtok: f64,
    /// Input tokens the provider served from its prompt cache.
    pub cached_input_usd_per_mtok: f64,
    /// Input tokens the provider wrote into its prompt cache, when that route reports them as a
    /// separately billed class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_usd_per_mtok: Option<f64>,
    /// Output tokens, including any reasoning tokens the provider billed as output.
    pub output_usd_per_mtok: f64,
}

/// Rates for one model, converted once into pico-dollars per token: integers, so a run's cost is
/// exact arithmetic rather than a float sum whose result depends on the order the turns arrived in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rates {
    input: u64,
    cached_input: u64,
    cache_creation_input: Option<u64>,
    output: u64,
}

/// Rates for some set of models, and where they came from: what a [`RunMeter`] prices each turn
/// with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateCard {
    /// Where these rates came from, in the operator's own words. Required: a price with no
    /// provenance is a number nobody can check.
    pub source: String,
    /// The day the rates were read, as `YYYY-MM-DD`.
    pub as_of: String,
    /// One entry per model. A model with no entry is a model this card does not price.
    pub models: BTreeMap<String, ModelRates>,
}

/// Why a rate card was refused. Every variant refuses the run rather than pricing part of it.
///
/// No `Eq`: [`RateCardError::BadRate`] carries the rate as it was written, a float.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RateCardError {
    #[error("the rate card is not readable JSON: {0}")]
    Unreadable(String),
    #[error("the rate card names no `source`; a price with no provenance cannot be checked")]
    MissingSource,
    #[error("`as_of` is `{0}`, which is not a `YYYY-MM-DD` date")]
    BadDate(String),
    #[error("`{model}.{field}` is `{value}`, which is not a rate this card can hold")]
    BadRate {
        model: String,
        field: &'static str,
        value: f64,
    },
}

impl RateCard {
    /// Reads a card, refusing every rate it cannot hold exactly.
    ///
    /// # Errors
    ///
    /// [`RateCardError`] for unreadable JSON, a missing source, a malformed date, or a rate that is
    /// negative, not finite, too large to convert or not a whole pico-dollar per token. Nothing is
    /// priced from a card that failed here: a partly valid card would report a figure for some
    /// turns and silence for others, and the silence would read as free.
    pub fn parse(text: &str) -> Result<Self, RateCardError> {
        let card: Self = serde_json::from_str(text)
            .map_err(|error| RateCardError::Unreadable(error.to_string()))?;
        if card.source.trim().is_empty() {
            return Err(RateCardError::MissingSource);
        }
        if !is_iso_date(&card.as_of) {
            return Err(RateCardError::BadDate(card.as_of.clone()));
        }
        for (model, rates) in &card.models {
            for (field, value) in [
                ("input_usd_per_mtok", Some(rates.input_usd_per_mtok)),
                (
                    "cached_input_usd_per_mtok",
                    Some(rates.cached_input_usd_per_mtok),
                ),
                (
                    "cache_creation_input_usd_per_mtok",
                    rates.cache_creation_input_usd_per_mtok,
                ),
                ("output_usd_per_mtok", Some(rates.output_usd_per_mtok)),
            ] {
                if let Some(value) = value
                    && pico_per_token(value).is_none()
                {
                    return Err(RateCardError::BadRate {
                        model: model.clone(),
                        field,
                        value,
                    });
                }
            }
        }
        Ok(card)
    }

    /// Every model this card prices, in order.
    pub fn priced_models(&self) -> impl Iterator<Item = &str> {
        self.models.keys().map(String::as_str)
    }

    /// What one turn of `model` cost, in millionths of a US dollar, for the tokens `usage` reports.
    ///
    /// `model` is the model the provider reported serving the turn, not the one the run asked
    /// for: an endpoint that served a different model billed for the one it served. Cached and
    /// cache-creation tokens are parts of the input tokens, each billed at its own rate, and an
    /// unnamed cached count is none.
    ///
    /// `None` when this card does not price `model`, when `usage` does not name its input or its
    /// output tokens, and when it names cache-creation tokens the card holds no rate for. Never
    /// `Some(0)` for a turn it cannot price: a zero is a claim that the turn was free.
    #[must_use]
    pub fn price(&self, model: &str, usage: &Usage) -> Option<u64> {
        let rates = self.rates_for(model)?;
        let input = usage.input_tokens?;
        let output = usage.output_tokens?;
        let cached = usage.cached_input_tokens.unwrap_or(0);
        let created = usage.cache_creation_input_tokens.unwrap_or(0);
        let creation_rate = match (created, rates.cache_creation_input) {
            (0, _) => 0,
            (_, Some(rate)) => rate,
            (_, None) => return None,
        };
        // An inconsistent report is the provider's: the uncached share clamps at nothing rather
        // than wrapping into an enormous charge.
        let uncached = u128::from(input.saturating_sub(cached).saturating_sub(created));
        let pico = uncached * u128::from(rates.input)
            + u128::from(cached) * u128::from(rates.cached_input)
            + u128::from(created) * u128::from(creation_rate)
            + u128::from(output) * u128::from(rates.output);
        Some(to_micro(pico))
    }

    /// The rates this card holds for `model`, or `None` when it holds none.
    fn rates_for(&self, model: &str) -> Option<Rates> {
        let written = self.models.get(model)?;
        Some(Rates {
            input: pico_per_token(written.input_usd_per_mtok)?,
            cached_input: pico_per_token(written.cached_input_usd_per_mtok)?,
            cache_creation_input: match written.cache_creation_input_usd_per_mtok {
                Some(rate) => Some(pico_per_token(rate)?),
                None => None,
            },
            output: pico_per_token(written.output_usd_per_mtok)?,
        })
    }
}

/// A rate in dollars per million tokens, as pico-dollars per token: `None` for anything not
/// finite, negative, beyond what the conversion holds exactly, or not landing on a pico-dollar.
fn pico_per_token(usd_per_mtok: f64) -> Option<u64> {
    if !usd_per_mtok.is_finite() || usd_per_mtok < 0.0 {
        return None;
    }
    let pico = usd_per_mtok * PICO_PER_USD_PER_MTOK;
    if pico > 1e18 || (pico - pico.round()).abs() > 1e-6 {
        return None;
    }
    // Through the decimal rather than through `as`, which reports nothing when it loses something.
    format!("{pico:.0}").parse().ok()
}

/// Rounds pico-dollars to the nearest micro-dollar, half away from zero.
fn to_micro(pico: u128) -> u64 {
    let micro = (pico + PICO_PER_MICRO / 2) / PICO_PER_MICRO;
    u64::try_from(micro).unwrap_or(u64::MAX)
}

/// `YYYY-MM-DD`, checked for shape only.
fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && [0, 1, 2, 3, 5, 6, 8, 9]
            .iter()
            .all(|&index| bytes[index].is_ascii_digit())
}

/// What a turn that answered reported it spent, as the meter takes it: the model the provider
/// reported serving it, else the model it is bound to, and its usage, when that usage is final and
/// names both its input and its output tokens. `None` is an answer without its usage.
fn reported(observation: &TurnObservation) -> Option<(&str, &Usage)> {
    let usage = observation.usage.as_ref().filter(|usage| {
        observation.final_usage && usage.input_tokens.is_some() && usage.output_tokens.is_some()
    })?;
    let model = observation
        .upstream_model
        .as_ref()
        .unwrap_or(&observation.binding.model);
    Some((model.as_str(), usage))
}

/// The usage of one run, shared by every metered model and the metered executor of the run.
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
    /// The sum of the priced turns and the reported costs; `None` once a turn could not be priced
    /// (without a card among the reasons) or a reported usage carried no cost.
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

    /// A step that has counted nothing yet: no call, and so a known cost of zero, card or none. A
    /// turn without a price, a card's absence included, makes it absent once it is counted.
    fn empty() -> Self {
        Self {
            cost_micro_usd: Some(0),
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

/// The report's effect usage for `charges`: the totals over every action, and each action; all
/// zero, with no actions, when nothing was charged. The totals are summed from the actions, not
/// read from the run's charge, which also holds what the run's executor calls reported
/// (`story:executor-reports-usage`): that usage is in its steps, so it is never counted here too.
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
    let actions: Vec<ActionUsage> = charges
        .actions
        .iter()
        .map(|charge| line(&charge.action, &charge.charged))
        .collect();
    let mut totals = EffectsUsage {
        trials: 0,
        input_tokens: 0,
        output_tokens: 0,
        duration_ms: 0,
        cost_micro_usd: Some(0),
        actions: Vec::new(),
    };
    for action in &actions {
        totals.trials = totals.trials.saturating_add(action.trials);
        totals.input_tokens = totals.input_tokens.saturating_add(action.input_tokens);
        totals.output_tokens = totals.output_tokens.saturating_add(action.output_tokens);
        totals.duration_ms = totals.duration_ms.saturating_add(action.duration_ms);
        totals.cost_micro_usd = totals
            .cost_micro_usd
            .zip(action.cost_micro_usd)
            .map(|(sum, cost)| sum.saturating_add(cost));
    }
    EffectsUsage { actions, ..totals }
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
        Self {
            shared: Arc::new(Mutex::new(Ledger {
                prices,
                tallies: vec![Tally::empty()],
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
        let listed: Vec<(usize, &Tally)> = ledger
            .tallies
            .iter()
            .enumerate()
            .filter(|(step, tally)| *step > 0 || tally.calls > 0)
            .collect();
        let steps: Vec<StepUsage> = listed
            .iter()
            .map(|(step, tally)| tally.usage(*step))
            .collect();
        // The totals are the listed steps' sums; a step whose cost is absent leaves the run's
        // absent.
        let mut total = Tally::empty();
        for (_, tally) in &listed {
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
        let last = carried
            .steps
            .iter()
            .filter_map(|step| usize::try_from(step.step).ok())
            .max()
            .unwrap_or(0);
        let mut tallies = vec![Tally::empty(); last + 1];
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
    /// `outside` (0, or for a fork of the ported loop's port, the step it was made in).
    pub(crate) fn step_here(&self, outside: usize) -> usize {
        self.open_here().unwrap_or(outside)
    }

    /// Counts one call in `step` now, before its turn runs, so a turn that fails, unwinds or is
    /// dropped unfinished is still a call. The call is settled with what the turn reported.
    pub(crate) fn begin(&self, step: usize) -> Call {
        {
            let mut ledger = self.ledger();
            let tally = &mut ledger.tallies[step];
            tally.calls = tally.calls.saturating_add(1);
        }
        Call {
            meter: self.clone(),
            step,
            settled: false,
        }
    }

    /// Adds what a counted call's turn reported to its `step`: the model it was served by and its
    /// usage. Without one (a turn that failed, unwound or was dropped, or an answer without a usage
    /// the meter takes) the tokens are unknown, so no price can be stated.
    fn record(&self, step: usize, reported: Option<(&str, &Usage)>) {
        let mut guard = self.ledger();
        let ledger = &mut *guard;
        let card = ledger.prices.as_ref();
        let tally = &mut ledger.tallies[step];
        let Some((model, usage, input, output)) = reported.and_then(|(model, usage)| {
            Some((model, usage, usage.input_tokens?, usage.output_tokens?))
        }) else {
            tally.cost_micro_usd = None;
            return;
        };
        tally.input_tokens = tally.input_tokens.saturating_add(input);
        tally.output_tokens = tally.output_tokens.saturating_add(output);
        tally.cached_input_tokens = tally
            .cached_input_tokens
            .saturating_add(usage.cached_input_tokens.unwrap_or(0));
        tally.cost_micro_usd = match (
            tally.cost_micro_usd,
            card.and_then(|card| card.price(model, usage)),
        ) {
            (Some(sum), Some(cost)) => Some(sum.saturating_add(cost)),
            _ => None,
        };
    }

    /// Adds `usage`, what the executor call of `step` reported it spent
    /// (`story:executor-reports-usage`), to that step: its input and output tokens, and its cost
    /// to the cost of the step's priced calls, so a step that made no model call takes the
    /// reported cost, with or without a rate card. A usage with no cost leaves the step's cost
    /// absent; a negative figure counts nothing. It is no model call, and its wall time is not a
    /// figure of the report.
    fn record_reported(&self, step: usize, usage: &ExecutorUsage) {
        let count = |figure: i64| u64::try_from(figure).unwrap_or(0);
        let mut ledger = self.ledger();
        let tally = &mut ledger.tallies[step];
        tally.input_tokens = tally.input_tokens.saturating_add(count(usage.input_tokens));
        tally.output_tokens = tally
            .output_tokens
            .saturating_add(count(usage.output_tokens));
        tally.cost_micro_usd = tally
            .cost_micro_usd
            .zip(usage.cost_micro_usd.map(count))
            .map(|(sum, cost)| sum.saturating_add(cost));
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
            ledger.tallies.push(Tally::empty());
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

/// A call counted before its turn ran ([`RunMeter::begin`]). Settled, it adds what the turn
/// reported; dropped unsettled (the turn unwound, or its future was dropped unfinished), its
/// tokens, and so its cost, are unknown.
pub(crate) struct Call {
    meter: RunMeter,
    step: usize,
    settled: bool,
}

impl Call {
    /// Adds what the turn reported: the model that served it and its usage, or `None` for a turn
    /// that failed or answered without a usage the meter takes.
    pub(crate) fn settle(mut self, reported: Option<(&str, &Usage)>) {
        self.settled = true;
        self.meter.record(self.step, reported);
    }
}

impl Drop for Call {
    fn drop(&mut self) {
        if !self.settled {
            self.meter.record(self.step, None);
        }
    }
}

/// An llm [`Model`] that counts every `turn` of the model it holds into a [`RunMeter`].
///
/// `M` is how the model is held: the `Arc<dyn Model>` or `Box<dyn Model>` a host built, a
/// reference to one, or an `Arc` of a model of its own. The metered model answers with the inner
/// model's provenance and capabilities, so an answer llm checks against the metered model's
/// binding is checked against the inner one's. Each `turn` is one call of the step of the executor
/// call running on the thread `turn` is called on, else of step 0, counted before the inner turn
/// runs, whichever thread then drives its future.
#[derive(Debug)]
pub struct MeteredModelPort<M> {
    pub(crate) inner: M,
    pub(crate) meter: RunMeter,
}

impl<M> MeteredModelPort<M> {
    /// `inner`, its turns counted into `meter`.
    pub fn new(inner: M, meter: &RunMeter) -> Self {
        Self {
            inner,
            meter: meter.clone(),
        }
    }

    /// The model this one meters.
    pub fn into_inner(self) -> M {
        self.inner
    }
}

impl<M> Model for MeteredModelPort<M>
where
    M: Deref + Send + Sync,
    M::Target: Model,
{
    fn provenance(&self) -> &Provenance {
        self.inner.provenance()
    }

    fn capabilities(&self) -> &Capabilities {
        self.inner.capabilities()
    }

    /// The inner model's turn, counted as one call before it runs. An answer adds its usage when
    /// that usage is final and names its input and output tokens; an error, an unwind or a future
    /// dropped unfinished adds none.
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        sink: &'a mut dyn StreamSink,
        cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        let call = self.meter.begin(self.meter.step_here(0));
        let turn = self.inner.turn(request, sink, cancel);
        Box::pin(async move {
            let outcome = turn.await;
            call.settle(
                outcome
                    .as_ref()
                    .ok()
                    .and_then(|answer| reported(&answer.observation)),
            );
            outcome
        })
    }
}

/// An [`AgentExecutor`] that opens one step of a [`RunMeter`] per call of `E`, and adds to it the
/// usage the call reports through [`AgentExecutor::run_with_usage`].
///
/// Calls of several metered executors on one meter may overlap, each on its own thread, and each
/// counts in its own step. Inside an executor call of the same meter (a `MeteredExecutor` over
/// another) it opens no step of its own: one executor call is one step.
///
/// The meter shows a reported usage only where the runtime can be charged it: a call through
/// `run_with_usage`, which hands the usage on with the outcome. A call through
/// [`AgentExecutor::run`], as a wrapper that does not forward `run_with_usage` makes, hands on the
/// outcome alone, so the usage reaches neither the run's charges nor the meter, and the two agree.
#[derive(Debug)]
pub struct MeteredExecutor<E> {
    inner: E,
    meter: RunMeter,
}

impl<E: AgentExecutor> MeteredExecutor<E> {
    /// `inner`, each of its calls one step of `meter`.
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

impl<E: AgentExecutor> MeteredExecutor<E> {
    /// Opens the step of the executor call beginning on this thread, unless one of this meter is
    /// open here already; returns the step it opened, closed when the guard drops.
    fn open(&self) -> (Option<usize>, Option<OpenStep<'_>>) {
        let opened = self.meter.open_step();
        let guard = opened.map(|step| OpenStep {
            meter: &self.meter,
            step,
        });
        (opened, guard)
    }
}

impl<E: AgentExecutor> AgentExecutor for MeteredExecutor<E> {
    /// `E`'s outcome, unchanged; every model call `E::run` makes is counted in a new step, or in
    /// the step already open on this thread for an executor call of the same meter. It records no
    /// usage the call reports: whoever calls `run` gets the outcome alone, so the runtime is never
    /// charged that usage, and the meter does not show it either.
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let _step = self.open();
        self.inner.run(commission, frontier)
    }

    /// `E`'s step, unchanged; every model call it makes is counted in a new step, or in the step
    /// already open on this thread for an executor call of the same meter. The usage the call
    /// reports, when it reports any, is added to the step it opened
    /// (`story:executor-reports-usage`); inside an executor call of the same meter it opened none,
    /// and the metered executor that opened the step adds it, once.
    fn run_with_usage(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorStep {
        let (opened, _step) = self.open();
        let step = self.inner.run_with_usage(commission, frontier);
        if let (Some(opened), Some(usage)) = (opened, &step.usage) {
            self.meter.record_reported(opened, usage);
        }
        step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> RateCard {
        RateCard::parse(
            r#"{
                "source": "a table the operator read",
                "as_of": "2026-08-24",
                "models": {
                    "m": {
                        "input_usd_per_mtok": 1.25,
                        "cached_input_usd_per_mtok": 0.125,
                        "output_usd_per_mtok": 10.0
                    }
                }
            }"#,
        )
        .expect("a valid card")
    }

    fn usage(input: u64, cached: u64, output: u64) -> Usage {
        Usage {
            input_tokens: Some(input),
            output_tokens: Some(output),
            cached_input_tokens: Some(cached),
            ..Usage::default()
        }
    }

    #[test]
    fn a_rate_becomes_an_exact_count_of_pico_dollars_per_token() {
        assert_eq!(pico_per_token(1.25), Some(1_250_000));
        assert_eq!(pico_per_token(0.0), Some(0));
        assert_eq!(pico_per_token(-1.0), None, "a negative rate is not a rate");
        assert_eq!(pico_per_token(f64::NAN), None);
        assert_eq!(pico_per_token(f64::INFINITY), None);
        assert_eq!(pico_per_token(0.000_000_1), None, "not a whole pico-dollar");
    }

    #[test]
    fn a_million_tokens_costs_exactly_what_the_card_says_a_million_tokens_costs() {
        assert_eq!(card().price("m", &usage(1_000_000, 0, 0)), Some(1_250_000));
        assert_eq!(card().price("m", &usage(0, 0, 1_000_000)), Some(10_000_000));
    }

    #[test]
    fn cached_tokens_are_billed_once_and_at_the_cache_rate() {
        let priced = card()
            .price("m", &usage(1_000_000, 800_000, 0))
            .expect("priced");
        assert_eq!(priced, 350_000, "$0.35, not $1.35");
    }

    #[test]
    fn an_unnamed_cached_count_is_none() {
        let reported = Usage {
            cached_input_tokens: None,
            ..usage(1_000_000, 0, 0)
        };
        assert_eq!(card().price("m", &reported), Some(1_250_000));
    }

    #[test]
    fn cache_creation_tokens_require_and_use_their_own_declared_rate() {
        let reported = Usage {
            cache_creation_input_tokens: Some(400_000),
            ..usage(1_000_000, 100_000, 0)
        };
        assert_eq!(
            card().price("m", &reported),
            None,
            "a distinct billed class with no declared rate is not charged as ordinary input"
        );
        let card = RateCard::parse(
            r#"{
                "source": "a table the operator read",
                "as_of": "2026-08-24",
                "models": {"m": {
                    "input_usd_per_mtok": 1.0,
                    "cached_input_usd_per_mtok": 0.1,
                    "cache_creation_input_usd_per_mtok": 2.0,
                    "output_usd_per_mtok": 10.0
                }}
            }"#,
        )
        .expect("a card with a cache-creation rate");
        assert_eq!(card.price("m", &reported), Some(500_000 + 10_000 + 800_000));
    }

    #[test]
    fn a_provider_reporting_more_cache_than_input_bills_the_cache_and_nothing_negative() {
        assert_eq!(
            card().price("m", &usage(10, 99, 0)),
            Some(12),
            "99 * $0.125/Mtok"
        );
        assert_eq!(
            card().price("m", &usage(0, 0, 0)),
            Some(0),
            "a turn with no tokens"
        );
    }

    #[test]
    fn a_turn_without_its_input_or_output_tokens_is_absent_rather_than_free() {
        for reported in [
            Usage {
                input_tokens: None,
                ..usage(100, 0, 10)
            },
            Usage {
                output_tokens: None,
                ..usage(100, 0, 10)
            },
        ] {
            assert_eq!(card().price("m", &reported), None, "{reported:?}");
        }
    }

    #[test]
    fn a_model_the_card_does_not_price_is_absent_rather_than_free() {
        assert_eq!(
            card().price("somebody-elses-model", &usage(1_000_000, 0, 1_000_000)),
            None
        );
    }

    #[test]
    fn a_card_with_no_source_or_date_is_refused() {
        assert_eq!(
            RateCard::parse(r#"{"source": "  ", "as_of": "2026-08-24", "models": {}}"#),
            Err(RateCardError::MissingSource)
        );
        assert_eq!(
            RateCard::parse(r#"{"source": "s", "as_of": "last tuesday", "models": {}}"#),
            Err(RateCardError::BadDate("last tuesday".to_owned()))
        );
        assert!(is_iso_date("2026-08-24"));
        assert!(!is_iso_date("2026-8-24"));
    }

    #[test]
    fn one_bad_rate_refuses_the_whole_card_rather_than_pricing_the_rest() {
        for (field, rates) in [
            (
                "input_usd_per_mtok",
                serde_json::json!({
                    "input_usd_per_mtok": -1.0,
                    "cached_input_usd_per_mtok": 0.0,
                    "output_usd_per_mtok": 0.0,
                }),
            ),
            (
                "cache_creation_input_usd_per_mtok",
                serde_json::json!({
                    "input_usd_per_mtok": 1.0,
                    "cached_input_usd_per_mtok": 0.0,
                    "cache_creation_input_usd_per_mtok": -2.0,
                    "output_usd_per_mtok": 0.0,
                }),
            ),
        ] {
            let card = serde_json::json!({
                "source": "s",
                "as_of": "2026-08-24",
                "models": {"m": rates},
            });
            let refused = RateCard::parse(&card.to_string()).expect_err("refused");
            let RateCardError::BadRate {
                model,
                field: named,
                ..
            } = &refused
            else {
                panic!("not a bad rate: {refused:?}");
            };
            assert_eq!((model.as_str(), *named), ("m", field), "{refused:?}");
        }
    }

    #[test]
    fn an_unknown_field_is_refused_so_a_typo_is_not_a_silent_zero() {
        let refused = RateCard::parse(
            r#"{"source": "s", "as_of": "2026-08-24", "models": {"m": {
                "input_usd_per_mtok": 1.0,
                "cached_input_usd_per_mtok": 0.0,
                "output_usd_per_mtok": 1.0,
                "reasoning_usd_per_mtok": 5.0
            }}}"#,
        )
        .expect_err("refused");
        assert!(
            matches!(refused, RateCardError::Unreadable(_)),
            "{refused:?}"
        );
    }

    #[test]
    fn the_card_lists_what_it_prices() {
        assert_eq!(card().priced_models().collect::<Vec<_>>(), vec!["m"]);
    }

    #[test]
    fn rounding_lands_on_the_nearer_micro_dollar() {
        assert_eq!(to_micro(1_499_999), 1);
        assert_eq!(to_micro(1_500_000), 2);
    }
}
