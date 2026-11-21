---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-design-r2
kind: review-result
status: active
title: Plan critic design, round 2, on epic:decision-requests-in-the-runtime
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
approve

The restored drafts change nothing in my verdict, and nothing in this round would change the shape of the set. The ledger's design entries (`.engineering/drafts/e3r2-changes.md:66-70`) match what I checked in the stored bodies:

- **Escalation-suppressed records (round-1 blocker):** fixed. The append and read operations for the records kept beside a request are now on the `DecisionStore` port in `story:decision-request-entity` (`.engineering/planning/story/decision-request-entity.md:30-33`). The file store implements them (`decision-request-file-store.md:25-29`). Routing writes through the port (`decision-routing-by-holder.md:91-96`) and reaches the entity through its chain of dependencies.
- **Tick reading a held `Decision` Suspension:** fixed. The acceptance of `story:silence-and-expiry-at-a-host-tick` no longer reads it (`silence-and-expiry-at-a-host-tick.md:62-76`). The line now sits only in `story:question-and-outcome-decisions-hold-runs`, which depends on `story:decision-suspension-reason`.
- **Approvals story joining the slice and the tree:** fixed. The tree's `AuthorityRecords` implementation moved to `story:tree-waits-open-decision-requests`, which now depends on `story:approvals-supplied-to-canon-evaluation`. Nothing that story depends on leaves the set any more, so the 0.9.0 chain no longer holds it.
- **The 10-deep queue:** declined by the coordinator, so not raised again.

Two round-1 notes are also settled:
- `HeldCaseGrounds` now takes the `AuthorityRecords` port as optional (`grounds-resolve-against-held-cases.md:35-40`).
- The epic's story order now matches the graph.

**What I read:** the epic, all 19 stories and the upstream blocker. After the restore I also read `e3r2-changes.md`, `e3-sdc-answers.md` 1 to 4 and `e3-round-1-design.md`; the stored copy of the round-1 verdict has the same text. Commands: `cat -n` on each body, `aep plan artifact relations`, `aep plan artifact graph --format json`, `aep plan artifact validate`.
- I walked all 370 `depends_on` and `blocks` edges in the store, outside the set too, and found no cycle. The set has 31 direct `depends_on` edges and 3 `blocks` edges.
- I traced 50 acceptance reads to a producer. 49 have an edge, direct or through a chain. The one left over has no producer in the set, so it is a scope gap, not a missing edge (first item below).

**What I could not establish:**
- Out of my lane (scope): `story:question-and-outcome-decisions-hold-runs` needs `decisions decide … --state DIR` to record an explicit decision (`question-and-outcome-decisions-hold-runs.md:86-87`). But `story:approval-decisions-resume-held-runs` gives `--state` only `GrantRecordWriter` (`approval-decisions-resume-held-runs.md:71-72`), and no body adds `ExplicitDecisionWriter` to it. The slice's state directory does hold a `FileCaseStore` (`crates/loom-intake-slice/src/state_dir.rs:4`).
- Out of my lane (scope/acceptance): three acceptances need a stub rule, but no CLI flag supplies a rule ("Loom ships no rule", `decider-port-and-grounding-check.md:31`):
  - the `b10x-loom run` acceptance at A4/A5 (`approval-decisions-resume-held-runs.md:116`, `:121`);
  - the `tree run` acceptance (`tree-waits-open-decision-requests.md:123`);
  - the flag list itself (`approval-decisions-resume-held-runs.md:20-22`) has no rule flag.
- Out of my lane (scope): `story:silence-and-expiry-at-a-host-tick` completes a question request decided by silence. No body says which writer it uses on the command line.
- Out of my lane (scope): answer h asks for the approvals story "as one story after 16" (`e3-sdc-answers-2.md`, item h). The revision puts the slice's supply at story 15 and the tree's inside story 16, which covers the same request. Whether the requester accepts that order is the coordinator's call.
- Out of my lane (scope): the fourth message settles the question that `e3r2-changes.md:88` (§ Open, point 4) waited on. The epic still lists that stop as not in scope (epic `:199-201`).
- `aep plan artifact validate` reports `valid`. Its warnings are all about older review-results.

```findings
[]
```
