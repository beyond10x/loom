---
format: aep.planning-md/3
id: story:dw-durable-run-state
kind: story
status: draft
title: A workflow step result and the usage report survive a restart of the run
relations:
- decomposes: epic:deterministic-workflows
- serves: vision:governed-autonomy
- depends_on: story:dw-typed-step-outputs
- depends_on: story:dw-run-model-meter
revision: 1
---
## Outcome

A workflow's typed step results and its run's model-usage report survive a restart of the run: they
are written beside the governor's `FileSuspensionStore` record, keyed by the Run id, and a
`resume_after_grant` in another process continues them, so a step after an approval reads a
result an earlier segment produced and the report covers the whole run.

## Why

Model usage and typed step results are counted per process segment: one `run_until_blocked` or
one `resume_after_grant` call, summed by the host. A bug-fix run usually stops at the merge
approval and resumes in another process. The first workflow whose step after an approval needs a
result from before it cannot run on per-segment state; this story adds the durable record then,
and not before.

## Acceptance

- A workflow suspended on an approval and resumed in a second process binds a step result the first
  process produced (`a_step_result_survives_a_restart_across_an_approval`, a real process restart
  as `crates/loom-commission-testkit/tests/approval_suspend_resume.rs` does).
- The usage report after the resume covers both segments
  (`the_usage_report_covers_every_segment_of_a_resumed_run`).
- A record whose Run id does not match the held Suspension is refused, never read
  (`a_step_result_of_another_run_is_refused`).

## ESS first

The durable record (Run id, step results, usage counters) is declared in `ess/domains/workflow.yaml`
and `ess/domains/metering.yaml` before code; on it `task drift` fails.

## Scope

`ess/domains/workflow.yaml`, `ess/domains/metering.yaml`, `generated/rust/loom/`,
`crates/loom-governor/src/` (beside `FileSuspensionStore`, the private `locked_dir` module),
`crates/loom-executor/src/workflow.rs`, `crates/loom-executor/src/metering.rs`, tests, docs.

## Ready

Not ready: waits on the first workflow story that needs a step result across an approval, and on
`story:dw-typed-step-outputs` and `story:dw-run-model-meter`.

## Not in scope

Changing what a segment counts; a remote store.
