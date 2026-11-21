---
format: aep.planning-md/3
id: story:question-and-outcome-decisions-hold-runs
kind: story
status: proposed
title: A slice run is held on a question or an outcome decision and resumed once it is decided
summary: DecisionHoldingExecutor; the outcome stop read from the decision document until Canon ships pending_decisions; a Decision hold stops DecisionRequired; resume_decision_hold in the library; ExplicitDecisionWriter on --state; decisions record through complete_decision with a revision-guarded update; a final holder's end stops the run with exit 3
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decision-suspension-reason
- depends_on: story:approval-decisions-resume-held-runs
scope:
- confidence: cited
  path: crates/loom-cli/src/decisions.rs
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: inferred
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-cli/tests/fixtures
- confidence: cited
  path: crates/loom-cli/tests/question_outcome_decisions.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/decision_hold.rs
- confidence: cited
  path: crates/loom-intake-slice/src/lib.rs
- confidence: cited
  path: crates/loom-intake-slice/src/resume.rs
- confidence: cited
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/decision_hold.rs
- confidence: cited
  path: ess/intake/domains/routing.yaml
revision: 18
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:22Z", actor: "agent:sdc", revision: 18, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

With the decision flags of `story:approval-decisions-resume-held-runs`, a slice run is also held,
not ended, when it stops on a question or on an outcome that needs an explicit decision.
`DecisionHoldingExecutor` in `b10x-loom-intake-slice` wraps the run's executor: `drive` wraps the
`LoomStep` it builds (`crates/loom-intake-slice/src/run.rs:610`) when the decision flags are
given, and a host that runs Commission itself wraps its own executor. Commission calls it with the
frontier on every step (`crates/loom-commission/src/runtime.rs:1873`). It passes `route_stop` the
slice's `ExpiryInstant`, the implementation `story:approval-decisions-resume-held-runs` gives over
the slice's clock.

- Before it calls the inner executor, it runs `complete_pending` over the decision store with the
  slice's writers, then reads the frontier's `canon-decision/1` document
  (`CanonGovernor::decision_document`, `crates/loom-governor/src/lib.rs:1225`; `AGENTS.md`,
  § Governor). When the frontier admits no action and an outcome is blocked with the reason
  `{"decision": <name>, "present": false}`
  (`~/beyond10x/canon/website/docs/reference/documents.md:77`), it routes an outcome stop whose
  action is the outcome, and returns `Suspended(Decision{request_id, unblocks: outcome})`
  (`DecisionHold`, `story:decision-suspension-reason`), unless the route answers `Decided` with a
  request for that stop at the current case revision that was Decided on "leave it untaken"
  (`story:decision-routing-by-holder` reuses it and opens none): then it returns the inner
  executor's outcome, so the run ends as the frontier decides.
- When the inner executor returns `NeedsHumanJudgment` whose request holds a question and at least
  two options, it routes a question stop and returns
  `Suspended(Decision{request_id, unblocks: question})`. A request without them is returned
  unchanged, and the slice treats it as today: `SliceError::Unexpected`
  (`crates/loom-intake-slice/src/run.rs:928-930`).
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

A run held on a `Decision` Suspension is reported, where today any Suspension but a budget one is
`Err(SliceError::Suspended)` (`crates/loom-intake-slice/src/run.rs:876-879`), with the stop line
`stopped: DecisionRequired (<request id>)` and a new `StopReason::DecisionRequired`, exit status 0
as for `ApprovalRequired`: the run reached a human gate (`exit_status`,
`crates/loom-cli/src/lib.rs:195-204`, and the `run --help` table, `:26-35`).

Writers behind `b10x-loom decisions … --state DIR` (and `decisions tick --state DIR`). When this
story lands `decisions` has `--store DIR` and `--at` (`story:decision-inbox-commands`), `tick`
(`story:silence-and-expiry-at-a-host-tick`) and `--state DIR`
(`story:approval-decisions-resume-held-runs`); this story adds `decisions record` and the
writers below to the `--state` set, and
`--tree STORE` comes with `story:tree-waits-open-decision-requests`.

- `canon_authority`: `GrantRecordWriter` (`story:approval-decisions-resume-held-runs`);
- `canon_decisions`: `ExplicitDecisionWriter` (`story:decided-record-written-then-recorded`) over
  the state directory's `FileCaseStore`, opened as `b10x-loom` already opens it for that directory
  (`crates/loom-cli/src/main.rs:550`, `:654`, `:1036`); this story adds the writer to the
  `--state` set, so `decisions decide` on an outcome request records the explicit decision, with
  record id `<outcome>/<decision>@r<N>`, before it exits;
- `artifact_revision`: no writer in the standing set, because only the host knows the revision
  that holds the answer. `complete_pending` over `--state` answers `AwaitsWriter` for a Decided
  question request and leaves it as it was, and `decisions decide` on one prints the command that
  records it.

That command is `b10x-loom decisions record ID --revision ARTIFACT@REV --state DIR`, run by the
host after it wrote the answer. First it reads the request: when the request already carries a
`produced_record`, `decisions record` (this story's code in `crates/loom-cli/src/decisions.rs`)
applies `RecordProduced` with `ARTIFACT@REV` directly and calls no writer, so the contract answers
it: the same record is `already-recorded` and changes nothing, a different one is refused
`ProducedRecordConflict` and the first record stands
(`~/beyond10x/sdc/ess/domains/decision.yaml:710-720`). Otherwise it calls `complete_decision`
(`story:decided-record-written-then-recorded`) with one more writer in the set,
`ArtifactRevisionWriter` given `ARTIFACT@REV`, so it goes through the same path and refusals as
every other record:

- the writer records the revision through a new governor entry point,
  `CanonGovernor::update_revision_at(case, artifact, revision, expected_revision)`, which checks
  inside the same `store.update` that the case is still at the request's `decided_case_revision`
  and otherwise writes nothing and returns a new `UpdateError::CaseMoved { expected, current }`
  (`UpdateError`, `crates/loom-governor/src/lib.rs:596`, is matched outside the crate only in
  tests, by named variants), as `CanonGovernor::decide_at_revision` does for an explicit decision
  (`story:decided-record-written-then-recorded`); it is `update_revision`
  (`crates/loom-governor/src/lib.rs:1020`, which refuses an artifact the case does not declare
  and raises the case revision, `:1042`) with that guard, and `update_revision` keeps its
  signature; then `complete_decision` sends `RecordProduced` with `ARTIFACT@REV`;
- idempotent by that id: a writer that finds the artifact already at `REV` with the case one
  revision past the decided one (`CanonGovernor::revisions`, `lib.rs:1097`, and the case revision
  of `CanonGovernor::action_facts`, `:1166`, `:1199`) answers the record and writes nothing, so a
  retry after a crash between the two steps records it once (`AlreadyRecorded`);
- when the case moved after `Decide` by something other than this answer, the writer writes
  nothing and the completion is refused `CaseMovedSinceDecision` naming both revisions as `r<N>`
  (answer e): the answer was given about a case that is no longer there. The request stays Decided
  without a record, the run stays held, as it does after an Expired request, and when the stop is
  next routed, routing supersedes the unrecorded request (`story:decision-routing-by-holder`);
- it is refused for a request that is not Decided on an option producing `artifact_revision`.

The record is `ARTIFACT@REV` of any artifact the case declares; SDC's `questions.yaml` revision
(`~/beyond10x/sdc/docs/decisions.md:212`; `~/beyond10x/sdc/ess/domains/decision.yaml:208`,
`:216`) is one instance of it, accepted by session `sdc` on 2026-11-21 (fifth message, point 2).

`resume_decision_hold`, a new library function of the slice, continues a commission holding a
`Decision` Suspension. It takes the ports `resume_after_decision` takes
(`story:decision-suspension-reason`) and the decision store, and reads the request. Decided on an
option that produces a record and carrying its `produced_record`, it calls
`resume_after_decision` with `Recorded` and the record; Decided on an option that produces none,
it calls it with `NoRecord`, so the resume and the end of the run follow the decision directly
(session `sdc`, 2026-11-21, answer d). Decided without the record its option produces, or not
Decided, it is refused `DecisionNotRecorded`, naming the request and its state. An Expired request
leaves the run held; a Superseded one is refused naming the superseding request.

`b10x-loom resume` does not call it in this story: `resume` continues `software-change@1` runs
only (`crates/loom-intake-slice/src/intent.rs:695-700`) and refuses every other protocol before
it reads the Suspension, and no `software-change@1` run holds a `Decision` Suspension (its
executor never returns `NeedsHumanJudgment`, and its outcomes need no explicit decision). A host
that runs another protocol resumes through `resume_decision_hold`, exposed through
`loom_sdk::intake::slice`; on the command line, `resume` of a `software-change@1` commission whose
Suspension is a `Decision` one exits 19, Commission's `HeldForDecision`
(`story:decision-suspension-reason`). `resume_decision_hold` returns Commission's refusals of
`resume_after_decision` as they are, `HeldForAuthority` (an `Authority` hold) and `OtherDecision`
(a hold for another request) included; since no command calls it in this story, it gives them no
exit status, and `story:decision-suspension-reason`'s mapping of both to 19 stands until a command
does.

A `HumanDecisionRequest` opens a question request only when it holds `question` and two or more
`options`, the shape this story declares; Commission declares no structure for it
(`ess/commission/domains/responsibility.yaml:106-109`). `DecisionHoldingExecutor` and
`resume_decision_hold` go in their own module of `crates/loom-intake-slice` (`decision_hold.rs`),
`ArtifactRevisionWriter` beside the slice's other writers (`decision.rs`, the module
`story:approval-decisions-resume-held-runs` creates), and `decisions record` in the inbox's module
of `crates/loom-cli` (`decisions.rs`).

## Why

Item 3 of the request resumes "the suspended run" for every kind of decision
(`~/beyond10x/sdc/docs/decisions.md:309-312`), while today only an approval holds a run (the
`Suspended` outcome is handled in `run_with_suspension`,
`crates/loom-commission/src/runtime.rs:1908-1924`, and Commission's resume continues only an
approval, `resume_after_grant`, `:2374`) and the slice treats `NeedsHumanJudgment` as unexpected
(`crates/loom-intake-slice/src/run.rs:928`). Until Canon ships `pending_decisions`, the outcome
stop is read from the decision document (session `canon`, 2026-11-21). A held question can only
resume once its record is set, so the command line needs a way to set it, and it goes through
`complete_decision` so the case-moved rule of answer e covers it. The held-run acceptance runs as
library tests, not through the binary: `drive` builds its executor inline (`run.rs:610`), so no
fixture executor reaches it from the command line, and `resume` continues `software-change@1`
only; adding a seam for both is larger than testing through the entries that exist
(`run_with_clock`, `run.rs:417`; Commission's `run_with_suspension`, `runtime.rs:1181`).

## Waits on

`story:decision-suspension-reason`: the `Decision` reason and `resume_after_decision`.
`story:approval-decisions-resume-held-runs`: the decision flags, the state directory's store and
`--state` on `decisions`, and both change `crates/loom-intake-slice/src/run.rs`, `resume.rs` and
`crates/loom-cli/src/lib.rs`. `story:decided-record-written-then-recorded` through it:
`complete_decision`, `AwaitsWriter`, `CaseMovedSinceDecision` and `decide_at_revision`, beside
which `update_revision_at` goes in `crates/loom-governor/src/lib.rs`.
`story:approvals-supplied-to-canon-evaluation` comes after this story: both change
`crates/loom-cli/src/main.rs` and `lib.rs` and the slice's governor build.

## ESS first

`ess/intake/domains/routing.yaml` gains `QuestionStop` (question, options), the shape a
`HumanDecisionRequest` must have to open a question request, the refusal `DecisionNotRecorded`,
and the stop reason `DecisionRequired`. On that commit `task intake-drift` fails, and
`crates/loom-intake-slice/tests/decision_hold.rs` and
`crates/loom-cli/tests/question_outcome_decisions.rs` do not compile.

## Acceptance

As a library test (`crates/loom-intake-slice/tests/decision_hold.rs`), with a fixture protocol
whose root outcome `close` is blocked on decision `release` and which declares artifacts `intent`
and `answers`, the slice's `FileDecisionStore` in a temporary state directory, fixture autonomy
documents and principals, a recorded decider, and no model call. Outcome bullets run the slice
through `run_with_clock` with a recorded agent that the test asserts is never called; question
bullets run Commission's `run_with_suspension` with `DecisionHoldingExecutor` wrapping a scripted
inner executor; resumes go through `resume_decision_hold`; decisions through `loom_sdk::decisions`
with the `--state` writer set.

- A run of the fixture protocol at `A0` reports `stopped: DecisionRequired (DR-…)` and holds a
  `Decision` Suspension whose hold is `Decision{request_id: DR-…, unblocks: outcome}`.
- Deciding that request `A` as `timo` records the explicit decision with record id
  `close/release@r<N>` and sets `produced_record`; `resume_decision_hold` continues and the case
  completes through `close`.
- `resume_decision_hold` before the decision is refused `DecisionNotRecorded` and the Suspension
  stays `Pending`.
- Deciding `B` ("leave it untaken") writes no record; `resume_decision_hold` releases the
  Suspension, opens no new request, and the run stops `NothingAdmissible` with nothing performed.
- A scripted executor returning `NeedsHumanJudgment` with a question and options `A`, `B` holds
  the run with `Decision{request_id: DR-…, unblocks: question}`; deciding it `A` leaves it Decided
  with no `produced_record`, and `resume_decision_hold` is refused `DecisionNotRecorded`.
- After `ArtifactRevisionWriter` records `answers@a2` for that request through
  `complete_decision`, `resume_decision_hold` continues at the moved case revision.
- A scripted `NeedsHumanJudgment` with a free-text request is returned unchanged by the wrapper and
  no request is opened.
- After a tick expires the held request, `resume_decision_hold` is refused and the `Decision`
  Suspension is still `Pending`.
- With the held request superseded by a fixture request `DR-2`, `resume_decision_hold` is refused
  naming `DR-2`, and the Suspension stays.
- At `A4` on `close` (`decider`), a recorded verified decider reply `A` decides the request at once
  and its record is written: the run holds no Suspension and completes through `close` in the same
  run.
- At `A4` on `close` with a decider reply `B` ("leave it untaken"), the run holds no Suspension,
  writes no record, and stops `NothingAdmissible` in the same run.
- At `A5` on `close` (`decider_final`), a decider failure ends the run with
  `stopped: Refused (decision DR-… ended the run: failure)`; no Suspension is held, and the request
  is Open with one `EscalationSuppressedRecord` of cause `failure`.
- At `A5` a question stop with a decider failure ends the run with the same stop line and holds no
  Suspension.

On the command line (`crates/loom-cli/tests/question_outcome_decisions.rs`), over a state
directory the test prepares through the library: a case of the same fixture protocol in its
`FileCaseStore`, and an outcome request and a question request opened by `route_stop` in its
`FileDecisionStore`:

- `decisions decide DR-o A --as timo --state DIR` exits 0, and the directory's governor holds the
  explicit decision `close/release@r<N>` with principal `timo`.
- `decisions decide DR-q A --as timo --state DIR` leaves the question request Decided with no
  `produced_record` and prints the `decisions record` command.
- After that, `decisions record DR-q --revision answers@a2 --state DIR` raises the case revision
  and sets `produced_record` `answers@a2`.
- Running the same `decisions record` again exits 0 and changes no byte of the state directory.
- `decisions record DR-q --revision answers@a3 --state DIR` after it exits non-zero naming
  `ProducedRecordConflict`; `answers` keeps `a2` and the request keeps `produced_record`
  `answers@a2`.
- With the case moved after the question's `Decide` by a revision of `intent` (the test raises it
  through `CanonGovernor::update_revision`), `decisions record DR-q --revision answers@a2 --state
  DIR` exits non-zero with `CaseMovedSinceDecision` naming both revisions as `r<N>`, `answers`
  keeps its revision, and the request stays Decided without a record.
- `decisions record` naming an artifact the case does not declare is refused naming it and changes
  nothing.
- `decisions record` on the outcome request is refused and changes nothing.
- Any `decisions … --state DIR` command run while the question request awaits its record exits 0
  and leaves it Decided without a record.
- `task check` exits 0.

## Out of scope

Writing a question's answer into an application file (the host does, then runs
`decisions record`). Trees (`story:tree-waits-open-decision-requests`). Building stops from
Canon's `pending_decisions` (`story:requests-from-canon-pending-decisions`). Releasing a run held
on a request that can no longer be recorded. `b10x-loom resume` of a protocol other than
`software-change@1`, and a seam that puts a fixture executor into `drive`.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-intake-slice` (the slice's run, resume and a new hold module) — cited, story Outcome and `## Waits on`
- **Files (slice):** `crates/loom-intake-slice/src/run.rs` (`LoomStep` at :1107, built in `drive` at :610; the `Suspended` reason mapping at :877; `NeedsHumanJudgment` at :928) — cited
- **Files (slice):** `crates/loom-intake-slice/src/resume.rs` — cited, `## Waits on` says this story changes it
- **Files (slice):** `crates/loom-intake-slice/src/lib.rs` (one `mod` line per module) — cited, epic Stories paragraph
- **Files (slice):** a new module for `DecisionHoldingExecutor`; the name is not given, e.g. `decision_hold.rs` — cited that the module is new, name inferred
- **Files (slice):** `ArtifactRevisionWriter` "beside the slice's other writers", so in the module story 13 gives `GrantRecordWriter`; that module does not exist yet — cited, location inferred
- **Files (slice, resume branch):** `crates/loom-intake-slice/src/intent.rs` (`resume_prepared` :676 builds `Mode::Resuming` and calls `drive`; `ResumeState` :536) — inferred, the `Decision` reading and `resume_after_decision` branch land here or in `run.rs:736`
- **Files (CLI):** `crates/loom-cli/src/lib.rs` (`Command`, `ResumeArgs`, `resume_refusal_status`) — cited, `## Waits on`
- **Files (CLI):** `crates/loom-cli/src/main.rs` (`resume` :616; case store opened at :550, :654, :1036) — inferred
- **Files (CLI):** `crates/loom-cli/src/decisions.rs` (the inbox module story 11 creates; `decisions record` and the `--state` writer set go there) — cited as "the inbox's module"; the name is from story 11, "its own module (`decisions.rs`)"
- **Files (ESS):** `ess/intake/domains/routing.yaml` (`QuestionStop`, `DecisionNotRecorded`) — cited
- **Files (test):** `crates/loom-cli/tests/question_outcome_decisions.rs` (new) — cited
- **Symbols:** `DecisionHoldingExecutor`, `ArtifactRevisionWriter`, `QuestionStop`, `DecisionNotRecorded`, `CaseMovedSinceDecision`, `AlreadyRecorded` — cited, new
- **Symbols (existing, read only):** `CanonGovernor::decision_document` (`crates/loom-governor/src/lib.rs:1225`), `CanonGovernor::update_revision` (:1020), `CanonGovernor::decide` (:1055) — cited
- **Symbols (from predecessors, absent from the tree):** `complete_decision`, `complete_pending`, `resume_after_decision`, `DecisionHold`, `ExplicitDecisionWriter`, `GrantRecordWriter` — cited. `git grep` finds none of them, as expected for stories 9, 10 and 13.
- **Also likely:** `crates/loom-cli/tests/fixtures/` (a fixture protocol with an `answers` artifact and a blocked outcome `release`/`close`) — inferred
- **Documents:** none beyond the house-rule files — cited, the acceptance is all code
- **Confidence:** medium — the story names the module placement and the lines it changes, and I read the code. The new file names and the test seam (defect 1) are not decided.
- **Would collide with:**
  - `story:approval-decisions-resume-held-runs`, on `run.rs`, `resume.rs`, `crates/loom-cli/src/lib.rs`, `main.rs` and `routing.yaml`. Ordered: a direct `depends_on`.
  - `story:decision-suspension-reason`, on the `Suspended` mapping in the slice and `grant.rs`. Ordered: a direct `depends_on`.
  - `story:decision-inbox-commands`, on `decisions.rs` and `lib.rs`. Ordered: 14 → 13 → 12 → 11.
  - `story:silence-and-expiry-at-a-host-tick`, on `crates/loom-cli`. Ordered: 14 → 13 → 12.
  - `story:decided-record-written-then-recorded`, on the writer set and `complete_decision`. Ordered: 14 → 13 → 12 → 9.
  - `story:approvals-supplied-to-canon-evaluation`, on `crates/loom-intake-slice` (`grant.rs`, slice governor build) and `crates/loom-cli` (`main.rs`, `lib.rs`). **Not ordered.** Both depend on 13 and neither depends on the other. Its body says "no edge between the two", each in its own module. The shared files are `lib.rs` and `main.rs` where the governor is built.
  - `story:authority-records-bound-and-grounded-in-canon`, on `ess/intake/domains/routing.yaml`. **Not ordered.** Its path is 21 → 15 → 13, and 14 → 13.
  - `story:tree-waits-open-decision-requests` (16), on `crates/loom-cli/src/lib.rs`. Ordered: 16 depends directly on 14.
  - `story:requests-from-canon-pending-decisions` (17), which reuses `DecisionHoldingExecutor`. Ordered: 17 → 16 → 14.
- **Safety fact:** a `NeedsHumanJudgment` request with no `question` and no two options keeps today's path, `SliceError::Unexpected` (`run.rs:928`), so the wrapper changes no run that does not stop on a decision — step 2, `run.rs:928-930`, unproven.
