---
format: aep.planning-md/3
id: story:budget-and-limit-stops-raise-decision-requests
kind: story
status: draft
title: A budget or limit stop of tree run raises a decision request that extends the run once within the hard limit or ends it
summary: budget_overrun and limit_hit of the built-in classes; rule:budget extends once, a second overrun goes to a person, rule_final ends the run, no LLM extends a limit; under epic:tree-daemon-under-enforced-limits
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:decision-routing-by-holder
- depends_on: story:tree-waits-open-decision-requests
- depends_on: story:action-limits-enforced
- depends_on: story:limit-hits-in-run-log
- depends_on: story:tree-usage-and-model-budget
revision: 4
---
## Outcome

When `tree run` stops at a per-run budget or limit, it raises a decision request of a built-in
class instead of ending at once, and the request's holder decides whether the run gets one
extension within the hard limit or ends.

- Which stop raises which class, from the rows of `~/beyond10x/sdc/docs/autonomy-levels.md` § 2.2, rows "budget overrun" and "limit hit":
  a stop at `--max-agent-calls` (`story:agent-node-run-records`) or at
  `--max-model-cost-micro-usd` (`story:tree-usage-and-model-budget`) is `budget_overrun`; a stop at
  `--max-steps`, `--max-wall-seconds` or the per-run CPU ceiling (`story:action-limits-enforced`
  § Outcome) is `limit_hit`. A process stopped at its own memory, CPU, wall or process limit ends
  its step, not the run (`story:limit-hits-in-run-log` § Outcome), and raises no request. A stop
  `CostUnreported` (`story:tree-usage-and-model-budget`) is no spent budget and raises no request.
- The request has two options, both producing no Canon record: `A` extend the spent budget member
  once by `extend_by` (a fraction of the member's value), and `B` end the run. The extension is
  kept in the tree's store and written to the root's run log as a `budget extended` line (member,
  from, to, request). When the extension would pass the member's hard limit, option `A` does not
  exist and no request is raised: the run ends at the budget, exit 5, as today. Extended values are
  rounded down to whole steps, calls, seconds, milliseconds and micro-USD. An extended member is
  the member in force from that line on: the `TreeBudget` that
  `story:capacity-given-to-an-admission-program` hands a program carries the extended value.
- The hard limits: the engine's per-run ceilings for wall time and CPU time
  (`story:action-limits-enforced` § Outcome); for steps, agent calls and model cost,
  `--hard-max-steps`, `--hard-max-agent-calls` and `--hard-max-model-cost-micro-usd`, without which
  those three members are never extended. `extend_by` is `--extend-by` (default none: no
  extension).
- Holders come from the grant table (`story:autonomy-grant-table`): `budget_overrun` person at
  `A0` to `A2`, rule at `A3` and `A4`, rule_final at `A5`; `limit_hit` rule at `A0` to `A4`,
  rule_final at `A5` (session `sdc`, 2026-11-21, point 1). The grant table never gives either class
  a holder that involves the decider, so no LLM extends a budget or a limit
  (`autonomy-levels.md` § 2.2, "Limits are never the decider's"; answer of session `sdc`, 2026-11-21).
- Loom ships the built-in budget rule (`rule:budget`), a `RuleDecider`: under `rule` it proposes
  `A` on the first overrun of a member when the extension fits within the hard limit, and answers
  `Escalate` otherwise, so the second overrun goes to the escalation person
  (`autonomy-levels.md` § 2.2, "Budget rule"); under `rule_final` it proposes `B` and the run ends, extending
  nothing (`autonomy-levels.md` § 3.4, "No limit is extended"). Its one ground is a `path_line` citation of the run log line
  that recorded the stop, source `autonomy_rule`, so the grounding check verifies it.
- A request that waits on a person makes `tree run` exit 3 and print the request and its
  `decisions decide` command; deciding `A` and running `tree run` again goes on under the extended
  budget; deciding `B` makes the next `tree run` exit 5 naming the budget. A rule's `A` goes on in
  the same `tree run`.
- The stop names the spent member by its `TreeBudget` name, the rule of
  `story:tree-usage-and-model-budget`: `max_steps`, `max_agent_calls`, `max_model_cost_micro_usd`,
  `wall_seconds`, `cpu_millis`.

## Why

Session `sdc`, 2026-11-21: `budget_overrun` and `limit_hit` requests are wanted, as a story under
`epic:tree-daemon-under-enforced-limits`; holders per level are the rows at
`autonomy-levels.md` § 2.2, rows "budget overrun" and "limit hit"; `rule` extends once within the hard limit (`limits.extend`), the
second overrun goes to a person, `rule_final` ends the run; a limit hit is never extended by an
LLM. The two classes are built into Loom because Loom raises them (point 9), and
`story:autonomy-grant-table` holds their rows; today `tree run` ends at every budget with exit 5
(`story:tree-run-command` § Outcome). The model cost budget is `story:tree-usage-and-model-budget`'s;
until this story lands its stop ends the run, exit 5, with no request.

## Waits on

- `story:decision-routing-by-holder` (`epic:decision-requests-in-the-runtime`): `route_stop` and
  `RuleDecider` route the request.
- `story:tree-waits-open-decision-requests` (`epic:decision-requests-in-the-runtime`): the tree's
  decision store, the printed `decisions decide` command and the decision events of the run log;
  it changes `crates/loom-tree`, `ess/domains/tree.yaml` and `ess/domains/runlog.yaml` before this
  story.
- `story:action-limits-enforced`: the per-run wall and CPU ceilings, `BudgetSpent` naming the
  spent member, and the engine's `limits` section.
- `story:limit-hits-in-run-log`: the last story of this epic to change `ess/domains/runlog.yaml`
  and the limit events before this one.
- `story:tree-usage-and-model-budget` (`epic:concurrent-and-standing-case-trees`):
  `--max-model-cost-micro-usd`, `TreeBudget.max_model_cost_micro_usd` and its stop, and the
  `agents/1` fixture with its stub adapter; it changes `ess/domains/tree.yaml` (`TreeBudget`) and
  `crates/loom-tree` before this story.

## ESS first

`ess/domains/tree.yaml`: `TreeBudget` gains `extend_by` and the hard values for steps, agent calls
and model cost, and `BudgetExtension` (member, from, to, request) is added.
`ess/domains/runlog.yaml`: `RunLogEvent` gains `BudgetExtended`. On that commit `task drift`
fails, and `crates/loom-tree/tests/budget_requests.rs` does not compile.

## Acceptance

With the fixture tree of `story:tree-run-command`, stub programs, fixture autonomy documents and
no model call:

- At `A3` with `--max-steps 4 --hard-max-steps 12 --extend-by 0.5`, the run reaches 4 steps,
  raises a `limit_hit` request that `rule:budget` decides `A`, writes one `budget extended` line
  from 4 to 6, and goes on; at 6 steps it raises a second request that the rule escalates to the
  escalation person, and `tree run` exits 3 printing its command.
- Deciding that request `A` and running `tree run` again extends from 6 to 9 and goes on; at 9
  steps an extension to 13 would pass 12, so no request is raised and `tree run` exits 5 naming
  `max_steps`. Deciding it `B` instead makes the next `tree run` exit 5 naming `max_steps`.
- At `A5` the same run raises a request that `rule:budget` decides `B`, writes no `budget
  extended` line, and exits 5.
- At `A1` with `--max-agent-calls 2 --hard-max-agent-calls 4 --extend-by 1`, the third agent step
  raises a `budget_overrun` request addressed to the first person; `decisions decide … A` and
  `tree run` take two more agent steps.
- Cost, a library test in `crates/loom-tree/tests/budget_requests.rs` with the `agents/1` fixture
  and the stub adapter of `story:tree-usage-and-model-budget` (three agent writers taken in
  identifier order, each session stating 1500 micro-USD): at `A1` with
  `--max-model-cost-micro-usd 3000 --hard-max-model-cost-micro-usd 6000 --extend-by 1`, the run
  stops before the third session and raises a `budget_overrun` request addressed to the first
  person, naming `max_model_cost_micro_usd`, exit 3; `decisions decide … A` writes one `budget
  extended` line from 3000 to 6000, and the next run takes the third session and reaches its
  terminal outcome, exit 0. Without `--hard-max-model-cost-micro-usd` the same run raises no
  request and exits 5 naming `max_model_cost_micro_usd`.
- Without `--hard-max-steps`, a stop at `--max-steps` raises no request and exits 5 as today.
- A process stopped at its memory limit raises no request (`LimitHit` line only).
- `--override budget_overrun=decider` is refused `NoOverride` before the first step.
- The `rule:budget` proposal passes the grounding check with its run log citation; after one byte
  of that line changes, `tree verify` fails and the check refuses the citation.
- `tree verify` exits 0 after the runs; `task check` exits 0.

## Out of scope

Extending a per-process limit. A budget in currency other than the tree's model cost budget
(`max_usd` of `autonomy-levels.md` § 3.1, `limits.max_usd`). Counting the extension across a daemon
restart (`story:tree-daemon`). Decider call caps. An extension across a wake of a standing tree: a
wake of a `Quiescent` tree starts a new run at the tree's budgets and does not carry it over
(`story:standing-tree-wakes-at-a-host-instant`); a wake of a tree that is not `Quiescent` starts no
new run and keeps it (`story:concurrent-cases-in-the-tree-daemon`).
