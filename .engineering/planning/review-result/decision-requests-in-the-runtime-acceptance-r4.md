---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-acceptance-r4
kind: review-result
status: active
title: Plan critic acceptance, confirming pass (round 4), on stories 1 to 14 and story:ess-format-31 of epic:decision-requests-in-the-runtime
relations:
- reviews: epic:decision-requests-in-the-runtime
- reviews: story:ess-format-31
- reviews: story:decision-request-entity
- reviews: story:decision-request-file-store
- reviews: story:autonomy-grant-table
- reviews: story:decider-port-and-grounding-check
- reviews: story:grounds-resolve-against-held-cases
- reviews: story:llm-decider
- reviews: story:decision-routing-by-holder
- reviews: story:escalate-when-conditions
- reviews: story:decided-record-written-then-recorded
- reviews: story:decision-suspension-reason
- reviews: story:decision-inbox-commands
- reviews: story:question-and-outcome-decisions-hold-runs
- reviews: story:silence-and-expiry-at-a-host-tick
- reviews: story:approval-decisions-resume-held-runs
revision: 1
---
needs-revision

story:question-and-outcome-decisions-hold-runs — the bullet "`decisions record DR-q --revision answers@a3 --state DIR` after it is refused `ProducedRecordConflict`" cannot come out of the path the body now declares for `decisions record`, which calls `complete_decision` and goes "through the same path and refusals as every other record"; `complete_decision` answers the existing record when the request already has one (story:decided-record-written-then-recorded § Outcome), and if the writer did run, `update_revision_at` would refuse `CaseMovedSinceDecision`; the body must state the check that refuses a different revision, or the bullet must expect what `complete_decision` gives — .engineering/planning/story/question-and-outcome-decisions-hold-runs.md:315-316 (pre-e3r5 wording at HEAD:61-65 sent `RecordProduced` directly)

story:decision-routing-by-holder — the bullet "At `A5` on `budget_overrun` (`rule_final`), a rule `Escalate` appends a record ..." states no stop that `route_stop` can classify as `budget_overrun`; the class comes only from scanning `actions` in the classes document, and story:autonomy-grant-table now accepts a built-in only as `{kind: control}` and refuses one that has `actions`, so a stop built as written gets `UnclassifiedAction`; the body must say how a stop gets a built-in class — .engineering/planning/story/decision-routing-by-holder.md (Opening, "The class is the one whose `actions` list the stop's action"; Acceptance, `budget_overrun` bullet); .engineering/planning/story/autonomy-grant-table.md (Acceptance, "`budget_overrun: {kind: control, actions: [x]}` is refused `BadDocument`")

story:grounds-resolve-against-held-cases — the bullet "the count is 0 after every resolution above" fails if read literally, because the fixture's held evidence id has to be submitted through `EvidencePort::receive`, which calls `EvaluationTime::at` (`crates/loom-governor/src/lib.rs:1512-1514`); it must say the count does not change across each resolution — .engineering/planning/story/grounds-resolve-against-held-cases.md (Acceptance, last bullet before `task check`)

story:decided-record-written-then-recorded — the bullet "a `CaseStore` wrapper over `MemoryCaseStore` whose first `update` raises the case revision" fires during setup if read literally, because reaching `r3` already goes through `update` (`update_revision`, `crates/loom-governor/src/lib.rs:1020`, through `CaseStore::update`, `:373`); it must say the wrapper is armed after setup, or name which update it moves — .engineering/planning/story/decided-record-written-then-recorded.md (Acceptance, fourth bullet)

What I read: all 15 ids. I read `story:ess-format-31` in full through `aep plan artifact show`. For the other fourteen I read the whole body from `.engineering/planning/story/<slug>.md`, plus `git diff HEAD` for stories 4, 7 and 12 and targeted diffs for 3 and 7. I also read the e3r5 ledger, the "Story defects" of all fourteen scope reports, the probe, and the cited lines in Loom (`runtime.rs`, `lib.rs` of governor and CLI, `grant.rs`, `file_store.rs`, `locked_dir.rs`, `workflow.rs`, `reconcile.rs`, conformance `lib.rs`/`resource.rs`, `AGENTS.md:993`/`:1663`, `Taskfile.yml`) and in SDC (`decision.yaml`, `merge-a5-veto.yaml`, `autonomy-levels.md`, `decisions.md`, `decision-grounds.sh`).

What I could not establish:
- story:ess-format-31: I did not check that each of the five commands answers something other than `Unknown { LoomAgentNotFound }` for an absent row at `ess/25`. If one already did, its existence-check bullet reads the same before the change as after. I relied on the probe.
- story:ess-format-31: the bullet "named in the story's commit" does not say which of the story's two commits.
- story:question-and-outcome-decisions-hold-runs: I did not check that `run_with_clock` runs a fixture protocol that is not `software-change@1` all the way to the outcome stop.
- Outside my lane (design): story:autonomy-grant-table ("`ess/25` has none") and story:decider-port-and-grounding-check ("no order at `ess/25`") still give `ess/25` as the reason after the raise to `ess/31`. Both statements are still true at `ess/31`, so this is wording only.
- Every other scope-report defect that the ledger marks fixed is fixed in the body.

```findings
- file: .engineering/planning/story/question-and-outcome-decisions-hold-runs.md
  line: 315
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the bullet expecting ProducedRecordConflict for decisions record --revision answers@a3 cannot come out of the path the body now declares, since complete_decision answers the existing record when the request has one and the writer would refuse CaseMovedSinceDecision; the body must state the check that refuses a different revision or the bullet must expect what complete_decision gives"
- file: .engineering/planning/story/decision-routing-by-holder.md
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the A5 budget_overrun bullet states no stop route_stop can classify as budget_overrun, since the class comes only from scanning actions in the classes document and story:autonomy-grant-table refuses actions on a built-in, so a stop built as written gets UnclassifiedAction; the body must say how a stop gets a built-in class"
- file: .engineering/planning/story/grounds-resolve-against-held-cases.md
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the bullet 'the count is 0 after every resolution above' fails if read literally, because the fixture's held evidence goes through EvidencePort::receive, which calls EvaluationTime::at (lib.rs:1512-1514); it must say the count does not change across each resolution"
- file: .engineering/planning/story/decided-record-written-then-recorded.md
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the CaseStore wrapper 'whose first update raises the case revision' fires during setup if read literally, because reaching r3 already goes through CaseStore::update; it must say the wrapper is armed after setup or name which update it moves"
```
