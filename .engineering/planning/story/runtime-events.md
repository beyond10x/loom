---
format: aep.planning-md/3
id: story:runtime-events
kind: story
status: draft
title: A host receives every RuntimeEvent of a run through an EventSink
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A host passes an `EventSink` and receives every `RuntimeEvent` of a run as it happens: step
started, action selected, proposed, authority decided, effect invoked, session closed (naming the
session and its report) and stopped. `NoEvents` is the default and costs nothing.

## Why

A host observes a run today only from stores after it ends; Agency would poll. Decided by the
orchestrator, Q4 A. `metaharness.event/1` records stay metaharness's (ADR 0096 point 7):
`RuntimeEvent` references a session and never re-emits its events.

## Acceptance

- `crates/loom-commission-testkit/tests/runtime_events.rs`
  (`every_step_of_a_run_is_emitted_in_order`): a recorded run emits each event once, in loop
  order, `Stopped` last and exactly once.
- (`a_sink_sees_no_event_for_a_step_that_did_not_happen`): a refused proposal emits `Proposed` and
  `AuthorityDecided`, never `EffectInvoked`.
- An event is never evidence and never changes the loop; a panicking sink is the host's.
- `crates/loom-sdk/tests/host_ports.rs` passes with `NoEvents`.

## ESS first

`commission` declares `RuntimeEvent` (the conformance codec reads it); `task commission:drift` is
red first.

## Not in scope

The `run --output jsonl` writer, which keeps its own V1 contract.

## Scope

Crates: `loom-commission`, `loom-commission-testkit`, `loom-sdk`, `ess/commission/`.
`crates/loom-commission/src/runtime.rs` (shared with `story:loop-context-split`: not in one wave).

## Ready

Ready now.
