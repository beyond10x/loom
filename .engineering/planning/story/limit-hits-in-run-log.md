---
format: aep.planning-md/3
id: story:limit-hits-in-run-log
kind: story
status: draft
title: Every limit a process hits is one event in its case's run log, and a result table can key on it
summary: 'From what Substrate observed: memory, CPU time, wall time, process count, output, egress bytes'
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:action-limits-enforced
- depends_on: story:substrate-daemon-runner
- depends_on: story:agent-node-run-records
revision: 1
---
## Outcome

Every limit a process hits is one event in its case's run log: the action, which limit (memory,
CPU time, wall time, process count, output bytes, egress bytes), the configured value, and where
Substrate reports it the measured peak. The event comes from what Substrate observed, never from
the program's output. The step's result table can key on the limit (`limit:memory`,
`limit:wall-time` and so on, beside `timeout`), so a protocol can turn "ran out of memory" into
an evidence result. `tree status` counts the limit events per case.

## Why

The live view must show every limit hit (session `sdc`, 2026-11-21). Substrate names the limit
that ended a run only on its daemon path: an `exec.cancelled` event with refusal code
`exec.timeout`, `exec.cpu-limit`, `exec.memory-limit` or `exec.aperture-byte-limit`
(`crates/substrate-host/src/process.rs:3280-3322`, read at 0.21.0). In process, `RunObservation`
carries an exit and a signal only, and `memory_oom_kills` only on `pod`
(`crates/substrate-host/src/confinement/mod.rs:681-706`). A process-count hit ends nothing and
is only a counter (`process_limit_hits`) when measurement was asked for.

## ESS first

`ess/domains/runlog.yaml`: `RunLogEvent` gains `LimitHit` (action, limit kind, configured value,
optional measured peak, enforced). `ess/domains/bindings.yaml`: the result table's keys gain the
`limit:` forms. On that commit `task drift` fails and
`crates/loom-tree/tests/limit_events.rs` does not compile.

## Acceptance

Through the Substrate daemon runner, with stub programs:

- A program stopped at its memory limit gives one `LimitHit` line with kind `memory` and the
  configured bytes; the same for `cpu-time` and `wall-time`.
- A program whose forks failed at the process limit and that then exited 0 gives one `LimitHit`
  line with kind `processes`, and the step is not refused for it.
- A program whose output was cut at the output limit gives one line with kind `output`.
- A step with `results: {"limit:memory": exhausted, "*": failed}` records `exhausted` when the
  memory limit ended it.
- A program that prints `exec.memory-limit` and exits 1 gives no `LimitHit` line.
- Under the in-process `linux` adapter, where Substrate names no reason, a run stopped at its
  wall time still gives a `wall-time` line (Loom set the deadline), and a kill by signal 9 with
  no named reason gives no line.
- `tree status` reports the number of `LimitHit` lines per case, and `tree verify` exits 0.
- `task check` exits 0.

## Out of scope

A per-connection event for a refused connection: Substrate reports none. Alerting.
