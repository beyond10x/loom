---
format: aep.planning-md/3
id: story:budget-stop-cause
kind: story
status: draft
title: A governed or workflow run stopped by a model-loop budget reports that budget as its stop reason
relations:
- decomposes: epic:deterministic-workflows
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A governed or workflow run that ends on a model-loop budget reports that budget stop as its stop
reason: the `LoopStop` (for example `max-input-tokens { limit, reported }`) reaches the executor's
outcome and the run's suspension, instead of being collapsed into `NoUsefulAction`.

## Acceptance

`a_governed_run_stopped_by_its_input_token_budget_reports_the_budget`
(`crates/loom-executor/tests/`): a governed run with `max_input_tokens = 100` against a scripted
model reporting 150; the run ends with a budget reason carrying `limit: 100, reported: 150`, not
`NoUsefulAction`, and the same holds through `WorkflowExecutor` for a judgement step.

## Why (Loom 0.22.0)

- Every `LoopStop` other than `Completed` and the approval suspension becomes
  `no_useful_action()` (`crates/loom-executor/src/harness/governed.rs:577-578`); the cause survives
  only inside `LoopRun.run` and the sink's events.
- The status page still lists budgets of a governed loop as planned
  (`website/data/status.json:56`, "Still planned: budgets").

## Sketch

`SuspensionReason::Budget(LoopStop)` (or an `ExecutorOutcome` variant carrying it), propagated
through `run_loop`, `LoopExecutor` and `WorkflowExecutor`; declared in ESS first.

## Workaround it removes

The host runs its budgeted model loop outside Commission and reads `LoopOutcome.stop` directly.

## Scope (inferred)

`ess/commission/domains/responsibility.yaml` or `ess/domains/run.yaml` (the reason),
generated models, `crates/loom-executor/src/harness/governed.rs`,
`crates/loom-executor/src/workflow.rs`, `crates/loom-commission/src/runtime.rs`, tests,
`website/data/status.json`, `CHANGELOG.md` (`Breaking:` for an exhaustive match).

## Not in scope

Run-wide and per-action budgets (`story:run-and-action-budgets`).
