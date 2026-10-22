---
format: aep.planning-md/3
id: story:checked-timeout-deadlines
kind: story
status: draft
title: Every timeout becomes a deadline through checked arithmetic
revision: 1
---
## Outcome

Every timeout Loom turns into a deadline is added with checked arithmetic, and a timeout too
large for the clock is refused before anything starts, as `run_child` and `spawn_child` refuse it
since wave 2026-10-22-w1.

## Why

- The wave 2026-10-22-w1 fix pass found the same unchecked `Instant + Duration` at
  `crates/loom-intake-slice/src/bindings.rs:391`, `crates/loom-intake-slice/src/executor.rs:1277`
  and `crates/loom-connectors/src/cli.rs:535`. A large `timeout_seconds` in a run configuration
  reaches the first one.

## Acceptance

- A run configuration, a test command and a Connectors configuration each with the largest
  timeout are refused naming the timeout, with no panic and no process started.

## Scope

- `crates/loom-intake-slice/src/bindings.rs`, `crates/loom-intake-slice/src/executor.rs`,
  `crates/loom-connectors/src/cli.rs`, their tests, `CHANGELOG.md`
