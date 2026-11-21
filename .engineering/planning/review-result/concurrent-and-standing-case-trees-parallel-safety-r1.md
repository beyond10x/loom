---
format: aep.planning-md/3
id: review-result:concurrent-and-standing-case-trees-parallel-safety-r1
kind: review-result
status: active
title: Plan critic parallel-safety, round 1, on epic:concurrent-and-standing-case-trees
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

- `story:flow-limits-recorded-as-events` — its acceptance reads what `story:standing-tree-rests-quiescent` and `story:standing-tree-wakes-at-a-host-instant` produce, and no `depends_on` edge reaches either. The acceptance says "The epic's acceptance checks pass in `tree_concurrent_standing.rs`" and "a guide page shows a standing tree, a wake and a limit change". Those checks need `Quiescent` and `tree wake`. The edge chain 7→6→2→{1, tree-daemon} reaches neither. Add the edges 7→3 and 7→4, or merge. (cited) — `.engineering/planning/story/flow-limits-recorded-as-events.md:87-89`, `story/standing-tree-rests-quiescent.md:77-85`, `story/standing-tree-wakes-at-a-host-instant.md:13-29`.

- `story:run-caps-per-arrival` — it lands in `crates/loom-tree` and writes the `arrived` line before `evaluated`, but no edge reaches `story:tree-run-command`, which creates that crate and the run. The body's "Through them … `story:tree-run-command`" is false. The dependency runs the other way: `tree-run-command` depends on `bound-artifact-revisions`. The graph's transitive ancestors of this story do not include `tree-run-command`. `crates/loom-tree` is not in the tree at `39f4da88`. Add `depends_on story:tree-run-command`, or drop `crates/loom-tree` and `arrivals.rs` from the story. (cited) — `story/run-caps-per-arrival.md:60`, `:74`, `:95-96`, `story/tree-run-command.md:37`, `:57`.

- `story:standing-tree-rests-quiescent` — its acceptance reads stop semantics that `story:concurrent-cases-in-the-tree-daemon` rewrites, and it has no edge to it. Story 3 defines rest as "no case is runnable" and "the daemon's slot release". Story 2 turns `TreeStop::Waiting` and `Stuck` into lists over cases and replaces the per-tree scheduler. Story 3's "open obligation and nothing useful exits 4 `Stuck`, not 7" is written against the old per-tree `Stuck`. Story 4 inherits the gap: it cites story 2 ("as … say") but depends only on 3. Add 3→2, which gives 4 the edge too. (cited) — `story/standing-tree-rests-quiescent.md:22`, `:85`, `:102`, `story/concurrent-cases-in-the-tree-daemon.md:36-38`, `:74`, `story/standing-tree-wakes-at-a-host-instant.md:28`.

- `story:capacity-given-to-an-admission-program` — `used` is "the tree's per-run wall seconds … as its budget counts them", and its acceptance says "equals the sum the tree's budget counts at that step". `story:standing-tree-wakes-at-a-host-instant` redefines that counter to start at the latest `woken` line. Neither body mentions the other, and no edge joins them. Both edit the budget counters in `crates/loom-tree`. Add an edge 6→4, or state that `used` counts from `tree open`. (cited) — `story/capacity-given-to-an-admission-program.md:29`, `:75`, `story/standing-tree-wakes-at-a-host-instant.md:34-37`, `:102`.

- `epic:concurrent-and-standing-case-trees` — the "Shared files, with no edge for them" list is incomplete, so some collisions are unnamed. It says `bindings.yaml` is touched by 1, 5, 6, but story 2 also adds `OutsideWorkspaceRoot` there. It omits:
  - `crates/loom-bindings/src/decode.rs` and `load.rs`: stories 5 and 6 (and 1 for `load.rs`).
  - The program port: stories 5 and 6.
  - `ess/scenarios/`: 2, 3, 4, 7.
  - The generated `generated/rust/loom/src/{tree,runlog,bindings}.rs`: stories 1 to 7.
  - `website/docs/reference/cli.md`: 3, 4, 7.
  - `ess/domains/engine.yaml`: story 2.

  Stories 5 and 6 share three files and a module, and no edge or body connects them. Landing "1 to 7, one at a time" lives only as prose and no edge records it, so `aep plan artifact waves` will not honour it. Extend the list, and record each shared file as the reason for an edge or split the surface. (cited; `ess/domains/{tree,runlog,engine}.yaml` do not exist at `39f4da88`, so the stories collide on files that are still to be created) — `epic/concurrent-and-standing-case-trees.md:117-124`, `story/run-caps-per-arrival.md:93-96`, `story/capacity-given-to-an-admission-program.md:287-291`, `story/concurrent-cases-in-the-tree-daemon.md:74-75`.

- `story:concurrent-cases-in-the-tree-daemon` — it redefines `TreeStop::Waiting` and `Stuck` as case lists. `story:tree-waits-open-decision-requests` (another epic) changes `Stuck` to carry the ending request and adds open requests per case to `TreeStatus`. Both edit the same variants and the same per-case entry. The epic names only the file `tree.yaml` and says "the same rule applies". The story does not name the shared variants. Name them and record an edge or a split. This extends the drafter's open point 11. (cited) — `story/concurrent-cases-in-the-tree-daemon.md:36-38`, `:74`, `story/tree-waits-open-decision-requests.md:117`.

- `story:standing-tree-rests-quiescent` — it creates "its fixtures (new)" at no stated path, while story 4 reuses its `area/1` fixture. Story 7 declares `crates/loom-cli/tests/fixtures/area/` "(new)", so two stories would create that directory (not there yet) without naming each other. State the fixture path in story 3, and have story 7 extend it or use another directory. (cited) — `story/standing-tree-rests-quiescent.md:77`, `:103`, `story/flow-limits-recorded-as-events.md:71`, `:99`.

**What you read:** 8 artifacts (the epic and 7 stories) with `aep plan artifact show` and `aep plan artifact graph --format json`. I ran `aep plan artifact waves` (all seven stories are `unassessed`, because none has a scope). I read the bodies of `tree-daemon`, `tree-waits-open-decision-requests` and `hash-chained-run-log`, and grepped other bodies (`child-case-runtime`, `tree-run-command`, `action-limits-enforced`, `limit-hits-in-run-log`, `budget-and-limit-stops-raise-decision-requests`). I also read `.engineering/drafts/e4-plan.md`, the code paths the bodies cite and the `ls` of `crates/` and `ess/domains/`.

**Surfaces:** 7 cited, 0 inferred, 0 unplaced. Some surfaces inside cited items are modules the bodies themselves mark "named by X, not read": the `open` port (story 1), the scheduler module (2) and the program and agent ports (5, 6). I counted those as cited only at the crate level.
**Acceptance reads traced to a producer in the set:** 8 (2←1, 4←3, 6←2, 7←6, 7←3, 7←4, 3←2, 6←4). A `depends_on` edge records 4 of them. The other 4 are the findings above.

**What I could not establish:**
- Whether `story:tree-waits-open-decision-requests` lands before or after story 2. The epic says its edges are untraced, and I did not trace them.
- Which story creates `engine-limits.yaml`, which story 6's acceptance reads. It appears only in story 6, so it is probably story 6's own fixture.
- Line numbers cite wrongly in several bodies (out of my lane; this belongs to the acceptance critic or the drafter).
  - `child-case-runtime.md:73` and `:158` cite the sentences that sit at `:82` and `:171`.
  - `tree-daemon.md:144` cites the sentence at `:165`.
  - `tree-run-command.md:80` and `:143` cite sentences at `:85` and `:148`.
  - `tree-daemon.md:37-40` and `:48-50` were not re-checked.
  - The quoted text is right in each case.
- Also out of my lane: whether `story:flow-limits-recorded-as-events` is one story or two (the `loom.capacity` `flow` member, and `SetLimits` with its log event). That is the design critic's question.

```findings
- file: .engineering/planning/story/flow-limits-recorded-as-events.md
  line: 87
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its acceptance (epic acceptance file, guide page with a standing tree and a wake) reads what story:standing-tree-rests-quiescent and story:standing-tree-wakes-at-a-host-instant produce, and no depends_on edge reaches either (7 depends on 6, 6 on 2, 2 on 1 and tree-daemon only); add edges 7 to 3 and 7 to 4 or merge"
- file: .engineering/planning/story/run-caps-per-arrival.md
  line: 60
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it edits crates/loom-tree and writes the arrived line before evaluated, but no edge reaches story:tree-run-command, which creates that crate and the run; the 'Through them' claim is false because tree-run-command depends on bound-artifact-revisions and not the reverse; add depends_on story:tree-run-command or drop the crate from the story"
- file: .engineering/planning/story/standing-tree-rests-quiescent.md
  line: 85
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its Stuck-not-7 acceptance and its slot release read the per-case Waiting/Stuck/scheduler that story:concurrent-cases-in-the-tree-daemon rewrites (TreeStop lists, per-case slots) with no edge from 3 to 2; story 4 cites story 2 in its outcome and depends only on 3; add 3 depends_on 2 or state which side rebases"
- file: .engineering/planning/story/capacity-given-to-an-admission-program.md
  line: 29
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "used reads the tree's budget counters as they count, and story:standing-tree-wakes-at-a-host-instant redefines their starting line to the latest woken line in the same crates/loom-tree code; neither body mentions the other and no edge joins them; add 6 depends_on 4 or say used counts from tree open"
- file: .engineering/planning/epic/concurrent-and-standing-case-trees.md
  line: 117
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the shared-files list omits story 2 on bindings.yaml and omits crates/loom-bindings/src/decode.rs and load.rs and the program port (stories 5, 6, and 1 on load.rs), ess/scenarios (2, 3, 4, 7), the generated rust files (all seven), website/docs/reference/cli.md (3, 4, 7) and ess/domains/engine.yaml (2); ess/domains/tree.yaml, runlog.yaml and engine.yaml do not exist yet; the landing order 1 to 7 is prose and no edge records it, so waves will not honour it"
- file: .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md
  line: 36
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it turns TreeStop Waiting and Stuck into lists of cases while story:tree-waits-open-decision-requests (another epic) adds the ending request to Stuck and open requests per case to TreeStatus, the same variants and entry; the epic names only the file tree.yaml and no edge orders the two; name the variants and record an edge or a split"
- file: .engineering/planning/story/standing-tree-rests-quiescent.md
  line: 103
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its fixtures are 'new' at no stated path while story 4 reuses its area/1 fixture and story 7 declares crates/loom-cli/tests/fixtures/area/ new, so two stories would create that directory (not yet present) without naming each other; state the fixture path and who extends it"
```
