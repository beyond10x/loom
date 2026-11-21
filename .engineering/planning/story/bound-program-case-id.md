---
format: aep.planning-md/3
id: story:bound-program-case-id
kind: story
status: active
title: A bound program and its arguments_from program read the id of the case they run for from LOOM_CASE, set by the port alone
summary: The environment of a program the effect port starts is PATH and LOOM_CASE; no request, report, output or bindings document sets it; the intake path keeps PATH alone
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:bound-program-result-table
scope:
- confidence: cited
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings
- confidence: inferred
  path: crates/loom-bindings/src/child.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_case_id.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/bindings.md
- confidence: inferred
  path: website/docs/reference/ess/loom-bindings.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T21:22:29Z", actor: "agent:sdc", revision: 6, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w8"}
- {from: "proposed", to: "active", at: "2026-11-21T21:22:29Z", actor: "agent:sdc", revision: 7, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w8"}
---
## Outcome

Every program the program effect port of `b10x-loom-bindings` starts, the bound program and its
`arguments_from` program alike, has in its environment one variable beside `PATH`: `LOOM_CASE`
(`CASE_VARIABLE`), whose value is the id of the case the port was built for, the `case` given to
`ProgramEffects::new`, as the governor holds it (`CaseId.0`, a child as `<parent id>.<name>`).
Its environment is these two variables and nothing else: `PATH` at the host's value, as today,
and `LOOM_CASE` set by the port after every other variable of the child, so no other source of
a variable can give it another value. The arguments still reach the program as one JSON object
on standard input only, unchanged.

The value comes from the port alone. A request for another case is refused before anything
starts, as today (`crates/loom-bindings/src/program.rs:155-160`); a port built for a case the
governor does not hold starts nothing (`program.rs:162`). No member of a request's arguments,
of an `arguments_from` program's output, of a program's report or of the bindings document, and
no variable of the host process's environment, sets or changes `LOOM_CASE`. A case id longer
than 4096 bytes refuses the step before anything starts, with the reason `<program> was not
started: the case id is longer than 4096 bytes`, and nothing is recorded.

The child is built by one public function of `b10x-loom-bindings`, `bound_case_child(argv,
arguments, timeout, workspace, local_reason, case)`, which calls
`b10x_loom_intake_slice::bindings::bound_program_child` and adds the variable through
`ChildCommand::with_environment`. It is the only caller of `bound_program_child` in
`crates/loom-bindings/src`, so every port of the crate that starts a program, the `open` port's
`arguments_from` included once it runs one, gives the case id. The environment is described by
a new type, `loom.bindings.ProgramEnvironment` (`path`, `loom_case`), from which the function
builds the variables. The intake slice's `run --action-bindings` path is not changed: its
commands keep `PATH` alone.

## Why

Runbook's driver gives every bound program `SDC_CASE`, the case id (`~/beyond10x/sdc/driver/src/
main.rs:1479-1489`). Its `list-stories` tool, the `arguments_from` of `sdc-epic`'s `stories.open`
(`driver/examples/sdc/sdc-epic.bindings.json:63-66`), runs under `set -u`
(`driver/examples/sdc/tools/sdc-tool.sh:5`) and reads the epic out of the case id
(`sdc-tool.sh:328-331`), so that opening fails without it. Loom gives a
bound program `PATH` alone and its arguments on standard input
(`crates/loom-intake-slice/src/bindings.rs:75`, `:403-450`;
`website/docs/guides/bindings.md:216-218`; `AGENTS.md:1244-1247` builds the child as
`bound_program_child` does). No story plans a per-case value:
`story:bound-program-declares-its-environment` gives literal values from the document only.

The rule's purpose is that nothing but the host steers a program through its environment: no
document, request or report names a variable. A variable with a fixed name, whose value the port
takes from the case it was built for, keeps that purpose: the host process gives `PATH`, the port
gives `LOOM_CASE`, and neither name nor value is data. Rejected: a member of the standard-input
document. That document is the request's arguments as written, chosen by the proposer (an
executor, possibly a model), and `null` or any other non-object value reaches the program as
written (`bound_program.rs`, `a_null_request_argument_reaches_the_program_as_null`). A host value
there would either rewrite the proposer's object, so a member of the same name must be overwritten
or refused, or wrap it (`{arguments, case}`), which changes what every program bound since wave
2026-11-21-w7 reads. Rejected: an argv placeholder such as `{case}`: `argv` is literal
(`ess/domains/bindings.yaml:43-44`), and a placeholder is bindings data that steers the start.
Runbook adapts its tools to this contract, so the name need not be `SDC_CASE`.

## ESS first

`ess/domains/bindings.yaml` first: `loom.bindings.ProgramEnvironment` (`path`: the host's
`PATH`; `loom_case`: the id of the case the port was built for), with the rule that no other
variable reaches a program the port starts. `story:bound-program-retry-records`, built in the
same wave, adds `loom_retry_feedback` to this type; the unit that merges second adds its member
to the type the first merged. No `ProgramRefusalReason` is added: the over-long case id is a
step refusal of the kind `program.rs:603-608` gives. On that commit `task drift` fails, and
`crates/loom-bindings/tests/bound_case_id.rs` does not compile.

## Acceptance

A fixture protocol of two artifacts with a `program` action `gate.check` bound to a stub, as in
`crates/loom-bindings/tests/bound_program.rs`; cases opened as `case-1`, as
`case-1.north.2` and as another protocol's case. No model, no network. A line that reads the
`ChildCommand` uses a recording runner that starts nothing and keeps the children it is given; a
line that starts a process runs `/usr/bin/env` under an `EngineRunner` on `local` with
`UNCONFINED_FLAG_REASON` and is ignored off Linux. Each test is a function of
`crates/loom-bindings/tests/bound_case_id.rs`, red before the story (the file does not exist),
and ends within 60 s.

- The bound program's child has `LOOM_CASE` = `case-1` and `PATH` = the host's value in
  `environment().set`, and no third variable.
  Test: `a_bound_programs_environment_is_path_and_loom_case`.
- The bound program's child allows no inherited host variable: `environment().allow` is empty.
  Test: `a_bound_program_inherits_no_other_host_variable`.
- The `arguments_from` child of the same step has the same two variables and no third.
  Test: `an_arguments_from_program_is_given_loom_case`.
- A case opened as `case-1.north.2` gives `LOOM_CASE` = `case-1.north.2`.
  Test: `a_child_case_id_reaches_the_program_as_written`.
- `/usr/bin/env` as the bound program prints exactly the lines `LOOM_CASE=case-1` and
  `PATH=<the host's PATH>`, compared as a set; the test reads the blob.
  Test: `a_bound_program_reads_its_case_id_from_loom_case`.
- Security: a request whose arguments hold `{"LOOM_CASE": "case-2"}` gives `LOOM_CASE` = `case-1`.
  Test: `an_argument_named_loom_case_does_not_change_the_variable`.
- Security: those arguments still reach standard input as written.
  Test: `an_argument_named_loom_case_reaches_standard_input_as_written`.
- Security: an `arguments_from` output `{"LOOM_CASE": "case-2"}` gives the bound program
  `LOOM_CASE` = `case-1`.
  Test: `arguments_from_output_named_loom_case_does_not_change_the_variable`.
- Security: a first step whose report holds `"LOOM_CASE": "case-2"` leaves `LOOM_CASE` =
  `case-1` on the second step. Test: `a_report_cannot_change_the_variable_of_the_next_step`.
- Security: the test binary, run again as a child with `LOOM_CASE=host-value` in its own
  environment (as `crates/loom-intake-slice/tests/adversary_w20261021w6_u3.rs:73-103` runs it),
  gives the bound program `LOOM_CASE` = `case-1`.
  Test: `a_loom_case_in_the_hosts_environment_does_not_reach_the_program`.
- Security: a bindings document whose `program` executor names `environment: {LOOM_CASE: x}` is
  refused `UndeclaredMember` at decoding, naming `executor.environment`.
  Test: `a_bindings_document_cannot_name_the_variable`.
- Security: a request for `case-2` to a port built for `case-1` is refused and the recording
  runner receives no child. Test: `a_request_for_another_case_starts_no_child`.
- Security: a port built for a case the governor does not hold gives the recording runner no
  child. Test: `a_port_for_a_case_the_governor_does_not_hold_starts_no_child`.
- A case id of 4097 bytes refuses the step with the reason the Outcome states.
  Test: `a_case_id_longer_than_4096_bytes_refuses_the_step`.
- With that case id the recording runner receives no child and the store holds no observation.
  Test: `a_case_id_longer_than_4096_bytes_starts_and_records_nothing`.
- `bound_program_child(` is called in `crates/loom-bindings/src` only inside `bound_case_child`;
  the test reads every `.rs` file under that directory.
  Test: `every_child_of_the_crate_is_built_by_bound_case_child`.
- The two tests that held `PATH` alone now hold `PATH` and `LOOM_CASE`:
  `a_bound_program_starts_with_path_alone_in_its_environment` (`bound_program.rs:686-705`) is
  renamed `a_bound_program_starts_with_path_and_loom_case_alone_in_its_environment`, and the
  expected sets of `security_w20261121w7_u2.rs:540-545` and `:578` gain `LOOM_CASE`. Command:
  `cargo test -p b10x-loom-bindings --locked --test bound_program --test security_w20261121w7_u2`
  exits 0.
- The intake slice's commands keep `PATH` alone. Command:
  `git diff --stat $(git merge-base main HEAD)..HEAD -- crates/loom-intake-slice
  crates/loom-governor` (one line) prints nothing.
- The guide and AGENTS.md state the variable. Command:
  `git grep -c LOOM_CASE -- website/docs/guides/bindings.md AGENTS.md` prints a count of at
  least 1 for each file.
- The generated code matches the specification. Command: `task drift` exits 0 after
  `task generate`; on the ESS-first commit it fails.
- The new test file runs. Command: `cargo test -p b10x-loom-bindings --locked --test bound_case_id`
  exits 0 and reports a count of passed tests greater than 0.

## Out of scope

The action id, the case directory, the timeout and the other per-step values Runbook's driver
sets (`SDC_ACTION`, `SDC_CASE_DIR`, `SDC_NOT_CALLED`, `SDC_TIMEOUT_S`, `SDC_VIRTUAL_TIMEOUT`,
`main.rs:1479-1514`). The retry feedback (`story:bound-program-retry-records`). Variables a
binding declares (`story:bound-program-declares-its-environment`, which must keep the two port
variables out of what a document may declare). The intake slice's `run --action-bindings` path.
Running the `arguments_from` of an `open` binding: `story:child-case-runtime` or
`story:tree-run-command`, either of which builds that child with `bound_case_child`.

Runbook's side: `sdc-tool.sh` reads `LOOM_CASE` in place of `SDC_CASE` (`sdc-tool.sh:330`), and
its other tools stop reading the variables Loom does not give.

## Scope

Derived 2026-11-21 from the tree at `8a8fad3b`. Every line is **cited** (read at the `file:line`
named) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings`, the program effect port's two child builds —
  cited (`crates/loom-bindings/src/program.rs:205-214`, `:364-373`)
- **Files (ESS source):** `ess/domains/bindings.yaml`: `ProgramEnvironment` new; the header
  comment (`:1-9`) names the environment — cited for the file, inferred for the comment
- **Files (generated):** `generated/rust/loom/**` regenerated by `task generate` — cited
  (`bound-program-result-table.md:413-416`)
- **Files (source):** `crates/loom-bindings/src/program.rs`: both calls of `bound_program_child`
  become calls of `bound_case_child`; the module doc (`:9-13`) names `LOOM_CASE` — cited
- **Files (source):** `crates/loom-bindings/src/child.rs`, new: `bound_case_child`,
  `CASE_VARIABLE`, `MAX_CASE_VARIABLE_BYTES` — inferred (name guessed)
- **Files (source):** `crates/loom-bindings/src/lib.rs`: `mod child`, the `pub use` beside `:24`
  — cited
- **Files (new test):** `crates/loom-bindings/tests/bound_case_id.rs` — cited (§ ESS first)
- **Files (existing tests):** `crates/loom-bindings/tests/bound_program.rs:686-705` and
  `crates/loom-bindings/tests/security_w20261121w7_u2.rs:538-545`, `:578`, which assert `PATH`
  alone — cited
- **Files (documents):** `website/docs/guides/bindings.md:216-218` ("`PATH` alone in its
  environment"); `AGENTS.md:1244-1247` (§ Bindings document; `:2197` is the intake slice's and
  stays); `CHANGELOG.md` one `Changed`
  bullet; `website/docs/reference/ess/loom-bindings.md` regenerated — cited for the two
  hand-written pages, inferred for the changelog
- **Symbols (read, unchanged):** `bound_program_child`
  (`crates/loom-intake-slice/src/bindings.rs:414-450`), `INHERITED` (`:75`),
  `ChildCommand::environment` and `with_environment`
  (`crates/loom-intake-slice/src/confinement.rs:1471`, `:1333`), `ExecEnvironment.set` as
  built at `bindings.rs:416-438` — cited
- **Symbols (added):** `bound_case_child`, `CASE_VARIABLE` (`"LOOM_CASE"`),
  `MAX_CASE_VARIABLE_BYTES` (4096); generated `ProgramEnvironment` — inferred (names proposed)
- **Not changed:** `crates/loom-intake-slice` (its `CommandRunner::child`,
  `bindings.rs:324-337`, keeps calling `bound_program_child`, so the intake path keeps `PATH`
  alone) and `crates/loom-governor` — cited
- **Would collide with:** `story:bound-program-retry-records` on `ess/domains/bindings.yaml`,
  `crates/loom-bindings/src/program.rs` (`perform`, the child build), `src/lib.rs`, the guide,
  AGENTS.md and `generated/rust/loom/**` — inferred. `story:bound-artifact-revisions` on
  `ess/domains/bindings.yaml`, `program.rs` and `generated/rust/loom/**` — cited from its Scope
  (`bound-artifact-revisions.md:93-95`), inferred for `program.rs`. `story:child-case-runtime` on
  `crates/loom-bindings/src/lib.rs`, and on its `open` port if that port runs `arguments_from`
  through `bound_program_child`: the source test above then fails until it calls
  `bound_case_child` — inferred (`child-case-runtime.md:109-112` names the port, not its
  arguments). The test-budget unit: no file in common — cited from the brief.
  `story:record-copy-between-cases`: `ess/domains/bindings.yaml`, `src/lib.rs` — cited
  (`record-copy-between-cases.md:100-104`)
- **Safety fact:** the value is `self.case` of the port, checked equal to the request's case
  (`program.rs:155-160`) and read from the governor before any start (`program.rs:162`), so it
  names a case the governor holds and the step is for — cited, unproven against a host that builds
  the port for a case it should not drive
