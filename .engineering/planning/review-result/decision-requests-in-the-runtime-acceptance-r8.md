---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-acceptance-r8
kind: review-result
status: active
title: Plan critic acceptance, round 8, confirming pass on stories 15, 16, 20, 21, 23 and 24 of epic:decision-requests-in-the-runtime
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

story:goals-grounds-and-free-answers — the slice's `goals_port.rs` bullet needs a held run, but the run comes from `run_with_clock`. That function runs `Mode::Once` and keeps no Suspension, which the e3r12 ledger itself gives as the reason story 21 dropped it. So "the slice's resume refuses `repository.merge` as not granted … Superseded" would get `NothingSuspended` instead. The bullet does not say how the run is held — .engineering/planning/story/goals-grounds-and-free-answers.md (§ Acceptance, `goals_port.rs` bullets, "After the same `take` … the slice's resume refuses") (crates/loom-intake-slice/src/run.rs:528; .engineering/drafts/e3r12-changes.md, findings 1 and 13)

story:goals-entries-from-a-resolved-document — the `F2` bullets do not say that `F2` is written at the path `goals.document` names. If `F2` is a separate file, the provider still reads `F`, whose revision no longer matches the root case's, so the `revision_moved` bullet and the no-restart daemon bullet get `invalid_goals` instead of verified — .engineering/planning/story/goals-entries-from-a-resolved-document.md (§ Acceptance, `engine_goals.rs` bullets "With `F` resolved again into `F2`" and "the test writes `F2`")

story:goals-entries-from-a-resolved-document — the daemon bullet moves the root case's `goals` revision while `tree serve` holds the store's one-writer lock. The setup does that through `CanonGovernor::update_revision` over the store (§ "The fixture"), and the body names no way to change a served store's case revision that the daemon then reads — .engineering/planning/story/goals-entries-from-a-resolved-document.md (§ Acceptance, "With the same daemon still running … sets the root case's revision to `F2`'s")

story:goals-entries-from-a-resolved-document — the tree bullets name no routing fixture: no autonomy documents, no principals, no level at which the recorded decider is asked. `goals-tree` is described only as a copy of `story:tree-run-command`'s tree with `goals` declared. Story 23's tree test, by contrast, names the routing of `tree_decisions.rs` — .engineering/planning/story/goals-entries-from-a-resolved-document.md (§ "The fixture"; § Acceptance, `engine_goals.rs` tree bullets)

story:approvals-supplied-to-canon-evaluation — the two slice bullets name no test file. § ESS first names only `crates/loom-governor/tests/authority_supplied.rs`, which cannot drive the slice. They also do not say that `decide` and `take` go through `loom_sdk::decisions`. Story 21 depends on this story, and from story 21 on, `decisions decide/take --state` resumes in the same invocation, so "`resume` performs it once" gives exit 10, and the `take` resumes before the lines are rewritten. Story 21 edits only three named `approval_decisions.rs` tests — .engineering/planning/story/approvals-supplied-to-canon-evaluation.md:118-125 (decisions-resume-the-held-run.md, § Outcome, "Three tests of `crates/loom-cli/tests/approval_decisions.rs` change")

story:non-independent-review-becomes-decision-request — the body names no `unblocks.kind` for a `Review` request. The contract has only `approval`, `question` and `outcome`, and since e3r12 this story lands after story 23, which allows a free answer exactly when the kind is `question`. No bullet checks that `decisions decide DR-… --answer` on a review request exits 10 naming `AnswerNotAllowed`, and a review's `canon_evidence` record has no member for an answer — .engineering/planning/story/non-independent-review-becomes-decision-request.md:28-39 (sdc 00e3557:ess/domains/decision.yaml:63-66, :568-573)

Round-7 findings, against the stored bodies:
- **Story 21:** both are fixed.
  - The holds are built by `run_with_suspension` over `FileSuspensionStore::open(DIR)` (:469-475; file_suspension.rs:93 exists).
  - `record --state` exits 31 at :62-66 and has its bullet in § Acceptance.
- **Story 23:** all three are fixed.
  - A re-check bullet each: `Take` withheld, prior not reused, record withheld, slice and tree supersede.
  - The port reaches the check through each entry (slice and tree, apart from the run-holding defect above).
  - Two `supports` bullets in `free_answer.rs`.
- **Story 24:** all five are fixed.
  - Six `GoalsDocumentNotResolved` bullets.
  - A daemon bullet that sees a rewritten document.
  - Its own `goals-tree` fixture.
  - § "What a resolved document cannot tell", with two bullets.
  - The revision check, with four bullets.
- **Story 10's** stale 31/32 line is corrected (:287).

Story 24 checks:
- The revision form matches SDC's `revision_of`: `t` plus the first 16 hex digits of the sha256 (sdc 00e3557:driver/src/main.rs:191-199).
- A mismatch and a root case without `goals` are both `invalid_goals`, each with a bullet.
- "What remains: SDC's write path" names the remaining gap honestly: whoever writes both the file and the case revision can make a ground verify. § Out of scope lists it.
- The § Open defaults I checked (5 and 4) match `check.rs:428-443`.

Story 16 checks:
- Its bullets are still checkable. Names (`AuthorityRecords`, `ExpiryInstant`, `CaseRevisions`, `complete_pending`) and statuses (3, 4, 37) match the settled stories.
- `StoreServed` is stated with its owner (`story:tree-daemon`) and its test owner (`story:daemon-continues-a-decided-request`) at :97-108.
- Every read is ordered: 12, 13, 15 and 21 are its edges, and the rest comes through them.

Story 20: nothing in the body assumes it lands before story 23. § Waits on (:122-131) states the new order and the reason for it.

Edges (design lane, not setting my verdict):
- No cycle over the whole graph, and every `depends_on` target exists.
- Story 20 → story 23 is an edge of order only by inference: whichever lands second edits the other's `ModelDecider`/`Propose` code (ledger § Open 8).
- Story 24's three edges order every read it makes.

What I read: 6 ids. Stories 24, 15, 16 and 20 in full from the stored bodies; stories 21 and 23 at the round-7 sections; story 10 :280-290; story 13 :94-100 and :244-256. Also the r8 brief, the e3r12 ledger, sdc `driver/src/main.rs:188-200` and `decision.yaml:60-100` at 00e3557, `governor/src/lib.rs:2040-2066`, `runtime.rs:2430-2450`, and a reachability and cycle script over `aep plan artifact graph --format json`.

What I could not establish:
- Whether story 13 changes `run_with_clock` so that it holds with the decision flags. Its `decision_authority_rule.rs` uses "`run_with_clock` and the slice's resume" (approval-decisions-resume-held-runs.md:248-250), which conflicts with the e3r12 ledger's reading. If story 13 does change it, finding 1 falls away.
- Story 15 cites the private `decide_case` at `lib.rs:2042-2065`. It is now at `:2058-2066`. The citation is stale but harmless; I am not raising it.

```findings
- file: .engineering/planning/story/goals-grounds-and-free-answers.md
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the slice goals_port.rs bullet needs a held run but builds it with run_with_clock, which runs Mode::Once and keeps no Suspension (run.rs:528, the e3r12 ledger's own reason for dropping it from story 21), so the slice's resume would be refused NothingSuspended rather than not granted, and the bullet does not say how the run is held"
- file: .engineering/planning/story/goals-entries-from-a-resolved-document.md
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the F2 bullets do not say F2 is written at the path goals.document names; if F2 is a separate file the provider still reads F, whose revision no longer matches the root case's, and the revision_moved and no-restart daemon bullets get invalid_goals instead of verified"
- file: .engineering/planning/story/goals-entries-from-a-resolved-document.md
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the daemon bullet moves the root case's goals revision while tree serve holds the store's one-writer lock, and the body names no way to change a served store's case revision that the daemon then reads"
- file: .engineering/planning/story/goals-entries-from-a-resolved-document.md
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the tree bullets name no routing fixture (autonomy documents, principals, the level at which the recorded decider is asked); goals-tree is only a copy of tree-run-command's tree with goals declared, whereas story 23's tree test names the routing of tree_decisions.rs"
- file: .engineering/planning/story/approvals-supplied-to-canon-evaluation.md
  line: 118
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the two slice bullets name no test file (ESS first names only the governor's authority_supplied.rs) and do not say decide and take go through loom_sdk::decisions; story 21 depends on this story and makes decisions decide/take --state resume in the same invocation, so 'resume performs it once' gives exit 10 and the take resumes before the lines are rewritten, while story 21 edits only three named approval_decisions.rs tests"
- file: .engineering/planning/story/non-independent-review-becomes-decision-request.md
  line: 28
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the body names no unblocks.kind for a Review request, while the contract has only approval, question and outcome and story 23, now ordered before this story, allows a free answer exactly when the kind is question; no bullet checks that decisions decide --answer on a review request exits 10 naming AnswerNotAllowed"
```
