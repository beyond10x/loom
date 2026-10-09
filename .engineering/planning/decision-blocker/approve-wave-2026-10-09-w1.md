---
format: aep.planning-md/3
id: decision-blocker:approve-wave-2026-10-09-w1
kind: decision-blocker
status: cleared
title: Approve wave 2026-10-09-w1 (selectors, compaction bound, governor comments)
revision: 2
transitions:
- {from: "open", to: "cleared", at: "2026-10-08T23:16:39Z", actor: "human:timo", revision: 2}
---
## Question

Approve wave 2026-10-09-w1: five units, each on its own `impl/<story>` branch merged into
`wave/2026-10-09-w1`, one integration branch and one pull request.

1. `story:reasoning-model-selector`
2. `story:confidence-fallback`, after unit 1 (both edit `crates/loom-executor/src/selection.rs`)
3. `story:laya-selector` (new crate `crates/loom-selector-laya`)
4. `story:compaction-target-bound`
5. `story:stale-governor-comments`

Adversary passes on units 2 and 4 (selection authority; the context bound). Recording a fallback as
two linked selections moves to `story:fallback-selection-recording`; `story:laya-arguments-slice`
follows in a later wave.

## Build

One joint package gate on the integration tree (executor, governor, selector-laya, and sdk,
intake-slice and conformance for the new `LoopStop` variant). Units build one at a time with
`CARGO_INCREMENTAL=0`; each unit tree is finished with its build cache removed before the gate.

## Decided

Approved 2026-10-09: all five units, with the compaction outcome of
`decision-blocker:compaction-target-outcome` (option C) and the Laya transport of
`decision-blocker:laya-transport` (option A). Builds start once `/` has at least 40G free.
