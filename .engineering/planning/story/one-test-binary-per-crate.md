---
format: aep.planning-md/3
id: story:one-test-binary-per-crate
kind: story
status: active
title: Every crate of Loom builds at most one integration-test executable, tests/main.rs with one module per test file, and the gate refuses a second
summary: 471 test targets in 31 crates become 30 (29 main + intake-conformance); autotests = false; loom-xtask test-layout; about 506 executables to 65; target/ under 15G
tags:
- gate
relations:
- serves: vision:O1
- depends_on: story:test-step-within-budget
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: Taskfile.commission.yml
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates
- confidence: cited
  path: crates/loom-xtask/src/main.rs
- confidence: inferred
  path: crates/loom-xtask/src/test_layout.rs
- confidence: cited
  path: website/docs/guides/run-the-checks.md
- confidence: cited
  path: website/docs/reference/run-events.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T22:25:08Z", actor: "agent:host-operator", revision: 6, executor: "agent:loom", correlation: "loom-gate-rules-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T22:25:08Z", actor: "agent:host-operator", revision: 7, executor: "agent:loom", correlation: "loom-gate-rules-2026-11-21"}
---
## Outcome

Every crate of Loom's workspace builds at most one integration-test executable. A crate with more
than one file under `tests/` has `autotests = false` in its `Cargo.toml` and one target
`[[test]] name = "main", path = "tests/main.rs"`. `tests/main.rs` declares one module per former
test file, `mod <file>;`, so every file stays where it is and keeps its name. A crate with one
test file keeps that file as its one target. The test profile carries no full debug info
(`[profile.dev] debug = false` in the workspace `Cargo.toml`; `test` inherits it). `task check`
fails when a crate gains a second integration-test target or a file under `tests/` that no
target compiles.

- `loom-xtask test-layout` (new, `crates/loom-xtask/src/test_layout.rs`), a step of `task check`.
  It reads `cargo metadata --no-deps --format-version 1 --locked --offline`, counts each
  package's targets of kind `test`, and also reads each package's `tests/` directory. It exits
  1 naming the package and the file in four cases:
  - a package has more than one `test` target;
  - a `tests/*.rs` file other than the target's own is not declared `mod <file>;` in
    `tests/main.rs`;
  - an example has `test = true`;
  - a target sets `harness = false`.
  Otherwise it exits 0 and prints the number of test targets.
- Shared code: `tests/main.rs` declares the crate's `support` module once (`mod support;`, or
  the `#[path]` that reaches another crate's `support`). Module files use `crate::support`. A
  file that included another test file by `#[path]` (`adversary2_gate.rs` including `ess_gate.rs`)
  uses `crate::<file>` instead, so no test is compiled twice.
- Test names gain their module: `ess_gate::gate_holds_on_the_specification`. Every test that
  starts its own binary again with `--exact <name>` passes the full name. Every test that looked
  for a sibling test binary by its prefix (`executor_port-`, `conform-`) runs `current_exe()`
  with that module's filter instead, since the module is now in the same binary.
- Commands that name a target by file are rewritten:
  - `--test ess_gate` becomes `--test main ess_gate::`. This covers `Taskfile.yml`,
    `Taskfile.commission.yml`, `AGENTS.md:1649`, the replica tests, and the tests that assert
    the Taskfile's text.
  - The two console blocks of `website/docs/reference/run-events.md` (`:189`, `:237`) are run
    again and their output pasted.

## Why

`~/beyond10x/AGENTS.md` § Tests stay fast: "Test binaries are few: one integration-test binary per
crate (`tests/main.rs` with modules), and `[profile.dev] debug = false` or a test profile without
full debug info." Loom on 2026-11-21: the first `cargo nextest run --workspace` in
`loom-w1121g-u4` linked 516 executables into `target/debug/deps`. The largest was 268,835,408
bytes (`conform-*`) and `libess_conformance-*.rlib` was 279,909,686 bytes, 33G in all within 15
minutes. The workspace `Cargo.toml` (`:1-10`) had no `[profile]` section.

Loom at `8a8fad3b` has 471 test files directly under `crates/*/tests/`, each its own target
(no crate sets `autotests`, `[[test]]` or `harness`). It also has 33 unit-test executables (27
`src/lib.rs`, 6 `src/main.rs`) and 2 examples that `cargo test` builds
(`crates/loom-sdk/examples/`).
That makes 506, against 516 measured; the 10 not accounted for are taken to be binaries of
earlier builds under other hashes (not established).

Test files per crate (files directly under `tests/`; a `support/` file in brackets):

| Crate | Files | Crate | Files |
|---|---|---|---|
| `loom-commission-testkit` | 84 | `loom-conformance` | 6 |
| `loom-executor` | 59 | `loom-commission-conformance` | 6 |
| `loom-intake-slice` | 54 (+1) | `loom-resource` | 5 |
| `loom-session-executor` | 34 | `loom-mandate` | 5 |
| `loom-governor` | 33 | `loom-commission-xtask` | 5 |
| `loom-cli` | 33 | `loom-bindings` | 5 |
| `loom-operator` | 30 | `loom-runlog`, `loom-plugin-slack` (+1) | 3 each |
| `loom-plugin` | 17 (+1) | `loom-intake-router`, `-intake-references` | 3 each |
| `loom-connectors` | 16 | `loom-decision`, `loom-boundary`, `loom-a2a` | 3 each |
| `loom-agent` | 15 | `loom-selector-laya`, `loom-docs` | 2 each |
| `loom-sdk` | 11 | `loom-intake-conformance` | 1 |
| `loom-xtask`, `loom-commission` | 10 each | `loom-commission-docs` | 0 |
| `loom-protocols` | 7 | **sum, 31 crates** | **471** |

After: 29 targets named `main`, plus `loom-intake-conformance`'s `conform`. With the 33 unit-test
executables and 2 examples, that is 65 executables.

Files that cannot simply become modules, and what each needs:

- `harness = false`: none.
- Process-wide state. Under nextest every test runs in its own process, so this is safe in the
  test step. It is not safe under `cargo test`, which `Taskfile.commission.yml:16` and the
  per-crate habit in AGENTS.md § Gate still run, with every module's tests in one process.
  - `crates/loom-session-executor/tests/adversary_w20261030w3_b.rs:502` calls
    `std::env::set_var` on five provider credentials. Its comment says no case "in this file"
    depends on their absence, and that no longer holds when 33 other files share the process.
    The case passes them to the child's `Command` instead.
  - No test calls `set_current_dir`.
  - `workflow_run_store.rs:275` calls `process::exit` only in a child it started itself
    (`:647-648`).
- Own `mod support` or `#[path]`:
  - 34 files declare `mod support;`: `loom-plugin` 16, `loom-intake-slice` 9, `loom-agent` 7,
    `loom-plugin-slack` 2.
  - Seven `loom-agent` files reach `loom-plugin` or `loom-plugin-slack` support by `#[path]`
    (`a2a.rs:13`, `http.rs:6`, …), and `loom-plugin-slack/tests/support/mod.rs:7` reaches
    `loom-plugin`'s.
  - Four `loom-sdk` files compile an example in by `#[path]` (`example_runs.rs:13`,
    `runtime_wrapper.rs:18`, `adversary_w20261023w1_u1.rs:14`, `governed_loop_runs.rs:10`).
    `tests/main.rs` includes each example once.
  - `loom-commission/tests/adversary2_gate.rs:11-12` compiles `ess_gate.rs` a second time; its
    own doc says the cases "run once more inside this binary".
- Same test names: none within one crate would clash as modules. `loom-protocols` has two names
  in two files each (`declined_with_a_decline`,
  `protocol_declares_its_actions_and_outcomes`); module paths keep them apart.
- Rely on their own binary or process. 30 files start `current_exe()` again with `--exact
  <name>`: `loom-intake-slice` 12, `loom-commission-testkit` 5, `loom-session-executor` 3,
  `loom-connectors` 2, `loom-governor` 2, and one each in `loom-agent`, `loom-bindings`,
  `loom-commission`, `loom-conformance`, `loom-runlog` and `loom-sdk`. Each name gains its
  module.
  - Three look for a sibling binary in `target/debug/deps`:
    `adversary2_executor_guard.rs:23-47` and `adversary_executor_guard.rs:33-49`
    (`executor_port-`), and `adversary2_w2_conformance_status.rs:58-70` (`conform-`).
  - Two run `cargo test --offline --test ess_gate` in a replica tree
    (`adversary_ess_gate.rs:77`, `adversary2_ess_gate.rs:122`). After the conversion the
    replica compiles all 59 executor test modules, not one; see ledger question 2.
- Statics: the counters (`WORKSPACES`, `ISSUED`, `RUNS`, `NEXT`; 17 `static` counters and
  locks in test files) stay per module. Scratch directories under `CARGO_TARGET_TMPDIR` (211
  uses) were already shared by every target of a package, so no new collision; a fixed scratch
  name used by two modules is one to check.

The shared-fixture consequence: a `OnceLock` in `tests/main.rs` or a module now serves the whole
crate under `cargo test`, but under nextest it is built once per test process, so once per test.
`workflow_run_store.rs:683-693` in `loom-w1121g-u4` already names its directory after the test
for that reason (to confirm). An expensive fixture therefore takes the committed-file form or a
cache shared across processes (`ess_suite::synthesize_once`, `story:test-step-within-budget`),
not a `OnceLock`.

What does not change:

- The nextest filter `test(/(^|::)slow_/)` in `.config/nextest.toml` matches
  `ess_gate::slow_…` as it matched `slow_…`, and `--skip slow_` still matches by substring.
- Citations of a test by file and function stay true, because the files keep their paths:
  `tests/<file>.rs` is cited 264 times in `AGENTS.md`, 20 in `website/docs`, 64 in `docs`,
  1,777 in 281 story files and in 17 evidence files.
- Citations that are commands or full names change:
  - `Taskfile.yml:64`, `:138`, `:143`;
  - `Taskfile.commission.yml:36`, `:41`, `:46`;
  - `AGENTS.md:1649`;
  - `website/docs/reference/run-events.md:189`, `:237`;
  - the tests that assert those lines: `ess_gate.rs` `taskfile_check_runs_the_ess_gate`,
    `conform.rs:148`, `loom-commission/tests/ess_gate.rs:590`,
    `adversary2_executor_guard.rs:293`;
  - two open stories: `story:tree-run-command` (proposed, `--test tree_run`) and
    `story:bound-program-result-table` (active, six `--test` names).
  - Stories already implemented and `docs/qualification/` are records of past runs and stay as
    written.

Forms chosen:

- `autotests = false` with files left in place. Rejected: moving the modules to `tests/main/`
  (cargo's other layout), which changes all 2,145 cited `tests/<file>.rs` paths.
- One target per crate. Rejected: one target per workspace (a single `tests` crate). It would
  need every crate's dev-dependencies at once, and a change in one crate would relink every
  test.
- Cost accepted: a change to one test module recompiles its crate's whole test binary.

## ESS first

No declaration under `ess/` changes; the layout of test targets is not part of what Loom runs.

## Acceptance

Measured by the implementor on this machine, before (at the branch's base) and after, with a warm
dependency build and the slot queue wait excluded. Every number goes in the unit's report.

- Executables: `find target/debug/deps -maxdepth 1 -type f -executable ! -name '*.*' | wc -l`
  after a clean `cargo test --workspace --locked --no-run` is at most 70 after (expected 65),
  against the number before (expected about 506).
- `du -sh target` after the whole test step (`loom-xtask test-step`) is at most 15G; the figure
  before is reported beside it.
- Build-and-link time of the test binaries, `cargo test --workspace --locked --no-run` from a
  `target/` holding only dependencies: both times are reported, and the time after is lower.
- The test step (`loom-xtask test-step`) ends within 120 s, no test over 60 s, and its ten
  slowest are printed; the count of tests it ran equals the count before, plus or minus the
  tests the unit names.
- `cargo nextest list --workspace --locked` lists the same test functions before and after, each
  with its module added; a script comparing the two lists modulo the module prefix prints no
  difference.
- `cargo metadata --no-deps --format-version 1 --locked --offline` shows at most one target of
  kind `test` per package.
- `loom-xtask test-layout` exits 0 on the tree. Its cases are in
  `crates/loom-xtask/tests/main.rs` module `test_layout`, each on a fixture workspace of one or
  two small crates under `CARGO_TARGET_TMPDIR`, each under 5 s:
  - `a_second_test_target_fails`: a fixture crate with `tests/a.rs` and `tests/b.rs` and no
    `autotests = false` exits 1 naming the crate and both targets.
  - `an_undeclared_test_file_fails`: `tests/main.rs` without `mod b;` beside `tests/b.rs` exits
    1 naming `tests/b.rs`.
  - `an_example_compiled_as_a_test_fails`: `[[example]] test = true` exits 1.
  - `a_custom_harness_fails`: `[[test]] harness = false` exits 1.
  - `one_target_with_its_modules_passes`: `autotests = false`, `[[test]] name = "main"`, and
    `mod a; mod b;` exits 0 and prints `1 test target`.
- `taskfile_check_runs_test_layout`: `Taskfile.yml` task `check` runs
  `cargo run -q --locked -p loom-xtask -- test-layout`.
- `ess_gate::slow_gate_over_a_copy_missing_one_cover_fails_at_synthesize` is left out by the
  default profile and listed by `cargo nextest list --profile slow`.
- `cargo test -p b10x-loom-session-executor --locked` (one process, threads) passes. The
  credential case no longer calls `std::env::set_var`: `git grep -n set_var -- crates/*/tests`
  prints nothing.
- `cargo test -p b10x-loom-commission --test main --locked -- --list` lists each `ess_gate` case
  once.
- The three sibling-binary lookups and the 30 `--exact` re-runs pass under `cargo nextest run`
  and under `cargo test -p <crate> --locked`.
- `slot task ess-gate`, `slot task conform`, `slot task commission:check` and
  `slot task intake-conform` exit 0 with their rewritten `--test` lines.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and
  `slot task check` exit 0.
- AGENTS.md § Gate states the layout, what a new test file needs (`mod <file>;` in
  `tests/main.rs`), how to run one module (`cargo test -p <crate> --test main <module>::`), and
  that an expensive fixture is not a `OnceLock` under nextest.

## Out of scope

- `consumers/pinned/` (its own workspace, 3 test files, built by `task consumer-check`).
- The test budget, nextest and the slow lane: `story:test-step-within-budget`.
- The move to Rust 1.100.0: `story:rust-1-100-toolchain`. This story removes the cause of its
  three red guard cases (a sibling binary looked up by path), and that story keeps its
  Acceptance on the toolchain.
- Merging crates or moving tests between crates.

## Scope

Read at `8a8fad3b` (`loom-sdc-plan`) and in `loom-w1121g-u4` (head `4ddbdb6a` plus uncommitted
work; what is read there is marked).

- **Primary surface:** the test targets of the 30 workspace crates that have tests — cited
- **Files (manifests):** `Cargo.toml` (`[profile.dev] debug = false`, in `loom-w1121g-u4`, to
  confirm), `crates/*/Cargo.toml` of the 29 crates with more than one test file — cited
- **Files (tests):** `crates/*/tests/main.rs` (new, 29) — inferred; the module files edited:
  34 with `mod support;`, 7 cross-crate `#[path]` in `loom-agent`, 4 in `loom-sdk`,
  `loom-commission/tests/adversary2_gate.rs`, 30 with `--exact`, the 3 sibling lookups, 2
  replicas, `adversary_w20261030w3_b.rs` — cited
- **Files (xtask):** `crates/loom-xtask/src/main.rs` (`TestLayout`),
  `crates/loom-xtask/src/test_layout.rs` (new) — inferred
- **Files (gate and docs):** `Taskfile.yml` (64, 138, 143, `check`), `Taskfile.commission.yml`
  (16, 36, 41, 46), `AGENTS.md` § Gate and `:1649`, `website/docs/reference/run-events.md` (189,
  237), `website/docs/guides/run-the-checks.md` — cited
- **Files (plan):** `story:tree-run-command`, `story:bound-program-result-table` (their
  `--test` lines) — cited
- **Would collide with:** every unit that adds or edits a file under `crates/*/tests/` (all
  planned waves); `story:test-step-within-budget` (`Taskfile.yml`, `Taskfile.commission.yml`,
  `ess_gate.rs`, the replicas, `conform.rs`, `adversary2_w2_conformance_status.rs`,
  `workflow_run_store.rs`, `Cargo.toml`, `crates/loom-xtask/src/main.rs`, AGENTS.md § Gate);
  `story:local-gate-equals-forge` (`Taskfile.yml` task `check`, `loom-xtask`);
  `story:rust-1-100-toolchain` (the two guard files) — cited
