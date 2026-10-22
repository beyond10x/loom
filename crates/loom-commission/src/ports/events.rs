//! The event port: where a host receives every [`RuntimeEvent`] of a run as it happens
//! (`story:runtime-events`).
//!
//! The host hands the runtime its sink through [`LoopContext::events`]
//! ([`crate::runtime::LoopContext`]); the default is [`NoEvents`], which receives nothing, and the
//! runtime builds no event for it. The events, and when each is emitted, are the
//! `RuntimeEvent` declaration of `ess/commission/domains/responsibility.yaml`: a step started, an
//! action selected, an action request proposed and revalidated, an authority decided, an effect
//! invoked, a session closed, and the loop stopped, last and exactly once for every Run the loop
//! started or resumed. Each names the Run.
//!
//! The runtime hands each event to the sink and reads nothing back. An event is never evidence and
//! never an observation, and the sink cannot change the loop: [`EventSink::emit`] returns nothing,
//! and the runtime decides every step as it would without a sink. A sink that panics is the
//! host's: the runtime catches nothing, so the panic unwinds out of the loop as any port's does.
//!
//! A session's own records (`metaharness.event/1`) stay metaharness's (ADR 0096 point 7):
//! `SessionClosed` names a session by the transcript digest its executor step states and carries
//! its report, and never re-emits the session's events.
//!
//! [`LoopContext::events`]: crate::runtime::LoopContext::events

use crate::model::responsibility::RuntimeEvent;

/// Receives a run's events as they happen, in the order the loop makes them.
pub trait EventSink {
    /// Receives `event`. The loop goes on as it would without the sink.
    fn emit(&mut self, event: &RuntimeEvent);

    /// Whether the sink receives events at all. The runtime builds no event for a sink that
    /// answers `false`, so a sink that wants none costs nothing; the default answers `true`.
    fn receives(&self) -> bool {
        true
    }
}

/// The sink that receives nothing, the default of [`crate::runtime::LoopContext::events`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoEvents;

impl EventSink for NoEvents {
    fn emit(&mut self, _event: &RuntimeEvent) {}

    fn receives(&self) -> bool {
        false
    }
}

/// A host's own list is a sink: it keeps every event, in order.
impl EventSink for Vec<RuntimeEvent> {
    fn emit(&mut self, event: &RuntimeEvent) {
        self.push(event.clone());
    }
}
