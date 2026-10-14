---
format: aep.planning-md/3
id: story:dw-run-model-meter
kind: story
status: draft
title: A run reports its model calls, tokens and cost per step, counted at the model port
relations:
- decomposes: epic:deterministic-workflows
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A host that embeds Loom through the SDK gets, for every Commission run, one usage report: the
number of model calls, the tokens the provider reported and the cost at a declared rate card, in
total and per step. The calls are counted at the `ModelPort` boundary, so the report's call count
equals the number of `ModelPort::turn` calls the run made, through the loop, the reasoning-model
selector, the argument generator and compaction alike.

## Why

A workflow cannot show that a change removed a model call unless each run reports its calls.
Counters exist, but none is a per-run total a workflow can be compared on:
`LoopOutcome` (`crates/loom-executor/src/harness/turn_loop/mod.rs:148`) carries `turns`, one
`Usage` per turn and `cost_micro_usd` for one loop only, and every executor call of a Commission
run starts a new loop (`LoopExecutor::run`, `crates/loom-executor/src/harness/governed.rs`); the
selector (`crates/loom-executor/src/selection.rs`) and the argument generator call the model
outside any `LoopOutcome`; the intake slice's `ContextMetrics`
(`crates/loom-intake-slice/src/context_metrics.rs`) counts at the llm port of the slice only.

## Design

- `crates/loom-executor/src/metering.rs` (new): `MeteredModelPort<M: ModelPort>` wraps any port
  and records each `turn` call: one call per `turn`, the `Usage` of a turn that answered, none for
  a turn that failed (a failed turn is still a call). It prices each turn once with the run's
  `RateCard` (`crates/loom-executor/src/harness/turn_loop/price.rs`, `RateCard::price`), and the
  run's cost is the sum of the rounded turns, as `price.rs` already sums them.
- `MeteredExecutor<E: AgentExecutor>` wraps any executor and opens one step per `run` call; every
  model call made during that call is counted in that step. A step is one executor call of the run
  (`crates/loom-commission/src/runtime.rs`, module docs item 6), counted from 1, including the call
  of the iteration that ends the run.
- A shared `RunMeter` both hold returns the report as the generated `RunUsageReport`; the SDK
  re-exports the three types. Commission's `LoopEnd` is not changed: Commission names no model
  (`AGENTS.md` § Rules).
- No price is reported as absent, never zero, as `price.rs` holds; a model the card does not list
  leaves the cost absent for that step and for the total.

## Domain relations

- `RunUsageReport` -> `commission.responsibility.Run`: a typed id field (`commission_run`,
  `loom.run.CommissionRunId`), not a relation, as `loom.run.Session` holds it
  (`ess/domains/run.yaml`, entity `loom.run.Session`, field `commission_run`: "A typed id, not a
  relation: ESS cannot name another system's entity") — inferable. The report covers one process
  segment of the Run: one `run_until_blocked` or `resume_after_grant` call metered by one
  `RunMeter`. Whether it spans a restart is not decided (see Open).
- `RunUsageReport` -> `StepUsage`: a value holding a list of values, ordered by step; no entity,
  no lifecycle.
- Step -> executor call: one step per `AgentExecutor::run` call of the run — inferable (inferred
  from `crates/loom-commission/src/runtime.rs:51`, item 6, and `crates/loom-commission/src/ports/executor.rs`,
  the port's one method; no ess/1 document declares a step).

## Acceptance

A metered run through the SDK reports model calls, tokens and cost per step and in total, and its
call count equals the `turn` calls a counting fake port saw.

- `crates/loom-executor/tests/run_metering.rs`
  `the_reported_calls_equal_the_turns_the_model_port_saw`: a scripted `ModelPort` that counts its
  own `turn` calls, behind `MeteredModelPort`, driven by `run_until_blocked` with a
  `MeteredExecutor` over `LoopExecutor`; the report's total calls equal the port's count, and the
  sum of the steps' calls equals the total.
- `a_step_counts_the_selector_and_argument_calls_made_in_it`: a `Loom` whose
  `ReasoningModelSelector` and model argument generator share the metered port; each step's calls
  equal the turns made during that executor call.
- `tokens_and_cost_per_step_sum_to_the_run_total`: with a rate card, per-step tokens and
  micro-dollar cost add up to the totals.
- `cost_is_absent_without_a_rate_card_and_for_an_unlisted_model`: no card, and a card that does not
  list the model, give an absent cost, never zero.
- `a_failed_turn_is_a_call_without_usage`: a port returning `WireError` raises the call count and
  adds no tokens.

## ESS first

The first commit changes only `ess/`: a new domain `loom.metering` in `ess/domains/metering.yaml`
declaring `loom.metering.StepUsage` (`step`, `model_calls`, `input_tokens`, `output_tokens`,
`cached_input_tokens`, `cost_micro_usd: Optional<Integer>`) and `loom.metering.RunUsageReport`
(`commission_run: loom.run.CommissionRunId`, the same totals, `steps: List<loom.metering.StepUsage>`),
and `loom.metering` listed in `ess/system.yaml`. On it `task drift` fails (the generated
`generated/rust/loom/src/metering.rs` does not exist); later commits regenerate and make
`the_reported_calls_equal_the_turns_the_model_port_saw` pass without changing `ess/`.

## Scope

- `ess/domains/metering.yaml` (new), `ess/system.yaml`
- `generated/rust/loom/` (regenerated: `src/metering.rs`, `src/lib.rs`, `plan.json`, `PLAN.md`)
- `crates/loom-executor/src/metering.rs` (new), `crates/loom-executor/src/lib.rs`
- `crates/loom-executor/tests/run_metering.rs` (new)
- `crates/loom-sdk/src/lib.rs` (re-export)
- `AGENTS.md` (§ Checks that hold the rules, one row), `CHANGELOG.md`, `website/data/status.json`

Touches `ess/domains/` and `generated/rust/loom` (another unit of the running wave also
regenerates `generated/rust/loom`: the two are serialised, or the later one regenerates on top).
Touches none of `crates/loom-intake-slice`, `crates/loom-governor`, `crates/loom-cli`,
`ess/intake/`, `generated/rust/intake`, `crates/loom-mandate`, `ess/domains/authority.yaml`,
`Cargo.toml`, `Cargo.lock`.

Ready now.

## Not in scope

- `b10x-loom run --output jsonl` and the intake slice's measurement report (`crates/loom-cli`,
  `crates/loom-intake-slice`): wiring the report into them is a later story.
- Wire retries inside one `ModelPort::turn` (the HTTP transport's retries): counted by the port,
  not here; the report counts `turn` calls.
- The router's classification call and the plugin turn's model budget, which use llm's port, not
  `ModelPort`.
- Steps named by a workflow definition: `story:dw-workflow-definition-executor` names them.

## Open

- Whether one Run's report spans a restart of the Run (`decision-blocker:dw-run-state-across-restart`).
  This story meters one process segment and does not depend on the answer.
