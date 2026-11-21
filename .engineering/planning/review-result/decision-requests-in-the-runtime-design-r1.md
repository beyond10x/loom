---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-design-r1
kind: review-result
status: active
title: Plan critic design, round 1, on epic:decision-requests-in-the-runtime
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

story:decision-routing-by-holder — the EscalationSuppressed records routing keeps "beside the request in the store" are split across three stories: story:decision-request-entity's `DecisionStore` port has no operation to append or read them; story:decision-request-file-store declares a records slot in `StoredDecisionRequest` before routing declares the record's type; routing declares that type, changes no file store and has no edge to file-store. Either the record slot and its port operation move into story:decision-request-entity, or routing extends `FileDecisionStore` itself and gets a depends_on edge to story:decision-request-file-store — .engineering/planning/story/decision-routing-by-holder.md:79-80, :100-101, :94-96; .engineering/planning/story/decision-request-file-store.md:25-26, :45-46; .engineering/planning/story/decision-request-entity.md:27-29; reader over the file store: .engineering/planning/story/decision-inbox-commands.md:24-25

story:silence-and-expiry-at-a-host-tick — its acceptance reads a held `Decision` Suspension, which story:decision-suspension-reason creates and which story:question-and-outcome-decisions-hold-runs (a dependant of this story) first holds for a request, and no depends_on edge runs to story:decision-suspension-reason; the line belongs in story:question-and-outcome-decisions-hold-runs, which already states it — .engineering/planning/story/silence-and-expiry-at-a-host-tick.md:59-60, .engineering/planning/story/decision-suspension-reason.md:18-23, .engineering/planning/story/question-and-outcome-decisions-hold-runs.md:87

story:approvals-supplied-to-canon-evaluation — it joins two pieces of work: the governor and slice supply, which needs only story:approval-decisions-resume-held-runs and story:grounds-resolve-against-held-cases, and the tree's supply. Its one edge, to story:tree-waits-open-decision-requests, holds both behind upstream-blocker:engineering-protocols-on-canon-0-9-0, although its body says it "needs no Canon release". The tree's `AuthorityRecords` implementation can move into story:tree-waits-open-decision-requests, with that story depending on this one, or into its own follow-on story — .engineering/planning/story/approvals-supplied-to-canon-evaluation.md:52-53, :59-60; path in `aep plan artifact graph --format json`: approvals-supplied → tree-waits → tree-run-command → record-copy-between-cases → child-case-runtime → canon-0-9-0-pin → upstream-blocker:engineering-protocols-on-canon-0-9-0

epic:decision-requests-in-the-runtime — the 14 unblocked stories form a 10-deep queue (1→3→4→7→8→9→11→12→13→14), and four of its edges exist only because stories share `crates/loom-decision/src/lib.rs`, `ess/domains/decision.yaml` and `crates/loom-cli/src/lib.rs`. The choice is between that order and giving each story its own module in loom-decision and loom-cli (lib.rs gains one `mod` line per story), which would remove the edges decider-port→grant-table, decided-record→escalate-when, tick→inbox and approval→tick — .engineering/planning/story/decider-port-and-grounding-check.md:66-67, .engineering/planning/story/decided-record-written-then-recorded.md:63-65, .engineering/planning/story/silence-and-expiry-at-a-host-tick.md:45-47, .engineering/planning/story/approval-decisions-resume-held-runs.md:66-67

**What I read:** the epic, all 19 stories, the upstream blocker, story:budget-and-limit-stops-raise-decision-requests, both sdc answer files, AGENTS.md §§ Rules, Governor and Mandate, and the `[dependencies]` of 9 crates. Commands: `aep plan artifact show`/`cat` on each, `aep plan artifact relations`, `aep plan artifact graph --format json`, `aep plan artifact validate`.
- I walked all 365 depends_on and blocks edges in the store, outside the set too; the set has 32 direct edges, and there are no cycles.
- I traced 46 acceptance reads to a producer in the set; 45 have an edge, directly or through a chain, and 1 does not (the tick finding).
- Crate placement holds. loom-decision depends only on the generated `loom` crate. ModelDecider goes in loom-executor, which already has llm-core. FileDecisionStore, HeldCaseGrounds and ExplicitDecisionWriter go in the governor. Commission gets only the `DecidedHold` JSON, so it reads no decision store or Canon. No story links Mandate. None of these adds a crate cycle (governor→executor→commission stays one-way).
- Suspension reason and Canon record write: one owner each, so no finding. `Decision` belongs only to story:decision-suspension-reason. ExplicitDecisionWriter belongs to story:decided-record-written-then-recorded, and each host owns its own approval writer.

**What I could not establish:**
- Out of my lane (ordering): the epic says its Stories list is "in dependency order", but story 16 depends on story 18 (.engineering/planning/story/requests-from-canon-pending-decisions.md:15).
- Out of my lane (scope): story:tree-waits-open-decision-requests lists an `expired` run-log event, but no story makes `decisions tick` write to a tree's run log (.engineering/planning/story/tree-waits-open-decision-requests.md:57-60; .engineering/planning/story/silence-and-expiry-at-a-host-tick.md:31-33).
- Out of my lane (scope): `AutonomyChanged` needs the run's previous resolution, and no story keeps it for the slice or changes a run's level (.engineering/planning/story/autonomy-grant-table.md:59-61).
- Not settled: story 13 re-checks grounds through `HeldCaseGrounds`, which resolves `canon_authority` through `AuthorityRecords`, but the slice's implementation of that port only arrives in story 18. Whether the resolver takes the port as optional is not stated (.engineering/planning/story/grounds-resolve-against-held-cases.md:32-35; .engineering/planning/story/approvals-supplied-to-canon-evaluation.md:36-39).
- Out of my lane (parallel safety): story:decision-suspension-reason and two stories of the other epic all change `responsibility.yaml` with no edge between them; the body says so.
- `aep plan artifact validate` reports `valid`; its warnings are all about older review-results.

```findings
- file: .engineering/planning/story/decision-routing-by-holder.md
  line: 79
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the EscalationSuppressed records routing keeps beside the request are split across three stories: the DecisionStore port of story:decision-request-entity has no operation for them, story:decision-request-file-store declares their slot before their type exists, and routing declares the type with no edge to story:decision-request-file-store; the slot and port operation belong in story:decision-request-entity, or routing extends FileDecisionStore and gets a depends_on edge to story:decision-request-file-store"
- file: .engineering/planning/story/silence-and-expiry-at-a-host-tick.md
  line: 59
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its acceptance reads a held Decision Suspension, which story:decision-suspension-reason creates and story:question-and-outcome-decisions-hold-runs (a dependant of this story) first holds for a request, and no depends_on edge runs to story:decision-suspension-reason; the line belongs in story:question-and-outcome-decisions-hold-runs, which already states it"
- file: .engineering/planning/story/approvals-supplied-to-canon-evaluation.md
  line: 59
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it joins the governor and slice supply, which needs only story:approval-decisions-resume-held-runs and story:grounds-resolve-against-held-cases, with the tree's supply, and its one edge to story:tree-waits-open-decision-requests holds both behind upstream-blocker:engineering-protocols-on-canon-0-9-0 although its body says it needs no Canon release; the tree's AuthorityRecords implementation can move into story:tree-waits-open-decision-requests, with that story depending on this one, or into its own follow-on story"
- file: .engineering/planning/epic/decision-requests-in-the-runtime.md
  line: 121
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the 14 unblocked stories form a 10-deep queue, and four of its edges exist only because stories share crates/loom-decision/src/lib.rs, ess/domains/decision.yaml and crates/loom-cli/src/lib.rs; the choice is between that order and giving each story its own module so lib.rs gains one mod line per story, which would remove the edges decider-port to grant-table, decided-record to escalate-when, tick to inbox and approval to tick"
```
