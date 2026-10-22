---
format: aep.planning-md/3
id: story:host-stop-before-step
kind: story
status: active
title: A host's own stop, read before every step, ends the run suspended for it
summary: LoopContext host stop for takt's ActiveTime and DiskLow
relations:
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-22T00:08:43Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-22T00:08:43Z", actor: "human:timo", revision: 4}
---
## Outcome

A host can stop a run before any step for a reason only the host can read (its active time less
its own pauses, the free space of a disk), and the run ends suspended for that reason, named as the
host named it. A host that supplies no stop gets 0.32.0's behaviour.

## Why

takt's `dependency-blocker:loom-runtime-stops` (§ Against Loom 0.32.0, last paragraph) needs two
stops Loom has no limit for:

- `ActiveTime`: wall time since the run opened less the time takt paused on the forge inside an
  effect's `invoke` (`crates/takt/src/run.rs:8639-8660` in takt). Commission cannot see a pause
  inside an effect, so it cannot measure this; `max_duration_ms` sums reported durations instead.
- `DiskLow`: free space on the file system of the case's tree or build directory under a
  threshold, read before every step (`run.rs:8678-8730`). Commission core reads no disk.

Both are read by takt before every step, where Loom reads the deadline. One host port covers both
and any later host-only stop, and keeps disk and pause accounting out of Commission core.

## Design

- ESS: `commission.responsibility.HostStop { name: String, detail: Json }`, and
  `SuspensionReason` gains `HostStop: commission.responsibility.HostStop`. `name` is the host's
  label (takt: `ActiveTime`, `DiskLow`); `detail` is the host's own data, which Commission never
  reads.
- `LoopContext` gains `fn host_stop(&mut self) -> Option<HostStop>`, default `None`. The runtime
  asks it where it reads the deadline before a step, right after the deadline: before each
  executor call and before a pending request is invoked again. `Some` suspends the Run for
  `HostStop` and ends the run `Suspended`, or `Waiting` while a request is pending, as at any
  budget stop. A stop that takes no step (`NoAdmissibleAction`, `AwaitingApproval`,
  `NoPerformableAction`) comes first, as it does for the deadline.
- A resume (`resume_after_grant`) asks it where it reads the deadline: before the Suspension is
  taken (a stop takes nothing and leaves the Suspension `Pending`) and right before the request is
  invoked (a stop releases the Suspension and performs nothing).
- `host_stop` is asked once per check and never after the run ended. The model never supplies it.
- `FileSuspensionStore` round-trips a `HostStop` reason; the conformance codec encodes it.

## ESS first

`ess/commission/domains/responsibility.yaml` gains `HostStop` and the `SuspensionReason` variant;
`task commission:generate` regenerates. Red tests in
`crates/loom-commission-testkit/tests/host_stop.rs` (new).

## Acceptance

- `a_host_stop_before_the_first_step_calls_no_executor`: a context answering
  `HostStop { name: "DiskLow", .. }` at once ends the run suspended for it with 0 executor calls.
- `a_host_stop_after_one_step_stops_before_the_next`: answering `None` once then `Some` calls the
  executor once; the run ends suspended for `HostStop`, its `detail` as the host gave it.
- `a_host_stop_while_a_request_is_pending_ends_the_run_waiting`: a pending request is not invoked
  again; the run ends `Waiting` naming the request.
- `a_resume_the_host_stops_takes_nothing`: the Suspension stays `Pending` and nothing is performed.
- `a_host_stop_round_trips_through_the_file_suspension_store` (`crates/loom-governor/tests/`).
- With the default `host_stop`, every existing Commission test stays green unchanged.

## Not in scope

- Measuring active time or free space in Loom. The host does.
- The per-limit breach choice (`story:breach-choice-per-limit`).

## Scope

- `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/`
- `crates/loom-commission/src/runtime.rs`, `crates/loom-commission/src/lib.rs`
- `crates/loom-commission-testkit/tests/host_stop.rs` (new)
- `crates/loom-governor/src/file_suspension.rs` and its tests
- every exhaustive `match` on `SuspensionReason` in the workspace (`rg 'SuspensionReason::'`),
  the intake slice and the CLI's stop names included
- `crates/loom-commission-conformance/src/codec.rs`
- `AGENTS.md`, `CHANGELOG.md`, `website/docs/reference/commission/` (regenerated)
