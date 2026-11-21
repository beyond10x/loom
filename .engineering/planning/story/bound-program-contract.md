---
format: aep.planning-md/3
id: story:bound-program-contract
kind: story
status: draft
title: A bound program can be named beside its bindings file, sees its run's timeout in its environment, and an exit its binding classes as a program error or as not called records nothing and is not counted
summary: program_root bindings and resolve_programs; LOOM_TIMEOUT_SECONDS beside LOOM_CASE; exits {error, not_called}, no default list; ProgramError ends the run
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:bound-program-case-id
- depends_on: story:bound-program-retry-records
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-bindings/src/child.rs
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: inferred
  path: crates/loom-bindings/src/lib.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: inferred
  path: crates/loom-bindings/tests/program_contract.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: inferred
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/bindings.md
revision: 5
---
## Outcome

A `program` binding of `loom.bindings/1` gains three things Runbook's driver gives its tools
(`~/beyond10x/sdc/driver/src/main.rs`, `D` below) and Loom does not. All three are in
`b10x-loom-bindings`; the stop a tree gives for a not-called writer is
`story:writer-not-called-stop`'s.

- **Programs beside the bindings file.** The document member `program_root` is `workspace` (the
  default, today's meaning) or `bindings`. With `bindings`, every `argv[0]` and
  `arguments_from[0]` that is relative and holds a `/` names a file relative to the directory of
  the bindings file, as an `open` executor's `bindings` member does
  (`crates/loom-bindings/src/load.rs:202-212`). A bare name is still found on `PATH`, and an
  absolute path runs as written. The new public function
  `loom_bindings::resolve_programs(document, directory)` returns the document with each such entry
  replaced by its absolute path under the canonical `directory`, and refuses
  `ProgramNotBesideBindings`, naming the action and the member, an entry with a `..` segment, one
  that names no regular file, and one whose real path (links resolved) lies outside `directory`.
  Under `linux` the resolved program is mounted read-only at its own path, as every absolute
  program outside the system view is (`crates/loom-intake-slice/src/bindings.rs:440-446`).
- **The run's timeout in its environment.** `loom.bindings.ProgramEnvironment`
  (`story:bound-program-case-id`: `path`, `loom_case`) gains `loom_timeout_seconds`: a bound
  program and its `arguments_from` program start with `PATH`, `LOOM_CASE` and
  `LOOM_TIMEOUT_SECONDS`, the whole seconds of that run's timeout (the binding's
  `timeout_seconds`, else 60, `crates/loom-bindings/src/program.rs:188`). `bound_case_child`
  sets it as it sets `LOOM_CASE`, after every other variable, so nothing else gives it another
  value. `LOOM_RETRY_FEEDBACK` stays as `story:bound-program-retry-records` sets it.
- **Exit classes.** A program binding may name `exits: {error: [...], not_called: [...]}`, each a
  list of exit codes from 0 to 255. Loom ships no default list: a binding with no `exits` puts
  every exit through its result tables, as today (`program.rs:270-291`). When `exits` is given,
  both lists are read before the result tables.
  - A run whose exit is in `error` is a program error: it records no evidence, the step is
    refused `ProgramError` with `ends_run` set, so Commission ends the run `EffectRefused`
    (`ess/commission/domains/responsibility.yaml:364`), and the run does not count against
    `budget.max_runs`.
  - A run whose exit is in `not_called` did nothing: it records no evidence, the step is
    performed, and the run does not count against `budget.max_runs`.
  - A run killed at its timeout reads the key `timeout` as today (`program.rs:470-472`), whatever
    the lists hold.
  - The run's observation, `loom.bindings.ProgramRun`, kept for every run
    (`program.rs:219-256`), gains `class` (`error`, `not_called`, absent otherwise), and the run
    count (`program.rs:134-148`) skips both classes.

The bindings document is the operator's: `program_root` and `exits` come from it alone, and no
request, report or output sets `LOOM_TIMEOUT_SECONDS`. A program chooses its exit code and so
classes its own run; a class only withholds the run's evidence and its count, and an error ends
the run. Nothing the program printed enters a refusal or the observation: they name the exit code
and the blob digest.

## Why

- `{dir}` is replaced by the bindings file's directory at `init` in every `argv`, `args_from` and
  child path (`D:885-890`); 58 bindings in 9 files start a tool under `{dir}/tools/`
  (`~/beyond10x/sdc/driver/examples/sdc/sdc-intake.bindings.json:24-27` and the like). Loom's
  `argv` is literal and starts at the workspace root (`bindings.rs:414-449`), and a relative
  `argv[0]` is a workspace path (`website/docs/guides/bindings.md:77`). Runbook's tools are not in
  the workspace, and Runbook commits no absolute home path (`~/beyond10x/sdc/AGENTS.md`, Rules).
- The driver gives each tool `SDC_TIMEOUT_S` (`D:1514`); Runbook's replay adapter reads it to time
  a recorded call out on its virtual clock (`~/beyond10x/sdc/driver/src/bin/sdc-replay.rs:1113`,
  `:1125`). Loom gives `PATH` and `LOOM_CASE` (`story:bound-program-case-id`).
- Exits 126, 127 and 75 are program errors in the driver: no evidence, not counted, the run stops
  (`D:576-581`, `:682`, `:1550`, `:1619-1629`). Loom puts every exit through the table, `*`
  included (`program.rs:270-291`), and 27 Runbook bindings have a `*` row, so a missing program
  becomes an `invalid` record about the subject. `story:run-caps-per-arrival` puts exit 126 and
  127 out of its scope.
- A tool that did nothing writes `SDC_NOT_CALLED`; the run is recorded `not_called` and not
  counted (`D:1497-1501`, `:1537-1540`, `:776`; `driver/examples/sdc/tools/llm-node.sh:34-36`). A
  file the program writes is not used here: under `linux` the workspace is read-only outside
  `target/` and the paths `story:bound-program-declares-its-environment` declares writable.

## ESS first

`ess/domains/bindings.yaml` first: `ProgramRoot` (`Workspace`, `Bindings`) and
`BindingsDocument.program_root: Optional<ProgramRoot>`; `ExitClasses` (`error`, `not_called`, each
`List<Integer>`) and `BoundAction.exits: Optional<ExitClasses>`; `ExitClass` (`Error`,
`NotCalled`) and `ProgramRun.class: Optional<ExitClass>`; `ProgramEnvironment` gains
`loom_timeout_seconds: Integer`; `BindingsRefusalReason` gains `ExitClassConflict` and
`ProgramNotBesideBindings`; `ProgramRefusalReason` gains `ProgramError`. On that commit
`task drift` fails, and `crates/loom-bindings/tests/program_contract.rs` does not compile.

## Acceptance

`crates/loom-bindings/tests/program_contract.rs` drives the program effect port as
`crates/loom-bindings/tests/bound_program.rs` does: Commission's `run_until_blocked`, a scripted
executor, a `CanonGovernor` over a `MemoryCaseStore`, a fixture protocol of two artifacts, stub
programs the test writes under `CARGO_TARGET_TMPDIR`, a `local` runner opened with
`UNCONFINED_FLAG_REASON`; no model, no network. The bindings file sits in directory `A`, the
workspace is directory `B`, and the process's working directory is a third one.

- With `program_root: bindings` and `argv: [bin/stub]`, the stub at `A/bin/stub` runs and its
  output is the run's blob (test `a_program_beside_the_bindings_file_runs`).
- With no `program_root`, `argv: [bin/stub]` runs `B/bin/stub` and not `A/bin/stub` (test
  `a_relative_program_is_a_workspace_path_by_default`).
- With `program_root: bindings`, `arguments_from: [bin/args]` runs `A/bin/args` and the bound
  program reads its object (test `an_arguments_program_beside_the_bindings_file_runs`).
- `argv: [../bin/stub]` is refused `ProgramNotBesideBindings` naming the action and
  `executor.argv[0]` (test `a_program_path_leaving_the_bindings_directory_is_refused`).
- `A/bin/stub` as a link to a file outside `A` is refused `ProgramNotBesideBindings` (test
  `a_linked_program_outside_the_bindings_directory_is_refused`).
- `argv: [bin/absent]` is refused `ProgramNotBesideBindings` before anything starts (test
  `a_missing_program_beside_the_bindings_file_is_refused`).
- `program_root: elsewhere` is refused `Malformed` naming `program_root` (test
  `an_unknown_program_root_is_refused`).
- A stub that prints its environment shows exactly `PATH`, `LOOM_CASE` and
  `LOOM_TIMEOUT_SECONDS=7` for a binding with `timeout_seconds: 7` (test
  `a_program_starts_with_its_timeout_in_its_environment`).
- With no `timeout_seconds`, the stub shows `LOOM_TIMEOUT_SECONDS=60` (test
  `a_program_without_a_timeout_reads_the_default`).
- An `arguments_from` stub that prints its environment shows the same three variables (test
  `an_arguments_program_starts_with_its_timeout_in_its_environment`).
- Security: a request whose arguments hold `{"LOOM_TIMEOUT_SECONDS": "3600"}` leaves the variable
  at `7`, and the arguments reach standard input as written (test
  `an_argument_named_loom_timeout_seconds_does_not_change_the_variable`).
- With no `exits` and a table `{"0": valid, "*": invalid}`, a stub exiting 127 records `invalid`
  (test `without_exit_classes_every_exit_goes_through_the_table`).
- With `exits: {error: [126, 127, 75]}` and the same table, a stub exiting 127 records no
  evidence, the step is refused `ProgramError` naming exit 127 and the blob digest, and the loop
  ends `EffectRefused` (test `an_error_exit_records_nothing_and_ends_the_run`).
- With `budget: {max_runs: 2}`, three runs exiting 127 and then one exiting 0: the fourth is
  performed and records `valid` (test `an_error_exit_does_not_count_against_max_runs`).
- With `exits: {not_called: [78]}`, a stub exiting 78 is performed, records no evidence, and its
  `ProgramRun` has `class: not_called` (test `a_not_called_exit_records_nothing`).
- With `budget: {max_runs: 1}`, a run exiting 78 and then one exiting 0: the second is performed
  (test `a_not_called_run_does_not_count_against_max_runs`).
- With `timeout_seconds: 1`, a table with a `timeout` row and `exits: {error: [0, 137, 143]}`, a
  stub that sleeps 5 s records the `timeout` row's result (test
  `a_timeout_is_never_an_exit_class`).
- `exits: {error: [256]}`, `[-1]` and `["x"]` are each refused `Malformed` naming
  `exits.error[0]` (test `an_exit_code_outside_0_to_255_is_refused`).
- An exit listed in both classes is refused `ExitClassConflict` naming the action and
  `exits.not_called[0]` (test `an_exit_in_both_classes_is_refused`).
- An exit listed in `exits.error` and as a key of a `from: exit` table of the same binding is
  refused `ExitClassConflict` naming the table's row (test
  `an_exit_class_and_a_table_row_conflict`).
- `exits` on a `person`, `agent`, `open` or `copy` executor is refused `MemberNotForExecutor`
  naming `exits` (test `exit_classes_belong_to_a_program`).
- A stub that prints a canary string and exits 127 leaves no canary in the refusal's text or in
  the `ProgramRun` observation (test `program_output_never_enters_a_refusal`).
- No test of the file runs 60 s; only `a_timeout_is_never_an_exit_class` waits, about 1 s, and
  `slot cargo test -p b10x-loom-bindings --test program_contract` passes.
- `task check` exits 0.

## Out of scope

The `WriterNotCalled` stop, the due rule for a not-called writer and `tree open` calling
`resolve_programs` (`story:writer-not-called-stop`). Naming an exit code that is in neither
`exits` nor a result table: the bindings-check story's (`b10x-loom bindings check`, not yet in
the store). A store or case directory in the environment: a program learns facts about the case
from its inputs, an `arguments_from` program or `tree status`. Mounting a program's sibling files
under `linux` (`read_only_roots` of `story:bound-program-declares-its-environment`). Exit classes
of an `agent` executor (`story:bound-agent-node`). Counting runs since an arrival
(`story:run-caps-per-arrival`). The action id in the environment: each binding's `argv` names it
literally. `{dir}` in a child's protocol path: the child's protocol is resolved in the host's
catalog (`story:child-case-runtime` § Outcome).

## Scope

Derived 2026-11-21 by the drafting agent. Every line is **cited** (read in the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings` (`program_root`, `resolve_programs`, `exits`, the
  run count, the timeout variable) — cited (`program.rs:126-148`, `:184-291`)
- **Files:** `ess/domains/bindings.yaml` (`BindingsDocument` :190-202, `BoundAction` :163-185,
  `ProgramRun` :258-272, `ProgramRefusalReason` :313-320; `ProgramEnvironment` from
  `story:bound-program-case-id`) — cited
- **Files:** `crates/loom-bindings/src/decode.rs` (`TOP` :14, `ACTION` :15-26, `bound_action`
  :192-295) — cited
- **Files:** `crates/loom-bindings/src/load.rs` (`check` :30-215, `fit` :228-284) — cited
- **Files:** `crates/loom-bindings/src/program.rs` (`runs` :134-148, `perform` :150-331,
  `arguments` :335-398, `refused` :603-608) — cited
- **Files:** `crates/loom-bindings/src/child.rs` (`bound_case_child`, from
  `story:bound-program-case-id`) — inferred, that story guesses the name
- **Files:** `crates/loom-bindings/src/lib.rs` (exports `resolve_programs`) — inferred
- **Not changed:** `crates/loom-intake-slice/src/bindings.rs`: `bound_case_child` adds the
  variables over `bound_program_child` (`story:bound-program-case-id` § Outcome) — cited
- **Files:** `crates/loom-bindings/tests/program_contract.rs` — inferred, name proposed here
- **Documents:** `website/docs/guides/bindings.md` (a relative `argv[0]` :77, the start of a
  program :216-218), `AGENTS.md` (§ Bindings document), `CHANGELOG.md` — cited
- **Symbols:** `resolve_programs`, `ProgramRoot`, `ExitClasses`, `ExitClass`, `ProgramRun.class`,
  `ProgramEnvironment.loom_timeout_seconds`, `ProgramNotBesideBindings`, `ExitClassConflict`,
  `ProgramError`, `LOOM_TIMEOUT_SECONDS` (new); `ProgramEffects::runs`, `bound_case_child`
  (changed) — cited / inferred
- **Would collide with:** every story on `ess/domains/bindings.yaml`, `decode.rs`, `load.rs` or
  `program.rs` (`story:bound-artifact-revisions`, `story:bound-program-retry-records`,
  `story:bound-program-case-id`, `story:run-caps-per-arrival`,
  `story:bound-program-declares-its-environment`, `story:person-actions`,
  `story:handoff-when`, `story:initial-child-revisions`); `story:bound-program-case-id` and
  `story:bound-program-retry-records` also on `ProgramEnvironment` and `bound_case_child`;
  `story:gate-program-identity-bound-to-spawn`, which reads a relative `argv[0]` with a `/` as a
  workspace path (its § Outcome) — cited
