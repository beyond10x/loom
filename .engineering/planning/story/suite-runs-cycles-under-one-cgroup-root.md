---
format: aep.planning-md/3
id: story:suite-runs-cycles-under-one-cgroup-root
kind: story
status: active
title: tree test runs cycles side by side under Substrate
summary: tree test --confinement substrate runs with --cgroup-root or a delegated scope started again, through embed::TreeRunner's runner (no SuiteSettings field); all cycle workspaces children of one directory (<scratch>/work/<n>, <scratch>/store/<n>) so one driver serves them; workers 2..16 under substrate, above 16 SubstrateWorkersUnserved; no cross-cycle workspace or process visibility
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:suite-runs-cycles-in-parallel
- depends_on: story:concurrent-confined-runs
- depends_on: story:tree-library-exports-what-an-engine-needs
- depends_on: story:person-action-runs-a-program
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/tree.rs
- confidence: cited
  path: crates/loom-cli/tests/common
- confidence: cited
  path: crates/loom-cli/tests/fixtures/tree-suite
- confidence: cited
  path: crates/loom-cli/tests/main.rs
- confidence: cited
  path: crates/loom-cli/tests/tree_run.rs
- confidence: cited
  path: crates/loom-cli/tests/tree_run/delegated_cgroup.rs
- confidence: cited
  path: crates/loom-cli/tests/tree_test/mod.rs
- confidence: cited
  path: crates/loom-tree/src/suite/mod.rs
- confidence: cited
  path: crates/loom-tree/src/suite/run.rs
- confidence: cited
  path: crates/loom-tree/src/suite/select.rs
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: cited
  path: generated/ess/loom
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/run-a-case-tree.md
- confidence: cited
  path: website/docs/reference/cli.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T12:27:10Z", actor: "agent:loom", revision: 5}
- {from: "proposed", to: "active", at: "2026-11-22T12:30:34Z", actor: "agent:sdc", revision: 6, executor: "agent:loom", correlation: "runbook-suite-stories-2026-11-22"}
---
## Outcome

`b10x-loom tree test --confinement substrate` runs, with any worker count from 1 to 16. Every
bound program of every cycle runs confined by Substrate under one held, delegated cgroup root,
and all the cycles share one driver.

```
b10x-loom tree test --suite <file> --protocols <dir> --scratch <dir> --at <instant>
                    [--confinement substrate|local] [--cgroup-root <dir>]
                    [--workers <n>] [--only <name>[,<name>...]]... [--report <file>]
```

Security-relevant: confined processes of several cycles run at the same time under one cgroup
root. The command can start itself again in a delegated systemd scope. A cycle must see neither
another cycle's workspace nor its processes.

### Today, and after `story:suite-runs-cycles-in-parallel`

- `tree test` opens its runner with `tree_runner(arguments.confinement, None)`
  (`crates/loom-cli/src/tree.rs:460`). Under `substrate`, `substrate_runner` refuses with
  `CgroupUndelegated: tree run holds no cgroup root` (`tree.rs:157-163`), exit 2.
- `story:suite-runs-cycles-in-parallel` refuses `--workers` above 1 under `substrate` with
  `SubstrateWorkersUnserved`.
- Substrate keys its driver by the workspace's parent directory
  (`crates/loom-intake-slice/src/confinement.rs:450-467`). Opening a second driver under a root
  while a launch runs there is refused `CapabilityUnserved` (`:485-525`). Cycle `n`'s workspace
  `<scratch>/<n>/work` has a parent of its own, so two cycles at once would open two drivers.
- With `story:concurrent-confined-runs`, a process holds a root once and every run shares that
  hold. The table it adds shares one driver between launches with an equal key, and a shared
  driver runs at most 16 executions at once (`concurrent-confined-runs.md:110-112`).
- Runbook refuses `substrate` itself, because Loom's runner takes no root
  (`runbook/src/test_suite.rs:84-89` at `impl/runbook-test-suite`).

### The layout

- Every cycle's workspace is a child of one shared directory, and so is every cycle's store:
  cycle `n` works in `<scratch>/work/<n>/` and keeps its tree in `<scratch>/store/<n>/`. `n` is
  still the cycle's position in the suite.
- All workspaces then have one parent, so every launch of every cycle has one driver key, and
  the cycles share one driver.
- The layout is one for every confinement (D1). `story:tree-suite-format`'s promise "Cycle `n`
  works in `<scratch>/<n>/` (`store/`, `work/`)" (`tree-suite-format.md:229`) is replaced by the
  new layout. These change with it:
  - `TREE_TEST_HELP` (`crates/loom-cli/src/lib.rs:296-307`);
  - the module docs (`crates/loom-tree/src/suite/run.rs:7`, `suite/mod.rs:9`,
    `suite/select.rs:2`, `:41-43`);
  - the guide;
  - every test that reads or asserts `<scratch>/<n>/` when this story merges, after the parallel
    and write stories:
    - `crates/loom-cli/tests/tree_test/mod.rs:318`, `:372` and `:390`;
    - `story:suite-runs-cycles-in-parallel`'s tests 1, 2 and 5-7 (`barrier.txt`, `own.txt`,
      `<scratch>/3`, `<scratch>/2/` and `/4/`);
    - `story:suite-cycle-writes-a-workspace-file`'s test 3 (the cycle directory's `x`);
    - any other found then by `git grep -nE '<scratch>/[0-9n]|scratch\.join\("[0-9]'` over
      `crates/` and `website/`.
- `CHANGELOG.md` names the layout under `**Breaking:**`. The sources `doc` and `files` with
  `in: work|store` read the cycle's own directories as before, so no suite changes.
- **The cross-cycle read check.** A shared parent must not widen what a program sees.
  - The driver adopts each workspace by its name (`workspace_root_identity`,
    `confinement.rs:473-481`). That it exposes that workspace and not its parent is inferred, and
    tests 3 and 4 pin it.
  - A program of cycle 1 that reads cycle 2's marker, by its host path or as `../2/marker`,
    fails.
  - `story:concurrent-confined-runs` states the same for two trees whose workspaces are sibling
    directories, in its test `a_confined_program_sees_no_workspace_of_the_other_tree`.

### Workers under Substrate

- `--workers` from 2 to 16 runs under `substrate`.
- Above 16, the run is refused before any cycle with `SubstrateWorkersUnserved`, exit 2. The
  detail says: `a Substrate driver runs at most 16 executions at once; --workers above 16 under
  --confinement substrate is refused`. A cycle has one program in flight at a time, so 16
  workers never ask a driver for more than 16.
- The check of `story:suite-runs-cycles-in-parallel` moves from "above 1" to "above 16", in
  `tree test` before the runner and in run_suite.

### The root and the runner

- Nothing is added to `SuiteSettings`. The caller builds the runner through `embed::TreeRunner`
  of `story:tree-library-exports-what-an-engine-needs`:
  - `on_root(root)` for a given root;
  - `in_own_scope()` in a delegated scope.

  It then passes `TreeRunner::runner()` as `SuiteSettings::runner`, with `confinement:
  Some("substrate")`. That runner owns the hold (`tree-library-exports-what-an-engine-needs.md:
  125-129`), so every worker's launches run under the one hold for as long as the suite runs.
- No raw `CgroupRootHold`, `hold_cgroup_root` or `prepare_delegated_cgroup` is re-exported.
- **CLI.** `test` calls `place`, generalised by `story:person-action-runs-a-program` to take the
  confinement and the root instead of `TreeRunArgs` (`person-action-runs-a-program.md:349-350`):
  - `--cgroup-root <dir>` is resolved and held through `TreeRunner::on_root`.
  - Without it, under `substrate`, the command starts itself again in a delegated scope as `tree
    run` does (`start_again`, `tree.rs:210-243`). The process started again builds
    `TreeRunner::in_own_scope()`. The first process exits with the second one's status: 0, 1 or
    2.
  - `--confinement local --cgroup-root <dir>` is refused `ConfinementRefused`, as for `tree run`
    (`tree.rs:186-193`).
  - A refused hold, a failed start-again or a refused handshake ends `tree test` with exit 2
    (`TREE_TEST_REFUSED`), not `tree run`'s 1 (`lib.rs:497`).
  - Each is printed after the prefix `b10x-loom: tree test refused:` (`tree.rs:446`). The
    start-again text says `tree test could not start itself again`.
- `TreeTestArgs` (`lib.rs:315-338`) gains `--cgroup-root`, with `tree run`'s help
  (`lib.rs:485-489`). The `--confinement` help (`lib.rs:331-334`) no longer says substrate is
  refused.

## Decisions

- **D1, the layout.**
  - A (default): one layout, `<scratch>/work/<n>/` and `<scratch>/store/<n>/`, for every
    confinement and worker count. A reader always looks in one place.
  - B: the new layout only under `substrate`, keeping `<scratch>/<n>/` under `local`. Two layouts
    to document and test.

## ESS first

- `loom.tree.SuiteRefusalReason`'s comment (`ess/domains/tree.yaml:1034-1067`) says that
  `SubstrateWorkersUnserved` covers more than 16 workers under `substrate`.
- No new type or variant.

## Acceptance

Bounds:

- No test runs longer than 60 s, and the test step of the gate ends within 120 s.
- No test makes a network request or a model call.
- Bound programs and fixtures are POSIX sh, never Python.
- A build or test job in the slot pool ends within 10 minutes, or names its time with
  `slot --timeout` (at most 30 minutes).

Delegated tests:

- They skip only when `LOOM_REQUIRE_DELEGATION` is unset, printing `skipped: missing
  prerequisite: …`.
- The gate sets `LOOM_REQUIRE_DELEGATION` and runs them under `systemd-run --user -p
  Delegate=yes --scope`.
- The implementation report lists the tests run and skipped. A skip counts as red.

Where the tests go:

- Every test of this story is in `crates/loom-cli/tests/tree_test/mod.rs`, with suites and POSIX
  sh stubs below `crates/loom-cli/tests/fixtures/tree-suite/substrate/`.
- **The shared delegation helper.** Today the helpers that make a delegated scope live in the
  separate `tree_run` test binary (`crates/loom-cli/tests/tree_run/delegated_cgroup.rs:18-112`):
  `REQUIRE`, `SCOPE`, `DELEGATED`, `missing`, `delegation_present` and `Scope`. They move,
  unchanged, to `crates/loom-cli/tests/common/delegated_cgroup.rs`.
  - `crates/loom-cli/tests/main.rs` declares `mod common;` with `common/mod.rs`.
  - `crates/loom-cli/tests/tree_run.rs` reaches the same file through `#[path]`, as it reaches
    its own modules (`tree_run.rs:38-44`).
  - The fixtures `scope.sh` and `enter.sh` stay in `crates/loom-cli/tests/fixtures/
    tree-delegated/`.
- **The host-side gate.** A Substrate-confined program writes only its own workspace, so the
  parent story's barrier, a directory outside `--scratch`, is not used here. Instead:
  - each cycle's program writes `started` into its own workspace, then polls its own `release`
    file every 0.1 s for at most 10 s;
  - the test, which runs `tree test` as a child process, polls every `<scratch>/work/<n>/started`
    from the host;
  - once all exist, it writes `release` into each workspace.

  All the `started` files existing at once, before any `release`, is the overlap.
- **How a process gets into the scope.** As the `tree run` tests do
  (`tree_run/delegated_cgroup.rs:194-221`, `:318-330`):
  - the command is started through `enter.sh <scope>/run`, which moves it into the child `run`
    of the scope `Scope::start` holds;
  - the command is given the scope's own directory as its root.
- **In-process test 6.** run_suite has to run in a test process that sits in a child of the root.
  - The test starts its own test binary again through `enter.sh <scope>/run`, with `--exact <the
    test> --nocapture`, `LOOM_SUITE_IN_SCOPE=1` and the scope's path in `LOOM_SUITE_ROOT`.
  - The re-entered run does the work. The outer run asserts its exit status and that its output
    names the test as passed.

Tests:

1. `cycles_work_in_one_shared_directory` (`local`, not delegated). A two-cycle run leaves
   `<scratch>/work/1`, `work/2`, `store/1` and `store/2`, and nothing else under `--scratch`.
   Each `work/<n>/own.txt` holds the `n`th cycle's name, copied by its program from its seeded
   `tag.txt`.
2. `substrate_runs_cycles_side_by_side_under_a_given_root` (delegated): `--cgroup-root <scope>`,
   `--workers 2`, two gated cycles.
   - The test sees both `started` files before it writes any `release`.
   - Each `work/<n>/own.txt` holds its own cycle's name, and the exit is 0.
   - Afterwards no process is left below the root except the scope's keeper.
3. `a_cycle_sees_no_workspace_of_another` (delegated, `--workers 2`, gated).
   - Each program writes `marker` into its own workspace before `started`.
   - When the test writes `release`, it first checks from the host that both
     `work/1/marker` and `work/2/marker` exist. That is the positive control: each marker exists
     before the other cycle reads it.
   - After `release`, cycle 1's program reads cycle 2's marker by its host path (seeded) and as
     `../2/marker`, and writes `unseen` when both reads fail. Cycle 2 does the same toward cycle
     1.
   - Both write `unseen`.
4. `a_cycle_sees_no_process_of_another` (delegated, `--workers 2`, gated).
   - Cycle 1's program is bound with the argv marker `TOKEN-ONE`. After `release` it waits for a
     `done` file, which the test writes once cycle 2 has written `procs.txt`.
   - Cycle 2's program is bound with `TOKEN-TWO`. After `release` it reads every
     `/proc/<pid>/cmdline` it can see and writes `procs.txt` with two lines: `self: seen` when
     one holds `TOKEN-TWO`, and `other: seen` or `other: unseen` for `TOKEN-ONE`.
   - The test asserts `self: seen` and `other: unseen`. A program that can read no `/proc` fails
     on the first line, and the gate shows that the two ran at once.
5. `substrate_starts_itself_again` (delegated). With no `--cgroup-root`, `tree test` starts itself
   again in a delegated scope. It exits 0 for a passing suite and 1 for a failing one.
6. `run_suite_takes_the_runner_that_holds_the_root` (delegated, re-entered in `<scope>/run` as
   above).
   - `SuiteSettings::new` takes `TreeRunner::on_root(<LOOM_SUITE_ROOT>)?.runner()`,
     `confinement: Some("substrate")` and `workers: 2`. Two run_suite calls, one after the
     other, both end as expected.
   - After the `TreeRunner` and the settings are dropped, a second `TreeRunner::on_root` of the
     same root, in the same process, succeeds. So the hold went with the runner.
7. `workers_above_sixteen_under_substrate_are_refused` (not delegated). `--confinement substrate
   --workers 17` exits 2 with `SubstrateWorkersUnserved` before any cycle, and `--scratch` stays
   absent.
8. `local_with_a_cgroup_root_is_refused` (not delegated). `--confinement local --cgroup-root
   <dir>` exits 2 with `ConfinementRefused`.
9. `a_refused_hold_exits_2` (not delegated). `--cgroup-root <a plain directory of the test>`,
   which is no cgroup, exits 2. Stderr starts `b10x-loom: tree test refused:` and names
   `CgroupUndelegated` and the directory. `--scratch` stays absent.
10. `a_failed_start_again_exits_2` (not delegated). With no `--cgroup-root` and a `PATH` that
    holds no `systemd-run`, as `refused_without_systemd_run` does for `tree run`
    (`tree_run/delegated_cgroup.rs:483-493`), the command exits 2. Stderr starts `b10x-loom: tree
    test refused:` and says `tree test could not start itself again`.
11. `a_refused_handshake_exits_2` (not delegated). With `B10X_LOOM_CONFINEMENT_REEXEC=1` set
    (`REEXEC_MARKER`, `lib.rs:426`) and no `B10X_LOOM_CONFINEMENT_READY`, the command exits 2.
    Stderr starts `b10x-loom: tree test refused:` and says `the delegation handshake path is
    missing` (`tree.rs:259`).

Docs: the guide's `tree test` section (`website/docs/guides/run-a-case-tree.md:361-420`) shows
`--cgroup-root`, `substrate` with workers, and the layout.

## Out of scope

- A cgroup root per cycle, which `story:concurrent-confined-runs` rejects.
- More than 16 workers under `substrate`.
- Changes to the confinement crate (`story:concurrent-confined-runs`) and to `embed`
  (`story:tree-library-exports-what-an-engine-needs`).

## Scope

Derived 2026-11-22 from the plan tree at `5bcc26d9` (`wave/2026-11-21-w7`). **Cited** means read
at the `file:line` named. **Inferred** means a reading that could be wrong.

- **ESS and generated:** `ess/domains/tree.yaml` (`:1034-1067`, the comment),
  `generated/rust/loom`, `generated/ess/loom` — cited.
- **Suite:**
  - `crates/loom-tree/src/suite/run.rs`: the module doc (`:7`), `cycle` (`:738-751`), and the
    worker bound in `run_suite` — cited.
  - `crates/loom-tree/src/suite/mod.rs` (`:9`) and `suite/select.rs` (`:2`, `:41-43`): the docs —
    cited.
- **CLI:**
  - `crates/loom-cli/src/lib.rs`: `TREE_TEST_HELP` (`:296-307`), `TreeTestArgs` (`:315-338`) —
    cited.
  - `crates/loom-cli/src/tree.rs`: `tree_runner` (`:139-155`), `place` (`:185-206`),
    `start_again` (`:210-243`), `test` (`:444-509`) — cited.
- **Read, not changed:** `crates/loom-intake-slice/src/confinement.rs` (`:450-525`) — cited.
- **Tests:**
  - `crates/loom-cli/tests/tree_test/mod.rs` — cited.
  - `crates/loom-cli/tests/fixtures/tree-suite`, where the new `substrate/` lives — cited.
  - The helper move:
    - out of `crates/loom-cli/tests/tree_run/delegated_cgroup.rs` (`:18-112`);
    - into `crates/loom-cli/tests/common/delegated_cgroup.rs` and `common/mod.rs` (new);
    - declared in `crates/loom-cli/tests/main.rs` and `crates/loom-cli/tests/tree_run.rs`
      (`:38-44`);

    — cited.
  - `crates/loom-cli/tests/fixtures/tree-delegated` (`scope.sh`, `enter.sh`), read and not
    changed — cited.
- **Docs:** `website/docs/guides/run-a-case-tree.md` (`:361-420`), `website/docs/reference/cli.md`
  (generated), `CHANGELOG.md` `[Unreleased]` — cited.

## Edges

Each is recorded as depends_on. This story comes after all of them, and its unit merges their
heads.

- `story:suite-runs-cycles-in-parallel` (draft): the workers, and the refusal this story moves
  from 1 to 16.
- `story:concurrent-confined-runs` (active): one driver per key and the 16-execution bound.
- `story:tree-library-exports-what-an-engine-needs` (active): `embed::TreeRunner`, `on_root`,
  `in_own_scope` and `runner()`.
- `story:person-action-runs-a-program` (active): the generalised `place`. This story is later in
  the order, so it only calls `place` and does not generalise it again.

**Would collide with**, no edge:

- `story:suite-cycle-writes-a-workspace-file` (draft). Shared:
  - `suite/run.rs`, `tree_test/mod.rs`, the fixtures and the guide;
  - `ess/domains/tree.yaml`, the `SuiteRefusalReason` comment, which the parallel story also
    edits;
  - `generated/rust/loom`.

  This story merges after it and updates its test 3 to the new layout.
- `story:tree-test-command` (draft): `suite/run.rs`, `tree_test/mod.rs`, the fixtures and the
  guide. That story merges after this one and merges its head.
- `story:person-action-runs-a-program` and `story:tree-library-exports-what-an-engine-needs`
  (active, beside their edges) scope `crates/loom-cli/tests/tree_run/delegated_cgroup.rs`, from
  which this story moves the shared helpers. Both merge first, so the move is made on their head.
- `story:one-test-binary-per-crate` (active) scopes all of `crates`, and so `tests/main.rs` and
  `tree_run.rs`. The `common` module is one more module of `tests/main.rs`, its own layout.
  Whichever merges second merges the other's head.
