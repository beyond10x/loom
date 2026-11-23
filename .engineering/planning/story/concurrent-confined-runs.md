---
format: aep.planning-md/3
id: story:concurrent-confined-runs
kind: story
status: active
title: Two case trees in one process run confined programs at the same time, because every runner of the process shares one driver per cgroup root
summary: one table of Substrate drivers for the whole process; two launches that share a driver run; a launch needing another directory or egress stays refused CapabilityUnserved; every static of the runtime crates listed and harmless
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:child-record-in-the-run-log
- depends_on: story:tree-status-names-what-it-read
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
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:06:13Z", actor: "agent:loom", revision: 5}
- {from: "proposed", to: "active", at: "2026-11-22T10:51:45Z", actor: "agent:sdc", revision: 7, executor: "agent:loom", correlation: "runbook-daemon-concurrent-runs-2026-11-22"}
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
that root, so the open is refused while the first run's launch runs (`:514-525`). This story
replaces the per-runner map with one table for the process. The table is keyed by the canonical
cgroup root, the directory that holds the workspace and the `DriverEgress` (`:260-278`). It
holds each driver weakly. Every runner keeps strong references to the drivers it used, so a
driver is closed when the last runner that used it is dropped. Two runs whose launches need an
equal key share one driver. Their launches then run side by side, as two launches of one runner
do today (`crates/loom-intake-slice/tests/adversary_w20261020w2_u4.rs:110-173`).
`RUNNING` stays as it is. It is already keyed by the canonical root (`:380-385`, `:401-419`,
`:491-497`), so a launch under root A never refuses a driver under root B.

**The hold of a cgroup root stays per process.** `story:tree-run-takes-a-delegated-cgroup`
(merged at `ec1a82f9`) added `CgroupRootHold` (`confinement.rs:3653-3668`), `hold_cgroup_root`
(`:3670-3707`) and `check_cgroup_root` (`:3709-3779`). `hold_cgroup_root` takes an advisory
`flock` on the root's directory through a file it opens itself, then runs `check_cgroup_root`.
The kernel keys that lock by the open file description, so a second hold of the same root is
refused `CgroupUndelegated` even in the same process (`:3687-3694`; the detail says "held by
another process"). This story keeps that and builds on it:

- A process holds each root once. Every run of the process that uses the root shares that one
  hold: its `EngineConfinement` names `hold.root()`. A daemon that runs many trees takes the hold
  before its first run and drops it after its last.
- `SubstrateRunner` takes no hold, and neither does the new table. A driver opens on the root a
  runner names whether or not anything holds it. The hold is the caller's.
- `open`'s inline root filter (`:493-506`) does not reuse `check_cgroup_root`, and this story
  does not make it. It only moves ahead of the table lookup, since the key needs the canonical
  root. The cgroup-free tests open drivers over a temporary directory shaped as a cgroup, which
  holds no process of the test (`:4356-4379`); `check_cgroup_root` refuses such a root. A caller
  that holds its root has already run `check_cgroup_root` through `hold_cgroup_root`.

**Why not one cgroup root per run.** Runbook's preferred form fails in Substrate, not in Loom.
Substrate's probe proves `limits` only for a root that holds the calling process's own cgroup
(Substrate `crates/substrate-host/src/probe.rs:931-945` at rev `adfde402`, the rev
`crates/loom-intake-slice/Cargo.toml:30-31` pins). One process sits in one leaf. Sibling roots
created per run therefore hold no process, and every run under them would be refused
`CgroupUndelegated` (`confinement.rs:3453-3457`). Changing that is a change to Substrate. The
table is also the smaller change: it touches one file and no type, refusal or ESS member.

**What a library caller does.**

1. Run the process in a delegated cgroup v2 subtree with the process in a leaf below the root.
   `prepare_delegated_cgroup` (`:3635-3647`) does this inside a scope that
   `systemd-run --user --scope -p Delegate=yes` started.
2. Hold the root once with `hold_cgroup_root`, and give every run the held root,
   `hold.root()`, as `cgroup_root` in `EngineConfinement`. Keep the hold until the last run
   ends. A runner given no root discovers an ancestor (`discover_cgroup_root`, `:3620-3634`),
   which another process may use too; `tree run` refuses to run without a root for that reason
   (`crates/loom-cli/src/tree.rs:154-160`), and the guide no longer offers discovery.
3. Give each run its own workspace directory, and make all these directories children of one
   shared directory, for example `<state>/workspaces/<run>`.
4. Open one `EngineRunner` per run, or share one. Both work.

A launch that needs a driver the table does not hold, while another launch runs under the same
root, is still refused `CapabilityUnserved` with today's text (`:518-524`). Two cases trigger
this: a workspace in another directory, and an egress set or certificate bundle that no running
driver has. The guide says so. Bound programs name no egress (`bound_program_child`,
`crates/loom-intake-slice/src/bindings.rs:414-448`), so for them the directory rule is the only
one.

**Statics that stay.** These are the statics of the Loom crates `b10x-loom-tree` links, as
`git grep` finds them at `6af4ed1d` (§ Acceptance, the last test). None lets one run change
another run's result:

- `RUNNING` (`confinement.rs:381`) counts the launches per root. It only refuses a driver open
  that would end a running launch. It never ends a launch.
- The new driver table. It shares a driver only between launches with an equal key. A shared
  driver runs at most 16 executions at once and tracks 128 (Substrate `HostConfig::minimum`,
  `crates/substrate-host/src/lib.rs:529-530`). A direct `tree run` has one launch in flight.
- Counters that make names unique: `NEXT_EXEC` (`confinement.rs:70`); `ISSUED`, one per
  `fresh_uuid`, in `crates/loom-bindings/src/program.rs:959`, `crates/loom-bindings/src/open.rs:
  889` and `crates/loom-bindings/src/copy.rs:578`, and in
  `crates/loom-intake-slice/src/executor.rs:1666`; `NEXT_TEMPORARY`
  (`crates/loom-governor/src/locked_dir.rs:35`, `crates/loom-intake-slice/src/state_dir.rs:35`);
  `NEXT_SCOPE` (`crates/loom-intake-slice/src/results.rs:21`); and `MADE`
  (`crates/loom-executor/src/lib.rs:306`). No run reads another run's value.
- `ENGINEERING` (`crates/loom-governor/src/lib.rs:2014`), the compiled-in catalog, and the
  regular expressions of `crates/loom-intake-references/src/lib.rs:96-124`. All are immutable.
- `RECORDINGS` (`crates/loom-intake-slice/src/git.rs:129`). It is keyed by the canonical
  workspace and replaced only for the same workspace (`:194-200`). It serves the intake slice's
  host git, not bound programs.
- `OPEN` and `TURNING` (`crates/loom-executor/src/metering.rs:346-357`). These are thread-local,
  and each run has its own thread.
- `OFFSET` (`crates/loom-intake-slice/src/clock.rs:100`), the virtual clock offset of the
  feature `test-clock`. Only `[dev-dependencies]` enable that feature
  (`crates/loom-intake-slice/Cargo.toml:42`, `crates/loom-cli/Cargo.toml:47`,
  `crates/loom-sdk/Cargo.toml:35`), so no build a host links has it. In a test build two runs
  share the offset; it only moves forward, so no run's clock goes back.

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

Two kinds of test, named in each line below:

- **Delegated** (five new tests: the three two-tree tests, the root test and the hold test).
  They need Linux, bubblewrap, a user systemd manager and a delegated cgroup v2. They use the
  harness `delegated` of `crates/loom-intake-slice/tests/support/mod.rs:37-126`, which starts the
  test binary again under `systemd-run --user --scope -p Delegate=yes` and runs the case there.
  Off Linux they are `#[ignore]`. Without a prerequisite they print
  `skipped: missing prerequisite: …` and return, but only when `LOOM_REQUIRE_DELEGATION` is
  unset: this story makes the harness fail the test instead when it is set, as
  `crates/loom-cli/tests/tree_run/delegated_cgroup.rs:22-39` does. The unit's gate sets it and
  runs them under `systemd-run --user -p Delegate=yes --scope`. The unit's report lists the
  tests run and the tests skipped, and a skip counts as red there.
- **Cgroup-free** (the five `driver_table_*` tests, the static test and the tests kept
  unchanged below, except the two that use the harness). They run everywhere, with no cgroup.

Every test runs with no model call and no network, and ends within 60 s. The `slow_` test runs
in the `slow` lane (`.config/nextest.toml:9-11`) and ends within 20 s. The default test step
stays within 120 s. A build or test job in the slot pool ends within 10 minutes. Fixture programs
are POSIX sh, never Python. The new tests go into existing test files, which become modules of
`tests/main.rs` under `story:one-test-binary-per-crate`. No test target is added.

- Delegated. Two trees run at once. Each has its own store, its own workspace (sibling
  directories) and its own `EngineRunner` on `linux`. The process holds the root once
  (`hold_cgroup_root`), and both `EngineConfinement`s name `hold.root()`. Each tree binds one
  program that sleeps 5 s and exits 0. They start from two plain threads 1 s apart. Both steps
  record exit 0, and the two runs together take less than 9 s of wall time. Test:
  `slow_two_trees_run_confined_programs_at_once_in_one_process`.
- Delegated. In the same setup, the program of tree A reads tree B's marker file by its host
  path and as `../<B's workspace>/marker`, and both reads fail. Tree B's program, run the same
  way, does not find A's marker either. Test:
  `a_confined_program_sees_no_workspace_of_the_other_tree`.
- Delegated. While tree A's program sleeps under a unique argv marker, tree B's program reads
  every `/proc/<pid>/cmdline` it can see, and none holds the marker. Test:
  `a_confined_program_sees_no_process_of_the_other_tree`.
- Delegated. A process holds its delegated root once. A second `hold_cgroup_root` of the same
  root in the same process is refused `CgroupUndelegated`, naming the root. Meanwhile a
  `SubstrateRunner` on `hold.root()` runs a child to exit 0, so the runner takes no hold. Once
  the first hold is dropped, a new hold of the root succeeds. Test:
  `a_second_hold_of_a_root_in_this_process_is_refused`.
- Delegated. A sibling cgroup of the delegated scope (one that does not hold the process), given
  as `cgroup_root`, is refused `CgroupUndelegated`. This pins why per-run roots are not the form.
  Test: `a_root_that_does_not_hold_this_process_is_refused_cgroup_undelegated`.
- Cgroup-free. Two runners asking for one root, one directory and `DriverEgress::NONE` get the
  same driver (`Arc::ptr_eq`), and the fake opener is called once. Test:
  `driver_table_two_runners_share_one_driver_for_one_key`.
- Cgroup-free. While a launch is counted under root A (`Running::enter`), a driver for root B
  opens. Test: `driver_table_a_launch_under_one_root_does_not_refuse_another_root`.
- Cgroup-free. While a launch is counted under a root, a second runner that needs a driver for
  another directory under that root is refused `CapabilityUnserved` with today's detail. The
  opener is not called. Test: `driver_table_another_directory_under_a_running_root_is_refused`.
- Cgroup-free. Two launches under one root with unequal egress are refused in the same way.
  Test: `driver_table_another_egress_under_a_running_root_is_refused`.
- Cgroup-free. When the last runner that used a driver is dropped, the table no longer holds the
  driver, and the next runner's request calls the opener again. Test:
  `driver_table_a_driver_dropped_by_its_last_runner_leaves_the_table`.
- Cgroup-free. A root that is not delegated is still refused `CgroupUndelegated`, and nothing
  runs. `an_undelegated_root_is_refused_and_nothing_falls_back`
  (`crates/loom-intake-slice/tests/engine_confinement.rs:911-933`) is unchanged and green.
- Cgroup-free. `no_driver_is_opened_over_a_launch_running_under_its_cgroup_root`
  (`confinement.rs:4356-4379`) keeps its refusal. Its `Arc::ptr_eq` line over the per-runner map
  now checks two clones that share the table entry.
- Delegated, unchanged and green: `building_a_linux_runner_ends_no_run`
  (`engine_confinement.rs:869-885`, through the harness) and
  `adv_a_second_child_launch_leaves_a_running_child_running`
  (`adversary_w20261020w2_u4.rs:110-173`, through that file's own harness, `:30-99`, which this
  story does not change).
- Cgroup-free. No static outside the list in § Outcome exists in the sources of
  `loom-intake-slice`, `loom-bindings`, `loom-governor`, `loom-commission`, `loom-executor`,
  `loom-protocols`, `loom-runlog`, `loom-tree` and `loom-intake-references`. The test reads every
  `.rs` file under their `src/`, test modules and feature-gated code included, and fails on a
  `static`, `thread_local!`, `OnceLock` or `LazyLock` whose file and name are not in the list.
  At `6af4ed1d` it finds exactly the list. Test:
  `every_static_of_the_runtime_crates_is_listed_harmless`.
- The guide and AGENTS.md state the caller's four steps. Command:
  `git grep -c 'one shared directory' -- website/docs/guides/embed-the-runtime.md AGENTS.md`
  prints a count of at least 1 for each file.
- `slot task check` exits 0, and `slot task drift` exits 0 on every commit of the unit.

## Out of scope

Any change to Substrate, including a driver that opens without ending other drivers' runs, and
per-run cgroup roots. Changes to Canon. Changes to the CLI's default (`--confinement`, the
delegated re-exec) and to `hold_cgroup_root`, `check_cgroup_root` and their texts. Running cases
of one tree at once, which is `story:concurrent-cases-in-the-tree-daemon`. Two runs that share
one workspace. Children that name different egress sets under one root at the same time; they
stay refused. The fixed text `UNCONFINED_FLAG_REASON` (`bindings.rs:79-80`) stays; Runbook
decided no story for it. `spawn_child` (`confinement.rs:634-675`) keeps its behaviour and shares
the table like every launch.

## Scope

Derived 2026-11-22 from the plan tree at `1d6757d1` (`wave/2026-11-21-w7`); every cite refreshed
and read again at `6af4ed1d`. **Cited** means read at the `file:line` named. **Inferred** means a
reading that could be wrong.

- **Primary surface:** `crates/loom-intake-slice/src/confinement.rs`: `SubstrateRunner`,
  `Drivers` (`:243-258`), `SubstrateRunner::new` (`:421-427`), `driver` (`:442-483`), `open`
  (`:485-575`; the root's resolution `:491-512` moves ahead of the lookup, since the key needs
  the canonical root), and `RUNNING` (`:380-419`, unchanged). `CgroupRootHold`,
  `hold_cgroup_root` and `check_cgroup_root` (`:3653-3779`) are read, not changed — cited
- **Files (tests, cgroup-free):** the `tests` module of `confinement.rs` (beside `:4356-4379`):
  the `driver_table_*` tests and `every_static_of_the_runtime_crates_is_listed_harmless` —
  cited for the module, inferred for the placement of the static test
- **Files (tests, delegated):** `crates/loom-intake-slice/tests/engine_confinement.rs`
  (`linux_confinement` `:856-862`, the `linux` section `:852-934`): the root and hold tests —
  cited. `crates/loom-intake-slice/tests/support/mod.rs` (`delegated` `:37-126` hands one
  `SubstrateRunner` and one workspace, and gains the `LOOM_REQUIRE_DELEGATION` check at every
  skip; a second workspace in the same directory is made by the case, as
  `adversary_w20261020w2_u4.rs:119-120` does) — cited
- **Files (tests, two trees):** `crates/loom-bindings/tests/bound_program.rs`: two governors,
  stores and workspaces, each `ProgramEffects` (`program.rs:134-161`) with its own `EngineRunner`
  on `linux`, reached by `#[path]` to the intake slice's `support` module — inferred (no
  bindings test uses the delegated harness today)
- **Files (docs):** `website/docs/guides/embed-the-runtime.md:68-69` (the cgroup root paragraph)
  gains the four steps and the refusal that stays, and drops "or allows discovery of the current
  delegated root"; `AGENTS.md:2678-2686` (§ Intake and the command line, "A runner and its clones
  share one driver…") becomes "every runner of the process"; `CHANGELOG.md` gets one `Changed`
  bullet under `[Unreleased]` (`:13`, `### Changed` `:59`) — cited
- **Not changed:** `ess/intake/domains/confinement.yaml`, `generated/rust/intake`,
  `crates/loom-intake-slice/src/confinement/engine.rs` (`EngineRunner::open`, `:89-119`, builds
  `SubstrateRunner::new` as before), `crates/loom-intake-slice/src/bindings.rs`,
  `crates/loom-cli/src/tree.rs` — cited
- **Edges:**
  - depends_on `story:child-record-in-the-run-log` (merged at `1a1b0461`) and
    `story:tree-status-names-what-it-read` (merged at `6af4ed1d`). Kept. Neither shares a
    surface of `confinement.rs` with this story; both wrote `AGENTS.md` and `CHANGELOG.md`
    `[Unreleased]`. The edges are met at `6af4ed1d`, the head this story's unit starts from, so
    they order only those two files and block nothing.
  - No edge to `story:tree-run-takes-a-delegated-cgroup`: merged at `ec1a82f9`, which
    `6af4ed1d` holds. This story builds on its `hold_cgroup_root` and changes none of it.
- **Would collide with:**
  - `story:cancel-a-running-bound-program` on `confinement.rs`. It depends on this story and
    comes after it — cited from its relations.
  - `story:tree-library-exports-what-an-engine-needs` on
    `website/docs/guides/embed-the-runtime.md:52-69`. This story edits `:68-69`, in the same
    paragraph block. The order follows the depends_on chain: that story depends on
    `story:cancel-a-running-bound-program`, which depends on this story, so this story merges
    first and the later units merge its head — cited from both stories' relations and Scope.
  - `story:next-decision-names-the-principal` on `CHANGELOG.md` `[Unreleased]` (its `Added`
    bullet beside this story's `Changed` bullet). No edge: neither reads the other's code.
    Whichever unit merges second merges the earlier head and adds its bullet to the section as
    merged — cited from its Scope.
  - `story:concurrent-cases-in-the-tree-daemon` on nothing named here (`crates/loom-tree`,
    `crates/loom-cli`) — cited from its Scope.
  - `story:per-run-program-path` on `AGENTS.md` and `CHANGELOG.md` only, in other sections —
    inferred.
