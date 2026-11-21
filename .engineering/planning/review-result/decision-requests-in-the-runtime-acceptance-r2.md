---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-acceptance-r2
kind: review-result
status: active
title: Plan critic acceptance, round 2, on epic:decision-requests-in-the-runtime
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

The round-1 fixes all hold in the stored bodies. The revision left four acceptance defects, all introduced by it.

Findings:

- `story:approval-decisions-resume-held-runs` — the moved-ground bullet cannot show its stated result. It moves `spec` and expects resume to refuse with the request "Superseded naming a new Open request". `update_revision` raises the case revision (`crates/loom-governor/src/lib.rs:1042`). `resume_after_grant` then refuses `CaseMoved` before it asks the provider (`crates/loom-commission/src/runtime.rs:2410`), so `DecisionAuthority` never reaches its grounds re-check. The bullet is also indistinguishable from the one above it, "With the case moved after the decision, `resume` is refused `CaseMoved`". The fix is a ground that moves while the case revision stays: a `path_line` file, or an `aep_artifact`. — `.engineering/planning/story/approval-decisions-resume-held-runs.md:135`
- `story:authority-records-bound-and-grounded-in-canon` — "After `spec` moves to `t2` with the case at `r3`" describes a state the governor cannot produce. Moving `spec` raises the case to `r4` (`lib.rs:1042`), which the bullet above already covers as "After the case moves to `r4`". As written, the set-aside by `stale_grounds` can't be told apart from the revision binding. The bullet needs a ground that moves at an unchanged case revision, or it needs to say what Canon reports when the case moves too. — `.engineering/planning/story/authority-records-bound-and-grounded-in-canon.md:71`
- `story:requests-from-canon-pending-decisions` — the bullet "An action approval-required by two requirements gives two entries and two requests with distinct `question_key`s and `record_key`s" contradicts `story:decision-routing-by-holder`, so it can't pass as written.
  - Routing defines `question_key` as `<case>|<action>|<question id or empty>` (`decision-routing-by-holder.md:42`), which is the same for two entries of one action.
  - Distinct record keys need distinct capabilities. Two entries with the same capability at one revision give one request, because the second is reused under `RecordKeyOpen`.
  - Either the bullet drops "distinct `question_key`s" and names a fixture with two capabilities, or routing's key gains the capability. — `.engineering/planning/story/requests-from-canon-pending-decisions.md:74`
- `story:approvals-supplied-to-canon-evaluation` — the story says "Loom's own check of an approval's grounds before it allows stays", but also that "the authority provider is asked only about actions Canon still reports approval-required". Once the grant is handed to Canon, the action is admissible and the provider that re-checks grounds is never asked. No bullet shows the moved-ground check still running after the supply, so the check can disappear at this story's commit with every bullet green. Add one bullet with a ground that moves at an unchanged case revision, run with the authority records supplied. — `.engineering/planning/story/approvals-supplied-to-canon-evaluation.md:48`

What I read: 21 of 21 artifacts (the epic, the 19 stories and the upstream blocker), via `aep plan artifact show` on the epic and blocker and the stored story bodies. I also read `e3-round-1-acceptance.md`, `e3r2-changes.md`, `e3-sdc-answers-4.md`, `autonomy-levels.md:236-262`, `decision.yaml` (error names, `question_key`), and `runtime.rs` and `lib.rs` of the governor.

Round-1 check: all nine findings hold as fixed.
- The `EscalationSuppressedRecord` bullets are in the file-store story.
- `NoLevel` and `UnknownClass` are covered.
- The grounding refusals and `Take` are covered, and the four hand-made rows are named.
- Every routing holder row now has a bullet.
- The `escalate_when` variants are covered.
- Inbox supersede, history, show-records and `--at` are covered.
- The Superseded refusal and no-suspend paths are covered.
- Tree-waits covers the moved ground and the run-log lines.

Fixture values in `autonomy-grant-table` and routing match `autonomy-levels.md:242-257`.

What I could not establish:
- I did not read `e3-sdc-answers.md`, `-2.md` or `-3.md` in full. I took their points as the epic's Why states them.
- I did not check Canon's own stale-ground semantics for the blocked stories 17 to 19, and I did not run anything.
- `story:tree-waits-open-decision-requests:143` (moved `spec` ground) looks reachable, because the tree has no `CaseMoved` refusal before its grounds check. I accepted it.

Out of my lane, and they did not set the verdict:
- Design: in a Loom-held case an `artifact_revision` ground always moves the case revision too, so Canon's `stale_grounds` on authority and decision entries is redundant with the revision binding. Only `path_line`, `aep_artifact` and `sdc_decisions` grounds can go stale alone.
- Design: `question-and-outcome-decisions-hold-runs` has no CLI writer for a question's record, so end-to-end resume of a question runs only through a fixture host writer.

```findings
- file: .engineering/planning/story/approval-decisions-resume-held-runs.md
  line: 135
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the moved-ground bullet moves spec, which raises the case revision, so resume is refused CaseMoved before the provider is asked and the stated result (request Superseded naming a new Open request) cannot be observed; the bullet also duplicates the CaseMoved bullet above it and needs a ground that moves at an unchanged case revision"
- file: .engineering/planning/story/authority-records-bound-and-grounded-in-canon.md
  line: 71
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "'After spec moves to t2 with the case at r3' describes a state the governor cannot produce (update_revision raises the case to r4), so the stale_grounds set-aside cannot be told from the revision binding of the bullet above"
- file: .engineering/planning/story/requests-from-canon-pending-decisions.md
  line: 74
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the bullet asks for two requests with distinct question_keys for one action, but the routing story defines question_key as case|action|question-id, identical for two entries of one action, and names no fixture with two capabilities for distinct record_keys"
- file: .engineering/planning/story/approvals-supplied-to-canon-evaluation.md
  line: 48
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the story says Loom's own grounds check before an allow stays while the provider is asked only about actions Canon still reports approval-required, and no bullet shows the moved-ground check still running after the approvals are supplied, so the check can vanish with every bullet green"
```
