---
format: aep.planning-md/3
id: epic:commission-core
kind: epic
status: implemented
title: 'Commission core: responsibility contracts over fakes'
refs:
- provider: commission
  reference: epic:commission-core
relations:
- serves: vision:governed-autonomy
- serves: vision:O1
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T10:40:12Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-08T10:40:12Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-08T10:40:12Z", actor: "human:timo", revision: 4}
---
> Re-filed from `beyond10x/commission` `epic:commission-core` at `e61e4f0` (status there: `active`) under Atlas ADR 0090
> (loom `story:import-commission`). Paths below are Commission's: `ess/` is now `ess/commission/`,
> `docs/` is now `docs/commission/`.

## Outcome

The responsibility model and runtime contracts, led by the ESS specification in `ess/`. Covers
TASKBOARD M-001 … M-010 and I-007 (the Commission ESS specification).

## Acceptance

With a fake governor and a fake executor, a Commission loads a case, obtains a frontier, invokes the
executor, refuses a proposed action absent from the current frontier or proposed against a stale case
revision, and represents blocked, suspended and completed outcomes; `cargo tree` shows no
model-provider crate; `task check` runs the Commission ESS conformance suite.

## Rule

Model types come from `ess generate synthesize`, not hand transcription (ess:specifying). The
bootstrap types in `crates/commission` are replaced by generated ones as the stories land.

## Source

Atlas `epic:ga-commission-core`; Atlas ADRs 0070, 0075; `docs/contracts/`.
