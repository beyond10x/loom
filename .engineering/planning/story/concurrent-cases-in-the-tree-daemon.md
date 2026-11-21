---
format: aep.planning-md/3
id: story:concurrent-cases-in-the-tree-daemon
kind: story
status: draft
title: tree serve gives one run slot to each running case, so admitted siblings run at once with one writer per case and per workspace
summary: Runnable cases queue for serve.run_slots; per-case states Queued, Running, HoldsChildren, Waiting, Stuck, Resting, Terminal; the tree's stop follows from them; daemon workspaces under workspace_root
tags:
- sdc
relations:
- decomposes: epic:concurrent-and-standing-case-trees
- serves: vision:O1
- depends_on: story:tree-daemon
- depends_on: story:child-case-own-workspace
- depends_on: story:standing-tree-rests-quiescent
- depends_on: story:tree-waits-open-decision-requests
revision: 3
---
## Outcome

`b10x-loom tree serve` schedules cases, not trees. A case is runnable when it is not terminated,
has no open child, has a useful action, and waits on no person, authority or decision request.
Every runnable case of every served tree takes one of the engine's `serve.run_slots` while its step
runs and gives it back when the step ends; the rest queue, oldest first. So the admitted siblings
of one parent, and cases of different subtrees, run at the same time. A parent still takes its
next step only once it has no open child, as in `story:tree-run-command`.

One writer per case: a case has at most one step in flight. One writer per workspace: two cases
whose workspaces are the same directory (a child opened without its own,
`story:child-case-own-workspace`) never have steps in flight at once; the second queues. Appends to
one case's run log are serialized in the daemon, so children that terminate at once each add their
`child closed` line to the parent's log and the chain verifies. The daemon still holds the store's
lock for the whole tree, and a direct command is still refused `StoreServed`.

Each case has a scheduling state, which `tree status` prints per case: `Queued`, `Running`,
`HoldsChildren` (open children, no step of its own), `Waiting` (a person, authority or a decision
request), `Stuck` (nothing useful and an open obligation or no legitimate way on, or stopped
`RunsSpent`, `story:run-caps-per-arrival`, or ended by a decision, `story:tree-waits-open-decision-requests`),
`Resting` (only in a standing tree: what `story:standing-tree-rests-quiescent` calls at rest:
nothing useful, no wait, no open obligation, no legitimate outcome) and `Terminal`.

The served tree's state follows from its cases: `Running` while any case holds a slot; `Queued`
while it has runnable cases and none holds one; otherwise it stops, in this order: `Terminal` at
the root's termination; `Waiting` (exit 3 for a blocking `tree run`) when at least one case waits,
naming each waiting case and its next decisions; `Stuck` (exit 4) when no case waits and at least
one is stuck, naming each stuck case and its reason; `Quiescent` (exit 7) when every case left is
`Resting`. This replaces the per-tree classification of `story:tree-daemon` and
`story:standing-tree-rests-quiescent`, which keeps its meaning for a tree of one open case.

Shared with `story:tree-waits-open-decision-requests`, which lands first: `TreeStop::Stuck`, which
that story gives the ending request and this story makes a list of cases, each with its reason
(the ending request is one such reason); `TreeStop::Waiting`, whose `NextDecision` entries carry
that story's request id; and the per-case entry of `TreeStatus`, which that story gives the open
requests and this story the scheduling state.

The tree's budgets stay one counter each (`story:tree-daemon`, `story:tree-usage-and-model-budget`):
the sum of step wall times counts two steps that ran at once twice, the elapsed wall time counts
their span once, and steps and agent calls count every case's.

Under an engine configuration with `adapter: daemon`, an opening whose child workspace is not
directly under `workspace_root` with a Substrate root name (`story:substrate-daemon-runner`) is
refused at the opening `WorkspaceInvalid`, reason `OutsideWorkspaceRoot`, before any child exists.

## Why

"drive admitted sibling cases at once; one run slot per running case, one writer per case"
(`.engineering/drafts/e4-request.md`, item 2); "The tree daemon's run slots are the natural place
(*inferred*: it schedules trees, not siblings; extend its slot to 'one per running case')"
(`~/beyond10x/sdc/docs/orchestration.md` § 2.4). `story:tree-daemon` § Outcome: "The daemon runs
up to the engine configuration's `serve.run_slots` trees at once and queues the rest", and its
§ Out of scope lists "Running two cases of one tree at once
(`story:concurrent-cases-in-the-tree-daemon`)"; `story:tree-run-command` § Outcome drives "every
open child before its parent's next step". The SDC driver walks children "in sorted order,
recursing depth first", one lock per tree, one shared `work/` (orchestration § 1.1). A daemon
workspace reaches Substrate only by adoption of a directory directly under its `workspace_root`
(`story:substrate-daemon-runner` § Outcome: "A child's workspace directory reaches the daemon by
adoption: its parent must be `workspace_root`"), so a child workspace elsewhere would fail at its
first step; refusing it at the opening keeps the child from existing half made.

## Waits on

- `story:tree-daemon`: the daemon, its slots and queue, `ServedTree`, `ServeSection` in
  `ess/domains/engine.yaml`.
- `story:standing-tree-rests-quiescent`: `Quiescent` and the rest condition, which this story
  places among the per-case states; it edits the daemon's stop classification in `crates/loom-tree`
  and `TreeStop` in `ess/domains/tree.yaml` before this story.
- `story:child-case-own-workspace`: a workspace per case; the story before this one on
  `ess/domains/bindings.yaml` (`WorkspaceInvalid`), `ess/domains/tree.yaml` (`TreeStatus`) and
  `crates/loom-tree`.
- `story:tree-waits-open-decision-requests`: `TreeStop::Stuck`, `TreeStop::Waiting`
  (`NextDecision`) and the per-case `TreeStatus` entry, named above, and the decision waits a case
  can be in. This edge puts this story, and the two after it, behind
  `epic:decision-requests-in-the-runtime`.

Comes before, on shared files: `story:capacity-given-to-an-admission-program` (`ess/domains/tree.yaml`,
`ess/domains/bindings.yaml`, the scheduler's counts in `crates/loom-tree`).

## ESS first

`ess/domains/tree.yaml`: `ServedCase` (case, state `Queued`, `Running`, `HoldsChildren`,
`Waiting`, `Stuck`, `Resting`, `Terminal`), the per-case entry of `TreeStatus` gains optional
`served`, `TreeStop::Waiting` and `TreeStop::Stuck` name a list of cases (each with its reason),
and the `ServedTree` lifecycle is described from its cases. `ess/domains/engine.yaml`:
`ServeSection.run_slots` is described as running cases. `ess/domains/bindings.yaml`:
`WorkspaceInvalid` gains the reason `OutsideWorkspaceRoot`. On that commit `task drift` fails, the
tree domain's synthesized scenarios are red, and `crates/loom-cli/tests/tree_concurrent.rs` does
not compile.

## Acceptance

With `crates/loom-cli/tests/fixtures/area/` (the `wave/1`, `item/1` and `step/1` files of
`story:child-case-own-workspace`), to which this story adds `engine-daemon-2.yaml` (adapter
`daemon`, two run slots), `item-person.bindings.yaml` (an item whose writer is bound to `person`),
`item-stuck.bindings.yaml` (an item with an open obligation and nothing useful) and `bin/gate`, a
stub that writes its start instant and waits until the test writes a release file named by its
case; a managed Substrate daemon; no model (`crates/loom-cli/tests/tree_concurrent.rs`):

- Three items, each in its own directory under `workspace_root`, two run slots, every `bin/gate`
  held: `tree status` shows two items `Running`, one `Queued`, the root `HoldsChildren` and the
  tree `Running`. After the test releases one item, the queued item becomes `Running`; after all
  are released, each item is `Terminal`, the root takes its next step only then, and the tree ends
  `Terminal`, exit 0.
- A second tree submitted while the three items hold both slots shows `Queued` in `tree status`
  until a slot frees, then runs.
- Each case's sequence of actions and its final Canon decision equal those of a direct sequential
  `tree run` of the same fixture with the same `--at` values.
- No case's run log holds two `action started` lines without an `action finished` between them.
- Three items opened without a workspace of their own never have steps in flight at once (the
  start instants and releases show one `bin/gate` held at a time).
- Two items, one bound by `item-person.bindings.yaml`: the other runs to its termination meanwhile;
  `tree status` shows the first `Waiting`; the tree stops `Waiting`, exit 3, naming only that case;
  after `tree approve` the daemon continues it with no `tree run`.
- Two items, one bound by `item-stuck.bindings.yaml`: its sibling terminates; `tree status` shows
  the first `Stuck`; the tree stops `Stuck`, exit 4, naming it and its reason.
- A standing tree (`area/1` opened `--standing`, whose root opens two items) whose items terminate
  and whose root then has nothing to do: the root is `Resting` and the tree stops `Quiescent`,
  exit 7.
- Five items terminating within one second each add one `child closed` line to the root's log, and
  `tree verify` exits 0.
- An opening naming a child workspace outside `workspace_root` is refused `WorkspaceInvalid`
  `OutsideWorkspaceRoot` and no child case file is written.
- `crates/loom-cli/tests/tree_daemon_limits.rs` (`story:tree-daemon`) passes unchanged.
- `task check` exits 0.

## Files

`ess/domains/tree.yaml`, `ess/domains/engine.yaml`, `ess/domains/bindings.yaml`, generated
`generated/rust/loom/src/{tree,engine,bindings}.rs`, `ess/scenarios/` (tree domain);
`crates/loom-tree` (the scheduler module of `story:tree-daemon`, not read: the crate does not exist
yet); `crates/loom-cli/tests/tree_concurrent.rs` (new); `crates/loom-cli/tests/fixtures/area/`
(adds `engine-daemon-2.yaml`, `item-person.bindings.yaml`, `item-stuck.bindings.yaml`,
`bin/gate`); `website/docs/guides/` (the daemon guide page of `story:tree-daemon` shows two items
running); `CHANGELOG.md`.

## Out of scope

Concurrent cases in a direct `tree run`, which stays sequential. A parent stepping while a child is
open. A slot priority other than oldest first. Concurrent reads of one case (a fan-out stays one
step, `AGENTS.md` § Checks that hold the rules). Creating or removing work trees.
