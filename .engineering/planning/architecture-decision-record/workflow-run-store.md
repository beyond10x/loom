---
format: aep.planning-md/3
id: architecture-decision-record:workflow-run-store
kind: architecture-decision-record
status: proposed
title: A workflow run's progress is written after every performed step and resumed after a restart
relations:
- decides: story:dw-durable-run-state
revision: 1
---
## Decision (proposed)

A workflow run's progress (the step reached, typed step outputs, the effect log of performed
requests and the run's usage) is written after every performed step to a run store, keyed by the
Run id, and `WorkflowExecutor` resumes a run from it after a process restart, not only after an
approval. Loom provides a file store beside `FileSuspensionStore` (same directory discipline);
a host may supply its own through a `RunStore` port. A record whose Run id or workflow definition
does not match is refused, never read. Resuming re-reads the governor's frontier and revalidates
before any proposal; an effect whose `Performed` was not recorded is proposed again, so effects
must tolerate one repeat or report idempotency.

## Context

`decision-blocker:dw-run-state-across-restart` (cleared) chose option C: per process segment now,
a durable record beside `FileSuspensionStore` once a workflow needs a result across an approval.
Workflow state is kept in process (`crates/loom-executor/src/workflow.rs:35`, `:41`). A host
running a loop for hours (model sessions per candidate) loses its place on any restart, at any
step, not only at an approval: that is the need option C waited for, and it widens option B from
"at suspension" to "after every step".

## Consequences

- Loom takes on a durable record it did not hold: format, locking and refusal of a foreign record
  are Loom's, declared in ESS first.
- `WorkflowExecutor` gains `with_store` and `resume(run_id)`; the SDK names them.
- The at-least-once rule for an unrecorded `Performed` is stated to hosts.

## Alternatives

- Only at suspension (option B as cleared). Rejected for this need: a crash between approvals
  loses every step since the last one.
- Host-only records (status quo). Rejected: each host re-implements resume.

## Source

story:dw-durable-run-state; decision-blocker:dw-run-state-across-restart.
