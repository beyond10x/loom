---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-acceptance-r6
kind: review-result
status: active
title: Plan critic acceptance, round 6, on story:decisions-resume-the-held-run, story:daemon-continues-a-decided-request and story:goals-grounds-and-free-answers of epic:decision-requests-in-the-runtime
relations:
- reviews: epic:decision-requests-in-the-runtime
- reviews: story:decisions-resume-the-held-run
- reviews: story:daemon-continues-a-decided-request
- reviews: story:goals-grounds-and-free-answers
- reviews: story:tree-waits-open-decision-requests
revision: 1
---
needs-revision

story:decisions-resume-the-held-run — the setup cannot build the library-made state directory: `run_prepared_intent_with_state` reaches `run_prepared`, which refuses any protocol that is not clock-compatible and binds no commands (`entry.clock_compatible().map_err(SliceError::Context)?`). A question hold also needs an inner executor that returns `NeedsHumanJudgment`, and none of `drive`'s executors (Software, Clock, Commands) does, which also contradicts "a recorded agent the test asserts is never called". So neither `DR-o` nor `DR-q` is held as written — .engineering/planning/story/decisions-resume-the-held-run.md:377-380 (crates/loom-intake-slice/src/intent.rs:921-922; crates/loom-intake-slice/src/run.rs:626-633; question-and-outcome-decisions-hold-runs.md:186-189)

story:decisions-resume-the-held-run — the rebuild "as `b10x-loom resume` does" goes through `resumed_request` (main.rs:681). That function refuses a case whose inputs are not a software change's and binds no commands, so the run goes down the Clock path and is refused again. `resume_decided_request`'s arguments carry no executor, only `ResumeBuilder`. So the outcome and question command-line bullets (:411-426) and the first library bullet (:483-485, scripted inner executor) cannot pass. The ledger's § Open 4 does not hold — .engineering/planning/story/decisions-resume-the-held-run.md:106-111 (crates/loom-intake-slice/src/intent.rs:624-625, :628-629; main.rs:681)

story:decisions-resume-the-held-run — the tick setup does not say that the two runs name different `--catalog` files. If they share one file, removing "the model catalog file of `DR-a`'s commission" also fails `DR-b`, and the bullet at :450-451 cannot pass — .engineering/planning/story/decisions-resume-the-held-run.md:439-443

story:decisions-resume-the-held-run — the error-path bullet reads the same whether or not the first tick decided anything before the error, because the second tick decides and resumes both requests either way. It also names no exit status ("the tick's error status"), and the silence story fixes no processing order — .engineering/planning/story/decisions-resume-the-held-run.md:472-475 (silence-and-expiry-at-a-host-tick.md:54-56)

story:decisions-resume-the-held-run — no bullet checks that `take` or `record` of a request whose run was stopped prints `stopped: C by P (reason)` and resumes nothing. Only `decide`, `resume` and `tick --resume` are tested after `stop` — .engineering/planning/story/decisions-resume-the-held-run.md:269-270 (bullets :429-437)

story:decisions-resume-the-held-run — the `stop` exit statuses 11 (`InDoubt`) and 20 (store unreadable or unwritable) have no bullet. Only 0 and 10 are tested — .engineering/planning/story/decisions-resume-the-held-run.md:271-274

story:tree-waits-open-decision-requests — exit 37, added in e3r10 for `StoreServed` and `StoreInUse`, has no acceptance bullet. § Acceptance asserts neither refusal or its status — .engineering/planning/story/tree-waits-open-decision-requests.md:100-102 (§ Acceptance :168-225)

story:goals-grounds-and-free-answers — the `show` bullet "prints … the note `revision_moved` when the check gave it" is conditional. The body names no stored member that holds the note: `Ground` has no note at 00e3557 — .engineering/planning/story/goals-grounds-and-free-answers.md:224-225 (sdc 00e3557:ess/domains/decision.yaml:159-167)

story:goals-grounds-and-free-answers — no bullet checks that `decisions list` or `show` prints `recommended_answer` — .engineering/planning/story/goals-grounds-and-free-answers.md:87 (bullets :216-229)

story:goals-grounds-and-free-answers — no bullet checks the Outcome's rule that `decide` with neither `OPTION` nor `--answer` exits 10 naming `OptionOrAnswer`. Only "both" is tested, and the parser could refuse "neither" with exit 2 — .engineering/planning/story/goals-grounds-and-free-answers.md:90 (bullet :223)

story:goals-grounds-and-free-answers — § ESS first names the port `GoalsEntries` but not the entry state it answers (present, digest, validity window, overridden, document valid), which is a new noun with no ESS home — .engineering/planning/story/goals-grounds-and-free-answers.md:38-41 (§ ESS first :158-166)

Round-5 status:
- Fixed, on the stored bodies: the `DR-a` 30 bullet (now `CaseMoved`, :455-458), `E` as the later instant (:443), the `record` and question-`decide` bullets (:415-419), the `awaits_record` bullet (:468-471), story 22's restart bullet (:205-207), its tick output bullet (:231-232), and its per-case ESS first (:166-174).
- Blocker 1 is not fixed. Moving the setup from `run` to a library build hits the same `clock_compatible` gate (findings 1 and 2).
- Story 22 has no remaining acceptance finding.
- Exit statuses 36 and 37 clash with nothing. `lib.rs` uses 0-4 and 10-23, and a grep for 24 to 49 across the stories and the epic finds only stories 16, 21 and 22.
- `stop` releases the Suspension and drops the context (:258-260, bullet :431-433). Commission's `take` and `release` ports exist (crates/loom-commission/src/ports/suspension.rs:71-75).
- The daemon cannot resume a stopped run: it serves trees and holds no Commission Suspension.

Story 23:
- Every member it names exists at 00e3557 with that spelling: :116, :146-147, :166, :245, :255, :276, :335, :340, :466, :482-495, :549-573, :609, :678.
- Every refusal cause and `revision_moved` has a bullet.
- The contract keys the free answer on the question kind (`when_subject: unblocks.kind != question`, :493 and :571), not on the two classes in `e3-sdc-answers-6.md`.
- Approvals and outcomes stay option-only (:198, :202, :214, :221).
- Its seven edges match the epic's "2, 5, 6, 11, 12, 14 and 21", and it holds none of stories 1 to 14.

Spot-check of moved citations at 00e3557: all five hold.
- decider-port :496-515 and :465
- inbox :746-749 and :785-788
- approval :783-787
- question :708-718
- decided-record :78

What I read: 3 of 3 ids in full (`cat -n` of the stored bodies), the r4, r5 and r6 briefs, the round-5 acceptance verdict, the e3r10 ledger, and the diffs of the epic and story 16. Story 22 in full, story 12 §§ Outcome and Acceptance, and the silence story's tick lines. Source: `crates/loom-cli/src/lib.rs:1-221` and `main.rs:600-685`; `intent.rs:120-180`, `:236-290`, `:440-740` and `:820-950`; `run.rs:590-640` and `:905-935`; `case.rs:166-186`; `loom-protocols/src/lib.rs:72-105`; `resume.rs:285-330`. Decision.yaml at 00e3557, lines 110-345, 455-615 and 670-685.

What I could not establish:
- Whether a goals ground is checked again at `Decide` or at the record, where a moved digest or an expired window would supersede the request. Story 23 tests only the check at `Recommend`. This is scope, not my lane.
- Out of my lane: `stop`'s "20 the suspension or context store" is said to match `resume`, but `resume` gives an unusable resume context exit 1, not 20 (lib.rs:129, :133). That is design.
- Out of my lane: story 23 and stories 16 and 22 all edit `decisions.rs` with no edge between them. That is parallel safety.

```findings
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 377
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the setup cannot build the library-made state directory: run_prepared_intent_with_state reaches run_prepared, which refuses a protocol that is not clock-compatible and binds no commands (intent.rs:921-922), and a question hold needs an inner executor returning NeedsHumanJudgment, which none of drive's executors does (run.rs:626-633), contradicting 'a recorded agent the test asserts is never called', so neither DR-o nor DR-q is held as written"
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 106
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the rebuild 'as b10x-loom resume does' goes through resumed_request (main.rs:681), which refuses a case whose inputs are not a software change's and binds no commands (intent.rs:624-629), and resume_decided_request's arguments carry no executor, so the outcome and question command-line bullets and the first library bullet cannot pass; ledger Open 4 does not hold"
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 439
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the tick setup does not say the two runs name different --catalog files; with one shared file, removing DR-a's catalog file also fails DR-b and the bullet at :450-451 cannot pass"
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 472
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the error-path bullet reads the same whether or not the first tick decided anything before the error, since the second tick decides and resumes both anyway, and it names no exit status for the tick's error"
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 269
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no bullet checks that take or record of a request whose run was stopped prints the stopped line and resumes nothing; only decide, resume and tick --resume are tested after stop"
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 271
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the stop exit statuses 11 (InDoubt) and 20 (store unreadable or unwritable) have no bullet; only 0 and 10 are tested"
- file: .engineering/planning/story/tree-waits-open-decision-requests.md
  line: 100
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "exit 37 for StoreServed and StoreInUse, added in e3r10, has no acceptance bullet; Acceptance asserts neither refusal nor its status"
- file: .engineering/planning/story/goals-grounds-and-free-answers.md
  line: 224
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the show bullet prints the revision_moved note 'when the check gave it', a conditional, and the body names no stored member holding the note (Ground has none at 00e3557:159-167)"
- file: .engineering/planning/story/goals-grounds-and-free-answers.md
  line: 87
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no bullet checks that decisions list or show prints recommended_answer"
- file: .engineering/planning/story/goals-grounds-and-free-answers.md
  line: 90
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no bullet checks that decide with neither OPTION nor --answer exits 10 naming OptionOrAnswer; only both is tested, and the parser could refuse neither with exit 2"
- file: .engineering/planning/story/goals-grounds-and-free-answers.md
  line: 38
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "ESS first names the port GoalsEntries but not the entry state it answers (present, digest, validity window, overridden, document valid), a new noun with no ESS home"
```
