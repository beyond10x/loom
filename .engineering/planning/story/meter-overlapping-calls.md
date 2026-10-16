---
format: aep.planning-md/3
id: story:meter-overlapping-calls
kind: story
status: implemented
title: A run meter counts overlapping and nested executor calls in their own steps
relations:
- decomposes: epic:deterministic-workflows
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T23:50:23Z", actor: "human:timo", revision: 2, correlation: "wave-2026-10-15-w1"}
- {from: "proposed", to: "active", at: "2026-10-14T23:50:23Z", actor: "human:timo", revision: 3, correlation: "wave-2026-10-15-w1"}
- {from: "active", to: "implemented", at: "2026-10-16T01:33:09Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

One `RunMeter` counts each executor call's model calls in that call's own step when executor
calls on the meter overlap in time (two executors on two threads) or nest (a `MeteredExecutor`
over another `MeteredExecutor` of the same meter), and calls made outside any executor call stay
in step 0. One executor call of the run is one step, however many metered wrappers it passes
through.

## Why

`RunMeter` keeps one open step per meter (`crates/loom-executor/src/metering.rs`, `Ledger::open`),
set when a `MeteredExecutor::run` begins and restored when it returns. Two overlapping executor
calls therefore count each other's calls in whichever step opened last, and a nested
`MeteredExecutor` opens a second step for the same executor call and leaves the first one empty. Nothing in the
repository reaches either today: Commission's runtime makes one executor call at a time and a host
wraps its executor once. Until this story lands, both are a documented contract of `RunMeter` and
`MeteredExecutor` (executor calls on one meter do not overlap; an executor is wrapped by one
`MeteredExecutor`), and the two adversary cases that exercise them are ignored with that reason.

## Acceptance

Overlapping and nested executor calls on one meter each count their own calls in their own step.

- `crates/loom-executor/tests/adversary_w11_u1_metering.rs`
  `two_executors_sharing_a_meter_each_count_their_own_calls`: A opens step 1, B opens step 2, A
  turns twice, B once, the host three times outside any call; the steps are
  `[(0, 3), (1, 2), (2, 1)]`. Its `#[ignore]` is removed and it passes.
- `nested_metered_executors_on_one_meter_open_one_step_per_executor_call`: a `MeteredExecutor`
  over a `MeteredExecutor` on one meter, one `run` call with two turns, reports the steps
  `[(1, 2)]`. Its `#[ignore]` is removed and it passes.
- The rest of `adversary_w11_u1_metering.rs` and `crates/loom-executor/tests/run_metering.rs`
  stay green, and the contract sentences in `RunMeter`'s and `MeteredExecutor`'s docs are removed.

## ESS first

No declaration changes: the report's shape (`ess/domains/metering.yaml`) is unchanged; only which
step a call is counted in. The story's first red is the two cases above with their `#[ignore]`
removed.

## Scope

- `crates/loom-executor/src/metering.rs` (cited): how a metered port finds the step it counts in
  (for example a step carried per thread or per executor call rather than one open step per meter),
  and how a nested `MeteredExecutor` of the same meter recognises that a step is already open.
- `crates/loom-executor/tests/adversary_w11_u1_metering.rs` (cited): the two `#[ignore]`
  attributes removed, nothing else.
- `CHANGELOG.md` (Unreleased).

## Not in scope

- The report's shape and the `loom.metering` domain.
- Concurrent model calls inside one executor call (delegates side by side): they count in that
  call's step already, and stay so.
- Wiring the report into `b10x-loom run` or the intake slice.
