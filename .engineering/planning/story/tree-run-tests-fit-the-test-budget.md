---
format: aep.planning-md/3
id: story:tree-run-tests-fit-the-test-budget
kind: story
status: draft
title: tree_run tests fit the test budget by starting b10x-loom only where the binary is what a test is about
summary: 'budget defect: tree_run 246 s summed (CC, load 80-113), step 205 s; library calls in process, a start budget in test_step, budget keyed to survive one binary per crate; D1 b (no split)'
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:tree-run-tests-fit-the-budget
scope:
- confidence: cited
  path: .config/nextest.toml
- confidence: inferred
  path: Taskfile.yml
- confidence: cited
  path: crates/loom-cli/tests/tree_run
- confidence: cited
  path: crates/loom-cli/tests/tree_run.rs
- confidence: cited
  path: crates/loom-xtask/src/test_step.rs
revision: 7
---
## Outcome

The `tree_run` tests of `b10x-loom-cli` fit the test budget of `~/beyond10x/AGENTS.md`
§ Tests stay fast by starting fewer `b10x-loom` processes. A test starts the binary only when the
binary is what it is about: its arguments, exit codes, printed text, signals, confinement and
delegation. A test about what the library does (a status value, the lines of a run log, a
record, a stop) calls `b10x_loom_tree` in process.

On a quiet host the tests' summed time is under 60 s, and the step ends within 120 s with margin.
No limit is raised and no shard is added. Cites are at `23f1f4b3`.

This is a budget defect. It goes ahead of the next feature (§ Tests stay fast). It is the next
step that `story:tree-run-tests-fit-the-budget` named: "if the binary still misses 60 s ... the
next step is fewer trees per test".

### Measured (correction unit CC, cited, not re-timed)

- `b10x-loom-cli::tree_run` summed 246.3 s in a step of 466 tests, on a host at load 80 to 113.
  It summed 261.2 s with the shared trees made first.
- Each test starts real `b10x-loom` processes: about 2.9 s per test at that load, against about
  0.54 s on a quiet host.
- The step took 205.5 s, over its 120 s budget.
- The slowest tests:
  - three `a_status_*` at 12 s each, waiting on the shared-tree lock;
  - `nothing_in_a_run_selects_or_replaces_a_protocol` at 10.5 s;
  - `a_tree_from_a_protocols_directory_runs_with_the_directory_deleted` at 8.9 s.

### What a test starts today

- The binary holds 92 tests: 71 in `tests/tree_run.rs` and 21 in its four `#[path]` modules
  (`tree_run.rs:38-46`).
- Every start of `b10x-loom` goes through `with_the_switch` (`tree_run.rs:68-80`), called by
  `Tree::command` (`:160-172`). The helpers `open`, `run`, `decided`, `printed` and `tree`
  (`:175-270`) each start one process. `run_opened` (`:687`) starts one per printed decision
  until the tree ends.
- The library entry points are already imported (`tree_run.rs:30-33`: `run_tree`,
  `tree_status`, `status_json`, `read_status`).
- One test already proves that the printed status is the library's value:
  `the_printed_status_is_the_value_of_tree_status` (`:1003-1014`). The other `a_status_*` tests
  (`:844-1176`, `:1305`, `:1697`) each start `tree status` again to read that same value.

### The change, under D1 (b)

- **The rule.** A test keeps its process starts when it asserts on the binary's arguments, exit
  code, standard output or error text, a signal, confinement or delegation. Every other start
  becomes a library call:
  - `tree status` → `tree_status` + `status_json`;
  - a `tree run` whose printed JSON the test only reads as a value → `run_tree` with the
    test's `RunSettings` (`settings`, `:586`);
  - `tree open` → the library's open.
- **One proof per path.** Each command keeps at least one test that starts it and compares its
  output with the library's value, as `:1003` does for `status`. The unit names them in its
  report.
- **Counting.** `with_the_switch` also appends one line per start, the test's thread name, to
  the file that `LOOM_TREE_RUN_STARTS` names, when that variable is set. `task test-step` sets it
  for its nextest run and prints the summed starts beside the summed time.
- **The start budget.** `crates/loom-xtask/src/test_step.rs` gains `START_BUDGETS`, beside
  `BINARY_BUDGETS` (`:110-113`).
  - The step fails when the `tree_run` starts exceed the bound, naming the bound and the count.
  - The unit sets the bound at its after-count. Starts are a load-free proxy: they do not move
    with host load, as seconds do.
- **The budget key survives one binary per crate.** `story:one-test-binary-per-crate` (active)
  turns `tree_run.rs` into a module of `tests/main.rs`. Its tests then run in the binary
  `b10x-loom-cli::main`, under the name prefix `tree_run::`.
  - `BINARY_BUDGETS`, the new `START_BUDGETS` and the nextest override (`.config/nextest.toml`,
    the `binary_id(b10x-loom-cli::tree_run)` filter) select the tests by
    `binary_id(...) and test(/^tree_run::/)`, or by the binary alone while it is its own.
  - Either story can then merge first.

## Decisions

- **D1. How the tests fit.** Default **b**.
  - **a.** Split `tree_run` by test file into several binaries, each with its own budget. This
    goes against two folder rules. "One integration-test binary per crate" (§ Tests stay fast)
    is an active story here (`story:one-test-binary-per-crate`). And a split moves the same
    summed seconds into more budgets, which is the "adding shards" § Tests stay fast says is not
    a fix. Neither rule yields to it.
  - **b.** Cut the process starts, running the library in process where a test is about the
    library. Both rules hold.
  - **c.** Both. Same objection as (a).
- **D2. A test still over 5 s after (b).** Default **a**.
  - **a.** The unit names it with its time and its starts. If it is about the binary, its tree
    is made once per test run under `target/`, keyed by a digest, as `ended_tree` is
    (`:771-814`).
  - **b.** Move it to the `slow` lane. That does not count against the 120 s, but it is only
    allowed for a full regeneration (§ Tests stay fast), and these tests are not that.

## ESS first

No noun changes. Test code and the gate only.

## Acceptance

Bounds: no test over 60 s (20 s for `tree_run`, `.config/nextest.toml`), the test step within
120 s, no network, no model call. Fixtures are POSIX sh, never Python. A slot job ends within 10
minutes, or names its time with `slot --timeout` (at most 30 minutes). A script that only
measures runs without the `slot` prefix (§ Scripts that do not build stay off slot).

**`crates/loom-xtask/src/test_step.rs`** (its unit tests, beside `tree_run_report`, `:748`):

1. `a_module_over_its_start_budget_fails_the_step`.
   - A starts file of one line more than the bound fails the verdict, naming `tree_run` and both
     numbers. A file at the bound passes.
   - Failing first: there is no start budget.
2. `the_budgets_select_tree_run_in_its_own_binary_and_in_main`.
   - A JUnit fixture whose `tree_run` cases sum to 61 s fails the step in two layouts: as binary
     `b10x-loom-cli::tree_run`, and as `b10x-loom-cli::main` with names `tree_run::…`.
   - Failing first: the second layout is not selected today (`:341-359` match the binary id
     alone).
2b. `the_tree_run_override_holds_in_both_layouts`.
   - It reads `.config/nextest.toml` and finds one override whose `filter` is
     `binary_id(b10x-loom-cli::tree_run) or (binary_id(b10x-loom-cli::main) and
     test(/^tree_run::/))`, with `slow-timeout = { period = "20s", terminate-after = 1 }`.
   - Failing first: the filter names `binary_id(b10x-loom-cli::tree_run)` alone, so under the
     `main` layout the 20 s bound would no longer apply.

**`crates/loom-cli/tests/tree_run.rs`**:

3. `every_start_goes_through_with_the_switch`.
   - A scan of `tests/tree_run.rs` and `tests/tree_run/*.rs` for `CARGO_BIN_EXE_b10x-loom`. It
     may appear only at these three sites:
     - `Tree::command` (`tree_run.rs:160-172`);
     - `delegated_cgroup.rs`'s `BINARY` (`:19`), whose `tree_run` (`:194-219`) also calls
       `with_the_switch`;
     - the shared trees' digest (`tree_run.rs:752`), which reads the binary's bytes and starts
       nothing.
   - Failing first: none. It is a guard that the counting is complete. It catches the planted
     defect of a new `Command::new(env!("CARGO_BIN_EXE_b10x-loom"))` beside the helper.
4. `each_command_keeps_one_test_through_the_binary`.
   - These six tests keep starting the binary, and each also compares what it printed or wrote
     with the library's value on a copy of the same store:

     - `tree open`: `two_opens_with_the_same_at_write_the_same_bytes` (`:465`), against the
       bytes of a third store that `open_tree` writes with the same `at`;
     - `tree run`: `a_one_case_tree_runs_through_its_waits_to_its_outcome` (`:482`), against
       the stop, case, steps and decisions of `run_json(run_tree(..))` on a copy taken before
       each run;
     - `tree status`: `the_printed_status_is_the_value_of_tree_status` (`:1003`), against
       `status_json(tree_status(..))`, as today;
     - `tree close`: `a_case_with_two_legitimate_outcomes_waits_for_tree_close` (`:1776`),
       against the `CaseClosed` that `close` returns on a copy, as the command prints it;
     - `tree verify`: `tree_verify_names_the_case_and_line_of_a_changed_byte` (`:366`), against
       `verify_json(verify_tree(..))`;
     - `tree approve`: `an_approval_is_recorded_for_one_case_one_capability_and_its_revision`
       (`:415`), against the approvals file `approve` writes in a copy with the same `--at`.

     The library functions are the exports of `crates/loom-tree/src/lib.rs:67-83`.
   - The test scans `tests/tree_run.rs`. Each of the six functions must exist, start its command
     through `Tree::command` or a helper of it, and call its library function.
   - Failing first: today the open, run, close, verify and approve tests call no library
     function. It catches the planted defect of moving one of them in process.

**The measurement**, the second check after the proxy, in the unit's report:

- The summed starts before and after.
- The summed and wall time of the `tree_run` tests, and its ten slowest tests.
- The host's load during the measurement.
- These must show a summed time under 60 s on a host whose load is under its core count, and
  the step under 120 s.
- A test over 5 s is named with its time (D2).
- If the sum misses 60 s, the story stays open and the report names the per-test cost left.

**Gate.** `task check` exits 0.

## Out of scope

- One integration-test binary per crate (`story:one-test-binary-per-crate`).
- The sync switch (`story:tree-run-tests-fit-the-budget`, implemented in `7a8e1e17`).
- The other `loom-cli` test files.

## Scope

Derived 2026-11-22 from the plan tree at `23f1f4b3`. **Cited** means read at the line named;
**inferred** means a reading that could be wrong.

- `crates/loom-cli/tests/tree_run.rs` (`:30-46`, `:68-80`, `:160-270`, `:586`, `:687`,
  `:771-814`, `:844-1176`, `:1003-1014`, `:2721-2800`) — cited.
- `crates/loom-cli/tests/tree_run/delegated_cgroup.rs` (`:19`, `:194-219`),
  `handoff_when.rs`, `principal.rs` and `signals.rs` — cited for the starts. Which of their
  tests move in process is inferred.
- `crates/loom-xtask/src/test_step.rs` (`:110-113`, `:341-359`, `:748`) — cited.
- `.config/nextest.toml` (the `tree_run` override) — cited.
- `Taskfile.yml` (`test-step`) — inferred, for the variable.

## Edges

- `depends_on story:tree-run-tests-fit-the-budget` (active, implemented in `7a8e1e17`): its
  switch, its `BINARY_BUDGETS` and its 20 s override are what this story extends.
- `AGENTS.md`, `CHANGELOG.md` and generated files are shared with most stories. They are
  textual merges and are not listed below.
- **Would collide with**, no edge. Each entry names the shared file and the merge order:
  - `.config/nextest.toml`, `test_step.rs` and the place of `tree_run.rs`:
    `story:one-test-binary-per-crate` (active). Textual merge, no order: the budget key and the
    override filter above hold in both layouts (tests 2 and 2b).
  - `test_step.rs` and `.config/nextest.toml`: `story:test-step-within-budget` (active).
    Textual merge, no order.
  - `.config/nextest.toml` and `tests/tree_run/`: `story:concurrent-confined-runs` (active).
    Textual merge, no order.
  - `crates/loom-xtask/`: `story:drop-the-substrate-git-feature` (active). It edits
    `src/main.rs`; this story edits `src/test_step.rs`. Textual merge, no order.
  - `Taskfile.yml` (the `test-step` task, inferred):
    - active: `local-gate-equals-forge`, `planning-only-pull-requests-skip-build` and
      `decision-request-entity`;
    - draft: `docs-check-builds-once-per-fresh-tree` and `chart-upgrade-applies-crd`.

    Other tasks. Textual merge, no order.
  - `tests/tree_run.rs` or `tests/tree_run/`, every story that adds `tree_run` tests:
    - active: `child-record-in-the-run-log`, `every-printed-decision-command-runs`,
      `handoff-when-a-program-decides`, `next-decision-names-the-principal`,
      `person-action-records-a-named-result`, `person-action-runs-a-program`,
      `program-reports-its-cost`, `suite-runs-cycles-under-one-cgroup-root`,
      `tree-library-exports-what-an-engine-needs`, `tree-run-command`,
      `tree-run-takes-a-delegated-cgroup`, `tree-status-names-what-it-read` and
      `tree-suite-stop-resume-reopen-steps`;
    - proposed: `per-run-program-path` and `tree-waits-open-decision-requests`;
    - draft: `bound-program-signal-exit-is-refused` and `run-cgroup-root-holds-the-root`, each
      adding one `#[path]` module that starts the binary under Substrate, which the rule keeps.

    This story merges before them, as a budget defect ahead of the next feature. A test that
    one of them adds follows the rule above. Where one merged first, its tests are moved in
    process by this story.
  - `test_step.rs`: `story:readme-logo-and-badges` and `story:website-ux-loop` (proposed).
    Textual merge, no order.
