---
format: aep.planning-md/3
id: story:bound-program-contract
kind: story
status: draft
title: A bound program can be named beside its bindings file, sees its run's timeout in its environment, and an exit its binding classes as a program error or as not called records nothing and is not counted
summary: program_root bindings and resolve_programs; LOOM_TIMEOUT_SECONDS, LOOM_CASE_NAME and LOOM_PARENT_CASE_NAME beside LOOM_CASE; exits {error, not_called}, no default list; ProgramError ends the run
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:bound-program-case-id
- depends_on: story:bound-program-retry-records
- depends_on: story:child-case-runtime
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
revision: 9
---
## Outcome

A `program` binding of `loom.bindings/1` gains four things Runbook's driver gives its tools
(`~/beyond10x/sdc/driver/src/main.rs`, `D` below) and Loom does not. All four are in
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
  The same holds for `arguments_from[0]` of an `open` binding: `resolve_programs` resolves it as
  it resolves a `program` binding's, and refuses it `ProgramNotBesideBindings` naming the action
  and `arguments_from[0]`; `story:open-children-from-arguments-from` runs it.
- **The run's timeout in its environment.** `loom.bindings.ProgramEnvironment`
  (`story:bound-program-case-id`: `path`, `loom_case`) gains `loom_timeout_seconds`, the whole
  seconds of that run's timeout (the binding's `timeout_seconds`, else 60,
  `crates/loom-bindings/src/program.rs:188`), as `LOOM_TIMEOUT_SECONDS`.
- **The name the child's opening gave it.** `ProgramEnvironment` gains `loom_case_name`, set as
  `LOOM_CASE_NAME` for a case a parent opened: the name its opening gave it, the `open`
  executor's `name` or the entry of its `each` argument (`ChildOpening.name`, "the last segment of
  its id"), without the repeat suffix. A child opened again for the same name is `<id>.2`,
  `<id>.3` (`story:child-case-runtime`), and gets the same `LOOM_CASE_NAME` as the first, so a
  tool that reads its epic or story from `LOOM_CASE` reads it from `LOOM_CASE_NAME` instead. A
  root case (no `parent` link) has no `LOOM_CASE_NAME`. The value is the `name` of the case's
  `parent` link as the governor stores it (`CaseState.parent`, read through
  `CanonGovernor::case_state` at the step), never a value of a request, an argument, a report, a
  program's output or the bindings document. It satisfies the rule of a child name, one or more
  ASCII letters, digits, `-` and `_`, and holds at most 4096 bytes, the bound of `LOOM_CASE`
  (`crates/loom-bindings/src/child.rs:17-18`); a stored name that does not refuses the step and
  nothing starts.
- **The name the parent's opening gave it.** `ProgramEnvironment` gains `loom_parent_case_name`,
  set as `LOOM_PARENT_CASE_NAME` only for a case whose parent was itself opened by a case: the
  `name` of the parent's own `parent` link, read through `CanonGovernor::case_state` of the
  parent the case's link names. One level up and no further. In a tree root -> epic -> story the
  root's programs get neither variable, the epic's get `LOOM_CASE_NAME=<epic>` alone, and the
  story's get `LOOM_CASE_NAME=<story>` and `LOOM_PARENT_CASE_NAME=<epic>`. A repeated parent
  `<epic>.2` gives its children the same `LOOM_PARENT_CASE_NAME` as the first. The rules of
  `LOOM_CASE_NAME` hold for it: the governor's stored link name, never a value of a request, an
  argument, a report, a program's output or the bindings document; a child name of at most 4096
  bytes, else the step is refused and nothing starts.
- **A parent that cannot be read.** When the read of the parent's stored link fails (the store
  no longer holds the parent, the parent's case file is refused on read, or the child's link names
  a parent whose state lists no such child), the step is refused before any process starts and
  before any write, with a refusal that names the case id and the parent id and quotes no stored
  text. `LOOM_PARENT_CASE_NAME` is never left unset as a fallback: a tool would then read the case
  as a child of the root.
- **The whole environment.** A bound program starts with `PATH`, `LOOM_CASE`,
  `LOOM_CASE_NAME` (a child only), `LOOM_PARENT_CASE_NAME` (a child of a child only),
  `LOOM_TIMEOUT_SECONDS` and, on a retried step,
  `LOOM_RETRY_FEEDBACK` as `story:bound-program-retry-records` sets it; its `arguments_from`
  program starts with the same variables but `LOOM_RETRY_FEEDBACK`. `bound_case_child` sets each
  as it sets `LOOM_CASE` (`child.rs:47-66`), after the child is built, so no host variable gives
  any of them another value.
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
request, report or output sets `LOOM_TIMEOUT_SECONDS`, `LOOM_CASE_NAME` or
`LOOM_PARENT_CASE_NAME`. A program chooses its exit code and so classes its own run; a class only
withholds the run's evidence and its count, and an error ends the run. Nothing the program
printed enters a refusal or the observation: they name the exit code and the blob digest.

## Why

- `{dir}` is replaced by the bindings file's directory at `init` in every `argv`, `args_from` and
  child path (`D:885-890`); 58 bindings in 9 files start a tool under `{dir}/tools/`
  (`~/beyond10x/sdc/driver/examples/sdc/sdc-intake.bindings.json:24-27` and the like). Runbook's
  `loom.bindings/1` files name `tools/<file>` with no `..` (`bindings/sdc-story.yaml:14` on
  Runbook branch `feat/loom-tools`, `be5eafa`). Loom's `argv` is literal and starts at the
  workspace root (`bindings.rs:414-449`), and a relative `argv[0]` is a workspace path
  (`website/docs/guides/bindings.md:77`). Runbook's tools are not in the workspace, and Runbook
  commits no absolute home path (`~/beyond10x/sdc/AGENTS.md`, Rules).
- The driver gives each tool `SDC_TIMEOUT_S` (`D:1514`); Runbook's replay adapter reads it to time
  a recorded call out on its virtual clock (`~/beyond10x/sdc/driver/src/bin/sdc-replay.rs:1113`,
  `:1125`). Loom gives `PATH` and `LOOM_CASE` (`story:bound-program-case-id`).
- Runbook's tools read the epic and the story from fields 2 and 3 of the case id
  (`SDC_CASE=$LOOM_CASE`, `bindings/tools/llm-node.sh:79` on `feat/loom-tools`;
  `driver/examples/sdc/tools/sdc-tool.sh:258`, `:330`; `llm-node.sh:229-230`). Loom names a child
  `<parent>.<name>`, and a repeat `<parent>.<name>.<n>` from 2
  (`crates/loom-governor/src/children.rs:478-485` on `impl/child-case-runtime`), so under a
  repeated epic a story's id is `IDEA.<epic>.2.<story>` and field 3 reads `2`. On Runbook branch
  `wave/rb1` (`24dca6b`) the tools still take the epic from field 2 and the story from field 3
  (`bindings/tools/llm-node.sh:347-348`, `bindings/tools/sdc-tool.sh:306`, `:378`), and an
  `sdc.story`, `sdc.spec`, `sdc.plan` or `sdc.release` case needs its epic for the `aep` ids: its
  parent's name. No tool needs the root's name.
- Exits 126, 127 and 75 are program errors in the driver: no evidence, not counted, the run stops
  (`D:576-581`, `:682`, `:1550`, `:1619-1629`). Loom puts every exit through the table, `*`
  included (`program.rs:270-291`), and 27 Runbook bindings have a `*` row, so a missing program
  becomes an `invalid` record about the subject. `story:run-caps-per-arrival` puts exit 126 and
  127 out of its scope.
- A tool that did nothing writes `SDC_NOT_CALLED`; the run is recorded `not_called` and not
  counted (`D:1497-1501`, `:1537-1540`, `:776`; `driver/examples/sdc/tools/llm-node.sh:34-36`).
  Under Loom it exits 0 today and takes a not-called exit code once a binding can name one
  (Runbook's `bindings/PENDING.md:34` on `feat/loom-tools`). A file the program writes is not
  used here: under `linux` the workspace is read-only outside `target/` and the paths
  `story:bound-program-declares-its-environment` declares writable.

## ESS first

`ess/domains/bindings.yaml` first: `ProgramRoot` (`Workspace`, `Bindings`) and
`BindingsDocument.program_root: Optional<ProgramRoot>`; `ExitClasses` (`error`, `not_called`, each
`List<Integer>`) and `BoundAction.exits: Optional<ExitClasses>`; `ExitClass` (`Error`,
`NotCalled`) and `ProgramRun.class: Optional<ExitClass>`; `ProgramEnvironment` (`:277-292`) gains
`loom_timeout_seconds: Integer`, `loom_case_name: Optional<String>` and
`loom_parent_case_name: Optional<String>`, its comment saying each name is the governor's stored
opening name (the case's link, and the parent's link), the first absent for a root, the second
absent unless the parent was opened by a case, each one child name of at most 4096 bytes, and
never set by a request, an argument, a report, a program's output or the bindings document;
`BindingsRefusalReason` gains `ExitClassConflict` and `ProgramNotBesideBindings`;
`ProgramRefusalReason` gains `ProgramError`. On that commit `task drift` fails, and
`crates/loom-bindings/tests/program_contract.rs` does not compile.

The stored name is `story:child-case-runtime`'s: `loom.governor.CaseLink` gains `name: String`
there (see § Scope, "Needs").

## Acceptance

`crates/loom-bindings/tests/program_contract.rs` drives the program effect port as
`crates/loom-bindings/tests/bound_program.rs` does: Commission's `run_until_blocked`, a scripted
executor, a `CanonGovernor` over a `MemoryCaseStore`, a fixture protocol of two artifacts, stub
programs the test writes under `CARGO_TARGET_TMPDIR`, a `local` runner opened with
`UNCONFINED_FLAG_REASON`; no model, no network. The bindings file sits in directory `A`, the
workspace is directory `B`, and the process's working directory is a third one. A child case is
opened with `CanonGovernor::open_children` from a fixture parent protocol with one `case_open`
action; the root case is `P`, and the three-level tree is `P` -> `P.epic` -> `P.epic.story`.

- With `program_root: bindings` and `argv: [bin/stub]`, the stub at `A/bin/stub` runs and its
  output is the run's blob (test `a_program_beside_the_bindings_file_runs`).
- With no `program_root`, `argv: [bin/stub]` runs `B/bin/stub` and not `A/bin/stub` (test
  `a_relative_program_is_a_workspace_path_by_default`).
- With `program_root: bindings`, `arguments_from: [bin/args]` runs `A/bin/args` and the bound
  program reads its object (test `an_arguments_program_beside_the_bindings_file_runs`).
- With `program_root: bindings`, an `open` binding's `arguments_from: [../bin/list]` is refused
  `ProgramNotBesideBindings` naming the action and `arguments_from[0]`, and its
  `arguments_from: [bin/list]` becomes `A/bin/list` in the resolved document (test
  `an_open_bindings_arguments_program_is_resolved_too`).
- `argv: [../bin/stub]` is refused `ProgramNotBesideBindings` naming the action and
  `executor.argv[0]` (test `a_program_path_leaving_the_bindings_directory_is_refused`).
- `A/bin/stub` as a link to a file outside `A` is refused `ProgramNotBesideBindings` (test
  `a_linked_program_outside_the_bindings_directory_is_refused`).
- `argv: [bin/absent]` is refused `ProgramNotBesideBindings` before anything starts (test
  `a_missing_program_beside_the_bindings_file_is_refused`).
- `program_root: elsewhere` is refused `Malformed` naming `program_root` (test
  `an_unknown_program_root_is_refused`).
- A stub of the root case `P` that prints its environment shows exactly `PATH`, `LOOM_CASE=P` and
  `LOOM_TIMEOUT_SECONDS=7` for a binding with `timeout_seconds: 7` (test
  `a_program_starts_with_its_timeout_in_its_environment`).
- With no `timeout_seconds`, the stub shows `LOOM_TIMEOUT_SECONDS=60` (test
  `a_program_without_a_timeout_reads_the_default`).
- An `arguments_from` stub of `P` that prints its environment shows the same three variables
  (test `an_arguments_program_starts_with_its_timeout_in_its_environment`).
- Security: a request whose arguments hold `{"LOOM_TIMEOUT_SECONDS": "3600"}` leaves the variable
  at `7`, and the arguments reach standard input as written (test
  `an_argument_named_loom_timeout_seconds_does_not_change_the_variable`).
- A stub of the child `P.story`, opened for the name `story`, shows exactly `PATH`,
  `LOOM_CASE=P.story`, `LOOM_CASE_NAME=story` and `LOOM_TIMEOUT_SECONDS` (test
  `a_child_program_reads_the_name_its_opening_gave_it`).
- The second opening for `story` gives `P.story.2`, whose stub shows `LOOM_CASE=P.story.2` and
  `LOOM_CASE_NAME=story` (test `a_repeated_child_reads_the_name_of_the_first`).
- Root level: a stub of `P` in the three-level tree shows exactly `PATH`, `LOOM_CASE=P` and
  `LOOM_TIMEOUT_SECONDS`, neither name (test `a_root_program_reads_neither_name`).
- Epic level: a stub of `P.epic` shows exactly `PATH`, `LOOM_CASE=P.epic`, `LOOM_CASE_NAME=epic`
  and `LOOM_TIMEOUT_SECONDS`, no `LOOM_PARENT_CASE_NAME` (test
  `a_child_of_the_root_reads_its_name_and_no_parent_name`).
- Story level: a stub of `P.epic.story` shows exactly `PATH`, `LOOM_CASE=P.epic.story`,
  `LOOM_CASE_NAME=story`, `LOOM_PARENT_CASE_NAME=epic` and `LOOM_TIMEOUT_SECONDS` (test
  `a_grandchild_reads_its_name_and_its_parents_name`).
- Under a repeated parent `P.epic.2`, its child opened for `story` shows
  `LOOM_CASE=P.epic.2.story`, `LOOM_CASE_NAME=story` and `LOOM_PARENT_CASE_NAME=epic` (test
  `a_child_of_a_repeated_parent_reads_its_own_name_and_the_first_parents_name`).
- Under `P.epic.story`, a fourth-level child `P.epic.story.task` shows
  `LOOM_PARENT_CASE_NAME=story` and no other ancestor's name (test
  `the_parent_name_is_one_level_up_and_no_further`).
- A repeated grandchild `P.epic.story.2` and a grandchild under a repeated parent
  `P.epic.2.story` each show `LOOM_CASE_NAME=story` and `LOOM_PARENT_CASE_NAME=epic` (test
  `a_repeat_at_the_third_level_reads_the_names_of_the_first`).
- An `arguments_from` stub of `P.epic.story` shows `LOOM_CASE_NAME=story` and
  `LOOM_PARENT_CASE_NAME=epic` (test `an_arguments_program_reads_both_names`).
- A child opened through `each` with the entries `a` and `b` gives `LOOM_CASE_NAME=a` and `=b` to
  the stubs of `P.a` and `P.b` (test `each_child_of_an_each_opening_reads_its_entry`).
- An `arguments_from` stub of `P.story` shows `LOOM_CASE_NAME=story` (test
  `an_arguments_program_reads_the_case_name`).
- With a retry record for `P.story`'s writer, its stub shows exactly `PATH`, `LOOM_CASE`,
  `LOOM_CASE_NAME`, `LOOM_TIMEOUT_SECONDS` and `LOOM_RETRY_FEEDBACK` (test
  `a_retried_child_writer_reads_its_name_and_its_feedback`).
- Security: a request whose arguments hold `{"LOOM_CASE_NAME": "x"}`, and an `arguments_from`
  stub that prints that member, leave the variable at `story` (test
  `an_argument_named_loom_case_name_does_not_change_the_variable`).
- Security: a request whose arguments hold `{"LOOM_PARENT_CASE_NAME": "x"}`, and an
  `arguments_from` stub that prints that member, leave the variable of `P.epic.story` at `epic`
  (test `an_argument_named_loom_parent_case_name_does_not_change_the_variable`).
- Security: a case file over a `FileCaseStore` whose stored link names `a b`, or a name of 4097
  bytes, refuses the step `<program> was not started: the case name is not a child name` and
  starts nothing (test `a_stored_name_that_is_not_a_child_name_starts_nothing`).
- Security: the same for the stored link of `P.epic` read for `P.epic.story`'s step, refused
  `<program> was not started: the parent case name is not a child name` (test
  `a_stored_parent_name_that_is_not_a_child_name_starts_nothing`).
- Security: for `P.epic.story` over a `FileCaseStore`, with `P.epic`'s case file removed, with it
  made unreadable, and with `P.epic`'s `children` no longer listing the story, each step is
  refused naming `P.epic.story` and `P.epic` and quoting no stored text, no process starts and no
  byte of the store changes (test `a_parent_that_cannot_be_read_refuses_the_step_before_anything`).
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
- `grep -c LOOM_CASE_NAME website/docs/guides/bindings.md` prints 2 or more: the bullet on how a
  program starts and the bullet on `LOOM_CASE` each name it.
- `grep -c LOOM_PARENT_CASE_NAME website/docs/guides/bindings.md` prints 2 or more: the same two
  bullets name it.
- `grep -c LOOM_CASE_NAME AGENTS.md` prints 1 or more: § Bindings document names it beside
  `LOOM_CASE`.
- `grep -c LOOM_PARENT_CASE_NAME AGENTS.md` prints 1 or more: § Bindings document names it
  beside `LOOM_CASE_NAME`.
- No test of the file runs 60 s; only `a_timeout_is_never_an_exit_class` waits, about 1 s, and
  `slot cargo test -p b10x-loom-bindings --test program_contract` passes within the 120 s step.
- `task check` exits 0.

## Out of scope

The `WriterNotCalled` stop, the due rule for a not-called writer and `tree open` calling
`resolve_programs` (`story:writer-not-called-stop`). Naming an exit code that is in neither
`exits` nor a result table: the bindings-check story's (`b10x-loom bindings check`, not yet in
the store). A store or case directory in the environment: a program learns facts about the case
from its inputs, an `arguments_from` program or `tree status`. The name of any ancestor above the
parent, the root's name included: no tool needs one. Mounting a program's sibling files under
`linux` (`read_only_roots` of `story:bound-program-declares-its-environment`). Exit classes of an
`agent` executor (`story:bound-agent-node`). Counting runs since an arrival
(`story:run-caps-per-arrival`). The action id in the environment: each binding's `argv` names it
literally. `{dir}` in a child's protocol path: the child's protocol is resolved in the host's
catalog (`story:child-case-runtime` § Outcome).

## Scope

Derived 2026-11-21 by the drafting agent, revised 2026-11-22 on `9845a17c` and
`impl/child-case-runtime` `2548daf9`. Every line is **cited** (read in the tree) or **inferred**
(a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings` (`program_root`, `resolve_programs`, `exits`, the
  run count, the timeout and name variables) — cited (`program.rs:126-148`, `:184-291`)
- **Files:** `ess/domains/bindings.yaml` (`BindingsDocument` :190-202, `BoundAction` :163-185,
  `ProgramRun` :258-272, `ProgramEnvironment` :277-292, `ProgramRefusalReason` :313-320) — cited
- **Files:** `crates/loom-bindings/src/decode.rs` (`TOP` :14, `ACTION` :15-26, `bound_action`
  :192-295) — cited
- **Files:** `crates/loom-bindings/src/load.rs` (`check` :30-215, `fit` :228-284) — cited
- **Files:** `crates/loom-bindings/src/program.rs` (`runs` :134-148, `perform` :150-331, the
  bound program's `bound_case_child` call :240-247, `arguments` :335-398 and its call :485-492,
  `refused` :603-608) — cited
- **Files:** `crates/loom-bindings/src/child.rs` (`bound_case_child` :33-52 gains the two names
  and the timeout; `environment` :56-66; the child-name checks) — cited
- **Files:** `crates/loom-bindings/src/lib.rs` (exports `resolve_programs`) — inferred
- **Not changed:** `crates/loom-intake-slice/src/bindings.rs`: `bound_case_child` replaces the
  environment of `bound_program_child`'s child (`child.rs:46-51`) — cited
- **Files:** `crates/loom-bindings/tests/program_contract.rs` — inferred, name proposed here
- **Documents:** `website/docs/guides/bindings.md` (a relative `argv[0]` :77, the start of a
  program :216-222, `LOOM_CASE` :223-230), `AGENTS.md` (§ Bindings document, :1245-1255),
  `CHANGELOG.md` — cited
- **Needs, from `story:child-case-runtime`** (open, `impl/child-case-runtime`): the stored name.
  The governor does not store the opening name apart from the case id today: `CaseLink` holds
  `parent`, `child`, `action`, `outcome_evidence`, `subject`, `subject_revision`
  (`ess/domains/governor.yaml:148-161` there), and `ChildOpening.name` is used only to build the
  id (`children.rs:478`). That unit adds `name: String` to `loom.governor.CaseLink`
  (`governor.yaml:148-161`), sets it from `child.name` where it builds the link
  (`children.rs:490-500`), and writes and reads it (`file_store.rs:672-707`, `LINK_MEMBERS`
  :689-696). This story reads `CaseState.parent`'s `name` (`crates/loom-governor/src/lib.rs:233`
  there) for `LOOM_CASE_NAME`, and for `LOOM_PARENT_CASE_NAME` the `name` of the `parent` link of
  the case that link's `parent` names, through a second `case_state` read — cited
- **Symbols:** `resolve_programs`, `ProgramRoot`, `ExitClasses`, `ExitClass`, `ProgramRun.class`,
  `ProgramEnvironment.loom_timeout_seconds`, `ProgramEnvironment.loom_case_name`,
  `ProgramEnvironment.loom_parent_case_name`, `ProgramNotBesideBindings`, `ExitClassConflict`,
  `ProgramError`, `LOOM_TIMEOUT_SECONDS`, `LOOM_CASE_NAME`, `LOOM_PARENT_CASE_NAME` (new);
  `ProgramEffects::runs`, `bound_case_child` (changed) — cited / inferred
- **Would collide with:** every story on `ess/domains/bindings.yaml`, `decode.rs`, `load.rs` or
  `program.rs` (`story:bound-program-retry-records`, `story:run-caps-per-arrival`,
  `story:bound-program-declares-its-environment`, `story:person-actions`,
  `story:handoff-when`, `story:initial-child-revisions`); `story:bound-program-retry-records`
  also on `ProgramEnvironment` and `bound_case_child`; `story:child-case-runtime` on the stored
  `CaseLink`; `story:gate-program-identity-bound-to-spawn`, which reads a relative `argv[0]` with
  a `/` as a workspace path (its § Outcome) — cited
