---
format: aep.planning-md/3
id: story:handoff-when-a-program-decides
kind: story
status: active
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
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: cited
  path: generated/ess/loom/
- confidence: cited
  path: generated/rust/loom/
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T06:37:59Z", actor: "agent:loom", revision: 5}
- {from: "proposed", to: "active", at: "2026-11-22T07:13:50Z", actor: "agent:sdc", revision: 8, executor: "agent:loom", correlation: "runbook-model-run-order-2026-11-22"}
---
## Outcome

A writer's handoff can depend on a fact only a program can read. It is offered while that program
says so, and the writer runs otherwise.

- A program binding may carry `handoff_when`, an argv held to the program contract of any bound
  program (`program_root`, the argv rules, `InvalidArgv`). It requires `handoff`, else
  `MissingMember`; on a `person` executor it is `MemberNotForExecutor`.
- **When it runs.** When the deterministic selector picks the action as the next step, the
  `handoff_when` program runs first, as a child built by `bound_case_child`
  (`crates/loom-bindings/src/child.rs:60`). It runs under the same runner as the binding's own
  program, with the binding's `timeout_seconds`.
- **Its reach is never more than the writer's:**
  - read-only on the whole workspace, `target/` included;
  - `PATH` and the `LOOM_*` variables only, with no retry feedback. `PATH` is the run's program
    path once `story:per-run-program-path` exists, otherwise the one every bound program of the run
    gets;
  - no aperture, no secret slot, no declared read-only root and no writable directory.
- **Exit 0.** No step is taken. The run stops `AwaitingPerson` with a `Handoff` decision for the
  action: `decides` is the `handoff` text and `command` is `tree run --store <store>`. This is the
  decision of `impl/tree-run-command` `db7a3d8b` (`crates/loom-tree/src/run.rs:795-830` there).
- **Any other exit.** The handoff is skipped, and the action's own program runs in the same step.
  The case is not stuck.
- **A launch error, a timeout or a signal** is never a skip. The run stops, refused
  `HandoffCheckFailed`, naming the action and the cause, with exit 1.
- **Run log.** Each evaluation writes one line, `HandoffChecked {action, exit?, refusal?,
  blob_digest?}`, before the stop or the step. It holds exactly one of `exit` and `refusal`;
  `blob_digest` names the check's kept output. The log replays the check without running it.
- **`handoff` without `handoff_when`** keeps the meaning it has in `db7a3d8b`: it is offered when
  the case is otherwise stuck (`run.rs:934-941` there).
- **A binding with `handoff_when` at a stuck stop.** Its handoff is given only through the check,
  which runs the same way: exit 0 gives the `Handoff`, and any other exit leaves the stop `Stuck`.
- **Format.** `loom.bindings/1` stays; `ACTION` (`decode.rs:22-34`) gains `handoff_when`. An older
  Loom refuses it `UndeclaredMember` (`decode.rs:843-858`); other documents read as before.

Alternative (b), cleaner in launches: `exits.handoff: [codes]` on the writer's own program,
beside `error` and `not_called` (`ess/domains/bindings.yaml:171-177`). It needs no second launch
and no new log kind, but the writer's run is recorded and counts against `max_runs`. Runbook asked
for the separate argv, so (a) is the default.

Security-relevant: yes. This adds a second confined launch per step, whose exit decides whether a
person is asked. The unit gets a security review.

## Why

Five of Runbook's bindings hand `amend.upstream` off only while a finding waits for a person:
intake, direction, spec, plan and story (Runbook `bindings/PENDING.md:125-133`). Each uses the
argv `[tools/sdc-tool.sh, findings-wait, --action, amend.upstream]`, where exit 0 means a finding
waits. Today `handoff_when` is refused at load, so every such binding fails `tree open`.

## ESS first

`ess/` changes first, in its own commit, then `task drift` and `task docs-generate`:
`bindings.yaml` (`BoundAction.handoff_when: List<String>`, empty when absent), `runlog.yaml`
(`RunLogEvent.HandoffChecked`), `tree.yaml` (`TreeRefusalReason.HandoffCheckFailed`).

An older run-log reader refuses the new line, as in `story:child-record-in-the-run-log`.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no network, no model
call.

The check programs are fixture scripts in POSIX sh, not Python (§ No Python for permanent tools),
and every brief of the unit names that rule. The tests share one tree fixture and copy its store,
rather than run a whole tree per test.

The tests that need a delegated cgroup skip without one only when `LOOM_REQUIRE_DELEGATION` is
unset. The unit's gate sets it and runs them under `systemd-run --user -p Delegate=yes --scope`.
The unit's report lists the tests run and the tests skipped, and a skip counts as red there.

- **Exit 0 hands off.** The run stops `AwaitingPerson` with the action's `Handoff` decision, the
  writer does not run, and the log holds one `HandoffChecked` line with `exit: 0`. Test:
  `a_check_that_exits_0_hands_the_action_off`.
- **Exit 1 lets the writer run.** The writer runs in that step, and the case is not stuck. The log
  holds `HandoffChecked` with `exit: 1` before the writer's `ActionStarted`. Test:
  `a_check_that_exits_1_lets_the_writer_run`.
- **At a stuck stop.** A case is otherwise stuck, and the admissible writer carries `handoff` and
  `handoff_when`. A check exiting 0 gives the `Handoff` (exit 3), and a check exiting 1 leaves the
  stop `Stuck` (exit 4). Test: `at_a_stuck_stop_the_check_decides_the_handoff`.
- **A failed check stops the run.** A check that times out, a missing check program, and a check
  killed by a signal each refuse the run `HandoffCheckFailed`, naming the action, with exit 1. The
  log holds the line with its `refusal`, and the writer does not run. Test:
  `a_failed_check_stops_the_run_never_skips`.
- **Load refusals, one test each, naming the action:** without `handoff`, `MissingMember`
  (`handoff_when_without_handoff_is_refused`); on a person, `MemberNotForExecutor`
  (`handoff_when_on_a_person_is_refused`); an empty argv, `InvalidArgv`
  (`an_invalid_handoff_when_argv_is_refused`).
- **The check's environment.** A check that prints its environment and exits 1 leaves a kept blob
  that holds `PATH` and `LOOM_*` variables only. It holds no `LOOM_RETRY_FEEDBACK`, also on a step
  the writer takes as a retry. Test: `the_check_gets_path_and_loom_variables_only`.
- **The check cannot write the workspace.** Under `--confinement substrate`, a check tries to
  create a file in the workspace and then exits 3. The log line holds `exit: 3`, not the 1 of "no
  handoff". The kept blob named by `blob_digest` holds the write's read-only error, and the file
  does not exist. Test: `the_check_cannot_write_the_workspace`.
- **`handoff` alone** keeps its behaviour: `a_writer_with_a_handoff_waits_on_a_person_with_its_text`
  (`db7a3d8b`) passes unchanged.
- **The hash chain.** A log with `HandoffChecked` lines verifies, and an old log without them
  verifies. Test: `handoff_checked_lines_verify_in_the_chain`.

## Not covered

- More reach for the check than above, or anything a program may read beyond the workspace:
  `story:bound-program-declares-its-environment`.
- A person's result for the handed-off work (`story:person-action-records-a-named-result`), and
  `{work}` placeholders in `handoff` text (Runbook's D14).

## Scope

- Cited: `crates/loom-bindings/src/decode.rs` (`:22-34`, `:300-310`, `:843-858`),
  `crates/loom-bindings/src/child.rs:60`, `crates/loom-tree/src/run.rs` (the step before `drive`,
  `waits` at `:786-890`; the `handoffs` site is from `db7a3d8b`), `crates/loom-runlog/src/lib.rs`,
  `ess/domains/runlog.yaml`, `ess/domains/tree.yaml`, `ess/domains/bindings.yaml` (`:171-177`,
  `:199-223`), `crates/loom-cli/tests/tree_run.rs`.
- Inferred: `generated/rust/loom/` and `generated/ess/loom/`, regenerated.

Edges:

- depends_on `story:tree-run-command` and `story:bound-program-contract`. There is no depends_on
  `story:person-action-records-a-named-result`.
- **Order on `runlog.yaml`, `crates/loom-runlog`, `tree.yaml` and the generated model files:**
  `story:child-record-in-the-run-log`, then `story:tree-status-names-what-it-read`, then this
  story, then `story:program-reports-its-cost`, which depends_on this one.
- Shares `waits` in `run.rs` and `tests/tree_run.rs` with
  `story:every-printed-decision-command-runs`. It shares `tests/tree_run.rs` with
  `story:person-action-records-a-named-result` and `story:tree-run-takes-a-delegated-cgroup`. The
  later unit merges the earlier head.
- Shares the `ACTION` member list (`decode.rs:22-34`) and `BoundAction` with
  `story:program-reports-its-cost` and `story:bound-program-model-credential-through-a-secret-slot`.
  Both come after this story.
