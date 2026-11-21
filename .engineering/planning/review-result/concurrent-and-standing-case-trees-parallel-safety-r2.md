---
format: aep.planning-md/3
id: review-result:concurrent-and-standing-case-trees-parallel-safety-r2
kind: review-result
status: active
title: Plan critic parallel-safety, round 2, on epic:concurrent-and-standing-case-trees
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

- `story:run-caps-per-arrival` — `story:bound-program-result-table`, an ancestor of this story, now has an acceptance bullet that only this story can satisfy, and no edge can fix that. The bullet reads "An observation ... is never refused `RunsSpent` (`story:run-caps-per-arrival`)". This story depends on `bound-program-result-table`, so an edge from it to this story would be a cycle. Landed first, that story either fails its own bullet or implements the writer/observation rule itself, on the same program port and effect-port rule. This story's `## Waits on` and `## Out of scope` (:85-94, :154-159) say nothing about it. Move the bullet out of `bound-program-result-table`'s acceptance, since this story's acceptance (:131-133) already checks it. Alternatively, state that this story and not the ancestor lands the rule. The revision added the sentence today (`git diff` of `bound-program-result-table.md`, revision 8 to 10). (cited) — `.engineering/planning/story/bound-program-result-table.md:91-95`, `.engineering/planning/story/run-caps-per-arrival.md:33-37`
- `story:tree-usage-and-model-budget` — `story:budget-and-limit-stops-raise-decision-requests` (outside the set, changed today) now says a stop at `--max-model-cost-micro-usd` "is `budget_overrun`". That flag and `TreeBudget.max_model_cost_micro_usd` are created by this story, and no `depends_on` edge runs from that story to this one. In the graph the two are unordered (`3?`). This story still says the cost stop's fate "is under the work order's § Open" (:70-72), which the sibling now contradicts. Add `budget-and-limit-stops-raise-decision-requests depends_on story:tree-usage-and-model-budget`, or drop the cost clause from the sibling. Update this story's Why to match. The epic's § Order (:158-159) names only "TreeBudget members beside 3's" for this pair. (cited) — `.engineering/planning/story/budget-and-limit-stops-raise-decision-requests.md:25-27`, `.engineering/planning/story/tree-usage-and-model-budget.md:68-72`
- `epic:concurrent-and-standing-case-trees` — § Order names two stories as unordered sharers of `ess/domains/tree.yaml`, `runlog.yaml` and `crates/loom-tree` (`tree-waits-open-decision-requests` and `budget-and-limit-stops-raise-decision-requests`). Several more share these files with all eight stories and have no path to or from them in the graph:
  - `non-independent-review-becomes-decision-request`: `runlog.yaml` and `tree.yaml` (`ReviewStop`), plus a module of `crates/loom-tree`.
  - `authority-records-bound-and-grounded-in-canon`: `tree.yaml` (`CaseApproval`) and `crates/loom-governor/src/lib.rs`, which story 1's `next_expiry` also edits.
  - `run-log-references-exec-file-changes`: the `action finished` event in `runlog.yaml`, which story 3 also extends with `cost_micro_usd`.
  - `requests-from-canon-pending-decisions`: `crates/loom-tree`.

  The store convention is "the second to land rebases" (`tree-waits-open-decision-requests.md:129-134`). Say that it covers these too, naming them. Or record an ordering edge per pair, or split the surface. (cited; file-level append conflicts, not a semantic clash) — `.engineering/planning/epic/concurrent-and-standing-case-trees.md:157-160`, `.engineering/planning/story/non-independent-review-becomes-decision-request.md:110-112`, `.engineering/planning/story/authority-records-bound-and-grounded-in-canon.md:71-78`, `.engineering/planning/story/run-log-references-exec-file-changes.md:48`

**Round-1 fixes that hold in the stored bodies**
- All seven edges exist: 2→1, 3→2, 4→3, 5→4, 6→5, 7→6 and 8→7 are all stored (the sorted order is in `aep plan artifact graph --format json`).
- 8→1, 8→2 and 8→4 are stored as well. 2 and 8 now list `tree-run-command`.
- 6→`tree-waits-open-decision-requests` is stored. 7→4 is stored.
- Both ends of every chain edge name the shared files: 1/2 (:81-84, :87-89), 2/3, 3/4, 4/5, 5/6, 6/7, 7/8.
- `used` in story 7 is now the run's usage with `since_open` beside it (:34-39).
- The fixture directory has one creator (story 1, :45-49), and each later story adds named files. The new file names (`draft.*`, `agents.*`, `wave/item/step.*`, `engine-daemon-{1,2,budget}.yaml`, `engine-limits.yaml`, `admit*.yaml`, `bin/{spin,gate,copy-stdin}`) do not collide.
- The graph has no cycle. Nothing outside the set is an ancestor of anything in it, which also means `tree-waits-open-decision-requests` reaches none of the eight.
- The epic's shared-file list (:133-142) matches the eight `## Files` sections, except as noted in the last line.

**What you read:** 9 artifacts (the epic and 8 stories), through `aep plan artifact graph --format json`, `waves` and `git diff` on the six outside bodies touched today. I read the round-1 verdict and `e4r1-changes.md`, and the 18 outside stories that touch `tree.yaml`, `runlog.yaml` or `crates/loom-tree`. I opened these citations and they hold: `selection.rs:285-305` and `:319`; `governor/src/lib.rs:1184-1196` and `:1938-1954`; `loom-bindings/src/load.rs:118-123`; `session-executor/src/lib.rs:1833-1852`; `metering.yaml:12-16`; `bindings.yaml:24-30`; `orchestrate.protocol.yaml:26-30`. `ess/domains/{tree,runlog,engine}.yaml` do not exist at `39f4da88`. Each is created by an ancestor (`tree-run-command`, `hash-chained-run-log`, `engine-configuration-document`), as the epic says (:144).

**Surfaces:** 9 cited, 0 inferred, 0 unplaced. Four surfaces inside cited stories are modules the bodies themselves mark "not read": the `open` port, the program and agent ports, and the scheduler module.

**Acceptance reads traced to a producer in the set:** 20 (2←1, 3←2, 4←1/2/3, 5←1, 6←1/2/3/5, 7←3/4/6, 8←1/2/4/5/6/7). A direct `depends_on` edge records 13 of them. The other 7 run through a path of chain edges, so none is unordered. Two further reads cross the set boundary without a usable edge. Those are the first two findings.

**What I could not establish**
- Story 5 hands the case's workspace to programs and agent sessions and resolves placed artifacts under it. Its `## Files` and the epic's port lines (program port 2 and 7, agent port 2 and 3) omit the ports. The chain orders it after those stories anyway, so this is a gap in the list, not a collision. I did not raise it as a finding.
- Whether story 3's cost reaches the line without editing `crates/loom-session-executor`. `ExecutorUsage.cost_micro_usd` already exists (`lib.rs:1852`).
- Out of my lane: whether the one-writer-per-workspace rule in story 6 (:28-30) reaches overlapping directories of different served trees. Story 5's overlap refusal covers one tree only.
- I broke the read-only brief once. `aep plan artifact graph --format json` was redirected into `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/g.json`, which may have overwritten an earlier graph dump. Nothing under the tree or `.engineering/` was written.

```findings
- file: .engineering/planning/story/run-caps-per-arrival.md
  line: 33
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "story:bound-program-result-table, which this story depends on, now carries an acceptance bullet (:91-95) that only this story can satisfy (an observation is never refused RunsSpent), and the edge cannot run the other way without a cycle; the body does not say who lands the rule, so the ancestor lands red or implements the rule itself on the same program port; move the bullet out of that story's acceptance or state that this story lands it"
- file: .engineering/planning/story/tree-usage-and-model-budget.md
  line: 70
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "story:budget-and-limit-stops-raise-decision-requests now names --max-model-cost-micro-usd (:25-27) as a budget_overrun stop, a flag this story creates, with no depends_on edge from that story to this one, while this body still calls the cost stop's fate open; add the edge or drop the clause from the sibling, and update this story's Why"
- file: .engineering/planning/epic/concurrent-and-standing-case-trees.md
  line: 157
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Order paragraph names two unordered sharers of tree.yaml, runlog.yaml and crates/loom-tree, but story:non-independent-review-becomes-decision-request, story:authority-records-bound-and-grounded-in-canon, story:run-log-references-exec-file-changes and story:requests-from-canon-pending-decisions also share them (and governor lib.rs with story 1) with no path to or from the eight; name them under the second-to-land-rebases rule or record an edge per pair (file-level append conflicts, cited)"
```
