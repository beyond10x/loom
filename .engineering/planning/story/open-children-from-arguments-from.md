---
format: aep.planning-md/3
id: story:open-children-from-arguments-from
kind: story
status: active
title: An open binding with arguments_from opens one child per name its program prints, runs that program as a program binding's arguments program runs, and resumes a cut opening without opening a child twice
summary: run_arguments_from shared by both ports; OpenEffects::new takes workspace, store and runner; printed object checked against the action's arguments schema; names checked before any write; the list kept as a loom/open observation and a blob, reused on resume
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:child-case-runtime
- depends_on: story:canon-0-10-0-pin
- depends_on: story:bound-program-case-id
- depends_on: story:bound-program-contract
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-bindings/src/arguments.rs
- confidence: inferred
  path: crates/loom-bindings/src/lib.rs
- confidence: cited
  path: crates/loom-bindings/src/open.rs
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_open.rs
- confidence: inferred
  path: crates/loom-bindings/tests/fixtures/open-from-program
- confidence: inferred
  path: crates/loom-bindings/tests/main.rs
- confidence: cited
  path: crates/loom-governor/src/children.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: inferred
  path: website/docs/guides/bindings.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T00:22:59Z", actor: "agent:sdc", revision: 6, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w9"}
- {from: "proposed", to: "active", at: "2026-11-22T00:23:00Z", actor: "agent:sdc", revision: 7, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w9"}
---
## Outcome

An action bound to an `open` executor with `each: <member>` and `arguments_from` opens one child
case per string in that member of the JSON object its `arguments_from` program prints. The
program runs as a `program` binding's `arguments_from` program runs, through one function both
ports call.

- **One function.** The new module `crates/loom-bindings/src/arguments.rs` holds
  `run_arguments_from(argv, input, timeout, workspace, runner, case)`, lifted from
  `ProgramEffects::arguments` (`crates/loom-bindings/src/program.rs:456-519`) with `one_object`
  (`program.rs:690-706`). It builds the child with `bound_case_child`, runs it once through the
  admitted runner's `run_child`, and returns the object, its bytes and their SHA-256, or why
  not: the program was not started, the runner refused it, it was killed at its timeout, it ended
  with an exit other than 0, it printed more than `MAX_REPORT_BYTES` (1 MiB,
  `crates/loom-intake-slice/src/bindings.rs:73`), or it printed anything but one JSON object.
  `ProgramEffects::arguments` calls it and keeps its refusal `ArgumentsUnreadable` and its texts.
- **The program's environment** is that of a `program` binding's `arguments_from` program under
  `story:bound-program-contract` § Outcome ("The whole environment"), for the case the port was
  built for: `PATH`, `LOOM_CASE`, `LOOM_CASE_NAME` (a child only), `LOOM_PARENT_CASE_NAME` (a
  child of a child only) and `LOOM_TIMEOUT_SECONDS`; never `LOOM_RETRY_FEEDBACK`. A root's opening
  program gets neither name. Both names are the governor's stored link names, read through
  `CanonGovernor::case_state`. When the read of the parent's stored link fails, the step is refused
  as that story refuses it, before any process starts and before any write, naming the case and
  the parent and quoting no stored text.
- **The port.** `OpenEffects::new` gains `workspace`, `store` and
  `runner: Option<Arc<dyn TestRunner>>`, as `ProgramEffects::new` takes them
  (`program.rs:132-150`), and returns `OpenPortError`: `Refused(Refusal)` or
  `Runner(ConfinementError)`. When a binding of the document has `arguments_from`, the runner is
  admitted by `admit_bound_program_runner` and checked with `preflight_child` on `workspace`; a
  document with none needs no runner. `arguments_from` on an `open` action whose protocol declares
  no `arguments` schema is refused `ArgumentsUndeclared`: Canon gives such an action `{}` alone.
- **A step**, in order. The object is the request's arguments when they are not `{}`, else the
  binding's literal `arguments`, else the object `arguments_from` prints, as for a program
  binding (`program.rs:462-473`); `name` takes none of them.
  1. A list stored for this opening (below) is taken, and the program is not run.
  2. Else the program runs under the binding's `timeout_seconds`, else 60, with the request's
     arguments on standard input and the workspace root as its start directory.
  3. When the action's protocol declares an `arguments` schema, the object is checked against it
     with Canon's `check_arguments`, whichever of the three sources it came from; a mismatch is
     `ArgumentsMismatched`, naming the JSON pointer and the keyword. When the protocol declares
     none, a request's or literal object is not checked, as today, and `arguments_from` is
     refused when the port is built (above).
  4. The member `each` names: missing or not a list (`EachNotAList`), an entry that is not a
     string (`EachItemNotText`), no entry (`NoChildNamed`), an entry given twice
     (`ChildNameRepeated`), an entry that is not one plain segment of ASCII letters, digits, `-`
     and `_` (`ChildNameInvalid`, the governor's rule, `loom_governor::plain_child_name`, made
     public from `crates/loom-governor/src/children.rs:211`).
  5. The program's output is kept as `blobs/<sha256>` in `store`; the port keeps one observation
     of source `loom/open` (`ChildNames`: case, action, case revision, source, the names in the
     order printed, the blob digest), then calls `CanonGovernor::open_children`.

  A refusal at 2, 3 or 4 opens no child and writes nothing: no blob, no observation, no record.
- **Resume.** The observation is the stored list. Before it runs the program, the port looks for
  a `loom/open` observation naming this case and action, source `ArgumentsFrom`, and the case
  revision the request was made at. When there is one, its names are the list, the program is not
  run, and the step goes on at 5 with that observation and writes no second one. A list is reused
  only at the case revision it was recorded at: at any other case revision the program runs
  again. Keeping an observation does not move the case revision: `CanonGovernor::observe` writes
  the store's observations alone (`crates/loom-governor/src/lib.rs:1594-1603`,
  `crates/loom-governor/src/file_store.rs:309-315`, `bf943c01`). The governor
  takes over each child already written for the same name, action and subject revision and opens
  the others (`story:child-case-runtime` § Outcome), so no child is opened twice. A second run is
  not used: the program reads state outside the case (Runbook's `list-epics` lists the planning
  store), so it can print another list at the same case revision. A child of the first list that
  the second list does not name would then stay written with a parent that does not list it.
- **What is recorded.** In the governor: the `ChildNames` observation, then one `case_outcome`
  record per child (`story:child-case-runtime`). In the report of a performed step: `names`,
  `source` and `blob_sha256` beside `children` and `evidence` (`open.rs:388-392`), from which
  `story:tree-run-command` writes the step's `action finished` line with `blob_sha256` as its
  `blob_digest`.

The children this port opens are the only source of the parent's `children.passed`: a
`case_outcome` record submitted through the evidence port is refused for every case whose
protocol declares a `case_open` action (`story:child-case-runtime`, the planner's decision of
2026-11-22).

What cannot be steered through the program's output: it names the children and is the action's
arguments. It cannot name the child's protocol, its bindings file or its independence, a record's
kind, result, subject or revision, or a child id beyond `<parent>.<name>`. It cannot change the
list once observed at a case revision. A refusal names the member, the entry's index, the JSON
pointer and the keyword, never the text the program printed.

## Why

Runbook's root opens one epic case per epic of its planning store, and each epic one story case
per story that delivers it: `epics.open` with `each: epics` and
`arguments_from: [tools/sdc-tool.sh, list-epics, ...]`, `stories.open` with `each: stories`
(`~/beyond10x/sdc/bindings/sdc.yaml:35-40`, `~/beyond10x/sdc/bindings/sdc-epic.yaml:45-50`, on
Runbook's branch `feat/loom-tools` at `be5eafa`). Both actions declare an `arguments` schema: an
object whose one member is an array of non-empty strings with `minItems: 1`
(`~/beyond10x/sdc/protocol/sdc.protocol.yaml:131-141`,
`~/beyond10x/sdc/protocol/sdc-epic.protocol.yaml:168-178`, same commit).

On branch `impl/child-case-runtime` (`bf943c01`), `OpenEffects::names`
(`crates/loom-bindings/src/open.rs:407-430`) takes `name`, else `each` from the request's
arguments or the literal `arguments`; it never reads `arguments_from`, so the step is refused
"the argument epics is not a list of strings" (`open.rs:299-302`). `OpenEffects::new` takes no
runner, workspace or timeout (`open.rs:158-164`). The only run of an `arguments_from` argv is
`ProgramEffects::arguments`, private to the program port (`program.rs:456-519`). The port keeps
its observation before the governor checks the names (`open.rs:306-319`;
`crates/loom-governor/src/children.rs:418-433`), so a refused name leaves an observation behind.
`story:tree-run-command` § Outcome hands the arguments to the `open` port when the port does not
read them itself; with this story it does.

## ESS first

`ess/domains/bindings.yaml` first: `OpenRefusalReason` (`ArgumentsUnreadable`,
`ArgumentsMismatched`, `EachNotAList`, `EachItemNotText`, `NoChildNamed`, `ChildNameRepeated`,
`ChildNameInvalid`) and `OpenRefusal` (reason, action, detail); `ChildNameSource` (`Name`,
`Request`, `Literal`, `ArgumentsFrom`) and `ChildNames` (case, action, case revision, source,
names, blob digest optional), the payload of the `loom/open` observation; `BindingsRefusalReason`
gains `ArgumentsUndeclared`. On that commit `task drift` fails, and the test module
`open_from_program` does not compile.

## Acceptance

The tests are the module `open_from_program` of the crate's one test binary,
`crates/loom-bindings/tests/main.rs`. If the base the unit starts from still has one file per
test binary, the module is the file `crates/loom-bindings/tests/open_from_program.rs`, and the
unit's report says so. The module drives the open port as `crates/loom-bindings/tests/bound_open.rs`
does (Commission's `run_until_blocked`, a scripted executor, a `CanonGovernor` over a
`FileCaseStore` under `CARGO_TARGET_TMPDIR`), with a `local` runner opened with
`UNCONFINED_FLAG_REASON` and stub programs the test writes; no model, no network. Its fixtures,
`crates/loom-bindings/tests/fixtures/open-from-program/`, have the shape of Runbook's and copy no
file of it: protocols `roadmap/1` (`epics.open` opens `epic`; `roadmap.note`, a `program`
action whose stub records one record about `roadmap`, moves the case revision), `epic/1`
(`stories.open` opens `story`), `story/1` (`tasks.open` opens `task`, a third level Runbook does
not have), `task/1`,
each opening action with the schema Runbook declares (`epics`, `stories` or `tasks`, an array,
`minItems: 1`, string items of `minLength: 1`, required, no other member), and `loose/1`, whose
opening action's schema is `{type: object}`; one bindings file per protocol in directory `A`,
beside `A/bin/`; the workspace is directory `B`. The cut tests wrap the `FileCaseStore` in a
`FallibleCaseStore` the module defines, of the form of `Faulty` in
`crates/loom-governor/tests/child_cases.rs:280-339` (`bf943c01`), which fails the next `update`
of a chosen case. The module's wrapper also fails the next `insert` of a chosen case id, because
`open_children` writes each child with `insert` and the parent with `update`
(`crates/loom-governor/src/children.rs:560-575`, `bf943c01`). A test file of another crate cannot
be imported, so the wrapper is written again here.

- `epics.open` with a stub that prints `{"epics": ["a", "b"]}` opens `root.a` and `root.b` under
  `epic@1`, with one `case_outcome` record each in `root` (test
  `an_each_opening_opens_one_child_per_printed_name`).
- In `root.a`, `stories.open` with a stub that prints `{"stories": ["s1", "s2", "s3"]}` opens
  `root.a.s1`, `root.a.s2` and `root.a.s3` (test `a_child_opens_its_own_children_from_its_program`).
- Once `root.a` and `root.b` are terminated `passed`, `root`'s `children.passed` is true; with
  one of them open it is unknown (test `children_passed_reads_the_children_the_program_named`).
- `{"epics": ["a"], "extra": 1}` is refused `ArgumentsMismatched` naming `additionalProperties`;
  no child is opened and the store directory's files are byte-identical (test
  `a_printed_object_outside_the_schema_is_refused`).
- `{"epics": []}` is refused `ArgumentsMismatched` naming `/epics` and `minItems`, and nothing is
  written (test `an_empty_list_is_refused_by_min_items`).
- The opening program of `root`, a stub that prints its environment, shows exactly `PATH`,
  `LOOM_CASE=root` and `LOOM_TIMEOUT_SECONDS`, and neither `LOOM_CASE_NAME` nor
  `LOOM_PARENT_CASE_NAME` (test `a_roots_opening_program_gets_no_case_name`).
- The opening program of `root.a` shows exactly `PATH`, `LOOM_CASE=root.a`, `LOOM_CASE_NAME=a`
  and `LOOM_TIMEOUT_SECONDS` (test `a_childs_opening_program_gets_its_name`).
- The opening program of `root.a.s1` shows exactly `PATH`, `LOOM_CASE=root.a.s1`,
  `LOOM_CASE_NAME=s1`, `LOOM_PARENT_CASE_NAME=a` and `LOOM_TIMEOUT_SECONDS` (test
  `a_grandchilds_opening_program_gets_both_names`).
- With `root`'s case file removed, `stories.open` in `root.a` is refused naming `root.a` and
  `root` and quoting no stored text; the stub's run count file does not exist and no byte of the
  store changes (test `an_opening_whose_parent_cannot_be_read_starts_nothing`).
- The stub reads the request's arguments, `{}`, on standard input and starts in `B` (test
  `an_open_arguments_program_reads_the_request_and_starts_in_the_workspace`).
- A stub that exits 1 is refused `ArgumentsUnreadable` naming exit 1, and nothing is written
  (test `an_arguments_program_ending_non_zero_opens_nothing`).
- With `timeout_seconds: 1`, a stub that sleeps 5 s is refused `ArgumentsUnreadable` naming the
  timeout (test `an_arguments_program_killed_at_its_timeout_opens_nothing`).
- A stub that prints 1 MiB and one byte is refused `ArgumentsUnreadable` (test
  `an_output_over_the_bound_opens_nothing`).
- Stubs that print `[]` and two objects are each refused `ArgumentsUnreadable` (test
  `an_output_that_is_not_one_object_opens_nothing`).
- `arguments_from: [bin/absent]` is refused before anything is written (test
  `an_arguments_program_that_does_not_start_opens_nothing`).
- Under `loose/1`, `{}` and `{"epics": "a"}` are each refused `EachNotAList` naming `epics` (test
  `a_missing_or_scalar_member_opens_nothing`).
- Under `loose/1`, `{"epics": ["a", 1]}` is refused `EachItemNotText` naming entry 1 (test
  `a_non_text_name_opens_nothing`).
- Under `loose/1`, `{"epics": []}` is refused `NoChildNamed` (test
  `an_empty_list_without_min_items_opens_nothing`).
- `{"epics": ["a", "a"]}` is refused `ChildNameRepeated` naming entry 1 (test
  `a_repeated_name_opens_nothing`).
- `["a b"]`, `["../x"]` and `["a.b"]` are each refused `ChildNameInvalid` naming entry 0, and the
  refusal's text holds no entry's text (test `a_name_that_is_not_one_segment_opens_nothing`).
- `arguments_from` on an `open` action whose protocol declares no `arguments` schema makes
  `OpenEffects::new` refuse `ArgumentsUndeclared` naming the action (test
  `an_arguments_program_needs_a_declared_schema`).
- With no runner, `OpenEffects::new` over a document with `arguments_from` answers
  `OpenPortError::Runner` (`NoRunner`), and over one without it is built (test
  `an_open_with_an_arguments_program_needs_a_runner`).
- After the opening, the store holds one `loom/open` observation with the case, the action, the
  case revision, source `ArgumentsFrom`, the names in the order printed and a blob digest whose
  file under `blobs/` holds the printed bytes; the step's report carries the same names and
  digest (test `the_list_is_recorded_with_its_digest`).
- The wrapper fails the `insert` of `root.a`: the step ends with an effect error, `root`'s
  revision is the one the request was made at, and the next step at that revision opens `root.a`
  and `root.b` while the stub's run count (a file it appends to) stays 1 (test
  `an_opening_cut_before_its_children_resumes_without_a_second_run`).
- The wrapper fails the `update` of `root` after both children were inserted: the next step opens
  no `root.a.2`, the tree lists `root.a` and `root.b` once each, and the stub ran once (test
  `an_opening_cut_after_its_children_opens_none_twice`).
- A stub that prints `["a", "b"]` on its first run and `["c"]` on any later one: after a cut, the
  children are `root.a` and `root.b` (test `a_list_once_observed_is_the_list_at_that_revision`).
- After a cut, the test moves `root` to another case revision (a record of another action):
  the next opening runs the stub again and opens `root.c` (test
  `a_list_is_not_reused_at_another_case_revision`).
- With `program_root: bindings` (`story:bound-program-contract`), `arguments_from:
  [bin/list-epics]` on `epics.open` runs `A/bin/list-epics` and not `B/bin/list-epics` (test
  `an_open_arguments_program_beside_the_bindings_file_runs`).
- `crates/loom-bindings/tests/bound_open.rs` and `crates/loom-bindings/tests/bound_program.rs`
  pass with the new constructor arguments and no other change to an assertion.
- No test of the module runs 60 s; only `an_arguments_program_killed_at_its_timeout_opens_nothing`
  waits, about 1 s, and `slot cargo test -p b10x-loom-bindings --test main open_from_program`
  passes (`--test open_from_program` on a base with one file per test binary).
- `task check` exits 0.

## Out of scope

The argument schema check of a `program` binding's arguments, which
`story:protocol-argument-schema` § Not in scope leaves to a story of the `crates/loom-bindings`
lane. Writing the run log's `action finished` line (`story:tree-run-command`). `tree open`
calling `resolve_programs` (`story:writer-not-called-stop`). Running the program again at the same
case revision after what it reads has changed. Counting `arguments_from` runs against
`budget.max_runs`: the program port does not count them either. The revisions a child opens with
(`story:initial-child-revisions`). `LOOM_CASE_NAME`, `LOOM_PARENT_CASE_NAME`, the failed-parent
read and `resolve_programs` themselves (`story:bound-program-contract`).

## Scope

Derived 2026-11-22 by the drafting agent from `impl/child-case-runtime` at `bf943c01` and the
integration branch at `9845a17c`. Every line is **cited** (read in the tree) or **inferred** (a
reading that could be wrong).

- **Primary surface:** `crates/loom-bindings` (the open port and the lifted `arguments_from`
  run) — cited
- **Files:** `crates/loom-bindings/src/open.rs` (`new` :158-271, `perform` :281-402, `names`
  :404-430) — cited
- **Files:** `crates/loom-bindings/src/arguments.rs` (`run_arguments_from`, `one_object`) —
  inferred, name proposed here
- **Files:** `crates/loom-bindings/src/program.rs` (`arguments` :456-519, `one_object` :690-706,
  `keep_blob` :734) — cited
- **Files:** `crates/loom-bindings/src/lib.rs` (exports `OpenPortError`) — inferred
- **Files:** `crates/loom-governor/src/children.rs` (`plain_segment` :211, made public as
  `plain_child_name`) — cited
- **Files:** `ess/domains/bindings.yaml` (`BindingsRefusalReason` :211-235) — cited
- **Files:** the module `open_from_program` of `crates/loom-bindings/tests/main.rs` (or
  `crates/loom-bindings/tests/open_from_program.rs` on a base with one file per test binary),
  `crates/loom-bindings/tests/fixtures/open-from-program/` — inferred, names proposed here
- **Files:** `crates/loom-bindings/tests/bound_open.rs` (the constructor calls) — cited
- **Read, not changed:** `crates/loom-intake-slice/src/bindings.rs` (`DEFAULT_TIMEOUT_SECONDS`
  :69, `MAX_REPORT_BYTES` :73, `admit_bound_program_runner` :382) — cited
- **Documents:** `CHANGELOG.md` (**Breaking:** `OpenEffects::new` takes the workspace, the store
  directory and the host's runner and returns `OpenPortError`), `website/docs/guides/bindings.md`
  (the `open` executor) — inferred
- **Symbols:** `run_arguments_from`, `OpenPortError`, `OpenRefusalReason`, `ChildNames`,
  `ChildNameSource`, `ArgumentsUndeclared`, `plain_child_name` (new); `OpenEffects::new`,
  `OpenEffects::names`, `ProgramEffects::arguments` (changed) — cited / inferred
- **Would collide with:** `story:child-case-runtime` (`open.rs`, `children.rs`);
  `story:bound-program-contract` (`program.rs`, `bindings.yaml`, the resolution of
  `arguments_from[0]`); `story:tree-run-command` (builds `OpenEffects`);
  `story:bound-program-retry-records`, `story:person-actions`, `story:handoff-when`
  (`program.rs`, `bindings.yaml`); `story:initial-child-revisions` (the `open` port);
  `story:child-case-own-workspace` (the `open` executor's members) — cited
