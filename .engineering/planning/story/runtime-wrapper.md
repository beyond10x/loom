---
format: aep.planning-md/3
id: story:runtime-wrapper
kind: story
status: draft
title: loom_sdk::Runtime runs, suspends and resumes a commission with the parts it owns
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

`loom_sdk::Runtime` owns a run's governor, executor, authority, effects, run store and context, and
`run`, `run_suspending` and `resume` call `run_until_blocked`, `run_with_suspension` and
`resume_after_grant` (`crates/loom-commission/src/runtime.rs:867`, `:898`, `:2049`) with them.

## Why

Every host wires the same arguments by hand (control-plane, `crates/loom-sdk/examples/software_change.rs`).

## Acceptance

- `crates/loom-sdk/tests/runtime_wrapper.rs`
  (`runtime_equals_the_free_functions`): on the example's recorded run, `Runtime::run` and
  `run_until_blocked` give equal `LoopEnd`s; the same for a suspend and a resume across a
  `FileSuspensionStore`.
- The free functions stay, unchanged.
- `sdk-stability.md` lists `Runtime`; `host_ports.rs` resolves it.

## ESS first

No behaviour change: a wrapper over existing functions with no new noun, so `ess/` is untouched.
The red step is `runtime_equals_the_free_functions`, failing before `Runtime` exists.

## Not in scope

Events (`story:runtime-events` adds a sink to `Runtime` later); a change to the loop.

## Scope

Crates: `loom-sdk`. `crates/loom-sdk/src/runtime.rs` (new), `crates/loom-sdk/src/lib.rs`, the new
test, `website/docs/reference/sdk-stability.md`, `crates/loom-sdk/tests/host_ports.rs`, CHANGELOG.md.

## Ready

Ready now.
