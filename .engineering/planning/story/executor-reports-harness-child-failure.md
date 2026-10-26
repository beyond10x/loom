---
format: aep.planning-md/3
id: story:executor-reports-harness-child-failure
kind: story
status: draft
title: The session executor reports a failed harness child's exit status and the tail of its stderr
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

When the harness child of a session fails, the session executor's error carries the child's exit
status and the tail of its standard error (bounded, walked for credentials like every other text
the executor reports), so the case records the cause instead of only "the session ended without
the JSON line naming its action".

## Why

The orchestrator, 2026-10-26, from the takt 0.13.1 trial (agent report, 19:28Z): "when the harness
child fails, the loom 0.43.0 session executor reports only \"ended without the JSON line naming
its action\" (crates/loom-session-executor/src/lib.rs:2163). The real cause was on the harness's
stderr (here: llm 0.15.0 refusing message_delta.usage.cost), so the case records no cause.
Expected: the executor's error carries the child's exit status and the tail of its stderr."

The trial's probe captured that standard error (206 bytes): "error: model wire refused:
Unsupported: Messages field is outside the declared subset: message_delta.usage.cost".

## Acceptance

- `a_failed_harness_child_reports_its_exit_status_and_stderr_tail`: a child that writes an error
  to standard error and exits non-zero without the JSON line yields an executor error naming the
  exit status and holding the last lines of that standard error.
- The tail is bounded (a named constant) and a credential in it is redacted or refused by the
  executor's existing walk, never repeated.
- A child that exits 0 without the JSON line keeps today's message, with its exit status added.

## Scope

Derived 2026-10-26 by the coordinator, read in the tree at `7f3d7357`.

- `crates/loom-session-executor/src/lib.rs` (`reply`, :2148-2163, and the code that runs the
  child and collects its output) — cited
- `crates/loom-session-executor/tests/` — inferred
- AGENTS.md, CHANGELOG.md — inferred
