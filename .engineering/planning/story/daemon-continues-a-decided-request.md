---
format: aep.planning-md/3
id: story:daemon-continues-a-decided-request
kind: story
status: draft
title: On a served tree, decisions decide, take and tick go through the daemon, and the daemon continues the decided case with no further command
summary: ServedTree gains Decide and Tick in ess/domains/tree.yaml; decisions --tree on a served store goes through the daemon's socket instead of being refused StoreServed; the daemon's next pass continues the case, after a restart too; a denied approval stops as a direct tree run would; the host ticks a served tree, and --resume on a tree is refused
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
revision: 4
---
## Outcome

On a tree that `b10x-loom tree serve` serves (`story:tree-daemon`), a decision recorded with
`decisions … --tree STORE` goes through the daemon, and the daemon's next pass continues the
decided case with no further command.

- `decisions decide` and `decisions take` with `--tree STORE`, on a store whose lock the daemon
  holds, reach the daemon through its socket, found as the `tree` commands find it: `--socket
  <path>`, then `B10X_LOOM_TREE_SOCKET`, then `$XDG_RUNTIME_DIR/b10x-loom/tree.sock`
  (`story:tree-daemon` § Outcome). They send the new `ServedTree` command `Decide`, carrying the
  request id, the option or a take, the principal, the reasons and the instant (`--at`, or the
  host clock).
- Before this story, the same commands are refused `StoreServed`
  (`story:tree-waits-open-decision-requests`). With nothing listening on the socket while the
  daemon holds the lock, they are still refused `StoreServed`, naming the socket, as a direct
  `tree run` is.
- The daemon applies `Decide` or `TakeRecommendation` on the tree's decision store and completes
  the request, with the writers and run-log lines of `story:tree-waits-open-decision-requests`:
  `CaseApprovalWriter`, `ExplicitDecisionWriter`, and the `decided` and `record produced` lines.
  The domain's refusals come back as the inbox's exit statuses
  (`story:decision-inbox-commands`): 10 for a refusal named on standard error, 11 for
  `StoreConflict`.
- The daemon's next pass treats the case as a direct `tree run` would treat it after the same
  decision (story 16). It continues past the wait when the request carries a `produced_record`
  whose grounds still resolve, and it supersedes the request when a ground moved. An outcome
  decided "leave it untaken" goes on directly. This is the rule `story:tree-daemon` gives a
  recorded approval ("makes the daemon continue that tree without a further command"), extended
  to every decision request. A case left waiting uses no run slot (`story:tree-daemon`).
- The command prints the inbox's output, then `unblocked: <case> (DR-…)`, then
  `continue: the daemon serving <STORE> continues it` (`story:decisions-resume-the-held-run`).
- A denied approval goes the same way. The daemon writes the denied `CaseApproval` and the two
  lines, performs nothing under that capability, and its next pass gives the tree the stop a
  direct `tree run` gives over the same store after the same denial.
- After a daemon restart, the daemon continues every tree from its store and run log
  (`story:tree-daemon`). A request decided through `Decide` before the restart, or decided and
  not yet recorded, is picked up by the first pass after it. That pass completes a missing record
  first (`complete_pending`, as story 16's `tree run` does) and then continues the case.

Who ticks a served tree: the host. Someone runs `decisions tick --at INSTANT --tree STORE`, for
example SDC's driver or a cron entry. The daemon calling the tick itself stays the later story
`epic:tree-daemon-under-enforced-limits` names (this epic's § Not in scope).

- On a served store the tick sends the new `ServedTree` command `Tick` (the instant). The daemon
  runs `tick` over the tree's store as story 16 does: the `expired` and silence `decided` lines,
  and the tick's `CaseRevisions` from the governor of each request's case.
- The daemon continues on its next pass every case whose request the tick took on silence and
  recorded. An expired request leaves its case waiting.
- `--resume` is not needed on a tree and is refused there: exit 2 before the store is read, as
  `story:decisions-resume-the-held-run` refuses `--resume` without `--state`.
- The command prints the `TickReport` as story 16's `--tree` tick does, then one
  `unblocked: <case> (DR-…)` line per request it took on silence.

Every other `decisions` subcommand on a served store answers as story 16 leaves it.

Files:
- `ess/domains/tree.yaml`: `Decide` and `Tick` on `ServedTree`, and the conformance scenarios
  synthesized from them (`ess/scenarios/`, `crates/loom-conformance`), as `story:tree-daemon`'s
  ESS first says for the tree domain's commands.
- `crates/loom-tree`: the daemon's handling of `Decide` and `Tick`, in the daemon module
  `story:tree-daemon` creates.
- `crates/loom-cli/src/decisions.rs` (the socket client path and `--socket`) and
  `crates/loom-cli/src/lib.rs` (the flag).
- `crates/loom-cli/tests/tree_daemon_decisions.rs` (new).
- `website/docs/guides/decisions.md` and `website/docs/reference/cli.md` (regenerated).

## Why

Session `sdc` answered on 2026-11-21 (`.engineering/drafts/e3-sdc-answers-8.md`). A decision
resumes nothing by itself on a tree, but SDC set two conditions. First, `decide` and `take` under
`--tree` print the unblocked case and the command that continues it
(`story:tree-waits-open-decision-requests`). Second, "the daemon's pass over a served tree picks
up a decided request without any further person action". The second needs the daemon, and story
16 does not wait on the daemon chain (coordinator's decision, work order e3r9). So it is its own
story, after both.

## Waits on

- `story:tree-waits-open-decision-requests`: `decisions --tree`, its writers, run-log lines and
  continuation rule, and the `StoreServed` refusal this story replaces.
- `story:tree-daemon`: the daemon, its socket, `ServedTree`, the restart rule, and `StoreServed`.
  It changes `ess/domains/tree.yaml` and `crates/loom-tree` before this story.
- `story:decisions-resume-the-held-run`: the `--tree` lines and the `--resume` refusal. Both
  change `crates/loom-cli/src/decisions.rs` and `lib.rs`.
- Shared with no edge: `story:concurrent-cases-in-the-tree-daemon` changes the daemon's per-case
  states and `ess/domains/tree.yaml`. Other stories of `epic:tree-daemon-under-enforced-limits`
  that change `ess/domains/tree.yaml` or `crates/loom-tree` after `story:tree-daemon` are in the
  typed scope, and `aep plan artifact waves` keeps them apart.

## ESS first

`ess/domains/tree.yaml`: `ServedTree`, declared by `story:tree-daemon`, gains two commands:

- `Decide`: the request id, an option or `take: true`, the principal, the reasons and the
  instant. It moves a `Waiting` tree to `Queued` when the decided request was the tree's only
  wait.
- `Tick`: the instant.

On that commit `task drift` fails, the synthesized scenarios for the two commands are red, and
`crates/loom-cli/tests/tree_daemon_decisions.rs` does not compile. `ess/domains/decision.yaml` is
unchanged.

## Acceptance

In `crates/loom-cli/tests/tree_daemon_decisions.rs`, with the three-level fixture tree of
`story:tree-run-command`, story 16's fixture autonomy documents, principals and recorded decider,
an engine configuration with the `Daemon` runner as `story:tree-daemon`'s tests use, and no model
call:

- On a served tree that is `Waiting` on `DR-…`, `decisions decide DR-… A --as timo --tree STORE`
  exits 0 and its standard error does not name `StoreServed`.
- After that decision, the daemon brings the root to its terminal outcome with no `tree run` and
  no other command: `tree status` shows it `Terminal`.
- That `decide` prints the last two lines `unblocked: <case> (DR-…)` and
  `continue: the daemon serving STORE continues it`.
- With `--socket` naming a path nothing listens on, while the daemon holds the store's lock, the
  same `decide` exits non-zero naming `StoreServed` and that socket, and the request file is
  byte-identical.
- With the daemon stopped after it applied `Decide` and before its next pass (the test kills it
  on the `decided` line), a restarted `tree serve` brings the root to `Terminal` with no further
  command.
- A request left Decided without a record (crash injected between `Decide` and the record) is
  completed by the first pass after a restart: the case's log gains one `record produced` line,
  and then the case continues.
- Deciding the approval `B` (deny) through the daemon writes a denied `CaseApproval`, and the
  bound executor of that capability records no call.
- After the denial, `tree status` shows the same tree stop as a direct `tree run` over a copy of
  the store after the same `decisions decide … B --tree` with no daemon.
- `decisions take DR-… --as timo --tree STORE` on a strong recommendation leaves the request
  Decided by `timo` through `TakeRecommendation`, and the root then reaches `Terminal`.
- `decisions decide` by a principal not in `eligible` exits 10 naming `NotEligible`, and the
  tree stays `Waiting`.
- `decisions tick --at E --tree STORE` on a served tree whose request has a strong default on
  silence due at `E` decides it by `silence`, and the daemon brings the root to `Terminal` with
  no further command.
- On a served tree whose request has no default, ticked past its `expires_at`, the request is
  Expired, and `tree status` still shows the case `Waiting`.
- `decisions tick --at E --tree STORE --resume` exits 2 before it reads the store.
- `task check` exits 0.

## Out of scope

- The daemon calling the tick itself (`epic:tree-daemon-under-enforced-limits`).
- `decisions escalate`, `supersede` and `record` through the daemon.
- The per-case slot states of `story:concurrent-cases-in-the-tree-daemon`.
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
