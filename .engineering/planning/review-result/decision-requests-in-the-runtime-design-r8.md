---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-design-r8
kind: review-result
status: active
title: Plan critic design, round 8, confirming pass on stories 15, 16, 20, 21, 23 and 24 of epic:decision-requests-in-the-runtime
relations:
- reviews: epic:decision-requests-in-the-runtime
- reviews: story:approvals-supplied-to-canon-evaluation
- reviews: story:tree-waits-open-decision-requests
- reviews: story:non-independent-review-becomes-decision-request
- reviews: story:decisions-resume-the-held-run
- reviews: story:goals-grounds-and-free-answers
- reviews: story:goals-entries-from-a-resolved-document
revision: 1
---
needs-revision

All six of my round-7 findings are fixed. No cycle anywhere. Five new things, all warnings.

story:decisions-resume-the-held-run — Its list of tests it changes leaves out two acceptance bullets of `story:approvals-supplied-to-canon-evaluation` that its new resume-on-`decide`/`take` breaks:
- "after `decisions decide … A`, … `resume` performs it once": after this story, `decide` performs the merge and a later `resume` exits 10.
- "taken on a strong recommendation … after those lines are rewritten … `resume` is refused": after this story, `take` resumes before the lines are rewritten.

Story 15 is ordered before this story and names no file for those bullets. Story 21 says `approval_decisions.rs` passes "with only its three tests named in the Outcome edited". Fix: story 15 names the test file, and story 21 lists these two tests among those it edits (decide or take through `loom_sdk::decisions` before the resume step). Cites: .engineering/planning/story/approvals-supplied-to-canon-evaluation.md:118-125; .engineering/planning/story/decisions-resume-the-held-run.md:355-362, :645.

story:non-independent-review-becomes-decision-request — The reversed edge orders this story after `story:daemon-continues-a-decided-request` (20 → 23 → 22). The daemon's `Decide` applies only story 16's writers, `CaseApprovalWriter` and `ExplicitDecisionWriter`. This story adds `ReviewEvidenceWriter` only to the direct `decisions … --tree` writer set. So a review decided on a served tree has no `canon_evidence` writer and stays Decided without a record. The body was written for the world before story 22. Fix: add `ReviewEvidenceWriter` to the daemon's writer set, with a served-tree bullet. Cites: .engineering/planning/story/non-independent-review-becomes-decision-request.md:54-56, :82; .engineering/planning/story/daemon-continues-a-decided-request.md:61-63.

story:goals-entries-from-a-resolved-document — The body never mentions `story:bound-artifact-revisions`, which is ordered before it (via tree-run-command). That story recomputes a bound artifact's revision as `t` plus 16 hex digits of the file at its path, before every evaluation. If the bindings place `goals` at the path `goals.document` names, every edit of the file becomes the root case's revision, and the revision check matches by construction. In that setup it does not stop "a file edited without the case", which "What remains" claims it does. This is also the binary's only way to move the revision; the tests use `update_revision`. Fix: state this interaction, and either refuse a `goals.document` that is the bound path of `goals` or narrow the "What remains" claim. Cites: .engineering/planning/story/goals-entries-from-a-resolved-document.md:91-96, :125-133; .engineering/planning/story/bound-artifact-revisions.md:34-42.

story:goals-grounds-and-free-answers — `GoalsEntries` is a port a host implements (the slice takes it through `RunOptions.goals`). But the story names no `host_ports!` row, no row under "Ports a host implements" on `sdk-stability.md`, and no `loom_sdk` path. AGENTS.md:172 requires the page to list exactly the ports a host implements, and story 21 added both rows for `ResumeBuilder`. Cites: .engineering/planning/story/goals-grounds-and-free-answers.md:125-149, :239-261; AGENTS.md:172; .engineering/planning/story/decisions-resume-the-held-run.md:311-315.

story:goals-entries-from-a-resolved-document — Same gap for `RootGoalsRevision`, which an embedding host implements and passes with `FileGoalsEntries` through `RunOptions.goals`: no `host_ports!` row, no row on the stability page, no `loom_sdk` path. Cites: .engineering/planning/story/goals-entries-from-a-resolved-document.md:75-78, :171-174, :253; AGENTS.md:172.

Round-7 findings, checked against the stored bodies:
1. Edge between stories 23 and 20: reversed. It is now an edge of order: story 23 changes `DeciderReply::Propose` and the `ModelDecider` schema, story 20 builds on both, and the second to land edits the other's code. That story 20's tests build `Propose` values is inferred (§ Open 8). Neither body assumes story 20 lands first (goals-grounds-and-free-answers.md:298; non-independent-review-becomes-decision-request.md:122-131).
2. Epic Order: now matches the graph. Story 23 has ten edges, story 24 three, and story 20 depends on 23 (epic:188-208).
3. Story 24 compared the document with nothing: fixed. It checks `t` plus the first 16 hex digits of the file's sha256 against the root case's `goals` revision. That is SDC's form (`revision_of`, sdc `00e3557` driver/src/main.rs:191-199). A mismatch or a missing artifact is `invalid_goals`, with bullets.
4. Window and override read from a filtered document: fixed. § "What a resolved document cannot tell" states the consequences, with the `company/c4` and `product/p5` bullets.
5. Fixture without `goals`: fixed with its own `goals-tree` fixture (:177-182).
6. Story 10's stale 31/32: fixed (decision-suspension-reason.md:7-8).

Stories 15, 16 and 20, read in full:
- Story 16: the `StoreServed` refusal is still stated, and its owner is named: `story:tree-daemon` brings the refusal, and `story:daemon-continues-a-decided-request` carries the bullet (story 16 § Outcome, the lock paragraph). Its states and exits match the trees epic's per-case states ("ended by a decision" is `Stuck`, concurrent-cases:102-105). Every read is ordered.
- Stories 15 and 20: every read is ordered by an edge, apart from the two defects above.

Edges over the whole epic: 372 `depends_on` edges walked across the whole store, not just the epic, with a cycle search. There is no cycle and no edge to a missing artifact. The epic's depth is 17. The drafts form one chain, 15 → 21 → 16 → 22 → 23 → {24, 20}. That is a serialising chain, but each edge has a written reason (shared `decisions.rs`, `lib.rs` and `main.rs`, or a type change that forces edits). So it is the trade-off between keeping the order and splitting those files, not a defect.

What I read: the body changes of stories 24, 21 and 23; stories 15, 16 and 20 in full; ledger e3r12; the epic's Order; story 22's daemon paragraph; `story:bound-artifact-revisions`; the tree-run-command fixture; AGENTS.md:172; sdc `driver/src/main.rs` at `00e3557`. Commands: `aep plan artifact show`, `aep plan artifact graph --format json`. I traced 30 acceptance reads to a producer, and edges order all 30. The two cross-story defects above run the other way: a later story changes what an earlier one reads or writes.

What I could not establish:
- Story 15 cites `canon-authority/1` at Canon `0.8.0`, while `story:canon-0-9-0-pin` moves the pin. Acceptance or scope lane.
- Story 23's Scope section (:489-493) still says "no path" to story 20. That is the scoper's text, out of my lane.
- A root `goals` artifact that SDC writes as `none` gives the mismatch reason, not "declares no goals artifact". Acceptance lane.

```findings
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 355
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the list of tests this story changes leaves out story:approvals-supplied-to-canon-evaluation's decide-then-resume and take-then-rewrite-then-resume bullets, which its resume on decide and take breaks; story 15 names no file for them, and this story claims only three tests change"
- file: .engineering/planning/story/non-independent-review-becomes-decision-request.md
  line: 54
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "now ordered after story:daemon-continues-a-decided-request, whose daemon Decide applies only CaseApprovalWriter and ExplicitDecisionWriter, this story adds ReviewEvidenceWriter only to the direct --tree writer set, so a review decided on a served tree stays Decided without a record"
- file: .engineering/planning/story/goals-entries-from-a-resolved-document.md
  line: 91
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the body never mentions story:bound-artifact-revisions, which recomputes a bound goals artifact's revision from its file before every evaluation, so when the bindings place goals at the goals.document path the revision check matches by construction and does not stop a file edited without the case, as What remains claims"
- file: .engineering/planning/story/goals-grounds-and-free-answers.md
  line: 125
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "GoalsEntries is a port a host implements, yet the story names no host_ports! row, no Ports a host implements row on sdk-stability.md and no loom_sdk path, which AGENTS.md:172 requires and story 21 gave ResumeBuilder"
- file: .engineering/planning/story/goals-entries-from-a-resolved-document.md
  line: 75
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "RootGoalsRevision is a callback an embedding host implements and passes with FileGoalsEntries through RunOptions.goals, yet the story names no host_ports! row, no sdk-stability.md row and no loom_sdk path"
```
