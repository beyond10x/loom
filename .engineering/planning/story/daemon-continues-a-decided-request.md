---
format: aep.planning-md/3
id: story:daemon-continues-a-decided-request
kind: story
status: proposed
title: On a served tree, decisions decide, take and tick go through the daemon, and the daemon continues the decided case with no further command
summary: ServedTree gains Decide and Tick in ess/domains/tree.yaml. decisions --tree on a served store goes through the daemon's socket instead of being refused StoreServed (exit 37). The decided case leaves Waiting and takes a slot or queues for one, and the tree's state is derived. It continues after a restart too, and a denied approval stops as a direct tree run would. The host ticks a served tree; a dropped connection exits 36
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:tree-waits-open-decision-requests
- depends_on: story:tree-daemon
- depends_on: story:decisions-resume-the-held-run
- depends_on: story:concurrent-cases-in-the-tree-daemon
scope:
- confidence: cited
  path: crates/loom-cli/src/decisions.rs
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: inferred
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-cli/src/tree.rs
- confidence: cited
  path: crates/loom-cli/tests/tree_daemon_decisions.rs
- confidence: cited
  path: crates/loom-conformance
- confidence: cited
  path: crates/loom-tree
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: cited
  path: ess/scenarios
- confidence: cited
  path: website/docs/guides/decisions.md
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T04:44:12Z", actor: "agent:sdc", revision: 7, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

On a tree that `b10x-loom tree serve` serves (`story:tree-daemon`), a decision recorded with
`decisions … --tree STORE` goes through the daemon. The decided case leaves `Waiting` and
continues with no further command.

**Reaching the daemon:**

- `decisions decide` and `decisions take` with `--tree STORE`, on a store whose lock the daemon
  holds, reach the daemon through its socket. They find it as the `tree` commands do: `--socket
  <path>`, then `B10X_LOOM_TREE_SOCKET`, then `$XDG_RUNTIME_DIR/b10x-loom/tree.sock`
  (`story:tree-daemon` § Outcome).
- They send the new `ServedTree` command `Decide`, carrying the request id, the option or a take,
  the principal, the reasons and the instant (`--at`, or the host clock).
- Before this story the same commands are refused `StoreServed`, exit 37
  (`story:tree-waits-open-decision-requests`). With nothing listening on the socket while the
  daemon holds the lock, they are still refused `StoreServed` with exit 37, naming the socket, as
  a direct `tree run` is.

**What the daemon does with a decision:**

- It applies `Decide` or `TakeRecommendation` on the tree's decision store and completes the
  request, with the writers and run-log lines of `story:tree-waits-open-decision-requests`:
  `CaseApprovalWriter`, `ExplicitDecisionWriter`, and the `decided` and `record produced` lines.
- The decided case leaves `Waiting` and becomes runnable. It takes a free run slot and is
  `Running`, or it queues for one, as `Queued` or `Blocked`
  (`story:concurrent-cases-in-the-tree-daemon` § Slots and its per-case states, stored lines
  68-83 and 96-98).
- The tree's state follows from its cases (that story, stored lines 111-112). Another case of the
  tree may still be `Waiting`.
- A `Waiting` case holds no slot: a case gives its slot back when it waits (that story, stored
  lines 68-71).
- Once the case runs, the daemon treats it as a direct `tree run` would after the same decision
  (story 16). It goes past the wait when the request carries a `produced_record` whose grounds
  still resolve. It supersedes the request and opens a new one when a ground moved, and the case
  is then `Waiting` on the new request. An outcome decided "leave it untaken" goes on directly.
- This is the rule `story:tree-daemon` gives a recorded approval ("makes the daemon continue that
  tree without a further command"), extended to every decision request.
- A denied approval goes the same way. The daemon writes the denied `CaseApproval` and the two
  lines, performs nothing under that capability, and gives the case the stop a direct `tree run`
  gives over the same store after the same denial.
- After a daemon restart, the daemon continues every tree from its store and run log
  (`story:tree-daemon`). A case decided through `Decide` before the restart is runnable again
  after it, with no further command. A request decided and not yet recorded has its record
  completed first (`complete_pending`, as story 16's `tree run` does), and then the case
  continues.

**What the command prints, and its exit status:**

- It prints the inbox's output, then `unblocked: <case> (DR-…)`, then
  `continue: the daemon serving <STORE> continues it` (`story:decisions-resume-the-held-run`).
- The lines are true when printed. If a ground moves before the case runs, the case waits again
  on a new request, which `tree status` lists.
- Exit statuses, as the inbox gives them (`story:decision-inbox-commands`):
  - `0` done;
  - `10` the domain refused the decision, named on standard error;
  - `11` `StoreConflict`;
  - `12` the decision was taken but the daemon's completion of the record failed, and the request
    is Decided without a record;
  - `2` the command line is not valid;
  - `1` anything else.
- Two statuses are this story's or story 16's:
  - `36` (new): the daemon closed the connection before it answered. Whether the decision was
    applied is not known, and `decisions show` tells.
  - `37`: `StoreServed` (story 16).

**Who ticks a served tree: the host.** Someone runs `decisions tick --at INSTANT --tree STORE`, for
example SDC's driver or a cron entry. The daemon calling the tick itself stays the later story
`epic:tree-daemon-under-enforced-limits` names (this epic's § Not in scope).

- On a served store the tick sends the new `ServedTree` command `Tick` (the instant). The daemon
  runs `tick` over the tree's store as story 16 does: the `expired` and silence `decided` lines,
  and the tick's `CaseRevisions` from the governor of each request's case.
- Each case whose request the tick decided by silence and recorded leaves `Waiting` and becomes
  runnable, as after `Decide`.
- An expired request has no decision. When the case is next evaluated, the daemon routes its stop
  again and opens a new request, as story 16's `tree run` does after an `expired` line, and the
  case is `Waiting` on that.
- The command prints the `TickReport` as story 16's `--tree` tick does, then one
  `unblocked: <case> (DR-…)` line per request the tick decided by silence.
- `--resume` on a tree is refused, exit 2, before the store is read. That is
  `story:decisions-resume-the-held-run`'s rule for `--resume` without `--state`. This story adds
  no behaviour there; its bullet is a regression test on a served store.

Every other `decisions` subcommand on a served store answers as story 16 leaves it.

**Files:**

- `ess/domains/tree.yaml`: `Decide` and `Tick` on `ServedTree`. Also the conformance scenarios
  synthesized from them (`ess/scenarios/`, `crates/loom-conformance`), as `story:tree-daemon`'s
  ESS first says for the tree domain's commands.
- `crates/loom-tree`: the daemon's handling of `Decide` and `Tick`, in the daemon module
  `story:tree-daemon` creates.
- `crates/loom-cli/src/decisions.rs` (the socket client path and `--socket`) and
  `crates/loom-cli/src/lib.rs` (the flag and the exit statuses 36 and 37 in the `decisions` help).
- `crates/loom-cli/tests/tree_daemon_decisions.rs` (new).
- `website/docs/guides/decisions.md` and `website/docs/reference/cli.md` (regenerated).

## Why

Session `sdc` answered on 2026-11-21 (`.engineering/drafts/e3-sdc-answers-8.md`). A decision
resumes nothing by itself on a tree, but SDC set two conditions:

- `decide` and `take` under `--tree` print the unblocked case and the command that continues it
  (`story:tree-waits-open-decision-requests`);
- "the daemon's pass over a served tree picks up a decided request without any further person
  action".

The second needs the daemon, and story 16 does not wait on the daemon chain (coordinator's
decision, work order e3r9). So it is its own story, after both, and after per-case scheduling
(work order e3r10).

## Waits on

- `story:tree-waits-open-decision-requests`: `decisions --tree`, its writers, run-log lines and
  continuation rule, and the `StoreServed` refusal this story replaces.
- `story:tree-daemon`: the daemon, its socket, `ServedTree`, the restart rule, and `StoreServed`.
- `story:concurrent-cases-in-the-tree-daemon`: the per-case states (`Waiting`, `Queued`,
  `Blocked`, `Running`), the slots and the derived tree state that `Decide` and `Tick` move a case
  through. It changes `ess/domains/tree.yaml` and `crates/loom-tree` before this story.
- `story:decisions-resume-the-held-run`: the `--tree` lines and the `--resume` refusal. Both
  change `crates/loom-cli/src/decisions.rs` and `lib.rs`.
- Other stories of `epic:tree-daemon-under-enforced-limits` and
  `epic:concurrent-and-standing-case-trees` that change `ess/domains/tree.yaml` or
  `crates/loom-tree` are in the typed scope, and `aep plan artifact waves` keeps them apart.

## ESS first

`ess/domains/tree.yaml`: `ServedTree`, declared by `story:tree-daemon`, gains two commands:

- `Decide`: the request id, an option or `take: true`, the principal, the reasons and the instant.
  The decided case moves from `Waiting` to runnable: `Running` when a slot and its workspace are
  free, otherwise `Queued` or `Blocked` (the per-case states of
  `story:concurrent-cases-in-the-tree-daemon`). The tree's state is derived from its cases.
- `Tick`: the instant. Each case whose request it decides by silence moves as under `Decide`.

On that commit `task drift` fails, the synthesized scenarios for the two commands are red, and
`crates/loom-cli/tests/tree_daemon_decisions.rs` does not compile. `ess/domains/decision.yaml` is
unchanged.

## Acceptance

In `crates/loom-cli/tests/tree_daemon_decisions.rs`, with no model call, and with:

- the three-level fixture tree of `story:tree-run-command`;
- story 16's fixture autonomy documents, principals and recorded decider;
- an engine configuration with the `Daemon` runner, as `story:tree-daemon`'s tests use, and
  `serve.run_slots: 1`;
- where a bullet needs the decided case to wait for a slot, a second served tree whose stub step
  blocks until the test writes a release file, holding the one slot.

The bullets:

- On a served tree whose case is `Waiting` on `DR-…`, `decisions decide DR-… A --as timo --tree
  STORE` exits 0, and its standard error does not name `StoreServed`.
- After that decision, the daemon brings the root to its terminal outcome with no `tree run` and
  no other command: `tree status` shows it `Terminal`.
- That `decide` prints the last two lines `unblocked: <case> (DR-…)` and
  `continue: the daemon serving STORE continues it`.
- With the slot held by the blocking tree, `tree status` right after the `decide` shows the
  decided case `Queued`, holding no slot, and the root not `Terminal`.
- After the release file is written, the root reaches `Terminal` with no further command.
- With `--socket` naming a path nothing listens on, while the daemon holds the store's lock, the
  same `decide` exits 37 naming `StoreServed` and that socket, and the request file is
  byte-identical.
- With the slot held by the blocking tree, the test kills the daemon after the `decide` exited 0.
  `tree status` shows the decided case `Queued` and the root not `Terminal` at the kill. A
  restarted `tree serve`, with the release file then written, brings the root to `Terminal` with
  no further command.
- With the directory holding the case's approval made unwritable by the test before `decide`, the
  `decide` exits 12 and the request is Decided without a record.
- After that, with the directory writable again, a restarted `tree serve` writes one
  `record produced` line in the case's log, and the root reaches `Terminal`.
- A test socket that accepts the connection and closes it without an answer makes `decide` exit 36
  naming the socket.
- Deciding the approval `B` (deny) through the daemon writes a denied `CaseApproval`, and the
  bound executor of that capability records no call.
- After the denial, `tree status` shows the same tree stop as a direct `tree run` over a copy of
  the store after the same `decisions decide … B --tree` with no daemon.
- At `A1`, with an approval taken on a strong recommendation whose `path_line` ground cites lines
  1-3 of `notes.md` in the store directory, and the slot held by the blocking tree: the test
  rewrites those lines after `decisions take` exits 0 and then writes the release file. The
  case's log gains one `superseded` line, and `tree status` shows the case `Waiting` on a new
  request id.
- `decisions take DR-… --as timo --tree STORE` on a strong recommendation leaves the request
  Decided by `timo` through `TakeRecommendation`, and the root then reaches `Terminal`.
- `decisions decide` by a principal not in `eligible` exits 10 naming `NotEligible`, and the case
  stays `Waiting`.
- `decisions tick --at E --tree STORE` on a served tree whose request has a strong default on
  silence due at `E` decides it by `silence`, and the daemon brings the root to `Terminal` with no
  further command.
- That tick prints the `TickReport` as its first line, then exactly one line,
  `unblocked: <case> (DR-…)`, for that request.
- On a served tree whose request has no default, ticked past its `expires_at`, the request is
  Expired, and `tree status` then lists the case `Waiting` on a request id other than the expired
  one.
- `decisions tick --at E --tree STORE --resume` on a served store exits 2 before it reads the
  store, a regression test of `story:decisions-resume-the-held-run`'s rule.
- `decisions decide --help` lists the statuses 36 and 37.
- `task check` exits 0.

## Out of scope

- The slot scheduling itself: the queue order, the workspace rule, keeping a slot across steps
  (`story:concurrent-cases-in-the-tree-daemon`).
- The daemon calling the tick itself (`epic:tree-daemon-under-enforced-limits`).
- `decisions escalate`, `supersede` and `record` through the daemon.
- A slice run (`story:decisions-resume-the-held-run`).
- A web inbox or notifications.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-tree`, the daemon's handling of `Decide` and `Tick` — cited (story § Files)
- **Files:** `ess/domains/tree.yaml` (`Decide`, `Tick` on `ServedTree`) — cited (§ Files, § ESS first)
- **Files:** `ess/scenarios/` and `crates/loom-conformance`, the synthesized scenarios for the two commands — cited (§ Files)
- **Files:** `crates/loom-cli/src/decisions.rs` (socket client path, `--socket`) and `crates/loom-cli/src/lib.rs` (the flag) — cited (§ Files)
- **Files:** `crates/loom-cli/tests/tree_daemon_decisions.rs` (new) — cited (§ Files, § Acceptance)
- **Files:** `crates/loom-cli/src/tree.rs`, where the `tree` commands' socket lookup (`--socket`, `B10X_LOOM_TREE_SOCKET`, `$XDG_RUNTIME_DIR/b10x-loom/tree.sock`) most likely lives and which `decisions` must share — inferred (`story:tree-run-command` scope plans the module; `story:tree-daemon` names no file)
- **Files:** `crates/loom-cli/src/main.rs`, dispatch of the new flag — inferred (`decisions-resume-the-held-run` and `tree-waits-open-decision-requests` both edit it for `decisions`)
- **Symbols:** `ServedTree`, `Decide`, `Tick`, `StoreServed`, `TakeRecommendation`, `complete_pending`, `CaseRevisions`, `TickReport` — cited (story); none is in the tree at `61fd500b`
- **Documents:** `website/docs/guides/decisions.md` — cited (§ Files); the file does not exist yet, so an earlier story creates it (`decision-inbox-commands`, `decisions-resume-the-held-run` list it)
- **Confidence:** medium. Every path the story names is cited, but `ess/domains/tree.yaml`, `crates/loom-tree`, `crates/loom-cli/src/decisions.rs` and the guide do not exist at `61fd500b`. None of the four prerequisite stories is landed. The module inside `crates/loom-tree` is unnamed by any story.
- **Would collide with** (a `depends_on` path from this story to the other means it lands first):
  - `story:tree-daemon`: `ess/domains/tree.yaml`, `crates/loom-tree`, `lib.rs`, `main.rs`. Ordered, direct edge.
  - `story:tree-waits-open-decision-requests`: `decisions.rs`, `lib.rs`, `main.rs`, `tree.yaml`, `crates/loom-tree`. Ordered, direct edge.
  - `story:decisions-resume-the-held-run`: `decisions.rs`, `lib.rs`, `main.rs`, `guides/decisions.md`. Ordered, direct edge.
  - `story:concurrent-cases-in-the-tree-daemon`: `tree.yaml`, `crates/loom-tree`, `lib.rs`. Ordered, direct edge. Its store `scope` lists those three paths.
  - Ordered through the chain, all before this story: `tree-run-command`, `decision-inbox-commands`, `silence-and-expiry-at-a-host-tick`, `question-and-outcome-decisions-hold-runs`, `approval-decisions-resume-held-runs`, `agent-node-run-records`, `engine-configuration-document`, `action-limits-enforced`, `limit-hits-in-run-log`, `tree-usage-and-model-budget`, `run-caps-per-arrival`, `standing-tree-rests-quiescent`, `standing-tree-wakes-at-a-host-instant`, `child-case-own-workspace`, `named-egress-per-action`, `executable-allow-list`, `confinement-level-required`.
  - Not ordered, on `tree.yaml` and `crates/loom-tree`: `budget-and-limit-stops-raise-decision-requests`, `non-independent-review-becomes-decision-request`, `capacity-given-to-an-admission-program`, `flow-limits-recorded-as-events`. `aep plan artifact waves` must keep these apart.
  - Not ordered, on `crates/loom-tree` only: `run-log-references-exec-file-changes`, `ingress-routes-per-action`, `confinement-level-on-oci-and-pod`, `requests-from-canon-pending-decisions`. The last has a `depends_on` edge to `tree-waits-open-decision-requests`, so it is after that story but not ordered against this one.
  - Not ordered, on `tree.yaml` only: `authority-records-bound-and-grounded-in-canon` (`CaseApproval`).
  - Open wave `2026-11-21-w2` (`story:canon-0-9-0-pin`): manifests only, no shared file.
- **Contradicted by its prerequisites:** see "Story defects" 1 to 5 below — cited
- **Safety fact:** none claimed. Since `decisions --tree` on a served store is refused `StoreServed` before this story, no existing caller reaches `Decide` or `Tick` — step 2 (`tree-waits-open-decision-requests` § Outcome), unproven
