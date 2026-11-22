---
format: aep.planning-md/3
id: story:mixed-egress-launches
kind: story
status: proposed
title: A confined launch that needs another egress set waits for the driver instead of being refused, bounded by its own timeout
summary: a launch under a root whose driver serves launches of another directory or egress set waits first come first served; the wait is bounded by the launch's own timeout; a timeout is DriverBusy, not CapabilityUnserved; two run-log lines per wait; no Substrate change
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:concurrent-confined-runs
- depends_on: story:cancel-a-running-bound-program
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_program.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/engine_confinement.rs
- confidence: inferred
  path: crates/loom-tree
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: cited
  path: ess/intake/domains/confinement.yaml
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:12:55Z", actor: "agent:loom", revision: 6}
---
## Outcome

A confined launch that needs a Substrate driver the process has not opened yet does not get
`CapabilityUnserved` because another launch is running under the same cgroup root. It waits for
the driver, bounded by its own timeout. The wait is two run-log lines. A wait that runs out is a
typed refusal that records nothing. A cancel handle ends a wait.

**The form: (a), waiting in Loom.** It needs no change to Substrate. The code shows why it is
right:

- Only opening a driver ends other launches. `HostDriver::open` reconciles the root
  (`ProcessRuntime::new_with_quotas`, Substrate `crates/substrate-host/src/process.rs:479-500` at
  rev `adfde402`). `reconcile_orphans` (`:504-542`) kills every `substrate-ex_*` cgroup directly
  under the root, whichever driver made it.
- Starting an execution on a driver that is already open reconciles nothing. So once two
  drivers with different egress sets are open under one root, their launches run side by side.
  The refusal (`crates/loom-intake-slice/src/confinement.rs:510-519`) guards only the open.
- At a moment when no launch of the process runs under the root, an open ends nothing that
  matters. Every launch drops its `Release` before its `Running` count is dropped:
  `_release` is declared after `prepared` (`:680-687`, `:720-723`), so it is dropped first, and
  finished executions are discarded before the count reaches 0. Each driver keeps its capsules
  in its own state directory (`driver_config`, `:342-356`). The aperture directories under the
  shared `.substrate-apertures` of the workspace directory (Substrate `HostConfig::minimum`,
  `crates/substrate-host/src/lib.rs:520`) belong to running executions only, and
  `reconcile_apertures` (`process.rs:687-756`) removes them only at an open.

So (a) is correct as long as the open waits for the root's count to reach 0 while holding the
`RUNNING` lock, as the open does today (`confinement.rs:510-526`). There is one condition. A
waiting open must hold back new launches on drivers that are already open, or a steady stream of
compatible launches keeps the count above 0 forever.

**Form (b) needs Substrate.** Several drivers that serve at once under one root require a driver
that reconciles only the cgroups it made, for example under a name prefix per driver, in place of
every `substrate-ex_*` (`process.rs:504-542`). A second root is no way out: Substrate proves
`limits` only for a root that holds the calling process (`crates/substrate-host/src/probe.rs:
931-948`), and the process sits in one leaf. This story does not take (b). Upstream note, not a
blocker: Substrate would have to serve "a driver opened under a root that ends only its own
orphaned executions", read by `reconcile_orphans`. It is to be filed if the waits measured by the
run-log lines below are too long.

**The waiting rules.** These hold per canonical cgroup root, in `confinement.rs`, beside `RUNNING`
and the driver table of `story:concurrent-confined-runs`:

- **Who waits.** A launch whose key (root, workspace directory, `DriverEgress`) has no open
  driver, while a launch of the process runs under the root.
- **First come, first served.** Each launch under a root takes a ticket in arrival order before
  it opens or starts. A launch on an open driver starts at once only when no earlier ticket is
  waiting. Otherwise it waits behind that ticket, even though its own driver is open. Tickets are
  served in order, so while a launch waits no new launch starts under the root.
- **No starvation.** The launches running when a waiter arrived end within their own timeouts
  plus `DRAIN_MARGIN` (Substrate ends each at its timeout, and Loom kills it 30 s later,
  `:807-823`).
  The waiter then opens its driver, and the tickets behind it go on in order. Launches behind it
  whose drivers are open start together.
- **The bound.** The waiting launch's own timeout (`ChildCommand::with_timeout`, the binding's
  `timeout_seconds`). The run that follows a wait gets its whole timeout, so a step takes at most
  twice its timeout.
- **A wait that runs out.** It is refused `ConfinementRefusal::DriverBusy`, which is not
  `CapabilityUnserved`. The detail and the typed holders name the egress set of each driver that
  has a running launch: the aperture names in order, whether it trusts a certificate bundle (the
  path is not given), and whether it serves another workspace directory. The waiter's ticket is
  removed, and nothing is opened.
- **A cancel.** The handle of `story:cancel-a-running-bound-program` (`Cancel`, given through
  `ChildCommand::with_cancel`) is read every 100 ms while the launch waits. A cancel ends the wait
  with `Cancelled` and removes the ticket, and nothing starts.
- **Run-log lines.** `ChildCommand::with_wait_notice(notice)` hands the launch a notice, which
  `ProgramEffects::with_wait_notice` passes on to the bound program's child and the
  `arguments_from` child. It is told once when the wait starts (the holders) and once when it ends
  (the whole seconds waited, rounded up, and the end: `Opened`, `TimedOut` or `Cancelled`).
  `run_tree` gives a notice that appends `launch waiting` (`action`, `holders`) and
  `launch waited` (`action`, `seconds`, `end`) to the case's run log. A launch that does not wait
  writes no line.
- **The step after a wait that ran out.** `ProgramEffects::perform` (`program.rs:177-434`)
  answers `Refused` with `ProgramRefusalReason::DriverBusy` and `ends_run: true`. It keeps no
  observation, because the program never started, so the wait does not count against
  `budget.max_runs`. It writes no evidence and no revision move. A wait ended by a cancel is the
  same, with `Cancelled`. The port knows from the notice's end that nothing started.

**What changes in `story:concurrent-confined-runs`.** Its two refusals become waits:
`driver_table_another_directory_under_a_running_root_is_refused` and
`driver_table_another_egress_under_a_running_root_is_refused` are replaced by this story's
`driver_gate_*` tests. Its four steps for a library caller stay as they are.

**Order.** This story comes after `story:cancel-a-running-bound-program` (the `Cancel` type).
`story:bound-agent-node` and `story:agent-node-run-records` start no launch with egress
through Substrate: the agent session's harness stays under metaharness, and "Confining the session
by Substrate" is out of scope there (`bound-agent-node.md:112-115`). So neither has an edge to
this story. The first story that gives a tree step egress under Substrate is
`story:named-egress-per-action` (draft), and `story:agent-session-through-substrate` (draft)
depends on that story. The edge: `story:named-egress-per-action` depends_on
`story:mixed-egress-launches`.

## Why

The planner's daemon runs several trees in one process. Once a step selects egress (an agent
node's model calls), run A's step with egress and run B's test program without egress overlap at
times nobody controls. Under `story:concurrent-confined-runs` the second of them to open its
driver is refused `CapabilityUnserved` (`confinement.rs:510-519`), and a correct run loses a step
to a race. Today a bound program names no egress (`bound_program_child`,
`crates/loom-intake-slice/src/bindings.rs:414-450`), so this has not happened yet. It happens the
day `story:named-egress-per-action` lands. Timo: runs "in an isolated, secure way". Waiting keeps
the isolation and removes the race.

## ESS first

`ess/` changes, with new members of declared types:

- `ess/intake/domains/confinement.yaml:12-14`: `ConfinementRefusal` gains `DriverBusy`. New:
  `intake.confinement.DriverHolder` (`apertures: List<String>`, `ca_bundle: Boolean`,
  `other_directory: Boolean`) and `intake.confinement.DriverWaitEnd` (`Opened`, `TimedOut`,
  `Cancelled`).
- `ess/domains/bindings.yaml:345-353`: `ProgramRefusalReason` gains `DriverBusy`.
- `ess/domains/runlog.yaml:36-50`: `RunLogEvent` gains `LaunchWaiting` (`action`,
  `holders: List<loom.runlog.LaunchHolder>`) and `LaunchWaited` (`action`, `seconds: Integer`,
  `end: loom.runlog.LaunchWaitEnd`), with `LaunchHolder` and `LaunchWaitEnd` spelled as the
  intake types are.

On that commit `slot task drift` and `slot task intake-drift` fail, and the `driver_gate_*` tests
in the `tests` module of `confinement.rs` do not compile.

## Acceptance

The cgroup-free tests drive the gate over a fake opener and fake running launches
(`Running::enter`, as `confinement.rs:4217-4241` does), with no Substrate driver, no model and no
network, and each ends within 5 s. The two delegated tests are `slow_`. They use the harness of
`crates/loom-intake-slice/tests/support/mod.rs:37-58`, skip naming the missing prerequisite where
there is no delegated cgroup v2 or the driver publishes no egress aperture, are `#[ignore]` off
Linux, and each ends within 30 s in the `slow` lane. Every test ends within 60 s, and the default
test step stays within 120 s.

- While a launch is counted under the root, a launch needing another key waits. It opens its
  driver within 200 ms after the count reaches 0, and the opener is called once. Test:
  `driver_gate_a_launch_needing_an_open_waits_until_the_root_is_idle`.
- A launch whose count never reaches 0 is refused `DriverBusy`, not `CapabilityUnserved`, after
  its timeout (1 s in the test) within 200 ms. Its holders name the running driver's aperture
  names, and the opener is never called. Test:
  `driver_gate_a_wait_past_the_timeout_is_refused_driver_busy`.
- Three waiters for three keys open in the order they arrived. Test:
  `driver_gate_waiters_open_in_arrival_order`.
- With a waiter queued, a launch on an already open driver does not start before the waiter
  opened. With launches on open drivers arriving every 50 ms, the waiter opens within the time
  the counted launches take to end. Test:
  `driver_gate_a_launch_on_an_open_driver_does_not_overtake_a_waiter`.
- A cancel set during a wait ends it within 200 ms with `Cancelled`. Its ticket is gone, and the
  next waiter is served. Test: `driver_gate_a_cancel_ends_a_wait`.
- Two keys whose drivers are both open run launches at the same time with no wait. Test:
  `driver_gate_open_drivers_with_different_egress_serve_side_by_side`.
- The notice is told `waiting` with the holders, then `waited` with the seconds rounded up and
  the end. A launch that did not wait tells it nothing. Test:
  `driver_gate_a_wait_is_reported_at_its_start_and_its_end`.
- A bound program whose runner answers `DriverBusy` after a reported wait refuses the step
  `DriverBusy` with `ends_run`. The governor holds no new observation, evidence or revision move,
  and with `budget.max_runs: 1` the next step still runs. Test (in
  `crates/loom-bindings/tests/bound_program.rs`):
  `a_step_whose_wait_ran_out_records_nothing`.
- A tree run whose step waited appends `launch waiting` and then `launch waited` before that
  step's `action finished` line, and the chain verifies. Test (in `crates/loom-tree`):
  `a_tree_run_writes_a_wait_as_two_lines`.
- A child with an egress aperture to a loopback listener runs 3 s. A child with no egress, in
  another workspace of the same directory, is launched 1 s later from another thread. The second
  waits about 2 s, then both exit 0. Test:
  `slow_a_launch_with_egress_and_one_without_wait_and_then_both_run`.
- After that, the same two children launched 1 s apart overlap with no wait, since both drivers
  are open. Test: `slow_once_both_drivers_are_open_mixed_launches_overlap`.
- `an_undelegated_root_is_refused_and_nothing_falls_back` (`engine_confinement.rs:895-922`) and
  `no_driver_is_opened_over_a_launch_running_under_its_cgroup_root` (`confinement.rs:4217-4241`,
  whose launch is never released and whose root therefore never idles; it now asserts
  `DriverBusy` after a 1 s timeout) pass.
- The generated code matches the specification: `slot task drift` and `slot task intake-drift`
  exit 0 after `task generate` and `task intake-generate`, and `slot task check` exits 0.

## Out of scope

Changes to Substrate, including form (b), and to Canon or the CLI's default. Launches of another
process under the same root, which Loom cannot count. The `local` adapter, which opens no driver.
A priority among waiters. Pre-opening the drivers a run will need. Keeping drivers open after their
last runner is dropped: `story:concurrent-confined-runs` closes them, so a key whose runs have all
ended waits again at its next open. The fixed text `UNCONFINED_FLAG_REASON` stays.

## Scope

Derived 2026-11-22 from the plan tree at `df62eaec` and Substrate rev `adfde402` (read with
`git show`). **Cited** means read at the `file:line` named. **Inferred** means a reading that could
be wrong.

- **Primary surface:** `crates/loom-intake-slice/src/confinement.rs`: `RUNNING` and `Running`
  (`:380-419`), `driver` (`:436-477`), the check and the open under the lock (`:508-526`),
  `Prepared::new`, where a launch enters the count (`:846-884`), `launch` and
  `launch_with_stdin` (`:675-804`), `ChildCommand::with_wait_notice` beside `with_timeout`
  (`:1432`) — cited; the driver table of `story:concurrent-confined-runs` — inferred (not built)
- **Files (source):** `crates/loom-bindings/src/program.rs`: `with_wait_notice` beside `new`
  (`:132-150`), the refusal path (`:323-330`) for `DriverBusy` with no observation (`:264-302`)
  — cited
- **Files (source):** `crates/loom-tree`: `run_tree`'s notice and the two lines — inferred
  (`run_tree` is not on `impl/tree-run-command` at `57728f01`)
- **Not changed:** `crates/loom-intake-slice/src/confinement/engine.rs` (the `local` route) and
  Substrate — cited
- **Files (ESS source):** `ess/intake/domains/confinement.yaml:12-14`,
  `ess/domains/bindings.yaml:345-353`, `ess/domains/runlog.yaml:36-50` — cited
- **Files (generated):** `generated/rust/intake/**`, `generated/rust/loom/**`,
  `docs/contracts/run-log-v1.json`, `website/docs/reference/ess/intake-confinement.md`,
  `loom-bindings.md` and `loom-runlog.md` — cited for the pages, inferred for the Rust paths
- **Files (tests):** the `tests` module of `confinement.rs` (`driver_gate_*`, the changed
  `:4217-4241`); `crates/loom-intake-slice/tests/engine_confinement.rs` (the two `slow_` tests;
  an egress child as `crates/loom-intake-slice/src/confinement/several_apertures.rs` builds one)
  — cited for the files, inferred for the loopback fixture; `crates/loom-bindings/tests/
  bound_program.rs` — cited; a test file of `crates/loom-tree` — inferred
- **Files (docs):** `AGENTS.md:2461-2468` (shared drivers, "the launch needing it is refused
  instead" becomes the wait) and § Run log (`:1392`); `website/docs/guides/embed-the-runtime.md:
  68-69`; `website/docs/guides/bindings.md` § Running a program binding (`:206`); `CHANGELOG.md`
  one `Changed` bullet (`:27`) — cited
- **Would collide with:** `story:concurrent-confined-runs` (the table, two of its tests) and
  `story:cancel-a-running-bound-program` (`ChildCommand`, `program.rs`, `ConfinementRefusal`,
  `ProgramRefusalReason`), which both land first; `story:per-run-program-path` on `program.rs`
  and `ess/domains/runlog.yaml` (`RunLogEvent`); `story:named-egress-per-action`, which depends
  on this story — cited from their Scope sections, inferred for the order
