---
format: aep.planning-md/3
id: decision-blocker:selection-telemetry-record
kind: decision-blocker
status: cleared
title: Nobody has decided what a selection telemetry record is attached to
relations:
- blocks: epic:fast-selector
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-04T01:09:52Z", actor: "human:timo", revision: 3}
---
## Question

What is a selection telemetry record attached to — one per `loom.run.Selection`, per turn, or per
session — and does it also record refusals at the execution boundary, which happen after selection
and outside the selector? Who consumes it (Metaharness only), and in what form?

## Why it is open

`epic:fast-selector` accepts on "unauthorized-action attempts at the execution boundary stay at 0 in
the selection telemetry", and `docs/contracts/loom-action-selection.md` safety rule 7 says telemetry
"should be available to Metaharness". No ess/1 document declares a telemetry record or its relation
to `loom.run.Selection`, `loom.run.Session` or the boundary refusal. Atlas ADR 0074 adds that a trace
is not evidence, so the record must not become evidence by default.

## What it stops

Selection telemetry, and with it the telemetry clause of the epic acceptance.
`story:laya-arguments-slice` demonstrates the slice by test assertions instead and says so.

## Source

Decomposition of `epic:fast-selector`; `docs/contracts/loom-action-selection.md`; Atlas ADRs 0073, 0074.

## Decision (coordinator, 2026-10-04)

One `SelectionRecord` per `loom.run.Selection` (the selection owns it, cardinality one): strategy,
number of candidates, chosen action id, confidence, whether it fell back and to which strategy,
selection latency in milliseconds, input and output tokens spent on selection. Refusals at the
execution boundary are counted on the `loom.run.Session` (`boundary_refusals`), because they happen
after selection. Metaharness reads both from Loom's session record. Neither is evidence (Atlas
ADR 0074). Built by story:selection-telemetry.
