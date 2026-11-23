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
- depends_on: story:wildcard-result-row-does-not-match-timeout
scope:
- confidence: cited
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-bindings/src/lib.rs
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_program.rs
- confidence: inferred
  path: crates/loom-tree/src/replay.rs
- confidence: cited
  path: crates/loom-tree/src/run.rs
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
- confidence: cited
  path: website/docs/reference/.generated.json
revision: 17
---
## Outcome

A bound program whose run allowance is spent for good has its `spent` record, whatever ended the
run that spent it, and `tree run` reaches the step that writes it. Today the record comes only
from a run that records; a last run that is counted and records nothing leaves the allowance spent
and no record, so a protocol whose failure outcome needs the `spent` claim has no legitimate
outcome and no action due: the tree stops (`tree run` exit 4) and a resume cannot move it. Runbook
reported this against 0.60.0. Cites are at `49499053` (tag 0.60.0).

**Before.**

- The rule, as the module states it: "On the run that brings the count to `budget.max_runs` its
  value is `RUNS_SPENT` [...] on any other run it records nothing. A run that records nothing
  (cancelled, or in an exit class) writes none either, so a cancelled last run spends the
  allowance and leaves no such record" (`crates/loom-bindings/src/program.rs:35-40`).
- The `spent` record is computed only after every early return of the step
  (`program.rs:428-448`: `spent` from `allowance(..).spent_by_the_last_run(binding)`, the
  `from: runs` entries last). A run is counted when its observation is kept and holds no `class`
  (the `positions` filter of `allowance`, `program.rs:1113-1124`). So every counted end that
  returns before line 428 spends without writing (read, not changed by this story):
  - cancelled: `:347-353` (moves alone, `Cancelled`);
  - refused by its runner: `:355-360`;
  - a signal: `:368-371` (`ProgramSignalled`, counted, "it holds no class");
  - a non-writer that changed a placed artifact: `:400-415` (`Discarded`, `ChangedByNonWriter`);
  - a report that cannot be read: `:451-458` (`ReportUnreadable`);
  - a value outside the table: `:462-473` (`ResultNotInTable`), which is also where a run killed
    at its `timeout_seconds` lands when its table names neither `timeout` nor `*` (and, after
    `story:wildcard-result-row-does-not-match-timeout`, whenever it names no `timeout`).
  Exits in an `exits` class are not counted (`:20-23`), so they are not part of the defect.
- Every later offer is refused `RunsSpent` in `prepare` before anything starts
  (`program.rs:808-822`, window; `:795-807`, rework limit), and records nothing.
- **`tree run` does not offer most of these again.** `step_inputs` drops from `due` and makes idle
  every action `set_aside` returns (`crates/loom-tree/src/run.rs:993-994`, `:1021`). `set_aside`
  returns each action whose `action finished` line since the last `evaluated` line holds a
  refusal that is not a cancel (`run.rs:1064-1072`, the doc at `:1026-1031`), and an `evaluated`
  line is appended only when the case revision or the decision changed (`evaluated`,
  `run.rs:877-899`). A refused step that moved nothing changes neither, so its action stays set
  aside: after a last run refused `ResultNotInTable`, `ReportUnreadable`, `ProgramSignalled` or by
  its runner, the writer is never offered again and the case stops `Stuck`. Only a cancelled last
  run (a cancel is no such refusal) and a `ChangedByNonWriter` run (performed, not refused) are
  offered again; that offer is refused `RunsSpent`, which sets the action aside in turn.
- The defect is pinned as intended behaviour: `a_cancelled_last_run_writes_no_spent_record`
  (`crates/loom-bindings/tests/bound_program.rs:2627-2665`; the function is at `:2630`) asserts no
  record after the cancel and none after the refused step. `AGENTS.md:1489-1495` documents the
  same and says "the case stops `Stuck` at the next `RunsSpent`".

**After.** Two changes, one rule ("owes its spent record", below), read in both places through one
function `b10x_loom_bindings` exports (inferred name `owes_spent_record(governor, case, binding)`),
computed from the same `Allowance` `prepare` reads (`program.rs:1058-1094`).

1. **The port (D1 a, D2 a).** When the port is offered a step of an action that owes its spent
   record, the step starts nothing and writes that entry's `spent` record (its table's value for
   `spent`) in one `record_step`, with no move, and is performed: `BoundStep` outcome `Recorded`,
   its one record the `spent` record, no run observation (nothing is counted). The `action
   finished` line names the record, so `tree replay` undoes it as any recorded line
   (`crates/loom-tree/src/replay.rs:10-11`). This is the offer path, `program.rs:786-822`, before
   the two `RunsSpent` refusals.
2. **The tree (D5 a).** `step_inputs` keeps in `due` an action `set_aside` returned when it owes
   its spent record (`run.rs:993-994`, `:1021`): it is offered once more, and that offer is the
   write of 1. Once the record is there the action no longer owes it, so a later refusal sets it
   aside as today; no loop is possible. Nothing else of `set_aside` changes.

Then the outcome that requires the `spent` claim ends the case at the next evaluation (the record
moves the decision, so `evaluated` appends a line). This covers every no-record end above, and a
case 0.60.0 left stuck: the first `tree run` after the upgrade offers the writer and writes the
record.

**Owes its spent record**: the binding has a `from: runs` entry; the case holds no record of that
entry's `kind` by that entry's `producer` (D4 a); and the allowance is spent for good:

- `budget.runs_per` absent or `case`: `window >= max_runs`;
- `runs_per: rejection`: the rework count is at least `rework_limit`, and either it is above it or
  `window >= max_runs`. A full window with the rework count below `rework_limit` is not spent for
  good (a later rejection opens a new window) and owes nothing.

### Interaction with the rework allowance of 0.60.0 (checked)

Form (a) as Runbook wrote it ("when an offer is refused `RunsSpent` [...] Loom writes it then")
breaks `story:rework-allowance-after-a-rejection` D10 if it writes on every `RunsSpent`: under
`runs_per: rejection` a full window refuses `RunsSpent` naming `budget.max_runs` while the rework
count is below `rework_limit` (`program.rs:808-822`), and D10 says "A full window alone writes no
`spent`: a later rejection opens a new window, so `spent` there would end a `given_up` protocol
with allowance still to come". With "spent for good", (a) keeps D10:

- the D10 stated limit (a full window below `rework_limit`, due again with no new rejection,
  refused naming `budget.max_runs`, stops `Stuck` with no record) is unchanged: owes nothing, and
  the run.rs exemption does not apply;
- D12's stated limit (a `retry_when` only the action's own run writes, rework count 0) is
  unchanged: never spent for good;
- a first run at `rework_count == rework_limit` that was cancelled or recorded nothing now gets its
  record on the next offer; one that recorded keeps D10's "only the first run made at that count
  writes it", because the case then holds the record;
- a rework count carried to `rework_limit` by an unplaced rejection (D2 of that story) with a full
  window gets its record; 0.60.0 never wrote one there.

Under `runs_per: case` nothing in the rework story is read.

### Defaults kept, and what the source changed

D1 a stays the default, but the source (`run.rs:877-899`, `:1022-1072`) shows that the port's
change alone is not reached by `tree run` for four of the seven ends; D5 a (the exemption in
`step_inputs`) is the smallest change that reaches it, and it is now part of the story. D2, D3 and
D4 keep their defaults.

## Decisions

- **D1. Which form.** Default **a**.
  - a. The offer of an action that owes its spent record writes it (with D5 a so `tree run` makes
    that offer). Covers every no-record end and a case 0.60.0 left stuck. Cost: one read of the
    case's records per offer of a spent action and per step of a set-aside writer; no run, no
    clock read.
  - b. A cancelled run is not counted. Covers only the cancel path; reverses D2 a of
    `story:outcome-when-reads-the-run-count` and the ESS comment "A cancelled run counts against
    `budget.max_runs`" (`ess/domains/bindings.yaml:359`); lets a cancelled store run the action
    again after the upgrade.
  - c. The run that spends writes `spent` on every counted end (each early return of
    `program.rs:347-473`). Covers new runs, not a case 0.60.0 left stuck (that writer stays set
    aside); contradicts "A refused step records no evidence" (`ess/domains/bindings.yaml:473-474`)
    and the cancel story's "nothing recorded".
- **D2. How the write shows in the run log.** Default **a**.
  - a. A performed step that starts nothing, its one record the `spent` record, named by the
    `action finished` line (no `exit`).
  - b. The offer stays refused `RunsSpent` and the port writes the record beside the refusal: the
    line names no record, so `tree replay` cannot undo it (its undo takes out only what a line
    names, `replay.rs:10-11`).
- **D3. The record's facts.** Default **a**: `observed_at` is the instant of the action's last
  counted run's observation, and the record is attributed to that observation; about the revision
  the case holds now for the entry's subject. No clock read. **b**: the evaluation's current
  instant, attributed to no run.
- **D4. "The case holds no record".** Default **a**: any record of the entry's `kind` by the
  entry's `producer` in the case, at any revision. **b**: only one about the subject's current
  revision (a revision another action moved would then write a second `spent`).
- **D5. How `tree run` reaches the offer.** Default **a**.
  - a. `step_inputs` keeps an action that owes its spent record out of the set-aside (above).
    Cost: one exported function and three lines in `step_inputs`.
  - b. `tree run` writes the record itself before it selects, for every set-aside action that owes
    it: a write no `action finished` line names, so replay and `tree verify` must accept a record
    without a line, and a second writer of records beside the port.
  - c. `set_aside` drops every refusal of an action whose allowance is spent (not only those that
    owe a record): a writer refused `RunsSpent` would be offered at every step, a loop bounded only
    by `max_steps`.

## ESS first

No noun changes. Comments only: `ess/domains/bindings.yaml:78-79` (`Runs`: also the step that
writes an owed record) and the `ProgramRun` comment at `:359`. Then `task generate`,
`task suite-generate` (`SLOT_TIMEOUT: 30m`) and `task docs-generate`, never by hand.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs are POSIX
sh, never Python. No test checks its own wall time. A slot job ends within 10 minutes, or names its
time with `slot --timeout` (at most 30 minutes). No test binary is added.

**`crates/loom-bindings/tests/bound_program.rs`**, in process through the port:

1. `a_cancelled_last_run_writes_no_spent_record` is renamed
   `a_cancelled_last_run_gets_its_spent_record_on_the_next_offer` and reversed: after the cancel
   no record; the next offer starts nothing (the marker file is absent), is performed, and the
   case then holds one record of the `from: runs` kind with the table's value for `spent`, whose
   `observed_at` equals the cancelled run's observation's instant and whose attribution names that
   observation (D3 a; D3 b fails it); the offer after that is refused `RunsSpent` and the case still
   holds one such record. Failing first: refused `RunsSpent`, no record.
2. `a_last_run_outside_the_table_gets_its_spent_record` (`max_runs: 1`, exit 3, table
   `{"0": valid}`): the run is refused `ResultNotInTable`; the next offer writes the `spent`
   record. Failing first: refused `RunsSpent`, no record.
3. `a_recorded_last_run_writes_spent_once`: `max_runs: 1`, exit 0: the run writes `spent` as today;
   the next offer is refused `RunsSpent` and writes nothing (one `spent` record). Guard.
4. Rework, `runs_per: rejection`, `rework_limit: 2`, `max_runs: 1`:
   `a_full_window_below_the_rework_limit_writes_no_spent` (one rejection, window full, offer
   refused naming `budget.max_runs`, no record; D10 kept; guard) and
   `a_cancelled_run_at_the_rework_limit_gets_its_spent_record` (two rejections, the run at the
   limit cancelled, the next offer writes `spent`; failing first: refused, no record).

**`crates/loom-tree/tests/runs_spent/mod.rs`**, through `run_tree`, the protocol of
`an_outcome_waits_for_the_spent_claim` (`:259`), the writer's table `{"0": done}` and its program
exiting 3 on its last allowed run:

5. `a_refused_last_run_ends_given_up` (new code): one `run_tree` on a fresh case: the last run is
   refused `ResultNotInTable`; the run ends `Terminal` with the `given_up` outcome, the log holds an
   `action finished` line of the writer that names the `spent` record and no `exit`, and
   `replay_tree` over the store reports no difference. Failing first: `Stuck`, no record.
6. `a_store_left_stuck_by_0_60_ends_given_up`: the test writes a store and log in the shape 0.60.0
   leaves (the writer's last counted run refused `ResultNotInTable`, its refused `action finished`
   line, no `evaluated` line after it, no `spent` record), then runs `run_tree` once: `Terminal`,
   `given_up`. Failing first: `Stuck`.
7. `a_refused_run_below_the_allowance_stays_set_aside` (guard of D5 a): `max_runs: 3`, a first run
   refused `ResultNotInTable`: the writer is not offered again before the case changes, as today.

**Gate.** `task check` exits 0; `task slow` passes after the ESS comment change.

## Does not do

- D1 b: count or not count a cancelled run differently.
- D1 c: write `spent` from the run that spends, on each of its early ends.
- D2 b: a `spent` record beside a `RunsSpent` refusal that no log line names.
- D3 b: an instant read from the clock.
- D4 b: a `spent` record per subject revision.
- D5 b: a record written by `tree run` outside a step; D5 c: lifting the set-aside of every spent
  writer.
- Write `spent` on a full window below `rework_limit` (D10 of the rework story stays).
- Change what a `from: runs` table may list, `RunsSpent`'s text, or a person or open executor.

## Scope

Derived 2026-11-23 from the source at `49499053`. **Cited**: read at the line named.
**Inferred**: a reading that could be wrong. Only what changes is listed as changed.

- `crates/loom-bindings/src/program.rs` — changed: the offer path `:786-822`, a new exported
  function beside `Allowance` (`:1058-1094`), its export in `src/lib.rs` (inferred). Read, not
  changed: the early ends `:347-473` (other stories change `:375-378` and `:462-473`).
- `crates/loom-tree/src/run.rs` — changed: `step_inputs` `:990-1021`; read: `:877-899`,
  `:1026-1101`, `:1118-1164` — cited.
- `crates/loom-tree/src/replay.rs` — read for D2 a (`:10-11`); changed only if a line with no
  `exit` needs a case there — inferred.
- `crates/loom-bindings/tests/bound_program.rs` (`:2627-2665`),
  `crates/loom-tree/tests/runs_spent/mod.rs` (`:259`) — cited.
- `ess/domains/bindings.yaml` (`:78-79`, `:359`) — cited; `generated/ess/loom`,
  `generated/rust/loom`, `website/docs/reference/.generated.json` regenerated.
- `AGENTS.md` (`:1489-1495`), `website/docs/guides/bindings.md` (`:199-203`) — cited;
  `CHANGELOG.md` — one `### Fixed` bullet under `[Unreleased]`.

## Edges

- **depends_on `story:wildcard-result-row-does-not-match-timeout`**: the same release (0.61.0),
  after it. After the wildcard story every timed-out run without a `timeout` row is refused;
  this story is what gives such a run's case its `spent` record when the run was the last.
- **ESS and generated files.** `story:bound-program-timeout-above-an-hour`, the wildcard story,
  `story:program-error-carries-its-reason` and this story change `ess/` and so the one inputs
  digest of `generated/ess/loom/synthesis.txt:5`: they merge one at a time, and each after the
  first rebuilds `generated/ess/loom`, `generated/rust/loom` and the docs stamp
  (`task generate`, `task suite-generate`, `task docs-generate`, then `task check`, whose
  `docs-check` reads the stamp, `Taskfile.yml:45`, `:204-207`). Not a textual merge.
- **Shared files.** `crates/loom-bindings/tests/bound_program.rs` with the timeout and wildcard
  stories (the timeout story's Edges do not name it); `program.rs` with the wildcard story
  (`look_up` region) and the program-error story (`program_error`). `CHANGELOG.md` `[Unreleased]`
  and `AGENTS.md` with all five stories (timeout, wildcard, program-error, CI budget, this): one
  bullet each, each rebased on the one merged before. The timeout story's `AGENTS.md:1461` cite
  has drifted (its text is at `:1474`); it is not edited here.
