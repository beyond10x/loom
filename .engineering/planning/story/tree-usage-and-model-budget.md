---
format: aep.planning-md/3
id: story:tree-usage-and-model-budget
kind: story
status: draft
title: A tree records its usage per run, both wall figures, model calls, tokens and cost, and a tree run stops at a model cost budget
summary: TreeUsage in tree status and on the root's stopped line; cost_micro_usd on agent steps; --max-model-cost-micro-usd stops BudgetSpent, an unknown cost CostUnreported
tags:
- sdc
relations:
- decomposes: epic:concurrent-and-standing-case-trees
- serves: vision:O1
- depends_on: story:run-caps-per-arrival
- depends_on: story:agent-node-run-records
revision: 1
---
## Outcome

A tree records what it used, and a tree run stops at a model cost budget.

`loom.tree.TreeUsage` holds, per tree, each figure counted from the line its budget counts from:

- `steps`; `wall_millis_sum`, the sum of the wall time of every step (what the per-run wall budget
  reads, `story:action-limits-enforced` and `story:tree-daemon`); `wall_millis_elapsed`, from the
  first `action started` line to the last `action finished` line of the same window over every
  case of the tree; `cpu_millis` where the runner reported CPU time;
- `agent_calls` (the `action started` lines marked agent, as `--max-agent-calls` counts them,
  `story:agent-node-run-records`), `model_calls` (its model call records), `input_tokens`,
  `output_tokens` and `cached_input_tokens` (the sums of the reported figures, each absent once one
  call in the window did not report it), and `cost_micro_usd`.

The cost is new in the run log: the `action finished` line of an agent step gains
`cost_micro_usd`, the cost the session executor charged for the session (`SessionExecutor`'s rule:
a session that never started is a known zero, one that stated a cost is charged it, one that
started and stated none has an unknown cost), absent when unknown, never 0. The tree's cost is the
sum over its agent steps, absent once one is unknown.

`TreeBudget` gains `max_model_cost_micro_usd`, set by `tree run --max-model-cost-micro-usd N`
beside `--max-agent-calls`, and counted over the whole tree across runs as `--max-agent-calls` is.
Before each agent step, a tree whose cost has reached the budget stops `BudgetSpent` naming
`model_cost_micro_usd`, exit 5; under that budget, a tree whose cost is unknown stops before its
next agent step `CostUnreported`, naming the agent step whose cost is unknown, exit 5. Steps that
are no agent step are not stopped by it. Together with `--max-agent-calls` this is the tree's
model budget (calls and cost); tokens are recorded and have no budget.

`tree status` prints `usage` per tree, and the root's `stopped` line carries the `TreeUsage` at the
stop; both are the readers later stories and tests use.

## Why

Session `sdc`, 2026-11-21 (`.engineering/drafts/e4-sdc-answers.md`), change 5: "record both
figures per run: the sum of step wall times (compute seconds, the budget) and the elapsed wall time
of the tree run (first step start to last step end). Flow KPIs need the second (speedup against the
sequential run); the budget reads the first." Change 6: "The capacity member a bound admission
program receives carries ... the tree's model budget and what is used (calls and cost, per tree;
tokens if you have them), and the tree stops with a budget reason when it is spent. SDC's driver
enforces `max_llm_calls` over the whole tree today and admission rule 3 in orchestration.md § 2.4
reads the remainder."

What exists, read 2026-11-21: `story:agent-node-run-records` § Outcome records "For each model
call the adapter reports: the adapter, the model, the input, output and cached tokens, and the
duration in milliseconds. A figure the adapter did not report is absent, never 0", and
"`tree run --max-agent-calls N` counts those lines over the logs of the whole tree, across runs,
and stops with exit 5"; no cost. `story:bound-agent-node` bounds one session by
`max_budget_micro_usd`, not a tree. The session executor computes a session's cost in micro-USD
(`crates/loom-session-executor/src/lib.rs:1833-1852`, `:4153-4154`), and the metering domain keeps
"A cost no rate card priced is absent, never zero" (`ess/domains/metering.yaml:12-16`) for a
Commission run, not a tree. `story:tree-daemon` § Outcome counts the per-run wall and CPU totals
"summed from the used times in the `action finished` lines"; no story records the elapsed time.
`story:budget-and-limit-stops-raise-decision-requests` § Outcome turns a stop at
`--max-agent-calls` into a `budget_overrun` request; whether the cost stop joins it is under the
work order's § Open.

## Waits on

- `story:run-caps-per-arrival`: the story before this one on `ess/domains/runlog.yaml`,
  `ess/domains/tree.yaml` (`TreeStop`) and `crates/loom-tree`.
- `story:agent-node-run-records`: the agent step lines, the model call records and
  `--max-agent-calls`.
- Through the chain, `story:tree-daemon` and `story:action-limits-enforced` (`TreeStop::BudgetSpent`
  naming its `limit`, the used wall and CPU times).

Comes before, on shared files: `story:standing-tree-wakes-at-a-host-instant` (`TreeUsage`,
`crates/loom-tree` counters, `ess/domains/tree.yaml`, `ess/domains/runlog.yaml`), and through it
every later story of the epic.

## ESS first

`ess/domains/tree.yaml`: `TreeUsage` (the figures above), `TreeBudget` gains
`max_model_cost_micro_usd`, `TreeStop` gains `CostUnreported` (case, action), `TreeStatus` gains
`usage`. `ess/domains/runlog.yaml`: the agent step's `action finished` gains optional
`cost_micro_usd`, and `stopped` gains optional `usage`. On that commit `task drift` fails and
`crates/loom-tree/tests/tree_usage.rs` does not compile.

## Acceptance

The tests drive the tree library (`crates/loom-tree/tests/tree_usage.rs`), with files added to
`crates/loom-cli/tests/fixtures/area/`: `agents.protocol.yaml` (`agents/1`, three agent writers)
and `agents.bindings.yaml`, a stub agent adapter registered by the test that reports fixed tokens
and a fixed stated cost per session, and stub programs; no model.

- Three agent steps, each reporting 100 input and 20 output tokens and a cost of 1500 micro-USD:
  `tree status` `usage` shows `agent_calls` 3, `input_tokens` 300, `output_tokens` 60,
  `cost_micro_usd` 4500, and each agent step's `action finished` line holds 1500.
- With `--max-model-cost-micro-usd 3000` the run stops `BudgetSpent` naming `model_cost_micro_usd`
  before the third agent step, exit 5; a second `tree run` with the same flag takes no agent step;
  `4500` completes.
- An adapter that reports no cost for the second session: its line has no `cost_micro_usd`, `usage`
  has none, and under `--max-model-cost-micro-usd 10000` the run stops `CostUnreported` naming that
  step's action before the third agent step; without the flag it completes.
- An adapter that leaves out `cached_input_tokens` for one call: `usage` has no
  `cached_input_tokens` and keeps `input_tokens`.
- Two program steps of 2 s each (stub sleeps, host clock): `wall_millis_sum` is at least 4000 and
  equals the sum of the wall times in the two `action finished` lines; `wall_millis_elapsed` equals
  the span from the first `action started` instant to the last `action finished` instant; the
  root's `stopped` line holds the same `usage` as `tree status`.
- `task check` exits 0.

## Files

`ess/domains/tree.yaml`, `ess/domains/runlog.yaml`, generated
`generated/rust/loom/src/{tree,runlog}.rs`; `crates/loom-tree` (the usage counter, the cost stop);
the agent port's line (named by `story:bound-agent-node` and `story:agent-node-run-records`, not
read); `crates/loom-cli/src/lib.rs` (`--max-model-cost-micro-usd`, `usage` in `tree status`);
`crates/loom-tree/tests/tree_usage.rs` (new); `crates/loom-cli/tests/fixtures/area/` (adds
`agents.protocol.yaml`, `agents.bindings.yaml`); `website/docs/reference/cli.md` (regenerated);
`CHANGELOG.md`.

## Out of scope

A token budget. A cost for program steps. Pricing by a rate card (the tree takes the executor's
stated cost). Extending the cost budget through a decision request
(`story:budget-and-limit-stops-raise-decision-requests`). Counting from a wake
(`story:standing-tree-wakes-at-a-host-instant`). Handing the usage to a program
(`story:capacity-given-to-an-admission-program`).
