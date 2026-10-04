---
format: aep.planning-md/3
id: decision-blocker:catalogue-ownership
kind: decision-blocker
status: cleared
title: Nobody has decided whether a Loom action catalogue belongs to a turn or to the session
relations:
- blocks: epic:loom-native-harness
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-04T00:59:17Z", actor: "human:timo", revision: 3}
---
## Question

Does a Loom action catalogue belong to a turn (re-projected before every model turn) or to the
session (re-projected only when the frontier case revision changes)?

## Why it is open

`ess/domains/run.yaml:81` marks it UNMAPPED and names no story to decide it. No ess/1 document
declares the relation. The sources point both ways: Harness refreshes a fail-closed tool inventory
before every turn (`harness-loop/src/environment.rs:47`, at `798325f0`) and also freezes its
catalogue before turn one; Atlas ADR 0072 requires a catalogue derived from the current frontier
but does not say when it is recomputed. The answer sets the ownership, cardinality and lifecycle of
`loom.run.ActionCatalogue` against `loom.run.Session` and `loom.run.Turn`.

## What it stops

Closing the marker at `ess/domains/run.yaml:81`. `story:frontier-projection` projects a catalogue
from a given frontier and declares no owner relation, so it is not stopped; the epic cannot close
with the marker open unless this blocker still holds it (`story:loom-ess-conformance`).

## Source

Decomposition of `epic:loom-native-harness`; `ess/domains/run.yaml`; Atlas ADR 0072.

## Decision (operator, 2026-10-04)

One catalogue per turn; modelled by story:ess-hard-gate.

In `ess/` this is `loom.run.ActionCatalogue.turn_id: loom.run.TurnId` and the relation `catalogue`
on `loom.run.Turn` (owns, cardinality one, via `turn_id`); the marker at `ess/domains/run.yaml:81`
is deleted by the same story.
