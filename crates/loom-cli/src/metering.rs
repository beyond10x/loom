//! Per-item model usage for a served agent's dashboard. The A2A and channel workers
//! each drive their own synchronous turn. Capture the meter at model dispatch so
//! completion of a future always charges the item that started the call.
use std::cell::RefCell;
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
    ACTIVE.with(|slot| slot.replace(Some((item.to_owned(), Arc::default()))));
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
    (
        Some(t.calls),
        t.input.complete(t.calls),
        t.output.complete(t.calls),
        t.cost.complete(t.calls),
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
