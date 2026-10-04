---
format: aep.planning-md/3
id: decision-blocker:run-stale-outcome
kind: decision-blocker
status: open
title: No run outcome says the case moved to another revision
refs:
- provider: commission
  reference: decision-blocker:run-stale-outcome
relations:
- blocks: story:effect-invocation
revision: 1
---
> Re-filed from `beyond10x/commission` `decision-blocker:run-stale-outcome` at `e61e4f0` (status there: `open`) under Atlas ADR 0090
> (loom `story:import-commission`). Paths below are Commission's: `ess/` is now `ess/commission/`,
> `docs/` is now `docs/commission/`.

## Question

Which `RunOutcome` does a run end with when its case moves to another revision?

## Context

A Run is bound to one case revision (`ess/domains/responsibility.yaml:386,390`). The local runtime
loop (story:local-runtime-loop, wave 2026-10-04-w7, adversary pass 1 F1) ends the run when the
governor reports another revision, and for now answers `NoAdmissibleAction`. That reads the same as an
empty frontier, and no `SuspensionReason` names a revision change.

## Options

- A: a `RunOutcome` variant for a stale run, carrying the run's and the current case revision.
- B: a `SuspensionReason` variant for a moved case, so the run ends `Suspended` and can be resumed
  or replaced.

Either is a specification change, made first in its own commit (Atlas ADR 0080). Together with the
missing terminal Run state (story:local-runtime-loop, J2), it decides how an effect-invoking loop
reports a case that moved under it.
