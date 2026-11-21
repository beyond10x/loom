---
format: aep.planning-md/3
id: review-result:concurrent-and-standing-case-trees-design-r4
kind: review-result
status: active
title: Plan critic design, confirming pass (round 4), on epic:concurrent-and-standing-case-trees
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

story:concurrent-cases-in-the-tree-daemon — it writes a `woken` line at a wake of a tree that was not `Quiescent` and says that wake "resets no counter", but story 4 defines the start of both usage windows (and of `--max-steps`) as "the latest `woken` line" or "the latest wake", and story 6 adds no `woken` member and gives no rule that marks which `woken` line starts a budget run; so a restarted daemon, which rebuilds the counters from the run logs, resets them at a non-`Quiescent` wake. The body should say where the counters start (for example a `woken` member such as `new_run`, or "the latest `woken` line written at a wake of a `Quiescent` tree") and check it once after a daemon restart — .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:127,135-138,223-229, .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md:72-75,85,100-102, .engineering/planning/story/tree-usage-and-model-budget.md:67, .engineering/planning/story/capacity-given-to-an-admission-program.md:88

**Round-3 design fixes: all three hold.**
- **`flow` open children (was the blocker).** Story 6 now lets a parent step beside its open children under the daemon, and keeps children first for a direct `tree run` (concurrent-cases.md:60-66). Story 8's daemon bullets can pass as written:
  - `admit-open/1` under three slots takes `kids.open`, then `wave.admit` while `a` and `b` are open (flow-limits.md:138-150).
  - The direct bullet expects an empty list (flow-limits.md:151-153), and the outcome says why (flow-limits.md:31-34).
- **A wake while other cases run resets budgets.** Fixed in story 4 (wakes.md:80-83) and story 6 (concurrent-cases.md:135-138). The hub wake bullet checks `usage.steps` 4 with no reset (concurrent-cases.md:313-321). The finding above is what this left: the in-memory rule holds, but the run log cannot tell the two kinds of `woken` line apart.
- **Conditional kept `TreeBudget`.** Now unconditional, as `TreeRun.budget`; each `tree run` replaces it (wakes.md:96-102, ESS first wakes.md:157).

**Question 2: states and stop order are one definition.**
- `HoldsChildren` is "open children, nothing useful, no wait". `Stuck` needs no open child, so a holding root is never stuck. `Quiescent` holds when every case left is `Resting` or `HoldsChildren` (concurrent-cases.md:99-115). So a root over resting children gives `Quiescent`, and the bullet at :322-325 agrees.
- Bounded trees: a bounded case with nothing useful and no legitimate outcome is `Stuck` (:102-104). The argument that `Quiescent` cannot occur in a bounded tree holds (:116-117): the cases at the ends of a bounded tree have no children and cannot rest.
- **Bullets in the other stories that count slots, states or steps, all consistent:**
  - story 7's `running 2, queued 1, blocked 0, free 0` (capacity.md:143-150);
  - story 4's one-case wake bullets, which stay single-case (wakes.md:69-70);
  - story 1's slot bullet (rests-quiescent.md:132-136);
  - story 8's wake while the root is `HoldsChildren` (flow-limits.md:148-150);
  - story 2 counts no slots.
- `tree wake` needs the daemon (wakes.md:44-45), so no direct-run path lets a parent step beside a resting child.
- **`sdc`'s two conditions are bullets a test can fail.**
  - Condition (a): `nest/1`, where the root is `Blocked` while its child's gate is held, and the root's `intake.read` starts after the child's `out.write` ends and before the child's `step.open` (concurrent-cases.md:297-306). There are also one-writer bullets for one tree and for two trees (:290-296).
  - Condition (b): `open_children` must be present on that line (:306) and absent on the root's first step (:299) and in a direct run (:307-309).

**Question 3: yes, a missing kept budget now fails a test.** The checks are wakes.md:233-237 (a wake stops `BudgetSpent` naming `max_steps` after `tree run --max-steps 1`), :250-254 (agent calls, plus the variant with no budget flag) and :257-259 (cost). Each passes only if the latest `tree run`'s budget is kept and replaced.

**Question 4: no fix the ledger marks done is missing from a body, in my lane.**

**What I read:**
- 8 stories and the epic as stored. For story 2 and story 5 only the outcome and the lines that touch states, slots and children.
- `budget-and-limit-stops-raise-decision-requests`:130-140.
- The ledger `e4r3-changes.md`, `e4-sdc-answers-3.md` and my round-3 verdict.
- Commands: `aep plan artifact relations`, `validate` (valid; it reports only old review-results outside the set), and `graph --format json` with a cycle search over all 340 `depends_on` edges.
- No cycle. I walked outside the set. The only edge into the set from outside is still budget-and-limit-stops → story 3. No path either way between budget-and-limit-stops and story 4 or story 6 (decided, § Open 5).
- I traced about 30 acceptance reads. 22 read something a story in the set produces, and an edge records every one, directly or through the chain 1→8.

**What I could not establish:**
- Whether `story:tree-daemon` rebuilds the per-run counters from the logs at a restart, or only the totals since `tree open`. I rest on its quoted sentence (wakes.md:90). If it does not, the finding narrows to the wording in stories 3, 4 and 7.
- Out of my lane (scope / parallel safety): story 6's `## Scope` prose was kept byte for byte and is now stale.
  - It still lists the daemon guide page (concurrent-cases.md:368) and says the story does not name `crates/loom-cli/src/lib.rs` (:367).
  - It omits `ess/domains/runlog.yaml` and the `nest` fixtures (:363-365).
  - It gives story 8 `tree_concurrent.rs` and the guide page (:373).
  - The typed scope entries are correct.
- Out of my lane (acceptance): "the first admission, which starts while both children hold their gates" (flow-limits.md:140) depends on timing. The root keeps its slot and can start `wave.admit` before the children's gates start. The assertion on `open_children` holds either way.

```findings
- file: .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md
  line: 135
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it writes a woken line at a wake of a tree that was not Quiescent and says that wake resets no counter, but story 4 defines the start of both usage windows and of --max-steps as the latest woken line or the latest wake, and story 6 adds no woken member and gives no rule that marks which woken line starts a budget run, so a restarted daemon that rebuilds the counters from the run logs resets them at a non-Quiescent wake; the body should say where the counters start (a woken member such as new_run, or the latest woken line written at a wake of a Quiescent tree) and check it once after a daemon restart"
```
