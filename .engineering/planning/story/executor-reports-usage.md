---
format: aep.planning-md/3
id: story:executor-reports-usage
kind: story
status: implemented
title: An executor that is not a model loop reports its usage into the run charges, budget and meter
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:pending-effect
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-commission-testkit/tests/executor_usage.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: crates/loom-executor/src/metering.rs
- confidence: cited
  path: crates/loom-executor/tests/run_metering.rs
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: cited
  path: ess/domains/metering.yaml
- confidence: cited
  path: generated/rust/commission
- confidence: cited
  path: generated/rust/loom
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T08:57:05Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-10-20T08:57:06Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "active", to: "implemented", at: "2026-10-20T15:47:26Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
---
## Outcome

An `AgentExecutor` that is not a model loop (an agent session, a remote executor) reports what one
call spent (input and output tokens, cost where known, wall time), and the runtime charges it to
the run's charges and budget and hands it to the run meter as that step's usage. Today only a
`Performed` effect's usage is charged (`crates/loom-commission/src/runtime.rs` module doc, "Run and
action budgets"), only turns through a metered `ModelPort` are metered
(`crates/loom-executor/src/metering.rs:3-12`), and `ExecutorOutcome` carries no usage
(`generated/rust/commission/src/responsibility.rs:357-372`).

## Why

takt's `--tokens` and `--hours` budgets and its per-step token report cover harness steps. When a
harness step becomes an `AgentExecutor` over a session, its spend must reach the same budget and
meter, or a session can run past a budget nobody sees.

## Acceptance

- `crates/loom-commission-testkit/tests/executor_usage.rs::an_executor_usage_is_charged_to_the_run`:
  an executor reporting 1,000 input tokens per call over three calls leaves the run's charges at
  3,000.
- `…::a_run_budget_breach_by_executor_usage_suspends_the_run`: a `max_input_tokens` below the
  reported total suspends the Run with `RunBudget`, as an effect's breach does.
- `…::an_unknown_cost_stays_unknown`: a usage with no cost leaves the run's cost absent, never zero.
- `crates/loom-executor/tests/run_metering.rs::a_non_model_executor_usage_is_its_step_usage`: the
  usage appears as its step's usage and the steps still sum to the run.
- `task check` exits 0.

## ESS first

Where executor usage is carried (on `ExecutorOutcome` or beside it) is declared in
`ess/commission/domains/responsibility.yaml` first, and in `ess/domains/metering.yaml` if the
report changes; the red step is the conformance scenario synthesized from it.

## Depends on

`story:pending-effect`, which changes the same declarations and `runtime.rs` first.

## Not in scope

- Moving the meter off the harness `ModelPort` (`story:metering-on-llm-model`, which depends on
  this story because both rewrite `metering.rs` and `run_metering.rs`).
- Pricing: a cost is what the executor reports, or absent.

## Scope

Crates: `loom-commission`, `loom-commission-testkit`, `loom-executor`.

- `ess/commission/domains/responsibility.yaml`, `ess/domains/metering.yaml`,
  `generated/rust/commission/`, `generated/rust/loom/` (regenerated)
- `crates/loom-commission/src/runtime.rs`
- `crates/loom-executor/src/metering.rs`, `crates/loom-executor/tests/run_metering.rs`
- `crates/loom-commission-testkit/tests/executor_usage.rs` (new)
- `CHANGELOG.md`, `AGENTS.md` (§ Checks that hold the rules, the metering row)

Shared with `story:pending-effect` (before) and `story:metering-on-llm-model` (after).

## Correction at close (wave 2026-10-20-w3)

- What shipped: the usage travels beside the outcome, `ExecutorStep { outcome, usage }`, through a
  provided `AgentExecutor::run_with_usage` whose default reports none; no existing executor breaks.
  Putting it on `ExecutorOutcome` would have touched about 300 constructors in about 90 files.
- No new command: the change is types only, so the red step was `task commission:drift` and then the
  story's tests, not a synthesized scenario.
- A spending-limit breach from executor usage ends the run once the step is observed, before its
  outcome is acted on; wall time is charged to the run budget only (the meter's step usage has no
  duration, which would change the stored workflow run record).
- Adversary pass 1 (fixed): `MeteredExecutor` called through plain `run` records no reported usage,
  so the meter and the run's charges agree; a step with no model call costs what its executor
  reported, or zero, with or without a rate card.
