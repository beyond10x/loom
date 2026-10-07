---
format: aep.planning-md/3
id: decision-blocker:run-stale-outcome
kind: decision-blocker
status: cleared
title: No run outcome says the case moved to another revision
refs:
- provider: commission
  reference: decision-blocker:run-stale-outcome
relations:
- blocks: story:effect-invocation
revision: 5
transitions:
- {from: "open", to: "cleared", at: "2026-10-07T00:06:18Z", actor: "human:timo", revision: 5, executor: "agent:loom"}
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

## From wave 2026-10-06-w1

Adversary pass 1 of wave 2026-10-06-w1 (`review-result:adversary-w1-loom-selection-revalidation-pass-1`,
finding F1) measured a second route to the same gap. A Loom built with `Loom::with_governor` refuses
a selection made at a revision the case has since left (`stale-revision`) and returns
`NoUsefulAction`, since no `ExecutorOutcome` says the case moved. Commission's `run_until_blocked`
then derives the run outcome from the frontier the case left (`crates/loom-commission/src/runtime.rs`,
the derive after a non-proposal) instead of reloading as it does for a stale proposal (item 8 of its
module documentation). Measured in `crates/loom-executor/tests/adversary_w1_runtime_stale.rs` and
`adversary_w1p2_runtime_windows.rs`: a case completed while the selector selects or while arguments
are generated ends `NoAdmissibleAction` instead of `Completed`, and a case that moved past an open
obligation ends `NeedsExternalEvidence(["tests-pass"])` for the superseded obligation. The cases
assert today's outcome and name this blocker. No caller in this repository composes
`with_governor` with `run_until_blocked` yet; the SDK re-exports both.

Pass 2 (`review-result:adversary-w1-loom-selection-revalidation-pass-2`, D2) found, by reading the
code, that neither option above ends this route on its own: A fires only where the runtime itself
sees the move, which it does not after an executor's `NoUsefulAction` (`runtime.rs:549`); B helps
only if Loom maps `stale-revision` to `Suspended`, and a completed case would still end `Suspended`,
since `Suspended` returns (`runtime.rs:461`) before completion is consulted. A third option follows:

- C: an `ExecutorOutcome` variant by which an executor reports that the case moved, on which the
  runtime reloads as it does for a stale proposal. A specification change in Commission's
  `ExecutorOutcome` union.

## Decision (2026-10-07)

Option C. Commission's `ExecutorOutcome` gains a variant by which an executor reports that the case
moved to another revision. On it, `run_until_blocked` reloads the case and derives the run outcome
from the current frontier, as it already does for a stale proposal. Loom reports its
`stale-revision` refusal through that variant instead of `NoUsefulAction`.

Options A (a stale `RunOutcome`) and B (a moved-case `SuspensionReason`) are not taken: neither ends
the route measured in `crates/loom-executor/tests/adversary_w1_runtime_stale.rs` and
`adversary_w1p2_runtime_windows.rs` (pass 2, D2 above).

The variant is declared in `ess/commission/` first (Atlas ADR 0080), then the runtime and Loom
follow; the two adversary suites flip from today's outcome to `Completed` for a case completed
during selection and to the current obligation for a case that moved past one.
