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
revision: 18
---
## Outcome

A bound program whose run allowance is spent for good has its `spent` record, whatever ended the
run that spent it: `tree run` writes the owed record before it evaluates the next step, as it
already settles placed revisions there. Today the record comes only from a run that records; a
last run that is counted and records nothing leaves the allowance spent and no record, so a
protocol whose failure outcome needs the `spent` claim has no legitimate outcome and no action due:
the tree stops (`tree run` exit 4) and a resume cannot move it. Runbook reported this against
0.60.0. Cites are at `49499053` (tag 0.60.0).

**Before.**

- The rule, as the module states it: "On the run that brings the count to `budget.max_runs` its
  value is `RUNS_SPENT` [...] on any other run it records nothing. A run that records nothing
  (cancelled, or in an exit class) writes none either, so a cancelled last run spends the
  allowance and leaves no such record" (`crates/loom-bindings/src/program.rs:35-40`).
- The `spent` record is computed only after every early return of the step
  (`program.rs:428-448`), and a run is counted when its observation is kept and holds no `class`
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
- **`tree run` does not offer such a writer again, and an offer would not reach a write.**
  - A refused, not cancelled `action finished` line since the last `evaluated` line sets its action
    aside (`crates/loom-tree/src/run.rs:1064-1072`), and `evaluated` appends a line only when the
    case revision or the decision changed (`run.rs:877-899`).
  - `due` holds only writers with a retry record (`due_actions`, `run.rs:955-969`); the refused
    line counts as performed at the current revision (`crates/loom-tree/src/history.rs:144-160`,
    `performed_at`, `:166-169`); and `spent_writers_idle` makes idle a writer not in `due` whose
    open evidence holds only its `from: runs` kinds (`run.rs:1018`, `:1124-1164`). The selector
    takes an action that is `due` before it reads `performed_at` (`crates/loom-executor/src/selection.rs:291-305`),
    so only `due` would bring it back.
  - The writer's `handoff_when` check runs before the selected action is performed
    (`run.rs:343-345`), and its exit 0 stops the run for a person.
  - So after a last run refused `ResultNotInTable`, `ReportUnreadable`, `ProgramSignalled` or by its
    runner, the writer is never offered again; after a cancelled one, the next offer is refused
    `RunsSpent` and then set aside. The case stops `Stuck` either way.
- `tree run` already writes to the case between steps outside any effect: before each step it
  moves the revisions of placed artifacts whose files changed (`PlacedRevisions::refresh`,
  `run.rs:304-312`), then appends `evaluated` (`:313-315`), then builds the step's inputs (`:319`).
- The defect is pinned as intended behaviour: `a_cancelled_last_run_writes_no_spent_record`
  (`crates/loom-bindings/tests/bound_program.rs:2627-2665`) asserts no record after the cancel and
  none after the refused offer. `AGENTS.md:1489-1495` documents the same and says "the case stops
  `Stuck` at the next `RunsSpent`".

**After.**

1. **One function, in `b10x-loom-bindings`** (inferred name `settle_spent(governor, case,
   binding)`, exported from `src/lib.rs`): when the binding **owes its spent record** (below) it
   writes that `from: runs` entry's record (its table's value for `spent`) in one `record_step`
   with no move, and returns the record; otherwise it writes nothing and returns none. It never
   refuses: it writes, writes nothing, or returns the store's error. The record's facts (D3 a):
   `observed_at` is the instant of the action's last counted run's observation, the record is
   attributed to that observation (as the port attributes a run's records, `program.rs:582-612`),
   about the revision the case holds now for the entry's subject. No clock read.
2. **`tree run` calls it (D5 c)** for every action of the case's bindings with a `from: runs`
   entry, after `PlacedRevisions::refresh` and before `evaluated` (`run.rs:304-315`). For each record
   written it appends one `action finished` line for that action naming the record, with `exit: 0`
   and no refusal, no blob and no moves (D2 a: the value `effect_summary` gives a performed effect
   that started no program, `run.rs:1329-1331`). Then `evaluated` sees the changed decision and the
   outcome that requires the `spent` claim ends the case.
3. **The port is unchanged.** An offer of a spent action is refused `RunsSpent` as today
   (`program.rs:795-822`); an embedder that drives the port without `tree run` calls the exported
   function itself.

What this settles, from the facts above:

- **Selection, `due` and idle are not involved**: no action is offered for the write, so neither
  the set-aside, `due`, `performed_at` nor `spent_writers_idle` is changed.
- **No `handoff_when` check runs for it**: the check runs only for an action the selector picked
  (`run.rs:343-345`) or when nothing is useful (`:1614-1617`); the write is neither.
- **At most once per evaluation, and once per case**: the function never refuses without writing,
  and once the record is there the binding no longer owes it, so the next step's call writes
  nothing. A store error ends `tree run` with that error, as a failed refresh does
  (`run.rs:309-312`); it is not retried in the run.
- **Every no-record end** above, and **a case 0.60.0 left stuck**: the first `tree run` after the
  upgrade writes the record before its first evaluation.

**Owes its spent record**: the binding has a `from: runs` entry; the case holds no record of that
entry's `kind`, `subject` and `producer` attributed to a `PROGRAM_SOURCE` observation naming this
case and this action (D4 a); and the allowance is spent for good, read from the `Allowance`
`prepare` reads (`program.rs:1058-1094`):

- `budget.runs_per` absent or `case`: `window >= max_runs`;
- `runs_per: rejection`: the rework count is at least `rework_limit`, and either it is above it or
  `window >= max_runs`. A full window with the rework count below `rework_limit` is not spent for
  good (a later rejection opens a new window) and owes nothing.

### Interaction with the rework allowance of 0.60.0 (checked)

Writing on every `RunsSpent` would break `story:rework-allowance-after-a-rejection` D10: under
`runs_per: rejection` a full window refuses `RunsSpent` naming `budget.max_runs` while the rework
count is below `rework_limit` (`program.rs:808-822`), and D10 says "A full window alone writes no
`spent`: a later rejection opens a new window, so `spent` there would end a `given_up` protocol
with allowance still to come". With "spent for good" D10 holds:

- the D10 stated limit (a full window below `rework_limit`, due again with no new rejection,
  refused naming `budget.max_runs`, stops `Stuck` with no record) is unchanged: owes nothing;
- D12's stated limit (a `retry_when` only the action's own run writes, rework count 0) is
  unchanged: never spent for good;
- the first run at `rework_count == rework_limit`, cancelled or recording nothing:
  - when it filled its window (`window >= max_runs` after it, as with `max_runs: 1`), the allowance
    is spent for good and the record is written before the next step;
  - with runs left in its window, the action runs again as today, and none of those later runs
    writes `spent` (only the first run made at that count does, `spent_by_the_last_run`,
    `program.rs:1082-1093`); once the window is full the allowance is spent for good and the record
    is written before the next step. 0.60.0 never wrote it on that path;
- a first run at that count that recorded keeps D10's "only the first run made at that count
  writes it": the case then holds the record, so nothing is owed;
- a rework count carried to `rework_limit` by an unplaced rejection (D2 of that story) with a full
  window gets its record; 0.60.0 never wrote one there.

Under `runs_per: case` nothing in the rework story is read.

### What the source changed, and the defaults

Round 1 put the write on the port's offer (D1 a) and kept the owing writer in the step's choice
(the earlier D5 a). The source shows that choice does not offer it (`due` holds only writers with
retry records, `run.rs:955-969`; `performed_at` and `spent_writers_idle` keep it out,
`history.rs:144-169`, `run.rs:1124-1164`, `selection.rs:291-305`), that an offer would first run its
`handoff_when` check (`run.rs:343-345`), and that an offer refused before the write
(`CaseOfAnotherProtocol`, `program.rs:297-307`) would repeat it within one run, bounded only by
`--max-steps`, which has no default (`crates/loom-cli/src/lib.rs:647-649`). Writing at the tree
level, where `tree run` already settles revisions between steps, needs none of that. So the
default form is now D5 c with the port unchanged; D3 a and D4 a (with action and subject added)
keep their defaults.

## Decisions

- **D1. What fixes the defect.** Default **a**.
  - a. The record owed by an allowance spent for good is written by one exported function of
    `b10x-loom-bindings` (After 1). Covers every no-record end and a case 0.60.0 left stuck.
  - b. A cancelled run is not counted. Covers only the cancel path; reverses D2 a of
    `story:outcome-when-reads-the-run-count` and the ESS comment "A cancelled run counts against
    `budget.max_runs`" (`ess/domains/bindings.yaml:359`); lets a cancelled store run the action
    again after the upgrade.
  - c. The run that spends writes `spent` on every counted end (each early return of
    `program.rs:347-473`). Covers new runs, not a case 0.60.0 left stuck (that writer stays set
    aside); contradicts "A refused step records no evidence" (`ess/domains/bindings.yaml:473-474`).
- **D2. How the write shows in the run log.** Default **a**.
  - a. One `action finished` line for the action, naming the record, `exit: 0`, no refusal, no
    blob, no moves. `tree replay` undoes it as any line that names a record
    (`crates/loom-tree/src/replay.rs:10-11`); the run log format is unchanged.
  - b. A new run-log event (for example `spent recorded`): a run-log format change, and a new case
    in `tree replay`, `tree status` and the history.
  - c. No line: `tree replay` cannot undo a record no line names (`replay.rs:10-11`).
- **D3. The record's facts.** Default **a**: the instant and attribution of the action's last
  counted run's observation, about the subject's current revision. **b**: the evaluation's
  current instant, attributed to no run.
- **D4. "The case holds the record".** Default **a**: a record of the entry's `kind`, `subject`
  and `producer` attributed to a `PROGRAM_SOURCE` observation naming this case and this action.
  Two writers whose `from: runs` entries share kind and producer (as the `runs_spent` fixture's
  `spent` by `loom`, `crates/loom-tree/tests/runs_spent/mod.rs:69`, could in a two-writer
  protocol) are told apart by the action their records cite. **b**: kind and producer only: one
  writer's record would mark another's present. **c**: also the subject's current revision: a
  revision another action moved would then write a second `spent`.
- **D5. Where the write happens.** Default **c**.
  - a. Keep the owing action in the step's choice: insert it into `due` before
    `spent_writers_idle` (`run.rs:1018`), and the port's offer writes. Needs the offer to skip the
    writer's `handoff_when` check (`run.rs:343-345`), a once-per-evaluation guard for an offer
    refused before the write (`program.rs:297-307`), and the port's write path (a performed step
    with no run, whose line `effect_summary` gives `exit: 0`, `run.rs:1329-1331`).
  - b. The port writes the record beside a `RunsSpent` refusal: not reached by `tree run` (the
    writer is set aside or idle, above), and the refused line names no record.
  - c. `tree run` writes it before `evaluated`, through the exported function, with its own
    `action finished` line (After 2). No selection change, no handoff check, no loop.

## ESS first

No noun changes. Comments only: `ess/domains/bindings.yaml:78-79` (`Runs`: also the record `tree
run` writes for an allowance spent for good) and the `ProgramRun` comment at `:359`. Then
`task generate`, `task suite-generate` (`SLOT_TIMEOUT: 30m`) and `task docs-generate`, never by
hand.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs are POSIX
sh, never Python. No test checks its own wall time. A slot job ends within 10 minutes, or names its
time with `slot --timeout` (at most 30 minutes). No test binary is added.

**`crates/loom-bindings/tests/bound_program.rs`**, in process through the port and the function:

1. `a_cancelled_last_run_writes_no_spent_record` passes unchanged (the port is unchanged; guard).
2. `a_cancelled_last_run_owes_its_spent_record` (red first): after the cancelled last run
   (`max_runs: 1`), the function writes one record of the `from: runs` kind with the table's value
   for `spent`, whose `observed_at` equals the cancelled run's observation's instant and whose
   attribution names that observation (D3 a; D3 b fails it); a second call writes nothing and the
   case holds one such record. Failing first: the function does not exist.
3. `a_last_run_outside_the_table_owes_its_spent_record`: `max_runs: 1`, exit 3, table
   `{"0": valid}`: refused `ResultNotInTable`; the function writes the record. Failing first: none.
4. `a_recorded_last_run_owes_nothing`: `max_runs: 1`, exit 0: the run writes `spent` as today; the
   function writes nothing. Guard.
5. `another_writers_spent_record_does_not_settle_this_one` (D4 a): two program writers whose
   `from: runs` entries are both `{kind: spent, producer: loom}` on one subject, each `max_runs: 1`;
   the first records `spent` on its run, the second's run is cancelled: the function on the second
   writes its own record. Failing first under D4 b: nothing written.
6. Rework, `runs_per: rejection`, `rework_limit: 2`:
   - `a_full_window_below_the_rework_limit_owes_nothing` (`max_runs: 1`, one rejection, window
     full): the function writes nothing (D10 kept). Guard.
   - `a_cancelled_run_at_the_rework_limit_owes_its_record` (`max_runs: 1`, two rejections, the run
     at the limit cancelled): the function writes `spent`.
   - `a_cancelled_first_run_at_the_limit_with_runs_left_owes_nothing_yet` (`max_runs: 2`, two
     rejections, the first run at the limit cancelled): the function writes nothing; after a second
     run that records, it writes `spent` (window full, no record from that run).

**`crates/loom-tree/tests/runs_spent/mod.rs`**, through `run_tree`, the protocol of
`an_outcome_waits_for_the_spent_claim` (`:259`) with `work.do` bound `budget: {max_runs: 1}`, no
`retry_when`, its exit entry `{"0": ok}` (no `*` row) and the fixture's `from: runs` entry; its
program exits 3:

7. `a_refused_last_run_ends_given_up` (new code): one `run_tree` on a fresh case: the only run is
   refused `ResultNotInTable`; the run ends `Terminal` with the `given_up` outcome; the log holds,
   after the refused line, one `action finished` line of `work.do` with `exit: 0` naming the
   `spent` record and then an `evaluated` line; `replay_tree` over the store reports no difference.
   Failing first: `Stuck`, no record.
8. `a_store_left_stuck_by_0_60_ends_given_up`: the test writes a store and log in the shape 0.60.0
   leaves (that refused run, its refused `action finished` line, no `evaluated` line after it, no
   `spent` record), then runs `run_tree` once: `Terminal`, `given_up`, one `spent` record. Failing
   first: `Stuck`.
9. `the_spent_record_is_written_once_per_case`: with `max_steps: Some(5)` and the outcome removed
   from the protocol (so the case does not end): the log holds exactly one `action finished` line
   with `exit: 0` naming a `spent` record and the case one `spent` record.
10. `no_handoff_check_runs_for_the_spent_record`: as test 7, `work.do` also has a `handoff_when`
    check that touches a marker file and exits 0: the run ends `Terminal` `given_up`, and the
    marker file does not exist.
11. `a_refused_run_below_the_allowance_stays_set_aside` (guard): `max_runs: 3`, a first run refused
    `ResultNotInTable`: no record is written and the writer is not offered again before the case
    changes, as today.

**Gate.** `task check` exits 0; `task slow` passes after the ESS comment change.

## Does not do

- D1 b: count or not count a cancelled run differently. D1 c: write `spent` from the run that
  spends, on each of its early ends.
- D2 b: a new run-log event. D2 c: a record no log line names.
- D3 b: an instant read from the clock.
- D4 b: a check by kind and producer only. D4 c: a `spent` record per subject revision.
- D5 a: bring the owing writer back into the step's choice (and its handoff check and guard).
  D5 b: a `spent` record written beside a `RunsSpent` refusal.
- Change the port's offer path, `RunsSpent`'s text, the set-aside, `due`, `performed_at` or
  `spent_writers_idle`.
- Write `spent` on a full window below `rework_limit` (D10 of the rework story stays).
- Change what a `from: runs` table may list, or a person or open executor.

## Scope

Derived 2026-11-23 from the source at `49499053`. **Cited**: read at the line named.
**Inferred**: a reading that could be wrong. Only what changes is listed as changed.

- `crates/loom-bindings/src/program.rs` — changed: a new exported function beside `Allowance`
  (`:1058-1094`), using the attribution of `:582-612`; `src/lib.rs` exports it (inferred). Read,
  not changed: `:347-473`, `:786-822`.
- `crates/loom-tree/src/run.rs` — changed: the step loop `:304-315` (the call and the line).
  Read, not changed: `:343-345`, `:877-899`, `:955-969`, `:1018`, `:1064-1072`, `:1124-1164`,
  `:1329-1331`.
- `crates/loom-tree/src/replay.rs` (`:10-11`), `src/history.rs` (`:144-169`),
  `crates/loom-executor/src/selection.rs` (`:291-305`) — read, not changed; `replay.rs` changes
  only if an `exit: 0` line with a record but no run needs a case there (inferred: it does not).
- `crates/loom-bindings/tests/bound_program.rs` (`:2627-2665`),
  `crates/loom-tree/tests/runs_spent/mod.rs` (`:57-70`, `:259`) — cited.
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
