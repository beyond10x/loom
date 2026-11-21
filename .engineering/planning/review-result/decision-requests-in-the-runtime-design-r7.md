---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-design-r7
kind: review-result
status: active
title: Plan critic design, round 7, on stories 21, 23 and 16 after revision e3r11 and first review of story:goals-entries-from-a-resolved-document
relations:
- reviews: epic:decision-requests-in-the-runtime
- reviews: story:decisions-resume-the-held-run
- reviews: story:goals-grounds-and-free-answers
- reviews: story:tree-waits-open-decision-requests
- reviews: story:goals-entries-from-a-resolved-document
- reviews: story:decision-suspension-reason
revision: 1
---
needs-revision

All four round-6 design findings are fixed. Item 3 (the edge to story 20) is file sharing only, and I found six other things to change.

story:goals-grounds-and-free-answers — The `depends_on` edge to `story:non-independent-review-becomes-decision-request` records shared files only: the body says "Neither reads the other's members, so the order is arbitrary". The edge puts two drafts into the ancestry of stories 23 and 24: story 20 and `story:selector-stop-names-set-aside-record` (`epic:protocol-tree-on-bindings`). This is the same kind of edge the coordinator removed from story 24. There are two options. One is to keep the order and accept that stories 23 and 24 wait on two unscoped drafts. The other is to drop the edge and give story 20 a typed scope (`ess/domains/decision.yaml`, the decider-port module, the `ModelDecider` module, `crates/loom-tree`) so `aep plan artifact waves` keeps the two apart. Reversing the edge (story 20 after story 23) also works. Cites: .engineering/planning/story/goals-grounds-and-free-answers.md:292-296; .engineering/planning/story/goals-entries-from-a-resolved-document.md:140-142; aep plan artifact graph --format json.

epic:decision-requests-in-the-runtime — The Order paragraph says story 24 depends on story 23 "and on every story that edits `ess/domains/engine.yaml` and is not already before it". The graph gives story 24 three edges, because the seven engine-section edges were removed. Cites: .engineering/planning/epic/decision-requests-in-the-runtime.md:170-172, :195-196; .engineering/planning/story/goals-entries-from-a-resolved-document.md:13-15.

story:goals-entries-from-a-resolved-document — § Open 7 is a security gap, and a source contradicts the default. SDC makes the goals revision the sha256 of the resolved file (`~/beyond10x/sdc` at `00e3557`, docs/requester.md:20). D93 checks the entry against "the current layers" (docs/decisions.md:142). Loom compares the file with nothing. So a stale or foreign document makes Loom verify grounds that are no longer true:
- A decider cites `goals@<old rev>#id` with the old digest. The entry is gone or changed at the case's current revision. Loom verifies the ground, with only the note `revision_moved`.
- Anyone who can write the configured path can make any cited digest verify, because Loom computes none.
- A verified strong recommendation with a default on silence is then taken by the tick with no person. On an approval, that is a grant.

Fix: hash the file and refuse `invalid_goals` when the hash differs from the held case's `goals` revision, with a bullet. Or state this accepted risk in § Outcome. Cites: .engineering/planning/story/goals-entries-from-a-resolved-document.md:35-36, :85-87.

story:goals-entries-from-a-resolved-document — The port judges window and override from a document already filtered at its own `at`, not at the request's `opened_at`:
- A present entry never gets `overridden_by`. If an override applies at `opened_at` but not at the document's `at`, the target stays in the document and is verified, although Loom reads `overrides`.
- An entry in its window at `opened_at` but dropped at `at` reads `no_entry`.

§ Open 5's default appears only as mechanics. The body does not state its consequence, and no bullet holds it: `company/c9` is absent from every layer. Cites: .engineering/planning/story/goals-entries-from-a-resolved-document.md:32-33, :40, :77-82, :202.

story:goals-entries-from-a-resolved-document — The tree bullet "root case's `goals` artifact at a later revision … holds `revision_moved`" reads a `goals` artifact. The three-level fixture tree of `story:tree-run-command` does not declare one, and no story adds it. Story 23 gives no note when the held case declares no `goals` artifact. Cites: .engineering/planning/story/goals-entries-from-a-resolved-document.md:216-223; .engineering/planning/story/goals-grounds-and-free-answers.md:110-111; .engineering/planning/story/tree-run-command.md:129.

story:decision-suspension-reason — Its § Out of scope still says story 21 numbers `HeldForAuthority` 31 and `OtherDecision` 32. After e3r11, 31 is `NotRebuildable` and 32 is unused. Cites: .engineering/planning/story/decision-suspension-reason.md:284-286; .engineering/planning/story/decisions-resume-the-held-run.md:169-173.

Round-6 findings, checked against the stored bodies:
1. Story 21 rebuild: fixed by narrowing. The command line resumes approval holds of `software-change@1` runs only and refuses any other hold `NotRebuildable` (31). `ResumeBuilder.decision_executor` supplies the executor on the library path (decisions-resume-the-held-run.md:116-145, :287-300).
2. `Stopped`: fixed. `DecidedResume` gains `Stopped`, `resume_decided_request` reads `RunStopped` itself, `--commission` on a stopped run returns `Stopped`, and a library `stop_held_run` exists (:261-277, :323).
3. `GoalsEntries` reaching the check: fixed. `with_goals_entries` adds the port, the instant is the request's `opened_at` passed through `resolve`, the slice takes it in `RunOptions.goals` and the tree in its shared build, and the note is kept in `grounding_report` (goals-grounds-and-free-answers.md:116-161).
4. No order against stories 16 and 22: fixed. Story 23 has an edge to story 22, and Files now name `crates/loom-tree` and `tree.yaml` (:241-253, :288-291).

Story 24, checks with no defect found:
- Each load-time refusal has one bullet: Missing, Unreadable (twice), FormatUnknown, NotResolved, TooLarge with the boundary size, and `MemberUndeclared`.
- The file is read on every check, and a bullet shows a changed file giving the new digest.
- A bullet with a recording `sdc-goals` on `PATH` and `SDC_GOALS` shows no program is run. No network is used.
- `tree run` and `tree serve` pass the provider, and a bullet covers no configuration.
- Its three edges order every read it makes: the loader, story 23's port pieces, `tree serve --engine`, and the fixture tree and tree routing through story 23 → 22 → 16. The edge to `story:tree-daemon` is redundant but harmless.

Exit statuses: no clash. 31 is used only for `NotRebuildable`, and story 21 says 32 is unused. Story 16's "30 to 36" and story 21's safety fact still hold. The only stale mention is story 10's, above.

What I read: stories 21, 23 and 24 in full, story 16's exit-37 lines, the ledger e3r11, the epic's Order, story 20, the engine configuration story, tree-run-command's fixture, story 10:282-288, and SDC's requester.md and decisions.md at `00e3557`. Commands: `aep plan artifact show`, `aep plan artifact graph --format json` (372 `depends_on` edges walked across the whole store, not just the set; no cycle; the edge to story 20 adds 2 ancestors). I traced 12 acceptance reads to a producer, and edges order 11 of them. The `goals` artifact on the fixture tree has no producer.

What I could not establish:
- Whether story 23's change to `DeciderReply::Propose` forces an edit in story 20's code. If it does, the story 20 edge is an edge of order after all.
- Story 24 has one `GoalsDocumentNotResolved` bullet for four members. Acceptance lane.
- § Open 8 (a relative path) and § Open 9 (slice runs through the binary) are defaults no source contradicts.

```findings
- file: .engineering/planning/story/goals-grounds-and-free-answers.md
  line: 292
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the depends_on edge to story:non-independent-review-becomes-decision-request records shared files only (neither reads the other's members), yet it holds stories 23 and 24 behind two unscoped drafts; either keep that order knowingly or drop the edge and give story 20 a typed scope naming decision.yaml, the decider-port module, the ModelDecider module and crates/loom-tree"
- file: .engineering/planning/epic/decision-requests-in-the-runtime.md
  line: 195
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Order paragraph says story 24 depends on every story that edits ess/domains/engine.yaml, but the graph gives it only three depends_on edges after the coordinator removed seven"
- file: .engineering/planning/story/goals-entries-from-a-resolved-document.md
  line: 85
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the document is never compared with the held case's goals revision, which SDC defines as its sha256, so a stale or foreign document verifies a ground whose entry is gone or changed at the current revision, and with a default on silence the tick then takes it without a person; refuse invalid_goals on a mismatch with a bullet, or state the accepted risk"
- file: .engineering/planning/story/goals-entries-from-a-resolved-document.md
  line: 77
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the port reads window and override from a document filtered at its own at: a present entry never gets overridden_by even when an override applies at opened_at, and an entry valid at opened_at but dropped at at reads no_entry; the body does not state this consequence of the Open 5 default and no bullet holds it"
- file: .engineering/planning/story/goals-entries-from-a-resolved-document.md
  line: 222
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the tree revision_moved bullet reads a goals artifact that the three-level fixture tree of story:tree-run-command does not declare and no story adds, and story 23 gives no note without one"
- file: .engineering/planning/story/decision-suspension-reason.md
  line: 286
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "Out of scope still says story 21 numbers HeldForAuthority 31 and OtherDecision 32, but after e3r11 31 is NotRebuildable and 32 is unused"
```
