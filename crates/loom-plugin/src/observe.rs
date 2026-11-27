//! Read-only, optional observation at the accepted-item boundary.
use crate::RecordLine;
use loom_sdk::EventSink;
use loom_sdk::commission::model::responsibility::RuntimeEvent;

/// A host's observer receives only the work that actually ran, after replay checks.
pub trait TurnObserver: Send + Sync {
    /// The item is about to be classified.
    fn started(&self, item: &str);
    /// A runtime event occurred while processing the item.
    fn event(&self, item: &str, event: &RuntimeEvent);
    /// The attempt ended, optionally with a durably appended record. A retryable failure
    /// ends an attempt without declaring the item terminal.
    fn finished(&self, item: &str, record: Option<&RecordLine>, failed: bool);
}

/// Balances the start notification on every exit, including retry and storage failures.
pub struct Attempt<'a> {
    observer: Option<&'a dyn TurnObserver>,
    item: &'a str,
}

impl<'a> Attempt<'a> {
    /// Begin observing one attempt.
    pub fn start(observer: Option<&'a dyn TurnObserver>, item: &'a str) -> Self {
        if let Some(observer) = observer {
            observer.started(item);
        }
        Self { observer, item }
    }

    /// Report a durable record or a terminal failure with no record.
    pub fn finish(mut self, record: Option<&RecordLine>, failed: bool) {
        if let Some(observer) = self.observer.take() {
            observer.finished(self.item, record, failed);
        }
    }
}

impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        if let Some(observer) = self.observer {
            observer.finished(self.item, None, false);
        }
    }
}

/// Fan-out preserving the caller's original event delivery.
pub struct Events<'a> {
    observer: Option<&'a dyn TurnObserver>,
    item: &'a str,
    caller: Option<&'a mut dyn EventSink>,
}

impl<'a> Events<'a> {
    /// A sink forwarding to the optional host observer and requesting client.
    pub fn new(
        observer: Option<&'a dyn TurnObserver>,
        item: &'a str,
        caller: Option<&'a mut dyn EventSink>,
    ) -> Self {
        Self {
            observer,
            item,
            caller,
        }
    }
}

impl EventSink for Events<'_> {
    fn emit(&mut self, event: &RuntimeEvent) {
        if let Some(observer) = self.observer {
            observer.event(self.item, event);
        }
        if let Some(caller) = self.caller.as_deref_mut() {
            caller.emit(event);
        }
    }

    fn receives(&self) -> bool {
        self.observer.is_some() || self.caller.as_deref().is_some_and(EventSink::receives)
    }
}
