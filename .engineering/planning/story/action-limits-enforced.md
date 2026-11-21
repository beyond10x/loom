---
format: aep.planning-md/3
id: story:action-limits-enforced
kind: story
status: draft
title: Every process of a tree run runs under memory, CPU, wall-time, process and output limits that Substrate enforces
summary: Ceilings and defaults in engine configuration, lower limits per action in the binding, per-run ceilings counted by Loom
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:tree-run-command
revision: 2
---
## Outcome

Every process a tree run starts runs under hard limits that Substrate enforces: memory, CPU
time, wall time, process count and output bytes. The engine configuration holds a ceiling for
each, per action and per run; a binding may state lower limits for its action and never higher
ones; an action with none takes the engine's defaults. A bindings file that asks for more than
the ceiling is refused at load, naming the action and the limit. Substrate bounds one process start; a limit summed over a
protocol run is Loom's (agreed with sessions `sdc` and `substrate`, 2026-11-21). So the per-run
ceilings (total wall time, total CPU time) are counted by Loom over the tree's steps and stop
the run before the step that would pass them. An engine whose confinement cannot enforce a limit (the `local` adapter)
refuses to start a run that declares one, unless the engine configuration says limits are
advisory there, and then each step's record says the limit was not enforced.

## Why

`loom.bindings/1` has `timeout_seconds` and `budget` only (`story:bindings-document`), and
`ChildCommand` applies fixed limits to every child: output 1 MiB, 2048 processes, 8 GiB, 3600 s
CPU (`crates/loom-intake-slice/src/confinement.rs:2954-2962`). Substrate takes them per run as
`ProcessBounds` and refuses, never degrades, on an adapter that cannot serve them
(`crates/substrate-host/src/confinement/profile.rs:160-172`, `mod.rs:622-630`, read at 0.21.0).
Substrate's own rule is that "a request selects among what configuration already permitted"
(Substrate ADR 0013), which is the split used here: ceilings are engine configuration, a binding
only narrows.

## ESS first

`ess/domains/bindings.yaml`: `BoundAction` gains optional `limits` (`ActionLimits`: memory bytes,
CPU milliseconds, wall seconds, processes, output bytes), and `BindingsRefusal` gains
`LimitAboveCeiling`. `ess/domains/tree.yaml`: `EngineLimits` (per-action ceilings and defaults,
per-run ceilings) and `TreeStop` gains `RunLimitSpent`. On that commit `task drift` fails and
`crates/loom-bindings/tests/action_limits.rs` does not compile.

## Acceptance

Under the `linux` adapter or the Substrate daemon, with stub programs:

- A program that allocates past its action's memory limit is stopped and the step is refused; one
  that stays under it completes.
- The same for a busy loop past the CPU limit, a sleep past the wall limit, and a fork loop past
  the process limit (the forks fail and the step records the limit as reached).
- A binding with a memory limit above the engine's ceiling is refused at load `LimitAboveCeiling`
  naming the action, the limit and both values; one below it runs under the binding's value, and
  an action with none runs under the engine's default.
- A tree whose steps have used the run's total wall-time ceiling stops with `RunLimitSpent`
  before its next step, exit 5, naming the limit.
- Under `local`, a run whose engine configuration or bindings declare a limit is refused before
  its first step; with limits declared advisory it runs and each step's record says
  `enforced: false`.
- `task check` exits 0.

## Out of scope

Recording a hit in the run log (`story:limit-hits-in-run-log`). CPU shares, block-IO and
bandwidth. Limits on an agent session (`story:agent-session-through-substrate`).
