---
format: aep.planning-md/3
id: epic:concurrent-and-standing-case-trees
kind: epic
status: draft
title: The tree daemon runs a tree's admitted cases at once, rests standing trees without a slot until the host wakes them, and hands an admission program its capacity and flow limits
summary: 'Follow-up to the daemon epic for SDC''s orchestration: a slot per running case, a workspace per case, Quiescent and tree wake --at, run caps for writers, tree usage and a model budget, loom.capacity on standard input, limits changed events'
tags:
- sdc
relations:
- serves: vision:O1
- serves: vision:governed-autonomy
- depends_on: epic:protocol-tree-on-bindings
- depends_on: epic:tree-daemon-under-enforced-limits
revision: 3
---
## Outcome

The tree daemon runs the admitted cases of a tree at the same time: every case that can take a
step holds one run slot while it runs, a case has at most one step in flight, and two cases that
share a workspace never run at once, so a child opened with a workspace of its own (for the SDC, a
story's managed worktree) runs beside its siblings. A tree opened as standing rests `Quiescent`
when it has nothing to do and nothing is wrong, holds no slot while it rests, and wakes when the
host passes an instant (`tree wake --at`) at which an artifact has a new revision or a held record
has expired; a wake starts a new run for budget purposes, with the totals since `tree open`
recorded beside it. A writer's `max_runs` can count since the latest outside revision of its case;
an observation is not capped by `max_runs` at all, and a writer whose runs are used up stops the
run naming itself and its last failure. A tree records its usage per run, both the sum of step wall
times and the elapsed wall time, model calls, tokens and cost, and stops at a model cost budget. A
program bound to an admission action is handed the tree's capacity on standard input: run slots,
running, queued and free, the engine's ceilings, the tree's budget, its usage per run and since
`tree open`, and the flow limits in force with the open children they apply to. A flow limit
changes only through `tree limits set` by a person or a rule, recorded as a `limits changed`
event (who, old, new) in the root's run log and read at the next admission.

## Why

Session `sdc`, 2026-11-21 (`.engineering/drafts/e4-request.md`), after the operator accepted the
orchestration design (`~/beyond10x/sdc/docs/orchestration.md` § 7), and its answers on the
drafter's defaults (`.engineering/drafts/e4-sdc-answers.md`; budgets at a wake,
`.engineering/drafts/e3-sdc-answers-5.md`). Four requests; request 1, `loom-child-cases`, is
`story:child-case-runtime` and gets nothing here. The sentences this epic answers:

- § 1.1: "No code anywhere runs two child cases of one tree concurrently, and the shared `work/`
  would make it unsafe if it did." § 2.4: "Each admitted child gets its own managed worktree from
  `worktree create` and its own build directory ... Children of one wave run concurrently; the
  runtime must drive siblings in parallel ... extend its slot to 'one per running case'."
  Upstream blocker `loom-concurrent-siblings`: "one run slot per running case ... and one writer
  per case instead of one per store".
- § 3: "A standing tree in quiescence holds no run slot ... It wakes on an artifact revision or an
  expiry." "per-action `max_runs` counted per subject revision, not per case life", which session
  `sdc` narrowed to writers: "an observation has no per-arrival cap; it runs at most once per
  subject revision unless reported due ... A used-up writer must stop the run with a reason that
  names the action and its last failure, never as a wait on a person" (evidence: trial 2, run note
  R4, `~/beyond10x/sdc/docs/trial-2.md`). Blocker `loom-standing-trees` adds "the governor never
  completes a case that has no legitimate outcome".
- § 2.4 rule 3: "story count ≤ free run slots: the engine's configured ceilings ... and the
  remaining model budget". Blocker `loom-run-ceilings`: "the configured run ceilings ... and the
  free run slots are readable by a program". Session `sdc`: the capacity member carries "the tree's
  model budget and what is used (calls and cost, per tree; tokens if you have them), and the tree
  stops with a budget reason when it is spent", and "both figures per run: the sum of step wall
  times ... and the elapsed wall time of the tree run".
- § 2.4a: "Limits, in the area's configuration, enforced by `wave.admit`, never by an LLM:
  `limits.wip.max_in_progress` ..., `max_open_worktrees`, `max_unmerged` ..., `max_item_age`."
  "a change is the recorded event `limits_changed` (who, old, new), by a person or a rule, read
  at the next admission and never retroactive. No LLM raises a limit."
- Budgets at a wake, session `sdc`: "a wake starts a new run for budget purposes with the tree's
  totals since `tree open` recorded beside the per-run counters; the capacity member given to an
  admission program carries both".

What the plan already answers, read 2026-11-21:

- Child cases and their `case_outcome` records: `story:child-case-runtime`, whose § Outcome says
  "The cases of one tree share the root's workspace".
- Run slots, a queue, a waiting tree holding no slot, totals counted from `tree open`:
  `story:tree-daemon` § Outcome ("The daemon runs up to the engine configuration's
  `serve.run_slots` trees at once and queues the rest; a tree that waits on a person uses no run
  slot"), per tree. `story:tree-run-command` § Outcome drives "every open child before its
  parent's next step" with "One writer per store".
- Per-action and per-run ceilings in the engine configuration, a binding only lowering them:
  `story:action-limits-enforced`. Nothing hands them to a program.
- Model calls and tokens per agent step and `--max-agent-calls` over the whole tree:
  `story:agent-node-run-records`; no cost, no elapsed wall time.
- The governor never completes a case without exactly one legitimate outcome: already held
  (`AGENTS.md` § Checks that hold the rules, `crates/loom-governor/tests/completion_rule.rs`).
  Nothing to draft. But a tree with no legitimate outcome and nothing useful exits 4, stuck
  (`story:tree-run-command` § Acceptance): `story:standing-tree-rests-quiescent` adds rest.
- An action performed at the current case revision is not selected again until an artifact moves
  or the host reports it due (`story:deterministic-default-selector`, implemented;
  `crates/loom-executor/src/selection.rs:285-305`), which is the observation rule session `sdc`
  asks for. `max_runs` counts per case life for every program action
  (`story:bound-program-result-table` § Acceptance).
- Expiry is computable at Canon 0.8.0: the governor applies Canon's freshness rule from the kind's
  `max_age` and the record's `observed_at` at a given instant (`crates/loom-governor/src/lib.rs:1938-1954`).

Work trees: Loom manages no Git work tree and the request needs none from it. The sentence is
"each story in its own work tree", and orchestration § 2.4 has it made by `worktree create`, which
the SDC's own program can run (it needs `story:writable-host-root` when confined, accepted by
session `sdc`). Loom lacks a case that runs in a directory other than its root's and a rule that
two cases in one directory never write at once: `story:child-case-own-workspace` and
`story:concurrent-cases-in-the-tree-daemon`. Merging and removing the work tree stays the SDC's.

## Stories

In landing order, each depending on the one before it:

1. `story:standing-tree-rests-quiescent`
2. `story:run-caps-per-arrival`
3. `story:tree-usage-and-model-budget`
4. `story:standing-tree-wakes-at-a-host-instant`
5. `story:child-case-own-workspace`
6. `story:concurrent-cases-in-the-tree-daemon`
7. `story:capacity-given-to-an-admission-program`
8. `story:flow-limits-recorded-as-events`

Which SDC blocker each answers: `loom-standing-trees` 1, 2 and 4; `loom-concurrent-siblings` 5
and 6; `loom-run-ceilings` 7, with 3 for the model budget and the wall figures; request item 4's
limits and `limits_changed` 8.

## Order

Edges for need: 2 on `story:tree-run-command`, `story:bound-program-result-table`,
`story:bound-agent-node` and `story:bound-artifact-revisions`; 3 on
`story:agent-node-run-records`; 4 on 1 (rest), 2 (the arrival detection) and 3 (the usage
counters); 5 on `story:child-case-runtime` and `story:tree-run-command`; 6 on `story:tree-daemon`,
1 (`Quiescent` among its per-case states), 5 (a workspace per case) and
`story:tree-waits-open-decision-requests` (below); 7 on 6 and 4; 8 on 7 and, for the epic's checks
it carries, on 1, 2 and 4. 1 depends on `story:tree-daemon`.

Edges for shared files make the eight one chain, 1 to 8, each body naming the files: 2 on 1, 3 on
2, 4 on 3, 5 on 4, 6 on 5, 7 on 6, 8 on 7. The files:

- `ess/domains/tree.yaml` and `crates/loom-tree`: all eight; `generated/rust/loom/src/*.rs` and
  `CHANGELOG.md`: all eight; `crates/loom-cli/tests/fixtures/area/`: created by 1, extended by name
  by every later story;
- `ess/domains/runlog.yaml`: 2, 3, 4, 7, 8;
- `ess/domains/bindings.yaml`: 2, 5, 6, 7; `crates/loom-bindings/src/decode.rs` and `load.rs`: 2, 5,
  7; the program port of `story:bound-program-result-table`: 2, 7; the agent port: 2, 3;
- `crates/loom-cli/src/lib.rs`: 1, 2, 3, 4, 8; `website/docs/reference/cli.md`: 1, 3, 4, 8;
  `ess/scenarios/` (tree domain): 1, 4, 6, 8; `website/docs/guides/`: 6, 8;
- `ess/domains/governor.yaml` and `crates/loom-governor/src/file_store.rs`: 5;
  `crates/loom-governor/src/lib.rs`: 1; `ess/domains/engine.yaml`: 6.

`ess/domains/tree.yaml`, `runlog.yaml` and `engine.yaml` do not exist at `39f4da88`; earlier
stories of the two parent epics create them.

Behind which epic. Every story here depends on `story:tree-run-command`, which depends on
`story:record-copy-between-cases`, which depends on `story:child-case-runtime` and through it on
`story:canon-0-9-0-pin`, held by
`upstream-blocker:engineering-protocols-on-canon-0-9-0` (`aep plan artifact graph --format json`):
the whole epic waits on Canon 0.9.0, not only stories 5 and 6. Stories 6, 7 and 8 also wait on
`epic:decision-requests-in-the-runtime`, through 6's edge to
`story:tree-waits-open-decision-requests`: both rewrite `TreeStop::Stuck`, `TreeStop::Waiting`
and the per-case entry of `TreeStatus`, and 6 classifies decision waits per case. Stories 1 to 5
do not: they add their own variants and members to `TreeStop` and `TreeStatus` (`Quiescent`,
`RunsSpent`, `CostUnreported`, `horizon`, `usage`, `workspace`) and change none of that story's,
so the requester's order ("behind ... the decision-request epic") holds for 6 to 8 only. Of 1 to 5
and `story:tree-waits-open-decision-requests`, or
`story:budget-and-limit-stops-raise-decision-requests` (which adds `TreeBudget` members beside 3's),
the second to land rebases on `ess/domains/tree.yaml` and `runlog.yaml`.

## Acceptance

`crates/loom-cli/tests/tree_concurrent_standing.rs` (`story:flow-limits-recorded-as-events`) runs
these on the Linux gate with the fixtures under `crates/loom-cli/tests/fixtures/area/`, stub
programs, a managed Substrate daemon, a Loom daemon with two run slots, no model and no network:

- A root opens three children, each with its own workspace directory: two run at once (their run
  logs overlap in time), the third starts after one of them ended, no case's log has two
  `action started` lines without an `action finished` between them, and each case's final Canon
  decision equals the one a direct sequential `tree run` of the same fixture gives.
- Three children opened without a workspace of their own never have steps in flight at once.
- A standing tree with nothing to do rests `Quiescent` (a blocking `tree run` exits 7), and a
  bounded tree submitted after it starts at once on a slot.
- `tree wake --at` before the earliest expiry `tree status` names leaves the standing tree
  `Quiescent` and changes no byte of its store; at that instant, or after the host changed a placed
  file, the tree runs and rests again.
- Three wakes of a standing tree whose one step each stays under the per-run wall ceiling, while
  their sum since `tree open` passes it, never stop `BudgetSpent`; `tree status` shows the last
  run's usage beside `since_open`.
- A writer with `max_runs: 2` and `runs_per: arrival` whose check keeps failing stops the tree
  `RunsSpent`, exit 4, naming the writer and the failing check's record, with no `tree approve`
  command; after the host rewrites its input the writer runs again; its check runs once per case
  revision and is never refused `RunsSpent`.
- The admission stub receives `loom.capacity` with two run slots, the running and queued counts,
  the engine's ceilings, the tree's budget and its usage per run and since `tree open`; after
  `tree limits set --as alice --kind person --at T max_in_progress=1` the next admission sees
  `max_in_progress` 1 and the root's run log holds one `limits changed` line naming `alice`, the
  old value and the new one; `--kind decider` is refused.
- `tree verify` exits 0 for every tree, and `task check` exits 0.

The model cost budget is checked by `story:tree-usage-and-model-budget`'s own test with a stub
agent adapter: the daemon serves no agent step before `story:agent-session-through-substrate`.

## Not in scope

- Creating, merging or removing a Git work tree, counting work trees or unmerged branches: the
  SDC's admission program does that with `worktree` and Git. Loom carries `max_open_worktrees` and
  `max_unmerged` as values only.
- Enforcing a flow limit in Loom: `wave.admit` refuses (orchestration § 2.4a); Loom records and
  hands over the limits.
- Computing the flow KPIs F1 to F17: they are read from the run logs Loom writes.
- A file watch or timer in the daemon: the host passes every wake instant (accepted by session
  `sdc`).
- A token budget; pricing by a rate card.
- Concurrent cases in a direct `tree run`: it stays sequential.
- Calling `decisions tick` from a wake.
- Changes in Canon, Substrate or the SDC.

## Source

Session `sdc`, 2026-11-21: `.engineering/drafts/e4-request.md`, `.engineering/drafts/e4-sdc-answers.md`,
`.engineering/drafts/e3-sdc-answers-5.md`. Read: `~/beyond10x/sdc/docs/orchestration.md` §§ 1.1,
1.2, 2.1-2.4a, 2.6, 3, 6, 7; `~/beyond10x/sdc/protocol/draft/orchestrate.protocol.yaml` (header,
`horizon`, `admission`); `~/beyond10x/sdc/docs/kpis.md` § 5; `~/beyond10x/sdc/docs/trial-2.md`
(run notes R3, R4); `~/beyond10x/sdc/.engineering/planning/upstream-blocker/loom-{child-cases,concurrent-siblings,standing-trees,run-ceilings}.md`.
Loom tree at `39f4da88` with the store's uncommitted pointers: the stories named above,
`crates/loom-governor/src/lib.rs`, `crates/loom-executor/src/selection.rs`,
`crates/loom-bindings/src/`, `crates/loom-session-executor/src/lib.rs`, `ess/domains/bindings.yaml`,
`ess/domains/metering.yaml`. Round-1 verdicts: `.engineering/drafts/e4-round-1-*.md`.
