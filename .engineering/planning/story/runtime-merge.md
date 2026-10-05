---
format: aep.planning-md/3
id: story:runtime-merge
kind: story
status: active
title: 'One run loop: Commission''s runtime and intake''s slice loop are one crate'
relations:
- decomposes: epic:runtime-consolidation
- depends_on: story:import-commission
- depends_on: story:import-intake
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: crates/commission/src/ports
- confidence: inferred
  path: crates/commission/src/runtime.rs
- confidence: inferred
  path: crates/intake-cli
- confidence: inferred
  path: crates/intake-slice
- confidence: inferred
  path: ess/commission
- confidence: inferred
  path: generated/rust/commission
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-05T00:24:13Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-05T00:24:13Z", actor: "human:timo", revision: 3}
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
