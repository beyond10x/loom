---
format: aep.planning-md/3
id: review-result:concurrent-and-standing-case-trees-acceptance-r4
kind: review-result
status: active
title: Plan critic acceptance, confirming pass (round 4), on epic:concurrent-and-standing-case-trees
relations:
- reviews: epic:concurrent-and-standing-case-trees
- reviews: story:capacity-given-to-an-admission-program
- reviews: story:concurrent-cases-in-the-tree-daemon
- reviews: story:run-caps-per-arrival
- reviews: story:child-case-own-workspace
- reviews: story:tree-usage-and-model-budget
- reviews: story:standing-tree-rests-quiescent
- reviews: story:flow-limits-recorded-as-events
- reviews: story:standing-tree-wakes-at-a-host-instant
revision: 1
---
needs-revision

All my round-3 fixes hold, and so do the `sdc` changes I checked. Four warnings remain, and the new rule introduced all four. None is a blocker. Line numbers are lines of the files under `.engineering/planning/`.

- story:concurrent-cases-in-the-tree-daemon — The `Stuck` entry has a second branch, "or a bounded tree with no legitimate outcome (an open obligation is not needed …)", and that branch does not repeat "no open child". So a bounded root that holds children and has nothing useful fits both `HoldsChildren` and `Stuck`, while the bullets expect `HoldsChildren` (`:271-272`, `:311`) — `.engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:102-105` against `:99`
- story:concurrent-cases-in-the-tree-daemon — The bullet "a parent beside its child" claims an order across two run logs: "the root's `intake.read` starts after the child's `out.write` finished and before the child's `step.open` starts". It names nothing that records that order. Every run-log line carries the `--at` instant, and the two cases run under different `--at` values (`T` for the child, `T+60s` for the root; `story:standing-tree-wakes-at-a-host-instant` `:56`, `:193-194`). `intake.read` (`bin/sleep1`) and `step.open` (a `case_open`) write no time from the host clock — `.engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:303-306`
- story:concurrent-cases-in-the-tree-daemon — The bullet "while one is held the others show `Blocked` or `Queued`, never `Running` with a step in flight" asks for a difference `tree status` cannot show. An item whose gate was released keeps its slot and waits for the workspace, so it shows `Running`, and `Running` covers "a step in flight, between two steps, or waiting for its workspace". The part a test can check is the order of the `bin/gate` start instants and the releases — `.engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:290-292` against `:81-82`, `:96`
- story:concurrent-cases-in-the-tree-daemon — The direct `tree run` bullet for `nest/1` with `nest-plain.bindings.yaml` cannot fail, even if a direct run let a parent step beside its children. After `kids.open` the root has nothing useful, because `intake` did not move and `intake_read` is counted (`:260-261`, `crates/loom-executor/src/selection.rs:298-304`). So "the child's steps before the root's next step" and "no `open_children`" both hold either way — `.engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:307-309`

**What I read:** 9 of 9 ids (the epic and the eight stories), each read in full from its stored file, or its Outcome and Acceptance:
- the round-3 verdicts, `e4r3-changes.md`, `e4-sdc-answers-3.md`, and the scope reports for stories 5 and 6;
- the e4r2→e4r3 diff of story 5;
- from other stories: `story:hash-chained-run-log` (the `at` on each line) and `story:tree-daemon` (counters from `tree open`);
- in the tree: `selection.rs:270-340`, `loom-governor/src/lib.rs:420-446` and the `CompletionRule` sites, `decode.rs:25-29` and `:297-350`, `load.rs:226-246`, `loom-session-executor/src/lib.rs:1830-1834`.

**Answers to the brief:**
1. **Round-3 fixes.** All 8 of my round-3 findings are fixed. Kept-budget bullets, 2099 instants, `--max-steps` restated per run, the hub stop that includes `HoldsChildren`, the racy state removed, A/B/C on their own workspaces, `admit-open/1` with `wave.admit` beside gated children, and `bin/gate-copy` plus a next admission after an append and a wake. I checked each in the body.
2. **States and stop order.** With the first finding above fixed, they form one consistent definition. A root over only resting children gives `Quiescent` (`:115-116`), and a bounded case with no obligation and nothing useful is `Stuck`. Every bullet in the other seven stories that counts slots, states or steps still holds under the new rule:
   - capacity: `running 2, queued 1` holds;
   - standing capacity: `used.steps` 1 with `since_open` 3, then 5;
   - flow: three slots for the root and its two gated children, and the wake acts on the holding root;
   - the wakes story's `tree_wake.rs` bullets hold under the per-case wake.
   - On `sdc`'s two conditions: (b) can fail as written, since the nest bullet requires `open_children` present and the first `intake.read` line requires it absent. (a) can fail through the two-tree `Blocked` bullet and the `bin/gate` instants. The "parent beside its child" form of (a) has no observable order (finding 2).
3. **Kept `TreeBudget`.** Yes. In `wake_budgets.rs` (`standing-tree-wakes-at-a-host-instant.md:250-259`), the wake after a run under `--max-agent-calls 1` (or cost `1500`) must stop `BudgetSpent` before `reply.write`. With no budget kept it would take both steps and return `Quiescent`, so the check fails. The run with no flag gives the control case.
4. **Scope-report fixes.**
   - Ledger item 66 says a new bullet was added for "an inheriting child against an entry naming the root's directory". The `root/` refusal bullet was already in e4r2 (`e4r2-story-child-case-own-workspace.md:115`). The defect is still closed: the Outcome was narrowed to "a grandchild without `workspace` is never refused `Overlaps`" (`child-case-own-workspace.md` § Outcome), and the first bullet checks that.
   - I found no other item the ledger marks fixed that the body leaves open.

**What I could not establish:**
- Which instant a child's step after the wake carries in the nest bullet: `T`, `T+60s`, or one from the host clock (`e4r1` § Open 8).
- How `bin/gate-copy` learns "the count of its runs" under a cleared environment.

**Out of my lane, not setting the verdict:**
- `standing-tree-wakes-at-a-host-instant.md:68` says the later story makes a wake act on "the tree's `Resting` cases". Story 6 `:123-124` says `Resting` and `HoldsChildren`. The design critic would raise that.
- The epic bullet "two run at once (their run logs overlap in time)" (`epic/concurrent-and-standing-case-trees.md:251-252`) has the same open question as finding 2 if the trees run under `--at`.

```findings
- file: .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md
  line: 102
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Stuck entry's branch 'or a bounded tree with no legitimate outcome' does not repeat 'no open child', so a bounded root holding children with nothing useful matches both HoldsChildren and Stuck while the bullets at :271-272 and :311 expect HoldsChildren"
- file: .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md
  line: 303
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the parent-beside-its-child bullet claims the root's intake.read starts after the child's out.write finished and before the child's step.open starts, but names nothing that records an order across two run logs: lines carry the --at instants (T for the child, T+60s for the root) and neither bin/sleep1 nor the case_open step writes a host-clock time"
- file: .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md
  line: 290
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the one-tree one-writer bullet expects the others to show Blocked or Queued, never Running with a step in flight, but a released item keeping its slot while it waits for the workspace shows Running and tree status cannot tell a step in flight from a wait, so only the bin/gate start instants and releases are checkable"
- file: .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md
  line: 307
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the direct tree run bullet for nest/1 with nest-plain.bindings.yaml cannot fail if a direct run stepped a parent beside its children, because after kids.open the root has nothing useful (intake did not move, intake_read is counted), so 'child's steps before the root's next step' and 'no open_children' hold either way"
```
