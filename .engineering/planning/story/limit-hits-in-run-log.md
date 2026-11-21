---
format: aep.planning-md/3
id: story:limit-hits-in-run-log
kind: story
status: draft
title: Every limit a process hits is one event in its case's run log, and a result table can key on it
summary: 'From the refusal code and usage Substrate reports, in process on the host-driver route and from the daemon: memory, CPU time, wall time, process count; under local the wall time only'
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:action-limits-enforced
- depends_on: story:substrate-daemon-runner
- depends_on: story:agent-node-run-records
revision: 8
---
## Outcome

Every limit a process hits is one `LimitHit` event in its case's run log: the action, which limit
(`LimitKind`: `memory`, `cpu-time`, `wall-time` or `processes`), the configured value, whether it
was enforced, and the measured value where Substrate reports one. Both values are integers in the
kind's unit, named by the kind: bytes for `memory` (measured: the memory peak), milliseconds for
`cpu-time` (measured: the CPU time used), seconds for `wall-time` (no measured value) and a count for
`processes` (measured: the process peak). The event comes from what Substrate reported, the exec's
refusal code and usage, or for the wall limit from the step's `ProgramRun.timed_out`, and never from
the program's output. A step ended by its memory or CPU limit looks up `limit:memory` or
`limit:cpu-time` in its result table before `*`, so a protocol can turn "ran out of memory" into an
evidence result. A step ended by its wall limit has `ProgramRun.limit` = `wall-time` beside
`timed_out`, and keeps the key `timeout` of `story:bound-program-result-table`; no
`limit:wall-time` key exists. A process-count hit ends nothing, so that step's result comes from its
exit as before. `enforced` is the step's `StepConfinement.enforced` (`story:action-limits-enforced`).
`tree status` gives each case's number of `LimitHit` lines.

What is recorded depends on the route:

- `Daemon`: all four kinds. The memory and CPU ends come from the refusal code on the applied
  record's `ended_by` (`exec.memory-limit`, `exec.cpu-limit`), the wall end from `timed_out`, and the
  process-count hit from the usage counter `process_limit_hits` above 0.
- In-process `linux`, on the host-driver route: all four kinds, from the same sources. This story
  makes `settle` (`crates/loom-intake-slice/src/confinement.rs:822-839`) set `ended_by` from the
  host driver's refusal code, which the driver sets for a memory, CPU or wall end whether or not
  usage was asked for (`crates/substrate-host/src/process.rs:3182`, `:3280-3322`). In-process
  starts ask for `ExecMeasurement::ResourceUsage` when the driver publishes `exec_resource_usage`,
  and `usage` comes from the driver's `ExecUsage::Observed` (`process.rs:1363-1367`,
  `:4172-4201`). On a driver that does not publish it a start asks for nothing, since one asking
  would be refused (`process.rs:2475-2479`): the memory and CPU lines then carry no `measured`,
  and no `processes` line is written, because that driver reports no count.
- `local`: the wall-time hit only, from `timed_out` at Loom's own deadline
  (`story:confinement-opens-from-engine-config`). `local` serves no limits and reports no usage.

A memory or CPU end reaches the result table: this story changes `child_result` and `live_result`
in `crates/loom-intake-slice/src/confinement.rs` (today `:939-960`, where any end with a refusal
other than `exec.timeout` or the byte ceiling is refused `CapabilityUnserved`) so that an end whose
`ended_by` is `exec.memory-limit` or `exec.cpu-limit` is a result with no exit status, and the
program port sets `ProgramRun.limit` from it.

## Why

The live view must show every limit hit (session `sdc`, 2026-11-21). Substrate names the limit
that ended a run in the exec's refusal code, `exec.timeout`, `exec.cpu-limit` or
`exec.memory-limit` (`crates/substrate-host/src/process.rs:3280-3322`, read at 0.21.0). A memory end
is not an `exec.cancelled`: `forced_cancellation` covers timeout, CPU, aperture, output and a
requested cancel, not memory (`:3151-3155`), so the run ends `Exited` (`:3221`) with that refusal
code. The reason is read from the refusal, never from the event's name. Today's in-process
`run_child` reads that code from the host driver (`settle`,
`crates/loom-intake-slice/src/confinement.rs:836-839`), and in-process `linux` launches stay on
that route after `story:confinement-opens-from-engine-config`, so this story reads memory, CPU and
process ends there. Loom asks the host driver for no usage today (`measurements` is empty,
`confinement.rs:1546`, `:3274`). `story:child-launches-through-confinement-adapters` moves those
launches onto the adapter route, whose `RunObservation` carries an exit and a signal, and OOM kills
only on `pod` (`crates/substrate-host/src/confinement/mod.rs:694-706`); when it lands it must keep
these reasons, which is what its `upstream-blocker:substrate-run-observation-names-the-limit`
waits for. The daemon reports each exec's usage
as `ExecUsage::Observed(ResourceUsage)` (`cpu_time_us`, `memory_peak_bytes`, `processes_peak`,
`process_limit_hits`; `crates/substrate-wire/src/lib.rs:2149-2184`), which the daemon runner asks for
(`story:substrate-daemon-runner`, `TestExecution.usage`). A process-count hit ends nothing and is only
that counter. `story:bound-program-result-table` already looks up `timeout` when a program times out
and falls to `*`.

## Waits on

- `story:action-limits-enforced`: the limits and their configured values, `enforced`, and the files
  it edits before this story (`ess/domains/bindings.yaml`, `ess/domains/tree.yaml`,
  `ess/domains/engine.yaml`, `crates/loom-tree`, `crates/loom-intake-slice/src/confinement.rs`).
- `story:substrate-daemon-runner`: `ended_by`, the daemon's refusal code on the applied record,
  and `usage`. This story fills both in process from the host driver as well.
- `story:agent-node-run-records`: the last story to add to `RunLogEvent` in
  `ess/domains/runlog.yaml` before this one.

`story:tree-waits-open-decision-requests` (`epic:decision-requests-in-the-runtime`) also adds
variants to `RunLogEvent` in `ess/domains/runlog.yaml`, adds to `TreeStatus` in
`ess/domains/tree.yaml`, and edits `crates/loom-tree` and `crates/loom-cli/src/tree.rs`, and no
edge orders the two: of this story and that one, the second to land rebases onto the first in each
of those files.

## ESS first

`ess/domains/engine.yaml`: `LimitKind` (`Memory`, `CpuTime`, `WallTime`, `Processes`), and
`StepConfinement` gains optional `memory_peak_bytes`, `processes_peak` and `process_limit_hits`,
filled from the runner's `usage`. `ess/domains/runlog.yaml`: `RunLogEvent` gains `LimitHit`
(action, `loom.engine.LimitKind`, `configured` integer, `enforced`, optional `measured` integer).
`ess/domains/bindings.yaml`: `ProgramRun` gains optional `limit` (`loom.engine.LimitKind`, the kind
that ended it). `ess/domains/tree.yaml`: `TreeStatus` gains `limit_hits`, a list of `CaseLimitHits`
(case id, count); `story:tree-run-command` names no per-case type, so this story adds one. On that
commit `task drift` fails and `crates/loom-tree/tests/limit_events.rs` does not compile. The code is
`child_result` and `live_result` in `crates/loom-intake-slice/src/confinement.rs`, the event in
`crates/loom-tree`, the result lookup in `crates/loom-bindings` and the `tree status` printer in
`crates/loom-cli/src/tree.rs`.

## Acceptance

Each bullet runs with stub programs under the route it names. "Both" means under `Daemon` with a
managed daemon, and under the in-process `linux` adapter on the host-driver route, on a host whose
driver proves `limits` and publishes `exec_resource_usage`, gated as the delegated tests are.

- Both: a program stopped at its memory limit gives one `LimitHit` line of kind `memory` whose
  `configured` is the configured bytes and whose `measured` equals the exec's `memory_peak_bytes`;
  one stopped at its CPU limit a `cpu-time` line in milliseconds with `measured` the CPU time used;
  one stopped at `timeout_seconds` a `wall-time` line in seconds with no `measured`; each says
  `enforced: true`.
- Both: a child ended by `exec.memory-limit` and one ended by `exec.cpu-limit` each come back from
  `run_child` as a result with no exit status and that `ended_by`, not as `CapabilityUnserved`.
- Both: a program whose forks failed at the process limit and that then exited 0 gives one
  `LimitHit` line of kind `processes` with the process peak, and the step's result comes from
  exit 0.
- Both: a step with `results: {"limit:memory": exhausted, "*": failed}` records `exhausted` when
  the memory limit ended it; one with `results: {"timeout": slow, "*": failed}` records `slow` when
  the wall limit ended it and its `ProgramRun` has `limit` `wall-time`; one with only
  `{"limit:wall-time": x, "*": failed}` records `failed`.
- Both: a program that prints `exec.memory-limit` and exits 1 gives no `LimitHit` line.
- Both: a kill by signal 9 with no refusal code gives no line.
- In process, the start input built for a driver whose facts publish `exec_resource_usage` carries
  `ExecMeasurement::ResourceUsage`, and the one built for a driver that does not carries none (a
  unit test in `crates/loom-intake-slice/src/confinement.rs`, no driver).
- In process, `settle` given an ended observation with the refusal `exec.memory-limit` and no
  `usage` returns `ended_by` `exec.memory-limit` and no `usage` (a unit test, no driver).
- `crates/loom-tree` builds from a result with `ended_by` `exec.memory-limit` and no `usage` one
  `memory` line with no `measured`, and from a result with no `usage` no `processes` line.

Under `local` with the engine's limits declared advisory, a sleep past `timeout_seconds` gives one
`wall-time` line that says `enforced: false`.

- `tree status` reports the number of `LimitHit` lines per case, and `tree verify` exits 0.
- `docs/contracts/run-log-v1.json` is regenerated and names `LimitHit`.
- `task check` exits 0.

## Out of scope

A per-connection event for a refused connection: Substrate reports none. Output and egress bytes:
neither has a ceiling in this epic. A `LimitHit` for an agent harness
(`story:agent-session-through-substrate`). Alerting. Memory, CPU and process-count hits under
`local`, which serves no limits and reports no usage.
