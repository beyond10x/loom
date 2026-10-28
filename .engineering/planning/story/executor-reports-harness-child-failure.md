---
format: aep.planning-md/3
id: story:executor-reports-harness-child-failure
kind: story
status: draft
title: The session executor reports a failed harness child's exit status and the tail of its stderr
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
revision: 2
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

## From the takt 0.17.0 trial (F2)

Finding F2 of the takt 0.17.0 trial, relayed by session `b10x-chef-orchestrator` on 2026-10-28:
"in loom 0.46.0, at loom-session-executor/src/lib.rs:2162, the harness's stderr error reaches no
record, so takt retries a refusal that repeats every time, 3 sessions in all."

Read on `origin/main` 78a6d278a: `reply` still returns only "the session ended without the JSON
line naming its action" (`crates/loom-session-executor/src/lib.rs:2162`).

Added acceptance, beside the three above:

- `a_failed_harness_child_records_its_cause`: the exit status and the bounded stderr tail reach the
  session's record (`loom.session.AgentSession`, through the executor's error and the case it
  fails), so a caller reading the record sees the same cause on a repeat and can stop instead of
  retrying it.

Unmerged prior work: branch `impl/executor-reports-harness-child-failure` (local only, managed tree
`loomw2-u4`, wave 2026-10-26-w2, never closed) holds a red test (`52ce12b90`,
`tests/harness_child_failure.rs`) and an implementation (`ce9f322cc`) of the first three items. A
wave that takes this story starts from those commits or records why not.
