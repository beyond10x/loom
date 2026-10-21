---
format: aep.planning-md/3
id: approval-record:wave-proposal-2026-10-21-w7
kind: approval-record
status: draft
title: Wave 2026-10-21-w7 proposal accepted with no operator turn
tags:
- non-interactive
relations:
- decides: story:stop-policy-gate-and-idle-bound
- decides: story:run-budget-tokens-and-deadline
- decides: story:pending-answer-ends-run
- decides: story:first-declared-completion
revision: 2
---
# Wave 2026-10-21-w7 proposal, accepted with no operator turn

The run is a dispatched session: no operator turn. The proposal was approved by session
`b10x-chef-orchestrator` under its standing approval for work implementing Atlas ADR 0099: build
the opt-in stop seams a host chooses per run, keep today's defaults, and release 0.32.0. Its
conditions: with the approval gate off, an approval-required action that was not granted is still
refused, held by a test and attacked by the adversary pass; the `KeepStep` CHANGELOG line says it
performs a proposal after a spending limit was passed; the specification changes first in
`ess/commission`.

| unit | serves | surface | scope | adversary |
|---|---|---|---|---|
| `story:stop-policy-gate-and-idle-bound` | `vision:governed-autonomy` | `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/`, `crates/loom-commission` (`runtime.rs`, `outcome.rs`, `lib.rs`), `crates/loom-commission-conformance`, `crates/loom-commission-testkit/tests/stop_policy.rs`, `docs/commission/design/approval-gate.md` | cited | in the one pass (approval gate off with an ungranted approval-required action) |
| `story:run-budget-tokens-and-deadline` | `vision:governed-autonomy` | the same specification and runtime, `crates/loom-commission-testkit/tests/run_and_action_budgets.rs`, `crates/loom-governor/src/file_suspension.rs`, every `RunBudget` literal | cited | in the one pass (deadline parsing, `KeepStep` admission) |
| `story:pending-answer-ends-run` | `vision:governed-autonomy` | the same specification and runtime, `crates/loom-commission-testkit/tests/pending_effect.rs`, every `EffectOutcomePending` literal | cited | in the one pass (fan-out, resume) |
| `story:first-declared-completion` | `vision:governed-autonomy` | `ess/domains/governor.yaml`, `generated/rust/loom/`, `crates/loom-governor/src/lib.rs`, `crates/loom-governor/tests/completion_rule.rs` | cited | in the one pass (declaration order, blocked outcomes) |

- Overlap: the first three units write `ess/commission/domains/responsibility.yaml`,
  `generated/rust/commission/` and `crates/loom-commission/src/runtime.rs`, so they run one after
  another, each based on the wave head after the previous merge. `CHANGELOG.md` and `AGENTS.md`
  are touched by every unit, one at a time.
- Breaks for a consumer: `RunBudget` gains two fields and `BudgetLimit` two variants;
  `EffectOutcomePending` gains `ends_run`. Each is a `**Breaking:**` line.
- Left out: the intake slice, plugin turns and `b10x-loom` keep the default policy.
- Pre-flight: `/` 50G free, load 15.10. One unit build at a time through `b10x-build-slot`, with
  `CARGO_INCREMENTAL=0`, each into its own tree's `target/`.
- Commits authorised: 1 opening store commit, the unit commits on each `impl/<story>` branch, 1
  merge per unit into `wave/2026-10-21-w7`, the release commit for 0.32.0, 1 closing store commit,
  the fast-forward of `main` to the wave head through the bot, the annotated tag 0.32.0 and its
  GitHub Release.

## Units

| unit | tree id | branch | stage |
|---|---|---|---|
| stop-policy-gate-and-idle-bound | loom-20261021-w7-u1 | impl/stop-policy-gate-and-idle-bound | proposed |
| run-budget-tokens-and-deadline | loom-20261021-w7-u2 | impl/run-budget-tokens-and-deadline | proposed |
| pending-answer-ends-run | loom-20261021-w7-u3 | impl/pending-answer-ends-run | proposed |
| first-declared-completion | loom-20261021-w7-u4 | impl/first-declared-completion | proposed |

Integration tree `loom-wave-20261021-w7`, branch `wave/2026-10-21-w7`. Scratch root
`~/.cache/loom-wave-2026-10-21-w7/`.

## Red run: a resume whose deadline passes after the take

The second case of `a_resume_past_its_deadline_performs_nothing`
(`crates/loom-commission-testkit/tests/run_and_action_budgets.rs`) covers a deadline that passes
after the resume took the Suspension. Unit 3's red run never reached it: the first case panicked
first. It was run alone on 2026-10-21, in a scratch tree at the wave head `4a07f3f2`.

| Runtime | Command | Result |
|---|---|---|
| `4a07f3f2` without the check after the take (`crates/loom-commission/src/runtime.rs`, five lines removed from the resumed branch: `deadline_reached`, `release_taken`, `budget_ended`) | `cargo test -p b10x-loom-commission-testkit --locked --test run_and_action_budgets -- --exact a_resume_past_its_deadline_performs_nothing` | `FAILED`, `run_and_action_budgets.rs:1075:5`, `nothing is performed`, `left: 1`, `right: 0`, exit 101 |
| `4a07f3f2` as merged | the same | `ok`, 1 passed, exit 0 |

The first case passed both times, so the failure at line 1075 is the second case's.
