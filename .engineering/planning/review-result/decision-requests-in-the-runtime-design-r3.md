---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-design-r3
kind: review-result
status: active
title: Plan critic design, round 3, on epic:decision-requests-in-the-runtime
relations:
- reviews: epic:decision-requests-in-the-runtime
- reviews: story:approvals-supplied-to-canon-evaluation
- reviews: story:decided-record-written-then-recorded
- reviews: story:approval-decisions-resume-held-runs
- reviews: story:decision-inbox-commands
- reviews: story:decision-request-file-store
- reviews: story:decision-suspension-reason
- reviews: story:decision-request-entity
- reviews: story:escalate-when-conditions
- reviews: story:grounds-resolve-against-held-cases
- reviews: story:decision-routing-by-holder
- reviews: story:non-independent-review-becomes-decision-request
- reviews: story:llm-decider
- reviews: story:question-and-outcome-decisions-hold-runs
- reviews: story:authority-records-bound-and-grounded-in-canon
- reviews: story:requests-from-canon-pending-decisions
- reviews: story:autonomy-grant-table
- reviews: story:silence-and-expiry-at-a-host-tick
- reviews: story:decider-port-and-grounding-check
- reviews: story:decided-records-carry-grounds-to-canon
- reviews: story:tree-waits-open-decision-requests
revision: 1
---
needs-revision

story:non-independent-review-becomes-decision-request — the story needs a fourth stop kind and a second kind of absent decider, and both belong to `route_stop` in story 7, yet its ESS-first and code placement cover only `crates/loom-tree` and the decider identity. The fourth stop kind is a review stop whose options `A`/`B` produce `canon_evidence`. The second kind of absent decider carries the reason `decider not independent: <condition>`, where story 7 gives only `no decider configured`. The story should declare a review variant of `DecisionStop`, the router's option rule for it, and a reason on an absent decider, in `ess/domains/decision.yaml` and the routing module. — .engineering/planning/story/non-independent-review-becomes-decision-request.md:27-35, :62-64, :83-84, :109-112; .engineering/planning/story/decision-routing-by-holder.md:30-33, :71-77, :139-140

story:question-and-outcome-decisions-hold-runs — `decisions record` applies `update_revision` and `RecordProduced` directly. That makes it a second completion path beside story 9's `DecisionRecordWriter`/`complete_decision`, and the case-moved refusal of answer e, which lives in that writer path, does not apply to it. Story 9 meanwhile says story 14 "gives the slice's" question writer. The command should run through `complete_decision` as an `artifact_revision` writer given the revision. — .engineering/planning/story/question-and-outcome-decisions-hold-runs.md:54-65; .engineering/planning/story/decided-record-written-then-recorded.md:26-27, :35-50, :52-55

The other three changes hold their shape:
- **Escalation mode removed:** no body still reads a mode. `autonomy-grant-table.md:57-66`, `:141`, `:151`, routing `:98-105`, epic `:57-60` and `:220` all agree.
- **`AwaitsWriter`:** sits cleanly in story 9 (`:38-43`), and stories 12 (`:44-46`) and 14 (`:59`) use it.
- **Withholding rule:** placed in story 15 (`:43-49`) with the recursion through `canon_authority` citations cut ("unfiltered records"). Story 16 repeats it (`:83-89`), and story 19 narrows both (`:43-50`) through the chain 19 → 18 → 17 → 16.
- **Round-2 notes:** each fix holds in the stored bodies. The `--state` writers are at story 14 `:47-65`, the rule-free command-line bullets at story 13 `:143-149` and `:176-189`, the tick writers per store at story 12 `:34-42`, and the stop that was out of scope is now story 20.

**What I read:** the epic, all 20 stories and the upstream blocker (full bodies). I also read `e3r3-changes.md`, `e3-sdc-answers-4.md`, `e3-round-2-design.md`, and `selector-stop-names-set-aside-record.md` and `bindings-document.md` from the other epic. Commands: `aep plan artifact relations`, `aep plan artifact graph --format json`, `aep plan artifact validate`.
- I walked 395 `depends_on`/`blocks` edges across the whole store, outside the set too, and found no cycle. The set has 34 `depends_on` edges and 3 `blocks` edges.
- I traced 32 acceptance reads again in the revised and new bodies (stories 9, 12 to 16, 19, 20). All 32 are recorded by a direct edge or a chain. Story 20's reads go to story 16, `story:selector-stop-names-set-aside-record`, and stories 3 and 7 through 16.
- `validate` reports `valid`. Its warnings are about older review-results only.

**What I could not establish:**
- Out of my lane (scope / parallel safety): story 20 makes `ModelDecider` report its model and adapter (`:57-59`). `ModelDecider` is in `crates/loom-executor` (`llm-decider.md:48`), but story 20 places code only in `crates/loom-tree` and `crates/loom-decision` (`:83-84`).
- Out of my lane (acceptance): story 20 declares `NotIndependent` as a refusal reason (`:110`), but its acceptance shows an Escalate with that reason and no refusal (`:131-136`).
- Out of my lane (acceptance): story 14 does not say whether `decisions record` refuses when the case moved after `Decide` by something other than the answer.
- Unease, not a finding: the grounds filter on the supply is written once per host, slice (story 15) and tree (story 16). Story 19 then has to narrow it at three sites (`DecisionAuthority`, the slice supply, the tree supply). A shared wrapper in story 15's governor module would give it one owner. The current split works, so this does not set my verdict.

```findings
- file: .engineering/planning/story/non-independent-review-becomes-decision-request.md
  line: 27
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the story needs a review stop kind with options producing canon_evidence and an absent decider with the reason 'decider not independent', both of which are route_stop's DecisionStop and option table in story:decision-routing-by-holder, yet its ESS first and code placement name only crates/loom-tree and the decider identity, so neither body adds them"
- file: .engineering/planning/story/question-and-outcome-decisions-hold-runs.md
  line: 60
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "decisions record sets produced_record through update_revision and RecordProduced outside story:decided-record-written-then-recorded's DecisionRecordWriter and complete_decision, so the answer-e case-moved refusal placed in that path does not cover it and story 9 says story 14 gives a writer; the command should complete through complete_decision as an artifact_revision writer given the revision"
```
