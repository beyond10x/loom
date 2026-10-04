---
format: aep.planning-md/3
id: epic:vertical-slices
kind: epic
status: draft
title: 'Vertical slices: software change, incident, suspend/resume'
refs:
- provider: commission
  reference: epic:vertical-slices
relations:
- serves: vision:governed-autonomy
- serves: vision:O1
revision: 1
---
> Re-filed from `beyond10x/commission` `epic:vertical-slices` at `e61e4f0` (status there: `proposed`) under Atlas ADR 0090
> (loom `story:import-commission`). Paths below are Commission's: `ess/` is now `ess/commission/`,
> `docs/` is now `docs/commission/`.

## Outcome

The end-to-end demonstrators, as integration tests under `tests/`. Covers TASKBOARD I-001 … I-003.

## Acceptance

As Atlas `epic:ga-vertical-slices` states it, per slice: I-001 stale R1 evidence, blocked merge,
projected actions changing after R2 tests, authority outside the model, stale-proposal refusal, no
unprojected action executed; I-002 incident leaves emergency mode with cause `UNKNOWN`; I-003
suspend at merge approval and resume after a process restart at the same case revision.

## Source

Atlas `epic:ga-vertical-slices`; build pack `START-HERE.md`.
