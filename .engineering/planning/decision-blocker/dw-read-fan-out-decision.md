---
format: aep.planning-md/3
id: decision-blocker:dw-read-fan-out-decision
kind: decision-blocker
status: cleared
title: Nobody has decided whether Commission's runtime may perform admitted reads concurrently
relations:
- blocks: story:dw-read-fanout
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-14T12:20:51Z", actor: "human:timo", revision: 3}
---
## Question

May Commission's runtime perform several admitted reads of one executor call concurrently, which
changes its loop contract, and is that recorded as an architecture decision before
story:dw-read-fanout is built?

Today one runtime iteration takes one `ProposedAction`, makes one `ActionRequest` and calls
`EffectPort::invoke` once (`crates/loom-commission/src/runtime.rs`, module docs items 6 and 8).
story:dw-read-fanout adds `ExecutorOutcome::ProposedReads`, an `EffectPort::reads` method, and
concurrent invocation of up to 16 admitted reads per iteration. `ExecutorOutcome` is matched by
hosts, so the new variant is a break for an exhaustive match.

## Options

- A. Record an architecture decision `read-fan-out` (proposed, then accepted by the operator) with
  the story's Design as its decision: reads only, no authority, each revalidated and invoked once,
  recorded in proposal order, one step for the budget and the meter. Build the story in the next
  wave. Cost: one decision round; the incident workflow waits for it.
- B. Build it as a story without an architecture decision. Cost: a change to the runtime loop
  contract lands without the record every earlier contract change has.
- C. No fan-out in Commission: the incident workflow reads its sources one per iteration, with no
  model call between them. Cost: the epic's "collect concurrently" goal is dropped; latency grows
  with the number of sources.

Recommendation: A.

## Stops

story:dw-read-fanout, and through it story:dw-incident-workflow.

## Decision (2026-10-14)

A. The operator accepted architecture-decision-record:read-fan-out. story:dw-read-fanout is built in the next wave, as its Design says.
