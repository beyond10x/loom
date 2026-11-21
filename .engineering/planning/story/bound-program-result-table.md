---
format: aep.planning-md/3
id: story:bound-program-result-table
kind: story
status: active
title: A bound program's exit code or report field becomes evidence through the binding's result table
summary: The program effect port for any registered protocol; the program never names a result, revision, producer or instant
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:bindings-document
- depends_on: story:bound-commands-run-confined
scope:
- confidence: cited
  path: crates/loom-bindings
- confidence: inferred
  path: crates/loom-bindings/Cargo.toml
- confidence: cited
  path: crates/loom-bindings/tests/bound_program.rs
- confidence: cited
  path: crates/loom-intake-slice/src/bindings.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: inferred
  path: website/docs/guides/bindings.md
revision: 20
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:45Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-11-21T13:30:58Z", actor: "agent:sdc", revision: 20, decided_on: {"recorded":{"review_outcome":7}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w7"}
---
## Outcome

An action bound to a `program` executor in a `loom.bindings/1` document runs through an effect
port in `b10x-loom-bindings` (`crates/loom-bindings`) for any registered protocol, and its result
becomes evidence by the binding's table alone. The port is built per case from the bindings
document, the case's workspace root and the host's runner, which is an
`Option<Arc<dyn TestRunner>>` because the host may give none. It starts each program through
`run_child` of that runner. The program's standard output and standard error are kept as one
content-addressed blob that the observation names. The arguments are the request's, else the
binding's literal `arguments`, else the object the `arguments_from` argv prints, run through the
same runner. A request has no arguments when its `arguments` member is the empty object `{}`;
every other value, JSON `null` included, is the request's arguments and reaches the program as
written. The member is a `ProposedActionArguments`, a
wrapper of JSON that is always present (`ActionRequestData.arguments`,
`generated/rust/commission/src/responsibility.rs:3105`), so the rule rests on the SDC driver:
its `--args` defaults to `{}` (`~/beyond10x/sdc/driver/src/main.rs:69`), and a binding's `args`
or `args_from` is used only when the arguments equal `{}` (`main.rs:1245-1254`). The intake path
has no binding literal and always gives the request's arguments
(`crates/loom-intake-slice/src/bindings.rs:450`). For each `evidence` entry Loom reads the value
`from` names (the
exit code, or a JSON pointer into the report on standard output), looks it up in `results`, and
submits one record of that kind about the entry's `subject`, with the binding's `producer`, the
observation's instant and the `upstream` artifacts' revisions. A value the table does not list
produces no record and the step is refused. A program never names a revision, a producer, an
instant or an evidence result.

The port admits the host's runner at construction through a public function of
`crates/loom-intake-slice`, new in this story. No runner is refused `NoRunner`. A runner that
launches through `local` for a reason other than `UNCONFINED_FLAG_REASON` is refused
`CommandUnconfined`. A `local` runner with that reason, and a runner that is not `local`, are
admitted. The members of a `loom.bindings/1` document are the lists `TOP`, `ACTION`, `EXECUTOR`
and `ENTRY` of the decoder (`crates/loom-bindings/src/decode.rs:14-41`, which refuses any other
member `UndeclaredMember`) and the structs `BindingsDocument`, `BoundAction`, `ExecutorSpec` and
`EvidenceEntry` of `ess/domains/bindings.yaml` (`:189`, `:162`, `:37`, `:89`); at `aa943153`
none of them has an opt-in member, so the reason on the host's runner is the only opt-in for the
port. The `NoRunner` that the admission gives says that the host gave the port no runner, and
names neither `--unconfined-bindings` nor `unconfined_bindings: true`; `select_runner` keeps its
own `NoRunner` text, which names both. The port then calls
`preflight_child` on the case's workspace root and passes its refusals on, so a runner that keeps
the trait's default, such as `UnconfinedRunner`
(`crates/loom-intake-slice/src/executor.rs:1165-1200`), is refused `NoRunner` at construction.

Each bound program's `ChildCommand` is built by a second public function of
`crates/loom-intake-slice`, new in this story, which holds the body of today's
`CommandRunner::child` (`crates/loom-intake-slice/src/bindings.rs:317-361`): the binding's argv,
the arguments as JSON on standard input, an environment that holds `PATH` alone at the host's
value, the binding's timeout, and the case's workspace root as the start directory. Under a runner
that is not `local` the program file is mounted read-only at its own path when that path is
absolute and outside `/usr`, `/bin`, `/lib` and `/lib64`. The function takes the argv, the
arguments, the timeout, the workspace root and the runner's `local_reason`, not the intake
slice's `ActionBinding`. When the host's `PATH` is not UTF-8 it returns the error "the host's
`PATH` is not UTF-8" (`bindings.rs:330-335`). The intake path turns that error into a refused
step, with the reason `<program> was not started: the host's PATH is not UTF-8` and a run that
goes on (`bindings.rs:491-494`). The port answers the same: a refused step with that reason, no
program started and nothing recorded.

The blob is the file `blobs/<sha256>` in the store directory. `crates/loom-bindings` writes it
through a temporary file and a rename, and it holds standard output followed by standard error.

`CommandRunner`, the runner of the `run --action-bindings` path, uses both functions, so the
opt-in rule and the mount rule each have one definition. `CommandRunner` stays `pub(crate)` and
`select_runner` stays private. The file opt-in `unconfined_bindings: true` stays with
`select_runner` alone. `select_runner` keeps its order: the flag's reason on the host's runner
first, then the file opt-in, then the public admission. The file opt-in wins today over a host
runner that is not `local` (`bindings.rs:375-382`), and calling the admission first would admit
that runner instead. The tests of wave 2026-11-21-w6 are not changed by this story.

A run configuration file given to `run --action-bindings` is run by `crates/loom-intake-slice`.
A `program` binding of a `loom.bindings/1` document is run by the port in `crates/loom-bindings`;
until this story that crate only decodes and loads the document and starts no process. Decided by
session `sdc` on 2026-11-21.

The record's subject revision is the subject's revision in the case after the step's revision
moves. In this story nothing moves, so it is the revision the case held when the program started;
`story:bound-artifact-revisions` adds the moves and writes them and the step's evidence in one
store write.

## Why

Loom turns a bound command into evidence only from a report that itself lists `{kind, result}`
(`report` at `crates/loom-intake-slice/src/bindings.rs:604-650`, `submit` at `:653-684`), so the
program decides its own evidence result, and no production code implements `EvidenceAdapter`
(`crates/loom-commission/src/ports/evidence.rs:49-52`; the only `impl` is in a test). The SDC
driver has the table as data (`~/beyond10x/sdc/driver/src/main.rs:1441-1462`: exit code or `*` to
result, per kind for several kinds) and that is what lets a plain tool such as `ess specify
validate` produce `valid` or `invalid` without a wrapper. `~/beyond10x/sdc/docs/boundary.md` row
19: the mechanism is runtime, the tables are application data. The SDC's `stories.open` takes its
list of stories from an `args_from` program
(`~/beyond10x/sdc/driver/examples/sdc/sdc-epic.bindings.json:63-66`).

## ESS first

`ess/domains/bindings.yaml` first: `ProgramRun` (action, exit, blob digest, timed out),
`RecordedEvidence` and the refusals `ResultNotInTable`, `ReportUnreadable`, `ArgumentsUnreadable`
and `RunsSpent`. On that commit `task drift` fails, and
`crates/loom-bindings/tests/bound_program.rs` does not compile.

The refusals `NoRunner` and `CommandUnconfined` are declared already, as variants of
`ConfinementRefusal` in `ess/intake/domains/confinement.yaml:14` (wave 6 added `NoRunner`). The
port reuses them and adds no variant, so the ESS change stays as written. The two public
functions of `crates/loom-intake-slice` are Rust functions, as `UNCONFINED_FLAG_REASON` is:
wave 6 declared no ESS item for it, and none is declared for them.

## Acceptance

Each with a fixture protocol of two artifacts and stub programs, no model. A line that starts a
process gives the port an `EngineRunner` on `local` opened with `UNCONFINED_FLAG_REASON`, as
`crates/loom-intake-slice/tests/bound_command_confined.rs:233-258` does, and its test is ignored
off Linux, as wave 6's are. A line that reads the `ChildCommand` the port builds gives the port a
recording runner: a `TestRunner` that answers `local_reason` with what the test gives it (`None`
unless the line says otherwise), keeps the `ChildCommand` it is given, starts nothing and is
built as `Answering` is (`bound_command_confined.rs:340`). The tests reach the port through
`b10x_loom_commission::runtime::run_until_blocked` (`crates/loom-commission/src/runtime.rs:1194`),
which is public: a scripted executor proposes the action with the request's arguments, as
`crates/loom-connectors/tests/adversary.rs:427-461` does for `ConnectorEffects`, with
`ScriptedExecutor` and `StaticAuthorityProvider` of `b10x-loom-commission-testkit`, a
dev-dependency the crate gains, and a `CanonGovernor` over a `MemoryCaseStore` in place of that
test's `FakeGovernor`, because the port records into the store. `AdmittedRequest::new` is
`pub(crate)` (`crates/loom-commission/src/ports/effect.rs:113`), so no test builds a request
itself. Each line ends with its
test, a function of `crates/loom-bindings/tests/bound_program.rs` (red before the story: the file
does not exist), or with the command that shows it:

- Exit 0 with `results: {"0": valid, "*": invalid}` records `valid`.
  Test: `exit_zero_records_the_result_the_table_lists_for_zero`.
- Exit 3 with the same table records `invalid`.
  Test: `an_exit_code_the_table_does_not_list_falls_to_star`.
- A timeout with a table that lists the key `timeout` records that entry's result.
  Test: `a_timeout_records_the_result_of_the_timeout_key`.
- A timeout with a table that lists `*` and no `timeout` key records the `*` result.
  Test: `a_timeout_without_a_timeout_key_falls_to_star`.
- A timeout with a table that lists neither key is refused `ResultNotInTable`.
  Test: `a_timeout_with_neither_key_is_refused_result_not_in_table`.
- A step refused `ResultNotInTable` leaves no record in the store.
  Test: `a_value_the_table_does_not_list_records_nothing`.
- `from: /status` reads the member `status` of the JSON report.
  Test: `a_pointer_reads_the_named_member_of_the_report`.
- A report that is not one JSON object is refused `ReportUnreadable`.
  Test: `a_report_that_is_not_one_json_object_is_refused_report_unreadable`.
- A report larger than 1 MiB is refused `ReportUnreadable`.
  Test: `a_report_larger_than_one_mib_is_refused_report_unreadable`.
- A report that lacks the pointer is refused `ReportUnreadable`.
  Test: `a_report_without_the_pointer_is_refused_report_unreadable`.
- A step refused `ReportUnreadable`, for each of the three causes, leaves no record in the store.
  Test: `a_report_refused_unreadable_records_nothing`.
- An entry list with two kinds records one record per kind.
  Test: `an_entry_list_of_two_kinds_records_one_record_per_kind`.
- The program of an entry list with two kinds starts once.
  Test: `an_entry_list_of_two_kinds_starts_the_program_once`.
- Each of those two records takes its result from its own kind's table.
  Test: `each_record_takes_its_result_from_its_own_kinds_table`.
- A bound program receives the arguments as one JSON object on standard input. The stub is
  `/usr/bin/cat`, whose standard output is its standard input; the test reads the blob.
  Test: `a_bound_program_receives_the_arguments_as_one_json_object_on_standard_input`.
- A bound program starts with `PATH` alone in its environment, at the host's value. The stub is
  `/usr/bin/env`, which is not a shell and so adds no `PWD`, `SHLVL` or `_` (wave 6's test,
  `bound_command_confined.rs:134-162`, reads `/proc/$$/environ` from a shell script instead); the
  blob equals `PATH=<the host's PATH>` and a newline.
  Test: `a_bound_program_starts_with_path_alone_in_its_environment`.
- A request whose `arguments` is a non-empty object, for an action whose binding has a literal
  `arguments`, gives the program the request's arguments. The stub is `/usr/bin/cat`; the test
  reads the blob.
  Test: `the_requests_arguments_win_over_the_bindings_literal_arguments`.
- A request whose `arguments` is the empty object `{}`, for an action whose binding has a
  literal `arguments`, gives the program the literal. The stub is `/usr/bin/cat`; the test reads
  the blob.
  Test: `the_bindings_literal_arguments_are_used_when_the_request_has_none`.
- A request whose `arguments` is JSON `null`, for an action whose binding has a literal
  `arguments`, gives the program `null`. The stub is `/usr/bin/cat`; the test reads the blob.
  Test: `a_null_request_argument_reaches_the_program_as_null`.
- With `arguments_from`, the program receives the object the argv printed.
  Test: `arguments_from_output_is_the_programs_standard_input`.
- An `arguments_from` program that exits non-zero is refused `ArgumentsUnreadable`.
  Test: `an_arguments_from_program_that_exits_non_zero_is_refused_arguments_unreadable`.
- An `arguments_from` program that prints anything but one JSON object is refused
  `ArgumentsUnreadable`.
  Test: `arguments_from_output_that_is_not_one_json_object_is_refused_arguments_unreadable`.
- In each of those two refusals the bound program does not start (its marker file is absent).
  Test: `a_refused_arguments_from_starts_no_bound_program`.
- The record's subject revision is the revision the case holds for the subject.
  Test: `the_records_subject_revision_is_the_revision_the_case_holds`.
- The record's instant is the observation's.
  Test: `the_records_instant_is_the_observations`.
- The record's producer is the binding's.
  Test: `the_records_producer_is_the_bindings`.
- The record's `facts` carry the member `upstream_revisions`, which maps each artifact the entry
  lists in `upstream` to the revision the case holds for it (the member Canon's `canon-evidence/1`
  record carries, as `crates/loom-governor/tests/adversary_w20261121w1_u3_pass2.rs:170` writes it).
  Test: `the_records_facts_carry_the_revision_of_each_upstream_artifact`.
- A report with a `producer`, `observed_at` and `subject_revision` member gives the same record as
  a report without them.
  Test: `producer_observed_at_and_subject_revision_in_the_report_change_nothing`.
- The record cites the observation.
  Test: `the_record_cites_the_observation`.
- The observation names the SHA-256 of the blob.
  Test: `the_observation_names_the_sha256_of_the_blob`.
- The blob is the file `blobs/<sha256>` in the store directory.
  Test: `the_blob_is_the_file_blobs_sha256_in_the_store_directory`.
- The blob holds standard output followed by standard error.
  Test: `the_blob_holds_standard_output_then_standard_error`.
- Two runs with the same output leave one file in the blob directory.
  Test: `two_runs_with_the_same_output_leave_one_blob_file`.
- After a run, every file in the blob directory is named by the SHA-256 of its content, so no
  temporary file is left.
  Test: `a_run_leaves_only_sha256_named_files_in_the_blob_directory`.
- `crates/loom-governor` is not changed by this story. Command:
  `git diff --stat $(git merge-base main HEAD)..HEAD -- crates/loom-governor` prints nothing.
- An action that has already run its `budget.max_runs` times in this case is not performed (its
  marker file is absent).
  Test: `an_action_at_its_max_runs_is_not_performed`.
- That action is reported `RunsSpent`.
  Test: `an_action_at_its_max_runs_is_reported_runs_spent`.
- The count is the number of observations the case's store holds for that action from this effect
  port; observations of another action and of another source do not count.
  Test: `the_run_count_is_the_observations_of_that_action_from_this_port`.
- The count survives a reopen of the store.
  Test: `the_run_count_survives_a_reopen_of_the_store`.
- The action that the four run-count tests drive is a writer, so they stay green when
  `story:run-caps-per-arrival` exempts observations. The four tests build their protocol with
  one shared helper, and this test parses that helper's protocol and asserts that the `effect`
  of the capped action (`model::Action::effect` of `b10x-canon`) is `write`.
  Test: `the_run_count_tests_drive_an_action_whose_effect_class_is_write`.
- The effect port reports `performs` for an action bound to `program`.
  Test: `performs_is_true_for_an_action_bound_to_program`.
- The effect port reports no `performs` for an action bound to `agent`, `person`, `open` or
  `copy`.
  Test: `performs_is_false_for_an_action_bound_to_any_other_executor`.
- A host that gives no runner is refused `NoRunner` at construction.
  Test: `a_host_with_no_runner_is_refused_no_runner_at_construction`.
- With no runner, no program is started (its marker file is absent).
  Test: `a_host_with_no_runner_starts_no_program`.
- With no runner, nothing is recorded (the store holds no observation and no record).
  Test: `a_host_with_no_runner_records_nothing`.
- The `detail` of the admission's `NoRunner` refusal names neither `--unconfined-bindings` nor
  `unconfined_bindings`.
  Test: `the_no_runner_detail_names_no_flag_and_no_file_member`.
- A host runner that keeps the trait's default `run_child` and `preflight_child` is refused
  `NoRunner` at construction. The runner is `UnconfinedRunner`: its `impl` in
  `crates/loom-intake-slice/src/executor.rs:1165-1200` defines `backend` and `run` only, so it
  answers `NoRunner` to both, and it is not `local`, so the admission admits it and the
  `preflight_child` of the case's workspace root refuses it.
  Test: `a_host_runner_with_the_default_run_child_is_refused_no_runner_at_construction`.
- With that runner, no program is started.
  Test: `a_host_runner_with_the_default_run_child_starts_no_program`.
- A `local` runner whose reason is not `UNCONFINED_FLAG_REASON` is refused `CommandUnconfined` at
  construction.
  Test: `a_local_runner_with_another_reason_is_refused_command_unconfined`.
- With that runner, no program is started.
  Test: `a_local_runner_with_another_reason_starts_no_program`.
- A `local` runner whose reason is `UNCONFINED_FLAG_REASON` is admitted at construction.
  Test: `a_local_runner_with_the_flags_reason_is_admitted_at_construction`.
- With that runner the program runs and its result is recorded.
  Test: `a_local_runner_with_the_flags_reason_runs_the_program`.
- The port calls `preflight_child` on the case's workspace root: the recording runner is given
  that root.
  Test: `preflight_child_is_called_with_the_workspace_root`.
- A refusal of `preflight_child` is passed on unchanged at construction.
  Test: `a_preflight_refusal_is_passed_on_unchanged_at_construction`.
- The child starts in the case's workspace root: the `ChildCommand` the recording runner receives
  has that root as its `directory()`.
  Test: `the_child_starts_in_the_cases_workspace_root`.
- Under a runner that is not `local`, a program at an absolute path outside `/usr`, `/bin`, `/lib`
  and `/lib64` is mounted read-only at its own path: the `ChildCommand` the recording runner
  receives holds that path in `host_paths()`.
  Test: `a_program_outside_the_system_view_is_mounted_at_its_own_path_under_a_non_local_runner`.
- Under a runner that is not `local`, a program under `/usr` is not mounted.
  Test: `a_program_under_usr_is_not_mounted`.
- Under a recording runner that answers `local_reason` with `UNCONFINED_FLAG_REASON`, no program
  is mounted: `host_paths()` is empty, since `local` refuses a host path.
  Test: `no_program_is_mounted_under_a_local_runner`.
- When the host's `PATH` is not UTF-8, the step is refused with the reason `<program> was not
  started: the host's PATH is not UTF-8`. The test runs the test binary again as a child process
  with a `PATH` the parent sets from bytes (`std::os::unix::ffi::OsStrExt::from_bytes`), as
  `crates/loom-intake-slice/tests/adversary_w20261021w6_u3.rs:73-103` does with an empty one.
  Test: `a_non_utf8_path_refuses_the_step_with_the_reason_the_outcome_states`.
- With that `PATH`, no program starts (its marker file is absent), the child built the same way.
  Test: `a_non_utf8_path_starts_no_program`.
- With that `PATH`, nothing is recorded (no observation and no record), the child built the same
  way.
  Test: `a_non_utf8_path_records_nothing`.
- The wave 6 path behaves as before: the test files of `crates/loom-intake-slice` that go through
  the bound-command path pass. At `aa943153` exactly five set `unconfined_bindings` or build
  `ActionBindings`: `git grep -l -e unconfined_bindings -e ActionBindings --
  crates/loom-intake-slice/tests` prints `bound_command`, `bound_command_confined`,
  `security_w20261121w6_u1`, `security_w20261022w2_u1` and `adversary_w20261021w6_u3`. Command,
  exits 0:
  ```
  cargo test -p b10x-loom-intake-slice --locked --test bound_command \
    --test bound_command_confined --test security_w20261121w6_u1 \
    --test security_w20261022w2_u1 --test adversary_w20261021w6_u3
  ```
- That file and every other test file `crates/loom-intake-slice` holds before this story are
  unchanged; the unit may add a test file. Command: `git diff --stat --diff-filter=MDR
  $(git merge-base main HEAD)..HEAD -- crates/loom-intake-slice/tests` prints nothing.
- `CommandRunner` stays `pub(crate)` and `select_runner` stays private. Command:
  `git grep -nE 'struct CommandRunner|fn select_runner' -- crates/loom-intake-slice/src/bindings.rs`
  prints the first with `pub(crate)` and the second with no `pub`.
- The generated code matches the specification. Command: `task drift` exits 0 after
  `task generate`; on the ESS-first commit it fails.
- The new test file runs. Command:
  `cargo test -p b10x-loom-bindings --locked --test bound_program` exits 0 and reports a count of
  passed tests greater than 0.

The mount under a real `linux` adapter is not run by this story: it needs a delegated cgroup scope
that the suite lacks outside `support::delegated`. Wave 6's
`under_linux_a_bound_command_reads_no_host_file_and_writes_its_workspace` runs a mounted script
in such a scope or skips, naming what is missing. Read in review, with no test: that
`CommandRunner` calls both new functions, so each rule has one definition.

## Out of scope

Moving artifact revisions after the program ran (`story:bound-artifact-revisions`).
Counting runs per arrival, and the rule that an observation (an action whose effect class is not
`write`) is never refused `RunsSpent` and may state no `max_runs`: both land in
`story:run-caps-per-arrival`, which depends on this story. Agent, person, open and copy executors.
Report checks that name a tool (a spec digest, a clean tree).

Not in this story: a bound program still gets `PATH` alone, no toolchain or cache root, no
writable directory beyond `target/` under the `linux` adapter, and no egress. A product tree's own
test command (`cargo test`, `task check`, `npm test`) therefore does not run through a binding
yet. A neighbour story, to be drafted for wave 8 planning, lets a binding declare its program's
environment, read-only roots and writable directories; egress apertures stay out of both stories.
A host that needs the exit code and output of its own command today uses the SDK's runner:
`EngineRunner::open(&EngineConfinement)`, then
`run_child(&ChildCommand) -> Result<TestExecution, ConfinementError>`. The result holds
`exit_code`, `timed_out`, the output tails and each stream, and the call records no observation
and no evidence. Under `linux` each stream is kept whole up to 1 MiB; the `local` route admits
its profile with no bounds. Cited: `EngineRunner::open` and `run_child` at
`crates/loom-intake-slice/src/confinement/engine.rs:89,134`, `TestExecution` at
`crates/loom-intake-slice/src/confinement.rs:197-207`, the `linux` 1 MiB bound at
`confinement.rs:3100`, the `local` profile with no bounds at `engine.rs:308-309`, and the
re-export at `crates/loom-sdk/src/lib.rs:120,123`. Decided by session `sdc` on 2026-11-21.

## Scope

Derived 2026-11-21 by `story-scoper`, re-derived after wave 2026-11-21-w6. Every line is **cited**
(read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings` (exists since `story:bindings-document`; `src/` holds
  only `decode.rs`, `load.rs`, `tree.rs`, `lib.rs`): the `program` effect port, result-table
  lookup, blob, run count: cited, Outcome names the crate
- **Files:** `crates/loom-bindings/tests/bound_program.rs` (does not exist yet): cited, ESS first
- **Files:** `ess/domains/bindings.yaml` (246 lines, none of the six new names present) gains
  `ProgramRun`, `RecordedEvidence`, `ResultNotInTable`, `ReportUnreadable`, `ArgumentsUnreadable`,
  `RunsSpent`: cited, ESS first
- **Files:** `crates/loom-bindings/Cargo.toml` (`:14` `b10x-canon`, last line `:17`) gains path deps
  `b10x-loom-intake-slice`, `b10x-loom-commission`, `b10x-loom-governor`, and `sha2`, and the
  dev-dependency `b10x-loom-commission-testkit` for the tests: inferred, the story names no
  manifest
- **Files:** `generated/rust/loom/` (contract digest on line 2 of `Cargo.toml`, `src/bindings.rs`,
  `src/lib.rs`), `website/data/ess/loom-bindings.domain-graph.json`,
  `website/docs/reference/ess/loom-bindings.md`, regenerated by `task generate`: inferred, from the
  `bindings-document` scope
- **Files:** `Cargo.lock` (the `b10x-loom-bindings` package block): inferred, from the new deps
- **Files:** `crates/loom-intake-slice/src/bindings.rs`: two functions made public, the admission
  of a host runner for bound programs (the host-runner rules of `select_runner`, `:366`) and the
  build of a bound program's `ChildCommand` (the body of `CommandRunner::child`, `:317-361`);
  `CommandRunner::child` and `select_runner` call them: cited, decided by session `sdc` on
  2026-11-21
- **Symbols:** `TestRunner::run_child` (`confinement.rs:103`, default refuses `NoRunner`),
  `TestRunner::preflight_child` (`:112`), `ChildCommand` (`:1166`; `new` `:1204`, `with_stdin`
  `:1443`, `with_timeout` `:1432`, `with_environment` `:1333`), `TestExecution` (`:197`),
  `INHERITED` (`bindings.rs:72`), `EffectPort` (`loom-commission/src/ports/effect.rs:41`),
  `submit_evidence` (`ports/evidence.rs:116`), `CanonGovernor::revisions`
  (`loom-governor/src/lib.rs:1099`), `try_observations` (`:1133`): cited, all used and none changed
- **Also likely:** none beyond the files above. `crates/loom-intake-slice/src/bindings.rs` is a
  Files bullet, not a guess. Decided by session `sdc` on 2026-11-21 (option B, replacing its
  earlier decision that the crate is not changed): the opt-in rule and the mount rule get one
  definition there; `CommandRunner` `:255` stays `pub(crate)` and `select_runner` `:366` stays
  private: cited
- **Documents:** `website/docs/guides/bindings.md:12`, `AGENTS.md:1180` (`## Bindings document`)
  and `CHANGELOG.md` `[Unreleased]` all say the crate "executes nothing, starts no process"; the
  `crates/loom-bindings/src/lib.rs:11` doc says the same. The new port starts a process, so all
  four change: inferred. `CHANGELOG.md` also gets one `Added` bullet for the two public
  functions of `crates/loom-intake-slice`: cited, decided by session `sdc` on 2026-11-21
- **Confidence:** high for the surface, the ESS and test files and `bindings.rs` (the story, the
  ruling of session `sdc` and the tree name them). Medium for the manifest and docs lines. No
  choice is open: session `sdc` decided on 2026-11-21 (option B) that the port takes the host's
  runner, admits it and builds each `ChildCommand` through two public functions of
  `crates/loom-intake-slice/src/bindings.rs`.
- **Would collide with:** `crates/loom-bindings/**`, `ess/domains/bindings.yaml`,
  the line-2 digest of every file under `generated/rust/loom/` (any unit that changes an ESS
  domain), `Cargo.lock`: cited for the first two, inferred for the rest. The port reads the
  public `ChildCommand` and `run_child` of `crates/loom-intake-slice/src/confinement.rs` and
  changes neither, so a unit that changes them breaks this one: inferred.
  `crates/loom-intake-slice/src/bindings.rs` (any unit that changes `CommandRunner`,
  `select_runner` or the two new functions): inferred. The wave's siblings:
  `story:engineering-protocols-dependency-removed` shares `Cargo.lock`, `AGENTS.md` (other
  sections) and `CHANGELOG.md`; `story:decision-request-entity` shares `Cargo.lock`,
  `CHANGELOG.md` and the line-2 digest of files under `generated/rust/loom/`. Each is merged by
  keeping both sides, or by running the generator on the merged sources: cited, from their Scope
  sections
- **Safety fact:** `crates/loom-governor` needs no change. `try_observations` returns the whole
  store, and `ObservationData` has no case id
  (`generated/rust/commission/src/responsibility.rs:2329-2340`), so the port puts case and action
  in the payload and filters on its own `source`. That `source` must differ from
  `"loom/action-command"` (`bindings.rs:84`). Step 2, unproven.
