---
format: aep.planning-md/3
id: story:refused-effect-ends-run
kind: story
status: implemented
title: A host ends a run on an effect refusal, and the outcome names the request and the reason
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:authority-sees-the-request
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-commission-testkit/tests/refused_effect_outcome.rs
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
- {from: "draft", to: "proposed", at: "2026-10-20T04:14:23Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-10-20T04:14:24Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "active", to: "implemented", at: "2026-10-20T08:39:23Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1,"review_outcome":4,"verification":1}}}
---
## Outcome

An effect port can refuse an admitted request as final, and the run ends at once with a run outcome
naming the request and carrying the refusal's reason. Today a `Refused` effect is an idle iteration:
the executor runs again, and the run ends only at the idle bound, as `NoAdmissibleAction`, with the
reason gone (`crates/loom-commission/src/runtime.rs` module doc, item 8 and the idle-bound
paragraph; `crates/loom-commission/src/outcome.rs:66-70`).

## Why

takt's edit effect refuses a proposed change its step guard does not admit, and takt stops the run
there with `StepRefused` and exit 3. On `run_until_blocked` that refusal is lost in an idle loop.

## Acceptance

- `crates/loom-commission-testkit/tests/refused_effect_outcome.rs::a_final_refusal_ends_the_run_naming_request_and_reason`:
  an effect that refuses an admitted request as final ends the run with the new outcome, naming the
  request id and the reason; the executor is called once.
- `…::an_ordinary_refusal_still_counts_idle`: a refusal not marked final ends the run as on 0.26.0.
- `…::the_refusal_is_observed_once_and_recorded`: the effect's observation is delivered once, and
  `LoopEnd::effects` and `LoopEnd::requests` each hold the request once.
- The intake slice maps the new outcome to the existing `StopReason::Refused`
  (`ess/intake/domains/routing.yaml:27`) with the reason; a unit test of the mapping in
  `crates/loom-intake-slice/src/run.rs` holds it, and `task docs-check` reports no change to the
  `run --output jsonl` contract.
- `task commission:conform` runs the new scenario; `task check` exits 0.

## ESS first

`ess/commission/domains/responsibility.yaml` declares the final refusal and the run outcome that
carries it before any code; the red step is the Commission conformance scenario synthesized from
it, then `a_final_refusal_ends_the_run_naming_request_and_reason`.

## Depends on

`story:authority-sees-the-request`, which edits the same `runtime.rs` and
`crates/loom-intake-slice/src/run.rs` first.

## Not in scope

- Retrying a refused effect: the loop never retries.
- Pending or observe-again answers (`story:pending-effect`).
- A new value of the slice's `StopReason`.

## Scope

Crates: `loom-commission`, `loom-commission-testkit`, `loom-intake-slice`.

- `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/` (regenerated)
- `crates/loom-commission/src/runtime.rs`, `crates/loom-commission/src/outcome.rs`
- `crates/loom-commission-testkit/tests/refused_effect_outcome.rs` (new)
- `crates/loom-intake-slice/src/run.rs` (outcome mapping, `:892-910`)
- `website/docs/reference/commission/` (regenerated), `CHANGELOG.md`, `AGENTS.md`

The same declarations, generated code, `runtime.rs`, `outcome.rs` and `run.rs` mapping are changed
by `story:idle-bound-outcome`, `story:unperformable-names-actions`, `story:pending-effect` and
`story:executor-reports-usage`, which land after this story in that order.

## Correction at close (wave 2026-10-20-w2)

- In a fan-out, a stale read is judged first; a final refusal beside it is kept and ends the run when the next load finds no move (a349ee2, 6c2e84d), from two adversary passes.
