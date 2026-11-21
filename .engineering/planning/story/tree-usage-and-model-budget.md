---
format: aep.planning-md/3
id: story:tree-usage-and-model-budget
kind: story
status: proposed
title: A tree records its usage per run, both wall figures, model calls, tokens and cost, and a tree run stops at a model cost budget
summary: TreeUsage in tree status and on the root's stopped line; cost_micro_usd on agent steps; --max-model-cost-micro-usd stops BudgetSpent, an unknown cost CostUnreported
tags:
- sdc
relations:
- decomposes: epic:concurrent-and-standing-case-trees
- serves: vision:O1
- depends_on: story:run-caps-per-arrival
- depends_on: story:agent-node-run-records
scope:
- confidence: cited
  path: crates/loom-bindings/src
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: inferred
  path: crates/loom-cli/src/tree.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/agents.bindings.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/agents.protocol.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/bin/sleep2
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/sleeps.bindings.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/sleeps.protocol.yaml
- confidence: inferred
  path: crates/loom-runlog/src
- confidence: cited
  path: crates/loom-tree
- confidence: cited
  path: crates/loom-tree/tests/tree_usage.rs
- confidence: cited
  path: docs/contracts/run-log-v1.json
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: cited
  path: ess/domains/tree.yaml
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:29:07Z", actor: "agent:sdc", revision: 9, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

A tree records what it used, and a tree run stops at a model cost budget.

`loom.tree.TreeUsage` holds, per tree, these figures, each over the window its budget counts:

- the run window: `steps`, the steps of the current `tree run` invocation, which `--max-steps`
  counts (`story:tree-run-command` maps the flag per invocation); `wall_millis_sum`, the sum of
  the wall time of every step, and `cpu_millis` where the runner reported CPU time, both from the
  line the per-run wall and CPU budgets count from (`story:tree-daemon` § Outcome: "for a direct
  `tree run`, the invocation's first step, as before; for a served tree, `tree open`"); and
  `wall_millis_elapsed`, from the first `action started` line to the last `action finished` line
  of that same window over every case of the tree;
- the tree window, over the whole tree across runs as `--max-agent-calls` counts
  (`story:agent-node-run-records`): `agent_calls` (the `action started` lines marked agent),
  `model_calls` (its model call records), `input_tokens`, `output_tokens` and
  `cached_input_tokens` (the sums of the reported figures, each absent once one call in the window
  did not report it), and `cost_micro_usd`.

`story:standing-tree-wakes-at-a-host-instant` moves both windows of a standing tree to its latest
`woken` line that starts a run (`new_run: true`, a wake of a `Quiescent` tree).

The cost is new in the run log: the `action finished` line of an agent step gains
`cost_micro_usd`, absent when unknown, never 0 for an unknown cost. The agent port, not the
adapter, applies the session executor's rule to the session report the adapter returns:
`usage_of` (`crates/loom-session-executor/src/lib.rs:1832-1854`, a free function): a session that
never started is a known zero, one that stated a cost is charged it, one that started and stated
none has an unknown cost. A stub adapter a test registers returns a report as a real one does, so
the rule is the port's in tests too. The cost crosses the agent port in its result:
`loom.bindings.AgentRun` (`story:bound-agent-node` § ESS first) gains optional `cost_micro_usd`,
which `crates/loom-tree` writes to the line. The tree's cost is the sum over its agent steps,
absent once one is unknown.

`TreeBudget` gains `max_model_cost_micro_usd`, set by `tree run --max-model-cost-micro-usd N`
beside `--max-agent-calls`, and counted over the tree window. Before each agent step, a tree whose
cost has reached the budget stops `BudgetSpent`, exit 5; under that budget, a tree whose cost is
unknown stops before its next agent step `CostUnreported`, naming the agent step whose cost is
unknown, exit 5. Steps that are no agent step are not stopped by it. Together with
`--max-agent-calls` this is the tree's model budget (calls and cost); tokens are recorded and have
no budget. These rules were accepted by session `sdc` on 2026-11-21
(`.engineering/drafts/e4-sdc-answers-2.md`, point 3).

The `limit` of `BudgetSpent` names the spent `TreeBudget` member by its exact name
(`story:action-limits-enforced` § ESS first: "`TreeStop::BudgetSpent` gains the member `limit`,
which names the spent `TreeBudget` member"): `max_model_cost_micro_usd` for this budget,
`max_agent_calls` for `story:agent-node-run-records`' cap, `wall_seconds` and `cpu_millis` for
`story:action-limits-enforced`'s. `story:tree-run-command` declares `TreeBudget` without naming
members (`story:action-limits-enforced` § Why); this story names the steps member `max_steps`,
which `--max-steps` sets.

The stop is this story's own: `BudgetSpent` with exit 5 and no decision request.
`story:budget-and-limit-stops-raise-decision-requests`, which depends on this story, later turns it
into a `budget_overrun` request and checks that; `CostUnreported` stays a stop with no request.

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
(`usage_of`, `crates/loom-session-executor/src/lib.rs:1832-1854`, `ExecutorUsage.cost_micro_usd`),
and the metering domain keeps "A cost no rate card priced is absent, never zero"
(`ess/domains/metering.yaml:12-16`) for a Commission run, not a tree. `story:tree-daemon` § Outcome
counts the per-run wall and CPU totals "summed from the used times in the `action finished`
lines"; no story records the elapsed time. The run-log contract `docs/contracts/run-log-v1.json`
is generated from `ess/domains/runlog.yaml` (`story:hash-chained-run-log`), so the two changed
events regenerate it.

## Waits on

- `story:run-caps-per-arrival`: the story before this one on `ess/domains/runlog.yaml`,
  `ess/domains/tree.yaml` (`TreeStop`), `crates/loom-tree` and the agent port.
- `story:agent-node-run-records`: the agent step lines, the model call records and
  `--max-agent-calls`.
- Through the chain, `story:tree-daemon` and `story:action-limits-enforced` (`TreeStop::BudgetSpent`
  naming its `limit`, the used wall and CPU times), and `story:bound-agent-node` (`AgentRun`).

Comes before, on shared files: `story:standing-tree-wakes-at-a-host-instant` (`TreeUsage`,
`crates/loom-tree` counters, `ess/domains/tree.yaml`, `ess/domains/runlog.yaml`), and through it
every later story of the epic; outside the epic,
`story:budget-and-limit-stops-raise-decision-requests` (`TreeBudget`, the cost stop; it depends on
this story).

Unordered with this story on shared files, under the store's rule that the second to land rebases
(the epic's § Order lists them): `story:tree-waits-open-decision-requests` and
`story:non-independent-review-becomes-decision-request` (`ess/domains/tree.yaml`,
`ess/domains/runlog.yaml`, `crates/loom-tree`), `story:authority-records-bound-and-grounded-in-canon`
(`ess/domains/tree.yaml`), and the stories the epic names on `crates/loom-tree` and
`crates/loom-cli/src/lib.rs`. No edge is added for them.

## ESS first

`ess/domains/tree.yaml`: `TreeUsage` (the figures above), `TreeBudget` gains
`max_model_cost_micro_usd` and names its steps member `max_steps`, `TreeStop` gains
`CostUnreported` (case, action), `TreeStatus` gains `usage`. `ess/domains/runlog.yaml`: the agent
step's `action finished` gains optional `cost_micro_usd`, and `stopped` gains optional `usage`.
`ess/domains/bindings.yaml`: `AgentRun` gains optional `cost_micro_usd`. On that commit `task
drift` fails and `crates/loom-tree/tests/tree_usage.rs` does not compile.

## Acceptance

Files added to `crates/loom-cli/tests/fixtures/area/`:

- `agents.protocol.yaml`, `agents/1`: placed artifacts `a`, `b`, `c`; three writers `a.write`,
  `b.write`, `c.write` (effect `write`), each may produce `<x>_written` about its own artifact; one
  outcome, `done`, legitimate when all three kinds are held. `agents.bindings.yaml` binds each
  writer to the agent adapter `stub` with an evidence entry `from: effect`. All three are writers,
  so the selector takes them by identifier, `a.write`, `b.write`, `c.write` (the sort key at
  `crates/loom-executor/src/selection.rs:331`, the least `(writes, id)` at `:336-338`), each once
  (its kind counted after its write).
- `sleeps.protocol.yaml`, `sleeps/1`: a placed artifact `s`; two observations `p.sleep` and
  `q.sleep`, each may produce its own kind about `s`; one outcome, `done`, legitimate when both are
  held. `sleeps.bindings.yaml` binds both to `bin/sleep2` (sleeps 2 s, exit 0). Taken by
  identifier, `p.sleep` then `q.sleep`.

`crates/loom-tree/tests/tree_usage.rs` drives the tree library and reads the returned `TreeStop`
(the command line maps it to the exit status named in brackets) and the `TreeStatus`. It registers
a stub agent adapter that writes its artifact and returns, per session, a report of a started
session with one model call record, 100 input, 20 output and 10 cached input tokens and a stated
cost of 0.0015 USD (1500 micro-USD), unless a bullet says otherwise; no model:

- `agents/1`: the run takes the three sessions and returns `Terminal` `done` (exit 0); `usage`
  shows `agent_calls` 3, `model_calls` 3, `input_tokens` 300, `output_tokens` 60,
  `cached_input_tokens` 30, `cost_micro_usd` 4500, `steps` 3; each agent step's `action finished`
  line holds `cost_micro_usd` 1500.
- `agents/1` with `--max-model-cost-micro-usd 3000`: the run returns `BudgetSpent` with `limit`
  `max_model_cost_micro_usd` before `c.write` (exit 5), with no decision request; a second run
  with the same flag takes no agent step and stops the same way.
- Usage after a second run: the first run of the bullet above, then a run with
  `--max-model-cost-micro-usd 4500` takes `c.write` and returns `Terminal` `done`; `usage` then
  shows `steps` 1 (that invocation's), `agent_calls` 3 and `cost_micro_usd` 4500 (the tree
  window).
- `agents/1` with a stub that returns, for `b.write`, a started session with no stated cost: that
  line has no `cost_micro_usd` and `usage` has none; under `--max-model-cost-micro-usd 10000` the
  run returns `CostUnreported` naming `b.write` before `c.write` (exit 5); without the flag it
  returns `Terminal` `done`.
- `agents/1` with a stub that returns, for `c.write`, a session that never started and writes
  nothing: that line holds `cost_micro_usd` 0, a known zero; `usage` shows `cost_micro_usd` 3000
  and `model_calls` 2; under `--max-model-cost-micro-usd 10000` no `CostUnreported` stop follows.
- `agents/1` with a stub that leaves out `cached_input_tokens` for `a.write`'s call: `usage` has
  no `cached_input_tokens` and keeps `input_tokens` 300.
- `sleeps/1` under the host clock: the run returns `Terminal` `done`; `usage.wall_millis_sum` is at
  least 4000 and equals the sum of the wall times in the two `action finished` lines;
  `wall_millis_elapsed` equals the span from the first `action started` instant to the last
  `action finished` instant; the root's `stopped` line holds the same `usage` figures as
  `tree status` (compared member by member, both read after the run).
- `docs/contracts/run-log-v1.json` is regenerated and names `cost_micro_usd` and `usage`.
- `task check` exits 0.

## Files

`ess/domains/tree.yaml`, `ess/domains/runlog.yaml`, `ess/domains/bindings.yaml` (`AgentRun`),
generated `generated/rust/loom/src/{tree,runlog,bindings}.rs`; `crates/loom-tree` (the usage
counter, the cost stop); the agent port in `crates/loom-bindings/src` (it applies `usage_of`;
neither `story:bound-agent-node` nor `story:agent-node-run-records` names the module);
`crates/loom-cli/src/lib.rs` (`--max-model-cost-micro-usd`, `usage` in `tree status`);
`crates/loom-tree/tests/tree_usage.rs` (new); `crates/loom-cli/tests/fixtures/area/` (adds
`agents.protocol.yaml`, `agents.bindings.yaml`, `sleeps.protocol.yaml`, `sleeps.bindings.yaml`,
`bin/sleep2`); `docs/contracts/run-log-v1.json` (regenerated); `website/docs/reference/cli.md`
(regenerated); `CHANGELOG.md`.

## Out of scope

A token budget. A cost for program steps. Pricing by a rate card (the tree takes the executor's
stated cost). Extending the cost budget through a decision request
(`story:budget-and-limit-stops-raise-decision-requests`). Counting from a wake
(`story:standing-tree-wakes-at-a-host-instant`). Handing the usage to a program
(`story:capacity-given-to-an-admission-program`).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-tree` (usage counter, cost stop) with `ess/domains/tree.yaml` and `ess/domains/runlog.yaml` — cited (story § Files, § ESS first); none exists at `61fd500b`, created by `story:tree-run-command` and `story:hash-chained-run-log`
- **Files:** `ess/domains/tree.yaml`, `ess/domains/runlog.yaml`, `crates/loom-tree`, `crates/loom-cli/src/lib.rs`, `crates/loom-tree/tests/tree_usage.rs` (new) — cited (story § Files)
- **Files:** `crates/loom-cli/tests/fixtures/area/{agents.protocol.yaml,agents.bindings.yaml,sleeps.protocol.yaml,sleeps.bindings.yaml,bin/sleep2}` — cited (story § Files); the directory is created by `story:standing-tree-rests-quiescent`
- **Symbols:** `TreeUsage`, `TreeBudget::max_model_cost_micro_usd`, `TreeStop::CostUnreported`, `TreeStop::BudgetSpent`, `TreeStatus::usage`, `cost_micro_usd` on `action finished`, `usage` on `stopped` — cited (story § Outcome, § ESS first); none exists yet
- **Symbols (read, not changed):** `usage_of` (`crates/loom-session-executor/src/lib.rs:1838`), `ExecutorUsage.cost_micro_usd`, test `only_a_session_that_never_started_costs_a_known_zero` (`lib.rs:4156`) — cited; the story's cost rule is this function, and it already yields known zero / stated / `None`
- **Also likely:** `crates/loom-bindings/src` (the agent port that carries the session's stated cost to the tree) and `ess/domains/bindings.yaml` (`AgentRun` holds no cost) — inferred; the story calls the port "not read" and its ESS first lists no bindings change
- **Also likely:** `docs/contracts/run-log-v1.json` — inferred; the story changes two run-log events, and `story:agent-node-run-records` (Acceptance) regenerates this contract for its events
- **Also likely:** `crates/loom-runlog/src` — inferred; the crate "declares the events" (`story:hash-chained-run-log` Outcome), unclear whether past the generated types
- **Also likely:** `crates/loom-cli/src/tree.rs` — inferred; `story:tree-run-command`'s Scope proposes this module for the tree output, and `usage` in `tree status` would print there
- **Documents:** none beyond the house-rule files — cited (Files names only `cli.md` and `CHANGELOG.md`); no guide page
- **Confidence:** medium — the surface and every named path are cited, but none of the code exists, so no symbol, line or module boundary was read. The cost rule at `usage_of` is real and matches the story's three cases.
- **Would collide with:** (`depends_on` closure from `aep plan artifact graph --format json`)
  - `ess/domains/tree.yaml`, `ess/domains/runlog.yaml`, `crates/loom-tree` and the agent port — cited — against `story:run-caps-per-arrival` (direct `depends_on`, ordered), `story:agent-node-run-records` (direct, ordered), `story:tree-run-command`, `story:hash-chained-run-log`, `story:tree-daemon`, `story:standing-tree-rests-quiescent`, `story:action-limits-enforced`, `story:limit-hits-in-run-log`, `story:engine-configuration-document`, `story:confinement-level-required`, `story:executable-allow-list`, `story:named-egress-per-action` (all ancestors, ordered)
  - the same files and `crates/loom-cli/src/lib.rs`, `fixtures/area` — cited — against descendants `story:standing-tree-wakes-at-a-host-instant` (direct), `story:budget-and-limit-stops-raise-decision-requests` (direct; names `TreeBudget`, the cost stop, and reuses `agents/1` and `sleep2`), `story:child-case-own-workspace`, `story:concurrent-cases-in-the-tree-daemon`, `story:capacity-given-to-an-admission-program`, `story:flow-limits-recorded-as-events`, `story:run-log-references-exec-file-changes` (all ordered by a `depends_on` path)
  - `ess/domains/tree.yaml`, `ess/domains/runlog.yaml`, `crates/loom-tree`, `crates/loom-cli/src/lib.rs` — cited from their bodies — against `story:tree-waits-open-decision-requests`: NO `depends_on` path either way (its body names `tree.yaml`, `runlog.yaml`, `crates/loom-tree`, `lib.rs`)
  - the same four — cited — against `story:non-independent-review-becomes-decision-request` (`tree.yaml`, `runlog.yaml`, a new `loom-tree` module): NO path
  - `ess/domains/tree.yaml` — cited — against `story:authority-records-bound-and-grounded-in-canon` (`CaseApproval`, a different type in one file): NO path
  - `crates/loom-tree` — cited — against `story:requests-from-canon-pending-decisions`, `story:ingress-routes-per-action`, `story:confinement-level-on-oci-and-pod` (each adds its own test or module): NO path, separate symbols
  - `crates/loom-cli/src/lib.rs` — cited — against `story:decision-inbox-commands`, `story:silence-and-expiry-at-a-host-tick`, `story:approval-decisions-resume-held-runs`, `story:question-and-outcome-decisions-hold-runs`, `story:secrets-subcommand`, `story:command-invocation-contract` (other `Command` or flag edits): NO path
  - `crates/loom-bindings/src`, `ess/domains/bindings.yaml` — inferred — against `story:bound-agent-node` (ancestor, ordered) and `story:run-caps-per-arrival` (direct, ordered)
- **Contradicted by code:** `crates/loom-session-executor/src/lib.rs:1833-1852` is not the whole of `usage_of`. The doc comment starts at `:1832` and the function closes at `:1854` — cited
- **Safety fact:** `usage_of` is the only producer of `ExecutorUsage.cost_micro_usd` for sessions, so reading the cost needs no change to the session executor — step 2 (`lib.rs:1838-1854`), unproven. The tree-side path from `SessionReport` to the log line is unproven because the port does not exist.
