---
format: aep.planning-md/3
id: story:silence-and-expiry-at-a-host-tick
kind: story
status: draft
title: A host tick at a given instant expires unanswered requests or takes their default on silence
summary: decisions tick --at, no clock read, idempotent, completing with the writers of its store option
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decision-inbox-commands
- depends_on: story:decided-record-written-then-recorded
revision: 4
---
## Outcome

`tick(store, writers, at)` in `b10x-loom-decision` ends every request nobody answered by the
instant `at`, which the host gives; nothing in it reads a clock. For each Open or Escalated request
whose `expires_at` is at or before `at`:

- with a default on silence, it applies `TakeRecommendation` with principal `silence`, principal
  kind `silence` and `decided_at` = `at`, then completes the request with the writers it was given
  (`story:decided-record-written-then-recorded`);
- without one, it applies `Expire` with `at`; the request is Expired ("the case stays stopped",
  `~/beyond10x/sdc/ess/domains/decision.yaml:800`).

A request with no `expires_at`, or one after `at`, is not touched: the tick never sends it a
command the contract would refuse `NotDue` (`decision.yaml:625-630`, `:782-786`). A tick answers a
`TickReport` (taken on silence, expired, completed, each by request id); a second tick at the same
instant changes nothing and answers an empty report. `loom_sdk::decisions::tick` is the same call.

`b10x-loom decisions tick --at INSTANT --store DIR` runs it and prints the report as JSON; `--at`
is required. On the command line the tick completes with the writers of the store option it is
given, the same set the other `decisions` commands use there:

- `--store DIR`: the governor's `ExplicitDecisionWriter` over the directory's `FileCaseStore`
  (`story:decision-inbox-commands`);
- `--state DIR`: the slice's writers (`story:approval-decisions-resume-held-runs`,
  `story:question-and-outcome-decisions-hold-runs`);
- `--tree STORE`: the tree's writers (`story:tree-waits-open-decision-requests`).

A request taken on silence whose record kind has no writer in that set stays Decided without a
record (`AwaitsWriter`): the report lists it under taken on silence and not under completed, and
the next command with a writer for it completes it. The tick changes requests only; what an
Expired request means for a held run is the host's (`story:question-and-outcome-decisions-hold-runs`),
and what a tick over a tree's store writes into the run logs is the tree's
(`story:tree-waits-open-decision-requests`).

Its code goes in its own module of `crates/loom-decision`, and the subcommand in the inbox's module
of `crates/loom-cli`.

## Why

Item 3 of the request: "expiry with an optional default on silence (fired by a host tick that
passes the instant)" (`~/beyond10x/sdc/docs/decisions.md:308-309`). Session `sdc` answered on
2026-11-21 (point 8): a host tick with an explicit instant argument, a CLI command now and a
daemon later, and no clock read inside the transition; the contract now takes the instant as `at`
on `Expire` (`decision.yaml:776`) and refuses a tick before `expires_at` (`decisions.md:84-86`).
Commission sets no timer (`AGENTS.md:423`), and the contract names no one who fires `Expire` or
the silence `TakeRecommendation` at `expires_at` beyond the Driver actor (`decision.yaml:286-296`).

## Waits on

`story:decision-inbox-commands`: both change `crates/loom-cli` and `crates/loom-sdk`'s decisions
module, and it gives `--store` its writer. `story:decided-record-written-then-recorded`: a silence
decision is completed, and both change `ess/domains/decision.yaml`.

## ESS first

`ess/domains/decision.yaml` gains `TickReport`. On that commit `task drift` fails, and
`crates/loom-decision/tests/tick.rs` does not compile.

## Acceptance

- A request with a strong default on silence, `expires_at` 10:00, ticked at 09:59 is unchanged;
  at 10:00 it is Decided by `silence` with its recommended option and `decided_at` 10:00, and its
  record is written and recorded.
- A request without a default ticked past its expiry is Expired and has no record.
- A request with a non-strong recommendation cannot carry a default on silence (the invariant of
  `decision.yaml:272`), so no tick decides it; it expires.
- A request with no `expires_at` is untouched by a tick at any instant.
- A second tick at the same instant answers an empty report and changes no byte of the store.
- An Escalated request past its expiry is handled as an Open one.
- `decisions tick` without `--at` exits non-zero before it reads the store.
- `decisions tick --at 10:00 --store DIR` over an outcome request with a strong default `A` and
  `expires_at` 10:00 decides it by `silence`, the directory's governor holds the explicit
  decision, and the printed report lists it under completed.
- The same command over a question request with a strong default and the same expiry decides it
  by `silence`, writes no record, and the report lists it under taken on silence and not under
  completed.
- The tick reads no clock: a test runs it under a host clock fixed at 1970 and the result depends
  only on `--at`.
- `task check` exits 0.

## Out of scope

A timer or daemon. The tree daemon of `epic:tree-daemon-under-enforced-limits` calling this tick
is a later story of that epic. A held run's Suspension after expiry
(`story:question-and-outcome-decisions-hold-runs`). Run-log lines for a tree
(`story:tree-waits-open-decision-requests`). Resuming a run after a silence decision
(`story:approval-decisions-resume-held-runs`). A writer of an approval or question record.
