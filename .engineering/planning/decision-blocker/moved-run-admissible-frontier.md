---
format: aep.planning-md/3
id: decision-blocker:moved-run-admissible-frontier
kind: decision-blocker
status: cleared
title: Nobody has decided which outcome ends a run whose case moved to a frontier that admits an action
relations:
- blocks: story:moved-run-named-outcome
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-07T08:12:59Z", actor: "human:timo", revision: 3, executor: "agent:loom"}
---
## Question

When a run's case moved to a revision whose current frontier admits an action, which outcome ends
the run?

## Why it is open

The decision of 2026-10-07 on `decision-blocker:run-stale-outcome` (option C) lets an executor
report the move and the runtime judge the run on the current frontier. That settles a complete case
(`Completed`) and an open obligation (`NeedsExternalEvidence`). It does not settle a frontier that
admits an action: the Run is bound to the revision it left, so it cannot go on, and today it ends
`NoAdmissibleAction`, which reads the same as an empty frontier.

## Options

- A: a `RunOutcome` variant for a run whose case moved on (naming the run's and the current
  revision), as option A of `decision-blocker:run-stale-outcome` proposed; the caller starts a new
  Run at the current revision.
- B: the Run goes on at the new revision (its bound revision moves); changes what a Run is bound to.
- C: keep `NoAdmissibleAction` and document it.

## What it stops

`story:moved-run-named-outcome`.

## Decision (2026-10-07)

Option A. A new `RunOutcome` variant ends a run whose case moved to a revision whose frontier still
admits an action; it names the revision the Run was bound to and the current one. The caller starts
a new Run at the current revision; a Run stays bound to one revision (option B, moving that bound,
is not taken; option C, keeping `NoAdmissibleAction`, would leave two different endings
indistinguishable to a caller).

The variant is declared in `ess/commission/` first, in its own commit, validated with the newest
`ess`, then regenerated and implemented (`story:moved-run-named-outcome`). The release that carries
it names the new variant in its notes, for callers that match `RunOutcome`.
