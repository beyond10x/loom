---
format: aep.planning-md/3
id: story:idle-bound-outcome
kind: story
status: implemented
title: A run ended by the idle bound says so, apart from a frontier that admits nothing
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:refused-effect-ends-run
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-commission-testkit/tests/idle_bound_outcome.rs
- confidence: cited
  path: crates/loom-commission/src/outcome.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: cited
  path: generated/rust/commission
- confidence: cited
  path: website/docs/reference/commission
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T04:14:24Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-10-20T04:14:24Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "active", to: "implemented", at: "2026-10-20T08:39:24Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1,"review_outcome":3,"verification":1}}}
---
## Outcome

A run that the idle bound ended has its own run outcome, and a run whose frontier admits nothing
keeps `NoAdmissibleAction`. Today both end as `NoAdmissibleAction(Unit)`
(`generated/rust/commission/src/responsibility.rs:561-562`; the idle-bound paragraph of
`crates/loom-commission/src/runtime.rs`'s module doc), so an SDK host cannot tell "the executor made
no progress for two iterations" from "nothing could be done".

## Why

takt reports `NoProgress` and `NothingAdmissible` as two stops with their own exit codes, and its
scorecard counts them apart.

## Acceptance

- `crates/loom-commission-testkit/tests/idle_bound_outcome.rs::two_idle_iterations_end_the_run_as_the_idle_bound`:
  an executor that proposes nothing twice on a frontier that admits an action ends the run with the
  idle-bound outcome.
- `…::a_frontier_that_admits_nothing_ends_as_no_admissible_action`: unchanged from 0.26.0.
- `…::a_repeated_admission_counts_toward_the_idle_bound`: a request with the action and arguments of
  one already admitted ends the run with the idle-bound outcome after two such iterations.
- The intake slice maps the idle-bound outcome to `StopReason::NothingAdmissible`, as today; a unit
  test of the mapping in `crates/loom-intake-slice/src/run.rs` holds it, and `task docs-check`
  reports no change to the `run --output jsonl` contract.
- `task check` exits 0.

## ESS first

The new `RunOutcome` variant is declared in `ess/commission/domains/responsibility.yaml` first; the
red step is the conformance scenario synthesized from it.

## Depends on

`story:refused-effect-ends-run`, which changes the same idle rule, declarations and files first.

## Not in scope

- Changing the idle rule itself (two idle iterations) or the unbudgeted step limit.
- A separate stop reason in `b10x-loom`'s output: a new `StopReason` value is a contract change for
  its own story.

## Scope

Crates: `loom-commission`, `loom-commission-testkit`, `loom-intake-slice`.

- `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/` (regenerated)
- `crates/loom-commission/src/runtime.rs`, `crates/loom-commission/src/outcome.rs`
- `crates/loom-commission-testkit/tests/idle_bound_outcome.rs` (new)
- `crates/loom-intake-slice/src/run.rs` (outcome mapping, `:892`)
- `website/docs/reference/commission/` (regenerated), `CHANGELOG.md`

Shared with `story:refused-effect-ends-run` (before), `story:unperformable-names-actions` and
`story:pending-effect` (after).

## Correction at close (wave 2026-10-20-w2)

- When the last idle iteration's own effect completed the case, the run ends Completed, as at the step budget (cfe37ee). A run-budget breach right after a performed effect still ends suspended before the case is loaded, as the runtime documents.
