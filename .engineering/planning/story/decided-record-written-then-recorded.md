---
format: aep.planning-md/3
id: story:decided-record-written-then-recorded
kind: story
status: proposed
title: A decision is taken first, then its Canon record is written, then recorded on the request
summary: Written at the decided case revision r<N> through a revision-guarded governor write, RecordProduced after an idempotent write; an option that produces no record completes at Decide; a record kind with no writer waits for one; completion runs before routing
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:grounds-resolve-against-held-cases
- depends_on: story:escalate-when-conditions
scope:
- confidence: inferred
  path: crates/loom-decision/src/completion.rs
- confidence: cited
  path: crates/loom-decision/src/lib.rs
- confidence: cited
  path: crates/loom-decision/tests/record_completion.rs
- confidence: inferred
  path: crates/loom-governor/src/explicit_decision_writer.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/explicit_decision_writer.rs
- confidence: cited
  path: ess/domains/decision.yaml
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:21Z", actor: "agent:sdc", revision: 13, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
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
  `decided_case_revision`, the case revision at `Decide` (answer e). Record ids are
  `<capability>@r<N>` for an approval and `<outcome>/<decision>@r<N>` for an outcome (point 7,
  confirmed in answer g), where `r<N>` is the governor's case revision as it stamps it
  (`format!("r{}", state.revision)`, `crates/loom-governor/src/lib.rs:1074`; the revision is an
  `i64`, `:193`). The Canon entry carries no request id; `produced_record` holds the link from the
  request to the record (answer g).
- The decision name of an outcome record has no field of its own in the contract: `Unblocks` names
  only the outcome (`~/beyond10x/sdc/ess/domains/decision.yaml:78` at `00e3557`) and `Consequence` only the
  kind of record (`:86-95`). The writer parses `<decision>` from the request's `record_key`
  (`<outcome>/<decision>@r<N>`, `story:decision-routing-by-holder`); a `record_key` of another
  shape is refused `WriterFailed`, naming it, and nothing is written.
- A host gives the completion a set of writers, at most one per `produces` kind
  (`canon_authority`, `canon_decisions`, `canon_evidence`, `artifact_revision`).
- `complete_decision(store, writers, request_id)` reads the request: Decided with a record, it
  answers the record; Decided on an option that produces a record and without one, it writes and
  records it with the writer for that kind; Decided on an option that produces none, it answers
  `NoRecord` and calls no writer; Decided on an option whose kind has no writer in the set, it
  answers `AwaitsWriter` naming the kind, writes nothing, and leaves the request as it was; in any
  other state it is refused `NotDecided`. `complete_pending(store, writers)` does this for every
  Decided request whose option produces a record and that has none, so the next attempt after a
  crash completes what the last one left; it answers the requests it left awaiting a writer, and a
  later call given that writer completes them.
- Order with routing: a host calls `complete_pending` before it routes any stop (at the start of a
  run and before each `route_stop`). Routing supersedes a Decided request without
  `produced_record` whose decided case revision moved (`story:decision-routing-by-holder`); run
  first, completion records a record that a crash left written but unrecorded, so routing never
  supersedes a request whose record exists.
- `ExplicitDecisionWriter` in `b10x-loom-governor` (its own module, one `mod` line in
  `crates/loom-governor/src/lib.rs`) writes an outcome request's record with the request's
  `decided_by` as principal, in two steps:
  1. It reads the case's explicit decisions through a new reader,
     `CanonGovernor::explicit_decisions(case) -> Result<Vec<ExplicitDecision>, GovernorError>`
     (the held `CaseState.decisions`, `lib.rs:205`, which no public reader returns today,
     `:1096-1131`). An entry for the outcome and decision at `r<N>` by the same principal means the
     record is written: it answers `AlreadyRecorded` and writes nothing, whatever the case revision
     is now. The same entry by another principal is `WriterFailed`, naming that principal, and
     nothing is written.
  2. Otherwise it writes through a new entry point,
     `CanonGovernor::decide_at_revision(case, outcome, decision, principal, expected_revision: i64)`,
     which compares `state.revision` with `expected_revision` inside the same `store.update`
     closure as the write (`decide`'s closure, `lib.rs:1068-1089`) and, when they differ, writes
     nothing and returns a new `DecisionError::CaseMoved { expected, current }` (`DecisionError`,
     `lib.rs:629`, is matched outside the crate only in `crates/loom-governor/tests/explicit_decisions.rs:174`,
     `:183`, inside `matches!`, so the variant breaks no caller). `decide` keeps its
     signature and behaviour. A case moved between step 1 and step 2 is caught by step 2.
- If the held case is no longer at the decided case revision and the record was not written at it,
  the completion is refused `CaseMovedSinceDecision { decided: "r<N>", current: "r<M>" }`; the
  request is superseded by the next routing of its stop (`story:decision-routing-by-holder`)
  (answer e).

Approval records are written by the writers of `story:approval-decisions-resume-held-runs` (the
slice's grant store) and `story:tree-waits-open-decision-requests` (the tree's case approvals); a
question's record by a host writer (`story:question-and-outcome-decisions-hold-runs` gives the
slice's).

## Why

Item 3 of the request resumes the suspended run in this order: "`Decide` (all guards, no Canon
write), the Canon write keyed by the record id, `RecordProduced`, then resume, which requires
`produced_record` where the decided option produces a record"
(`~/beyond10x/sdc/docs/decisions.md:310-312`; point 6, and the contract's own order,
`decisions.md:222-237`, `~/beyond10x/sdc/ess/domains/decision.yaml:546`). A Canon-first order
would write an authority record before eligibility is checked. `RecordProduced` with the record
already stored changes nothing (`decision.yaml:708-713`), which is what lets a retry record an
existing record (`AlreadyRecorded`). Which revision the record binds to is answer e; the refusal
when the case moved before the write is stated in prose only (`decisions.md:226-228`), so Loom
enforces it here. `decide` stamps the revision it finds inside its own update
(`crates/loom-governor/src/lib.rs:1074`), so a check made before calling it is a separate step
that a concurrent `update_revision` (`lib.rs:1020`) can overtake; the guard has to sit in the same
update. That only an option producing a record has one to write is answer d (`decision.yaml:95`).
"The decision applies to this revision only" (`decision.yaml:232`). A command line has the writers
of its store option only (`story:decision-inbox-commands`), so a request whose kind it cannot write
must wait for one that can, not fail every command.

## Waits on

`story:escalate-when-conditions`: the last story before this one to change
`ess/domains/decision.yaml` and `crates/loom-decision` (this story's code goes in its own module,
so `lib.rs` gains one `mod` line); through it
`story:decision-routing-by-holder`, whose `Decided` answer this completes.
`story:grounds-resolve-against-held-cases`: both change `crates/loom-governor/src/lib.rs`.

## ESS first

`ess/domains/decision.yaml` gains `CompletionOutcome` (`Recorded`, `AlreadyRecorded`, `NoRecord`,
`AwaitsWriter`) and `CompletionRefusal` (`NotDecided`, `CaseMovedSinceDecision` with `decided` and
`current` case revisions as `r<N>` strings, `WriterFailed`). On that commit `task drift` fails, and
`crates/loom-decision/tests/record_completion.rs` and
`crates/loom-governor/tests/explicit_decision_writer.rs` do not compile.

## Acceptance

In `crates/loom-governor/tests/explicit_decision_writer.rs`, with a fixture protocol whose outcome
`close` declares decision `taken`, held at case revision `r3`:

- An outcome request with `record_key` `close/taken@r3`, Decided `A` at `r3` by `alice`, is
  completed: `explicit_decisions` holds `close`/`taken`/`alice` at `r3`, the request's
  `produced_record` is `canon_decisions` `close/taken@r3`, and the next evaluation reports the
  outcome legitimate.
- A request whose `record_key` is `close@r3` (no decision part) is refused `WriterFailed` naming
  it, and `explicit_decisions` is unchanged.
- With the case moved to `r4` by `update_revision` after `Decide` at `r3`, the completion is
  refused `CaseMovedSinceDecision` with `decided: r3` and `current: r4`, and `explicit_decisions`
  holds no new entry.
- With a `CaseStore` wrapper over `MemoryCaseStore`, armed by the test after setup (the case at
  `r3`, the request Decided) and before the completion, whose next `update` after arming, the one
  `decide_at_revision` makes, raises the case revision from `r3` to `r4` before it applies the
  change (a move between the writer's read and its write),
  the completion is refused `CaseMovedSinceDecision` with `decided: r3` and `current: r4`, and
  `explicit_decisions` holds no new entry.
- `decide_at_revision` with `expected_revision` 3 on a case at `r3` holds the decision at `r3`;
  with 2 it returns `DecisionError::CaseMoved { expected: 2, current: 3 }` and holds nothing.
- With `close`/`taken`/`alice` already held at `r3` and the case moved to `r4`, completing the
  request answers `AlreadyRecorded` with `close/taken@r3`, and `explicit_decisions` holds one entry.
- With `close`/`taken`/`bob` held at `r3`, completing a request decided by `alice` is refused
  `WriterFailed` naming `bob`, and `explicit_decisions` is unchanged.

In `crates/loom-decision/tests/record_completion.rs`, with the in-memory store and fixture writers:

- A crash injected after `Decide` (the writer's fixture fails once) leaves the request Decided
  without a record; `complete_pending` then writes the record once and records it; the writer
  fixture holds one record, not two.
- A crash injected after the write and before `RecordProduced` leaves the record written;
  `complete_pending` writes nothing and records the existing record.
- A crash after the write and before `RecordProduced`, then a move of the case revision from `r3`
  to `r4`, then `complete_pending`, then `route_stop` of the same outcome stop at `r4`: the first
  request carries `produced_record` `close/taken@r3` and is not superseded, and the new request's
  `record_key` is `close/taken@r4` with an empty `supersedes`.
- `complete_decision` on an Open request is refused `NotDecided` and the writer is not called.
- An outcome request decided "leave it untaken" answers `NoRecord`, the writer is not called, and
  `complete_pending` skips it.
- A question request Decided `A` (`produces: artifact_revision`) with only a `canon_decisions`
  writer in the set answers `AwaitsWriter` naming `artifact_revision`, the store is unchanged, and
  `complete_pending` lists it and completes the outcome requests beside it; given a fixture
  `artifact_revision` writer afterwards, `complete_pending` records it.
- `task check` exits 0.

## Out of scope

Resuming the run (`story:decision-suspension-reason`,
`story:approval-decisions-resume-held-runs`). Grounds on the Canon record
(`story:decided-records-carry-grounds-to-canon`). An AEP `approval-record`. Any writer of an
approval or question record. A new field on the request for the decision name.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-decision` (crate `b10x-loom-decision`, a new crate that does not exist at `61fd500b`; `story:decision-request-entity` creates it) and the `loom.decision` domain `ess/domains/decision.yaml` (also created by that story) — cited
- **Files:** `ess/domains/decision.yaml` (gains `CompletionOutcome`, `CompletionRefusal`), `crates/loom-decision/src/lib.rs` (one `mod` line), `crates/loom-decision/tests/record_completion.rs`, `crates/loom-governor/src/lib.rs` (`CanonGovernor::decide` at `:1055`; the story's "Waits on" says this story and `story:grounds-resolve-against-held-cases` both change this file), `crates/loom-governor/tests/explicit_decision_writer.rs` — cited
- **Files (new modules):** `crates/loom-decision/src/completion.rs` (`DecisionRecordWriter`, `complete_decision`, `complete_pending`) and `crates/loom-governor/src/explicit_decision_writer.rs` (`ExplicitDecisionWriter`, a new module plus a `mod` line in `lib.rs`) — inferred; the story names the symbols and the crates, not the file names
- **Symbols:** `DecisionRecordWriter`, `complete_decision`, `complete_pending`, `ExplicitDecisionWriter`, `CompletionOutcome`, `CompletionRefusal`, `ProducedRecord`, `CanonGovernor::decide` (`lib.rs:1055`) — cited
- **Symbols (read, not changed):** `CaseState.revision` (`lib.rs:193`), `CaseState.decisions` (`:205`), `CaseFacts.case_revision` (`:1199`), `DecisionError` (`:629`) — cited
- **Also likely:** an internal revision-guarded variant of `decide` in `crates/loom-governor/src/lib.rs` — inferred, see defect 1; `crates/loom-conformance/src/decision.rs` is probably untouched, because the new types are not commands
- **Documents:** none beyond the ESS file. The generated reference pages and `AGENTS.md` are house-rule files — cited
- **Confidence:** high for the `ess/` file, the two test files and the `loom-decision` module, because the story names them. Medium for the governor change, because `decide` takes no revision (defect 1) and the story does not say how the guard is made atomic.
- **Would collide with** (`depends_on` order read from `aep plan artifact graph --format json`):
  - `ess/domains/decision.yaml` and `crates/loom-decision/src/lib.rs`, ordered **before** this story by `depends_on` paths: `decision-request-entity`, `autonomy-grant-table`, `decider-port-and-grounding-check`, `llm-decider`, `decision-routing-by-holder`, `escalate-when-conditions`.
  - Same two files, ordered **after** it by `depends_on` paths: `silence-and-expiry-at-a-host-tick` (direct), `decision-inbox-commands` (direct), `approval-decisions-resume-held-runs`, `question-and-outcome-decisions-hold-runs`, `tree-waits-open-decision-requests`, `non-independent-review-becomes-decision-request`.
  - `crates/loom-governor/src/lib.rs`, ordered **before**: `grounds-resolve-against-held-cases` (direct), `decision-request-file-store`.
  - `crates/loom-governor/src/lib.rs` and `ExplicitDecisionWriter`, ordered **after**: `decided-records-carry-grounds-to-canon` (it changes the writer and `decide`), `approvals-supplied-to-canon-evaluation`, `authority-records-bound-and-grounded-in-canon`, `requests-from-canon-pending-decisions`.
  - `crates/loom-governor/src/lib.rs`, **no `depends_on` path either way**: `canon-0-9-0-pin` (wave `2026-11-21-w2`, active; its hunks are `lib.rs:129-132` and `:1963-1990`, away from `decide`), `bound-artifact-revisions`, `child-case-runtime`, `hash-chained-run-log`, `engineering-registry-calls-replaced`, `record-copy-between-cases`. The same file would be a textual merge, not a semantic one, unless one of them also edits `decide` or `CaseState`.
  - `crates/loom-governor/src/file_store.rs` is not touched by this story. If the revision guard is implemented in `file_store.rs`, the unordered `child-case-runtime` and `hash-chained-run-log` would collide.
  - `story:decision-suspension-reason` is unordered with this story but touches no file this story touches (`file_suspension.rs` and commission files).
- **Safety fact:** a retry cannot write twice, because `decide` returns `Ok(())` without a change when the same `ExplicitDecision` (decision, outcome, principal, revision) is already held (`crates/loom-governor/src/lib.rs:1076-1078`). Step 2, unproven. It holds only for the same principal at the current revision; see defects 1 and 2.
