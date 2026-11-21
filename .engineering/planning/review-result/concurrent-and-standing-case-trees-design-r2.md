---
format: aep.planning-md/3
id: review-result:concurrent-and-standing-case-trees-design-r2
kind: review-result
status: active
title: Plan critic design, round 2, on epic:concurrent-and-standing-case-trees
relations:
- reviews: epic:concurrent-and-standing-case-trees
- reviews: story:flow-limits-recorded-as-events
- reviews: story:capacity-given-to-an-admission-program
- reviews: story:child-case-own-workspace
- reviews: story:tree-usage-and-model-budget
- reviews: story:concurrent-cases-in-the-tree-daemon
- reviews: story:run-caps-per-arrival
- reviews: story:standing-tree-rests-quiescent
- reviews: story:standing-tree-wakes-at-a-host-instant
revision: 1
---
needs-revision

story:tree-usage-and-model-budget — its cost stop is half of a seam with a story outside the set, and no edge says which side lands first. Its Outcome makes a stop at `--max-model-cost-micro-usd` end the run with exit 5, and its § Why still says "whether the cost stop joins it is under the work order's § Open". But the pointer added today to story:budget-and-limit-stops-raise-decision-requests (§ Outcome) already makes that stop a `budget_overrun` request. Neither story has a `depends_on` edge to the other, and that story's Acceptance has no check of the cost stop. Fix: either story:budget-and-limit-stops-raise-decision-requests depends_on this story and adds a cost check, or this story depends_on that one and raises the request itself. Neither edge would make a cycle. Either way, the stale § Why sentence goes. — .engineering/planning/story/tree-usage-and-model-budget.md:38-42,70-72, .engineering/planning/story/budget-and-limit-stops-raise-decision-requests.md:26-27,87-109, `aep plan artifact graph --format json`

story:concurrent-cases-in-the-tree-daemon — story 4 hands it the per-case scheduling of a woken tree, and its body never mentions a wake. Story 4 lets a wake act only on a tree that is `Quiescent` as a whole. This story makes a tree `Quiescent` only when every case left is `Resting`. So a standing tree with one case `Resting` and another `Waiting` refuses every wake, and nothing says when the resting case is evaluated again (that evaluation is where its arrival would be detected). The body should say how a wake treats per-case states, and that woken cases queue per case. — .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md:38-39,160-161, .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:42-48 (no "wake" in the body: `rg -i wake` returns nothing)

**Round-1 fixes, all five hold in the stored bodies:**
- story 8 has edges to 1, 2 and 4 (flow-limits.md:14-16).
- story 2 has an edge to `story:tree-run-command`, and its § Waits on is corrected (run-caps.md:16,90).
- story 6 depends on story 1 and places `Resting`/`Quiescent` last in its stop order (concurrent-cases.md:15,39-48).
- story 4 no longer cites story 2 and now queues "as `story:tree-daemon` says" (wakes.md:32,160-161).
- story 4 takes its moved artifacts from the arrival detection of run-caps, and says why both `arrived` and `woken` exist (wakes.md:14,25-27,42-46).

**The chain:** the eight stories form one queue: 1→2→3→4→5→6→7→8, as the epic's § Order lists them (epic.md:130-131). Every edge that exists only for a shared file names the file in both bodies. The coordinator decided this shape, and it is not raised here.
- No edge contradicts a need. No story needs a later one.
- The new story 3 is not a split abstraction with 4 or 7: 3 defines `TreeUsage` from its budget's counting line, 4 moves that line and adds `since_open`, and 7 hands the result to a program. Each step works and can be shown on its own.
- The edges from 8 to 1, 2 and 4 repeat what the chain already gives and are labelled as such (flow-limits.md:64-67).

**What I read:**
- Whole bodies: the epic, all 8 stories and story:budget-and-limit-stops-raise-decision-requests. Pointer lines in five other stories.
- Commands: `aep plan artifact show`, `relations`, `graph --format json` (reachability and a cycle search over the whole store), `validate`, and `rg` over the store.
- I walked 93 `depends_on` edges through 47 nodes, outside the set into both parent epics. There is no cycle, and no `depends_on` edge points into the set from outside.
- I traced 18 acceptance reads to a producer in the set. All 18 have an edge, directly or through the chain.
- `validate` reports nothing about this set.

**What I could not establish:**
- Stories 1 and 4 name "a decision request" wait in their rest and wake conditions, while the epic says stories 1 to 5 do not wait on `story:tree-waits-open-decision-requests`. This is open point 11's assumed default, and no source contradicts it, so it is not a finding.
- A `budget extended` line carried over a wake (ledger § Open 7) is the coordinator's open question. I did not trace it.
- Out of my lane (acceptance):
  - story 2's agent-writer check (run-caps.md:134) names no agent protocol fixture. The `agents.protocol.yaml` fixture is created later, by story 3 (tree-usage.md:98).
  - story 5's `workspace_agent.rs` binds a stub adapter to a writer that the item bindings bind to a program (child-case-own-workspace.md:111).
- I did not run `aep plan artifact waves`: no story is scoped yet.

```findings
- file: .engineering/planning/story/tree-usage-and-model-budget.md
  line: 70
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its cost stop ends the run with exit 5 and its Why still calls joining budget_overrun open, while story:budget-and-limit-stops-raise-decision-requests now makes a stop at --max-model-cost-micro-usd a budget_overrun request; no depends_on edge orders the two and neither acceptance checks the cost request, so add either budget-and-limit-stops depends_on this story with a cost check, or this story depends_on that one and raises the request, and drop the stale Why sentence"
- file: .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md
  line: 42
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "story:standing-tree-wakes-at-a-host-instant hands it the per-case scheduling of a woken tree, but its body never mentions a wake; under its states a standing tree with one Resting and one Waiting case is not Quiescent, so every wake is refused and nothing says when the resting case is evaluated again, so the body should say how a wake treats per-case states and that woken cases queue per case"
```
