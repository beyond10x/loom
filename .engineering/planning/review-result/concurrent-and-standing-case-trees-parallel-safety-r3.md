---
format: aep.planning-md/3
id: review-result:concurrent-and-standing-case-trees-parallel-safety-r3
kind: review-result
status: active
title: Plan critic parallel-safety, round 3, on epic:concurrent-and-standing-case-trees
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

Round-2 fixes of my charter all hold in the stored bodies. Three new findings follow, all warnings.

- `story:concurrent-cases-in-the-tree-daemon` — Slot keeping leaves two things unsaid. It does not say whether the workspace claim of a case is held across its consecutive steps as the slot is. Two slot-holders on one workspace can both be runnable, and the body does not say which steps next or how each counts. It also does not say whether a case blocked only by a workspace, with a slot free, is `Queued`. The body's "the second queues even with a slot free" implies it is. `story:capacity-given-to-an-admission-program` defines `queued` as "runnable cases ... waiting for [a slot]" and `free` as `run_slots` minus `running`. So `queued > 0` with `free > 0` is reachable, and an admission rule that counts free slots against queued cases reads it wrongly. State the claim rule and which state counts as `running` and `queued`, then align story 7's `queued` wording. — `.engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:52-54`, `:61-64`, `:70`; `.engineering/planning/story/capacity-given-to-an-admission-program.md:64-66`
- `story:run-caps-per-arrival` — Inferred. `story:bound-program-result-table` keeps its bullet "An action that has already run its `budget.max_runs` times in this case ... is reported `RunsSpent`". The bullet names no effect class, and the story is ahead of this one in the chain. Its Out of scope now says this story lands the observation rule. If that story's own `RunsSpent` test uses a non-`write` action, which the word "observation" in its Outcome suggests, this story turns a landed green test red. This story's `## Files` names neither that test (`crates/loom-bindings/tests/bound_program.rs`, created by the ancestor) nor the effect class it uses. Either say the ancestor's test drives a writer, or name the test as edited here. — `.engineering/planning/story/bound-program-result-table.md:91-92`, `.engineering/planning/story/run-caps-per-arrival.md:213-233`
- `epic:concurrent-and-standing-case-trees` — § Order's list of outside sharers stops at `tree.yaml`, `runlog.yaml`, `crates/loom-tree` and governor `lib.rs`. That is the same class as round-2 finding 3, left on the other surfaces the eight claim. The epic's own file list at :151-158 names these surfaces.
  - `ess/domains/bindings.yaml` and `load.rs` (stories 2, 5, 6, 7) are shared with `story:bindings-child-path-refusal-names-the-bindings-directory` (edits `load.rs:203`, `:286-294`, `:27-42`), `story:agent-session-through-substrate` (extends `AgentRun`, `:69`, `:75-76`) and `story:autonomy-grant-table`.
  - `ess/domains/governor.yaml` and `file_store.rs` (story 5, `CaseLink`) are shared with `story:authority-records-bound-and-grounded-in-canon`, `story:decided-records-carry-grounds-to-canon` and `story:requests-from-canon-pending-decisions`. All three edit `governor.yaml`.
  - `ess/domains/engine.yaml` (story 6) is shared with `story:agent-session-through-substrate`, `story:confinement-level-on-oci-and-pod`, `story:ingress-routes-per-action`, `story:executable-descendants-held` and `story:substrate-daemon-on-another-host`.
  - None of these has a path to or from the eight. Say the second-to-land-rebases rule covers them, or record an edge or a split per pair.
  - The paragraph also says `story:run-log-references-exec-file-changes` shares with "all eight" (`:185-187`). It depends on `story:budget-and-limit-stops-raise-decision-requests`, which depends on story 3. So it lands after 1 to 3 and is unordered only with 4 to 8.
  - `story:budget-and-limit-stops-raise-decision-requests` also meets story 7 on `Capacity.budget`, "the tree's `TreeBudget` in force". An extension that story creates is "in force" or not, and neither body says which. Name this beside story 4's carry-over gap.
  — `.engineering/planning/epic/concurrent-and-standing-case-trees.md:151-158`, `:174-198`; the outside bodies cited above

**Round-2 fixes that hold**
- `story:bound-program-result-table` has the observation pointer only in Out of scope, and the acceptance bullet is gone. `story:run-caps-per-arrival` says it lands the rule (Outcome, :1-6 of the Outcome text). The blocker is closed.
- The edge `budget-and-limit-stops-raise-decision-requests depends_on tree-usage-and-model-budget` is stored. The sibling's cost bullet uses `agents/1` and the stub adapter of story 3. Story 3's Outcome says its own stop has no request. The graph has no cycle (checked over all stored artifacts).
- All seven chain edges are stored (2→1 through 8→7), plus 8→1, 8→2, 8→4, 6→`tree-waits-open-decision-requests`, 7→4.
- Fixture names do not collide across the eight (checked every Files list). `bin/gate` has one creator (story 4), `engine-daemon-1.yaml` one (story 1), and the wall and CPU engine files are separate, so the CPU check cannot stop on the wall limit.

**What slot keeping does**
- Fairness: a queued case now waits as long as any slot-holder stays runnable. That is bounded only by the run budgets and `max_runs`, not by the queue. A one-case tree behaves as under `story:tree-daemon`'s per-tree slot, so story 1's one-slot daemon bullet and story 4's gated-wake bullets are unchanged. The sequences in these bullets are deterministic under keeping:
  - epic bullet 1;
  - story 6, bullets 1 to 3;
  - story 7, bullet 2 (`running 2, queued 1, free 0`);
  - story 7's standing bullet (`used.steps` 1, `since_open.steps` 3 then 5; I recounted).
- Story 4's "queues ... as `story:tree-daemon` says" (:186) is superseded by story 6, which says so.
- Story 6's "wake acts on `Resting` cases" does not break story 4's `tree_wake.rs`. Its `Running` and `Stuck` bullets have no `Resting` case.

**What I read:** 11 artifacts (the epic, the 8 stories, and the 2 outside stories for their one-sentence changes). I used `aep plan artifact graph --format json` (cycle check, ancestors and descendants of each story), `git diff HEAD` on the two outside bodies, and a scan of every open story for the eight's files. I also read `e4r2-changes.md`, the round-2 verdict, and the story-scoper `## Scope` blocks now in story 2. I opened these citations and they hold:
- `ess/domains/bindings.yaml:22-30`, `:113-125`
- `crates/loom-bindings/src/decode.rs:664-695` and `lib.rs:30`
- `crates/loom-bindings/src/load.rs:284-294`
- `crates/loom-executor/src/selection.rs:283-306`, `:317-320`
- `tree-run-command.md:61`, `:79`, and `tree-daemon.md:20-60`

**Surfaces:** 9 cited, 0 inferred, 0 unplaced. Modules the bodies mark "not read" are the program, agent and `open` ports and the scheduler. `crates/loom-tree`, `tree.yaml`, `runlog.yaml` and `engine.yaml` do not exist at `61fd500b`.
**Acceptance reads traced to a producer in the set:** 19. A direct `depends_on` records 14, and the other 5 run through chain edges. Three more reads cross the set boundary and have edges: story 2 reads `bound-program-result-table`, story 3 reads `agent-node-run-records`, and story 6 reads `tree-waits-open-decision-requests`.

**What I could not establish**
- Whether `story:tree-daemon` already keeps the last `TreeBudget` beside the root's case file. Story 4 says "not read" and adds it if absent, so its surface is `crates/loom-tree` either way.
- Whether the ancestor's `RunsSpent` test uses a writer (finding 2).
- Out of lane (acceptance and design): story 8's bullets (`flow-limits-recorded-as-events.md:56`, `:115`) expect an admission step to see "the asking case's open children". Story 6 (:57-58) and `tree-run-command.md:61` say a parent steps only with no open child, so the root's admission cannot run while its children are open.
- Out of lane (scope): `website/docs/guides/bindings.md:164` and `docs/contracts/loom-action-selection.md:168` state `max_runs` rules that story 2 changes, and neither is in its Files. The scoper already notes this.
- Open item 1 of `e4r2-changes.md` (a `budget extended` line over a wake) is an assumed default and no source contradicts it. I did not raise it. Finding 3 only asks that the epic name the pair.

```findings
- file: .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md
  line: 61
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "slot keeping across consecutive steps leaves open whether a case's workspace claim is held across its steps like the slot, which of two slot-holders sharing a workspace steps next, and whether a case blocked only by a workspace with a slot free is Queued; story:capacity-given-to-an-admission-program (:64-66) defines queued as runnable cases waiting for a slot and free as run_slots minus running, so queued greater than 0 with free greater than 0 is reachable and unexplained; state the claim rule and the Running or Queued state, and align story 7's queued wording"
- file: .engineering/planning/story/run-caps-per-arrival.md
  line: 213
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "inferred from the ancestor's wording: story:bound-program-result-table keeps a RunsSpent acceptance bullet for any action (:91-92) while this story makes an observation never refused RunsSpent, so if the ancestor's test (crates/loom-bindings/tests/bound_program.rs, created by it) drives a non-write action, this story turns it red; this story's Files do not name that test as edited or say the ancestor's test drives a writer"
- file: .engineering/planning/epic/concurrent-and-standing-case-trees.md
  line: 174
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Order list of outside sharers covers tree.yaml, runlog.yaml, crates/loom-tree and governor lib.rs only; it omits ess/domains/bindings.yaml and load.rs (stories 2, 5, 6, 7) shared with story:bindings-child-path-refusal-names-the-bindings-directory, story:agent-session-through-substrate and story:autonomy-grant-table, ess/domains/governor.yaml (story 5) shared with story:authority-records-bound-and-grounded-in-canon, story:decided-records-carry-grounds-to-canon and story:requests-from-canon-pending-decisions, and ess/domains/engine.yaml (story 6) shared with story:agent-session-through-substrate, story:confinement-level-on-oci-and-pod, story:ingress-routes-per-action, story:executable-descendants-held and story:substrate-daemon-on-another-host, none with a path to the eight; name them under the second-to-land-rebases rule or record an edge or split per pair; it also says story:run-log-references-exec-file-changes shares with all eight (:185-187) although it depends on budget-and-limit-stops, which depends on story 3, so it is unordered only with 4 to 8; and story:budget-and-limit-stops-raise-decision-requests meets story 7's Capacity.budget (TreeBudget in force) with no edge and no word on whether an extension is in force"
```
