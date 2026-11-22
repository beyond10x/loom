---
format: aep.planning-md/3
id: story:tree-run-binary-fits-its-start-bound
kind: story
status: active
title: The tree_run test binary fits its start bound of 66 again, and its shared trees are built once and safe against a killed builder
summary: 'budget defect: 94 starts against 66 (SR 94, SG 91); fixture builds counted as test starts (23 in one test); ended_tree half-built after a kill at 20 s gives StoreNotEmpty; build once (setup script), atomic rename, fixture: lines fail the step; wave 15 starts in process'
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:tree-run-tests-fit-the-test-budget
- depends_on: story:run-cgroup-root-holds-the-root
- depends_on: story:concurrent-confined-runs
scope:
- confidence: cited
  path: .config/nextest.toml
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/tests/tree_run.rs
- confidence: cited
  path: crates/loom-cli/tests/tree_run/stop.rs
- confidence: cited
  path: crates/loom-xtask/src/main.rs
- confidence: inferred
  path: crates/loom-xtask/src/shared_trees.rs
- confidence: cited
  path: crates/loom-xtask/src/test_step.rs
revision: 15
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T20:59:39Z", actor: "agent:sdc", revision: 14, executor: "agent:loom", correlation: "runbook-wave16-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T20:59:39Z", actor: "agent:sdc", revision: 15, executor: "agent:loom", correlation: "runbook-wave16-2026-11-22"}
---
## Outcome

The `tree_run` test binary is back at or under its start bound of 66, and the existing guard
holds that count. The shared trees are built once, outside the timed window, and kept until
Loom's sources change; a builder killed half-way leaves nothing a later test can trip on. No
`tree_run` test takes over 5 s on an idle host. The wave 15 tests of `stop.rs` and
`signal_exit.rs` start `b10x-loom` only where the binary is what the test is about. Cites are at
`3cd1de48`; the files cited are unchanged since `afa46eea`.

**Measured** (cited from the unit reports, not re-run here):

- The binary starts `b10x-loom` 94 times. Unit SR measured 94 and unit SG measured 91. 90 starts
  were there before wave 15 added the hold, reopen and signal-exit tests. The bound is 66, so
  the test step fails its own bound.
- `a_status_counts_the_attempts_of_a_retried_writer` alone made 23 starts. The test itself
  starts nothing (`crates/loom-cli/tests/tree_run.rs:1381-1410`), so the 23 are the build of the
  shared `ended_tree` (`:1229-1271`), made by the first test that asked for it.
- Cold, under load (about 120), building `ended_tree` takes over 20 s. 20 s is the `tree_run`
  limit in `.config/nextest.toml:10-12`. A builder killed there left a half-built store, and the
  next test was refused `StoreNotEmpty`. With the tree already built, 10 of 10 passed in 3.3 s
  (unit SR).
- The `tree_run` tests sum to about 270 s on 4 threads against their 60 s
  (`BINARY_BUDGETS`, `crates/loom-xtask/src/test_step.rs:141-143`). Unit SG measured that on a
  loaded host; it is not a test-step measurement (inferred).

**What `story:tree-run-tests-fit-the-test-budget` already did, and what is left.**

- **Done:**
  - library calls run in process;
  - every start is counted through `with_the_switch` (`tree_run.rs:79-115`);
  - the bound is 66 (`START_BUDGETS`, `test_step.rs:155-161`);
  - the test step runs the ignored `the_shared_trees_are_made` (`tree_run.rs:2017-2028`) with
    `cargo test --test tree_run` after its build and before its timed run
    (`test_step.rs:284-312`, `SHARED_TREES` `:510`).
- **Left 1: fixture builds are counted as test starts.** When the timed run builds a shared tree
  itself, every start of that build is counted under the test that asked first. A plain
  `cargo nextest run` never runs the pre-step.
  - Under the test step, too, the 23 starts mean the pre-step's tree was not the one read.
  - Inferred cause: the key `ended_digest` (`:1180-1217`) hashes the bytes of the `b10x-loom`
    binary (`:1211`) and of the running test executable (`current_exe()`, `:1212`). The
    workspace uses resolver 2 (`Cargo.toml:2`), so a build of another scope (`--workspace`
    against `-p b10x-loom-cli`, `test_step.rs:211-220`) can unify other features and give other
    executables. Then the keys differ. Acceptance 1 measures this.
- **Left 2: a half-built tree.** `ended_tree`, `stepped_tree` (`:1474-1535`) and
  `observed_tree` (`:1965-2015`) build at one fixed path under a lock file:
  - a tree whose `ended.json`, `stepped.json` or `seen.json` is absent is rebuilt there, after
    `let _ = fs::remove_dir_all(&root)` (`:1236`, `:1487`, `:1977`), whose failure is ignored;
  - a killed builder's `b10x-loom` child can still be writing into `root/store` while the next
    builder removes it and opens it again (inferred mechanism of the `StoreNotEmpty` refusal).
- **Left 3: the wave 15 starts.** `stop.rs` starts the binary four times:
  - `:162`, `hold_started`, read for the printed hold;
  - `:177`, `run_started`, the printed `TreeStopped` refusal;
  - `:191`, `reopen_started`, the printed lift;
  - `:279`, `hold_started` on an absent store. Its check (the store is never read nor made)
    holds in process too, through `hold_in` (`:19-40`).

  `signal_exit.rs:75-91` starts the binary under a delegated scope. The binary under Substrate is
  the subject there, so that start stays.

**After.**

- **One way to build a shared tree.** A helper `shared_fixture(name, key, build)` (inferred
  name) serves `ended_tree`, `stepped_tree` and `observed_tree`. It builds each attempt in a
  directory of its own and marks a finished attempt with a sentinel.
  - Under the lock file of `<name>-<key>`, it looks for an attempt directory
    `tree-run-<name>-<key>-<pid>-<nanos>` that holds the sentinel (`ended.json`, `stepped.json`,
    `seen.json`, each written last) and reads it.
  - When none holds it, it builds a new attempt directory with `Tree::at` and writes the
    sentinel as the build's last act.
  - A directory without the sentinel is never read, never reused and never removed by a test.
    That is the directory of a killed builder, or one a child of a killed builder still writes.
  - Why attempt directories and not one fixed path with a sentinel: a fixed path forces the next
    builder to remove a half-built tree, and that removal races a child still writing (above).
  - Why not build elsewhere and rename: the store names its workspace by its absolute path
    (`open` passes `--workspace`, `tree_run.rs:340`; `Tree::at`, `:221-245`; the copies read the
    shared workspace, `:1253`), so a tree cannot move after it is built.
  - The setup (D2) removes every attempt directory of another key before it builds, when no
    test runs (below).
- **Counting.** `shared_fixture` sets a thread-local `FIXTURE` to the tree's name for the length
  of `build`. `count_a_start` (`:86-99`, called by `with_the_switch`, `:104-115`) then writes
  `fixture:<name>` in place of the thread's name.
  - The child process `observed_tree` starts (`:1980-1990`, its test `:2036-2045`) is told by
    an environment variable, `LOOM_TREE_RUN_FIXTURE=observed` (inferred name), and counts the
    same way.
  - The step fails when its timed run counts a `fixture:` line: a test built what the setup
    should have built.
- **A key over the inputs and the sources, no executable (D3 a).** The key of the shared trees
  is SHA-256 of:
  - the files of the fixture directory (`FIXTURE`, its stubs and bindings);
  - the source text of the builders, `tests/tree_run.rs` and `tests/tree_run/*.rs`;
  - every file that cargo's dep-info of the `b10x-loom` binary lists (`<target>/debug/b10x-loom.d`).
    That is each workspace source the binary was built from, its `build.rs` files and the files
    pulled in by `include_str!`. `target/debug/loom-xtask.d` lists
    `crates/loom-protocols/build.rs` and `protocols/*/1.yaml` in this tree; it names no file
    outside the workspace;
  - `Cargo.lock`, `rust-toolchain.toml` and `.cargo/config.toml` when present;
  - the scope the tests were built in (`LOOM_TEST_SCOPE`, else `-p b10x-loom-cli`). Under
    resolver 2 (`Cargo.toml:2`) the scope decides which features are unified, so equal sources
    in another scope can give another binary.

  It reads no executable: not `current_exe()` (`:1212`), not the `b10x-loom` binary (`:1211`).
  Any key over executable bytes differs whenever the setup and the timed run were built in
  different scopes. A key over the sources changes exactly when Loom's code, the lock file, the
  toolchain or the scope change, so a tree built by older code is never read.
- **Computed once, by the setup.** `loom-xtask shared-trees` computes the key. Reading about
  6 MiB (the tracked files under `crates/` and `generated/rust/` that are not under `tests/`
  measure 6.0 MiB at `3cd1de48`) is done once per run, not per test.
  - It gives the key to every `tree_run` test as `LOOM_TREE_RUN_KEY` (inferred name) through
    nextest's environment file for setup scripts (`NEXTEST_ENV`; inferred for 0.9.143, as the
    setup-script form in D2).
  - It gives the same key to the `cargo test … the_shared_trees_are_made` it runs.
  - A test without the variable (`cargo test --test tree_run`, no setup) builds its trees under
    its own process id. No other test reads them, and their starts count as `fixture:` lines.
- **The installed home is keyed the same way (`installed_home`, `tree_run.rs:148-197`).** Its key
  becomes the protocols' bytes plus `LOOM_TREE_RUN_KEY`, and no longer the test executable's
  length and modification time (`:156-163`).
  - The install code it runs, `InstallStore` (`crates/loom-protocols/src/store.rs:14`), is in a
    crate `b10x-loom` links (`crates/loom-cli/Cargo.toml:21`), so the dep-info covers it.
  - Inferred: the dep-info lists `store.rs`; the unit checks it and names the result.
  - Without the variable the home is private to the process, as the trees are.
- **Kept across runs (D3 a).** The setup reads a sealed attempt directory of the current key and
  builds only what is missing. Before it builds, it removes every attempt directory and home of
  another key, while no test runs. A run with unchanged sources builds nothing.
- **Once per test run (D2 a).** A nextest setup script builds what is missing of the shared
  trees and the installed home, before any `tree_run` test. It is attached to
  `binary_id(b10x-loom-cli::tree_run)` in `.config/nextest.toml`.
  - The script is `loom-xtask shared-trees` (Rust, no Python). It runs
    `cargo test --locked <scope> --test tree_run -- --ignored --exact
    the_shared_trees_are_made`.
  - `<scope>` is the value of `LOOM_TEST_SCOPE`, which the step sets on its nextest run (beside
    the start files it sets there, `test_step.rs:338-355`): `--workspace`, or its `-p`
    packages. So the setup's cargo finds the step's build current and rebuilds nothing.
  - Without the variable (a plain `cargo nextest run`), `<scope>` is `-p b10x-loom-cli`.
    If the run used another scope, cargo may build again and replace the `b10x-loom` binary the
    tests start with one of other features. The key then names that scope's sources and
    features and differs from the run's, and the trees are rebuilt. That is named in the
    setup's help, and the gate never takes that path.
- **Where the time is counted.** The test step's pre-step (`test_step.rs:284-312`, printed "not
  counted in the budget", `:306-311`) calls the same subcommand with the step's scope, before
  `started` (`:318`). It builds what is missing outside the timed window, as today.
  - The setup script then runs inside the timed window. With the same key it finds every tree
    sealed and only computes the key.
  - The step prints the pre-step's time and the setup's time on lines of their own.
  - A run whose sources changed pays the build once, in the pre-step.
- **The starts.** The count is attributed per test from the starts file. Starts then move in
  process until the timed run counts at most 66, with the bound unchanged:
  - in `stop.rs`, `:279` runs through `hold_in`;
  - `:162` stays as the one printed hold, beside `:177` and `:191`, so `stop.rs` starts three
    times;
  - the rest follows the attribution, by the rule of the merged story: a start stays only where
    the binary is what the test is about.

## Decisions

- **D1. What happens to the bound.** Default **a**.
  - **a.** It stays 66. Starts move in process until the count fits. Raising a bound is not a
    fix (`~/beyond10x/AGENTS.md` § Tests stay fast).
  - **b.** Delegated tests, whose start of the binary under `systemd-run` is the subject, get a
    bound of their own. Only if the attribution shows that they alone exceed 66.
- **D2. Where the shared trees are built.** Default **a**.
  - **a.** A nextest setup script running `loom-xtask shared-trees`, as above. Every way of
    running the tests gets the trees; they are built when missing (D3). Inferred: the
    `[scripts.setup]` form, whether nextest 0.9.143 still asks for
    `experimental = ["setup-scripts"]`, and its `NEXTEST_ENV` export to the tests.
  - **b.** Only the test step's pre-step. A test that finds no tree fails, naming the test
    step, and does not build one. A plain `cargo nextest run` then fails those tests.
  - **c.** As today, the first test builds, now in an attempt directory. Its starts still count
    against that test's 20 s limit.
- **D3. Whether the shared trees outlive a test run.** Default **a**.
  - **a.** They are kept across runs under the key of the inputs and Loom's sources (above). The
    pre-step builds what is missing outside the timed window. Inside it, the setup only computes
    the key and finds every tree. A changed source changes the key, so a stale tree is never
    read.
  - **b.** No cache: the setup removes and rebuilds every shared tree on every nextest run. It
    runs inside the timed window (`started`, `test_step.rs:318`), so its time is counted in the
    step's 120 s, every run. A cold build of `ended_tree` alone took over 20 s under load
    (unit SR).

## ESS first

Exempt: no noun or rule of Loom's specification changes. This is test code and the test step.

## Acceptance

Bounds: no test over 60 s (20 s for `tree_run`), the test step within 120 s, no network, no
model call. Stubs and fixtures are POSIX sh, never Python; the setup is Rust. A slot job ends
within 10 minutes, or names its time with `slot --timeout` (at most 30 minutes). A script that
only measures runs without the `slot` prefix. Every test below names its module, its fixture and
a failing-first reason.

The step is run as `cargo run -q --locked -p loom-xtask -- test-step -p b10x-loom-cli`
(`crates/loom-xtask/src/main.rs:142-145`, `test_step.rs:219`, `:374`). `task test-step` takes no
package (`Taskfile.yml:80-85`).

1. **Attribution and cause first, before any change.** The unit runs the step once on an idle
   host, on `3cd1de48` or a later head of the branch. `3cd1de48` holds
   `run-cgroup-root-holds-the-root`, whose tests are in the crate's shared binary and start
   nothing counted in `tree_run` (see Edges).
   - It reports, per test, the lines of the starts file, split into fixture builds and test
     starts, and the attributed total against 94.
   - It reports `ended_digest` and the path of `current_exe()` as the pre-step printed them and
     as the timed run's first `ended_tree` printed them (a temporary `eprintln!`, not
     committed). The report says whether the keys differed, and why.
2. **`loom-xtask` (`crates/loom-xtask/src/test_step.rs`, its `mod tests`):**
   `a_fixture_start_in_the_timed_run_fails_the_step`.
   - `start_verdict` over a starts text holding one `fixture:ended` line fails, naming the tree.
   - Over the same count of test lines it passes.
   - Failing first: `start_verdict` has no fixture lines to tell apart today.
3. **`tree_run` (`crates/loom-cli/tests/tree_run.rs`):** `a_half_built_attempt_is_never_read`.
   - Fixture: a small builder that writes one file and its sentinel, no tree and no start. Beside
     it, an attempt directory of the same key without a sentinel, holding a file, as a killed
     builder leaves it.
   - `shared_fixture` returns a new directory that holds the sentinel; the half-built one and
     its file are untouched.
   - While the builder runs, no directory of the key holds the sentinel. A second thread asking
     for the same key waits, then gets the same directory, and its builder is never called.
   - Failing first: there is no `shared_fixture`. Today's fixtures remove a half-built tree and
     build at the same path (`:1236`).
4. **`tree_run`:** `a_start_in_a_fixture_build_is_counted_as_the_fixture`.
   - The test sets `LOOM_TREE_RUN_STARTS` to a scratch file of its own, before it starts a
     thread. Nextest runs each test in its own process, so the step's own file is not touched.
   - A small builder starts `b10x-loom --version` once through `Tree::command`.
   - The file holds `fixture:tiny`. A start after the build holds the test's name.
   - Failing first: today every start writes the thread's name.
5. **The count.** On the head of acceptance 1 plus this change, the step counts at most 66
   starts and no `fixture:` line, with `START_BUDGETS` unchanged at 66. Failing first: 94 or
   more.
6. **The setup (D2 a, D3 a), a command check in the unit's report.**
   - Remove the trees: every `tree-run-*` under `target/tmp`, which is `CARGO_TARGET_TMPDIR`.
   - Run `LOOM_TREE_RUN_STARTS=<file> cargo nextest run -p b10x-loom-cli --test tree_run`.
     - It shows one run of the setup script, which builds the trees.
     - The file holds no `fixture:` line, and no test runs over 20 s.
   - Run it again unchanged. The setup builds no tree.
   - The same with the step's scope: run `LOOM_TEST_SCOPE=--workspace
     LOOM_TREE_RUN_STARTS=<file> cargo nextest run --workspace -E
     'binary_id(b10x-loom-cli::tree_run)'`.
     - It shows one setup run that compiled nothing (no `Compiling` line from the setup).
     - The file holds no `fixture:` line: the timed tests found the trees the setup built for
       that scope's key.
   - Failing first: today no setup exists, and the first test that asks builds the tree.
7. **`loom-xtask` (`crates/loom-xtask/src/`, the module of `shared-trees`, its `mod tests`):**
   `the_key_ignores_the_executables`.
   - Fixture: a scratch directory under `target/tmp` holding:
     - a fixture directory of two files;
     - two builder sources;
     - two Loom sources;
     - a `Cargo.lock`;
     - a fake `debug/b10x-loom` and its `debug/b10x-loom.d`, which lists the two Loom sources
       in cargo's dep-info form.

     No cargo run, no start.
   - The tree key of two copies at two paths is equal. It changes when one byte changes in any
     of these:
     - a fixture file;
     - a builder source;
     - a Loom source the dep-info lists;
     - `Cargo.lock`;
     - the scope (`--workspace` against `-p b10x-loom-cli`).
   - It does not change when the fake executable's bytes or modification time change.
   - The home key (`installed_home`'s, as a function of the protocols' bytes and the tree key)
     changes with a protocol file and with the tree key, and not with the executable.
   - A scan of `installed_home` and the key functions finds no `current_exe` and no
     `CARGO_BIN_EXE`.
   - Failing first: today's tree key reads both executables (`tree_run.rs:1211-1212`), and the
     home key reads the test executable's length and modification time (`:156-163`).
8. **The time.**
   - On an idle host (load under 2), the step's ten slowest list no `tree_run` test over 5 s.
     Any test over 5 s is named with its time in the report.
   - `BINARY_BUDGETS` (60 s summed) holds.
   - The same run under load is reported and not judged.
9. **Wave 15.** `stop.rs` starts the binary three times:
   - `:162`, the one printed hold;
   - `:177`, the printed refusal of `tree run` on a held tree;
   - `:191`, the output of `tree reopen`.

   `:279` runs in process. `signal_exit.rs` keeps its one delegated start.
   - `every_start_goes_through_with_the_switch` (`tree_run.rs:3722-3775`) loses one entry of
     its list, `fn ended_digest` (`:3763`), because the key no longer names the binary. Its doc
     comment changes with it, and it passes with the other two sites.
10. **The step's time with the setup apart, a command check in the unit's report (D3 a).** On
    an idle host, run the step twice, unchanged in between.
    - The report gives for each run:
      - the step's wall time;
      - the pre-step's time, from its "not counted" line;
      - the setup script's time, from nextest's setup line;
      - the timed window, `nextest_seconds` (`test_step.rs:362`).
    - In the second run the pre-step and the setup build no tree, and the step ends within
      120 s.
    - Failing first: today there is no setup line, and the first test that asks builds the tree
      in the timed window.
11. **A changed Loom source invalidates the trees (D3 a), a command check in the unit's
    report.**
    - Run the step once. Then add one comment line to a source under `crates/` that
      `b10x-loom`'s dep-info lists (`crates/loom-tree/src/run.rs`), and run it again.
    - The second run's pre-step prints another key and builds the three trees and the home
      again. The attempt directories of the first key are removed before the build. Its setup
      prints the same new key and builds nothing.
    - The timed run holds no `fixture:` line.
    - The comment is reverted before the report.
    - Failing first: the setup and its key line do not exist today.

**Gate.** `task check` exits 0.

## Out of scope

- `crates/loom-xtask/tests/crd.rs`, the test
  `slow_crd_drift_names_a_changed_or_missing_manifest_and_crd_generate_restores_it` (`:69-71`).
  It is a `slow_` test already, excluded from the test step by the default filter
  (`.config/nextest.toml:2`). It runs in `task slow`, whose profile lets one test run 20 x 60 s
  (`:16`), so 146 s is within its lane. Not this story, and no story of its own.
- Splitting `tree_run` into binaries (`story:one-test-binary-per-crate`).
- Removing attempt directories while tests run.

## Scope

Derived 2026-11-22 from the plan tree at `3cd1de48`; the files cited are unchanged since
`afa46eea`. **Cited** means read at the line named; **inferred** means a reading that could be
wrong.

- `crates/loom-cli/tests/tree_run.rs` (`:38-57`, `:79-115`, `:143-197`, `:221-245`, `:340`,
  `:1172-1271`, `:1381-1410`, `:1474-1535`, `:1965-2045`, `:3722-3775`) — cited.
- `crates/loom-cli/tests/tree_run/stop.rs` (`:19-90`, `:159-200`, `:253-293`) — cited.
- `crates/loom-cli/tests/tree_run/signal_exit.rs` (`:75-91`), read and not changed — cited.
- `crates/loom-xtask/src/test_step.rs` (`:141-161`, `:211-220`, `:284-312`, `:318`,
  `:338-355`, `:362`, `:374`, `:510`) and `crates/loom-xtask/src/main.rs` (`:142-145`, the new
  `shared-trees`) — cited. The subcommand's module, `crates/loom-xtask/src/shared_trees.rs`
  (new, with the key), is inferred.
- `.config/nextest.toml` (`:1-16`, the setup script) — cited; the setup-script form is inferred.
- `Taskfile.yml` (`:80-85`), `Cargo.toml` (`:2`), `crates/loom-cli/Cargo.toml` (`:21`) and
  `crates/loom-protocols/src/store.rs` (`:14`), read — cited. Cargo's dep-info form, read in
  `target/debug/loom-xtask.d` of this tree — cited.
- `AGENTS.md` and `CHANGELOG.md` — inferred.

## Edges

- `depends_on story:tree-run-tests-fit-the-test-budget` (active, merged on this branch): the
  bound, the counting and the pre-step are its.
- `AGENTS.md` and `CHANGELOG.md` are shared with most stories. They are textual merges and are
  not listed below.
- `depends_on story:run-cgroup-root-holds-the-root` (active, merged on this branch at
  `3cd1de48`; recorded edge).
  - It put its tests in the crate's shared binary (`crates/loom-cli/tests/main.rs:4`,
    `run_holds_the_root/mod.rs:6-8`), not in `tree_run`. So its starts of `b10x-loom` do not
    count against 66, and it left `tree_run.rs` unchanged (`afa46eea` to `3cd1de48`).
  - The edge stays for the files its scope names and this story edits (`tree_run.rs`), and so
    that acceptance 1 and 5 count on a head that holds it, `3cd1de48` or later.
- `depends_on story:concurrent-confined-runs` (active, not merged; recorded edge). It changes
  `crates/loom-intake-slice/src/confinement.rs`, which `b10x-loom` links. So it changes the
  binary the delegated `tree_run` tests start, and with D3 a the key of the shared trees. This
  story's times (acceptance 8 and 10) are measured on a head that holds it.
- **Would collide with**, no edge. Each entry names the shared file and the merge order:
  - `suite-runs-cycles-under-one-cgroup-root` (active) edits the `#[path]` list of
    `tree_run.rs` (`:43-57` at `afa46eea`). Textual merge, no order.
  - `program-reports-its-cost` (active) adds `tree_run` tests that start the binary. Textual
    merge, no order. Whichever lands first, the starts are counted on the merged head: by this
    story's acceptance 5 if it lands first, by the guard at its own merge if this story does.
  - `every-printed-decision-command-runs` and `person-action-records-a-named-result` (active,
    merged on this branch at `5af3f9c2`): their `tree_run` tests are in the count of acceptance
    1 and 5. No order.
  - **Other stories that add `tree_run` tests that start `b10x-loom`.** Each start counts
    against 66. They merge after this story, and each fits the bound or moves starts in process:
    - proposed: `per-run-program-path` and `tree-waits-open-decision-requests`;
    - draft: `mantle-placement`.
  - `crates/loom-cli/tests/tree_run.rs`, `tree_run/stop.rs`: `one-test-binary-per-crate`
    (active) turns them into modules of `tests/main.rs`. Textual merge, no order. The start
    counting keys on the test's thread name, which survives the move. The setup's `binary_id`
    filter follows the move.
  - `crates/loom-xtask/src/test_step.rs`, `crates/loom-xtask/src/main.rs` and
    `.config/nextest.toml`: `test-step-within-budget` and `one-test-binary-per-crate` (active).
    Textual merge, no order.
  - `crates/loom-xtask/src/main.rs`: `local-gate-equals-forge` (active, not merged) adds the
    subcommands `ForgeParity` and `ForgeWatch`; this story adds `SharedTrees` beside
    `TestStep` (`:142-145`). Textual merge, no order.
  - `readme-logo-and-badges` (proposed) names `crates/loom-cli/tests` as a whole. Textual
    merge, no order.
