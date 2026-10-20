---
format: aep.planning-md/3
id: story:metering-on-llm-model
kind: story
status: draft
title: The run meter and the reasoning-model selector run on llm Model, not the harness ModelPort
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:chooser-executor-for-plugin-turns
- depends_on: story:executor-reports-usage
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-executor/src/metering.rs
- confidence: cited
  path: crates/loom-executor/src/selection.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w11_u1_metering.rs
- confidence: cited
  path: crates/loom-executor/tests/confidence_fallback.rs
- confidence: cited
  path: crates/loom-executor/tests/fallback_selection_recording.rs
- confidence: cited
  path: crates/loom-executor/tests/reasoning_model_selector.rs
- confidence: cited
  path: crates/loom-executor/tests/run_metering.rs
- confidence: cited
  path: crates/loom-executor/tests/selection_telemetry.rs
- confidence: cited
  path: crates/loom-selector-laya/tests/laya_arguments_slice.rs
revision: 7
---
## Outcome

The run meter and the executor's reasoning-model selector run on llm's `Model` instead of the
harness `ModelPort`. A metered wrapper over an llm `Model` counts each `turn` as one call, adds the
tokens llm reports and prices it with the run's rate card, with the rules of today (a failed turn is
a call with unknown tokens; a cost no card priced is absent, never zero; steps sum to the run).
The reasoning-model selector asks an llm `Model` and keeps its contract: the model is offered only
its candidates, and an answer outside them is a selection error, never a selection. Today the meter
wraps the harness `ModelPort` and takes its `RateCard` from the ported turn loop
(`crates/loom-executor/src/metering.rs:51-52`), and the selector takes the harness `ModelPort`
(`crates/loom-executor/src/selection.rs:26`). The two move together because the metering tests run
the selector through the meter (`crates/loom-executor/tests/run_metering.rs:391-393`).

Where the selector `story:chooser-executor-for-plugin-turns` moved into `loom-executor` already
holds the reasoning-model selector's contract, `ReasoningModelSelector` is removed and its checks run
against the moved one.

## Why

ADR 0098 sends action selection to llm directly, and its consequences name re-pointing metering at
llm's `Model` before the port is deleted.

## Acceptance

- `crates/loom-executor/tests/run_metering.rs` passes over the llm-backed wrapper:
  `the_reported_calls_equal_the_turns_the_model_port_saw`,
  `a_step_counts_the_selector_and_argument_calls_made_in_it`,
  `cost_is_absent_without_a_rate_card_and_for_an_unlisted_model`.
- `crates/loom-executor/tests/reasoning_model_selector.rs` passes over an llm `Model` test double,
  including `an_action_outside_the_candidate_set_is_a_selection_error_and_never_a_selection`;
  `confidence_fallback.rs`, `fallback_selection_recording.rs` and `selection_telemetry.rs` pass.
- `git grep -n 'impl.*ActionSelector for' crates/loom-executor/src crates/loom-intake-slice/src`
  lists exactly one selector that asks a model.
- The rate card type lives outside `harness::`, and `git grep -n 'harness::'
  crates/loom-executor/src/metering.rs crates/loom-executor/src/selection.rs` prints nothing.
- `CHANGELOG.md` names, under `Breaking:`, the changed `MeteredModelPort` (now over llm's `Model`),
  the new path of `RateCard` and the `ReasoningModelSelector` change.
- `task check` exits 0.

## ESS first

No noun the specification lacks; `loom.metering` and the recorded selections are unchanged. The
red step is `the_reported_calls_equal_the_turns_the_model_port_saw` over an llm `Model` test double.

## Depends on

- `story:chooser-executor-for-plugin-turns`, which moves the llm-backed selector this story keeps or
  removes.
- `story:executor-reports-usage`, which rewrites `metering.rs` and `run_metering.rs` first.

## Not in scope

- Executor usage that is not a model call (`story:executor-reports-usage`).
- The Laya selector, which already calls llm's HTTP client.

## Scope

Crates: `loom-executor`, `loom-sdk`, `loom-selector-laya` (test only).

- `crates/loom-executor/src/metering.rs`, `crates/loom-executor/src/selection.rs`
- `crates/loom-executor/tests/run_metering.rs`, `adversary_w11_u1_metering.rs`,
  `reasoning_model_selector.rs`, `selection_telemetry.rs`, `confidence_fallback.rs`,
  `fallback_selection_recording.rs`
- `crates/loom-selector-laya/tests/laya_arguments_slice.rs`
- `crates/loom-sdk/src/lib.rs` if the meter is re-exported
- `CHANGELOG.md`, `AGENTS.md` (the metering and selector rows)
