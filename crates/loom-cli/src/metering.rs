//! Per-item model usage for a served agent's dashboard. The A2A and channel workers
//! each drive their own synchronous turn. Capture the meter at model dispatch so
//! completion of a future always charges the item that started the call.
use loom_sdk::commission::model::responsibility::{ExecutorUsage, RuntimeEvent};
use loom_sdk::{EventSink, session::SessionExecutor};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use llm_core::{
    BoxFuture, Cancel, Capabilities, Error, Model, Provenance, StreamSink, TurnOutcome,
    TurnRequest, Usage,
};
use loom_sdk::loom::RateCard;

#[derive(Default)]
struct Total {
    amount: u64,
    measured: u64,
}

impl Total {
    fn add(&mut self, value: Option<u64>) {
        if let Some(value) = value {
            self.amount = self.amount.saturating_add(value);
            self.measured += 1;
        }
    }

    fn complete(&self, calls: u64) -> Option<u64> {
        (self.measured == calls).then_some(self.amount)
    }
}

#[derive(Default)]
struct Tally {
    external: bool,
    sessions: u64,
    calls: u64,
    input: Total,
    output: Total,
    cost: Total,
}

type Active = (String, Arc<Mutex<Tally>>);
thread_local! {
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
}

/// Begin metering one item on its worker, before classification.
pub fn begin(item: &str) {
    start(item, false);
}

/// External harnesses report usage per session; their underlying provider-call count is unknown.
pub fn begin_external(item: &str) {
    start(item, true);
}

fn start(item: &str, external: bool) {
    ACTIVE.with(|slot| {
        slot.replace(Some((
            item.to_owned(),
            Arc::new(Mutex::new(Tally {
                external,
                ..Tally::default()
            })),
        )))
    });
}

/// Model calls, input tokens, output tokens and cost in millionths of a US dollar.
/// A missing measurement in any call leaves that total absent.
pub type Totals = (Option<u64>, Option<u64>, Option<u64>, Option<u64>);

/// Finish the matching item. A turn outside observation carries no usage evidence.
#[must_use]
pub fn end(item: &str) -> Totals {
    let tally = ACTIVE.with(|slot| slot.borrow_mut().take_if(|(id, _)| id == item));
    let Some((_, tally)) = tally else {
        return (None, None, None, None);
    };
    let t = tally
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if t.external && t.sessions == 0 {
        return (None, None, None, None);
    }
    let samples = if t.external { t.sessions } else { t.calls };
    (
        (!t.external).then_some(t.calls),
        t.input.complete(samples),
        t.output.complete(samples),
        t.cost.complete(samples),
    )
}

#[derive(Clone, Copy, Default)]
struct Reported {
    input: bool,
    output: bool,
    cost: bool,
}

struct SessionEvents<'a> {
    item: &'a str,
    reported: Rc<Cell<Reported>>,
    caller: &'a mut dyn EventSink,
}

fn session_event(item: &str, started: bool, usage: Option<&ExecutorUsage>, reported: Reported) {
    ACTIVE.with(|slot| {
        let active = slot.borrow();
        let Some((_, tally)) = active.as_ref().filter(|(id, _)| id == item) else {
            return;
        };
        let mut t = tally
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !t.external {
            return;
        }
        if started {
            t.sessions += 1;
            return;
        }
        let Some(usage) = usage else {
            return;
        };
        let nonnegative = |value| u64::try_from(value).ok();
        if reported.input {
            t.input.add(nonnegative(usage.input_tokens));
        }
        if reported.output {
            t.output.add(nonnegative(usage.output_tokens));
        }
        t.cost.add(if reported.cost {
            usage.cost_micro_usd.and_then(nonnegative)
        } else {
            None
        });
    });
}

impl EventSink for SessionEvents<'_> {
    fn emit(&mut self, event: &RuntimeEvent) {
        match event {
            RuntimeEvent::StepStarted(step) if step.frontier_id.is_some() => {
                self.reported.set(Reported::default());
                session_event(self.item, true, None, Reported::default());
            }
            RuntimeEvent::SessionClosed(closed) => session_event(
                self.item,
                false,
                closed.usage.as_ref(),
                self.reported.take(),
            ),
            _ => {}
        }
        self.caller.emit(event);
    }
}

/// Observe only the presence of vendor-reported figures, then take their amounts from the
/// corresponding SessionClosed event. Its budget accounting maps missing token figures to zero.
pub fn observe_session<'a>(
    executor: SessionExecutor,
    item: &'a str,
    caller: &'a mut dyn EventSink,
) -> (SessionExecutor, impl EventSink + 'a) {
    let presence = Rc::new(Cell::new(Reported::default()));
    let seen = Rc::clone(&presence);
    let executor = executor.with_observer(move |line, _| {
        if let loom_sdk::session::metaharness::protocol::Event::SessionEnded {
            usage,
            total_cost_usd,
            ..
        } = &line.event
        {
            seen.set(Reported {
                input: usage.as_ref().is_some_and(|u| u.input_tokens.is_some()),
                output: usage.as_ref().is_some_and(|u| u.output_tokens.is_some()),
                cost: total_cost_usd.is_some(),
            });
        }
    });
    (
        executor,
        SessionEvents {
            item,
            reported: presence,
            caller,
        },
    )
}

/// Optional operator-supplied pricing in the existing Loom rate-card format.
///
/// # Errors
/// Returns the read or parse failure for a configured rate card.
pub fn prices() -> Result<Option<Arc<RateCard>>, String> {
    let Some(path) = std::env::var_os("LOOM_DASHBOARD_RATE_CARD") else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read dashboard rate card: {e}"))?;
    RateCard::parse(&text)
        .map(|v| Some(Arc::new(v)))
        .map_err(|e| e.to_string())
}

/// Wrap a serving model. The caller shares it between workers and uses identical
/// provenance and capabilities for request validation.
pub fn observe(inner: Arc<dyn Model>, card: Option<Arc<RateCard>>) -> Arc<dyn Model> {
    Arc::new(Observed { inner, card })
}

struct Observed {
    inner: Arc<dyn Model>,
    card: Option<Arc<RateCard>>,
}

impl Model for Observed {
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
        let tally = ACTIVE.with(|s| s.borrow().as_ref().map(|(_, t)| Arc::clone(t)));
        let Some(tally) = tally else {
            return self.inner.turn(request, sink, cancel);
        };
        tally
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .calls += 1;
        let future = self.inner.turn(request, sink, cancel);
        Box::pin(async move {
            let answer = future.await;
            if let Ok(outcome) = &answer
                && outcome.observation.final_usage
                && let Some(usage) = &outcome.observation.usage
            {
                let Usage {
                    input_tokens,
                    output_tokens,
                    ..
                } = usage;
                let model = outcome.observation.binding.model.as_str();
                let mut t = tally
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                t.input.add(*input_tokens);
                t.output.add(*output_tokens);
                t.cost
                    .add(self.card.as_ref().and_then(|c| c.price(model, usage)));
            }
            answer
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_steps_sum_reported_fields_only_and_never_invent_provider_calls() {
        let item = "a2a:session-usage";
        let start_step = || session_event(item, true, None, Reported::default());
        let usage = ExecutorUsage {
            input_tokens: 0,
            output_tokens: 7,
            duration_ms: 90,
            cost_micro_usd: Some(123),
        };
        begin_external(item);
        assert_eq!(end(item), (None, None, None, None));
        begin_external(item);
        start_step();
        session_event(
            item,
            false,
            Some(&usage),
            Reported {
                input: true,
                output: true,
                cost: true,
            },
        );
        assert_eq!(end(item), (None, Some(0), Some(7), Some(123)));
        begin_external(item);
        start_step();
        session_event(
            item,
            false,
            Some(&usage),
            Reported {
                input: true,
                output: true,
                cost: true,
            },
        );
        start_step();
        session_event(
            item,
            false,
            Some(&usage),
            Reported {
                input: false,
                output: true,
                cost: false,
            },
        );
        assert_eq!(end(item), (None, None, Some(14), None));
        begin_external(item);
        start_step();
        assert_eq!(end(item), (None, None, None, None));
    }
}
