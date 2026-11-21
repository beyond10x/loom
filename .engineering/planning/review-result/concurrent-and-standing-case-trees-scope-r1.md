---
format: aep.planning-md/3
id: review-result:concurrent-and-standing-case-trees-scope-r1
kind: review-result
status: active
title: Plan critic scope, round 1, on epic:concurrent-and-standing-case-trees
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

epic:concurrent-and-standing-case-trees — § Order says the epic follows the daemon epic's chain "as the requester ordered ('behind wave 2026-11-21-w1 and the decision-request epic')", but no edge places the set behind `epic:decision-requests-in-the-runtime`, and that epic does not depend on the daemon epic either. The body therefore claims an order that nothing holds. It should say the decision-request part of the order is not held (work order § Open 11 says so, the body does not) or add the edge. — .engineering/planning/epic/concurrent-and-standing-case-trees.md:114 (request: .engineering/drafts/e4-request.md:3)
story:standing-tree-wakes-at-a-host-instant — Every host wake resets the engine's per-run wall and CPU totals and the tree's step and agent-call counts ("Budgets of a standing tree count from its latest `woken` line"). The request does not ask for this: it asks for "run caps counted per revision", which `story:run-caps-per-arrival` claims. Neither the body nor the work order's § Open records this reset as the drafter's choice, although it loosens a ceiling that `story:action-limits-enforced` lets a binding "only lower". — .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md:34 (request: .engineering/drafts/e4-request.md:17-19)

**Promises**
- I wrote down 15 promises before reading the set: 14 from e4-request.md items 2-4, plus "the governor never completes a case that has no legitimate outcome" from the `loom-standing-trees` blocker.
- All 15 trace to an item or to a recorded exclusion:
  - siblings at once, a slot per running case, one writer per case: story 2
  - "each story in its own work tree": story 1. Reframed as a workspace per case, as the coordinator decided.
  - no slot while quiescent: story 3
  - wake on a new revision or a host instant: story 4. Host-driven only, which § Open 2 and epic § Not in scope record.
  - run caps per revision: story 5. Read as per arrival, recorded in § Open 3.
  - ceilings and free slots: story 6
  - the four limits, `limits changed` (who, old, new), read at the next admission, never by an LLM, never retroactive: story 7
  - the governor rule: already held; epic § Why and story 3 say so.
- Every narrowing I saw has its decision recorded:
  - a direct `tree run` stays sequential
  - build slots and model budget are left out (§ Open 10)
  - limits live per tree instead of in the area's configuration (§ Open 6)
  - KPIs F1-F17 are not computed by Loom
- No two items claim one outcome. Nothing else in the store claims part of the request: grep for `limits changed`, `max_in_progress`, `runs_per`, `loom.capacity`, `Quiescent` and `--standing` finds only this set and the pointers added today.

**What I read:** the request, work order e4-plan.md, orchestration.md §§ 0-3, kpis.md § 5, the three SDC blockers, protocol yaml lines 26-30. In the store, `aep plan artifact show` for the 8 artifacts of the set plus `epic:decision-requests-in-the-runtime` and `story:tree-waits-open-decision-requests`, `story/tree-daemon.md`, cited lines of `child-case-runtime.md` and `tree-run-command.md`, and a grep of `.engineering/planning/` for pointers into the set and overlapping claims.

**What I could not establish, or is outside my lane**
- Design: story 2 holds a slot per step and returns it after each step. `story:tree-daemon` holds one per tree while the tree runs (tree-daemon.md:37). So `free` in story 6 is a value at one instant. Whether that matches SDC admission rule 3 ("story count ≤ free run slots") is a design question.
- Design: stories 2 and 3 treat an open decision request as a wait, but no edge leads to `epic:decision-requests-in-the-runtime`.
- Design or acceptance: line citations have drifted.
  - epic and story 1 cite `child-case-runtime.md:73` and `:158`; the quoted sentences are at `:82` and `:171`
  - epic cites `tree-run-command.md:80`; the sentence is at `:85`
  - epic and story 3 cite `tree-run-command.md:143`; the sentence is at `:148`
  - story 1 cites `bound-artifact-revisions.md:86`; that one is correct
- Design: Open 7. `--kind person` is whatever the caller declares, so "no LLM raises a limit" holds only against declared kinds. Nothing stops a bound agent session from calling the CLI.

```findings
- file: .engineering/planning/epic/concurrent-and-standing-case-trees.md
  line: 114
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "§ Order says the epic follows the daemon epic's chain as the requester ordered (behind wave 2026-11-21-w1 and the decision-request epic), but no edge places the set behind epic:decision-requests-in-the-runtime and that epic does not depend on the daemon epic, so the body claims an order nothing holds; it should say the decision-request part of the order is not held or add the edge"
- file: .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md
  line: 34
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the story resets the engine's per-run wall and CPU totals and the tree's step and agent-call counts at every host wake, which the request does not ask (it asks run caps counted per revision, claimed by story:run-caps-per-arrival), and neither the body nor the work order's § Open records it as the drafter's choice, although it loosens a ceiling a binding may only lower"
```
