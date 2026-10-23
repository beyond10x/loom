---
format: aep.planning-md/3
id: upstream-blocker:ess-0-69-existing-selection-guard
kind: upstream-blocker
status: open
title: ESS 0.69.0 synthesis reaches a guard on an existing selection id
relations:
- blocks: story:refused-selection-stays-refused
revision: 1
---
## Waiting on

ESS 0.69.0, which the orchestrator says ships G-U34/G-U35 (2026-10-24): synthesis that reaches a
`loom.run.SelectAction` outcome guarding a selection id the run already holds, with both catalogue
guards (`not-in-catalogue`, `revision-mismatch`) kept. ESS 0.67.0 and 0.68.0 refuse it with
ESS-SYNTH-004 (`decision-blocker:ess-existing-selection-guard`, § What was observed).

## Clears when

On ESS 0.69.0, a copy of `ess/` with the `selection-exists` outcome synthesizes with 0 refusals and
`task ess-gate` exits 0, so `story:refused-selection-stays-refused` can make its `ess/` commit.
