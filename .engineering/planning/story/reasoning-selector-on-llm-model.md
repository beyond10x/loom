---
format: aep.planning-md/3
id: story:reasoning-selector-on-llm-model
kind: story
status: archived
title: The reasoning-model selector asks llm Model, not the harness ModelPort
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-executor/src/selection.rs
- confidence: cited
  path: crates/loom-executor/tests/reasoning_model_selector.rs
revision: 3
transitions:
- {from: "draft", to: "archived", at: "2026-10-20T01:08:56Z", actor: "human:timo", revision: 3}
---
## Outcome

The executor's reasoning-model selector asks an llm `Model`, not the harness `ModelPort`
(`crates/loom-executor/src/selection.rs:26`), and keeps its contract: the model is offered only its
candidates, and an answer outside them is a selection error, never a selection. Where the llm-backed
selector moved by `story:chooser-executor-for-plugin-turns` already holds that contract, one of the
two is removed and the checks keep holding against the survivor.

## Why

ADR 0098 sends action selection to llm directly. The selector is one of the port's callers inside
Loom.

## Acceptance

- `crates/loom-executor/tests/reasoning_model_selector.rs` passes over an llm `Model` test double,
  including `an_action_outside_the_candidate_set_is_a_selection_error_and_never_a_selection`.
- `crates/loom-executor/tests/confidence_fallback.rs`, `fallback_selection_recording.rs` and
  `selection_telemetry.rs` pass.
- `crates/loom-executor/src/selection.rs` names no `harness::` path.
- `task check` exits 0.

## ESS first

No behaviour change; the selector's contract and its recorded selections are unchanged. The red step
is `reasoning_model_selector.rs` compiled against the llm `Model` double.

## Not in scope

- The Laya selector, which already calls llm's HTTP client.

## Scope

Crates: `loom-executor`.

- `crates/loom-executor/src/selection.rs`
- `crates/loom-executor/tests/reasoning_model_selector.rs`, `selection_telemetry.rs`
- `CHANGELOG.md`, `AGENTS.md` (the selector row)
