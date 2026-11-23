---
format: aep.planning-md/3
id: story:results-row-records-nothing
kind: story
status: draft
title: A program binding's results row can record nothing for its value, without refusing the step
summary: 'Runbook: a failed writer try cannot be recorded under its own kind without the written kind recording too, so Canon reads unknown and the tree stops RunsSpent; D1 b a row whose result is false records nothing (skip is a legal result name today, so it would be Breaking); load refuses an all-false table, false in from: runs and outside program tables; Added in 0.62.0, first story of wave 9'
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:wildcard-result-row-does-not-match-timeout
- depends_on: story:spent-program-has-its-spent-record
scope:
- confidence: cited
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/tests/bindings_document.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_program.rs
- confidence: inferred
  path: crates/loom-tree/tests/main.rs
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

A `results` row of a bound program's evidence entry can say "record nothing" for its value: the
entry writes no record on that run, and the step is not refused. A writer can then record a failed
try under a kind of its own (`<writer>_try_failed`) without its written kind also recording on the
same run, and Canon reads one result per kind at the revision the run left. Asked by Runbook (its
unit `bugfix-writer-is-sent-back-on-loom`, adversary pass 2, three reproduced cycles). The first
story of wave 9 (0.62.0). Cites are at `49499053` (tag 0.60.0); Runbook read Loom at `c9b4c177`
(`0.59.0-516`, before the tag), so its line numbers differ from these.

**Before.**

- An entry has no condition: its members are `kind`, `subject`, `producer`, `from`, `results` and
  `upstream` (`crates/loom-bindings/src/decode.rs:638-764`, ESS `loom.bindings.EvidenceEntry`,
  `ess/domains/bindings.yaml:97-117`).
- On a counted run every entry is looked up and records
  (`crates/loom-bindings/src/program.rs:438-500`, the loop at `:443`): a value with no row and no
  `*` row ends the step `ResultNotInTable`, and nothing of the run is recorded (`:462-473`;
  `look_up` `:1367-1373`). A `null` row records a record of the kind with no result
  (`decode.rs:707`; guide `website/docs/guides/bindings.md:207-208`; ESS comment
  `bindings.yaml:87-88`).
- A row's result is any non-empty string or `null`; any other YAML value is refused at decode
  ("is …; it is a result name or null", `decode.rs:705-716`). So `skip` is a legal result name
  today, and a boolean `false` is not.
- So the written kind records on a failed run too (as a result or as a result-less record), and
  Runbook's cycle (its report, not reproduced here): the writer fails after writing its file, the
  sent-back run changes nothing, both runs record about revision R, and Canon reads the claim as
  `unknown`; after the third run the `spent` record is written but no failure outcome holds, and
  `tree run` stops `RunsSpent`, exit 4.
- A kind of its own already works for the rest: the retry records read any kind `retry_when`
  lists (`crates/loom-bindings/src/retry.rs:197-201`), and the `spent` record is written after the
  run's other records (`program.rs:428-442`).

**After.**

1. **A row whose result is `false` records nothing (D1 b).** `results: {"0": written, "*": false}`:
   for a value whose row (or `*` row) is `false`, the entry writes no record on that run and the
   step goes on to the next entry. The run's other entries record as today; a run every entry of
   which records nothing is performed with no record, its moves written (`:501-517`). The quoted
   `"false"` stays a result name.
2. **The model.** ESS first: `loom.bindings.ResultRule` gains a field (inferred name
   `records: Boolean`, false for such a row) beside `result`; its comment and `EvidenceEntry`'s say
   what a row that records nothing does. Then `task generate`; `decode.rs:730` builds the field.
   `look_up` (`:1367-1373`) answers three ways: no row (refused as today), a row that records
   nothing, a row with its result.
3. **Load checks (D2 a)**, each refused `InvalidEvidence` naming the member:
   - a table every row of which records nothing (`evidence[i].results`): the entry never records;
     remove it;
   - a row that records nothing in a `from: runs` table (`evidence[i].results.spent`): that table
     lists `spent` alone and exists to write it (`crates/loom-bindings/src/load.rs:155-170`);
   - such a row in an entry of a `person`, `open`, `agent` or `copy` executor
     (`evidence[i].results.<value>`): those entries are read elsewhere (`open.rs:480-486`,
     `crates/loom-tree/src/person.rs:120`, `approve.rs:478`) and keep their tables as they are.
   A row that records nothing is a row: `exits` classes refuse it for a classed exit as any row
   (`load.rs:371-405`).
4. **Docs.** `website/docs/guides/bindings.md` (`:207-208`), `AGENTS.md` (`:1560-1563`), and the
   reference stamp `website/docs/reference/.generated.json` through `task docs-generate`.

**What Canon reads (checked, unchanged by this story).**

- The case revision moves only with an artifact (`crates/loom-governor/src/lib.rs:1108-1123`,
  `:1865-1879`), never with a record; a kind is open for an action while no record counts for it
  (`lib.rs:1258-1294`), and the selector takes an action not performed at the current revision
  while it has an open kind (`crates/loom-executor/src/selection.rs:291-305`).
- So a kind whose entry recorded nothing stays open. The writer is not taken again at the revision
  its run left (`performed_at`), but at a later case revision, moved by another action, the open
  `<writer>_try_failed` kind makes it useful again, and that run counts against `max_runs`. That is
  the same as an action with any kind it did not record today; D3 keeps it.
- A run that succeeds at the revision an earlier failed try recorded leaves that failed record
  about the current revision: the writer stays due (`retry.rs:197-201`, `due_actions`
  `crates/loom-tree/src/run.rs:955-969`) until the subject moves. Inferred from those lines; not
  changed here. Runbook's cycle (every try fails) does not reach it.

**With the wave-8 stories.**

- `story:wildcard-result-row-does-not-match-timeout`: `*` does not match a run killed at its
  timeout. Every table of a binding whose program can time out needs a `timeout` row, else that
  run is refused `ResultNotInTable` and records nothing. Runbook's example therefore reads
  `<writer>_written: {"0": written, "*": false, timeout: false}` beside
  `<writer>_try_failed: {"0": false, "*": failed, timeout: failed}`.
- `story:spent-program-has-its-spent-record`: its owed-record check reads the `from: runs` entry's
  kind only, and a `from: runs` table cannot hold a row that records nothing (After 3), so an entry
  that recorded nothing is never read as a missing `spent` record.

## Decisions

- **D1. How a row says "record nothing".** Default **b**.
  - a. A reserved result name `skip`, as Runbook asked: `skip` is a legal result today
    (`decode.rs:705-716`), so a loaded table that records `skip` would record nothing after the
    upgrade: Breaking, and the name is lost as a result.
  - b. The YAML boolean `false` as the row's result: refused at decode today, so no table that
    loads today changes meaning; `CHANGELOG.md` `### Added`.
  - c. A per-entry condition (for example `when: [0]`): a seventh entry member and a second place
    that decides whether an entry records, beside the table that already maps every value.
  - d. `null` records nothing: Breaking, since `null` records a result-less record today (guide
    `:208`), and a table that relies on it to close its kind would leave the kind open.
- **D2. Where such a row is allowed.** Default **a**: a `program` executor's `exit` and report
  tables only, refused elsewhere (After 3). **b**: every table but `from: runs`: changes the
  `open`, `person` and `approve` paths for no consumer ask.
- **D3. How a kind that recorded nothing reads in the action's facts.** Default **a**: unchanged,
  open as any kind not recorded (above). **b**: `tree run` makes the writer idle when its open
  kinds are only those its latest counted run recorded nothing for, as `spent_writers_idle` does for
  `from: runs` kinds (`run.rs:1124-1164`): the run would have to name the kinds it skipped (a field
  of `loom.bindings.ProgramRun`), a second ESS change and a selection change no consumer has
  reported yet.

## ESS first

`ess/domains/bindings.yaml`: the field on `loom.bindings.ResultRule` (`:89-95`) and the comments
of `ResultRule` and `EvidenceEntry` (`:87-88`, `:97-100`). Then `task generate`,
`task suite-generate` (`SLOT_TIMEOUT: 30m`) and `task docs-generate`, never by hand.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs are POSIX
sh, never Python. A slot job ends within 10 minutes, or names its time with `slot --timeout` (at
most 30 minutes). No test binary is added.

**`crates/loom-bindings/tests/bindings_document.rs`** (decode and load):

1. `a_false_row_decodes_as_recording_nothing`: `results: {"0": written, "*": false}` decodes to
   two rows, the second recording nothing; `{"*": "false"}` decodes to the result name `false`.
   Failing first on 0.60.0: refused `InvalidEvidence`, "is false; it is a result name or null".
2. `a_table_that_records_nothing_on_every_row_is_refused`: `{"0": false, "*": false}` refused
   `InvalidEvidence` at `evidence[0].results`.
3. `a_runs_table_cannot_record_nothing`: `from: runs, results: {spent: false}` refused at
   `evidence[0].results.spent`.
4. `only_a_program_table_records_nothing`: an `open`, `person`, `agent` and `copy` entry with a
   `false` row: each refused at `evidence[0].results.<value>`.

**`crates/loom-bindings/tests/bound_program.rs`** (through the port, protocol `fixture@1`,
`spec.check` bound with `entry("gate", "exit", …)` and a second entry of kind `review`):

5. `a_row_that_records_nothing_writes_no_record_and_performs`: `gate` `{"0": valid, "*": false}`,
   `review` `{"0": false, "*": failed}`; exit 0: one `gate` `valid` record, no `review`, step
   `Recorded`; exit 1: one `review` `failed` record, no `gate`. Failing first: refused at decode.
6. `a_timed_out_run_needs_its_own_row`: with the wildcard story merged, a run killed at its
   timeout against `{"0": valid, "*": false}` is refused `ResultNotInTable`; with
   `timeout: false` added it records nothing for that entry and performs.

**`crates/loom-tree/tests/`** (one new module of the existing binary `tests/main.rs`, through
`run_tree`; inferred name `results_skip`): Runbook's cycle. A protocol with a writer `work.write`
that may produce `work_written` and `work_try_failed`, outcomes `finished` requiring
`work_written` and `given_up` requiring `work_try_failed` `failed` and the `spent` claim; the
writer bound `budget: {max_runs: 3}`, `retry_when: {work_try_failed: [failed]}`,
`work_written: {"0": written, "*": false, timeout: false}`,
`work_try_failed: {"0": false, "*": failed, timeout: failed}` and a `from: runs` entry.

7. `a_writer_that_fails_after_writing_ends_given_up`: the program writes `work.txt` on its first
   run, and every run exits 1. Three runs: the first moves `work` to R, the other two change
   nothing; the case holds three `work_try_failed` `failed` records about R and no `work_written`
   record; Canon's decision reads the `work_try_failed` claim `failed`, not `unknown`; the run ends
   `Terminal` `given_up`, not `RunsSpent` exit 4. Failing first: the bindings are refused at load.
8. `a_writer_that_succeeds_records_only_its_written_kind`: the program writes `work.txt` and exits
   0: one `work_written` `written` record, no `work_try_failed`; `Terminal` `finished` after one
   run.

**Gate.** `task check` exits 0; `task slow` passes after the ESS change.

## Does not do

- D1 a: the reserved name `skip`. D1 c: a per-entry condition. D1 d: `null` records nothing.
- D2 b: rows that record nothing in `open`, `person`, `agent` or `copy` tables.
- D3 b: an idle rule for kinds a run recorded nothing for.
- Change what `null` does, or the `ResultNotInTable` refusal of a value with no row.
- Clear a failed try's record when a later run at the same revision succeeds.
- Change a `from: runs` table or the `spent` record.

## Scope

Derived 2026-11-23 from the source at `49499053`. **Cited**: read at the line named.
**Inferred**: a reading that could be wrong.

- `ess/domains/bindings.yaml` (`:87-117`) — cited; `generated/ess/loom`, `generated/rust/loom`
  (`src/bindings.rs:482-489`, `ResultRule`) and `website/docs/reference/.generated.json`
  regenerated.
- `crates/loom-bindings/src/decode.rs` (`:691-740`) — cited, the row decode.
- `crates/loom-bindings/src/program.rs` (`:443-500`, `look_up` `:1367-1373`) — cited.
- `crates/loom-bindings/src/load.rs` (`:138-198`, `:371-405`) — cited, the checks.
- Read, not changed — cited: `crates/loom-bindings/src/retry.rs` (`:197-201`), `src/open.rs`
  (`:480-486`); `crates/loom-tree/src/person.rs` (`:120`), `src/approve.rs` (`:478`),
  `src/run.rs` (`:955-969`, `:1124-1164`); `crates/loom-governor/src/lib.rs` (`:1108-1123`,
  `:1258-1294`, `:1865-1879`); `crates/loom-executor/src/selection.rs` (`:291-305`).
- Tests: `crates/loom-bindings/tests/bindings_document.rs`, `tests/bound_program.rs` — cited
  files; `crates/loom-tree/tests/main.rs` and a new module under `crates/loom-tree/tests/` —
  inferred.
- `website/docs/guides/bindings.md` (`:207-208`), `AGENTS.md` (`:1560-1563`) — cited;
  `CHANGELOG.md` — one `### Added` bullet under `[Unreleased]`.

## Edges

- **depends_on `story:wildcard-result-row-does-not-match-timeout`** and
  **`story:spent-program-has-its-spent-record`**: both change `ess/domains/bindings.yaml` and
  `program.rs`, and this story's tables and tests read their rules (a `timeout` row; the owed
  `spent` record). It merges after the four ESS-changing stories of wave 8 (timeout, wildcard,
  program-error, spent), as the first story of wave 9, in 0.62.0.
- **ESS and generated files.** One inputs digest (`generated/ess/loom/synthesis.txt:5`): rebased
  on the last of them, it rebuilds `generated/ess/loom`, `generated/rust/loom` and the docs stamp
  (`task generate`, `task suite-generate`, `task docs-generate`, then `task check`).
- **Shared files.** `program.rs` (`look_up`, the entry loop) and `tests/bound_program.rs` with the
  wildcard story; `bindings.md`, `AGENTS.md` and `CHANGELOG.md` with all of wave 8. One bullet
  each, rebased on the one merged before.
