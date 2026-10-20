---
format: aep.planning-md/3
id: story:judgement-step-any-executor
kind: story
status: draft
title: A workflow judgement step runs on any AgentExecutor, the chooser by default
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:chooser-executor-for-plugin-turns
- depends_on: story:metering-on-llm-model
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-commission-testkit/tests/workflow_run_store.rs
- confidence: cited
  path: crates/loom-executor/src/workflow.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w11_u2.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w11_u3.rs
- confidence: cited
  path: crates/loom-executor/tests/budget_stop_cause.rs
- confidence: cited
  path: crates/loom-executor/tests/workflow_executor.rs
- confidence: cited
  path: crates/loom-executor/tests/workflow_step_outputs.rs
revision: 7
---
## Outcome

A workflow's judgement step runs on any `AgentExecutor` the host supplies, the chooser executor over
an llm `Model` by default, narrowed to the step's actions as today. Today the judgement step builds
a `LoopExecutor` itself (`crates/loom-executor/src/workflow.rs:110`, `:1302`).

## Why

The judgement step is the second product caller of the ported harness loop. It also lets a
workflow step run on an agent session executor once one exists, which takt asked for.

## Acceptance

- `crates/loom-executor/tests/workflow_executor.rs` passes with the default executor:
  `deterministic_steps_call_no_model`, `a_judgement_step_offers_the_model_only_its_actions`,
  `a_step_the_frontier_does_not_list_ends_needing_human_judgment`,
  `a_step_advances_only_after_its_effect_was_performed`.
- `…::a_judgement_step_runs_on_a_supplied_executor`: a test executor supplied by the host is called
  for the judgement step and offered only the step's actions; a proposal outside them is refused.
- `crates/loom-executor/tests/workflow_step_outputs.rs` passes: the `<workflow-inputs>` block
  reaches the supplied executor's instructions.
- A judgement step stopped by its executor's budget suspends the run for it
  (`a_judgement_step_stopped_by_its_input_token_budget_suspends_the_run_for_it`, adapted to the
  chooser's budget).
- `crates/loom-commission-testkit/tests/workflow_run_store.rs` passes, metering the workflow through
  the llm-backed meter (`:437` today uses `MeteredModelPort`).
- `crates/loom-executor/tests/adversary_w11_u2.rs` (7 cases, the workflow executor's governance
  rules) and `adversary_w11_u3.rs` (25 cases, typed step outputs) are migrated off `LoopConfig` and
  the `ModelPort` double to a supplied executor, and all 32 cases pass; `git grep -n 'harness::'`
  on both prints nothing.
- `git grep -n 'harness::' crates/loom-executor/src/workflow.rs` prints nothing.
- `task check` exits 0.

## ESS first

No noun the specification lacks; `loom.workflow` is unchanged. The red step is
`a_judgement_step_runs_on_a_supplied_executor`.

## Depends on

- `story:chooser-executor-for-plugin-turns`, which puts the llm-backed chooser where the workflow
  executor can use it.
- `story:metering-on-llm-model`, whose wrapper meters the chooser's calls.

## Not in scope

- A session executor for workflows (`story:metaharness-session-executor`).

## Scope

Crates: `loom-executor`, `loom-commission-testkit`.

- `crates/loom-executor/src/workflow.rs`
- `crates/loom-executor/tests/workflow_executor.rs`, `workflow_step_outputs.rs`, `budget_stop_cause.rs`,
  `adversary_w11_u2.rs`, `adversary_w11_u3.rs`
- `crates/loom-commission-testkit/tests/workflow_run_store.rs`
- `CHANGELOG.md`, `AGENTS.md` (§ Workflow definitions)
