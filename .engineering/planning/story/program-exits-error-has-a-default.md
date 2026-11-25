---
format: aep.planning-md/3
id: story:program-exits-error-has-a-default
kind: story
status: draft
title: A bindings document names one exits list that every program binding without its own takes
summary: 'a downstream consumer A11 asks a Loom default exits.error [75, 126, 127]: that is Breaking and reverses bound-program-contract''s no-default rule; D1 a a document-level exits member each program binding without exits takes, exits: {} opts out; D2 b a code its from: exit table lists is left out; expanded at decode; Added'
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:results-row-records-nothing
scope:
- confidence: cited
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/tests/bindings_document.rs
- confidence: cited
  path: crates/loom-bindings/tests/program_contract.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: generated/ess/loom
- confidence: inferred
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/bindings.md
- confidence: cited
  path: website/docs/reference/.generated.json
- confidence: inferred
  path: website/docs/reference/ess/loom-bindings.md
revision: 15
---
## Outcome

A bindings document can name its exit classes once, at the top, and every `program` binding that
names no `exits` of its own takes them. a downstream consumer states `exits: {error: [75, 126, 127]}` on
every action today (its ask A11, its report); with this story it states it once. Loom itself
still ships no default list: a document that names none means what it means today. Cites are at
`03f64d4e` (0.60.0 with the timeout and wildcard units of wave 8 merged).

**Before.**

- A binding's `exits` holds `error` and `not_called`, each a list of exit codes from 0 to 255,
  a list left out empty (`crates/loom-bindings/src/decode.rs:36`, `:340-343`, `:360-402`); the
  document's top-level members are `format`, `protocol`, `program_root`, `artifacts`,
  `independence` and `actions` (`decode.rs:14-21`), and any other is refused `UndeclaredMember`
  (`known`, `:1063-1078`).
- `exits` belongs to a `program` binding alone (`load.rs:307-318`, `MemberNotForExecutor`). A code
  in both lists, or in a list and as a key of a `from: exit` table of the same binding, is
  refused `ExitClassConflict` (`load.rs:386-428`).
- A run whose exit `error` lists records nothing, is not counted, and refuses the step
  `ProgramError`, which ends the run (`program.rs:374-380`, `class_of` `:1498-1511`,
  `program_error` `:1516`).
- **No default list, by decision.** `story:bound-program-contract` chose "Loom ships no default
  list: a binding with no `exits` puts every exit through its result tables", and the ESS says so
  (`ess/domains/bindings.yaml:194-201`), as do the guide (`website/docs/guides/bindings.md:242-248`)
  and `AGENTS.md` (`:1664-1667`). With no `exits`, a program exiting 127 against
  `{"0": valid, "*": invalid}` records `invalid` (test
  `without_exit_classes_every_exit_goes_through_the_table`,
  `crates/loom-bindings/tests/program_contract.rs:810`); that story counted 27 Runbook bindings
  with a `*` row.
- **Served elsewhere already.** A `handoff_when` check that exits 126 or 127 is refused, never an
  answer (`LAUNCH_FAILURE_EXITS`, `crates/loom-bindings/src/handoff.rs:13-16`, `:42-44`, `:192`),
  and an `arguments_from` program that exits other than 0 refuses the step
  (`crates/loom-bindings/src/arguments.rs:91-97`). The ask concerns the bound program's own run.
- **Served in part.** The parser expands YAML aliases (`crates/loom-bindings/src/tree.rs:3-5`;
  test `an_alias_used_within_the_node_bound_decodes`, `tests/bindings_document.rs:1263-1267`), so
  `exits: &errors {error: [75, 126, 127]}` on the first action and `exits: *errors` on the others
  already loads today; the list is written once, the member still on every action.

**Where the ask is wrong.** A default `[75, 126, 127]` that Loom applies to every binding with no
`exits` (D1 b) would make a loaded binding mean something else: an exit 75, 126 or 127 that today
reaches a `*` row and records a result would record nothing and end the run, and a binding whose
`from: exit` table lists `"75"` would be refused `ExitClassConflict`. Both are Breaking and reverse
a recorded decision. 75 is no exit Loom gives a meaning; it is a downstream consumer's.

**After.**

1. **A top-level `exits` (D1 a).** The document may name `exits: {error: [...], not_called:
   [...]}` beside `actions`, decoded and bounded as a binding's (`exits`, `decode.rs:360-402`;
   a refusal names `exits.error[<i>]` with no action).
2. **Who takes it.** Each `program` binding that names no `exits` takes the document's lists at
   decode, so `BoundAction.exits` holds what applies and every reader after decode (`load.rs`,
   `program.rs:929`, `class_of`) is unchanged. A binding with its own `exits` keeps only its own,
   so `exits: {}` opts a binding out. A `person`, `open`, `agent` or `copy` binding takes nothing.
3. **Table keys win (D2 b).** A code the document lists that a binding's `from: exit` table names
   as a key is left out of that binding's copy: the binding's own table is its word for that exit.
   No document that names a top-level `exits` is refused `ExitClassConflict` for it; a binding's
   own `exits` is checked as today.
4. **The model (D3 a).** The top-level member is expanded at decode, as an alias is, and is not a
   field of `loom.bindings.BindingsDocument`: the generated Rust types and every struct literal of
   `BindingsDocument` stay as they are. ESS comments only: `ExitClasses` (`bindings.yaml:194-201`,
   "Loom ships no default list" gains "a document may name one for its program bindings") and
   `BindingsDocument` (`:266-269`).
5. **Docs.** The guide (`:242-248`, `:417-421`) and `AGENTS.md` (`:1664-1667`) say the same, with
   the alias form as the 0.61.0 way. `CHANGELOG.md` `[Unreleased]` `### Added`: "A bindings
   document may name `exits` at the top; each `program` binding that names no `exits` takes it,
   less any code its own `from: exit` table lists as a key. A document without it reads as
   before (`story:program-exits-error-has-a-default`)."

## Decisions

- **D1. Where the default list comes from.** Default **a**.
  - a. The document names it once at the top (After 1-2). Added: a document without it means
    what it means today; Loom names no codes.
  - b. Loom ships `[75, 126, 127]` for every `program` binding with no `exits`: Breaking for a
    binding whose `*` row records those exits today, a load refusal for a table that lists one as
    a key, and the reverse of `story:bound-program-contract`'s decision.
  - c. No change: the YAML alias form (Before) in the guide only. Leaves `exits: *errors` on every
    action, the repetition the ask is about.
  - d. Loom ships `[126, 127]` only (the launcher's codes, as `LAUNCH_FAILURE_EXITS`): Breaking
    for the same reason as b, and a downstream consumer's 75 still on every action.
- **D2. A code the document lists and a binding's `from: exit` table names.** Default **b**.
  - a. Refused `ExitClassConflict`, as if written on the binding: the refusal would name a list
    the binding does not hold, and one table row would refuse a document-wide default.
  - b. Left out of that binding's copy (After 3).
- **D3. How the model holds it.** Default **a**.
  - a. Expanded at decode; ESS comments only (After 4).
  - b. A field `exits: Optional<loom.bindings.ExitClasses>` on `BindingsDocument`: a change of
    the generated Rust struct that every struct literal must name (`decode.rs:153-160` and
    `tests/adversary_w20261121w1_u2.rs:296` here; an embedder's too), for no reader after decode.

## ESS first

`ess/domains/bindings.yaml`: the comments of `ExitClasses` (`:194-201`) and `BindingsDocument`
(`:266-269`); no field changes (D3 a). Then `task generate`, `task suite-generate`
(`SLOT_TIMEOUT: 30m`) and `task docs-generate`, never by hand.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs are POSIX
sh, never Python. A slot job ends within 10 minutes, or names its time with `slot --timeout` (at
most 30 minutes). No test binary is added.

**`crates/loom-bindings/tests/bindings_document.rs`** (decode and load):

1. `a_document_exits_applies_to_each_program_binding_without_its_own`: top-level
   `exits: {error: [75, 126, 127]}`; a `program` binding with no `exits` decodes with `error`
   `[75, 126, 127]`; one with `exits: {not_called: [78]}` keeps `error` empty and `not_called`
   `[78]`; one with `exits: {}` holds empty lists; a `person` binding holds no `exits`. Failing
   first: refused `UndeclaredMember` at `exits`.
2. `a_code_a_table_lists_is_left_out_of_the_document_exits`: the same top-level list and a
   `program` binding with `{kind: …, from: exit, results: {"0": valid, "75": later, "*": invalid}}`:
   it loads, and the binding's `error` is `[126, 127]`. Failing first: refused at decode.
3. `a_document_exits_code_out_of_range_is_refused`: top-level `exits: {error: [256]}` refused
   `Malformed` at `exits.error[0]`, naming no action. Failing first: refused `UndeclaredMember`.

**`crates/loom-bindings/tests/program_contract.rs`** (through the port; `document(head, …)`,
`:128-133`, takes the top-level line as its `head`):

4. `a_document_exits_error_refuses_the_step_program_error`: head `exits: {error: [126, 127,
   75]}`, the binding with no `exits` and `VALID_TABLE`, a stub exiting 127: no record, the step
   refused `ProgramError` naming 127 and the blob digest, the run ended `EffectRefused`, the
   observation's `class` `error` (as `an_error_exit_records_nothing_and_ends_the_run`, `:818`).
   Failing first: refused at decode.
5. `without_exit_classes_every_exit_goes_through_the_table` (`:810`) passes unchanged: a document
   with no top-level `exits` records `invalid` for exit 127. Guard.

**Documents.**

6. Read in the diff: `CHANGELOG.md` `[Unreleased]` `### Added` holds the bullet of After 5 word for
   word; the guide and `AGENTS.md` name the top-level `exits`, who takes it, the table-key rule and
   `exits: {}`; `task docs-check` passes.

**Gate.** `task check` exits 0; `task slow` passes after the ESS comment change.

## Does not do

- D1 b: a default list Loom ships. D1 c: docs only. D1 d: a default of 126 and 127.
- D2 a: refuse a document-wide default for one table key.
- D3 b: a `BindingsDocument` field.
- Change `exits` on a binding, `class_of`, `ProgramError`, the handoff check's launch-failure
  rule or `arguments_from`.
- A top-level default for any other member (`budget`, `timeout_seconds`, `retry_when`).

## Scope

Derived 2026-11-24 from the source at `03f64d4e`. **Cited**: read at the line named.
**Inferred**: a reading that could be wrong.

- `crates/loom-bindings/src/decode.rs` (`:14-21`, `:153-160`, `:340-343`, `:360-402`) — cited, the
  top-level member and its expansion into each `program` binding.
- `ess/domains/bindings.yaml` (`:194-201`, `:266-269`) — cited, comments; `generated/ess/loom`
  and `website/docs/reference/.generated.json` regenerated (cited); `generated/rust/loom` and
  `website/docs/reference/ess/loom-bindings.md` (`:182-188`) regenerated if the comment reaches
  them (inferred).
- Read, not changed — cited: `crates/loom-bindings/src/load.rs` (`:126`, `:307-318`,
  `:386-428`), `src/program.rs` (`:374-380`, `:929`, `:1498-1516`), `src/handoff.rs` (`:13-16`,
  `:42-44`, `:192`), `src/arguments.rs` (`:91-97`), `src/tree.rs` (`:3-5`).
- Tests: `crates/loom-bindings/tests/bindings_document.rs` (`:1263-1267`),
  `tests/program_contract.rs` (`:128-133`, `:806-860`) — cited.
- `website/docs/guides/bindings.md` (`:242-248`, `:417-421`), `AGENTS.md` (`:1664-1667`) — cited;
  `CHANGELOG.md` — one `### Added` bullet under `[Unreleased]` (After 5).

## Edges

- **depends_on `story:results-row-records-nothing`**: both change `ess/domains/bindings.yaml` and
  `decode.rs`; this story merges after it and after the 0.61.0 ESS-changing stories of wave 8
  (timeout, wildcard, program-error, spent).
- **`story:program-error-carries-its-reason`** changes the `ProgramError` path (`program_error`,
  the refusal's text); this story reads it and does not change it. Test 4 asserts the exit code and
  the blob digest only, which that story keeps.
- **ESS and generated files.** One inputs digest (`generated/ess/loom/synthesis.txt:5`): rebased
  on the last ESS-changing story merged before it, it rebuilds `generated/ess/loom`,
  `generated/rust/loom` and the docs stamp (`task generate`, `task suite-generate`,
  `task docs-generate`, then `task check`).
- **Shared files.** `decode.rs` and `tests/bindings_document.rs` with
  `story:results-row-records-nothing` (row decode; other functions); `tests/program_contract.rs`;
  `bindings.md`, `AGENTS.md` and `CHANGELOG.md` with every story of the wave: one bullet each,
  rebased on the one merged before.
