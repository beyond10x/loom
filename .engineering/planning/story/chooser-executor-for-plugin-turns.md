---
format: aep.planning-md/3
id: story:chooser-executor-for-plugin-turns
kind: story
status: implemented
title: Plugin turns run on the chooser executor over llm Model, not the ported harness loop
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-cli/src/plugin.rs
- confidence: cited
  path: crates/loom-executor/Cargo.toml
- confidence: cited
  path: crates/loom-executor/src/lib.rs
- confidence: inferred
  path: crates/loom-executor/src/model_selector.rs
- confidence: cited
  path: crates/loom-intake-slice/src/lib.rs
- confidence: cited
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: crates/loom-intake-slice/src/selector.rs
- confidence: cited
  path: crates/loom-plugin/src/lib.rs
- confidence: cited
  path: crates/loom-plugin/src/turn.rs
- confidence: cited
  path: crates/loom-plugin/tests/adversary_2.rs
- confidence: cited
  path: crates/loom-plugin/tests/earlier_reads.rs
- confidence: cited
  path: crates/loom-plugin/tests/support/mod.rs
- confidence: cited
  path: crates/loom-sdk/src/lib.rs
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T17:46:08Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-10-20T17:46:10Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "active", to: "implemented", at: "2026-10-20T23:18:24Z", actor: "human:timo", revision: 12, decided_on: {"recorded":{"test_result":1,"review_outcome":3}}}
---
## Outcome

A plugin turn runs on the chooser executor (Loom's select-then-arguments path, `Loom::run`) over an
llm `Model`, under `run_until_blocked` as today, instead of the ported harness loop
(`LoopExecutor`, `crates/loom-plugin/src/turn.rs:48-50`). Each step selects one action from the
frontier, generates its arguments, and is revalidated and performed by `DataSourceEffects` as now;
the reports of the reads performed earlier in the turn are given to later steps as data, never as
instructions. The llm-backed selector and argument generator the intake slice already uses
(`ModelSelector`, `ModelArguments`; `crates/loom-intake-slice/src/selector.rs:3-4`, `:580`, `:796`)
move to `crates/loom-executor/src/` so the plugin host can use them, with the `Briefing` they hold
(`selector.rs:118`, `:570`, `:646`). The slice's `selector` module keeps the path by re-exporting
them, so the five slice files that import `crate::selector::Briefing` (`bindings.rs:43`,
`clock.rs:21`, `effect.rs:42`, `handoff.rs:48`, `intent.rs:44`) are not edited. `b10x-loom plugin
run` stops building an `LlmModelPort` (`crates/loom-cli/src/plugin.rs:111`).

## Why

ADR 0098 sends Loom's own decisions to llm and leaves Path B (the ported loop) to this plan: run it
through metaharness or retire it. Plugin turns are Path B's product caller. Through metaharness a
turn's reads could not be performed by Loom per call (metaharness 0.9.3, ADR 0098 Context), so the
turn moves to the chooser path, where every read stays one admitted, revalidated Commission step.

## Acceptance

- The plugin host's tests pass with the new executor: `crates/loom-plugin/tests/effects.rs`
  (`effects_refuse_an_undeclared_read`, `propose_writes_the_record_only`),
  `crates/loom-plugin-slack/tests/run.rs` (`fixture_run_invokes_no_write`,
  `fixture_run_records_three_proposals`) and `crates/loom-cli/tests/plugin.rs`, on recorded responses.
- `crates/loom-plugin/tests/earlier_reads.rs::a_later_step_sees_an_earlier_read_as_data`: the
  argument request for `reply.propose` carries the earlier `source.read` report in one data block no
  value can close.
- `TURN_MODEL_BUDGET` still bounds every request sent, retries included (the existing budget test in
  `crates/loom-plugin/tests/` passes).
- The intake slice's tests pass with the moved selector (`crates/loom-intake-slice/tests/selector_executor.rs`).
- `git grep -nE 'harness::|LlmModelPort' crates/loom-plugin crates/loom-cli/src/plugin.rs` prints
  nothing, `crates/loom-plugin/tests/adversary_2.rs` included.
- `task check` exits 0.

## ESS first

If the earlier-reads block is a declared type, it is declared in `ess/domains/plugin.yaml` first;
otherwise the red step is `a_later_step_sees_an_earlier_read_as_data`.

## Not in scope

- Workflow judgement steps (`story:judgement-step-any-executor`).
- The reasoning-model selector and the meter (`story:metering-on-llm-model`).
- Deleting the port (`story:delete-harness-port`).
- Changing what a plugin may read or propose.

## Scope

Crates: `loom-plugin`, `loom-cli`, `loom-intake-slice`, `loom-executor`, `loom-sdk`.

- `crates/loom-plugin/src/turn.rs`, `crates/loom-plugin/src/lib.rs`,
  `crates/loom-plugin/tests/support/mod.rs`, `crates/loom-plugin/tests/adversary_2.rs`,
  `crates/loom-plugin/tests/earlier_reads.rs` (new)
- `crates/loom-cli/src/plugin.rs`
- `crates/loom-intake-slice/src/selector.rs` (moved out, a re-export left),
  `crates/loom-intake-slice/src/lib.rs:44`, `crates/loom-intake-slice/src/run.rs:156` (the import)
- `crates/loom-executor/src/` (the selector's new module, name inferred), `crates/loom-executor/src/lib.rs`,
  `crates/loom-executor/Cargo.toml` (gains `b10x-llm-core`, which it lacks today, `:10-11`), `Cargo.lock`
- `crates/loom-sdk/src/lib.rs` (a re-export)
- `CHANGELOG.md`, `AGENTS.md` (§ Plugin host, § Slack handler)

`crates/loom-intake-slice/src/run.rs` is also edited by `story:authority-sees-the-request` and the
Commission outcome stories (other regions); this story is later in the epic's order.

## Correction at close (wave 2026-10-20-w4)

- `Briefing` did not move: it holds the slice's result store, working context and metrics, so the
  executor gained a `Briefing` trait the slice's struct implements, and the slice keeps
  `ModelSelector` and `ModelArguments` at their paths as aliases (`run.rs` and `lib.rs` untouched).
- A turn sends a selection and an argument request per step, so `TURN_MODEL_BUDGET` is 16; plugin
  turns retry through the slice's `send_with_retries`; earlier reads reach later steps as escaped
  strings in one block.
- Adversary pass 1 (a blocker, fixed): a model answer without a usable call (a refusal or
  content-filter stop, a text-only answer, an answer llm refuses) ended the turn `Unavailable` and
  stalled the host; it now ends the turn `stopped`, and only an unanswered request is `Unavailable`.
