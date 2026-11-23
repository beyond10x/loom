---
format: aep.planning-md/3
id: story:spent-program-has-its-spent-record
kind: story
status: draft
title: A bound program whose run allowance is spent for good has its spent record, whatever ended the run that spent it
summary: 'Runbook: a counted last run that records nothing (cancelled, ResultNotInTable, a timeout outside the table, ReportUnreadable, a signal, a runner refusal, ChangedByNonWriter) leaves runs >= max_runs and no spent record, so the tree stops (exit 4); D1 a: the offer of an allowance spent for good writes the missing from: runs record, starting nothing; under runs_per: rejection only at rework count >= rework_limit, so D10 of the rework story holds'
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_program.rs
- confidence: inferred
  path: crates/loom-tree/src/history.rs
- confidence: cited
  path: crates/loom-tree/tests/runs_spent
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: generated/ess/loom
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/bindings.md
revision: 11
---
## Outcome

A bound program whose run allowance is spent for good has its `spent` record, whatever ended the
run that spent it. Today the record comes only from a run that records; a last run that is
counted and records nothing leaves the allowance spent and no record, so a protocol whose failure
outcome needs the `spent` claim has no legitimate outcome and no action due: the tree stops
(`tree run` exit 4) and a resume cannot move it. Runbook reported this against 0.60.0. Cites are at
`49499053` (tag 0.60.0).

**Before.**

- The rule, as the module states it: "On the run that brings the count to `budget.max_runs` its
  value is `RUNS_SPENT` [...] on any other run it records nothing. A run that records nothing
  (cancelled, or in an exit class) writes none either, so a cancelled last run spends the
  allowance and leaves no such record" (`crates/loom-bindings/src/program.rs:35-40`).
- The `spent` record is computed only after every early return of the step
  (`program.rs:428-448`: `spent` from `allowance(..).spent_by_the_last_run(binding)`, the
  `from: runs` entries last). A run is counted when its observation is kept and holds no `class`
  (`allowance`, `program.rs:1096`, the `positions` filter). So every counted end that returns
  before line 428 spends without writing:
  - cancelled: `:347-353` (moves alone, `Cancelled`);
  - refused by its runner: `:355-360`;
  - a signal: `:368-371` (`ProgramSignalled`, counted, "it holds no class");
  - a non-writer that changed a placed artifact: `:400-415` (`Discarded`, `ChangedByNonWriter`);
  - a report that cannot be read: `:451-458` (`ReportUnreadable`);
  - a value outside the table: `:462-473` (`ResultNotInTable`), which is also where a run killed
    at its `timeout_seconds` lands when its table lists neither `timeout` nor `*`
    (`value_of`, `:1337-1339`; `look_up`, `:1367-1373`).
  Exits in an `exits` class are not counted (`:20-23`), so they are not part of the defect.
- Every later offer is refused `RunsSpent` in `prepare` before anything starts
  (`program.rs:808-822`, window; `:795-807`, rework limit), and records nothing.
- The defect is pinned as intended behaviour: `a_cancelled_last_run_writes_no_spent_record`
  (`crates/loom-bindings/tests/bound_program.rs:2627-2665`; the test function is at `:2630`)
  asserts no record after the cancel and none after the refused step. `AGENTS.md:1489-1495`
  documents the same and says "the case stops `Stuck` at the next `RunsSpent`".

**After.** When the port is offered a step of an action whose binding has a `from: runs` entry,
whose allowance is spent for good (rule below), and the case holds no record of that entry's kind
by that entry's producer, the step starts nothing and writes that entry's `spent` record (its
table's value for `spent`) in one `record_step`, with no move (D1 a, D2 a). The next offer is
refused `RunsSpent` as today, and the writer is idle because its only open evidence is the spent
kind (`spent_writers_idle`, `crates/loom-tree/src/run.rs:1118-1164`), so the outcome that
requires the `spent` claim ends the case. This covers every path above, and a resume of a case
that 0.60.0 left stuck: the first offer after the upgrade writes the record.

"Spent for good", read from the same `Allowance` `prepare` reads (`program.rs:1058-1094`):

- `budget.runs_per` absent or `case`: `window >= max_runs`;
- `runs_per: rejection`: the rework count is at least `rework_limit` and either it is above it or
  `window >= max_runs`. A full window with the rework count below `rework_limit` is not spent for
  good (a later rejection opens a new window) and writes nothing.

### Interaction with the rework allowance of 0.60.0 (checked)

Form (a) as Runbook wrote it ("when an offer is refused `RunsSpent` [...] Loom writes it then")
breaks `story:rework-allowance-after-a-rejection` D10 if it writes on every `RunsSpent`: under
`runs_per: rejection` a full window refuses `RunsSpent` naming `budget.max_runs` while the rework
count is below `rework_limit` (`program.rs:808-822`), and D10 says "A full window alone writes no
`spent`: a later rejection opens a new window, so `spent` there would end a `given_up` protocol
with allowance still to come". With the "spent for good" rule above, (a) keeps D10:

- the D10 stated limit (a full window below `rework_limit`, due again with no new rejection,
  refused naming `budget.max_runs`, stops `Stuck` with no record) is unchanged: no write;
- D12's stated limit (`rework_on`/`retry_when` that only the action's own run writes, rework
  count 0) is unchanged: never spent for good, no write;
- a first run at `rework_count == rework_limit` that was cancelled or recorded nothing now gets its
  record on the next offer; one that recorded keeps D10's "only the first run made at that count
  writes it", because the case then holds the record and nothing more is written;
- a rework count carried to `rework_limit` by an unplaced rejection (D2 of that story) with a full
  window gets its record on the refused offer; 0.60.0 never wrote one there.

Under `runs_per: case` nothing in the rework story is read.

## Decisions

- **D1. Which form.** Default **a**.
  - a. Write the missing `spent` record on the offer of an allowance spent for good (rule above).
    Covers every no-record end (seven paths above), a timeout under
    `story:wildcard-result-row-does-not-match-timeout`, and a case 0.60.0 already left stuck.
    Cost: one check of the case's records per offer of a spent action; no run, no clock read.
  - b. A cancelled run is not counted. Covers only the cancel path, not `ResultNotInTable`, a
    timeout, `ReportUnreadable`, a signal, a runner refusal or `ChangedByNonWriter`; it reverses
    D2 a of `story:outcome-when-reads-the-run-count` and the ESS comment "A cancelled run counts
    against `budget.max_runs`" (`ess/domains/bindings.yaml:359`), and lets a cancelled store run
    the action again after the upgrade.
  - c. The run that spends writes `spent` on every counted end: each early return of
    `program.rs:347-473` writes the moves and the `spent` record. Covers new runs only, not a case
    0.60.0 left stuck; contradicts "A refused step records no evidence"
    (`ess/domains/bindings.yaml:473-474`) and the cancel story's "nothing recorded".
- **D2. How the write shows in the run log.** Default **a**.
  - a. The offer is a performed step that starts nothing: `BoundStep` outcome `Recorded`, its one
    record the `spent` record, no run observation (so nothing is counted), and the step report
    names the record, so the `action finished` line names it and replay and `tree verify` read it
    as any recorded step (inferred: a performed step with no `run` needs a report shape without
    `exit`; the unit says which).
  - b. The offer stays refused `RunsSpent` and the port writes the record beside the refusal: the
    log line names no evidence, so the replay must accept a record no line names.
- **D3. The record's facts.** Default **a**: observed at the instant of the action's last counted
  run's observation, attributed to that observation, about the revision the case holds now for the
  entry's subject (no clock read, deterministic). **b**: the evaluation's current instant.
- **D4. "The case holds no record yet".** Default **a**: any record of the entry's `kind` by the
  entry's `producer` in the case, at any revision. **b**: only one about the subject's current
  revision (a revision moved by another action would then write a second `spent`).

## ESS first

No noun changes. Comments only: `ess/domains/bindings.yaml:78-79` (`Runs`: also on the offer of an
allowance spent for good) and the `ProgramRun` comment near `:359`. Then `task generate` and
`task suite-generate` (`SLOT_TIMEOUT: 30m`), never by hand.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs are POSIX
sh, never Python. No test checks its own wall time. A slot job ends within 10 minutes, or names its
time with `slot --timeout` (at most 30 minutes). No test binary is added.

**`crates/loom-bindings/tests/bound_program.rs`**, in process through the port:

1. `a_cancelled_last_run_writes_no_spent_record` is renamed
   `a_cancelled_last_run_gets_its_spent_record_on_the_next_offer` and reversed: after the cancel
   no record; the next offer starts nothing (the marker file is absent), is performed, and the case
   then holds one record of the `from: runs` kind with the table's value for `spent`; the offer
   after that is refused `RunsSpent` and the case still holds one such record. Failing first:
   refused `RunsSpent`, no record.
2. `a_last_run_outside_the_table_gets_its_spent_record` (`max_runs: 1`, exit 3, table
   `{"0": valid}`): the run is refused `ResultNotInTable`; the next offer writes the `spent` record.
   Failing first: refused `RunsSpent`, no record.
3. `a_recorded_last_run_writes_spent_once`: `max_runs: 1`, exit 0: the run writes `spent` as today;
   the next offer is refused `RunsSpent` and writes nothing (one `spent` record). Guard.
4. Rework guards (`runs_per: rejection`, `rework_limit: 2`, `max_runs: 1`):
   `a_full_window_below_the_rework_limit_writes_no_spent` (one rejection, window full, offer refused
   naming `budget.max_runs`, no record; D10 kept) and
   `a_cancelled_run_at_the_rework_limit_gets_its_spent_record` (two rejections, the run at the
   limit cancelled, the next offer writes `spent`). Failing first: the second test.

**`crates/loom-tree/tests/runs_spent/mod.rs`**, through `run_tree`:

5. `a_case_left_stuck_by_a_cancel_ends_given_up_on_resume`: the fixture of
   `an_outcome_waits_for_the_spent_claim` (`:259`) whose last run is cancelled stops `Stuck` today;
   the next `run_tree` on the same store ends `Terminal` with the `given_up` outcome, and the log's
   `action finished` line of that step names the `spent` record. Failing first: `Stuck`.

**Gate.** `task check` exits 0; `task slow` passes after the ESS comment change.

## Does not do

- Count or not count a cancelled run differently (D1 b).
- Write `spent` on a full window below `rework_limit` (D10 of the rework story stays).
- Change what a `from: runs` table may list, `RunsSpent`'s text, or a person or open executor.

## Scope

Derived 2026-11-23 from the source at `49499053`. **Cited**: read at the line named.
**Inferred**: a reading that could be wrong.

- `crates/loom-bindings/src/program.rs` (`:35-40`, `:347-473`, `:786-822`, `:1058-1096`) — cited.
- `crates/loom-bindings/tests/bound_program.rs` (`:2627-2665`) — cited.
- `crates/loom-tree/tests/runs_spent/mod.rs` (`:259`) — cited.
- `crates/loom-tree/src/run.rs` (`:1118-1164`), read, not changed — cited.
- `ess/domains/bindings.yaml` (`:78-79`, `:359`, `:473-474`) — cited; `generated/` regenerated.
- `AGENTS.md` (`:1489-1495`), `website/docs/guides/bindings.md` (`:199-203`) — cited;
  `CHANGELOG.md` — inferred.
- The log and replay readers for a performed step with no run (D2 a): `crates/loom-tree/src/history.rs`,
  `crates/loom-tree/src/replay*` — inferred.

## Edges

- No `depends_on`.
- **Would collide with**, no edge: `story:wildcard-result-row-does-not-match-timeout` and
  `story:program-error-carries-its-reason` both change `program.rs` (other regions: `look_up`,
  `program_error`); textual merge, either order. With the wildcard story a timed-out last run
  records nothing more often; this story is what gives such a run its `spent` record.
