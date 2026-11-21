---
format: aep.planning-md/3
id: story:flow-limits-recorded-as-events
kind: story
status: draft
title: A tree's four flow limits change only by tree limits set from a person or a rule, recorded as limits changed and read at the next admission
summary: max_in_progress, max_open_worktrees, max_unmerged, max_item_age; old and new on the root's run log; flow in loom.capacity; Loom enforces none
tags:
- sdc
relations:
- decomposes: epic:concurrent-and-standing-case-trees
- serves: vision:O1
- depends_on: story:capacity-given-to-an-admission-program
- depends_on: story:standing-tree-rests-quiescent
- depends_on: story:standing-tree-wakes-at-a-host-instant
- depends_on: story:run-caps-per-arrival
revision: 2
---
## Outcome

A tree holds a flow limit set: `max_in_progress`, `max_open_worktrees`, `max_unmerged` (each a
count, at least 0) and `max_item_age` (whole seconds, at least 1), each absent until set (accepted
by session `sdc` on 2026-11-21). It changes only by `b10x-loom tree limits set <store> --as
<principal> --kind person|rule --at INSTANT <limit>=<value>... [<limit>=none]`, through the daemon
(`SetLimits`) when one serves the tree, on the store under its lock otherwise. Each change appends
one `limits changed` line to the root's run log: the principal, its kind, the old and the new
value of every limit it touches, and the instant. A kind other than `person` or `rule` (`decider`,
`silence`, an agent or a model) is refused `LimitChangerRefused` naming the kind; an unknown limit,
a value out of range and a change that changes nothing are refused `LimitInvalid` naming the limit;
`--at` is required; a refusal writes nothing. No executor writes the line and no step's output
changes a limit. `tree limits show <store>` prints the set in force and the changes that made it.

The set in force at a step is the one the root's run log holds when the step starts. A
`loom.capacity` (`story:capacity-given-to-an-admission-program`) gains `flow`: the limits in force,
`at` (the step's instant), and the asking case's open children, each with its case id and the
instant it was opened. A change appended while a step runs reaches the next step, never that one,
and no earlier line changes. Loom counts only what it holds: open children and their ages are in
the capacity; open work trees and unmerged branches are the program's to count with Git, and Loom
carries their limits as values. Loom enforces none of the four.

This story carries the epic's acceptance file and the guide page, which read every story of the
epic; it depends on each story whose outcome they check.

## Why

"Limits, in the area's configuration, enforced by `wave.admit`, never by an LLM:
`limits.wip.max_in_progress` (open story child cases), `limits.wip.max_open_worktrees`,
`limits.wip.max_unmerged` (closed `passed`, branch not merged), `limits.wip.max_item_age`"; "a
change is the recorded event `limits_changed` (who, old, new), by a person or a rule, read at the
next admission and never retroactive. No LLM raises a limit" (`~/beyond10x/sdc/docs/orchestration.md`
§ 2.4a). "The KPIs report each interval under the limit then in force"
(`~/beyond10x/sdc/docs/kpis.md` § 5, notes); F1 counts "child cases opened and not closed" and F6
"age of the oldest open item at the instant" from case events, F3 and F5 from `git worktree list`
and `wave_result` (§ 5, table). The run log is hash-chained and append-only
(`story:hash-chained-run-log` § Outcome: "Lines are never rewritten"), so a change recorded there
is the event the KPIs read and cannot be rewritten later. The principal kinds follow the decision
contract's (`story:silence-and-expiry-at-a-host-tick` uses `silence`), of which only `person` and
`rule` may change a limit here.

## Waits on

- `story:capacity-given-to-an-admission-program`: `Capacity`, which gains `flow`; the story before
  this one on `ess/domains/tree.yaml`, `ess/domains/runlog.yaml` and `crates/loom-tree`.
- `story:standing-tree-rests-quiescent`, `story:standing-tree-wakes-at-a-host-instant` and
  `story:run-caps-per-arrival`: the rest, the wake, the budgets at a wake and the per-arrival count
  that the epic's acceptance file and the guide page check (also reached through the chain; the
  edges are recorded for the checks).
- Through the chain, `story:tree-daemon` (the daemon's commands), `story:child-case-runtime` (the
  open children and their opening instants) and `story:concurrent-cases-in-the-tree-daemon`.

## ESS first

`ess/domains/tree.yaml`: `FlowLimits` (four optional members), `LimitChangerKind` (`Person`,
`Rule`), the command `SetLimits` (store, principal, kind, instant, changes), `LimitChange` (limit,
old, new), the refusals `LimitChangerRefused` (kind) and `LimitInvalid` (limit, reason), and
`Capacity` gains `flow` (`limits`, `at`, open children). `ess/domains/runlog.yaml`: the event
`limits changed` (principal, kind, changes). On that commit `task drift` fails, the scenario for
`SetLimits` is red, and `crates/loom-cli/tests/tree_flow_limits.rs` does not compile.

## Acceptance

With `crates/loom-cli/tests/fixtures/area/` (the `admit/1` files of
`story:capacity-given-to-an-admission-program`, to which this story adds `admit-open.protocol.yaml`,
an admission tree whose root first opens two `item/1` children that wait on `bin/gate`), stub
programs, a managed Substrate daemon, no model (`crates/loom-cli/tests/tree_flow_limits.rs`):

- `tree limits set --as alice --kind person --at T max_in_progress=2` appends one `limits changed`
  line naming `alice`, `person`, old absent, new 2, instant `T`; `tree limits show` prints it.
- `--kind decider`, `--kind silence` and `--kind agent` are refused `LimitChangerRefused` naming
  the kind, and the root's log is byte-identical; the same command without `--at` exits 2.
- `max_in_progress=-1`, `max_item_age=0`, `max_parallel=3` and a change to the value in force are
  refused `LimitInvalid` naming the limit, writing nothing.
- An admission stub that copies its standard input sees `flow.limits.max_in_progress` 2, `at`, and
  the root's two open children with their opening instants.
- Through the daemon, a change made while the admission stub is held by `bin/gate` is absent from
  that step's capacity and present in the next admission's; the earlier `action started` line is
  unchanged.
- `tree limits set --as alice --kind person --at T2 max_in_progress=none` removes the limit; the
  next capacity omits it.
- No program step can add a `limits changed` line: a stub printing one to standard output leaves
  the log without it, and `crates/loom-tree` writes the line from `SetLimits` only (a test greps).

The epic's acceptance checks pass in `crates/loom-cli/tests/tree_concurrent_standing.rs` with the
fixtures under `crates/loom-cli/tests/fixtures/area/`.

- `website/docs/reference/cli.md` is regenerated and a guide page shows a standing tree, a wake,
  a writer stopped `RunsSpent` and a limit change with pasted output.
- `task check` exits 0.

## Files

`ess/domains/tree.yaml`, `ess/domains/runlog.yaml`, generated
`generated/rust/loom/src/{tree,runlog}.rs`, `ess/scenarios/` (tree domain); `crates/loom-tree`
(the limit set, `SetLimits`, `flow` in the capacity); `crates/loom-cli/src/lib.rs` (`tree limits`);
`crates/loom-cli/tests/tree_flow_limits.rs`, `crates/loom-cli/tests/tree_concurrent_standing.rs`
(new); `crates/loom-cli/tests/fixtures/area/` (adds `admit-open.protocol.yaml` and its bindings);
`website/docs/reference/cli.md` (regenerated), a guide page under `website/docs/guides/` (name not
chosen); `CHANGELOG.md`.

## Out of scope

Loom refusing an opening or a step on a flow limit. Counting work trees or unmerged branches.
Computing F1 to F17. Initial limits from the engine configuration or a file at `tree open`.
Authenticating `--as` beyond the local user.
