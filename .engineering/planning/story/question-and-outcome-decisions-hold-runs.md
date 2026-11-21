---
format: aep.planning-md/3
id: story:question-and-outcome-decisions-hold-runs
kind: story
status: draft
title: A slice run is held on a question or an outcome decision and resumed once it is decided
summary: DecisionHoldingExecutor; the outcome stop read from the decision document until Canon ships pending_decisions; ExplicitDecisionWriter on --state and decisions record for a question's answer revision; a final holder's end stops the run with exit 3
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decision-suspension-reason
- depends_on: story:approval-decisions-resume-held-runs
revision: 8
---
## Outcome

With the decision flags of `story:approval-decisions-resume-held-runs`, a slice run is also held,
not ended, when it stops on a question or on an outcome that needs an explicit decision.
`DecisionHoldingExecutor` in `b10x-loom-intake-slice` wraps the run's executor; Commission calls
it with the frontier on every step (`crates/loom-commission/src/runtime.rs:1873`).

- Before it calls the inner executor, it reads the frontier's `canon-decision/1` document
  (`CanonGovernor::decision_document`, `AGENTS.md:480-488`). When the frontier admits no action
  and an outcome is blocked with the reason `{"decision": <name>, "present": false}`
  (`~/beyond10x/canon/website/docs/reference/documents.md:77`), it routes an outcome stop and
  returns `Suspended(Decision{request_id, outcome})`, unless the store holds a request for that
  stop at the current case revision that was Decided on "leave it untaken": then it opens nothing
  and returns the inner executor's outcome, so the run ends as the frontier decides.
- When the inner executor returns `NeedsHumanJudgment` whose request holds a question and at least
  two options, it routes a question stop and returns `Suspended(Decision{request_id, question})`.
  A request without them ends the run `NeedsHumanJudgment`, as today.
- If the route decided the request at once and its record is written, or the decided option
  produces none, the wrapper does not suspend: it returns the inner executor's outcome on the next
  step.
- On `RunEnds` (`story:decision-routing-by-holder`; an outcome and a question have no denying
  option, so a final holder with no usable result ends the run) the wrapper does not suspend: for
  an outcome stop it returns the inner executor's outcome, and the frontier, which admits nothing,
  ends the loop `NoAdmissibleAction`; for a question stop it returns the inner
  `NeedsHumanJudgment`, which ends the loop. The slice reports
  `stopped: Refused (decision <id> ended the run: <cause>)` and exits 3, as
  `story:approval-decisions-resume-held-runs` does for an approval, and holds no Suspension. The
  request is left Open, for the reasons that story gives: `Supersede` needs a newer request, a
  listed person may still decide it, and the next run routes it again by its record key.

Writers behind `b10x-loom decisions … --state DIR` (and `decisions tick --state DIR`):

- `canon_authority`: `GrantRecordWriter` (`story:approval-decisions-resume-held-runs`);
- `canon_decisions`: `ExplicitDecisionWriter` (`story:decided-record-written-then-recorded`) over
  the state directory's `FileCaseStore` (`crates/loom-intake-slice/src/state_dir.rs:4`), which this
  story adds to the set, so `decisions decide` on an outcome request records the explicit decision
  before it exits;
- `artifact_revision`: no writer in the set. The contract's question record is "a new revision of
  `questions.yaml` with `answer` and `answered_by`" (`~/beyond10x/sdc/docs/decisions.md:212`), id
  `artifact@revision` (`~/beyond10x/sdc/ess/domains/decision.yaml:208`, `:214`), written by SDC's
  runtime into SDC's own file; it does not say who writes the answer for another host, and writing
  an answer into an application file is not Loom's (epic, Not in scope). So `complete_pending` over
  `--state` answers `AwaitsWriter` for a Decided question request and leaves it as it was, and
  `decisions decide` on one prints the command that records it. The smallest host writer is a
  command: `b10x-loom decisions record ID --revision ARTIFACT@REV --state DIR`, run by the host
  after it wrote the answer, records the revision in the held case (`CanonGovernor::update_revision`,
  `crates/loom-governor/src/lib.rs:1020`, which refuses an artifact the case does not declare) and
  then `RecordProduced` with `ARTIFACT@REV`, idempotent by that id. It is refused for a request that
  is not Decided on an option producing `artifact_revision`.

`b10x-loom resume` on a commission holding a `Decision` Suspension reads the request. Decided on
an option that produces a record and carrying its `produced_record`, it calls
`resume_after_decision` with `Recorded` and the record; Decided on an option that produces none, it
calls it with `NoRecord`, so the resume and the end of the run follow the decision directly
(session `sdc`, 2026-11-21, answer d; `story:decision-suspension-reason`). Decided without the
record its option produces, or not Decided, it is refused `DecisionNotRecorded`, naming the request
and its state. An Expired request leaves the run held; a Superseded one is refused naming the
superseding request.

A `HumanDecisionRequest` opens a question request only when it holds `question` and two or more
`options`, the shape this story declares; Commission declares no structure for it
(`ess/commission/domains/responsibility.yaml:106-109`). `DecisionHoldingExecutor` goes in its own
module of `crates/loom-intake-slice`, and `decisions record` in the inbox's module of
`crates/loom-cli`.

## Why

Item 3 of the request resumes "the suspended run" for every kind of decision
(`~/beyond10x/sdc/docs/decisions.md:309-312`), while today only an approval holds a run
(`runtime.rs:384-392`) and the slice treats `NeedsHumanJudgment` as unexpected
(`crates/loom-intake-slice/src/run.rs:928`). Until Canon ships `pending_decisions`, the outcome
stop is read from the decision document (session `canon`, 2026-11-21). A held question can only
resume once its record is set, so the command line needs a way to set it.

## Waits on

`story:decision-suspension-reason`: the `Decision` reason and `resume_after_decision`.
`story:approval-decisions-resume-held-runs`: the decision flags, the state directory's store and
`--state` on `decisions`, and both change `crates/loom-intake-slice/src/run.rs`, `resume.rs` and
`crates/loom-cli/src/lib.rs`.

## ESS first

`ess/intake/domains/routing.yaml` gains `QuestionStop` (question, options), the shape a
`HumanDecisionRequest` must have to open a question request, and the refusal
`DecisionNotRecorded`. On that commit `task intake-drift` fails, and
`crates/loom-cli/tests/question_outcome_decisions.rs` does not compile.

## Acceptance

With fixture protocols (one declaring an artifact `answers`), stub programs and a recorded
selector, no model call:

- A run whose root outcome is blocked on decision `release` with nothing admissible ends
  `Suspended` with a `Decision` Suspension; `decisions decide DR-… A --as timo --state DIR`
  records the explicit decision in the state directory's case store and sets `produced_record`;
  `resume` continues and the case completes through that outcome.
- `resume` before the decision is refused `DecisionNotRecorded` and the Suspension stays.
- `decisions decide DR-… B --as timo --state DIR` ("leave it untaken") writes no record; `resume`
  releases the Suspension, opens no new request, and the run ends `NoAdmissibleAction` with
  nothing performed.
- A fixture executor returning `NeedsHumanJudgment` with a question and options `A`, `B` holds the
  run for a question request; `decisions decide DR-… A --as timo --state DIR` leaves the request
  Decided with no `produced_record` and prints the `decisions record` command; `resume` is then
  refused `DecisionNotRecorded`.
- After that, `decisions record DR-… --revision answers@a2 --state DIR` raises the case revision,
  sets `produced_record` `answers@a2`, and `resume` continues at the moved case revision; running
  the same `decisions record` again changes nothing, and one with `answers@a3` is refused
  `ProducedRecordConflict`.
- `decisions record` naming an artifact the case does not declare is refused naming it and
  changes nothing; on an outcome request it is refused and changes nothing.
- Any `decisions … --state DIR` command run while that question request awaits its record exits 0
  and leaves it Decided without a record.
- `NeedsHumanJudgment` with a free-text request ends the run as today and opens no request.
- After `decisions tick` expires the request, `resume` is refused and the run stays held: its
  `Decision` Suspension is still `Pending`.
- With the held request superseded (`decisions supersede DR-1 --by DR-2 --reason … --state DIR`,
  `DR-2` a fixture request), `resume` is refused naming `DR-2`, and the Suspension stays.
- At `A4` on the root outcome `close` (`decider`), a recorded verified decider reply `A` decides
  the request at once and its record is written: the run holds no Suspension and completes through
  that outcome in the same `run`. With a reply `B` ("leave it untaken") the run holds no
  Suspension, writes no record, and ends `NoAdmissibleAction` in the same `run`.
- At `A5` on the root outcome `close` (`decider_final`), a decider failure ends the run with
  `stopped: Refused (decision DR-… ended the run: failure)` and exit 3; no Suspension is held, and
  the request is Open with one `EscalationSuppressedRecord` of cause `failure`. At `A5` a question
  stop with the same failure ends the run the same way.
- `task check` exits 0.

## Out of scope

Writing a question's answer into an application file (the host does, then runs
`decisions record`). Trees (`story:tree-waits-open-decision-requests`). Building stops from
Canon's `pending_decisions` (`story:requests-from-canon-pending-decisions`).
