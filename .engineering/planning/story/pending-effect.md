---
format: aep.planning-md/3
id: story:pending-effect
kind: story
status: draft
title: An effect can answer pending and be invoked again within the budget without counting idle
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:unperformable-names-actions
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-commission-testkit/tests/pending_effect.rs
- confidence: cited
  path: crates/loom-commission/src/ports/effect.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: crates/loom-connectors/src/lib.rs
- confidence: cited
  path: crates/loom-intake-slice/src/bindings.rs
- confidence: cited
  path: crates/loom-intake-slice/src/clock.rs
- confidence: cited
  path: crates/loom-intake-slice/src/effect.rs
- confidence: cited
  path: crates/loom-intake-slice/src/handoff.rs
- confidence: cited
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: crates/loom-plugin/src/effects.rs
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: cited
  path: generated/rust/commission
- confidence: cited
  path: website/docs/reference/commission
revision: 8
---
## Outcome

An effect can answer that its request is pending (the work it started has not finished, observe
again), and the runtime invokes it again within the run's step and run budgets without counting the
iteration idle. When a budget ends the run while a request is still pending, the outcome names the
request as waiting. Today an effect answers only `Performed` or `Refused`
(`generated/rust/commission/src/responsibility.rs:301-306`), and anything else is idle.

## Why

takt's validators wait on work outside the run (a CI job, a review) and report `Waiting`, exit 6,
when it has not finished. On `run_until_blocked` a validator has no way to say "not yet".

## Acceptance

- `crates/loom-commission-testkit/tests/pending_effect.rs::a_pending_answer_is_invoked_again_and_not_counted_idle`:
  an effect answering pending twice and then `Performed` is invoked three times and the run goes on;
  the idle count does not move.
- `…::each_reinvocation_is_revalidated`: a pending request whose case moved is not invoked again.
- `…::a_budget_reached_while_pending_ends_the_run_waiting`: the outcome names the pending request.
- `…::each_answer_is_charged_and_observed`: every pending answer is one trial on the run's budget
  and one observation; none is evidence.
- The intake slice maps the waiting outcome to the existing `StopReason::StepBudget` (a budget ended
  the run); a unit test of the mapping in `crates/loom-intake-slice/src/run.rs` holds it, and
  `task docs-check` reports no change to the `run --output jsonl` contract.
- Every `EffectPort` in the workspace compiles against the new answer and its tests pass unchanged.
- `task check` exits 0.

## ESS first

The pending answer and the waiting outcome are declared in
`ess/commission/domains/responsibility.yaml` first; the red step is the conformance scenario
synthesized from them.

## Depends on

`story:unperformable-names-actions`, which changes the same declarations and `runtime.rs` first; its
idle rule is the one `story:refused-effect-ends-run` and `story:idle-bound-outcome` set.

## Not in scope

- A wait between invocations or a timer: the host's effect decides how long one invocation takes.
- Final refusals (`story:refused-effect-ends-run`).

## Scope

Crates: `loom-commission`, `loom-commission-testkit`, `loom-connectors`, `loom-plugin`,
`loom-intake-slice`.

- `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/` (regenerated)
- `crates/loom-commission/src/runtime.rs`, `crates/loom-commission/src/ports/effect.rs`
- the `EffectPort` impls (`git grep -nE 'impl.* EffectPort for' crates`): `crates/loom-connectors/src/lib.rs`,
  `crates/loom-intake-slice/src/{bindings,clock,effect,handoff}.rs`, `crates/loom-plugin/src/effects.rs`
- `crates/loom-commission-testkit/tests/pending_effect.rs` (new)
- `crates/loom-intake-slice/src/run.rs` (outcome mapping, `:866-916`)
- `website/docs/reference/commission/` (regenerated), `CHANGELOG.md`

Shared with `story:unperformable-names-actions` (before) and `story:executor-reports-usage` (after).
The slice's `EffectPort` files are not edited by `story:chooser-executor-for-plugin-turns`, which
leaves their `crate::selector` imports as they are.
