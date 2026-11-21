---
format: aep.planning-md/3
id: review-result:concurrent-and-standing-case-trees-design-r3
kind: review-result
status: active
title: Plan critic design, round 3, on epic:concurrent-and-standing-case-trees
relations:
- reviews: epic:concurrent-and-standing-case-trees
- reviews: story:flow-limits-recorded-as-events
- reviews: story:capacity-given-to-an-admission-program
- reviews: story:concurrent-cases-in-the-tree-daemon
- reviews: story:child-case-own-workspace
- reviews: story:run-caps-per-arrival
- reviews: story:standing-tree-rests-quiescent
- reviews: story:standing-tree-wakes-at-a-host-instant
- reviews: story:tree-usage-and-model-budget
revision: 1
---
needs-revision

story:flow-limits-recorded-as-events — the `flow` member hands the asking case "its open children", but story:concurrent-cases-in-the-tree-daemon (like story:tree-run-command) lets a case step only once it has no open child, so the list is empty at every admission and the bullet that sees "the root's two open children" cannot pass; the body should hand the open children of the tree (or of every case) rather than the asking case's, or story 6 should let a parent step while its children are open — .engineering/planning/story/flow-limits-recorded-as-events.md:35-36,82-84,93-94, .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:21-22,27-28, .engineering/planning/story/tree-run-command.md:61

story:concurrent-cases-in-the-tree-daemon — it lets a wake act while other cases are Running, Queued or Waiting and keeps story 4's rule that such a wake starts a new budget run, so waking one resting case resets the per-run counters of the cases running beside it and drops any budget extension they hold; the body should say that only a wake of a tree that was `Quiescent` starts a new run (or say how the budget runs per case) — .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:57-58,64-66, .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md:57-60,66-69

story:standing-tree-wakes-at-a-host-instant — whether its ESS commit adds the kept `TreeBudget` depends on story:tree-daemon ("this story adds it if not", "if it does not already"), but story:tree-daemon's body keeps no budget for a submitted tree, so the body should add `TreeRun`'s kept budget without the condition — .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md:60-62,110, .engineering/planning/story/tree-daemon.md:31-55

**Round-2 fixes, both hold:**
- Cost stop: there is now a `depends_on` edge from story:budget-and-limit-stops-raise-decision-requests to story:tree-usage-and-model-budget. That story has a cost bullet, and the stale § Why sentence is gone (tree-usage.md:47-49; budget-and-limit-stops.md:17, 112-120).
- Wake: story 6 now states the wake over per-case states and checks it with a bullet (concurrent-cases.md:55-66, 190-196). Story 4 hands that condition over (wakes.md:42-47).

**The design changes under review:**
- **Keeping the slot: fairness.**
  - A case's run of steps without giving back its slot is bounded by the writer's `max_runs`, by once per case revision for observations, and by the tree's per-run budgets.
  - Arrivals that come while the case runs keep it runnable, so a busy standing case holds its slot until a per-run budget stops it. The second finding removes the one thing that resets that budget mid-run.
  - On one wave, the first `run_slots` items keep their slots until they first wait, hold children, rest or end, and the rest queue. That is the stated cost (concurrent-cases.md:101-102) and open point 2 of `e4r2-changes.md` § Open.
- **Keeping the slot: bullets that count slots.**
  - Unchanged: story 1's slot bullet (rests-quiescent.md:125-129) and story 4's tree-level queue.
  - Consistent with it: story 6's first bullet and its oldest-first bullet (concurrent-cases.md:163-175), story 7's `running 2, queued 1` (capacity.md:123-129) and the epic's first bullet.
- **`max_runs` optional:** inside the set only story 2 reads it. Its readers outside the set are the program and agent ports, which are ancestors and are in story 2's Files. No open story outside the set reads the member.
- **CPU engine file:** it is not a design matter. No finding.

**The graph:**
- No cycle: a search over all 317 `depends_on` edges of the store.
- From the set I walked 98 edges through 48 nodes, outside the set into both parent epics.
- The only edge into the set from outside is the new one from budget-and-limit-stops to story 3.
- The chain 1→8 is decided, and every edge that exists only for a shared file names the file.

**What I read:**
- Commands: `aep plan artifact show`, `relations`, `graph --format json` (reachability and a cycle search), `validate`, and `rg` over the store and the tree.
- Whole bodies: the epic and the 8 stories.
- The diffs of story:bound-program-result-table and story:budget-and-limit-stops-raise-decision-requests.
- story:tree-daemon and story:tree-run-command where cited; the ledger and my round-2 verdict.
- Sources: orchestration.md §§ 2.4 and 2.4a, orchestrate.protocol.yaml:160-260, and `e3-sdc-answers-5.md`, `e4-sdc-answers-2.md`.
- I traced 35 acceptance reads. 19 of them read something a story in the set produces, and an edge records all 19, directly or through the chain. The exception is story 8's open children of the asking case, which no story can produce (first finding).
- `validate` reports nothing about the set.

**What I could not establish:**
- Whether the area root of the SDC can ever run `wave.admit` while its idea or story children are open. Under Loom's rule that a parent steps only with no open child, it cannot. That is a question for the scope critic, and the first finding is its symptom inside the set.
- Out of my lane (scope):
  - story 2's Files omit `crates/loom-bindings/tests/adversary_w20261121w1_u2.rs:370`, which asserts `budget.max_runs == 8` and stops compiling once the member is optional.
  - Its Files also omit `AGENTS.md:1146-1147`, "an action with no `budget.max_runs` holds 8", which stops being true.
- Out of my lane (acceptance):
  - story 6's stop order has no state for a standing `hub/1` whose root is `HoldsChildren` above `Resting` children. "Quiescent when every case left is `Resting`" excludes the root, yet the bullet expects exit 7 (concurrent-cases.md:47-52, 199).
  - story 6 does not say whether a case that holds a slot also holds its workspace between two steps.
  - `engine-daemon-cpu.yaml` names no adapter (wakes.md:133).
- I did not run `aep plan artifact waves`: no story is scoped yet.

```findings
- file: .engineering/planning/story/flow-limits-recorded-as-events.md
  line: 35
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the flow member hands the asking case its open children, but story:concurrent-cases-in-the-tree-daemon (like story:tree-run-command) lets a case step only once it has no open child, so the list is empty at every admission and the bullet that sees the root's two open children cannot pass; the body should hand the open children of the tree (or of every case) rather than the asking case's, or story 6 should let a parent step while its children are open"
- file: .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md
  line: 57
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it lets a wake act while other cases are Running, Queued or Waiting and keeps story 4's rule that such a wake starts a new budget run, so waking one resting case resets the per-run counters of the cases running beside it and drops any budget extension they hold; the body should say that only a wake of a tree that was Quiescent starts a new run (or say how the budget runs per case)"
- file: .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md
  line: 60
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "whether its ESS commit adds the kept TreeBudget depends on story:tree-daemon (this story adds it if not; if it does not already), but story:tree-daemon's body keeps no budget for a submitted tree, so the body should add TreeRun's kept budget without the condition"
```
