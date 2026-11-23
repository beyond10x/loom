---
format: aep.planning-md/3
id: story:results-row-records-nothing
kind: story
status: draft
title: A program binding's results row can record nothing for its value, without refusing the step
summary: 'Runbook: a failed writer try cannot be recorded under its own kind without the written kind recording too; D1 b a row whose result is false (False, FALSE) records nothing and does not refuse the step; load refuses a false row in from: runs and outside program tables, then an all-false table; D3 a keeps the re-run after another action''s move (pinned by test, idle rule a follow-up); Added in 0.62.0, first story of wave 9'
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:wildcard-result-row-does-not-match-timeout
- depends_on: story:spent-program-has-its-spent-record
- depends_on: story:program-error-carries-its-reason
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
  path: crates/loom-tree/tests/runs_spent/mod.rs
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
- confidence: cited
  path: website/docs/reference/ess/loom-bindings.md
revision: 21
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
  `tree run` stops `Stuck`, exit 4 (the writer's next offer refused `RunsSpent`).
- A kind of its own already works for the rest: the retry records read any kind `retry_when`
  lists (`crates/loom-bindings/src/retry.rs:197-201`), and the `spent` record is written after the
  run's other records (`program.rs:428-442`).

**After.**

1. **A row whose result is `false` records nothing (D1 b).** `results: {"0": written, "*": false}`:
   for a value whose row (or `*` row) is `false`, the entry writes no record on that run and the
   step goes on to the next entry. The run's other entries record as today; a run every entry of
   which records nothing is performed with no record, its moves written (`:501-517`). The quoted
   `"false"` stays a result name. The parser reads `False` and `FALSE` as the same boolean
   (`serde_yaml_ng` 0.10.0, `parse_bool`; Loom's tree takes it through `visit_bool`,
   `crates/loom-bindings/src/tree.rs:79`, `:187-188`), so those record nothing too; `true`,
   `True` and `TRUE` stay refused at decode, with the text of `decode.rs:709-716` naming every
   legal form: "is true; it is a result name, null or false".
2. **The model.** ESS first: `loom.bindings.ResultRule` gains a field (inferred name
   `records: Boolean`, false for such a row) beside `result`; its comment and `EvidenceEntry`'s say
   what a row that records nothing does. Then `task generate`; `decode.rs:730` builds the field.
   `look_up` (`:1367-1373`) answers three ways: no row (refused as today), a row that records
   nothing, a row with its result.
3. **Load checks (D2 a)**, in `load.rs` beside the entry checks (`:138-198`), each refused
   `InvalidEvidence` naming the member, in this order for each entry: the row checks first, then
   the table check, so one document gets one refusal:
   1. a `false` row in a `from: runs` table, at `evidence[i].results.spent`: "a from: runs table
      writes spent; its row cannot be false" (that table lists `spent` alone and exists to write
      it, `crates/loom-bindings/src/load.rs:155-170`);
   2. a `false` row in an entry of a `person`, `open`, `agent` or `copy` executor, at
      `evidence[i].results.<value>`: "only a program's table has a false row" (those entries are
      read elsewhere, `open.rs:480-486`, `crates/loom-tree/src/person.rs:120`, `approve.rs:478`,
      and keep their tables as they are);
   3. a table every row of which is `false`, at `evidence[i].results`: "every row is false, so
      the entry never records; remove it".
   So `{spent: false}` and a one-row `{performed: false}` are refused by their row check, never by
   the table check. A `false` row is a row: `exits` classes refuse it for a classed exit as any
   row (`load.rs:371-405`).
4. **Docs.** `website/docs/guides/bindings.md` (`:207-208`) and `AGENTS.md` (`:1560-1563`) by
   hand (Acceptance 10); `website/docs/reference/ess/loom-bindings.md` (`ResultRule`, `:262-267`)
   and the stamp `website/docs/reference/.generated.json` through `task docs-generate`.
5. **CHANGELOG.** One bullet under `[Unreleased]` `### Added`: "A bound program's `results` row may
   be `false`: for that value the entry records nothing and the step goes on, so a failed try can
   be recorded under a kind of its own (`story:results-row-records-nothing`). A table every row of
   which is `false`, and a `false` row in a `from: runs` table or outside a `program` executor's
   table, are refused at load."

**What Canon reads (checked, unchanged by this story).**

- The case revision moves only with an artifact (`crates/loom-governor/src/lib.rs:1108-1123`,
  `:1865-1879`), never with a record; a kind is open for an action while no record counts for it
  (`lib.rs:1258-1294`), and the selector takes an action not performed at the current revision
  while it has an open kind (`crates/loom-executor/src/selection.rs:291-305`).
- So a kind whose entry recorded nothing stays open, and this story makes that common: after a
  successful run, `<writer>_try_failed` is open. The writer is not taken again at the revision its
  run left (`performed_at`). At every later case revision another action moves, the open kind
  makes it useful again, and it runs: that run counts against `max_runs`, and it is the re-run
  `spent_writers_idle` exists to prevent (`run.rs:1117-1164`). That rule cannot catch it: it idles
  a writer only when its open kinds are all `from: runs` kinds, and `<writer>_try_failed` is not
  one. D3 a keeps this, and test 9 pins the run count; the idle rule for such kinds is a
  follow-up story (Does not do).
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
  open as any kind not recorded; the writer runs again at each later case revision another action
  moves, each run counted against `max_runs` (above, test 9). **b**: `tree run` makes the writer
  idle when its open kinds are only those its latest counted run recorded nothing for, as
  `spent_writers_idle` does for `from: runs` kinds. Not taken here, because no small rule is safe:
  - read from the bindings alone (every kind with a `false` row), it would also idle a writer
    whose subject another action moved, opening its written kind: that kind has a `false` row
    too, and today that writer runs again for it;
  - read per run, the run would have to name the kinds it recorded nothing for (a field of
    `loom.bindings.ProgramRun`), a second ESS change and a selection change.

## ESS first

`ess/domains/bindings.yaml`: the field on `loom.bindings.ResultRule` (`:89-95`) and the comments
of `ResultRule` and `EvidenceEntry` (`:87-88`, `:97-100`). Then `task generate`,
`task suite-generate` (`SLOT_TIMEOUT: 30m`) and `task docs-generate`, never by hand.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs are POSIX
sh, never Python. A slot job ends within 10 minutes, or names its time with `slot --timeout` (at
most 30 minutes). No test binary is added.

**`crates/loom-bindings/tests/bindings_document.rs`** (decode and load). `assert_refused`
(`:76-90`) checks reason, action and member; each refusal test below also asserts the refusal's
text, since 0.60.0 refuses some of the same documents at the same member with
"is false; it is a result name or null" (`decode.rs:709-716`).

1. `a_false_row_decodes_as_recording_nothing`: `results: {"0": written, "*": false}` decodes to
   two rows, the second recording nothing; `False` and `FALSE` decode the same; `{"*": "false"}`
   decodes to the result name `false`. Failing first: refused "is false; it is a result name or
   null".
2. `a_true_row_is_refused_naming_every_form`: `{"*": true}` is refused `InvalidEvidence` at
   `evidence[0].results.*` with "is true; it is a result name, null or false". Failing first: the
   text ends "result name or null".
3. `a_table_false_on_every_row_is_refused`: a program entry `{"0": false, "*": false}`, refused at
   `evidence[0].results` with "every row is false, so the entry never records; remove it". Failing
   first: refused at `evidence[0].results.0` with 0.60.0's text.
4. `a_runs_table_cannot_be_false`: `from: runs, results: {spent: false}`, refused at
   `evidence[0].results.spent` with "a from: runs table writes spent; its row cannot be false"
   (the row check, before the table check). Failing first: same member, 0.60.0's text.
5. `only_a_program_table_has_a_false_row`: a `person` and an `open` entry
   `{kind: …, from: effect, results: {performed: false}}`, and an `agent` and a `copy` entry
   `{… from: exit, results: {"0": ok, "*": false}}`: each refused at its `false` row's member
   (`evidence[0].results.performed`, `evidence[0].results.*`) with "only a program's table has a
   false row", never with the table check's text. Failing first: 0.60.0's text.

**`crates/loom-bindings/tests/bound_program.rs`** (through the port, protocol `fixture@1`,
`spec.check` bound with a `gate` entry `{"0": valid, "*": false}` and a `review` entry
`{"0": false, "*": failed}`, both on `spec`):

6. `a_false_row_writes_no_record_and_performs`: exit 0: one `gate` `valid` record, no `review`,
   step `Recorded`. Then a second step through a second port whose runner answers 1 (as
   `a_cancelled_last_run_writes_no_spent_record` drives its second step, `:2643-2652`), at the same
   revision: one `review` `failed` record and still exactly one `gate` record about that revision
   (Runbook's case b at the record level). Failing first: refused at decode.
7. `a_timed_out_run_needs_its_own_row`: with the wildcard story merged, a run killed at its
   timeout against `gate` `{"0": valid, "*": false}` is refused `ResultNotInTable`; with
   `timeout: false` added it records no `gate` and performs.

**`crates/loom-tree/tests/`**, through `run_tree`, in a new module of the one binary
`tests/main.rs` (inferred name `results_false`). It uses the `runs_spent` module's `Tree`,
`steps_of`, `performed` and `plain` (`runs_spent/mod.rs:149-244`, `:248`, `:319`, `:338-346`),
which `runs_spent` exposes to its sibling module (`pub(super)`), not copies. `Tree::open` already
writes `work.txt`, `note.txt`, `do.sh` and `note.sh` (`:155-170`).

The protocol, `halt@1` shaped as `IDLE_PROTOCOL` (`runs_spent/mod.rs:81-128`): artifacts `work`
and `note`; kinds `done`, `try_failed`, `spent`, `noted`, `checked`; claims `work.done`
(`done` `ok`), `work.try_failed` (`try_failed` `failed`), `work.spent` (`spent` `spent`),
`note.written`, `note.checked`; `work.do` (write) may produce `done`, `try_failed` and `spent`;
`note.write` (write, precondition `{claim: work.done}`) may produce `noted`; `finished` requires
`work.done`, `note.written` and `note.checked` (nothing produces `checked`); `given_up` requires
`work.spent`. The bindings: `work.do` runs `sh do.sh`, `budget: {max_runs: 3}`, with
`{kind: done, …, results: {"0": ok, "*": false, timeout: false}}`,
`{kind: try_failed, …, results: {"0": false, "*": failed, timeout: failed}}` and
`{kind: spent, subject: work, producer: loom, from: runs, results: {spent: spent}}`; `note.write`
as in `IDLE_BINDINGS`.

8. `a_writer_that_fails_after_writing_ends_given_up` (Runbook's cycle, and its case a): with
   `retry_when: {try_failed: [failed]}`; `do.sh` is
   `grep -q first work.txt || echo first >> work.txt` then `exit 1`. `tree.run()`:
   - `performed(&lines, "work.do")` is 3: the first run moves `work` to R, the second and third
     name no move and are taken at R only because the first run's `try_failed` `failed` record is
     a retry record (`due_actions`, `run.rs:955-969`): a run whose `done` row is `false` counts
     for `retry_when` and for the run budget;
   - the case holds three `try_failed` `failed` records about R, no `done` record, and one
     `spent` record, written by the third run (the run that reached `max_runs`);
   - the root's decision document reads `work.try_failed` `true`, `work.spent` `true` and
     `work.done` `unknown`;
   - the run ends `Terminal` with `given_up`, not `Stuck` (exit 4, Runbook's report on the
     0.60.0 tables).
   Failing first: `Tree::open` panics, the bindings refused at load ("is false; …").
9. `a_successful_writer_runs_again_after_another_actions_move` (D3 a pinned, and Runbook's
   case b): no `retry_when`; `do.sh` is `grep -q first work.txt && exit 1`, then
   `echo first >> work.txt` and `exit 0`. `tree.run()`:
   - the steps are `work.do`, `note.write`, `work.do`, in that order, and `performed` of `work.do`
     is 2: the second run is taken only because `try_failed` is open after the first, at the case
     revision `note.write`'s move made, and it counts against `max_runs` (2 of 3);
   - the first run records `done` `ok` about R and no `try_failed`; the second changes nothing,
     exits 1 and records `try_failed` `failed` about R and no `done`: the case holds exactly one
     `done` record about R, and the document reads `work.done` `true` (no disagreeing record at
     that revision) and `work.try_failed` `true`;
   - the run stops `Stuck`: `finished` waits for `note.checked`, and the writer's only open kind
     is `spent` (`spent_writers_idle`).
   Failing first: `Tree::open` panics. The follow-up idle story would make the run count 1 and
   changes this test on purpose.

**Documents.**

10. Read in the diff: `CHANGELOG.md` `[Unreleased]` `### Added` holds the bullet of After 5 word for
    word; `website/docs/guides/bindings.md` (`:207-208`) and `AGENTS.md` (`:1560-1563`) each say
    that a `false` row records nothing for its value and does not refuse the step, and name the
    three load refusals; `task docs-check` passes, `website/docs/reference/ess/loom-bindings.md`
    showing the new `ResultRule` field.

**Gate.** `task check` exits 0; `task slow` passes after the ESS change.

## Does not do

- D1 a: the reserved name `skip`. D1 c: a per-entry condition. D1 d: `null` records nothing.
- D2 b: `false` rows in `open`, `person`, `agent` or `copy` tables.
- D3 b: an idle rule for kinds a run recorded nothing for. A follow-up story: it needs each run to
  name the kinds it recorded nothing for (a `loom.bindings.ProgramRun` field, an ESS change) and
  a change to `spent_writers_idle`, and a rule read from the bindings alone would idle a writer
  whose subject another action moved. Until then a writer with a `false` row runs again at each
  later case revision another action moves (test 9).
- Change what `null` does, or the `ResultNotInTable` refusal of a value with no row.
- Clear a failed try's record when a later run at the same revision succeeds.
- Change a `from: runs` table or the `spent` record.

## Scope

Derived 2026-11-23 from the source at `49499053`. **Cited**: read at the line named.
**Inferred**: a reading that could be wrong.

- `ess/domains/bindings.yaml` (`:87-117`) — cited; `generated/ess/loom`, `generated/rust/loom`
  (`src/bindings.rs:482-489`, `ResultRule`), `website/docs/reference/ess/loom-bindings.md`
  (`:262-267`) and `website/docs/reference/.generated.json` regenerated.
- `crates/loom-bindings/src/decode.rs` (`:691-740`) — cited, the row decode and the `true` text;
  `src/tree.rs` (`:79`, `:187-188`) read, not changed.
- `crates/loom-bindings/src/program.rs` (`:443-500`, `look_up` `:1367-1373`) — cited.
- `crates/loom-bindings/src/load.rs` (`:138-198`, `:371-405`) — cited, the checks.
- Read, not changed — cited: `crates/loom-bindings/src/retry.rs` (`:197-201`), `src/open.rs`
  (`:480-486`); `crates/loom-tree/src/person.rs` (`:120`), `src/approve.rs` (`:478`),
  `src/run.rs` (`:955-969`, `:1117-1164`); `crates/loom-governor/src/lib.rs` (`:1108-1123`,
  `:1258-1294`, `:1865-1879`); `crates/loom-executor/src/selection.rs` (`:291-305`).
- Tests: `crates/loom-bindings/tests/bindings_document.rs` (`:76-90`), `tests/bound_program.rs`
  (`:2643-2652`), `crates/loom-tree/tests/runs_spent/mod.rs` (`:81-128`, `:149-244`, `:248`,
  `:319`, `:338-346`; helpers made `pub(super)`) — cited; `crates/loom-tree/tests/main.rs` and the
  new module — inferred.
- `website/docs/guides/bindings.md` (`:207-208`), `AGENTS.md` (`:1560-1563`) — cited;
  `CHANGELOG.md` — one `### Added` bullet under `[Unreleased]` (After 5).

## Edges

- **depends_on `story:wildcard-result-row-does-not-match-timeout`**,
  **`story:program-error-carries-its-reason`** and **`story:spent-program-has-its-spent-record`**:
  each changes `ess/domains/bindings.yaml`, and the wildcard and spent stories change `program.rs`
  and the rules this story's tables and tests read (a `timeout` row; the owed `spent` record). It
  merges after the four ESS-changing stories of wave 8 (timeout, wildcard, program-error, spent),
  as the first story of wave 9, in 0.62.0.
- **ESS and generated files.** One inputs digest (`generated/ess/loom/synthesis.txt:5`): rebased
  on the last of them, it rebuilds `generated/ess/loom`, `generated/rust/loom` and the docs stamp
  (`task generate`, `task suite-generate`, `task docs-generate`, then `task check`).
- **Shared files.** `program.rs` (`look_up`, the entry loop) and `tests/bound_program.rs` with the
  wildcard story; `crates/loom-tree/tests/runs_spent/mod.rs` with the spent story (its tests 11-15
  add to that module); `bindings.md`, `AGENTS.md` and `CHANGELOG.md` with all of wave 8. One
  bullet each, rebased on the one merged before.
