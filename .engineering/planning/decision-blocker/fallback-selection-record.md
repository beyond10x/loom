---
format: aep.planning-md/3
id: decision-blocker:fallback-selection-record
kind: decision-blocker
status: cleared
title: Nobody has decided whether a fallback records one selection or two linked ones
relations:
- blocks: epic:fast-selector
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-07T00:06:18Z", actor: "human:timo", revision: 3, executor: "agent:loom"}
---
## Question

When the confidence fallback replaces a fast selection with the stronger selector's choice, is that
recorded as one `loom.run.Selection` (strategy `Hybrid`) or as two selections, the fast one linked to
the one that replaced it? If two, which owns which, and may the fast one exist with no successor?

## Why it is open

`ess/domains/run.yaml` declares `loom.run.Selection` with a `strategy` field and the variants
`ReasoningModel`, `FastTyped`, `Rule`, `Hybrid`, and no relation from one selection to another. Atlas
ADR 0073 § Fallback says only "uncertain → reasoning-model selector/planner". Nothing states the
cardinality (one-to-zero-or-one, or none at all) or the ownership.

## What it stops

Recording a fallback so Metaharness can measure how often and why fast selection was overruled
(`docs/contracts/loom-action-selection.md` safety rule 7). The fallback behaviour itself is drafted in
`story:confidence-fallback`, whose acceptance checks only the returned choice and does not depend on
this answer.

## Source

Decomposition of `epic:fast-selector`; `ess/domains/run.yaml`; Atlas ADR 0073.

## Decision (2026-10-07)

Option B. A fallback records two `loom.run.Selection`s: the fast selection and the stronger
selector's selection that replaced it. The fast one references its replacement, zero or one (a fast
selection that was not overruled has none); both belong to the run's turn as every selection does.
`ess/domains/run.yaml` gains that relation before the recording is implemented, so Metaharness can
count how often and why fast selection was overruled (`docs/contracts/loom-action-selection.md`
safety rule 7).

Option A (one `Hybrid` selection) loses the overruled pick; option C keeps it outside the run model.
