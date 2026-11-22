---
format: aep.planning-md/3
id: story:tree-library-exports-what-an-engine-needs
kind: story
status: active
title: 'b10x-loom-tree exports what an in-process engine needs: catalog, line encoder, and a runner on a held cgroup root'
summary: 'embed module: TreeRunner on a held root (never discovery), in own scope, local; ProtocolCatalog, host_catalog, encode_line, ConfinementError re-exported; fixture consumer on b10x-loom-tree only; SDK stability rows'
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:tree-run-command
- depends_on: story:tree-run-takes-a-delegated-cgroup
- depends_on: story:cancel-a-running-bound-program
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.toml
- confidence: inferred
  path: Taskfile.yml
- confidence: cited
  path: crates/loom-cli/src/tree.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/tree-delegated
- confidence: cited
  path: crates/loom-cli/tests/tree_run/delegated_cgroup.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement/engine.rs
- confidence: cited
  path: crates/loom-protocols/src/lib.rs
- confidence: cited
  path: crates/loom-protocols/src/store.rs
- confidence: cited
  path: crates/loom-runlog/src/lib.rs
- confidence: inferred
  path: crates/loom-sdk/Cargo.toml
- confidence: cited
  path: crates/loom-sdk/src/lib.rs
- confidence: inferred
  path: crates/loom-sdk/tests/host_ports.rs
- confidence: inferred
  path: crates/loom-tree/src/embed.rs
- confidence: cited
  path: crates/loom-tree/src/lib.rs
- confidence: cited
  path: crates/loom-tree/src/open.rs
- confidence: cited
  path: crates/loom-tree/src/run.rs
- confidence: cited
  path: crates/loom-tree/src/status.rs
- confidence: inferred
  path: crates/loom-tree/tests/embedder
- confidence: inferred
  path: crates/loom-tree/tests/main.rs
- confidence: cited
  path: crates/loom-xtask/src/release.rs
- confidence: cited
  path: website/docs/guides/embed-the-runtime.md
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T10:29:42Z", actor: "agent:loom", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-22T10:41:24Z", actor: "agent:sdc", revision: 5, executor: "agent:loom", correlation: "runbook-engine-gaps-2026-11-22"}
---
## Outcome

An engine that depends on `b10x-loom-tree` alone can open, run, approve, close and read a tree
under Substrate on a delegated cgroup root it holds, cancel a run, and encode the run-log lines
its observer gets. It can name every type those calls take. Cites are at `ec1a82f9`; the cites
added in this revision are at `b9ec2657`, with no code change between the two.

### What is missing today

`crates/loom-tree/src/lib.rs:46-75` re-exports from other crates only:

- the runner types `ConfinementAdapter`, `EngineConfinement`, `EngineRunner`, `TestRunner` and
  `UNCONFINED_FLAG_REASON` (`:46-51`);
- `log_file_name` and `read_log` (`:52-53`);
- the generated `runlog` and `model` modules (`:54-56`).

An embedder still needs four more crates:

- **`ProtocolCatalog`** (`crates/loom-protocols/src/lib.rs:181`) is a parameter of `run_tree`
  (`crates/loom-tree/src/run.rs:126-132`), `tree_status` (`crates/loom-tree/src/status.rs:67`)
  and `OpenRequest::catalog` (`crates/loom-tree/src/open.rs:59`). It is not re-exported, and
  neither is the host's catalog: the CLI builds that from `InstallStore`
  (`crates/loom-cli/src/tree.rs:79-88`, `crates/loom-protocols/src/store.rs:14`).
- **The run-log line encoder.** A `RunObserver` gets each `RunLogLine` (`run.rs:73-76`). Its
  text comes from `b10x_loom_runlog::encode` (`crates/loom-runlog/src/lib.rs:428-429`), which is
  not re-exported.
- **`SubstrateRunner`** and **`ConfinementError`**
  (`crates/loom-intake-slice/src/confinement.rs:244`, `:211`) are not re-exported. An embedder
  can still reach Substrate through the re-exported `EngineRunner::open` with
  `ConfinementAdapter::Linux` (`crates/loom-intake-slice/src/confinement/engine.rs:89-100`).
  With `cgroup_root: None`, that builds `SubstrateRunner::new(None)` (`confinement.rs:422`), and
  its driver discovers an ancestor (`discover_cgroup_root`, `:3621`). `tree run` never allows
  that (`crates/loom-cli/src/tree.rs:154-160`).
- **The cross-process hold.** `hold_cgroup_root` and `CgroupRootHold` (`confinement.rs:3658`,
  `:3673`) of `story:tree-run-takes-a-delegated-cgroup` are not re-exported, and neither is
  `prepare_delegated_cgroup` (`:3637`). The CLI's `place` (`tree.rs:182-202`) holds the root it is
  given, or moves into a leaf of its own delegated scope and holds that. The re-exec
  (`start_again`, `tree.rs:207`) is the CLI's alone: a library cannot start its host again.

### The entry

One public module of `b10x-loom-tree`, `embed`. This story owns it, and it holds:

- **`TreeRunner::on_root(root: &Path) -> Result<TreeRunner, Refused>`.** It holds `root` with
  `hold_cgroup_root` and builds `SubstrateRunner::new(Some(root))`. It never discovers a root. A
  root another process holds, or one that does not qualify, is refused `ConfinementRefused` with
  the text of `tree run`.
- **`TreeRunner::in_own_scope() -> Result<TreeRunner, Refused>`**, for a process an operator
  started in a delegated scope (`systemd-run --user --scope -p Delegate=yes`). It calls
  `prepare_delegated_cgroup`, which moves the calling process, every thread of it, into a leaf of
  the scope. Then it holds the scope as `on_root` does. The embedder accepts that move by calling
  it.
- **`TreeRunner::local() -> Result<TreeRunner, Refused>`.** This is Substrate's `local` adapter,
  opened with `UNCONFINED_FLAG_REASON`, which `local()` supplies itself, as `tree_runner` builds
  it (`tree.rs:144-150`).
- **`TreeRunner::runner() -> Arc<dyn TestRunner>`.** The value given to `RunSettings::runner`
  (`run.rs:93`) owns the hold. It is a runner type of `embed` that holds the `SubstrateRunner`
  and an `Arc<CgroupRootHold>`, so every clone of the `Arc` keeps the hold. The root is released
  only when the `TreeRunner` and every clone of its runner are dropped, so no runner outlives its
  hold.
- **Re-exports:**
  - `ProtocolCatalog`;
  - `host_catalog()`, the CLI's function moved into the library, which the CLI then calls;
  - `encode_line`, which is `b10x_loom_runlog::encode`;
  - `ConfinementError`;
  - `Cancel` of `story:cancel-a-running-bound-program`. That story adds the type to the intake
    slice, and `run_tree` keeps `cancel: Option<&AtomicBool>`; this story re-exports the type.

`embed` narrows nothing. `RunSettings::runner` still takes any `TestRunner`, and
`EngineRunner::open` and `loom_sdk::SubstrateRunner::new(None)` still discover a root. `embed` is
the path that does not, and the guide says to use it.

`tree_runner` and `place` in the CLI call the same functions, so the CLI and an embedder build one
runner the same way.

### D1: where the entry is promised

- (a) **Default.** `b10x-loom-sdk` re-exports `b10x_loom_tree::embed` as `loom_sdk::tree`.
  - **The stability page.** Its rows join "Types and functions an embedding calls" of
    `website/docs/reference/sdk-stability.md:112`, held by `crates/loom-sdk/tests/host_ports.rs`.
    - Stable: `TreeRunner`, `on_root`, `in_own_scope` (it moves the calling process into a
      leaf), `local` (it supplies `UNCONFINED_FLAG_REASON`), `runner`, `host_catalog`,
      `encode_line` and `Cancel`.
    - `ProtocolCatalog` is one type with two paths: `loom_sdk::ProtocolCatalog`
      (`crates/loom-sdk/src/lib.rs:139`) and `loom_sdk::tree::ProtocolCatalog`. The page lists
      both in one row.
    - The `model` and `runlog` types stay *generated* (`sdk-stability.md:33-35`).
  - **The CHANGELOG.** `crates/loom-sdk/tests/host_ports.rs` is a stable surface of
    `release-check` (`crates/loom-xtask/src/release.rs:44-49`). A tag whose entry says "No
    breaking changes." while that file differs is refused (`:11-14`). So the release that ships
    this carries a `**Breaking:**` bullet naming `loom_sdk::tree` added to the witness, and moves
    the minor version. `release-check` is not changed.
- (b) It is in `b10x-loom-tree` only, outside the promise. `sdk-stability.md` gains one row
  saying `b10x_loom_tree::embed` is outside the promise. That row has its own check, the test
  `the_stability_page_names_the_tree_entry_outside_the_promise`.

### The guide

`website/docs/guides/embed-the-runtime.md` gains a section, "Run a case tree". Its example is the
fixture's code: catalog, `TreeRunner::on_root`, `RunSettings`, `run_tree` with an observer and a
cancel flag set from another thread, `approve`, `close` and `tree_status`. The lines at `:68-69`
say a host "allows discovery"; they gain that a tree runner from `embed` never discovers.

Security-relevant: yes. This lets an embedder choose where confined processes run, and it must
not reopen discovery, bypass the hold or let a runner outlive it. The unit gets a security review.

## Why

Runbook's engine runs trees in-process. It links `b10x-loom-tree`, calls `run_tree` with a
`RunObserver` and the cancel flag, and calls `open_tree`, `approve`, `close` and `tree_status`.
Since `story:tree-run-takes-a-delegated-cgroup`, `tree run` holds a delegated root and never
discovers one. An embedder must be able to do the same without copying the CLI's private
functions or depending on four more crates.

## ESS first

`ess/` does not change: no noun, no wire member. The first commit holds the fixture package and
the tests below; they do not compile until the exports exist.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no network, no model
call. Stub programs are POSIX sh, not Python (§ No Python for permanent tools). Every brief of the
unit names that rule and the slot pool's 10-minute limit. Scratch directories are under
`CARGO_TARGET_TMPDIR`, never `/tmp`.

**The fixture package** is `crates/loom-tree/tests/embedder/`, package
`b10x-loom-tree-embedder-fixture`. It is added to the workspace's `members`, because
`Cargo.toml:3` matches `crates/*` only.

- It has a binary, `embedder`. The binary opens a one-case tree, runs it with an observer that
  encodes each line, and approves, closes and reads it, all through `b10x_loom_tree` paths alone.
  Its arguments are `--cgroup-root DIR`, `--own-scope` or `--local`, a store and a workspace, and
  `--cancel-after-ms N`, which sets the `run_tree` flag from a second thread. Its code names
  `embed::Cancel`. With `--hold-clone` it runs no tree:
  - it builds `TreeRunner::on_root`, clones `runner()`, drops the `TreeRunner` and prints
    `clone-held`;
  - after one line on its standard input, it drops the clone and prints `clone-dropped`;
  - it then waits until its standard input closes.
- It has its own tests in `crates/loom-tree/tests/embedder/tests/main.rs`, which start
  `env!("CARGO_BIN_EXE_embedder")` as child processes. A second process is a second `embedder`
  child.
- **The delegated root.** It comes from the CLI's fixture
  `crates/loom-cli/tests/fixtures/tree-delegated/scope.sh`, which holds a scope of `systemd-run
  --user --scope -p Delegate=yes` and prints its directory. Each `embedder` process enters a leaf
  through `enter.sh` in the same directory, as
  `crates/loom-cli/tests/tree_run/delegated_cgroup.rs:1-12` describes.
- **The stub.** A POSIX sh program copies `/proc/self/cgroup` into its standard output, which is
  kept as the run's blob.
- **Delegation.** The tests that need a delegated cgroup skip without one only when
  `LOOM_REQUIRE_DELEGATION` is unset. The unit's gate sets it and runs them under
  `systemd-run --user -p Delegate=yes --scope`. The report lists the tests run and the tests
  skipped, and a skip counts as red there.

The tests:

- **Only `b10x-loom-tree`** (fixture `tests/main.rs`). The test reads the fixture's `Cargo.toml`:
  `[dependencies]` holds `b10x-loom-tree` alone, and `[dev-dependencies]`,
  `[build-dependencies]` and every `[target.*]` table are empty. `Cargo.lock` gains no package but
  the fixture's own entry, whose dependencies are `b10x-loom-tree` alone. Test:
  `the_embedder_fixture_depends_on_b10x_loom_tree_only`.
- **Offline.** `task check` runs `slot cargo check --offline --locked -p
  b10x-loom-tree-embedder-fixture`, and it exits 0.
- **A run on a held root** (fixture). `embedder --cgroup-root <scope leaf>` runs the stub. The
  blob's cgroup path, read under `/sys/fs/cgroup`, starts with the held root. Test:
  `an_embedded_tree_runs_under_the_root_it_holds`.
- **No second holder** (fixture). While one `embedder` holds a root, a second `embedder` on it is
  refused `ConfinementRefused`, naming the root, and exits non-zero. Once the first has exited, a
  third gets it. Test: `a_held_root_is_refused_to_a_second_embedder`.
- **The runner owns the hold** (fixture). An `embedder --hold-clone --cgroup-root <leaf>`,
  entered through `enter.sh`, prints `clone-held`.
  - A second `embedder` on the same root is refused `ConfinementRefused`.
  - The test then writes one line to the first process and reads `clone-dropped`. A third
    `embedder` on the root runs its tree while the first process is still alive, so the release
    came from dropping the clone, not from the process exiting.

  Test: `a_runner_clone_keeps_the_hold`.
- **Never discovery** (fixture). With `scope.sh`'s delegated empty scope as an ancestor,
  `embedder --cgroup-root <dir>` names a root that does not qualify (not empty, or not
  delegated). It is refused `ConfinementRefused`, the stub writes no blob, and no process starts
  under the ancestor. Test: `an_unqualified_root_is_refused_and_no_ancestor_is_used`.
- **Own scope** (fixture). An `embedder --own-scope` started in `scope.sh`'s scope without
  `enter.sh` moves into a leaf. Every line of its `/proc/<pid>/task/*/cgroup` names that leaf,
  and the stub's cgroup starts with the scope. Test: `in_own_scope_moves_every_thread_into_a_leaf`.
- **Local** (fixture). `embedder --local` runs the stub under Substrate's `local` adapter, and the
  runner's `local_reason()` (`confinement.rs:93`) is `UNCONFINED_FLAG_REASON`. Test:
  `local_supplies_the_unconfined_reason`.
- **Cancel from another thread** (fixture). `embedder --cancel-after-ms 1000` on a stub that
  sleeps 30 s ends with `TreeRun.stop` `Cancelled` within the bound of
  `story:cancel-a-running-bound-program`. Test: `an_embedder_cancels_a_running_tree`.
- **The encoder** (`crates/loom-tree/tests/main.rs`, module `embed`). `encode_line` returns a
  line without its line feed (`crates/loom-runlog/src/lib.rs:428`). The lines an observer got,
  each encoded and followed by `\n`, are exactly the bytes of the case's log file. Test:
  `encode_line_gives_the_bytes_of_the_log`.
- **The CLI's runner is the library's.** The tests of
  `crates/loom-cli/tests/tree_run/delegated_cgroup.rs` pass unchanged, and so does the `local`
  case of `crates/loom-cli/tests/tree_run.rs` (`--confinement local`).
- **The stability page** (`crates/loom-sdk/tests/host_ports.rs`, under D1 (a)). The file has one
  `use` per new `loom_sdk::tree` path, and fails when the page's rows and the code disagree. This
  is the existing test `host_ports`, extended.

## Not covered

- The `Cancel` type, the kill and the cancelled stop: `story:cancel-a-running-bound-program`.
- A new form of confinement, or changes to Substrate.
- The CLI's re-exec, which stays the CLI's.

## Scope

- Cited:
  - `crates/loom-tree/src/lib.rs:46-75`;
  - `crates/loom-tree/src/run.rs:73-76`, `:93`, `:126-132`, `crates/loom-tree/src/status.rs:67`,
    `crates/loom-tree/src/open.rs:59`;
  - `crates/loom-cli/src/tree.rs:79-88`, `:136-160`, `:182-202`, `:207`;
  - `crates/loom-intake-slice/src/confinement.rs:93`, `:211`, `:244`, `:422`, `:3621`, `:3637`,
    `:3658`, `:3673`, and `crates/loom-intake-slice/src/confinement/engine.rs:89-100`;
  - `crates/loom-protocols/src/lib.rs:181`, `crates/loom-protocols/src/store.rs:14`,
    `crates/loom-runlog/src/lib.rs:428-429`;
  - `crates/loom-cli/tests/tree_run/delegated_cgroup.rs:1-12`, the fixtures `scope.sh` and
    `enter.sh` of `crates/loom-cli/tests/fixtures/tree-delegated/`;
  - `website/docs/guides/embed-the-runtime.md:52-69`,
    `website/docs/reference/sdk-stability.md:33-35`, `:112`;
  - `crates/loom-sdk/src/lib.rs:139`, `crates/loom-xtask/src/release.rs:11-14`, `:44-49`;
  - `Cargo.toml:3`.
- Inferred:
  - the new `crates/loom-tree/src/embed.rs`;
  - the fixture package `crates/loom-tree/tests/embedder/` (`Cargo.toml`, `src/main.rs`,
    `tests/main.rs`, the stub);
  - a module `embed` of `crates/loom-tree/tests/main.rs`;
  - `Taskfile.yml` for the offline check;
  - under D1 (a): `crates/loom-sdk/Cargo.toml`, `crates/loom-sdk/src/lib.rs`,
    `crates/loom-sdk/tests/host_ports.rs`;
  - `CHANGELOG.md`.

Edges:

- depends_on `story:tree-run-command`, `story:tree-run-takes-a-delegated-cgroup` and
  `story:cancel-a-running-bound-program`, whose `Cancel` this story re-exports.
- Shares `website/docs/guides/embed-the-runtime.md` (`:52-69`) with
  `story:cancel-a-running-bound-program`, whose Scope names `:53-69`.
- Shares `crates/loom-tree/src/lib.rs`, `crates/loom-tree/Cargo.toml` and
  `crates/loom-tree/tests/main.rs` with `story:tree-suite-format`.
- Shares `crates/loom-cli/src/tree.rs` with `story:tree-run-takes-a-delegated-cgroup`.
- The later unit merges the earlier head.
