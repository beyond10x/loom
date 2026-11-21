---
format: aep.planning-md/3
id: story:capacity-given-to-an-admission-program
kind: story
status: draft
title: 'A program bound with capacity: true reads the run slots, the engine''s ceilings, the tree''s budget and its usage per run and since tree open on standard input'
summary: loom.capacity added after revalidation; slots at the step's start; recorded on the action started line; not given to arguments_from; Loom decides nothing from it
tags:
- sdc
relations:
- decomposes: epic:concurrent-and-standing-case-trees
- serves: vision:O1
- depends_on: story:concurrent-cases-in-the-tree-daemon
- depends_on: story:standing-tree-wakes-at-a-host-instant
revision: 4
---
## Outcome

A `program` executor in a `loom.bindings/1` document may say `capacity: true`. The program's
standard input, the arguments object (`story:bound-program-result-table`), then also holds the
member `loom.capacity`, a `loom.tree.Capacity` taken when the step starts:

- `served` (whether a daemon runs the tree), `run_slots`, `running` (cases of every served tree
  holding a slot, this step's included), `queued` (runnable cases of every served tree waiting for
  one) and `free` (`run_slots` minus `running`); a direct `tree run` gives `served: false`, one
  slot, one running, none queued, none free. The slot figures are those of one instant, the step's
  start; a slot freed or taken a moment later is not in them;
- `ceilings`: the engine configuration's `limits` section as loaded
  (`story:action-limits-enforced`: per action the ceiling and default of memory bytes, CPU
  milliseconds, processes and `timeout_seconds`; per run `wall_seconds` and `cpu_millis`), absent
  when the engine has none;
- `budget`: the tree's `TreeBudget` in force, its model budget included (`--max-agent-calls`,
  `--max-model-cost-micro-usd`, `story:tree-usage-and-model-budget`);
- `used`: the tree's `TreeUsage` for the current run (the sum of step wall times and the elapsed
  wall time, CPU time, steps, agent and model calls, tokens and cost) and beside it `since_open`,
  the same figures since `tree open` (for a standing tree the run starts at its latest wake,
  `story:standing-tree-wakes-at-a-host-instant`; session `sdc`, 2026-11-21: "the capacity member
  given to an admission program carries both, so an admission rule can bound a standing tree's
  total spend").

The member is added after revalidation, by the program port, so the request Commission revalidates
and the arguments a protocol's schema checks do not hold it. The step's `action started` line in
the run log carries the capacity it was given, so what an admission read is recorded. An
`arguments_from` program is not given it. Loom decides nothing from it: the program does.

## Why

"story count ≤ free run slots: the engine's configured ceilings (... ceilings in engine config, a
binding may only lower them) and free build slots (`slot status`), and the remaining model budget"
(`~/beyond10x/sdc/docs/orchestration.md` § 2.4, rule 3); blocker `loom-run-ceilings`: "the
configured run ceilings (a binding may only lower them) and the free run slots are readable by a
program, so `wave.admit` rule 3 can count them"; session `sdc`, 2026-11-21, change 6: the capacity
member carries "the tree's model budget and what is used (calls and cost, per tree; tokens if you
have them)", and change 5: both wall figures. The ceilings live in the engine configuration
(`story:action-limits-enforced` § Outcome: "The engine configuration's `limits` section holds, per
action, a ceiling and a default") and the slots in the daemon (`story:tree-daemon` § Outcome); a
bound program runs confined, with "its arguments as JSON on standard input and a cleared
environment" (`story:bound-program-result-table` § Outcome), so it can reach neither the daemon's
socket nor the engine file. Standard input is the one channel it has. Free build slots of `slot`
are the host's.

## Waits on

- `story:concurrent-cases-in-the-tree-daemon`: slots counted per running case, which `running`,
  `queued` and `free` report; the story before this one on `ess/domains/tree.yaml`,
  `ess/domains/bindings.yaml` and the scheduler in `crates/loom-tree`.
- `story:standing-tree-wakes-at-a-host-instant`: the run's starting line and `since_open`, which
  `used` reads; the budget counters in `crates/loom-tree`.
- Through them, `story:tree-usage-and-model-budget` (`TreeUsage`, the model budget),
  `story:run-caps-per-arrival` (the earlier story on `crates/loom-bindings/src/decode.rs`,
  `crates/loom-bindings/src/load.rs` and the program port), `story:tree-daemon` and
  `story:action-limits-enforced` (`LimitsSection` in `ess/domains/engine.yaml`).

Comes before, on shared files: `story:flow-limits-recorded-as-events` (`Capacity` in
`ess/domains/tree.yaml`, `ess/domains/runlog.yaml`, `crates/loom-tree`).

## ESS first

`ess/domains/tree.yaml`: `Capacity` (`served`, `run_slots`, `running`, `queued`, `free`, optional
`ceilings` as `loom.engine.LimitsSection`, `budget` as `TreeBudget`, `used` as `TreeUsage` with
`since_open`). `ess/domains/bindings.yaml`: `ExecutorSpec` gains optional `capacity` (`Program`
only). `ess/domains/runlog.yaml`: `action started` gains optional `capacity`. On that commit `task
drift` fails and `crates/loom-cli/tests/tree_capacity.rs` does not compile.

## Acceptance

With files added to `crates/loom-cli/tests/fixtures/area/`: `admit.protocol.yaml` (`admit/1`, an
action `wave.admit` bound to `bin/copy-stdin`, a stub that copies its standard input into a placed
file and exits 0, with `capacity: true`, after one program step of 1 s), `admit.bindings.yaml`,
`admit-from.bindings.yaml` (the same action also with an `arguments_from` program that copies its
own standard input to a second file) and `engine-limits.yaml` (adapter `daemon`, two run slots, a
`limits` section with known values); the `bin/gate` stub of
`story:concurrent-cases-in-the-tree-daemon`; a managed Substrate daemon; no model
(`crates/loom-cli/tests/tree_capacity.rs`):

- Direct `tree run --max-agent-calls 5 --max-model-cost-micro-usd 9000`: the copied object's
  `loom.capacity` has `served: false`, `run_slots: 1`, `running: 1`, `queued: 0`, `free: 0`, the
  `limits` section of `engine-limits.yaml` as `ceilings`, and `budget` with 5 agent calls and 9000
  micro-USD.
- `used.wall_millis_sum` equals the sum of the wall times of the tree's `action finished` lines
  before the admission's `action started` line, and `used.wall_millis_elapsed` the span of those
  lines, both read with the run log reader of `b10x-loom-runlog`; `used.steps` is 1.
- Through the daemon under `engine-limits.yaml`, a tree `A` whose `bin/gate` step holds one slot,
  then the admission tree, then a tree `C` submitted while both run: the admission reads
  `served: true`, `run_slots: 2`, `running: 2`, `queued: 1`, `free: 0`.
- A standing admission tree woken twice: at the second wake's admission `used` counts only that
  wake's steps and `since_open` counts both wakes'.
- Under an engine with no `limits` section, `ceilings` is absent.
- The `action started` line of the admission step holds the same capacity as the program read.
- Under `admit-from.bindings.yaml`, the `arguments_from` program's copied input holds no
  `loom.capacity`; the admission program's does.
- An action without `capacity: true` receives its arguments exactly as before (byte-identical
  standard input).
- A bindings file with `capacity` on an `agent`, `open` or `person` executor is refused at load
  naming the action and the member.
- `task check` exits 0.

## Files

`ess/domains/tree.yaml`, `ess/domains/bindings.yaml`, `ess/domains/runlog.yaml`, generated
`generated/rust/loom/src/{tree,bindings,runlog}.rs`; `crates/loom-bindings/src/decode.rs`,
`crates/loom-bindings/src/load.rs`, the program port (named by `story:bound-program-result-table`,
not read); `crates/loom-tree` (taking the capacity from the scheduler and the counters);
`crates/loom-cli/tests/tree_capacity.rs` (new); `crates/loom-cli/tests/fixtures/area/` (adds
`admit.protocol.yaml`, `admit.bindings.yaml`, `admit-from.bindings.yaml`, `engine-limits.yaml`,
`bin/copy-stdin`); `CHANGELOG.md`.

## Out of scope

Flow limits (`story:flow-limits-recorded-as-events`). Build slots of `slot`. Loom refusing a step
on its capacity. Reserving slots for an admitted wave.
