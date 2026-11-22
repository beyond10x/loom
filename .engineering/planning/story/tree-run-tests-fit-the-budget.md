---
format: aep.planning-md/3
id: story:tree-run-tests-fit-the-budget
kind: story
status: active
title: 'tree_run tests fit the test budget: the store syncs off only under a test switch a release binary cannot hold'
summary: 'budget defect: tree_run binary 98-145 s, tests 32-48 s, cause ~1,000 fsyncs per tree; one sync door in loom-governor, test feature + env var under debug_assertions only, durability tests keep full sync, tree_run under 60 s, no test over 20 s'
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:tree-run-command
- depends_on: story:test-step-within-budget
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T09:44:42Z", actor: "agent:loom", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-22T09:50:11Z", actor: "agent:loom", revision: 4}
---
## Outcome

The `b10x-loom-cli::tree_run` test binary fits the test budget of `~/beyond10x/AGENTS.md`
§ Tests stay fast, and a release `tree run` keeps every sync it makes today. Cites are at
`1a1b0461`.

### The door

Every `sync_all` of the five crates on the tree path goes through one new module of
`b10x-loom-governor`, `durable` (inferred name). It has two functions:

- `sync_file(&File) -> io::Result<()>` returns the sync's error, as each file sync does today.
- `sync_dir(&Path)` ignores a directory sync's error, as each directory sync does today.

These are the calls that move into it:

- `crates/loom-governor/src/locked_dir.rs`: the probe at each open (`:78`), a replace (`:131`),
  and the directory sync in `sync` (`:184-188`), called after a replace (`:141`) and a remove
  (`:180`).
- `crates/loom-governor/src/file_store.rs`: `:415`, and the directory at `:421`.
- `crates/loom-runlog/src/lib.rs`: `:295`, and the directory at `:304`.
- `crates/loom-tree/src/store.rs`: `:73`, and the directory at `:77`, in `write_file` (`:63-85`).
- `crates/loom-tree/src/lock.rs:115`, `crates/loom-bindings/src/program.rs:784` and
  `crates/loom-cli/src/stage.rs:185`.

The door counts: `static SYNCED: AtomicU64` and `static SKIPPED: AtomicU64`, read through
`durable::counts()`.

- The counters are compiled in every build, release included. They cost one atomic add beside an
  fsync.
- They must exist without the feature, because the count tests below run in a lane that does not
  have it.
- In a build without the feature, `SKIPPED` is always 0.

### The switch, D1 (a), the default

- **The feature.** `b10x-loom-governor` has a cargo feature `test-unsynced-store`. It is enabled
  only from `[dev-dependencies]` of `b10x-loom-cli`. With resolver 2 (`Cargo.toml:2`), a
  dev-dependency's feature reaches a build only when that build includes test targets.
- **No release build can hold it.** Under `#[cfg(all(feature = "test-unsynced-store",
  not(debug_assertions)))]` the crate fails with `compile_error!`.
  - So `cargo test --release` and `cargo build --release --all-targets` fail in this workspace.
  - No gate or CI step runs either. The only `--release` is `binaries.yml:38`, which builds one
    package and one binary.
- **Turning it on.** With the feature compiled, the switch is on only while
  `B10X_LOOM_TEST_UNSYNCED_STORE=1` is in the loom process's own environment. Then the door syncs
  nothing and counts `SKIPPED`.
  - Without the feature the variable is never read.
  - No bindings document, protocol file, engine configuration, flag, argument or store file can
    turn it on.
- **Test reports.** Under the feature only, `B10X_LOOM_TEST_SYNC_COUNTS=<file>` makes the binary
  write `{"synced":n,"skipped":n}` to that file at exit. `main` (`crates/loom-cli/src/main.rs:62`)
  calls `durable::write_counts()`, which does nothing without the feature. This is how a test reads
  the counters of a `b10x-loom` child.
- **The marker.** Under the feature the crate holds `#[used] static` bytes
  `b"b10x-loom-test-unsynced-store-marker"`. `loom-xtask unsynced-store <binary>` refuses a binary
  that contains them.
- **Who sets the variable.** `tree_run.rs` sets it on each child in its `command()` helper
  (`crates/loom-cli/tests/tree_run.rs:118-127`). `tests/support/loom.rs` may set it for other CLI
  tests. Nothing else does. `task test-step` refuses to start when the variable is in its own
  environment.
- **Bound programs.** A bound program's environment is built whole: `PATH` and `LOOM_*`
  (`crates/loom-bindings/src/child.rs:60`). So the variable never reaches a program, nor a
  `b10x-loom` that a program starts.

### What keeps full sync

The tests that prove durability run in-process, never set the variable, and are the only tests
that need sync:

- `crates/loom-runlog/tests/run_log.rs:585`;
- `crates/loom-governor/tests/`: `restart_durability.rs:37`, `step_write.rs:271` and `:310`,
  `decision_store.rs:264` and `:456`, `child_cases.rs:736` and `:766`.

### The budget is enforced

`task test-step` already prints the ten slowest tests and fails a step over 120 s of wall time
(`crates/loom-xtask/src/test_step.rs:27`, `:31`, `:222`); that stays. Two bounds are added, beside
it and not instead of it:

- `.config/nextest.toml` gains an override with the filter `binary_id(b10x-loom-cli::tree_run)`.
  It kills a test at 20 s: `slow-timeout = { period = "20s", terminate-after = 1 }`, the form of
  `:3`.
- `test_step.rs` sums the `time` of every `testcase` of the JUnit `testsuite` whose `name` is
  `b10x-loom-cli::tree_run`, nextest's binary id (the form at `test_step.rs:467`). It fails the
  step when that sum is over 60 s, naming the binary and the sum.

### The release path

- `binaries.yml` gains a step after the build (`:38`) beside `linked-libraries` (`:41`):
  `loom-xtask unsynced-store target/release/b10x-loom`.
- A lane `task durable-check` runs in `task check` and in CI, outside the 120 s test step:
  - the four count tests per package, without the feature: `cargo nextest run -p
    b10x-loom-governor -p b10x-loom-runlog -p b10x-loom-tree -p b10x-loom-bindings` with a filter
    on their names. No package with the dev-dependency is selected, so the feature is off;
  - a negative build, `cargo check --release -p b10x-loom-governor --features
    test-unsynced-store`, which must fail with the `compile_error!` text;
  - `cargo tree -p b10x-loom-cli -e normal,features --target all`, whose output must not hold
    `test-unsynced-store`.

### Options for D1, the form of the switch

- (a) **Default**, as above.
- (b) Batched sync, one `syncfs` per command, for tests only. It takes more code, still costs one
  sync per command, and is a product mode one flag away.
- (c) Test stores on tmpfs. It needs no code, but the host forbids work files in `/tmp` and
  `/dev/shm`, and CI's runner differs.
- (d) **Rejected:** fewer full trees per test.
  - U1's shared fixture already took the binary from 145 s to 98.4 s.
  - The tests left each assert on a run of their own: an edit after `tree open`, a deleted
    protocol directory, an observer of child lines. Each makes at least one `tree run`: 123 fsyncs
    in U1's probe, about 7 s at 57 ms each under load.
  - 56 tests × 7 s is about 390 s summed, so even one tree per test misses 60 s.
- (e) **Rejected:** moving scenarios to `story:tree-suite-format`'s in-process `tree test`, which
  starts no `b10x-loom` process (`tree-suite-format.md:250`).
  - The `tree_run` tests prove the binary: its arguments, exit codes and printed decisions.
  - `tree test` writes through the same stores, so it makes the same syncs (inferred).
- **Refused**, because each can weaken a product run: a variable read in every build; any member
  of a bindings, protocol or engine document.

Security-relevant: yes. This touches the durability of the run log and the store. A switch that
reached a product run would let a crash lose an acknowledged step. The unit gets a security review.

## Why

`~/beyond10x/AGENTS.md` § Tests stay fast: the step ends within 120 s, no test runs over 60 s, and
a step over the budget is a defect fixed before the next feature. Measured by units at host load
about 100 on 20 cores:

| Head | Tests | `tree_run` time |
|---|---|---|
| `726a38c2` | not recorded | 145 s |
| after U1's shared fixture | 34 | 98.4 s |
| more tests merged | not recorded | 129.5 s |
| `a5889afd` | 56 | 144.5 s |

The slowest tests take 32 to 48 s:

- `an_edit_after_tree_open_changes_nothing_in_the_open_tree` (`tree_run.rs:2323`);
- `a_tree_from_a_protocols_directory_runs_with_the_directory_deleted` (`:2312`);
- `nothing_in_a_run_selects_or_replaces_a_protocol` (`:2345`);
- four `a_status_*` tests (`:787`, `:820`, `:866`, `:898`);
- `an_observer_is_given_the_record_of_each_child_line` (`:1359`).

The cause, measured by U1:

- one `tree run` took 1.9 s, and 87.5 % of its syscall time was 123 `fsync` calls of 57 ms each
  under disk load;
- a full two-level tree makes about 1,000 fsyncs, which is 30 to 45 s.

Every test starts the real binary (`tree_run.rs:120`).

## ESS first

`ess/` does not change: no noun, no wire member. The first commit holds the tests below. The count
tests pass today, which proves no sync is lost; the switch and budget tests are red.

## Acceptance

Bounds for every test below: no test over 60 s (20 s in `tree_run`), the test step within 120 s,
no network, no model call. Fixture programs are POSIX sh, not Python (§ No Python for permanent
tools). Every brief of the unit names that rule and the slot pool's 10-minute limit.

- **Count tests: every durable write syncs.** With the switch off, the door's `SYNCED` grows by
  the syncs listed above, and `SKIPPED` stays 0:
  - a `FileStore` write: 2;
  - a `LockedDir` replace: 2;
  - a remove: 1;
  - a run-log append: 2;
  - a tree store write: 2;
  - a kept blob: 1.

  Tests: `each_governor_write_syncs_its_file_and_directory`,
  `a_run_log_append_syncs_its_file_and_directory`,
  `a_tree_store_write_syncs_its_file_and_directory` and `a_kept_blob_is_synced`. They run in the
  workspace step with the feature compiled, and again in `durable-check` without it.
- **No sync outside the door.** No `.sync_all()` or `.sync_data()` appears in `src/` of
  `loom-governor`, `loom-runlog`, `loom-tree`, `loom-bindings` or `loom-cli`, except in the
  `durable` module. Test: `no_sync_outside_the_door`.
- **A failing file sync is still an error.** With the switch off, `sync_file` on the write end of
  a pipe returns its `EINVAL`. `sync_dir` on a path that cannot be opened returns, as today. Test:
  `a_failing_file_sync_is_an_error_with_the_switch_off`.
- **The switch is off by default.** With the feature compiled and the variable unset, or set to
  any value but `1`, the door syncs. Test: `the_switch_is_off_unless_its_variable_is_1`.
- **Load-free proxy.** With the switch on, a full two-level tree of the `tree_run` fixture
  reports `synced: 0` and `skipped` above 0. With it off, the same tree reports `skipped: 0`.
  Test: `with_the_switch_a_two_level_tree_makes_no_sync`.
- **A bound program never sees the switch.** With the switch on, a POSIX sh program prints its
  environment into its kept blob, and the blob does not hold the variable's name. Test:
  `a_bound_program_never_sees_the_switch`.
- **Documents cannot set it.** With the switch unset, a tree runs whose bindings hold a program
  argv `[env, B10X_LOOM_TEST_UNSYNCED_STORE=1, ./prog.sh]` and whose protocol holds the string
  `B10X_LOOM_TEST_UNSYNCED_STORE=1` as a description. It reports `skipped: 0` and the full
  `synced` count. A bindings member of that name is refused `UndeclaredMember` at load. Test:
  `a_bindings_or_protocol_member_cannot_set_the_switch`.
- **A person step.** `tree approve --action` takes the person action through
  `take_person_action` (`crates/loom-tree/src/approve.rs:287`), which writes the records and one
  run-log line. `tree.rs:244` (`crates/loom-cli/src/tree.rs`) only prints the result. With the
  switch unset, the step reports `skipped: 0` and the `synced` count of its writes. Test:
  `a_person_step_syncs_in_full`.
- **The release check.** `loom-xtask unsynced-store` refuses a fixture file holding
  `b10x-loom-test-unsynced-store-marker` and passes one without it, naming the file. Test:
  `a_binary_with_the_test_sync_switch_is_refused`.
- **The gate refuses the switch.** `task test-step` started with the variable set exits non-zero
  before any test, naming it. Test: `the_test_step_refuses_the_switch_in_its_environment`.
- **Who may name the variable.**
  - The allow-list of files that may name it: the `durable` module, `test_step.rs`, the
    `unsynced-store` subcommand of `loom-xtask`, `tests/tree_run.rs` and `tests/support/loom.rs`
    of `loom-cli`, and the files of the tests named in this Acceptance.
  - No file outside the list names it, under `crates/` including every `tests/` directory, in
    `Taskfile.yml` or in `.github/`.
  - No file but `tests/tree_run.rs` and `tests/support/loom.rs` sets it, through `.env(`,
    `set_var` or `setenv`, as distinct from reading it.

  Test: `only_the_allowed_files_name_or_set_the_switch`.
- **The durability tests stay.** The eight tests listed above pass unchanged and do not name the
  variable (checked by the same test).
- **The budget holds.**
  - A JUnit fixture whose `b10x-loom-cli::tree_run` test cases sum to 61 s fails the step, naming
    the binary. Test: `a_binary_over_its_budget_fails_the_step`.
  - `.config/nextest.toml` has the override above for `binary_id(b10x-loom-cli::tree_run)`, and
    that binary id is in `cargo nextest list -p b10x-loom-cli --list-type binaries-only` output.
    Test: `the_tree_run_binary_has_the_20_s_override`.
  - The timing is the second check, after the proxy. The unit's report gives `tree_run`'s summed
    and wall time and its ten slowest tests, measured under the gate with the host's load named.
    It must show under 60 s summed and no test over 20 s.
  - What remains once sync is gone is the non-sync cost per test times the test count. If the
    binary still misses 60 s, the story stays open. The unit reports the per-test non-sync time,
    and the next step is fewer trees per test.

## Not covered

- One integration-test binary per crate (`tests/main.rs`): `crates/loom-cli/tests/` holds 36 files
  and no `main.rs`. That is a story of its own.
- The protocol catalog install (`crates/loom-protocols/src/store.rs:198`, `:203`). It does not
  depend on `b10x-loom-governor` and costs two syncs per `tree open`.
- `state_dir.rs` of `loom-intake-slice` and `loom-plugin`: not on the tree path.

## Scope

- Cited:
  - `crates/loom-governor/src/locked_dir.rs` (`:54-82`, `:127-142`, `:177-182`, `:184-188`) and
    `crates/loom-governor/src/file_store.rs:415`, `:421`;
  - `crates/loom-runlog/src/lib.rs:295`, `:304`;
  - `crates/loom-tree/src/store.rs:63-85` and `crates/loom-tree/src/lock.rs:115`;
  - `crates/loom-bindings/src/program.rs:784` and `crates/loom-cli/src/stage.rs:185`;
  - `crates/loom-cli/src/main.rs:62` and `crates/loom-cli/tests/tree_run.rs:118-127`;
  - `.config/nextest.toml:3`, `crates/loom-xtask/src/test_step.rs`, `binaries.yml:38-41` and
    `Taskfile.yml`.
- Inferred:
  - the `durable` module in `crates/loom-governor/src/`;
  - `crates/loom-governor/Cargo.toml` and `crates/loom-cli/Cargo.toml`;
  - `crates/loom-cli/tests/support/loom.rs`;
  - `crates/loom-xtask/src/main.rs`.
- Not changed: `crates/loom-cli/src/tree.rs`. The switch is read in the governor.

Edges:

- depends_on `story:tree-run-command` and `story:test-step-within-budget`, which owns
  `test_step.rs` and `.config/nextest.toml`.
- Shares `tests/tree_run.rs` with every story that adds tree tests:
  - `story:every-printed-decision-command-runs`;
  - `story:person-action-records-a-named-result`;
  - `story:handoff-when-a-program-decides`;
  - `story:program-reports-its-cost`;
  - `story:tree-run-takes-a-delegated-cgroup`;
  - `story:tree-status-names-what-it-read`.
- Shares the sync sites:
  - `crates/loom-runlog/src/lib.rs` with `story:child-record-in-the-run-log` and the stories
    after it in the run-log order;
  - `locked_dir.rs` and `file_store.rs` with `story:decision-request-file-store` and
    `story:bound-artifact-revisions`;
  - `binaries.yml` with `story:release-binaries-macos`.

  Each of the first three names `no_sync_outside_the_door` in its own Edges: a sync it adds goes
  through the door.
- The later unit merges the earlier head. As a budget defect, this story goes ahead of the next
  feature.
