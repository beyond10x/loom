---
format: aep.planning-md/3
id: story:concurrent-confined-runs
kind: story
status: proposed
title: Two case trees in one process run confined programs at the same time, because every runner of the process shares one driver per cgroup root
summary: one table of Substrate drivers for the whole process; two launches that share a driver run; a launch needing another directory or egress stays refused CapabilityUnserved; every static of the runtime crates listed and harmless
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/tests/bound_program.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/engine_confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/support/mod.rs
- confidence: cited
  path: website/docs/guides/embed-the-runtime.md
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:06:13Z", actor: "agent:loom", revision: 5}
---
## Outcome

Two case trees, each with its own store, workspace and `linux`-confined bound programs, run at
the same time in one process, each from its own plain thread, each through its own
`EngineRunner`. Neither run is refused, neither program sees the other's workspace, and
neither sees the other's processes.

**The form: one table of opened Substrate drivers for the whole process.** Today every
`SubstrateRunner` keeps its own drivers (`Drivers`, `crates/loom-intake-slice/src/confinement.rs:
249-252`, a new `Arc::default()` per runner at `:422-427`). The second run's runner therefore
opens a second driver under the same cgroup root. Opening a driver ends every confined run under
that root, so the open is refused while the first run's launch runs (`:508-519`). This story
replaces the per-runner map with one table for the process. The table is keyed by the canonical
cgroup root, the directory that holds the workspace and the `DriverEgress` (`:274-278`). It
holds each driver weakly. Every runner keeps strong references to the drivers it used, so a
driver is closed when the last runner that used it is dropped. Two runs whose launches need an
equal key share one driver. Their launches then run side by side, as two launches of one runner
do today (`crates/loom-intake-slice/tests/adversary_w20261020w2_u4.rs:104-159`).
`RUNNING` stays as it is. It is already keyed by the canonical root (`:380-385`, `:401-419`,
`:485-486`), so a launch under root A never refuses a driver under root B.

**Why not one cgroup root per run.** Runbook's preferred form fails in Substrate, not in Loom.
Substrate's probe proves `limits` only for a root that holds the calling process's own cgroup
(Substrate `crates/substrate-host/src/probe.rs:931-948` at rev `adfde402`, the rev
`crates/loom-intake-slice/Cargo.toml:30-31` pins). One process sits in one leaf. Sibling roots
created per run therefore hold no process, and every run under them would be refused
`CgroupUndelegated` (`confinement.rs:3441-3462`). Changing that is a change to Substrate. The
table is also the smaller change: it touches one file and no type, refusal or ESS member.

**What a library caller does.**

1. Run the process in a delegated cgroup v2 subtree with the process in a leaf below the root.
   `prepare_delegated_cgroup` (`:3631-3641`) does this inside a scope that
   `systemd-run --user --scope -p Delegate=yes` started.
2. Give every run the same `cgroup_root` in `EngineConfinement`, or none (the discovered root,
   `discover_cgroup_root`, `:3615-3628`).
3. Give each run its own workspace directory, and make all these directories children of one
   shared directory, for example `<state>/workspaces/<run>`.
4. Open one `EngineRunner` per run, or share one. Both work.

A launch that needs a driver the table does not hold, while another launch runs under the same
root, is still refused `CapabilityUnserved` with today's text (`:512-518`). Two cases trigger
this: a workspace in another directory, and an egress set or certificate bundle that no running
driver has. The guide says so. Bound programs name no egress (`bound_program_child`,
`crates/loom-intake-slice/src/bindings.rs:414-450`), so for them the directory rule is the only
one.

**Statics that stay.** These are the statics of the crates `b10x-loom-tree` links. None lets
one run change another run's result:

- `RUNNING` (`confinement.rs:381`) counts the launches per root. It only refuses a driver open
  that would end a running launch. It never ends a launch.
- The new driver table. It shares a driver only between launches with an equal key. A shared
  driver runs at most 16 executions at once and tracks 128 (Substrate `HostConfig::minimum`,
  `crates/substrate-host/src/lib.rs:529-530`). A direct `tree run` has one launch in flight.
- `NEXT_EXEC` (`confinement.rs:70`), `ISSUED` (`crates/loom-bindings/src/program.rs:909`,
  `crates/loom-intake-slice/src/executor.rs:1666`), `NEXT_TEMPORARY`
  (`crates/loom-governor/src/locked_dir.rs:34`, `crates/loom-intake-slice/src/state_dir.rs:35`),
  `NEXT_SCOPE` (`crates/loom-intake-slice/src/results.rs:21`) and `MADE`
  (`crates/loom-executor/src/lib.rs:278`). These are counters that make names unique. No run
  reads another run's value.
- `ENGINEERING` (`crates/loom-governor/src/lib.rs:1956`), the compiled-in catalog, and the
  regular expressions of `crates/loom-intake-references/src/lib.rs:96-123`. All are immutable.
- `RECORDINGS` (`crates/loom-intake-slice/src/git.rs:129`). It is keyed by the canonical
  workspace and replaced only for the same workspace (`:194-200`). It serves the intake slice's
  host git, not bound programs.
- `OPEN` and `TURNING` (`crates/loom-executor/src/metering.rs:346-357`). These are thread-local,
  and each run has its own thread.

## Why

Runbook will link Loom's crates into one binary. Its daemon will run many case trees in one
process from plain threads, because the runtime blocks (`run_until_blocked` is a plain `fn`,
`crates/loom-commission/src/runtime.rs:1194`). Timo: "multiple such runs should be possible to
run there", "in an isolated, secure way". Today the second tree's first confined program is
refused while the first tree's program runs. Concurrency inside one tree is the daemon's
(`story:concurrent-cases-in-the-tree-daemon`). Across trees with separate runners there is none.

## ESS first

`ess/` does not change. No type, refusal or member is added: the refusal kept is
`CapabilityUnserved`, and `EngineConfinement` keeps its three members
(`ess/intake/domains/confinement.yaml:23-28`). `slot task drift` stays green on every commit.

The first commit adds the cgroup-free tests of the table to the `tests` module of
`crates/loom-intake-slice/src/confinement.rs`. They call the table over a fake opener that
counts its calls and opens no Substrate driver. On that commit
`cargo nextest run -p b10x-loom-intake-slice --lib -E 'test(driver_table)'` fails to compile
because the table does not exist. That is the red check the unit records.

## Acceptance

Only the first test needs a delegated cgroup v2. It and the three other delegated tests use the
harness of `crates/loom-intake-slice/tests/support/mod.rs:37-58`: the test binary restarts
itself under `systemd-run --user --scope -p Delegate=yes`. Where bubblewrap, a user systemd
manager or the driver is missing, the test prints `skipped: missing prerequisite: …` and
returns. Off Linux it is `#[ignore]`. Each cgroup-free test runs everywhere with no model and no
network, and ends within 60 s. The `slow_` test runs in the `slow` lane (`.config/nextest.toml`)
and ends within 20 s. The default test step stays within 120 s. The new tests go into existing
test files, which become modules of `tests/main.rs` under `story:one-test-binary-per-crate`. No
test target is added.

- Two trees run at once. Each has its own store, its own workspace (sibling directories) and its
  own `EngineRunner` on `linux`. Each binds one program that sleeps 5 s and exits 0. They start
  from two plain threads 1 s apart. Both steps record exit 0, and the two runs together take less
  than 9 s of wall time. Test: `slow_two_trees_run_confined_programs_at_once_in_one_process`.
- In the same setup, the program of tree A reads tree B's marker file by its host path and as
  `../<B's workspace>/marker`, and both reads fail. Tree B's program, run the same way, does not
  find A's marker either. Test: `a_confined_program_sees_no_workspace_of_the_other_tree`.
- While tree A's program sleeps under a unique argv marker, tree B's program reads every
  `/proc/<pid>/cmdline` it can see, and none holds the marker. Test:
  `a_confined_program_sees_no_process_of_the_other_tree`.
- Two runners asking for one root, one directory and `DriverEgress::NONE` get the same driver
  (`Arc::ptr_eq`), and the fake opener is called once. Test:
  `driver_table_two_runners_share_one_driver_for_one_key`.
- While a launch is counted under root A (`Running::enter`), a driver for root B opens. Test:
  `driver_table_a_launch_under_one_root_does_not_refuse_another_root`.
- While a launch is counted under a root, a second runner that needs a driver for another
  directory under that root is refused `CapabilityUnserved` with today's detail. The opener is
  not called. Test: `driver_table_another_directory_under_a_running_root_is_refused`.
- Two launches under one root with unequal egress are refused in the same way. Test:
  `driver_table_another_egress_under_a_running_root_is_refused`.
- When the last runner that used a driver is dropped, the table no longer holds the driver, and
  the next runner's request calls the opener again. Test:
  `driver_table_a_driver_dropped_by_its_last_runner_leaves_the_table`.
- A root that is not delegated is still refused `CgroupUndelegated`, and nothing runs.
  `an_undelegated_root_is_refused_and_nothing_falls_back`
  (`crates/loom-intake-slice/tests/engine_confinement.rs:895-922`) is unchanged and green.
- A sibling cgroup of the delegated scope (one that does not hold the process), given as
  `cgroup_root`, is refused `CgroupUndelegated`. This pins why per-run roots are not the form.
  Test: `a_root_that_does_not_hold_this_process_is_refused_cgroup_undelegated`.
- `no_driver_is_opened_over_a_launch_running_under_its_cgroup_root` (`confinement.rs:4217-4241`)
  keeps its refusal. Its `Arc::ptr_eq` line over the per-runner map now checks two clones that
  share the table entry. `building_a_linux_runner_ends_no_run` (`engine_confinement.rs:854-874`)
  and `adv_a_second_child_launch_leaves_a_running_child_running` are unchanged and green.
- No static outside the list in § Outcome exists in the sources of `loom-intake-slice`,
  `loom-bindings`, `loom-governor`, `loom-commission`, `loom-executor`, `loom-protocols`,
  `loom-runlog`, `loom-tree` and `loom-intake-references`. The test reads each `.rs` file under
  their `src/` and fails on a `static`, `thread_local!`, `OnceLock` or `LazyLock` that is not in
  the list. Test: `every_static_of_the_runtime_crates_is_listed_harmless`.
- The guide and AGENTS.md state the caller's four steps. Command:
  `git grep -c 'one shared directory' -- website/docs/guides/embed-the-runtime.md AGENTS.md`
  prints a count of at least 1 for each file.
- `slot task check` exits 0, and `slot task drift` exits 0 on every commit of the unit.

## Out of scope

Any change to Substrate, including a driver that opens without ending other drivers' runs, and
per-run cgroup roots. Changes to Canon. Changes to the CLI's default (`--confinement`, the
delegated re-exec). Running cases of one tree at once, which is
`story:concurrent-cases-in-the-tree-daemon`. Two runs that share one workspace. Children that
name different egress sets under one root at the same time; they stay refused. The fixed text
`UNCONFINED_FLAG_REASON` (`bindings.rs:79-80`) stays; Runbook decided no story for it.
`ConfinedChild` and `spawn_child` (`confinement.rs:643-669`) keep their behaviour and share the
table like every launch.

## Scope

Derived 2026-11-22 from the plan tree at `1d6757d1` (`wave/2026-11-21-w7`). **Cited** means read
at the `file:line` named. **Inferred** means a reading that could be wrong.

- **Primary surface:** `crates/loom-intake-slice/src/confinement.rs`: `SubstrateRunner`,
  `Drivers` (`:243-252`), `SubstrateRunner::new` (`:421-427`), `driver` (`:436-477`), `open`
  (`:479-569`; the root's resolution `:487-506` moves ahead of the lookup, since the key needs
  the canonical root), and `RUNNING` (`:380-419`, unchanged) — cited
- **Files (tests, cgroup-free):** the `tests` module of `confinement.rs` (beside `:4217-4241`):
  the `driver_table_*` tests and `every_static_of_the_runtime_crates_is_listed_harmless` —
  cited for the module, inferred for the placement of the static test
- **Files (tests, delegated):** `crates/loom-intake-slice/tests/engine_confinement.rs`
  (`linux_confinement` `:845-852`, the `linux` section `:843-922`): the workspace, process and
  root tests — cited. `crates/loom-intake-slice/tests/support/mod.rs` (`delegated` `:37-58`
  hands one `SubstrateRunner` and one workspace; a second workspace in the same directory is
  made by the case, as `adversary_w20261020w2_u4.rs:107-109` does) — cited
- **Files (tests, two trees):** `crates/loom-bindings/tests/bound_program.rs`: two governors,
  stores and workspaces, each `ProgramEffects` (`program.rs:132-150`) with its own `EngineRunner`
  on `linux`, reached by `#[path]` to the intake slice's `support` module — inferred (no
  bindings test uses the delegated harness today)
- **Files (docs):** `website/docs/guides/embed-the-runtime.md:68-69` (the cgroup root paragraph)
  gains the four steps and the refusal that stays; `AGENTS.md:2461-2468` (§ Intake and the
  command line, "A runner and its clones share one driver…") becomes "every runner of the
  process"; `CHANGELOG.md` gets one `Changed` bullet under `[Unreleased]` (`:13`, `### Changed`
  `:27`) — cited
- **Not changed:** `ess/intake/domains/confinement.yaml`, `generated/rust/intake`,
  `crates/loom-intake-slice/src/confinement/engine.rs` (`EngineRunner::open`, `:89-117`, builds
  `SubstrateRunner::new` as before), `crates/loom-intake-slice/src/bindings.rs` — cited
- **Would collide with:** `story:cancel-a-running-bound-program` on `confinement.rs` (it comes
  after this story); `story:concurrent-cases-in-the-tree-daemon` on nothing named here
  (`crates/loom-tree`, `crates/loom-cli`) — cited from its Scope; `story:per-run-program-path`
  on `AGENTS.md` and `CHANGELOG.md` only, in other sections — inferred
