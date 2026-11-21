---
format: aep.planning-md/3
id: review-result:concurrent-and-standing-case-trees-acceptance-r1
kind: review-result
status: active
title: Plan critic acceptance, round 1, on epic:concurrent-and-standing-case-trees
relations:
- reviews: epic:concurrent-and-standing-case-trees
- reviews: story:capacity-given-to-an-admission-program
- reviews: story:child-case-own-workspace
- reviews: story:concurrent-cases-in-the-tree-daemon
- reviews: story:flow-limits-recorded-as-events
- reviews: story:run-caps-per-arrival
- reviews: story:standing-tree-rests-quiescent
- reviews: story:standing-tree-wakes-at-a-host-instant
revision: 1
---
needs-revision

- `epic:concurrent-and-standing-case-trees` — the limits-change check runs `tree limits set --as alice --kind person max_in_progress=1` with no `--at`, but `story:flow-limits-recorded-as-events` requires `--at INSTANT` in the command it defines, so the epic's check exits 2 as written — `.engineering/planning/epic/concurrent-and-standing-case-trees.md:148` against `.engineering/planning/story/flow-limits-recorded-as-events.md:20`, `:74`
- `epic:concurrent-and-standing-case-trees` — the Outcome promises that budgets count from the latest wake and that a binding counts runs per outside revision, and no Acceptance bullet exercises either, so the epic can close while `story:standing-tree-wakes-at-a-host-instant` and `story:run-caps-per-arrival` are unproven together — `.engineering/planning/epic/concurrent-and-standing-case-trees.md:131` (Acceptance; Outcome at `:17-30`)
- `story:run-caps-per-arrival` — the check "after two runs the action is refused `RunsSpent` … and the action runs twice more" names no way for the observer to be selected a second time. The story's own Why cites the selector rule that an action performed at the current case revision is not useful again until an artifact moves. The selector story adds that a program that writes no evidence needs `retry_when` to run again. The fixture has neither, and the story does not say whether `runs_per_arrival.rs` drives the port directly. A direct drive cannot produce an `arrived` line, which `crates/loom-tree` writes — `.engineering/planning/story/run-caps-per-arrival.md:72-76` against `story/deterministic-default-selector.md:72-74`
- `story:standing-tree-wakes-at-a-host-instant` — the Outcome says "when no revision moved nothing is written", which leaves open what is written when a revision moved and no case has a useful action. No Acceptance bullet covers that state, so the promise that a second wake at the same instant changes no byte is checked only where no file changed — `.engineering/planning/story/standing-tree-wakes-at-a-host-instant.md:29`
- `story:standing-tree-wakes-at-a-host-instant` — the Outcome moves four counters to start at the latest `woken` line (wall, CPU, steps, agent calls), but the only budget check is the per-run wall ceiling, so a step-count or agent-call counter that still counts from `tree open` passes — `.engineering/planning/story/standing-tree-wakes-at-a-host-instant.md:34` against `:88`
- `story:child-case-own-workspace` — the Outcome says a child's agent sessions get its workspace and its own `open` children inherit it, and no Acceptance bullet opens a grandchild or runs an agent in the child's directory; every bullet uses a program step and a direct child — `.engineering/planning/story/child-case-own-workspace.md:22-23` against `:77-96`
- `story:concurrent-cases-in-the-tree-daemon` — `tree status` is to give each case `Queued`, `Running`, `Waiting`, `Stuck`, `HoldsChildren` or `Terminal`, and the served tree `Running` or `Queued`. Acceptance asserts only `Waiting` and the two stop statuses, so a status that always prints `Running` passes — `.engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:37` against `:92`
- `story:standing-tree-rests-quiescent` — "a blocking `tree run` exits 7" through the daemon is in the Outcome, and the daemon bullet checks only the resting state, the bounded tree and the restart. Exit 7 is checked for a direct run only — `.engineering/planning/story/standing-tree-rests-quiescent.md:26` against `:86`
- `story:capacity-given-to-an-admission-program` — `used.wall_seconds` is checked against "the sum the tree's budget counts" with no named reader, and `tree status` prints no totals. The check needs the lines to sum (the used times in `action finished`, `story:tree-daemon` `:52`). Two Outcome members have no check: `queued`, and that an `arguments_from` program is not given the member — `.engineering/planning/story/capacity-given-to-an-admission-program.md:75` (Outcome `:16-34`)

**What I read:** 8 of 8 ids (the epic and its seven stories) with `aep plan artifact show <id>`, plus `aep plan artifact kinds` and `lifecycle story`. I opened the cited sources: `.engineering/drafts/e4-plan.md`, the sdc `orchestration.md` and `kpis.md`, the draft protocol, `crates/loom-governor/src/lib.rs:1938-1954`, `ess/domains/bindings.yaml`, `crates/loom-bindings/src/load.rs:286-292` and the upstream stories the bodies cite.

**What I could not establish:**
- Whether `story:run-caps-per-arrival` means to drive the program port directly or the tree. That decides whether the finding on it is a fixture gap or a layer gap.
- The `expired` rule at `crates/loom-governor/src/lib.rs:1938-1954` reads as "later than `observed_at` by more than `max_age`", so `observed_at` + `max_age` + 1 s in `story:standing-tree-rests-quiescent` agrees with it. I ran no test.

**Out of my lane (do not set my verdict):**
- **Stale line citations.** `story:child-case-runtime`, `story:tree-daemon` and `story:tree-run-command` carry uncommitted edits (the pointers added today), so these citations no longer open on the quoted text:
  - `child-case-runtime.md:73` and `:158` are now `:82` and `:171`.
  - `tree-daemon.md:37-38`, `:48-50` and `:144` are now about `:42`, `:52-55` and `:165`.
  - `tree-run-command.md:80` and `:143` are now `:85` and `:148`.
  - The affected bodies are `story:child-case-own-workspace`, `story:concurrent-cases-in-the-tree-daemon`, `story:standing-tree-rests-quiescent`, `story:standing-tree-wakes-at-a-host-instant`, `story:capacity-given-to-an-admission-program` and the epic Why.
  - Citations to files with no uncommitted edits (the selector, bindings-document, substrate-daemon-runner and sdc docs) hold. This is for the scope critic or the drafter.
- **Per-arrival counting.** `story:run-caps-per-arrival` counts per arrival, but `orchestration.md:292` and the request say "per subject revision". This is the drafter's open point 3, defaulted; I take it as a scope matter.
- **New fixture files.** `story:standing-tree-rests-quiescent` (`tests/tree_standing.rs` "and its fixtures (new)") and `story:flow-limits-recorded-as-events` (`fixtures/area/ (new)`) both claim to create the `area/` fixtures. `engine-limits.yaml` exists in no story or crate and is created nowhere by name. This is parallel-safety.
- **Bullet-list Acceptance.** Every story uses a bullet-list Acceptance, as `story:tree-daemon` does. I treated that as the store's convention and raised no finding for it.

```findings
- file: .engineering/planning/epic/concurrent-and-standing-case-trees.md
  line: 148
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the limits-change check runs tree limits set --as alice --kind person max_in_progress=1 with no --at, but story:flow-limits-recorded-as-events requires --at INSTANT in that command, so the epic's check exits 2 as written"
- file: .engineering/planning/epic/concurrent-and-standing-case-trees.md
  line: 131
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome promises budgets counted from the latest wake and runs counted per outside revision, and no Acceptance bullet exercises either, so the epic can close with story:standing-tree-wakes-at-a-host-instant and story:run-caps-per-arrival unproven together"
- file: .engineering/planning/story/run-caps-per-arrival.md
  line: 76
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the check 'after two runs the action is refused RunsSpent ... and the action runs twice more' names no way for the observer to be selected again (the selector does not reselect an action performed at the current case revision without retry_when, deterministic-default-selector.md:72-74), and does not say whether the port or the tree is driven, though crates/loom-tree writes the arrived line"
- file: .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md
  line: 29
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says nothing is written when no revision moved but not what is written when a revision moved and no case has a useful action, and no Acceptance bullet covers that state, so the same-instant no-byte-change promise is checked only where no file changed"
- file: .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md
  line: 88
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome moves the wall, CPU, step and agent-call counters to start at the latest woken line, but the only budget check is the per-run wall ceiling, so a step or agent-call count still counted from tree open passes"
- file: .engineering/planning/story/child-case-own-workspace.md
  line: 22
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says a child's agent sessions get its workspace and its own open children inherit it, and no Acceptance bullet opens a grandchild or runs an agent in the child's directory"
- file: .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md
  line: 37
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "tree status is to give each case Queued, Running, Waiting, Stuck, HoldsChildren or Terminal and the served tree Running or Queued, but Acceptance asserts only Waiting and the stop statuses, so a status that always prints Running passes"
- file: .engineering/planning/story/standing-tree-rests-quiescent.md
  line: 86
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says a blocking tree run through the daemon exits 7, and the daemon bullet checks only the resting state, the bounded tree and the restart, so exit 7 is checked for a direct run only"
- file: .engineering/planning/story/capacity-given-to-an-admission-program.md
  line: 75
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "used.wall_seconds is checked against 'the sum the tree's budget counts' with no named reader (tree status prints no totals), and the queued member and the rule that an arguments_from program is not given loom.capacity have no check"
```
