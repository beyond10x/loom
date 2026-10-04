---
format: aep.planning-md/3
id: story:runtime-merge
kind: story
status: draft
title: 'One run loop: Commission''s runtime and intake''s slice loop are one crate'
relations:
- decomposes: epic:runtime-consolidation
- depends_on: story:import-commission
- depends_on: story:import-intake
revision: 1
---
## Outcome

One run loop: Commission's runtime (`runtime.rs`, `story:local-runtime-loop`) and intake's slice
loop become one runtime crate, which rechecks the frontier, case revision and authority, then
invokes the effect (Atlas ADR 0082).

## Acceptance

- One loop implementation in Loom; `intake-slice`'s loop and `LocalExecutor` are gone or are the
  runtime's.
- The slice's stop reasons (ApprovalRequired, NothingAdmissible, StepBudget, NoLocalExecutor,
  Refused) and its gate-stop rule (`7dc84ef`) hold, each by a named test.
- Commission's `story:local-runtime-loop` tests and intake's slice tests pass against the merged
  runtime.
- `task check` exits 0.

## Depends on

`story:import-commission`, `story:import-intake`.

## Scope (inferred)

loom: `crates/commission/src/runtime.rs`, `crates/intake-slice/`, new runtime crate.
