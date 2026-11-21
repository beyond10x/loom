---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-scope-r2
kind: review-result
status: active
title: Plan critic scope, round 2, on epic:decision-requests-in-the-runtime
relations:
- reviews: epic:decision-requests-in-the-runtime
- reviews: story:approval-decisions-resume-held-runs
- reviews: story:approvals-supplied-to-canon-evaluation
- reviews: story:authority-records-bound-and-grounded-in-canon
- reviews: story:autonomy-grant-table
- reviews: story:decided-records-carry-grounds-to-canon
- reviews: story:decided-record-written-then-recorded
- reviews: story:decider-port-and-grounding-check
- reviews: story:decision-inbox-commands
- reviews: story:decision-request-entity
- reviews: story:decision-request-file-store
- reviews: story:decision-routing-by-holder
- reviews: story:decision-suspension-reason
- reviews: story:escalate-when-conditions
- reviews: story:grounds-resolve-against-held-cases
- reviews: story:llm-decider
- reviews: story:question-and-outcome-decisions-hold-runs
- reviews: story:requests-from-canon-pending-decisions
- reviews: story:silence-and-expiry-at-a-host-tick
- reviews: story:tree-waits-open-decision-requests
revision: 1
---
needs-revision

- story:decision-routing-by-holder — The router takes "a decider and a rule (`Decider` ports, either absent)", but no row of its holder table says what an absent decider does. Answer b's "under `decider_final` a missing, failed or withheld decider result decides the denying option, or ends the run where no option denies" is claimed only for the failed and withheld cases. The fix is to name the missing case in the `decider_final` row (as cause `failure`, or a new cause) and say what an absent port does under the other holders that involve a decider or a rule. — `.engineering/drafts/e3-sdc-answers-2.md:8-10`, `.engineering/planning/epic/decision-requests-in-the-runtime.md:89-91`, `.engineering/planning/story/decision-routing-by-holder.md:22-23`, `:76`, `:94-95`

**Coordinator's note.** I re-read the restored `e3r2-changes.md`, `e3-sdc-answers.md`, `-2.md`, `-3.md` and `e3-round-1-scope.md`. My round-1 verdict there is the same as the stored `review-result:decision-requests-in-the-runtime-scope-r1`. The answer files say what the epic quotes, and nothing in them changes the verdict or the finding. The ledger's § Open points 3, 4 and 5 (a run that no Commission port can end, the non-independent-review stop, no host computing touched paths) are named omissions, not gaps.

**Round-1 fixes.** All three hold in the stored bodies, as the ledger's scope items 14, 15 and 16 say:
- **"End the run under a final holder" is now claimed at host level.** The epic Outcome says it (`epic/decision-requests-in-the-runtime.md:22-24`). Each host story has its own section and acceptance for `RunEnds`:
  - the approval story: `story/approval-decisions-resume-held-runs.md:48-62`, acceptance `:121-129`, exit 3;
  - the question and outcome story: `story/question-and-outcome-decisions-hold-runs.md:37-44`, acceptance `:104-107`, exit 3;
  - the tree story: `story/tree-waits-open-decision-requests.md:59-66`, acceptance `:135-138`, exit 4.
  - The approval story says why it is exit 3 and not answer 3's exit 6 (`:53-56`).
- **Expiry now has a source.** It is `expires_after` per class in the classes document (`story/autonomy-grant-table.md:22-24`, `:62-67`). The router sets `expires_at` from it (`story/decision-routing-by-holder.md:49-51`). Real runs reach it in acceptance (`story/approval-decisions-resume-held-runs.md:130-133`, `story/tree-waits-open-decision-requests.md:148-150`).
- **`AwaitingPerson` is gone from the tree story.** The story leaves it with `tree approve --action` and tests that it opens no request (`story/tree-waits-open-decision-requests.md:32-33`, `:156-157`). The epic's Not in scope now names it (`epic/decision-requests-in-the-runtime.md:146-147`).

**The rest of the revision.**
- No item claims anything the parent did not ask for.
- No two items claim the same outcome. The split between slice and tree (grounds re-checked before an allow, approvals supplied to Canon, `RunEnds`) is one claim per host.
- Prior-decision reuse is not reach (story 4, `decision-grounds.sh:68-79`), and neither is `evaluate --authority` (story 15, answer h "or in a replay").
- The tree story leaves out questions ("handoffs", `story/tree-waits-open-decision-requests.md:168-169`), and says so. That is a named omission, not a gap.
- The non-independent-review stop is a named exclusion (`epic/decision-requests-in-the-runtime.md:148-150`, ledger § Open point 4). Message 4 concerns that undrafted story and is outside this set; I raise nothing on it.

**What I read.** 21 artifacts: the epic first, written down as promises before any item, then 19 stories and the blocker. I used `aep plan artifact show` on the epic and on the five related stories outside the set (`budget-and-limit-stops-raise-decision-requests`, `run-log-references-exec-file-changes`, `selector-stop-names-set-aside-record`, `tree-run-command`, `canon-0-9-0-pin`), and read the 19 stories and the blocker as files under `.engineering/planning/`. I also ran `aep plan artifact validate`, which says `valid` with nothing about this set. I read my round-1 verdict, the four answer files, the ledger, and `autonomy-levels.md:295-322`, `:375-395`.
- I extracted 49 promises: the request's four items, the nine points, answers a to h with `escalate_when` and the budget/limit and file-change requests, the six points of the third message and its selector rule, and the epic Outcome's own claims.
- 45 trace to a story in the set and 2 to stories of `epic:tree-daemon-under-enforced-limits`, which exist in the graph. The selector rule is a named exclusion, and answer b's missing case traces only in part (the finding).

**What I could not establish.**
- **I could not see the round-1 bodies.** The store and git hold only the current revisions. I checked each fix against the current text and the ledger, not against a diff.
- Out of my lane, not counted in the verdict:
  - **Design:** story 3 resolves an escalation mode (`none`/`person`/`final`), but no story reads it. Finality comes only from the `*_final` holders, so the router does not say what happens when an A5 lowering to `decider` meets mode `final`, or an A1 raise meets `none`.
  - **Design:** the `rule_and_decider_final` row decides "the denying option" on a veto, but does not cover an outcome or question stop, which has none.
  - **Design and acceptance:** `decisions` over `--state` with a question request has no writer for `artifact_revision`, and the bodies do not say what `complete_pending` does with it.
  - **Acceptance:** epic acceptance bullet 5 (a slice run decided by `silence` with its default) is met only by the library test of story 12. Stories 13 and 14 test expiry only.

```findings
- file: .engineering/planning/story/decision-routing-by-holder.md
  line: 76
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the router takes a decider and a rule that may be absent but no holder row says what an absent decider does, so answer b's \"a missing, failed or withheld decider result decides the denying option, or ends the run where no option denies\" is claimed only for failed and withheld results; the decider_final row should name the missing case (cause failure or a new cause) and say what an absent port does under the other decider- and rule-involving holders"
```
