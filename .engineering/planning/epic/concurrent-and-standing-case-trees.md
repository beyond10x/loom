---
format: aep.planning-md/3
id: epic:concurrent-and-standing-case-trees
kind: epic
status: proposed
title: The tree daemon runs a tree's admitted cases at once, rests standing trees without a slot until the host wakes them, and hands an admission program its capacity and flow limits
summary: 'Follow-up to the daemon epic for SDC''s orchestration: a slot per running case, a workspace per case, Quiescent and tree wake --at, run caps for writers, tree usage and a model budget, loom.capacity on standard input, limits changed events'
tags:
- sdc
relations:
- serves: vision:O1
- serves: vision:governed-autonomy
- depends_on: epic:protocol-tree-on-bindings
- depends_on: epic:tree-daemon-under-enforced-limits
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:29:06Z", actor: "agent:sdc", revision: 7, decided_on: {"recorded":{"review_outcome":12}}, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

The tree daemon runs the admitted cases of a tree at the same time: every case that can take a
step holds one run slot from its first step until it stops being runnable, a case has at most one
step in flight, and two steps on one workspace never run at once, so a child opened with a
workspace of its own (for the SDC, a story's managed worktree) runs beside its siblings. Under the
daemon a parent steps while it has open children whenever it has a useful action, never at the
same time as a step on its workspace, and the run log names, on the parent step's `action
started` line, the children that were open; a direct bounded `tree run` keeps taking every open
child before its parent's next step. A tree opened as standing rests `Quiescent` when it has
nothing to do and nothing is wrong, holds no slot while it rests, and wakes when the host passes
an instant (`tree wake --at`) at which an artifact has a new revision or a held record has
expired; once cases have states of their own, a wake acts on the resting cases and on the cases
holding children beside waiting ones. Only a wake of a `Quiescent` tree starts a new run for
budget purposes, with the totals since `tree open` recorded beside it; any other wake resets no
counter and drops no extension. A writer's `max_runs` can count since the latest outside revision
of its case; an observation is never refused `RunsSpent`, a `max_runs` stated on one is refused at
load, and a writer whose runs are used up stops the run naming itself and its last failure. A tree
records its usage per run, both the sum of step wall times and the elapsed wall time, model calls,
tokens and cost, and stops at a model cost budget. A program bound to an admission action is
handed the tree's capacity on standard input: run slots, running, queued, blocked and free, the
engine's ceilings, the tree's budget, its usage per run and since `tree open`, and the flow limits
in force with the asking case's open children. A flow limit changes only through `tree limits
set` by a person or a rule, recorded as a `limits changed` event (who, old, new) in the root's run
log and read at the next admission.

## Why

Session `sdc`, 2026-11-21 (`.engineering/drafts/e4-request.md`), after the operator accepted the
orchestration design (`~/beyond10x/sdc/docs/orchestration.md` § 7), and its answers on the
drafter's defaults (`.engineering/drafts/e4-sdc-answers.md`, `.engineering/drafts/e4-sdc-answers-2.md`,
`.engineering/drafts/e4-sdc-answers-3.md`; budgets at a wake, `.engineering/drafts/e3-sdc-answers-5.md`).
Four requests; request 1, `loom-child-cases`, is `story:child-case-runtime` and gets nothing here.
The sentences this epic answers:

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
  at the next admission and never retroactive. No LLM raises a limit." This epic keeps the limits
  in the root's run log; reading them from the area's configuration is not implemented
  (`story:flow-limits-recorded-as-events`).
- Budgets at a wake, session `sdc`: "a wake starts a new run for budget purposes with the tree's
  totals since `tree open` recorded beside the per-run counters; the capacity member given to an
  admission program carries both".
- A parent beside its children, session `sdc` (`.engineering/drafts/e4-sdc-answers-3.md`): "Under
  the tree daemon a parent may take a step while it has open children, whenever the selector finds
  a useful action for it; `flow` hands the asking case its own open children with their opening
  instants; the bounded `tree run` keeps children first", with two conditions, "a parent step and a
  child step never write one workspace at the same time" and "the run log records, for a parent
  step taken with open children, which children were open"; and "only a wake of a Quiescent tree
  starts a new budget run".

Accepted by session `sdc` on 2026-11-21 (`.engineering/drafts/e4-sdc-answers-2.md`): an
observation runs once per case revision; a used-up writer stops the run `RunsSpent`, exit 4; the
tree's cost is the session executor's stated cost, counted over the whole tree across runs and
from the latest wake for a standing tree, with `CostUnreported` before the next agent step when a
cost is unknown; an extension does not carry over a wake. Chosen by session `sdc` the same day:
"an explicit `budget.max_runs` on an observation is refused at load with a named refusal" (its
point 5). Answered by session `sdc` the same day (`.engineering/drafts/e4-sdc-answers-3.md`): a
parent steps beside its open children under the daemon, and only a wake of a `Quiescent` tree
starts a new budget run.

What the plan already answers, read 2026-11-21:

- Child cases and their `case_outcome` records: `story:child-case-runtime`, whose § Outcome says
  "The cases of one tree share the root's workspace".
- Run slots, a queue, a waiting tree holding no slot, totals counted from `tree open`:
  `story:tree-daemon` § Outcome ("The daemon runs up to the engine configuration's
  `serve.run_slots` trees at once and queues the rest; a tree that waits on a person uses no run
  slot"), per tree. `story:tree-run-command` § Outcome drives "every open child before its
  parent's next step" with "One writer per store"; `story:concurrent-cases-in-the-tree-daemon`
  changes the daemon's scheduling only, and the direct run keeps that order.
- Per-action and per-run ceilings in the engine configuration, a binding only lowering them:
  `story:action-limits-enforced`. Nothing hands them to a program.
- Model calls and tokens per agent step and `--max-agent-calls` over the whole tree:
  `story:agent-node-run-records`; no cost, no elapsed wall time.
- The governor never completes a case without exactly one legitimate outcome: `AGENTS.md` § Checks
  that hold the rules states it, and `crates/loom-governor/tests/completion_rule.rs` checks a
  blocked outcome, first-declared order and one legitimate outcome, but no case with none;
  `story:standing-tree-rests-quiescent` adds that check. A tree with no legitimate outcome and
  nothing useful exits 4, stuck (`story:tree-run-command` § Acceptance):
  `story:standing-tree-rests-quiescent` adds rest.
- An action performed at the current case revision is not selected again until an artifact moves
  or the host reports it due (`story:deterministic-default-selector`, implemented;
  `crates/loom-executor/src/selection.rs:285-305`), which is the observation rule session `sdc`
  asks for. `max_runs` counts per case life for every program action
  (`story:bound-program-result-table` § Acceptance); `story:run-caps-per-arrival` lands the
  writer-only rule.
- Expiry is computable from the governor without a Canon change: it applies Canon's freshness rule
  from the kind's `max_age` and the record's `observed_at` at a given instant
  (`crates/loom-governor/src/lib.rs:1938-1954`). The epic's tests run under the Canon 0.9.0 pin
  (`story:canon-0-9-0-pin`, below).

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

Which SDC blocker each answers: `loom-standing-trees` 1, 2 and 4 (6 extends the wake to per-case
states); `loom-concurrent-siblings` 5 and 6 (6 also lets a parent step beside its open children);
`loom-run-ceilings` 7, with 3 for the model budget and the wall figures; request item 4's limits
and `limits_changed` 8.

## Order

Edges for need: 2 on `story:tree-run-command`, `story:bound-program-result-table`,
`story:bound-agent-node` and `story:bound-artifact-revisions`; 3 on
`story:agent-node-run-records`; 4 on 1 (rest), 2 (the arrival detection) and 3 (the usage
counters); 5 on `story:child-case-runtime` and `story:tree-run-command`; 6 on `story:tree-daemon`,
1 (`Quiescent` among its per-case states), 5 (a workspace per case) and
`story:tree-waits-open-decision-requests` (below); 7 on 6 and 4; 8 on 7 and, for the epic's checks
it carries, on 1, 2 and 4. 1 depends on `story:tree-daemon`. Outside the epic,
`story:budget-and-limit-stops-raise-decision-requests` depends on 3 (it turns 3's cost stop into a
`budget_overrun` request and checks it).

Edges for shared files make the eight one chain, 1 to 8, each body naming the files: 2 on 1, 3 on
2, 4 on 3, 5 on 4, 6 on 5, 7 on 6, 8 on 7. Inside the set no further edge is needed: the chain
orders every pair that shares a file. The files:

- `ess/domains/tree.yaml` and `crates/loom-tree`: all eight; `generated/rust/loom/src/*.rs` and
  `CHANGELOG.md`: all eight; `crates/loom-cli/tests/fixtures/area/`: created by 1, extended by name
  by every later story (`bin/gate` is created by 4 and used by 6 to 8);
- `ess/domains/runlog.yaml`: 2, 3, 4, 6, 7, 8; `docs/contracts/run-log-v1.json` (regenerated): 3,
  8;
- `ess/domains/bindings.yaml`: 2, 3, 5, 6, 7; `crates/loom-bindings/src/decode.rs` and `load.rs`: 2,
  5, 7; the program port of `story:bound-program-result-table`: 2, 5, 7; the agent port of
  `story:bound-agent-node`: 2, 3, 5; `website/docs/guides/bindings.md`: 2, 7;
  `docs/contracts/loom-action-selection.md` and `AGENTS.md`: 2;
- `crates/loom-cli/src/lib.rs`: 1, 2, 3, 4, 6, 8; `website/docs/reference/cli.md`: 1, 3, 4, 8;
  `ess/scenarios/` (tree domain): 1, 4, 6, 8; a new guide page under `website/docs/guides/`: 8;
- `ess/domains/governor.yaml` and `crates/loom-governor/src/file_store.rs`: 5;
  `crates/loom-governor/src/lib.rs`: 1, 5; `crates/loom-governor/tests/completion_rule.rs`: 1;
  `ess/domains/engine.yaml`: 6.

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
so the requester's order ("behind ... the decision-request epic") holds for 6 to 8 only.

Stories outside the epic that share a file with the eight and have no path to or from the story
they share it with (`aep plan artifact graph --format json`, 2026-11-21, open stories only; the
round-3 verdicts and the eight scope reports; a story named here only for a citation of the file
is left out). For each pair the rule is the store's: the second to land rebases. The typed scope
of each story hands the collision to `aep plan artifact waves`. No edge is added for them.

- `ess/domains/tree.yaml`, `ess/domains/runlog.yaml`, `crates/loom-tree`:
  - `story:tree-waits-open-decision-requests`, with 1 to 5 (it is an ancestor of 6 to 8).
  - `story:budget-and-limit-stops-raise-decision-requests` (`TreeBudget`), with 4 to 8 (it lands
    after 1 to 3). It also meets 7 on `Capacity.budget`, "the tree's `TreeBudget` in force": a
    member it extends with a `budget extended` line is in force at its extended value, and 7 says
    so.
  - `story:non-independent-review-becomes-decision-request` (a new module of `crates/loom-tree`),
    with all eight.
  - `story:run-log-references-exec-file-changes` (the `action finished` event, which 3 extends,
    and `crates/loom-tree`): it depends on `story:budget-and-limit-stops-raise-decision-requests`,
    which depends on 3, so it lands after 1 to 3 and is unordered only with 4 to 8.
  - `story:authority-records-bound-and-grounded-in-canon` (`tree.yaml`, `CaseApproval`), with all
    eight.
  - `story:requests-from-canon-pending-decisions`, `story:confinement-level-on-oci-and-pod` and
    `story:ingress-routes-per-action` (`crates/loom-tree`; the last two each add a test under
    `crates/loom-tree/tests/` and the code it checks), with all eight.
- `crates/loom-governor/src/lib.rs`: `story:authority-records-bound-and-grounded-in-canon`,
  `story:requests-from-canon-pending-decisions`, `story:decided-records-carry-grounds-to-canon`,
  `story:engineering-registry-calls-replaced` and `story:frontier-names-movers`, with 1
  (`next_expiry`) and 5 (the workspace in the case state).
- `ess/domains/bindings.yaml` and `crates/loom-bindings/src/load.rs` (with `decode.rs`), stories 2,
  3, 5, 6, 7: `story:bindings-child-path-refusal-names-the-bindings-directory` (`decode.rs`,
  `load.rs`), `story:agent-session-through-substrate` (`AgentRun`, which 3 extends) and
  `story:autonomy-grant-table`.
- `ess/domains/governor.yaml` and `crates/loom-governor/src/file_store.rs`, story 5 (`CaseLink`):
  `story:authority-records-bound-and-grounded-in-canon`,
  `story:decided-records-carry-grounds-to-canon`, `story:requests-from-canon-pending-decisions`,
  `story:decision-request-file-store` and `story:engineering-registry-calls-replaced`.
- `ess/domains/engine.yaml`, story 6 (`ServeSection`): `story:agent-session-through-substrate`,
  `story:confinement-level-on-oci-and-pod`, `story:ingress-routes-per-action`,
  `story:executable-descendants-held` and `story:substrate-daemon-on-another-host`.
- `crates/loom-cli/src/lib.rs`, stories 1, 2, 3, 4, 6, 8: `story:decision-inbox-commands`,
  `story:silence-and-expiry-at-a-host-tick`, `story:approval-decisions-resume-held-runs`,
  `story:question-and-outcome-decisions-hold-runs`, `story:approvals-supplied-to-canon-evaluation`
  and `story:decision-suspension-reason` with 1 to 4 (they are ancestors of 6 and 8, through
  `story:tree-waits-open-decision-requests`), and
  `story:secrets-subcommand`, `story:command-invocation-contract` and
  `story:agent-selects-the-connectors-relay` with all six.

## Acceptance

`crates/loom-cli/tests/tree_concurrent_standing.rs` (`story:flow-limits-recorded-as-events`) runs
these on the Linux gate with the fixtures under `crates/loom-cli/tests/fixtures/area/`, stub
programs, a managed Substrate daemon, a Loom daemon with two run slots, no model and no network.
No run-log line orders the steps of two cases (each carries its run's `--at` instant), so an order
across cases is read from the start instants `bin/gate` writes and the test's own releases:

- A root opens three children, each with its own workspace directory and its step on `bin/gate`:
  two run at once (both gates write their start instants before the test releases either), the
  third's gate writes its start instant only after the test released one of the first two, no
  case's log has two `action started` lines without an `action finished` between them, and each
  case's final Canon decision equals the one a direct sequential `tree run` of the same fixture
  gives.
- Three children opened without a workspace of their own never have steps in flight at once.
- A standing root with one open child that inherits its workspace and holds a `bin/gate`: after
  the host appends to the root's input and calls `tree wake`, the root's log gains no `action
  started` line while the gate is held; after the test releases it, the root's next `action
  started` line names the child in `open_children`.
- A standing tree with nothing to do rests `Quiescent` (a blocking `tree run` exits 7), and a
  bounded tree submitted after it starts at once on a slot.
- `tree wake --at` before the earliest expiry `tree status` names leaves the standing tree
  `Quiescent` and changes no byte of its store; at that instant, or after the host changed a placed
  file, the tree runs and rests again.
- A standing tree with one case `Waiting` and one `Resting`: after the host changes the resting
  case's placed file, `tree wake --at` runs that case and leaves the waiting one untouched.
- Three wakes of a standing tree whose steps at each wake stay under the per-run wall ceiling,
  while their sum since `tree open` passes it, never stop `BudgetSpent`; `tree status` shows the
  last run's usage beside `since_open`.
- A writer with `max_runs: 2` and `runs_per: arrival` whose check keeps failing stops the tree
  `RunsSpent`, exit 4, naming the writer and the failing check's record, with no `tree approve`
  command; after the host rewrites its input the writer runs again; its check runs once per case
  revision and is never refused `RunsSpent`, and a bindings file stating `max_runs` on the check is
  refused at load.
- The admission stub receives `loom.capacity` with two run slots, the running, queued and blocked
  counts, the engine's ceilings, the tree's budget and its usage per run and since `tree open`.
- An admission on a root with two open children sees both, with their opening instants, in
  `flow.open_children`.
- After `tree limits set --as alice --kind person --at T max_in_progress=1` the next admission sees
  `max_in_progress` 1 and the root's run log holds one `limits changed` line naming `alice`, the
  old value and the new one; `--kind decider` exits 1 naming `LimitChangerRefused`.
- `tree verify` exits 0 for every tree, and `task check` exits 0.

The model cost budget is checked by `story:tree-usage-and-model-budget`'s own test with a stub
agent adapter: the daemon serves no agent step before `story:agent-session-through-substrate`.

## Not in scope

- Creating, merging or removing a Git work tree, counting work trees or unmerged branches: the
  SDC's admission program does that with `worktree` and Git. Loom carries `max_open_worktrees` and
  `max_unmerged` as values only.
- Enforcing a flow limit in Loom: `wave.admit` refuses (orchestration § 2.4a); Loom records and
  hands over the limits.
- Reading flow limits from the area's configuration (orchestration § 2.4a): they come from the
  root's run log only.
- Computing the flow KPIs F1 to F17: they are read from the run logs Loom writes.
- A file watch or timer in the daemon: the host passes every wake instant (accepted by session
  `sdc`).
- A token budget; pricing by a rate card.
- Concurrent cases in a direct `tree run`: it stays sequential, children first.
- Calling `decisions tick` from a wake.
- Changes in Canon, Substrate or the SDC.

## Source

Session `sdc`, 2026-11-21: `.engineering/drafts/e4-request.md`, `.engineering/drafts/e4-sdc-answers.md`,
`.engineering/drafts/e4-sdc-answers-2.md`, `.engineering/drafts/e4-sdc-answers-3.md`,
`.engineering/drafts/e3-sdc-answers-5.md`. Read:
`~/beyond10x/sdc/docs/orchestration.md` §§ 1.1, 1.2, 2.1-2.4a, 2.6, 3, 6, 7;
`~/beyond10x/sdc/protocol/draft/orchestrate.protocol.yaml` (header, `horizon`, `admission`);
`~/beyond10x/sdc/docs/kpis.md` § 5; `~/beyond10x/sdc/docs/trial-2.md` (run notes R3, R4);
`~/beyond10x/sdc/.engineering/planning/upstream-blocker/loom-{child-cases,concurrent-siblings,standing-trees,run-ceilings}.md`.
Loom tree at `39f4da88` (source unchanged at `61fd500b`) with the store's uncommitted pointers: the
stories named above, `crates/loom-governor/src/lib.rs`, `crates/loom-executor/src/selection.rs`,
`crates/loom-bindings/src/`, `crates/loom-session-executor/src/lib.rs`, `ess/domains/bindings.yaml`,
`ess/domains/metering.yaml`. Round verdicts: `.engineering/drafts/e4-round-1-*.md`,
`.engineering/drafts/e4-round-2-*.md`, `.engineering/drafts/e4-round-3-*.md`; scope reports
`.engineering/drafts/scope-report-*.md`.
