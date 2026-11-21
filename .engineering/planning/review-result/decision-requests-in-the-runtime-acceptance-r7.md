---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-acceptance-r7
kind: review-result
status: active
title: Plan critic acceptance, round 7, on stories 21, 23 and 16 after revision e3r11 and first review of story:goals-entries-from-a-resolved-document
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

story:decisions-resume-the-held-run — the setup does not say where the two library-built holds keep their Suspensions. The command-line bullets need them in DIR's `FileSuspensionStore`, but `run_with_clock` runs `Mode::Once` and keeps nothing (run.rs:525-526), and `run_with_suspension` keeps the Suspension in whatever store the test passes. As written, the resume step can find no hold for `DR-o` or `DR-q` — .engineering/planning/story/decisions-resume-the-held-run.md:465-473 (crates/loom-intake-slice/src/run.rs:525-526)

story:decisions-resume-the-held-run — § Outcome still says `decisions record … --state DIR` resumes once `RecordProduced` is applied. `record` only applies to question requests, and the command line refuses every question hold `NotRebuildable`, so `record` never resumes there. No bullet checks that `record` on a held question that was not stopped exits 31 — .engineering/planning/story/decisions-resume-the-held-run.md:61-63 (bullets :504-511, :524-528)

story:decision-suspension-reason — § Out of scope still says story 21 numbers `HeldForAuthority` 31 and `OtherDecision` 32. Since e3r11, 31 is `NotRebuildable` and 32 is unused — .engineering/planning/story/decision-suspension-reason.md:286 (decisions-resume-the-held-run.md:169-170)

story:tree-waits-open-decision-requests — the new exit-37 `StoreServed` bullet runs `b10x-loom tree serve STORE`, and the `StoreServed` refusal itself comes from `story:tree-daemon` (:99). No `depends_on` path leads from story 16 to `story:tree-daemon`, which is a draft, so if story 16 lands first the bullet cannot be run — .engineering/planning/story/tree-waits-open-decision-requests.md:222-224 (`aep plan artifact graph --format json`: no path to story:tree-daemon)

story:goals-grounds-and-free-answers — no bullet checks any of the places § "When a goals ground is checked again" lists. Nothing tests a `Take` withheld, a prior not reused, a record withheld, or a slice or tree superseding a request whose goals digest changed after `Decide` — .engineering/planning/story/goals-grounds-and-free-answers.md:163-181

story:goals-grounds-and-free-answers — no bullet checks that the slice's `RunOptions.goals`, or the port the tree takes from its host, reaches `with_goals_entries`. `held_case_goals.rs` builds `HeldCaseGrounds` directly — .engineering/planning/story/goals-grounds-and-free-answers.md:143-149 (bullets :343-354)

story:goals-grounds-and-free-answers — no bullet checks `check_grounds`' new reading of `supports`, namely that a ground without `supports` makes a free-answer recommendation verified. The `free_answer.rs` `Recommend` bullet hands the check verdict in as an input — .engineering/planning/story/goals-grounds-and-free-answers.md:186-188 (bullet :358-359)

story:goals-entries-from-a-resolved-document — `GoalsDocumentNotResolved` has four causes (`at` missing, `entries` missing, `layer` present, an entry without `id` or `digest`), and only the layer-file bullet, which names `at`, tests any of them — .engineering/planning/story/goals-entries-from-a-resolved-document.md:55-56 (bullet :180-181)

story:goals-entries-from-a-resolved-document — no bullet checks that a served tree sees a rewritten document without a restart. The only daemon bullet checks a single verified recommendation — .engineering/planning/story/goals-entries-from-a-resolved-document.md:70-71 (bullet :228-229)

story:goals-entries-from-a-resolved-document — the tree bullets need a case that declares artifact `goals`, but `story:tree-run-command`'s three-level fixture declares none. With none, story 23 gives no `revision_moved` note (:110-111), so the bullet at :222-223 cannot pass. The body also does not say which case holds `goals` or what `<goals revision>` is — .engineering/planning/story/goals-entries-from-a-resolved-document.md:216-223 (goals-grounds-and-free-answers.md:110-111)

story:goals-entries-from-a-resolved-document — § Open 5's default is only implied by :80-82. The body does not say that an entry `resolve` dropped, because it is outside its window at the document's `at`, answers `no_entry` rather than `expired` or `not_yet_valid`. The `company/c9` bullet (:202) holds the mechanism but not that consequence — .engineering/planning/story/goals-entries-from-a-resolved-document.md:80-82

story:goals-entries-from-a-resolved-document — § Open 7 is a security gap, and the body states it nowhere and no bullet pins it. Loom reads each `digest` as written (:43) and does not compare the file with the case's `goals` revision (:87). So whoever can write the named file can make Loom verify any cited goals ground by writing the cited id and digest. A stale document of an earlier revision verifies entries that have since been removed or overridden, with no `revision_moved` note. The only safeguard is the file's permissions — .engineering/planning/story/goals-entries-from-a-resolved-document.md:43, :85-87

Round-6 findings, checked against the stored bodies:
- **Story 21:** blockers 1 and 2 are fixed by limiting the scope. The command line now resumes approval holds only, and `NotRebuildable` (31) is tested at :504-507. The catalog files (:539-541), the error path (:573-585, exit 20 per decision-inbox-commands.md:70), `take` and `record` after `stop` (:521-528) and `stop`'s exits 11 and 20 (:532-536) are fixed. Finding 1 above is what is left of the setup problem.
- **Story 16:** the exit-37 bullets are added at :222-227, but the `StoreServed` one has no order (finding 4).
- **Story 23:** the unconditional note (:395-396), the `recommended_answer` printing (:384-386), the neither case (:390-391) and `GoalsEntryState` (:308-309) are all fixed.

Exit statuses: 31 is `NotRebuildable` and 32 is unused. Neither clashes with 36 or 37, and the only stale mention is the one in story 10 (finding 3).

Story 24, what holds:
- Every loader refusal except the `NotResolved` causes has a bullet. The 1 MiB boundary is tested on both sides, and `MemberUndeclared` has a bullet.
- The file is read on every call (:211-212), and a file that turns bad between checks is tested (:207-210).
- No program is run (:213-214). Opening no socket is stated but untestable as written; I accept that.
- `tree run` and `tree serve` pass the provider, `tree open` only loads it, and the no-configuration case is tested (:226-227).
- SDC at 00e3557 matches the body: `resolve` at check.rs:428-443 and `entry_json` at main.rs:181-191.
- Its three edges order every read it makes. Story 16, story 22, `story:tree-run-command`, `story:grounds-resolve-against-held-cases` and `story:engine-configuration-document` are all reachable, and there is no cycle for stories 23 or 24.

Edge question, out of my lane (design): story 23's edge to story 20 is file sharing only. Story 23 says so itself: "Neither reads the other's members, so the order is arbitrary" (goals-grounds-and-free-answers.md:292-296). Story 20 is a draft that depends on story 16, so the edge holds stories 23 and 24 behind it for no reason of order.

What I read: 4 ids in full (stories 21, 23, 16 and 24 via `grep -n`/`cat -n` of the stored bodies), plus story 10 :280-290. Also the r7 brief, the e3r11 ledger, story 12's acceptance, `engine-configuration-document.md`, `decision-inbox-commands.md:66-90`, `run.rs:395-560` and `:1229-1251`, `goals/src/main.rs` and `check.rs` at 00e3557, and a reachability script over `aep plan artifact graph --format json`.

What I could not establish:
- Whether story 12's "decision flags" make `run_with_clock` hold a Suspension in a `FileSuspensionStore`. Its acceptance (question-and-outcome-decisions-hold-runs.md:215-221) says a run "holds a `Decision` Suspension" but names no store.
- Whether story 1's "fourteen errors" count is right. I did not count them.

```findings
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 465
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the setup does not say the two library-built holds keep their Suspensions in DIR's FileSuspensionStore; run_with_clock runs Mode::Once and keeps nothing (run.rs:525-526) and run_with_suspension keeps it in whatever store the test passes, so as written the resume step finds no hold for DR-o or DR-q"
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 61
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome still says record --state resumes once RecordProduced is applied, but record applies only to question requests, whose holds the command line always refuses NotRebuildable, and no bullet checks that record on a held question that was not stopped exits 31"
- file: .engineering/planning/story/decision-suspension-reason.md
  line: 286
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "Out of scope still says story 21 numbers HeldForAuthority 31 and OtherDecision 32, while since e3r11 31 is NotRebuildable and 32 is unused"
- file: .engineering/planning/story/tree-waits-open-decision-requests.md
  line: 222
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the exit-37 StoreServed bullet runs b10x-loom tree serve and relies on the StoreServed refusal of story:tree-daemon, but no depends_on path leads from story 16 to story:tree-daemon, a draft, so the bullet cannot be run if story 16 lands first"
- file: .engineering/planning/story/goals-grounds-and-free-answers.md
  line: 163
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no bullet checks any place the section 'When a goals ground is checked again' lists: a Take withheld, a prior not reused, a record withheld, or a slice or tree superseding a request whose goals digest changed after Decide"
- file: .engineering/planning/story/goals-grounds-and-free-answers.md
  line: 143
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no bullet checks that the slice's RunOptions.goals or the port the tree takes from its host reaches with_goals_entries; held_case_goals.rs builds HeldCaseGrounds directly"
- file: .engineering/planning/story/goals-grounds-and-free-answers.md
  line: 186
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no bullet checks check_grounds' new reading that a ground without supports makes a free-answer recommendation verified; the free_answer.rs Recommend bullet hands the check verdict in as an input"
- file: .engineering/planning/story/goals-entries-from-a-resolved-document.md
  line: 55
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "GoalsDocumentNotResolved has four causes (at missing, entries missing, layer present, an entry without id or digest) and only the layer-file bullet, which names at, tests any of them"
- file: .engineering/planning/story/goals-entries-from-a-resolved-document.md
  line: 70
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no bullet checks that a served tree sees a rewritten goals document without a restart"
- file: .engineering/planning/story/goals-entries-from-a-resolved-document.md
  line: 216
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the tree bullets need a case that declares artifact goals, but the three-level fixture of story:tree-run-command declares none, so story 23 gives no revision_moved note and the bullet at :222-223 cannot pass; the body does not say which case holds goals or what the goals revision is"
- file: .engineering/planning/story/goals-entries-from-a-resolved-document.md
  line: 80
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the body does not state Open 5's default, that an entry resolve dropped because it is outside its window at the document's at answers no_entry rather than expired or not_yet_valid; the company/c9 bullet holds the mechanism but not that consequence"
- file: .engineering/planning/story/goals-entries-from-a-resolved-document.md
  line: 85
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the body does not state, and no bullet pins, that a document whose sha256 is not the held case's goals revision is accepted; since Loom reads each digest as written, whoever can write the file can verify any cited goals ground, and a stale document verifies entries since removed or overridden with no revision_moved note"
```
