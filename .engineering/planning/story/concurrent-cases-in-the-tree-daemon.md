---
format: aep.planning-md/3
id: story:concurrent-cases-in-the-tree-daemon
kind: story
status: proposed
title: tree serve gives one run slot to each running case, so admitted siblings run at once with one writer per case and per workspace
summary: Runnable cases queue for serve.run_slots, oldest first, and keep a slot across consecutive steps; a parent steps beside its open children; one step per workspace at a time, the longest waiter next; per-case states Running, Queued, Blocked, HoldsChildren, Waiting, Stuck, Resting, Terminal; action started names open_children; only a wake of a Quiescent tree resets budgets
tags:
- sdc
relations:
- decomposes: epic:concurrent-and-standing-case-trees
- serves: vision:O1
- depends_on: story:tree-daemon
- depends_on: story:child-case-own-workspace
- depends_on: story:standing-tree-rests-quiescent
- depends_on: story:tree-waits-open-decision-requests
scope:
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/bin/read-feed
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/engine-daemon-2.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/hub.bindings.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/hub.protocol.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/item-gated.bindings.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/nest.bindings.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/nest.protocol.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/watch.bindings.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/watch.protocol.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/wave-gated-shared.bindings.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/wave-gated.bindings.yaml
- confidence: cited
  path: crates/loom-cli/tests/tree_concurrent.rs
- confidence: cited
  path: crates/loom-tree
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/engine.yaml
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: cited
  path: ess/domains/tree.yaml
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:29:08Z", actor: "agent:sdc", revision: 14, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

`b10x-loom tree serve` schedules cases, not trees. A case is runnable when it is not terminated,
has a useful action, and waits on no person, authority or decision request. Under the daemon a
case with open children is runnable whenever the selector finds a useful action for it: its claims
over its children's outcomes read `unknown` until the children end, so an action that needs those
outcomes is not useful before. A direct `tree run` keeps its order, every open child before its
parent's next step (`story:tree-run-command` § Outcome); this story changes the daemon's
scheduling only.

**Slots.** Every runnable case of every served tree takes one of the engine's `serve.run_slots`
when its step starts and keeps it across its consecutive steps while it stays runnable; it gives
the slot back when it stops being runnable (it waits, holds children, rests, is stuck or
terminates). A runnable case with no slot queues, oldest first, by the instant it became runnable.
So the admitted siblings of one parent, a parent beside its children, and cases of different
subtrees run at the same time.

**One writer per case, one writer per workspace.** A case has at most one step in flight. Two
cases, of one tree or of two served trees, whose workspaces are the same directory or one inside
the other after symbolic links are resolved never have steps in flight at once: a parent and a
child that inherits its workspace (`story:child-case-own-workspace`) included. A case claims its
workspace only while a step of it is in flight, not across its consecutive steps as it keeps its
slot. When a step ends and runnable cases wait for that workspace, the one that has waited longest
for it steps next, and the case whose step just ended waits behind them. A slot-holder that waits
for its workspace keeps its slot and is `Running`; a runnable case with no slot, while a slot is
free and its workspace is in use, is `Blocked` and takes a slot when the workspace frees.

**The record of a parent step.** The `action started` line of a step taken while the case has
open children gains `open_children`, the ids of the case's children open when the step started;
the member is absent when the case has no open child, so a direct `tree run` never writes it. A
replay can then tell a parent step taken beside its children from one taken after them.

Appends to one case's run log are serialized in the daemon, so children that terminate at once
each add their `child closed` line to the parent's log and the chain verifies. The daemon still
holds the store's lock for the whole tree, and a direct command is still refused `StoreServed`.

**Case states**, which `tree status` prints per case:

- `Running`: holds a slot (a step in flight, between two steps, or waiting for its workspace);
- `Queued`: runnable, no slot, no slot free;
- `Blocked`: runnable, no slot, a slot free, its workspace in use by another case's step;
- `HoldsChildren`: open children, nothing useful, no wait;
- `Waiting`: a person, authority or a decision request, or two legitimate outcomes awaiting
  `tree close`;
- `Stuck`: no open child, nothing useful, no wait, and either an open obligation or, in a bounded
  tree, no legitimate outcome (an open obligation is not needed there: a bounded case with no open
  child, nothing useful and no way on is stuck); also a case stopped `RunsSpent`
  (`story:run-caps-per-arrival`) or ended by a decision (`story:tree-waits-open-decision-requests`).
  A case with an open child is never `Stuck`;
- `Resting`: only in a standing tree, what `story:standing-tree-rests-quiescent` calls at rest: no
  open child, nothing useful, no wait, no open obligation, no legitimate outcome;
- `Terminal`.

**The tree's state** follows from its cases: `Running` while any case holds a slot; `Queued` while
it has `Queued` or `Blocked` cases and none holds a slot; otherwise it stops, in this order:
`Terminal` at the root's termination; `Waiting` (exit 3 for a blocking `tree run`) when at least
one case waits, naming each waiting case and its next decisions; `Stuck` (exit 4) when no case
waits and at least one is stuck, naming each stuck case and its reason; `Quiescent` (exit 7) when
every case left is `Resting` or `HoldsChildren`. A root that holds only resting children therefore
gives `Quiescent`. In a bounded tree `Quiescent` cannot occur: the cases at the ends of its open
subtrees hold no children, cannot rest, and so wait or are stuck. For a tree whose only open case
is the root this order gives exactly what `story:standing-tree-rests-quiescent` gives. It replaces
the per-tree classification of `story:tree-daemon` and that story.

**A wake over per-case states.** This story changes the wake condition of
`story:standing-tree-wakes-at-a-host-instant`, which lands before it and wakes only a tree that is
`Quiescent` as a whole. From this story on, `tree wake --at INSTANT` acts on every `Resting` and
`HoldsChildren` case of a standing tree, whatever the tree's state: a tree with a `Waiting` (or
`Stuck`, `Running`, `Queued`) case is not refused a wake. Each such case goes through the arrival
detection and is evaluated at `INSTANT`; one that then has a useful action is woken and queues for
a slot as a case (oldest first); one that has none keeps its state. The `woken` line names the
woken cases and is written when at least one is woken. Cases in another state are not touched by
the wake: a `Waiting` case gets its arrivals at its next evaluation, after its wait ends. A tree
with no `Resting` or `HoldsChildren` case, or a bounded tree, is answered with its state and
changes nothing. A wake of a tree that was stopped answers when the tree stops again; a wake of a
tree with a case `Running`, `Queued` or `Blocked` answers once the woken cases are evaluated, with
the tree's state then.

**Budgets at a wake.** Only a wake of a tree that was `Quiescent` starts a new run for budget
purposes (session `sdc`, `.engineering/drafts/e4-sdc-answers-3.md`); its `woken` line holds
`new_run: true` (`story:standing-tree-wakes-at-a-host-instant`). A wake of a tree in any other
state appends its `arrived` lines and a `woken` line holding `new_run: false`, resets no counter
of `TreeUsage` and drops no `budget extended` line of
`story:budget-and-limit-stops-raise-decision-requests`. The per-run counters start at the latest
`woken` line that starts a run (`new_run: true`), so a daemon that rebuilds them from the run log
after a restart (`story:tree-daemon` § Outcome: "After a restart the daemon continues every tree
from its store and run log") skips a `woken` line holding `new_run: false`.

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

A parent steps beside its open children because the area case needs it: `wave.admit` counts the
open story children (`~/beyond10x/sdc/docs/orchestration.md` § 2.4a), which it can only do while
they are open. Session `sdc` chose it on 2026-11-21 (`.engineering/drafts/e4-sdc-answers-3.md`): "A.
Under the tree daemon a parent may take a step while it has open children, whenever the selector
finds a useful action for it; `flow` hands the asking case its own open children with their opening
instants; the bounded `tree run` keeps children first", with two conditions: "a parent step and a
child step never write one workspace at the same time ..., and the run log records, for a parent
step taken with open children, which children were open, so a replay can tell the two orders
apart. Second point agreed: only a wake of a Quiescent tree starts a new budget run."

A case keeps its slot across consecutive steps so that a case between two steps is never overtaken
by a queued one: what a program reads about the slots between two steps of one case
(`story:capacity-given-to-an-admission-program`) is then fixed by the cases, not by timing. The
cost is that a long case holds its slot until it waits, holds children, rests or ends. The
workspace claim is held only while a step is in flight, so a parent and its inheriting child, both
holding slots, alternate on their one directory instead of one locking the other out. A standing
tree whose cases rest, hold children and wait independently needs a wake that reaches a resting or
holding case beside a waiting one; without it, a tree with one `Waiting` case would refuse every
wake, and a parent holding children would never see an arrival.

## Waits on

- `story:tree-daemon`: the daemon, its slots and queue, `ServedTree`, `ServeSection` in
  `ess/domains/engine.yaml`.
- `story:standing-tree-rests-quiescent`: `Quiescent` and the rest condition, which this story
  places among the per-case states; it edits the daemon's stop classification in `crates/loom-tree`
  and `TreeStop` in `ess/domains/tree.yaml` before this story.
- `story:standing-tree-wakes-at-a-host-instant` (through `story:child-case-own-workspace`): `Wake`,
  the wake condition and the budget rule this story extends, the `woken` line and its `new_run`,
  and `bin/gate` with the `pulse/1` files.
- `story:child-case-own-workspace`: a workspace per case, `bin/list-dirs` and the `wave/1`,
  `item/1` and `step/1` files; the story before this one on `ess/domains/bindings.yaml`
  (`WorkspaceInvalid`), `ess/domains/tree.yaml` (`TreeStatus`) and `crates/loom-tree`.
- `story:tree-waits-open-decision-requests`: `TreeStop::Stuck`, `TreeStop::Waiting`
  (`NextDecision`) and the per-case `TreeStatus` entry, named above, and the decision waits a case
  can be in. This edge puts this story, and the two after it, behind
  `epic:decision-requests-in-the-runtime`.

Comes before, on shared files: `story:capacity-given-to-an-admission-program`
(`ess/domains/tree.yaml`, `ess/domains/bindings.yaml`, `ess/domains/runlog.yaml` `action started`,
the scheduler's counts in `crates/loom-tree`).

Unordered with this story on shared files, under the store's rule that the second to land rebases
(the epic's § Order lists them): `story:budget-and-limit-stops-raise-decision-requests` and
`story:non-independent-review-becomes-decision-request` (`ess/domains/tree.yaml`,
`ess/domains/runlog.yaml`, `crates/loom-tree`); `story:run-log-references-exec-file-changes`
(`action started` and `action finished` in `ess/domains/runlog.yaml`);
`story:substrate-daemon-on-another-host`, `story:ingress-routes-per-action`,
`story:confinement-level-on-oci-and-pod`, `story:executable-descendants-held` and
`story:agent-session-through-substrate` (`ess/domains/engine.yaml`). No edge is added for them.

## ESS first

`ess/domains/tree.yaml`: `ServedCase` (case, state `Queued`, `Blocked`, `Running`,
`HoldsChildren`, `Waiting`, `Stuck`, `Resting`, `Terminal`), the per-case entry of `TreeStatus`
gains optional `served`, `TreeStop::Waiting` and `TreeStop::Stuck` name a list of cases (each with
its reason), the `ServedTree` lifecycle is described from its cases, and `Wake` is described over
`Resting` and `HoldsChildren` cases. `ess/domains/runlog.yaml`: `action started` gains optional
`open_children` (a list of case ids). `ess/domains/engine.yaml`: `ServeSection.run_slots` is
described as running cases. `ess/domains/bindings.yaml`: `WorkspaceInvalid` gains the reason
`OutsideWorkspaceRoot`. On that commit `task drift` fails, the tree domain's synthesized scenarios
are red, and `crates/loom-cli/tests/tree_concurrent.rs` does not compile.

## Acceptance

Files added to `crates/loom-cli/tests/fixtures/area/` (beside the `wave/1`, `item/1`, `step/1` and
`pulse/1` files, `bin/list-dirs` and `bin/gate` of earlier stories):

- `engine-daemon-2.yaml`: adapter `daemon`, two run slots.
- `wave-gated.bindings.yaml`: `wave.bindings.yaml` with its children bound by
  `item-gated.bindings.yaml`, which binds `out.write` to `bin/gate`;
  `wave-gated-shared.bindings.yaml`: the same without `workspace`.
- `hub.protocol.yaml`, `hub/1`: one `case_open` action `hub.open` opening `watch/1` children from
  an `each` argument; no outcome. `hub.bindings.yaml`: `hub.open` with `arguments_from:
  bin/list-dirs`, `each: dirs` and `workspace: /dir`, its children bound by `watch.bindings.yaml`.
- `watch.protocol.yaml`, `watch/1`: a placed artifact `feed`; an observation `feed.read` (may
  produce `feed_read` about `feed`); an action `feed.answer` bound to a person, admissible when a
  `feed_read` `ask` about the current `feed` is held (may produce `feed_answered` about `feed`);
  the obligation `feed.settled`, discharged by a `feed_read` `quiet` or a `feed_answered` about the
  current `feed`; no outcome. `watch.bindings.yaml`: `feed.read` to `bin/read-feed` (exit 3 when
  the last line of `feed` is `ask`, exit 1 when it is `bad`, else exit 0; `results: {"0": quiet,
  "3": ask, "*": invalid}`), `feed.answer` to `person`. So a child whose `feed` ends `ask` is
  `Waiting`, one whose `feed` ends `bad` is `Stuck` (obligation open, nothing useful), and one
  whose `feed` ends otherwise is `Resting` in a standing tree.
- `nest.protocol.yaml`, `nest/1`: placed artifacts `plan` and `intake`; the observation
  `intake.read` (may produce `intake_read` about `intake`); the `case_open` action `kids.open`,
  whose effect class is not `write`, opening one `item/1` child with subject `plan` (it may produce
  `case_outcome` about `plan`, recorded `open` from the moment the child is opened,
  `story:child-case-runtime`); no obligation, no outcome. Both are non-`write`, so the selector
  takes `intake.read` then `kids.open` (`crates/loom-executor/src/selection.rs:331`, `:336-338`);
  after an append to `intake` only `intake.read` is useful again (`plan` did not move, so
  `case_outcome` about it stays counted). `nest.bindings.yaml`: `intake.read` bound to
  `bin/sleep1`, `kids.open` with `name: kid` and no `workspace`, its child bound by
  `item-gated.bindings.yaml`.

The test's temporary directory is the daemon's `workspace_root`, and every child directory is
directly under it unless a bullet says otherwise. `crates/loom-cli/tests/tree_concurrent.rs` drives
`b10x-loom tree` through a managed Substrate daemon, no model; each bullet names its engine file.
No run-log line orders the steps of two cases against each other (each line carries its run's
`--at` instant), so an order across cases is read from what `bin/gate` writes (its start instant)
and from the test's own releases:

- `engine-daemon-2.yaml`, `wave/1` with `wave-gated.bindings.yaml` and three items `a`, `b`, `c`,
  every `bin/gate` held: `tree status` shows two items `Running`, one `Queued`, the root
  `HoldsChildren` (its outcome needs every child's `case_outcome`, so nothing is useful) and the
  tree `Running`. After the test releases one running item, it keeps its slot for `step.open`, then
  holds children and gives the slot back; the queued item, older than the new grandchild, becomes
  `Running`. After all gates are released, each item is `Terminal`, the root terminates `done`
  after the last, and the tree ends `Terminal`, exit 0.
- `engine-daemon-2.yaml`: a second tree, over its own workspace directory, submitted with `tree run
  --detach` while the three items of the bullet above hold both slots shows `Queued` in `tree
  status` until a slot frees, then runs.
- Oldest first, `engine-daemon-1.yaml`: tree X of `pulse/1` with `pulse-gated.bindings.yaml` is
  submitted and its gate held; then tree A, then tree B, the same, each over its own workspace
  directory, are submitted and show `Queued`. After X's gate is released, X keeps the slot for
  `inbox.sort` and then stops; A's `bin/gate` writes its start instant and B's does not until A
  stops.
- `engine-daemon-2.yaml`, `wave/1` with `wave.bindings.yaml` and the items `a`, `b`, `c`: each
  case's sequence of actions and its final Canon decision equal those of a direct sequential `tree
  run` of the same fixture with the same `--at` values.
- In the run logs of the first bullet, no case's log holds two `action started` lines without an
  `action finished` between them.
- One writer per workspace, one tree, `engine-daemon-2.yaml`: `wave-gated-shared.bindings.yaml`,
  three items: each item's `bin/gate` writes its start instant only after the test released the
  gate that wrote the start instant before it, so no two gates are held at once.
- One writer per workspace, two trees, `engine-daemon-2.yaml`: two trees of `pulse/1` with
  `pulse-gated.bindings.yaml` opened over the same root workspace directory: while the first
  tree's gate is held the second tree's root shows `Blocked` with a slot free, and its gate writes
  no start instant until the first tree's step ends.
- A parent beside its child, `engine-daemon-2.yaml`, `nest/1` with `nest.bindings.yaml` opened
  `--standing`, `tree run --detach --at T`: the root takes `intake.read`, whose `action started`
  line has no `open_children`, then `kids.open`; the child `<root>.kid` holds its gate on
  `out.write` in the root's workspace and the root shows `HoldsChildren`. The test appends a line
  to `intake` and calls `tree wake --at T+60s`, which answers `Running`: the root has an arrival,
  `intake.read` is useful, the root shows `Blocked` (a slot free, its workspace in use), and the
  root's log gains no `action started` line while the gate is held. After the test releases the
  gate, the root's log gains the `action started` line of `intake.read`, which holds
  `open_children: ["<root>.kid"]` (the child was still open), and the child's log then goes on
  with `step.open` after its `out.write` `action finished` line. Which of the root's `intake.read`
  and the child's `step.open` started first is not asserted: no line records it.
- A direct `tree run --at T` of `nest/1` with `nest.bindings.yaml` (no daemon), run in the
  background: while the child's gate is held the test appends a line to `intake`, so the root has
  a useful action while its child is open. After the test releases the gate, the root's log holds
  the `child closed` line of `<root>.kid` before the `action started` line of its second
  `intake.read`, and no `action started` line of the tree holds `open_children`.
- Waiting over Stuck, `engine-daemon-2.yaml`, bounded `hub/1`, children `a` (`feed` ends `ask`)
  and `c` (`feed` ends `bad`): `tree status` shows `a` `Waiting`, `c` `Stuck` with its reason and
  the root `HoldsChildren`; the blocking `tree run` stops `Waiting`, exit 3, naming only `a`.
- A wake over per-case states, `engine-daemon-2.yaml`, standing `hub/1`, children `a` (`feed` ends
  `ask`) and `b` (`feed` ends `hello`): the blocking `tree run --at T` takes `hub.open` and one
  `feed.read` per child and stops `Waiting`, exit 3, `a` `Waiting`, `b` `Resting`; `tree status`
  shows `usage.steps` 3. `tree wake --at T+60s` with no file changed answers `Waiting` and writes
  nothing. After the test appends `more` to `b`'s `feed`, `tree wake --at T+60s` is not refused: it
  writes one `arrived` line in `b`'s log and one `woken` line naming only `b`, with `new_run:
  false`, in the root's log; `b` runs `feed.read` once with instant `T+60s` and is `Resting` again;
  `a`'s run log gains no line; the report answers `Waiting`; `tree status` shows `usage.steps` 4
  and `since_open.steps` 4, as a wake of a tree that was not `Quiescent` resets no counter.
- The same tree, after the bullet above: the test kills the daemon with SIGKILL while no step is in
  flight and starts it again; `tree status` shows `usage.steps` 4 and `since_open.steps` 4, the
  counters rebuilt from the run log past the `woken` line holding `new_run: false`.
- `engine-daemon-2.yaml`: `wave/1` opened `--standing` with `wave.bindings.yaml`, whose items
  terminate: the root's outcome `done` is legitimate, so the root terminates and the tree ends
  `Terminal`, exit 0. With standing `hub/1` and only `b`: the root `HoldsChildren` over the
  `Resting` `b`, and the tree stops `Quiescent`, exit 7.
- `engine-daemon-2.yaml`, `wave/1` with `wave.bindings.yaml` and five items whose steps end within
  one second: each adds one `child closed` line to the root's log, and `tree verify` exits 0.
- `engine-daemon-2.yaml`, `hub/1` with `hub.bindings.yaml` and a `dirs.json` naming one child
  directory `outside/x`, which the test creates beside `workspace_root`, not under it: the opening
  is refused `WorkspaceInvalid` `OutsideWorkspaceRoot`, no child case file is written, and the
  blocking `tree run` exits 4 naming the root.
- `crates/loom-cli/tests/tree_daemon_limits.rs` (`story:tree-daemon`) and
  `crates/loom-cli/tests/tree_wake.rs` (`story:standing-tree-wakes-at-a-host-instant`) pass
  unchanged.
- `task check` exits 0.

## Files

`ess/domains/tree.yaml`, `ess/domains/runlog.yaml` (`open_children`), `ess/domains/engine.yaml`,
`ess/domains/bindings.yaml`, generated `generated/rust/loom/src/{tree,runlog,engine,bindings}.rs`,
`ess/scenarios/` (tree domain); `crates/loom-tree` (the scheduler module of `story:tree-daemon` and
`wake`, not read: the crate does not exist yet); `crates/loom-cli/src/lib.rs` (the per-case states
in `tree status`); `crates/loom-cli/tests/tree_concurrent.rs` (new);
`crates/loom-cli/tests/fixtures/area/` (adds `engine-daemon-2.yaml`, `wave-gated.bindings.yaml`,
`wave-gated-shared.bindings.yaml`, `item-gated.bindings.yaml`, `hub.protocol.yaml`,
`hub.bindings.yaml`, `watch.protocol.yaml`, `watch.bindings.yaml`, `nest.protocol.yaml`,
`nest.bindings.yaml`, `bin/read-feed`); `CHANGELOG.md`. No guide page: `story:tree-daemon` names
none to extend.

## Out of scope

Concurrent cases in a direct `tree run`, which stays sequential with children first. A slot
priority other than oldest first, and giving a slot back between two steps of a runnable case.
Concurrent reads of one case (a fan-out stays one step, `AGENTS.md` § Checks that hold the rules).
Creating or removing work trees.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-tree` (the scheduler module of `story:tree-daemon`, and `wake`) — cited (story § Files; the crate does not exist in the tree at `61fd500b`)
- **Files (ESS):** `ess/domains/tree.yaml`, `ess/domains/runlog.yaml` (`open_children` on `action started`), `ess/domains/engine.yaml`, `ess/domains/bindings.yaml` — cited (story § ESS first). None exists in the tree yet; the earlier stories of the chain create them.
- **Files (tests):** `crates/loom-cli/tests/tree_concurrent.rs` (new) — cited (story § Files)
- **Files (fixtures, new, under `crates/loom-cli/tests/fixtures/area/`):** `engine-daemon-2.yaml`, `wave-gated.bindings.yaml`, `wave-gated-shared.bindings.yaml`, `item-gated.bindings.yaml`, `hub.protocol.yaml`, `hub.bindings.yaml`, `watch.protocol.yaml`, `watch.bindings.yaml`, `nest.protocol.yaml`, `nest.bindings.yaml`, `bin/read-feed` — cited (story § Files and § Acceptance). The directory is created by `story:standing-tree-rests-quiescent`.
- **Symbols:** `ServedTree`, `ServedCase` (new), `TreeStop::{Waiting,Stuck}`, `TreeStatus` per-case `served`, `Wake`, `ServeSection.run_slots`, `WorkspaceInvalid` reason `OutsideWorkspaceRoot` — cited (story § ESS first). None resolves by `git grep` in the tree.
- **Files:** `crates/loom-cli/src/lib.rs`, for the `tree status` per-case print and the exit table — cited (story § Files).
- **Documents:** none. The story names no guide page (§ Files: `story:tree-daemon` names none to extend).
- **Documents (generated):** the generated `tree`, `engine` and `bindings` Rust modules and the reference pages follow from the ESS files — inferred, house-rule files.
- **Confidence:** medium. The story names every file, but the crate, the ESS files and the fixture directory do not exist at this commit. Which module of `crates/loom-tree` holds the scheduler is unknown.
- **Would collide with** (`depends_on` paths read from `aep plan artifact graph --format json`):
  - **Ordered before this story** (transitive `depends_on` path exists): `story:tree-daemon`, `story:standing-tree-rests-quiescent`, `story:standing-tree-wakes-at-a-host-instant`, `story:child-case-own-workspace`, `story:tree-waits-open-decision-requests`, `story:run-caps-per-arrival`, `story:tree-usage-and-model-budget`, `story:tree-run-command`.
  - **Ordered after this story:** `story:capacity-given-to-an-admission-program` (direct `depends_on` this story) on `tree.yaml`, `engine.yaml`, `bindings.yaml`, `crates/loom-tree`, `fixtures/area/`. `story:flow-limits-recorded-as-events` (through the capacity story) on `tree.yaml`, `runlog.yaml`, `crates/loom-tree` and `fixtures/area/`.
  - **Unordered, `tree.yaml` and `crates/loom-tree`:** `story:budget-and-limit-stops-raise-decision-requests` (`TreeBudget`, `crates/loom-tree`). `story:non-independent-review-becomes-decision-request` (`tree.yaml`, a new `crates/loom-tree` module). Both share only the `story:tree-waits-open-decision-requests` ancestor with this story.
  - **Unordered, `tree.yaml` (different member):** `story:authority-records-bound-and-grounded-in-canon` (`CaseApproval`).
  - **Unordered, `engine.yaml` (other sections):** `story:substrate-daemon-on-another-host` (`ConfinementSection`; its body says no edge orders it against the siblings). `story:ingress-routes-per-action`. `story:confinement-level-on-oci-and-pod`. `story:executable-descendants-held`.
  - **Unordered, `engine.yaml` and `bindings.yaml`:** `story:agent-session-through-substrate`.
  - **Unordered, `crates/loom-tree` (new test files or loose mention):** `story:requests-from-canon-pending-decisions`, `story:run-log-references-exec-file-changes`.
  - The two `tree.yaml` plus `crates/loom-tree` collisions are real merge risks. The others add new members or test files, so they are likely clean but unproven.
  - All of this is cited from the stories' own bodies, not from code.
- **Contradicted by the code:** nothing contradicts it. Everything the story builds on is absent from the tree, and the story says so (§ Files: "the crate does not exist yet").
