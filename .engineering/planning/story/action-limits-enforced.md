---
format: aep.planning-md/3
id: story:action-limits-enforced
kind: story
status: draft
title: Every process of a tree run runs under memory, CPU-time, wall-time and process-count limits that Substrate enforces
summary: Ceilings and defaults in the engine configuration, lower limits per action in the binding, per-run wall and CPU ceilings counted by Loom
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:tree-run-command
- depends_on: story:confinement-level-required
revision: 7
---
## Outcome

Every process a tree run starts runs under hard limits that Substrate enforces: memory, CPU time,
wall time and process count. The engine configuration's `limits` section holds, per action, a
ceiling and a default for memory bytes, CPU milliseconds, processes and `timeout_seconds`, and per
run a ceiling on total wall seconds and on total CPU milliseconds. The engine's `timeout_seconds`
ceiling and default lie in 1 to 3600, the range `loom.bindings/1` admits, and the loader refuses a
value outside it `ValueOutOfRange`, naming the member's path, the value and the range. A binding's
`limits` may state memory, CPU and processes for its action at or below the ceiling. An action's
wall limit is its `timeout_seconds` from `loom.bindings/1`, and the engine's ceiling caps it;
`limits` has no wall member. An action that states no value runs under the engine's default. A
bindings file above a ceiling is refused at `tree open` and `tree run`, naming the action, the limit
and both values. The per-action values reach the runner on the child:
`ChildCommand::with_limits(ChildLimits)` (memory bytes, CPU milliseconds, processes) beside the
existing `with_timeout`, carried in `ConfinementSpec`, and `limits()`
(`crates/loom-intake-slice/src/confinement.rs:2969-2977`) takes them in place of its fixed values.

Substrate bounds memory and process count per process start, so neither has a per-run sum; a limit
summed over a protocol run is Loom's (agreed with sessions `sdc` and `substrate`, 2026-11-21). Loom
counts the two per-run totals over one `tree run` invocation: wall time from the host clock around
each step, and CPU time from the usage the runner returns for each process
(`story:substrate-daemon-runner`). Before each step, a run whose total has reached a ceiling stops
with `TreeStop::BudgetSpent` naming the limit, exit 5. `tree run --max-wall-seconds` above the
engine's per-run wall ceiling is refused `RunBudgetAboveCeiling`, naming the limit, both values,
exit 2, and without the flag the ceiling is the run's wall budget.

Whether the confinement can enforce limits is read through the mapping of
`story:confinement-level-required`. A confinement that does not serve the family `limits` (`local`)
refuses to start a run whose engine configuration has a `limits` section or whose bindings state
`limits`, unless the section declares limits advisory, and then each step's confinement record says
`enforced: false`. Under `local` the runner sends no bounds, which `local` would refuse, and ends a
child at its `timeout_seconds` by killing its process group, which
`story:confinement-opens-from-engine-config` delivers and this story relies on; memory, CPU and
processes are unbounded there, and the per-run wall budget is counted as everywhere. With no
`limits` section and no `limits` in the bindings a run behaves as today, under the fixed bounds
where the confinement serves `limits` and under that deadline on `local`.

A per-run CPU ceiling needs reported CPU time, which the mapping answers with the fact
`cpu_time_usage` (under `Daemon` only); a run with that ceiling on a confinement that does not prove
it is refused before its first step `CpuTimeUnreported`, naming the ceiling, and a step that comes
back with no usage under that ceiling stops the run before its next step with the same refusal,
naming the action. Each step's confinement record holds the limits applied and the wall time and
CPU time used.

## Why

`loom.bindings/1` has `timeout_seconds` (1 to 3600) and `budget` (`story:bindings-document`), and
every child gets the same fixed bounds from `limits()`: output 1 MiB, 2048 processes, 8 GiB, 3600 s
CPU (`confinement.rs:2969-2977`). Substrate takes the bounds per run as `process_bounds`, which
needs the family `limits` (`crates/substrate-host/src/confinement/profile.rs:160-172`, `:202`), and
refuses, never degrades, an adapter that does not serve it (`FamilyUnserved`, `mod.rs:622-630`, read
at 0.21.0); `local` does not (`local.rs:107-112`). The daemon takes them as `ExecutionPolicy`, which
requires all five (`crates/b10x-substrate-sdk/src/model.rs:675-681`), so output bytes is passed as
the fixed cap. Substrate's own rule is that "a request selects among what configuration already
permitted" (Substrate ADR 0013), which is the split used here: ceilings are engine configuration, a
binding only narrows. `ChildCommand` carries only a timeout per child today (`confinement.rs:1304`)
and `ConfinementSpec` only `timeout_ms` (`ess/intake/domains/confinement.yaml:146`).

CPU time used is reported by the daemon (`ResourceUsage.cpu_time_us`,
`crates/substrate-wire/src/lib.rs:2163-2184`, published as `exec_resource_usage.cpu_time`,
`:4348-4359`); the in-process adapter route's `RunObservation` has none
(`crates/substrate-host/src/confinement/mod.rs:694-706`). `story:tree-run-command` maps
`--max-wall-seconds` to `HostControl::host_stop` per invocation and stops with `BudgetSpent`, exit
5; this story reuses that stop for both per-run ceilings. That story declares `TreeBudget` without
naming members.

## Waits on

- `story:confinement-level-required`: the mapping that says whether the opened confinement serves
  `limits` and proves `cpu_time_usage`, and the files it edits before this story:
  `ess/domains/engine.yaml` (`StepConfinement`), `ess/domains/bindings.yaml` (`BoundAction`) and
  `ess/intake/domains/confinement.yaml`. Through it, `story:substrate-daemon-runner` (`usage`).
- `story:tree-run-command`: `TreeStop::BudgetSpent`, `TreeBudget` and `--max-wall-seconds`.

## ESS first

`ess/domains/engine.yaml`: `LimitsSection` (per action `ActionCeilings` and `ActionDefaults`, each
with memory bytes, CPU milliseconds, processes and `timeout_seconds`; per run `wall_seconds` and
`cpu_millis`; `advisory`), `EngineConfiguration` gains optional `limits`, `StepConfinement` gains
`limits` (the applied values), `enforced`, `wall_millis_used` and optional `cpu_millis_used`, and
`EngineRefusal` gains `LimitAboveCeiling` (action, limit, asked, ceiling), `RunBudgetAboveCeiling`
(limit, asked, ceiling), `ValueOutOfRange` (path, value, range), `LimitsUnenforced` and
`CpuTimeUnreported` (the ceiling, optional action). `ess/domains/bindings.yaml`: `BoundAction`
gains optional `limits` (`ActionLimits`: memory bytes, CPU milliseconds, processes).
`ess/intake/domains/confinement.yaml`: `ConfinementSpec` gains optional `memory_bytes`,
`cpu_millis` and `processes`. `ess/domains/tree.yaml`: `TreeBudget` gains two new members,
`wall_seconds` (it holds the budget `--max-wall-seconds` sets, which `story:tree-run-command` keeps
unnamed) and `cpu_millis`, and `TreeStop::BudgetSpent` gains the member `limit`, which names the
spent `TreeBudget` member. On that commit `task drift` and `task intake-drift` fail and
`crates/loom-tree/tests/action_limits.rs` does not compile.

## Acceptance

Each bullet runs with stub programs under the route it names; "both" means under the in-process
`linux` adapter (on a host whose probe proves `limits`, as the delegated tests are gated) and under
`Daemon` with a managed daemon.

- Both: a program that allocates past its action's memory limit is stopped before it writes its
  completion marker; one that stays under the limit writes it.
- Both: a busy loop past the action's CPU limit is stopped before it writes its marker.
- Both: a sleep past `timeout_seconds` is stopped before it writes its marker, and its `ProgramRun`
  says it timed out.
- Both: in a fork loop past the process limit, the forks beyond the limit fail: the program writes
  a count of failed forks above 0 and exits 0, and the step takes its result from that exit.
- Both: a binding with a memory limit above the engine's ceiling is refused at `tree open`
  `LimitAboveCeiling`, naming the action, `memory_bytes` and both values; the same for a
  `timeout_seconds` above the engine's ceiling. A binding below the ceiling runs under its own value
  and an action with none under the engine's default, as the step's confinement record shows.
- An engine configuration whose `timeout_seconds` ceiling is 3601, and one whose default is 0, are
  refused by the loader `ValueOutOfRange` naming the member's path.
- Both: a tree whose steps have used the per-run wall ceiling stops with `BudgetSpent` naming
  `wall_seconds` before its next step, exit 5.
- Under `Daemon`: a tree whose steps have used the per-run CPU ceiling stops with `BudgetSpent`
  naming `cpu_millis` before its next step, exit 5. Under in-process `linux`: a run whose engine
  configuration sets a per-run CPU ceiling is refused before its first step `CpuTimeUnreported`.
- `tree run --max-wall-seconds` above the engine's per-run wall ceiling exits 2 naming
  `RunBudgetAboveCeiling`, `wall_seconds` and both values.
- Under `local`: a run whose engine configuration has a `limits` section is refused before its
  first step `LimitsUnenforced` naming `limits`; with `advisory: true` it runs, each step's
  confinement record says `enforced: false`, and a sleep past `timeout_seconds` is killed at that
  deadline with no process of it left and its `ProgramRun` says it timed out; with no `limits`
  section and no `limits` in the bindings the same sleep is killed the same way.
- Both: each step's confinement record names the applied memory, CPU, process and
  `timeout_seconds` values and the wall time used; under `Daemon` it also names the CPU time used.
- `task check` exits 0.

## Out of scope

Recording a hit in the run log (`story:limit-hits-in-run-log`). Output bytes: the fixed 1 MiB cap
stays. A per-run sum of memory or of process count. Counting the per-run totals of a served tree
across invocations and daemon restarts (`story:tree-daemon`). CPU shares, block-IO and bandwidth.
Limits on an agent session (`story:agent-session-through-substrate`).
