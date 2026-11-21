---
format: aep.planning-md/3
id: story:decided-record-written-then-recorded
kind: story
status: draft
title: A decision is taken first, then its Canon record is written, then recorded on the request
summary: Written at the decided case revision, RecordProduced after an idempotent write; an option that produces no record completes at Decide; a retry completes a half-finished decision
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:grounds-resolve-against-held-cases
- depends_on: story:escalate-when-conditions
revision: 4
---
## Outcome

A decided request whose option produces a Canon record gets that record in the order session
`sdc` answered on 2026-11-21 (point 6): `Decide` or `TakeRecommendation` first, with every guard
and no Canon write; then the runtime writes the record the decided option's consequence names;
then `RecordProduced` sets `produced_record`; then the run may resume. A decided option that
produces no record (an outcome's "leave it untaken") has nothing to write: the run resumes or ends
directly after `Decide` (answer d).

- `DecisionRecordWriter` in `b10x-loom-decision` is the port that writes a decided request's
  record and answers the `ProducedRecord`. The record is written at the request's
  `decided_case_revision`, the case revision at `Decide` (answer e). A write is idempotent by
  record id (`<capability>@<case_revision>`, `<outcome>/<decision>@<case_revision>`, point 7,
  confirmed in answer g): a writer that finds that record already written by the same principal
  answers it and writes nothing. The Canon entry carries no request id; `produced_record` holds the
  link from the request to the record (answer g).
- `complete_decision(store, writer, request_id)` reads the request: Decided with a record, it
  answers the record; Decided on an option that produces a record and without one, it writes and
  records it; Decided on an option that produces none, it answers `NoRecord` and calls no writer;
  in any other state it is refused `NotDecided`. `complete_pending(store, writer)` does this for
  every Decided request whose option produces a record and that has none, so the next attempt after
  a crash completes what the last one left.
- `ExplicitDecisionWriter` in `b10x-loom-governor` writes an outcome request's record through
  `CanonGovernor::decide` (`crates/loom-governor/src/lib.rs:1052`) at the request's decided case
  revision, with the request's `decided_by` as principal.
- If the held case is no longer at the decided case revision and the record was not written at it,
  the writer writes nothing and the completion is refused `CaseMovedSinceDecision`, naming both
  revisions; the request is superseded by the next routing of its stop
  (`story:decision-routing-by-holder`) (answer e).

Approval records are written by the writers of `story:approval-decisions-resume-held-runs` (the
slice's grant store) and `story:tree-waits-open-decision-requests` (the tree's case approvals); a
question's record by a host writer.

## Why

Item 3 of the request resumes the suspended run in this order: "`Decide` (all guards, no Canon
write), the Canon write keyed by the record id, `RecordProduced`, then resume, which requires
`produced_record` where the decided option produces a record"
(`~/beyond10x/sdc/docs/decisions.md:310-312`; point 6, and the contract's own order,
`decisions.md:222-237`, `~/beyond10x/sdc/ess/domains/decision.yaml:517`). A Canon-first order
would write an authority record before eligibility is checked. `RecordProduced` with the record
already stored changes nothing (`decision.yaml:665-670`), which is what lets a retry record an
existing record (`AlreadyRecorded`). Which revision the record binds to is answer e; the refusal
when the case moved before the write is stated in prose only (`decisions.md:226-228`), so Loom
enforces it here. That only an option producing a record has one to write is answer d
(`decision.yaml:95`). "The decision applies to this revision only" (`decision.yaml:230`).

## Waits on

`story:escalate-when-conditions`: the last story before this one to change
`ess/domains/decision.yaml` and `crates/loom-decision` (this story's code goes in its own module,
so `lib.rs` gains one `mod` line); through it
`story:decision-routing-by-holder`, whose `Decided` answer this completes.
`story:grounds-resolve-against-held-cases`: both change `crates/loom-governor/src/lib.rs`.

## ESS first

`ess/domains/decision.yaml` gains `CompletionOutcome` (`Recorded`, `AlreadyRecorded`, `NoRecord`)
and `CompletionRefusal` (`NotDecided`, `CaseMovedSinceDecision`, `WriterFailed`). On that commit
`task drift` fails, and `crates/loom-decision/tests/record_completion.rs` and
`crates/loom-governor/tests/explicit_decision_writer.rs` do not compile.

## Acceptance

- An outcome request Decided `A` is completed: the governor holds the explicit decision at the
  decided case revision with the request's principal, the request's `produced_record` is
  `canon_decisions` `<outcome>/<decision>@<rev>`, and the next evaluation reports the outcome
  legitimate.
- A crash injected after `Decide` (the writer's fixture fails once) leaves the request Decided
  without a record; `complete_pending` then writes the record once and records it; the governor
  holds one explicit decision, not two.
- A crash injected after the write and before `RecordProduced` leaves the record written;
  `complete_pending` writes nothing and records the existing record.
- `complete_decision` on an Open request is refused `NotDecided` and the writer is not called.
- With the case revision moved between `Decide` and the write, the completion is refused
  `CaseMovedSinceDecision` naming both revisions, the governor holds no new decision, and routing
  the stop again supersedes the request.
- An outcome request decided "leave it untaken" answers `NoRecord`, the writer is not called, and
  `complete_pending` skips it.
- `task check` exits 0.

## Out of scope

Resuming the run (`story:decision-suspension-reason`,
`story:approval-decisions-resume-held-runs`). Grounds on the Canon record
(`story:decided-records-carry-grounds-to-canon`). An AEP `approval-record`.
