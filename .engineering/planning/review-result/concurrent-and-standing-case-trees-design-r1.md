---
format: aep.planning-md/3
id: review-result:concurrent-and-standing-case-trees-design-r1
kind: review-result
status: active
title: Plan critic design, round 1, on epic:concurrent-and-standing-case-trees
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

story:flow-limits-recorded-as-events — its acceptance runs the epic's checks in `tree_concurrent_standing.rs` and a guide page showing "a standing tree, a wake", which read `Quiescent` and `tree wake` produced by story:standing-tree-rests-quiescent and story:standing-tree-wakes-at-a-host-instant, and no depends_on path runs from it to either (its only path is 7→6→2→1) — .engineering/planning/story/flow-limits-recorded-as-events.md:87-89, .engineering/planning/epic/concurrent-and-standing-case-trees.md:139-143, `aep plan artifact graph --format json`

story:run-caps-per-arrival — its acceptance file `crates/loom-tree/tests/arrivals.rs` and its `arrived` writer live in `crates/loom-tree`, which story:tree-run-command creates, and no depends_on path reaches story:tree-run-command, although § Waits on says "Through them ... story:tree-run-command" (its edges lead only to bound-program-result-table, bound-agent-node and bound-artifact-revisions, none of which reach it), so a depends_on edge to story:tree-run-command is missing — .engineering/planning/story/run-caps-per-arrival.md:60,74,95, .engineering/planning/story/tree-run-command.md:57, `aep plan artifact graph --format json`

story:concurrent-cases-in-the-tree-daemon — its list of served-tree states (`Running`, `Queued`, `Terminal`, `Waiting`, `Stuck`) has no `Quiescent`, while story:standing-tree-rests-quiescent defines `Quiescent` using this story's term "runnable" and its own slot release; both stories rewrite the one classification of when a served tree stops or rests and no edge orders them, so story 2 should depend_on story:standing-tree-rests-quiescent and say where `Quiescent` falls in its states (story 2 is held behind Canon 0.9.0 through story:child-case-runtime, so story 3 will land first) — .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:33-36, .engineering/planning/story/standing-tree-rests-quiescent.md:22-27, .engineering/planning/epic/concurrent-and-standing-case-trees.md:123-125

story:standing-tree-wakes-at-a-host-instant — its outcome has a woken tree queue for slots and rest or stop "as ... story:concurrent-cases-in-the-tree-daemon say[s]", but no depends_on path runs from it to that story; either drop the reference or add the edge, which would hold wake behind Canon 0.9.0 — .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md:27-28,60, `aep plan artifact graph --format json`

story:standing-tree-wakes-at-a-host-instant — its `woken` line records "the artifacts whose revision moved" per case. That is the same outside-move detection story:run-caps-per-arrival records as `arrived` before evaluation, with two reset points: budgets reset at `woken`, run caps at `arrived`. The two stories are unordered, so the body should either take the moved artifacts from the arrival detection (depends_on story:run-caps-per-arrival) or say why both lines exist — .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md:25,34,68, .engineering/planning/story/run-caps-per-arrival.md:24-28,66

**What I read:** 8 artifacts (the epic and 7 stories, whole bodies), plus story:bound-artifact-revisions, the relevant parts of story:tree-run-command, story:tree-daemon and story:tree-waits-open-decision-requests, and `.engineering/drafts/e4-plan.md` § Open. Commands: `aep plan artifact show`, `relations`, `graph --format json` and `validate`, plus rg/ls on the tree at `39f4da88`. I walked 57 depends_on edges through 33 nodes, going outside the set into both parent epics. There is no cycle, and no node outside the set points back into it. I traced 9 acceptance reads to a producer inside the set; an edge records 7 of them, directly or through other stories, and 2 are unrecorded (story 7 reading stories 3 and 4). `validate` reports nothing about this set.

**What I could not establish:**
- The open-point-11 default (stories 2 and 3 treat decision-request waits as waits, with no edge to story:tree-waits-open-decision-requests) is an assumed default, and no source I read contradicts it. Not a finding.
- story:budget-and-limit-stops-raise-decision-requests changes budget stops, while story 4 moves where budgets start counting. I did not trace their interaction; it is outside the set and may belong to the scope critic.
- Shared files with no edge (`ess/domains/tree.yaml`, `runlog.yaml`, `crates/loom-tree`, `crates/loom-cli/src/lib.rs`) belong to the parallel-safety critic.
- `aep plan artifact waves` cannot place stories that have no scope entries, so I did not run it.
- I did not open the `open` port module of story:child-case-runtime; story 1 names it as "not read".

```findings
- file: .engineering/planning/story/flow-limits-recorded-as-events.md
  line: 87
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its acceptance runs the epic's checks in tree_concurrent_standing.rs and a guide page showing a standing tree and a wake, which read Quiescent and tree wake produced by story:standing-tree-rests-quiescent and story:standing-tree-wakes-at-a-host-instant, and no depends_on path runs from it to either (its only path is 7 to 6 to 2 to 1)"
- file: .engineering/planning/story/run-caps-per-arrival.md
  line: 74
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its acceptance file crates/loom-tree/tests/arrivals.rs and its arrived writer live in crates/loom-tree, which story:tree-run-command creates, and no depends_on path reaches story:tree-run-command although Waits on claims one through its dependencies, so a depends_on edge to story:tree-run-command is missing"
- file: .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md
  line: 33
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its served-tree states have no Quiescent while story:standing-tree-rests-quiescent defines Quiescent with this story's term runnable and its own slot release; both rewrite the one stop-or-rest classification with no edge between them, so this story should depend_on story:standing-tree-rests-quiescent and say where Quiescent falls in its states"
- file: .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md
  line: 27
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its outcome has a woken tree queue for slots and rest or stop as story:concurrent-cases-in-the-tree-daemon says, but no depends_on path runs from it to that story; either drop the reference or add the edge, which would hold wake behind Canon 0.9.0"
- file: .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md
  line: 25
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its woken line records the artifacts whose revision moved per case, the same outside-move detection story:run-caps-per-arrival records as arrived before evaluation, with budgets reset at woken and run caps at arrived in two unordered stories; it should take the moved artifacts from the arrival detection (depends_on story:run-caps-per-arrival) or say why both lines exist"
```
