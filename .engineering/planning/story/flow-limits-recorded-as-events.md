---
format: aep.planning-md/3
id: story:flow-limits-recorded-as-events
kind: story
status: proposed
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
scope:
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: inferred
  path: crates/loom-cli/src/tree.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area
- confidence: cited
  path: crates/loom-cli/tests/tree_concurrent_standing.rs
- confidence: cited
  path: crates/loom-cli/tests/tree_flow_limits.rs
- confidence: inferred
  path: crates/loom-conformance/src/lib.rs
- confidence: inferred
  path: crates/loom-conformance/src/tree.rs
- confidence: cited
  path: crates/loom-tree
- confidence: cited
  path: docs/contracts/run-log-v1.json
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: cited
  path: ess/domains/tree.yaml
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:29:08Z", actor: "agent:sdc", revision: 9, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
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
a value out of range and a change that changes nothing are refused `LimitInvalid` naming the limit.
Both refusals exit 1 naming the reason and write nothing; a missing `--at` is a usage error and
exits 2. No executor writes the line and no step's output changes a limit.

`tree limits show <store>` prints one JSON object on standard output and exits 0: `limits`, the
set in force, one member per limit that is set (an absent limit has no member); and `changes`, the
`limits changed` lines in log order, each `{"principal", "kind", "at", "changes": [{"limit",
"old", "new"}]}` with `old` or `new` `null` where the limit was absent.

The limits come from the root's run log and from nowhere else. Reading them from the area's
configuration, where SDC keeps them (`~/beyond10x/sdc/docs/orchestration.md:179-180`: "Limits, in
the area's configuration, enforced by `wave.admit`"), is not implemented by this story; until a
story does, the area sets them with `tree limits set`.

The set in force at a step is the one the root's run log holds when the step starts. A
`loom.capacity` (`story:capacity-given-to-an-admission-program`) gains `flow`: `limits` (the set
in force, the same form as `tree limits show`'s), `at` (the step's instant, the `at` of its
`action started` line), and `open_children`, the asking case's own open children, each with its
case id and the instant it was opened. Under the tree daemon a parent steps while it has open
children (`story:concurrent-cases-in-the-tree-daemon`), so an admission on the root sees the
children still open; a direct bounded `tree run` takes every open child before its parent's next
step, so there the list is empty once the children end. A change appended while a step runs
reaches the next step, never that one, and no earlier line changes. Loom counts only what it
holds: open children and their ages are in the capacity; open work trees and unmerged branches are
the program's to count with Git, and Loom carries their limits as values. Loom enforces none of the
four.

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
`rule` may change a limit here. Session `sdc`, 2026-11-21 (`.engineering/drafts/e4-sdc-answers-3.md`):
"`flow` hands the asking case its own open children with their opening instants; the bounded `tree
run` keeps children first. That is what the area case needs: it admits the next wave on a new
arrival while earlier stories are still open".

## Waits on

- `story:capacity-given-to-an-admission-program`: `Capacity`, which gains `flow`, `bin/copy-stdin`
  and the `admit/1` files; the story before this one on `ess/domains/tree.yaml`,
  `ess/domains/runlog.yaml` and `crates/loom-tree`.
- `story:standing-tree-rests-quiescent`, `story:standing-tree-wakes-at-a-host-instant` and
  `story:run-caps-per-arrival`: the rest, the wake, the budgets at a wake and the per-arrival count
  that the epic's acceptance file and the guide page check (also reached through the chain; the
  edges are recorded for the checks).
- Through the chain, `story:tree-daemon` (the daemon's commands), `story:child-case-runtime` (the
  open children and their opening instants), `story:child-case-own-workspace` (`bin/list-dirs`,
  `item/1`, `item.bindings.yaml`) and `story:concurrent-cases-in-the-tree-daemon` (a parent
  stepping beside open children, `item-gated.bindings.yaml`).

Unordered with this story on shared files, under the store's rule that the second to land rebases
(the epic's § Order lists them): `story:budget-and-limit-stops-raise-decision-requests`,
`story:non-independent-review-becomes-decision-request` and
`story:run-log-references-exec-file-changes` (`ess/domains/tree.yaml`, `ess/domains/runlog.yaml`,
`crates/loom-tree`), `story:requests-from-canon-pending-decisions`,
`story:confinement-level-on-oci-and-pod` and `story:ingress-routes-per-action` (`crates/loom-tree`),
`story:authority-records-bound-and-grounded-in-canon` (`ess/domains/tree.yaml`), and
`story:agent-selects-the-connectors-relay`, `story:secrets-subcommand` and
`story:command-invocation-contract` (`crates/loom-cli/src/lib.rs`). No edge is added for them.

## ESS first

`ess/domains/tree.yaml`: `FlowLimits` (four optional members), `LimitChangerKind` (`Person`,
`Rule`), the command `SetLimits` (store, principal, kind, instant, changes), `LimitChange` (limit,
old, new), the refusals `LimitChangerRefused` (kind) and `LimitInvalid` (limit, reason), the view
`LimitsShown` (`limits`, `changes`), and `Capacity` gains `flow` (`limits`, `at`,
`open_children`, each a case id and an instant). `ess/domains/runlog.yaml`: the event `limits
changed` (principal, kind, changes). On that commit `task drift` fails, the scenario for
`SetLimits` is red, and `crates/loom-cli/tests/tree_flow_limits.rs` does not compile.

## Acceptance

Files added to `crates/loom-cli/tests/fixtures/area/` (beside the `admit/1` files and
`bin/copy-stdin` of `story:capacity-given-to-an-admission-program`, `bin/list-dirs`, `item/1` and
`item.bindings.yaml` of `story:child-case-own-workspace`, and `item-gated.bindings.yaml` of
`story:concurrent-cases-in-the-tree-daemon`):

- `admit-open.protocol.yaml`, `admit-open/1`: placed artifacts `plan` and `intake`; the `case_open`
  action `kids.open`, whose effect class is not `write`, opening `item/1` children from an `each`
  argument with subject `plan` (it may produce `case_outcome` about `plan`, recorded `open` from
  the moment a child is opened, `story:child-case-runtime`); the observation `wave.admit` (may
  produce `admitted` about `intake`); no obligation, no outcome. Both are non-`write`, so the
  selector takes `kids.open` then `wave.admit` (`crates/loom-executor/src/selection.rs:331`,
  `:336-338`); after an append to `intake` (an arrival, `story:run-caps-per-arrival`) only
  `wave.admit` is useful again, because `plan` did not move.
- `admit-open.bindings.yaml`: `kids.open` with `arguments_from: bin/list-dirs`, `each: dirs` and
  `workspace: /dir`, its children bound by `item.bindings.yaml`; `wave.admit` bound to
  `bin/copy-stdin` with `capacity: true`. `admit-open-gated.bindings.yaml`: the same with the
  children bound by `item-gated.bindings.yaml` and `wave.admit` bound to `bin/gate-copy`, which
  prints its standard input to standard output, then waits until the test writes the release file
  `release-<n>` in its working directory, the root's workspace, and exits 0. Its environment is
  cleared and its standard input names no count, so it learns `n`, the count of its runs, from a
  file it keeps in that directory: at each start it appends one line to `gate-copy.runs` and takes
  the number of lines. `admit-forge.bindings.yaml`: `admit.bindings.yaml` with `wave.admit` bound
  to `bin/print-limits-changed`, which prints a JSON object shaped as a `limits changed` line
  (`max_in_progress` from absent to 9) and exits 0.
- `engine-daemon-3.yaml`: adapter `daemon`, three run slots.

The test's temporary directory is the daemon's `workspace_root`; `dirs.json` names two entries,
`{"name": "a", "dir": <a/>}` and `{"name": "b", "dir": <b/>}`, each a directory under it and apart
from the root's. `crates/loom-cli/tests/tree_flow_limits.rs` drives `b10x-loom tree` with stub
programs and, where a bullet says so, a managed Substrate daemon, no model; each bullet names its
engine file:

- `tree limits set --as alice --kind person --at T max_in_progress=2` on a tree of `admit/1`
  opened under `engine-limits.yaml`, no daemon: one `limits changed` line naming `alice`,
  `person`, `max_in_progress` old absent and new 2, instant `T`; then `tree limits show` exits 0
  and prints `{"limits": {"max_in_progress": 2}, "changes": [{"principal": "alice", "kind":
  "person", "at": T, "changes": [{"limit": "max_in_progress", "old": null, "new": 2}]}]}`.
- The same tree: `--kind decider`, `--kind silence` and `--kind agent` each exit 1 naming
  `LimitChangerRefused` and the kind, and the root's run log is byte-identical before and after.
- The same tree: the same command without `--at` exits 2 and the root's run log is byte-identical.
- The same tree, after the first bullet: `max_in_progress=-1`, `max_item_age=0`, `max_parallel=3`
  and `max_in_progress=2` each exit 1 naming `LimitInvalid` and the limit, and the root's run log
  is byte-identical.
- Through the daemon under `engine-daemon-3.yaml`, `admit-open/1` with
  `admit-open-gated.bindings.yaml` opened `--standing`, `tree run --detach --at T`: the first
  admission, taken while both children are open (whether their gates have started is not
  asserted), has a copied object (read once the test releases it, as in the next bullet) whose
  `flow.open_children` holds `<root>.a` and `<root>.b`, each with an opening instant no later than
  `flow.at`, and whose `flow.limits` is `{}`.
- The same tree: while the first admission is held at its gate, `tree limits set --as alice
  --kind person --at T+1s max_in_progress=2` exits 0 through the daemon; the test then releases
  the first admission. The first admission's `action started` line holds no `max_in_progress` in
  `capacity.flow.limits`.
- The same tree, after the bullet above: the test appends a line to `intake` and calls `tree wake
  --at T+60s` (the root `HoldsChildren`, the children still held); once the test releases it, the
  second admission's copied object holds `flow.limits.max_in_progress` 2 and the same two open children.
- Direct `tree run --at T` of `admit-open/1` with `admit-open.bindings.yaml` under
  `engine-limits.yaml`, no daemon: the admission's copied object holds `flow.open_children` as an
  empty list (both children ended before the root's next step).
- The tree of the first bullet, after `tree limits set --as alice --kind person --at T2
  max_in_progress=none`: `tree limits show` prints `limits` as `{}`, and a direct `tree run --at
  T3` of it with `admit.bindings.yaml` gives an admission whose `flow.limits` has no
  `max_in_progress`.
- Direct `tree run --at T` of `admit/1` with `admit-forge.bindings.yaml` under
  `engine-limits.yaml`: the root's run log holds no `limits changed` line and `tree limits show`
  prints `{"limits": {}, "changes": []}`.
- `limits_changed_is_written_only_by_set_limits` in `crates/loom-cli/tests/tree_flow_limits.rs`
  reads every `.rs` file under `crates/loom-tree/src` and finds the `limits changed` event built in
  the `SetLimits` handler and nowhere else.

The epic's acceptance checks pass in `crates/loom-cli/tests/tree_concurrent_standing.rs` with the
fixtures under `crates/loom-cli/tests/fixtures/area/`.

- `website/docs/reference/cli.md` is regenerated and a guide page shows a standing tree, a wake,
  a writer stopped `RunsSpent` and a limit change with pasted output.
- `task check` exits 0.

## Files

`ess/domains/tree.yaml`, `ess/domains/runlog.yaml`, generated
`generated/rust/loom/src/{tree,runlog}.rs`, `ess/scenarios/` (tree domain); `crates/loom-tree`
(the limit set, `SetLimits`, `flow` in the capacity); `crates/loom-cli/src/lib.rs` (`tree limits
set` and `tree limits show`); `crates/loom-cli/tests/tree_flow_limits.rs`,
`crates/loom-cli/tests/tree_concurrent_standing.rs` (new); `crates/loom-cli/tests/fixtures/area/`
(adds `admit-open.protocol.yaml`, `admit-open.bindings.yaml`, `admit-open-gated.bindings.yaml`,
`admit-forge.bindings.yaml`, `engine-daemon-3.yaml`, `bin/gate-copy`, `bin/print-limits-changed`);
`docs/contracts/run-log-v1.json` (regenerated); `website/docs/reference/cli.md` (regenerated), a
guide page under `website/docs/guides/` (name not chosen); `CHANGELOG.md`.

## Out of scope

Loom refusing an opening or a step on a flow limit. Counting work trees or unmerged branches.
Computing F1 to F17. Initial limits from the engine configuration, the area's configuration or a
file at `tree open`. Authenticating `--as` beyond the local user.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-tree` (the limit set, `SetLimits`, `flow` in the capacity) — cited (story § Files, § Outcome). The crate does not exist at `61fd500b`: `story:tree-run-command` creates it (its § Outcome and scope).
- **Files:** `ess/domains/tree.yaml`, `ess/domains/runlog.yaml` — cited (story § ESS first, § Files). Neither exists at `61fd500b`: the tree domain comes from `story:tree-run-command`, `runlog.yaml` from `story:hash-chained-run-log`.
- **Files:** `crates/loom-cli/src/lib.rs` (`tree limits set|show`) — cited (story § Files). The file exists; it has no `tree` command yet.
- **Files:** `crates/loom-cli/tests/tree_flow_limits.rs`, `crates/loom-cli/tests/tree_concurrent_standing.rs` (both new) — cited (story § Files, § Acceptance)
- **Files:** `crates/loom-cli/tests/fixtures/area/` (adds `admit-open.protocol.yaml` and its bindings) — cited (story § Files, § Acceptance). The directory is created by `story:capacity-given-to-an-admission-program`.
- **Symbols:** `FlowLimits`, `LimitChangerKind`, `SetLimits`, `LimitChange`, `LimitChangerRefused`, `LimitInvalid`, `Capacity.flow`, event `limits changed` — cited (story § ESS first). None exists in the tree: `git grep` finds no `SetLimits` and no `loom-tree` outside `.engineering`.
- **Symbols:** `Capacity` (`ess/domains/tree.yaml`) — cited (`story:capacity-given-to-an-admission-program` § ESS first, line 83)
- **Also likely:** `crates/loom-cli/src/tree.rs` (tree output and exit mapping) — inferred. `story:tree-run-command`'s scope lists it as inferred and it does not exist yet.
- **Also likely:** `crates/loom-conformance/src/lib.rs`, `crates/loom-conformance/src/tree.rs` — inferred. The story says the `SetLimits` scenario goes red, so a conformance handler is needed. `story:tree-run-command` § scope gives the same two paths as "only if `tree.yaml` declares a command".
- **Also likely:** `ess/scenarios/<tree domain directory>` — inferred. The story says "ess/scenarios/ (tree domain)" and `story:tree-run-command` names `ess/scenarios/tree-run/`, so the directory name is not settled and no path is given in the block below.
- **Documents:** `website/docs/guides/<new page>` — cited that it exists (story § Acceptance, § Files), name "not chosen". `website/docs/reference/cli.md` is regenerated and is a house-rule file.
- **Confidence:** medium. The story names every file and symbol (cited), but the crate, both domain files and the fixture directory exist only in unmerged stories of the chain. Nothing here could be checked against code.
- **Would collide with:** `crates/loom-tree`, `ess/domains/tree.yaml`, `ess/domains/runlog.yaml`, `crates/loom-cli/src/lib.rs`, `crates/loom-cli/tests/fixtures/area/`. Read from the dependency graph (`aep plan artifact graph --format json`, a transitive `depends_on` walk), on the stories' bodies.
  - Already ordered, because each is reachable from this story by `depends_on` and so lands first:
    - `story:capacity-given-to-an-admission-program` (`Capacity`, `tree.yaml`, `runlog.yaml`, `loom-tree`, `fixtures/area/`)
    - `story:standing-tree-rests-quiescent` (`tree.yaml` `TreeStop`, `loom-tree`, `lib.rs`, `fixtures/area/`)
    - `story:standing-tree-wakes-at-a-host-instant` (`tree.yaml`, `runlog.yaml`, `loom-tree`, `lib.rs`, `fixtures/area/`)
    - `story:run-caps-per-arrival` (`tree.yaml`, `runlog.yaml`, `loom-tree`, `lib.rs`, `fixtures/area/`)
    - `story:tree-usage-and-model-budget` (same files)
    - `story:child-case-own-workspace` (`tree.yaml`, `loom-tree`, `fixtures/area/`)
    - `story:concurrent-cases-in-the-tree-daemon` (`tree.yaml`, `loom-tree`, `fixtures/area/`)
    - `story:tree-daemon` (`tree.yaml`, `loom-tree`, `lib.rs`)
    - `story:tree-run-command` (`loom-tree`, `tree.yaml`, `lib.rs`, `lib.rs` tests)
    - `story:hash-chained-run-log` (`runlog.yaml`)
    - `story:limit-hits-in-run-log`, `story:agent-node-run-records`, `story:tree-waits-open-decision-requests`, `story:engine-configuration-document`, `story:action-limits-enforced` (`tree.yaml`, `runlog.yaml` or `loom-tree`)
    - `story:decision-inbox-commands`, `story:silence-and-expiry-at-a-host-tick`, `story:approval-decisions-resume-held-runs`, `story:question-and-outcome-decisions-hold-runs`, `story:decision-suspension-reason`, `story:bound-commands-run-confined`, `story:confinement-opens-from-engine-config` (`lib.rs`)
    - `story:confinement-level-required`, `story:executable-allow-list`, `story:named-egress-per-action` (`loom-tree`)
  - NOT ordered: no `depends_on` path runs either way. This is the list the coordinator must act on.
    - `story:budget-and-limit-stops-raise-decision-requests` (`tree.yaml`, `runlog.yaml`, `loom-tree`; its § Waits on says it edits them "before this", and it does not mention this story).
    - `story:non-independent-review-becomes-decision-request` (`decision.yaml`, `runlog.yaml`, `tree.yaml`, a new `loom-tree` module).
    - `story:run-log-references-exec-file-changes` (`runlog.yaml`, `loom-tree`, `loom-tree/tests`).
    - `story:requests-from-canon-pending-decisions` (`loom-tree`).
    - `story:authority-records-bound-and-grounded-in-canon` (`tree.yaml` `CaseApproval`).
    - `story:confinement-level-on-oci-and-pod`, `story:ingress-routes-per-action` (`loom-tree` plus its own test files; each depends on `story:tree-daemon` only).
    - `story:agent-selects-the-connectors-relay`, `story:secrets-subcommand`, `story:command-invocation-contract` (`crates/loom-cli/src/lib.rs`).
  - Nothing depends on this story. `story:capacity-given-to-an-admission-program` § "Comes before" names it, but no `depends_on` edge points at it.
- **Safety fact:** adding `flow` to `Capacity` touches every reader of `loom.capacity`, which are the bound-program stdin and the `action started` line of `story:capacity-given-to-an-admission-program` — step 1 (stated by the story), unproven. The `limits changed` line extends the hash-chained run log; the chain verifier and `docs/contracts/run-events-v1.json` must accept an event kind they do not know yet — step 1, unproven.
