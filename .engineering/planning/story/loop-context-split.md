---
format: aep.planning-md/3
id: story:loop-context-split
kind: story
status: draft
title: LoopContext splits into Clock, Ids, Limits and HostControl with a blanket impl
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

`LoopContext` (`crates/loom-commission/src/runtime.rs:510`) splits into `Clock`, `Ids`, `Limits`
and `HostControl`, with a blanket `impl<T: Clock + Ids + Limits + HostControl> LoopContext for T`,
so a host supplies limits per agent and a stop per process.

## Why

Agency needs per-agent limits and a per-process stop separately (design § 3.2). Decided by the
orchestrator, Q3 A.

## Acceptance

- Every `LoopContext` implementor in the workspace and in `consumers/pinned` compiles unchanged, or
  each one that cannot is a `**Breaking:**` CHANGELOG bullet naming it.
- `crates/loom-commission-testkit/tests/loop_context_split.rs`
  (`a_context_from_four_parts_runs_as_one`): a context assembled from four parts gives the same
  `LoopEnd` as the single-trait context on a recorded run.
- `sdk-stability.md` lists the four traits.

## ESS first

No behaviour change and no noun: a trait split, so `ess/` is untouched. The red step is
`a_context_from_four_parts_runs_as_one`.

## Not in scope

New context methods.

## Scope

Crates: `loom-commission`, `loom-commission-testkit`, `loom-sdk`. `crates/loom-commission/src/runtime.rs`
(shared with `story:runtime-events`: not in one wave), `host_ports.rs`, `sdk-stability.md`,
CHANGELOG.md.

## Ready

Ready now.
