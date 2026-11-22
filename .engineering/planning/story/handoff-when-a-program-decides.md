---
format: aep.planning-md/3
id: story:handoff-when-a-program-decides
kind: story
status: proposed
title: 'A writer''s handoff can depend on a fact only a program can read: handoff_when holds an argv that decides'
summary: 'handoff_when argv run like a bound program before the action: exit 0 gives the Handoff decision, another exit skips it, launch error or timeout refuses; one run-log line; security-relevant'
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:tree-run-command
- depends_on: story:bound-program-contract
- depends_on: story:child-record-in-the-run-log
- depends_on: story:tree-status-names-what-it-read
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/child.rs
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-cli/tests/tree_run.rs
- confidence: cited
  path: crates/loom-runlog
- confidence: cited
  path: crates/loom-tree/src/run.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/runlog.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T06:37:59Z", actor: "agent:loom", revision: 5}
---
## Outcome

A writer's handoff can depend on a fact only a program can read. It is offered while that program
says so, and the writer runs otherwise.

- A program binding may carry `handoff_when`, an argv held to the same program contract as any
  bound program: `program_root`, the argv rules and `InvalidArgv`.
  - It requires `handoff`, the text; without it the binding is refused `MissingMember`, naming
    `handoff`.
  - On a `person` executor it is refused `MemberNotForExecutor`.
- **When it runs.** When the deterministic selector picks the action as the next step, the
  `handoff_when` program runs first, as a child built by `bound_case_child`:
  - confined like the binding's own program, under the same runner;
  - read-only on the whole workspace, `target/` included;
  - with `PATH` and the `LOOM_*` variables only, no retry feedback, and the binding's
    `timeout_seconds`.
- **Exit 0.** No step is taken. The run stops `AwaitingPerson` with a `Handoff` decision for the
  action: `decides` is the `handoff` text and `command` is `tree run --store <store>`. This is the
  decision of `impl/tree-run-command` `db7a3d8b` (`crates/loom-tree/src/run.rs:795-830` there).
- **Any other exit.** The handoff is skipped, and the action's own program runs in the same step.
- **A launch error, a timeout or a signal** is never a skip. The run stops, refused
  `HandoffCheckFailed`, naming the action and the cause, with exit 1.
- **Run log.** Each evaluation writes one line, `HandoffChecked {action, exit?, refusal?,
  blob_digest?}`, before the stop or the step. It holds exactly one of `exit` and `refusal`. The
  `blob_digest` is the kept output, as for a program run. Reading the log replays which way the
  check went, without running it again.
- **`handoff` without `handoff_when`** keeps the meaning it has in `db7a3d8b`: offered when the
  case is otherwise stuck (`run.rs:934-941` there).
- **A binding with `handoff_when`** gives its handoff only through the check. At a stuck stop the
  check runs the same way: exit 0 gives the `Handoff`, any other exit leaves the stop `Stuck`.
- **Format.** `loom.bindings/1` stays. The decoder's action members (`ACTION`,
  `crates/loom-bindings/src/decode.rs:22-34`) gain `handoff_when`. Today it is refused
  `UndeclaredMember` (`decode.rs:843-858`), which stays the rule: a Loom from before this change
  refuses a document that uses it, and a document without it reads unchanged.

Alternative (b), cleaner in launches: `exits.handoff: [codes]` on the action's own program
(beside `error` and `not_called`, `ess/domains/bindings.yaml:171-177`).

- The writer checks first and exits with a listed code, which gives the `Handoff`. There is no
  second launch and no new log kind.
- The cost: the writer's own run is recorded and counts against `max_runs` unless that code is
  also `not_called`, and the check lives inside the writer.
- Runbook asked for the separate argv, so (a) is the default.

Security-relevant: yes. This is a second confined launch per step, whose exit decides whether a
person is asked. The unit gets a security review.

## Why

Five of Runbook's bindings hand `amend.upstream` off only while a finding waits for a person:
intake, direction, spec, plan and story (Runbook `bindings/PENDING.md:125-133`). Each uses the
argv `[tools/sdc-tool.sh, findings-wait, --action, amend.upstream]`, where exit 0 means a finding
waits. Today `handoff_when` is refused at load, so every such binding fails `tree open`.

## ESS first

`ess/` changes first, in its own commit, then `task drift` and `task docs-generate`:

- `bindings.yaml`: `BoundAction` gains `handoff_when: List<String>`, empty when absent.
- `runlog.yaml`: `RunLogEvent` gains `HandoffChecked`.
- `tree.yaml`: `TreeRefusalReason` gains `HandoffCheckFailed`.

The run-log decoder is closed, so an older reader refuses the new line. This is handled as in
`story:child-record-in-the-run-log`.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no network, no model
call. The check programs are fixture scripts. The tests share one tree fixture and do not run a
whole tree per test where a store copy does. A `linux` test runs in a delegated scope or skips,
naming what is missing.

- A check that exits 0: the run stops `AwaitingPerson` with the `Handoff` decision of the action,
  the writer does not run, and the log holds one `HandoffChecked` line with `exit: 0`. Test:
  `a_check_that_exits_0_hands_the_action_off`.
- A check that exits 1: the writer runs in that step, and the log holds `HandoffChecked` with
  `exit: 1` before the writer's `ActionStarted`. Test: `a_check_that_exits_1_lets_the_writer_run`.
- A check that times out, a missing check program, and a check killed by a signal each refuse the
  run `HandoffCheckFailed`, naming the action, with exit 1. The log holds the line with its
  `refusal`, and the writer does not run. Test: `a_failed_check_stops_the_run_never_skips`.
- `handoff_when` without `handoff`, on a person, or with an invalid argv is refused at load. Test:
  `handoff_when_is_refused_at_load_where_it_cannot_apply`.
- Under `--confinement substrate`, a check that writes into the workspace fails with a read-only
  error. Test: `the_check_cannot_write_the_workspace`.
- `handoff` alone keeps its behaviour: the test of `db7a3d8b`,
  `a_writer_with_a_handoff_waits_on_a_person_with_its_text`, passes unchanged.
- A log with `HandoffChecked` lines verifies, and an old log without them verifies. Test:
  `handoff_checked_lines_verify_in_the_chain`.

## Not covered

- What the check program may read beyond the workspace:
  `story:bound-program-declares-its-environment`.
- A person's result for the handed-off work: `story:person-action-records-a-named-result`.
- `{work}` placeholders in `handoff` text (Runbook's D14).

## Scope

- `crates/loom-bindings/src/decode.rs`: `:22-34`, `:300-310`, `:843-858`. Cited.
- `crates/loom-bindings/src/child.rs`: `bound_case_child`. Inferred.
- `crates/loom-tree/src/run.rs`: the step before `drive`, and `waits` at `:786-890`. Cited; the
  `handoffs` site is from `db7a3d8b`.
- `crates/loom-runlog/src/lib.rs` and `ess/domains/runlog.yaml`. Cited.
- `ess/domains/bindings.yaml`: `:199-223`, `:171-177`. Cited.
- `crates/loom-cli/tests/tree_run.rs`. Cited.

Edges:

- depends_on `story:tree-run-command` and `story:bound-program-contract`.
- Collides on `runlog.yaml` and `crates/loom-runlog` with `story:child-record-in-the-run-log`,
  `story:tree-status-names-what-it-read` and `story:program-reports-its-cost`, and starts after
  them.
