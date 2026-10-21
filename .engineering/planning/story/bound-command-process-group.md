---
format: aep.planning-md/3
id: story:bound-command-process-group
kind: story
status: active
title: A timed-out bound command is killed with every process it started
relations:
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-intake-slice/src/bindings.rs
- confidence: cited
  path: crates/loom-intake-slice/src/executor.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T13:40:12Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-21T13:40:12Z", actor: "human:timo", revision: 4}
---
## Outcome

A bound command (`run --action-bindings`) that times out is killed together with every process it
started. A failure to poll its status is reported as a failure with the error, not as a timeout.

## Why

On `05699aae`, `crates/loom-intake-slice/src/bindings.rs`:

- `stop` (line 546) calls `child.kill()` on the direct child only. A program that forked keeps
  its children, and they keep running after the run reports `TimedOut`. The test runner already
  starts its child as a process-group leader (`executor.rs:1263`, `command.process_group(0)`)
  and kills the group (`executor.rs:1308-1318`).
- `run` (line 407) matches `Ok(None) | Err(_)` on `child.try_wait()` together. A wait error is
  reported as `Ended::TimedOut` and the error is dropped.

## Design

- Start the bound program with `process_group(0)` and kill the group on timeout, sharing the
  helper `executor.rs` uses instead of a second copy.
- `try_wait` failing: kill and reap the group, then return an `Ended` variant that carries the
  `io::Error`, reported as `Refused` with the error text.
- AGENTS.md § Intake, the `--action-bindings` bullet: "a timeout past which it is killed and
  reaped" becomes "past which it and the processes it started are killed and reaped".

## ESS first

`ess/intake/domains/bindings.yaml` declares no kill or wait semantics, so no declaration changes.
The story's new test is red first: `crates/loom-intake-slice/tests/` (bindings)
`a_timed_out_command_leaves_no_process_it_started`, a script that forks a sleeping child, writes
the child's pid to a file and outlives the timeout; the test then checks that the pid is gone.

## Acceptance

- `a_timed_out_command_leaves_no_process_it_started` passes.
- A unit test in `bindings.rs` shows a `try_wait` error reaching the report with its text, not as
  `TimedOut`.

## Scope

- `crates/loom-intake-slice/src/bindings.rs`
- `crates/loom-intake-slice/src/executor.rs` (the shared kill helper)
- `AGENTS.md`
