---
format: aep.planning-md/3
id: story:run-and-action-budgets
kind: story
status: implemented
title: A run enforces run-wide and per-action budgets, charging what each effect reports it spent
relations:
- decomposes: epic:deterministic-workflows
- serves: vision:governed-autonomy
- informed_by: story:meter-overlapping-calls
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-commission/src/runtime.rs
- confidence: inferred
  path: crates/loom-executor/src/metering.rs
- confidence: inferred
  path: ess/commission/domains/responsibility.yaml
- confidence: inferred
  path: ess/domains/metering.yaml
- confidence: inferred
  path: generated/rust/loom
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-16T16:35:00Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-16T16:35:01Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-17T00:12:47Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

A run carries budgets for the whole run and for each action: steps, input and output tokens, wall
time, cost and a count of repetitions (trials). An effect reports what it spent (its own model
calls' tokens and time) with its `Performed` outcome, and that usage is charged to the run and to
the action. A breach stops the run, and the stop and its cause are recorded.

## Acceptance

`an_effect_that_reports_usage_past_its_action_budget_stops_the_run`
(`crates/loom-commission-testkit/tests/`): a run with `per_action[evaluate].max_tokens_in = 1000`;
an effect reporting 600 input tokens per `Performed`; the second performance breaches, the Run
suspends for `Budget` naming the action and the limit, no third request is invoked, and the run's
usage report shows 1200 tokens charged to that action and to the run.

## Why (Loom 0.22.0)

- `Budget` (`crates/loom-executor/src/harness/turn_loop/budget.rs:14-43`) bounds one model-loop
  run: turns, tokens, duration, cost.
- At run level only `LoopContext::step_budget()` exists
  (`crates/loom-commission/src/runtime.rs:232-238`): a count of steps, nothing per action.
- `EffectOutcomePerformed` carries `report`, `attempt` and `audit` only
  (`ess/commission/domains/responsibility.yaml:286-294`), so an effect that runs dozens of model
  sessions (an evaluation) is unbounded and unmetered by Loom.

## Sketch

`RunBudget { max_steps, max_tokens_in, max_tokens_out, max_duration_ms, max_cost,
per_action: BTreeMap<ActionId, ActionBudget> }`, with a trial count per action;
`EffectOutcomePerformed { .., usage: Optional<Usage> }` reported by the effect, charged by the
runtime and summed into the run meter's report beside model-port usage. ESS first: the budget and
the usage field in `ess/commission/domains/responsibility.yaml` and `ess/domains/metering.yaml`.

## Workaround it removes

The host's effect adapter counts its own trials and tokens, refuses past its budget
(`EffectOutcome::Refused`), and writes the consumption into its own run record.

## Scope (inferred)

`ess/commission/domains/responsibility.yaml`, `ess/domains/metering.yaml`, generated models,
`crates/loom-commission/src/runtime.rs`, `crates/loom-executor/src/metering.rs`, tests,
`CHANGELOG.md` (`EffectOutcomePerformed` gains a field: `Breaking:` for a host that builds it with a
struct literal).

## Not in scope

The model-loop `Budget` itself (unchanged); keeping the cause of a model-loop budget stop
(`story:budget-stop-cause`).
