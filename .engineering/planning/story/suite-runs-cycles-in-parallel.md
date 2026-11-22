---
format: aep.planning-md/3
id: story:suite-runs-cycles-in-parallel
kind: story
status: proposed
title: The suite runner runs cycles in parallel under local confinement and filters them by name
summary: SuiteSettings gains workers (default 1) and only; results and observer calls in suite order; cancel starts no further cycle and a cancelled run step ends 'the suite was cancelled'; tree test gains --workers (1..64) and --only; CycleUnknown and SubstrateWorkersUnserved (workers above 1 under substrate, for story:suite-runs-cycles-under-one-cgroup-root); findings left to later
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:tree-suite-format
- depends_on: story:cancel-a-running-bound-program
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/tree.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/tree-suite
- confidence: cited
  path: crates/loom-cli/tests/tree_test/mod.rs
- confidence: cited
  path: crates/loom-tree/src/suite/mod.rs
- confidence: cited
  path: crates/loom-tree/src/suite/run.rs
- confidence: cited
  path: crates/loom-tree/tests/suite_format/mod.rs
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
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T12:27:09Z", actor: "agent:loom", revision: 9}
---
## Outcome

`run_suite` runs up to `workers` cycles at once under `--confinement local`, and runs only the
cycles a caller names. `b10x-loom tree test` gains `--workers` and `--only`. Under `--confinement
substrate`, more than one worker is refused before any cycle; the follow-up story
`story:suite-runs-cycles-under-one-cgroup-root` takes it.

```
b10x-loom tree test --suite <file> --protocols <dir> --scratch <dir> --at <instant>
                    [--confinement substrate|local] [--workers <n>]
                    [--only <name>[,<name>...]]... [--report <file>]
```

Security-relevant: bound programs of several cycles run at the same time, each cycle in its own
workspace. Under `local` they are unconfined, as every `local` program is today.

### Today

- The runner is serial. The loop at `crates/loom-tree/src/suite/run.rs:534-555` runs one cycle
  after another, and the module doc says so (`suite/mod.rs:15-17`).
- `SuiteSettings` (`run.rs:338-375`) has no worker count and no filter.
- Runbook's measurement (`docs/cycles-suite.md` at `impl/runbook-test-suite`, lines 120-133):
  - 35 cycles take 279 s wall, serially and with the build included.
  - The slowest cycle takes 29.7 s.
  - 130 entries at that rate are about 17 minutes, and the gate's test step allows 120 s.
  - Runbook filters with a temporary copy of the suite next to the original, because Loom has no
    filter (`runbook/src/test_suite.rs:110-122` on that branch).

### Workers

- `SuiteSettings` gains `workers: NonZeroUsize`. `SuiteSettings::new` sets it to 1, so every
  caller of `new` compiles unchanged, and with 1 the run is exactly today's.
- With `n`, run_suite runs the cycles on `n` threads of a `std::thread::scope`. Each thread takes
  the next cycle in suite order until none is left. A run never starts more threads than it has
  cycles to run.
- The CLI takes `--workers <n>`, from 1 to 64. Any other value is clap's usage error, exit 2.
- **Substrate.** More than one worker with `confinement` `substrate` is refused before any cycle
  with the new `SuiteRefusalReason` `SubstrateWorkersUnserved`, exit 2 (`TREE_TEST_REFUSED`).
  The detail says: `--workers above 1 under --confinement substrate waits for
  story:suite-runs-cycles-under-one-cgroup-root`.
  - `tree test` checks this before it opens the runner, so the refusal names the workers and not
    the missing root (`crates/loom-cli/src/tree.rs:460`).
  - run_suite refuses it too, before it reads anything, for an embedder whose settings say
    `confinement: Some("substrate")`.
  - The reason: Substrate keys its driver by the workspace's parent directory
    (`crates/loom-intake-slice/src/confinement.rs:450-467`). Cycle `n`'s workspace
    `<scratch>/<n>/work` has a parent of its own, so a second cycle would open a second driver
    under the root while the first cycle's launch runs, which is refused `CapabilityUnserved`
    (`:485-525`).
- **Isolation per cycle.** The layout stays `<scratch>/<n>/` with `work/` and `store/`
  (`run.rs:738-751`). `n` is the cycle's position in the suite, not the order in which it starts.
  - Each cycle's seeds go into its own workspace, and its steps open its own store.
  - Each `tree run` gets the seconds left of the cycle's own 60 s (`run.rs:1062-1070`).
  - The cycles share only what they read: the protocol catalog, read once (`run.rs:511-517`), the
    suite directory, and `settings.runner`, which is `Send + Sync`
    (`crates/loom-intake-slice/src/confinement.rs:78`).
  - Under `local` nothing is confined, as today. A program can read any path the user can, the
    other cycles' workspaces included. `local` stays the declared-insecure level.
- **Order.** The report holds the results in suite order, whatever order the cycles end in. So do
  `SuiteReport.cycles`, the cycle lines, `failed`, `pending_passing`, `waiting` and `not_run`. A
  cycle's `seconds` stays its own wall time.
- **Observer.** See D1.
- **A failing cycle does not stop the others.** A difference ends only its own cycle, as today.
- **Cancel.**
  - Once the flag is set, no thread starts another cycle.
  - A running cycle ends before its next step with the difference "the suite was cancelled"
    (`run.rs:797-805`).
  - Its running `tree run` gets the same flag (`run.rs:1071`). After
    `story:cancel-a-running-bound-program`, that run kills its program and ends with the stop
    `Cancelled`, which `stop_exit` (`run.rs:623-630`) maps to 5, as `tree run` exits 5.
  - A `run` step that ends `Cancelled` while the suite's flag is set ends the cycle with the
    difference `StepFailed`, found "the suite was cancelled", not `ExitDiffers`.
  - The report holds every cycle that started, in suite order. A cycle that never started is
    absent, as in a serial run today (`run.rs:536-538`).

### Filter

- `SuiteSettings` gains `only: Vec<String>`. `new` leaves it empty, and every cycle then runs.
- `tree test` takes `--only <name>`. The flag may repeat, and a comma separates names, so
  `--only b --only d` and `--only b,d` are one selection. A cycle name never holds a comma
  (`crates/loom-tree/src/suite/decode.rs:428-437`).
- A name that is no cycle of the suite is refused before any cycle with the new
  `SuiteRefusalReason` `CycleUnknown`, naming it. Exit 2, and nothing is written to `--scratch`.
- A cycle left out is not run and not in the report. A selected cycle keeps its suite position as
  its directory number, and its `pending` and `when` apply as in a full run.
- With `--only`, the first summary line says how many were selected, so that it is never taken
  for a full run: `cycles: selected <S> of <C>, <N> expected, <M> as expected, <K> failed:
  <names|none>`. Without `--only` the line is today's.
- `SuiteReport` gains `only`, the selected names in suite order, absent in a full run, and
  `suite_cycles`, the number of cycles in the suite.

### CLI changes, together

- New flags on `tree test`: `--workers <n>` (1..=64, default 1) and `--only <name>[,<name>...]`
  (repeatable).
- New exit-2 refusals: `CycleUnknown` and `SubstrateWorkersUnserved`.
- `TREE_TEST_HELP` (`crates/loom-cli/src/lib.rs:296-307`) says the above: cycles at once under
  `local`, the selection line and the two refusals.
- The guide's `tree test` section (`website/docs/guides/run-a-case-tree.md:361-420`) gains the two
  flags.

## Decisions

- **D1, how the observer is called.**
  - A (default): on run_suite's own thread, in suite order. Result `n` is passed as soon as
    results `1..=n` have all ended, and later results wait. `SuiteObserver` (`run.rs:326-336`)
    stays as it is and needs no `Send`. The CLI prints its lines in suite order, so the output is
    the same with any worker count.
  - B: `SuiteObserver: Send`, called from the worker threads in the order the cycles end. The
    lines then come sooner and in a changing order.

## ESS first

In `ess/domains/tree.yaml`:

- `loom.tree.SuiteRefusalReason` (`:1034-1067`) gains `CycleUnknown` and
  `SubstrateWorkersUnserved`, and its comment names both.
- `loom.tree.SuiteReport` (`:1011-1032`) gains `only: Optional<List<String>>` and
  `suite_cycles: Integer`. Its comment says that `cycles` is in suite order with any worker
  count.

`SuiteSettings` stays Rust-only data, as today. The generated Rust and JSON Schema are
regenerated from the domain, never edited by hand.

## Acceptance

Bounds:

- No test runs longer than 60 s, and the test step of the gate ends within 120 s.
- No test makes a network request or a model call.
- Bound programs and fixtures are POSIX sh, never Python.
- A build or test job in the slot pool ends within 10 minutes, or names its time with
  `slot --timeout` (at most 30 minutes).

Where the tests go:

- Every test that runs cycles goes in `crates/loom-cli/tests/tree_test/mod.rs`. That module holds
  the run harness: `local_settings` (`:330-344`), the protocols fixture (`:30-33`) and
  `fresh_scratch` (`:346-353`).
- The new suites and stub programs go below `crates/loom-cli/tests/fixtures/tree-suite`.
- Each test asserts its own time under the module's 5 s (`TEST_BOUND`, `within_bound`,
  `:42`, `:183-186`), except test 2, which is exempt below.
- The one pure test, test 13, goes in `crates/loom-tree/tests/suite_format/mod.rs`.
- The `--only` tests use cycles whose only step is `open`, as `one.suite.yaml` does, so they start
  no program.

The barrier stub: a POSIX sh program bound to every cycle of the workers tests.

- It writes the marker `<barrier>/<cycle name>`. `<barrier>` is a directory under the test's own
  `fresh_scratch`, outside `--scratch`, and the program reads its path from a seeded file the
  test writes.
- It then polls every 0.1 s, for at most 2 s, until the markers of all the cycles named in its
  seeded `expect.txt` exist.
- It writes `met` or `timed out` into `work/barrier.txt` and exits 0.
- **The overlap rule:** with as many workers as cycles, every cycle writes `met`, because each
  waits for markers that only a running cycle can write. With one worker, the first cycle writes
  `timed out`, because no other cycle has started.

Tests, all in `tree_test/mod.rs` except test 13:

1. `workers_run_cycles_at_the_same_time`: three barrier cycles, `workers: 3`. Each
   `<scratch>/<n>/work/barrier.txt` says `met`. The report lists the cycles in suite order. Under
   5 s.
2. `one_worker_runs_cycles_one_at_a_time`: the same suite with `workers: 1`. Cycle 1's
   `barrier.txt` says `timed out`.
   - It is exempt from the 5 s assertion, because it waits out the 2 s poll by design. It asserts
     the 60 s bound instead.
   - Its measured time is named in the implementation report.
3. `results_reach_the_observer_in_suite_order`: cycle 1 `slow`, cycle 2 `fast`, `workers: 2`.
   - Cycle `slow`'s program waits (bounded 3 s) for the marker that cycle `fast`'s program writes
     as its last act, then sleeps 1 s more.
   - The observer gets `slow` before `fast`, and the report equals the one of `workers: 1`
     except for `seconds`.
   - A runner that calls the observer in the order cycles end fails this test.
4. `a_failing_cycle_does_not_stop_the_others_at_once`: four cycles, `workers: 2`.
   - Cycle 1 fails at its first step (`open` of bindings that do not exist, `ExitDiffers`).
   - Cycles 2-4 each run `open`, `run` and `status` and end as expected. Cycles 3 and 4 start
     after the failure, so a runner that stops the others on a difference fails this test.
   - `failed` is `[<cycle 1>]`, and `tree test` on the same suite exits 1.
5. `each_cycle_keeps_to_its_own_directory_at_once`: three cycles, `workers: 3`.
   - Each cycle seeds `tag.txt` holding its own name. Its program copies `tag.txt` to `own.txt`
     after a sleep keyed by the name (0.9, 0.5 and 0.1 s in suite order), so the cycles end in
     reverse order.
   - For each `n`, `<scratch>/<n>/work/own.txt` holds the name of the `n`th cycle of the suite. A
     numbering by start or end order fails, and so does a write into another cycle's directory.
   - Every path under `--scratch` lies below one `<n>/`. The test's directory holds nothing
     beside `--scratch` and the barrier directory.
6. `cancel_starts_no_further_cycle`: six cycles and `workers: 2`, with run_suite on a thread of
   the test.
   - Each program writes its started marker, then waits (bounded 5 s) for a release file.
   - The test waits until two markers exist, then sets the flag, then writes the release file.
     Both workers were held at that gate when the flag was set.
   - The report holds exactly cycles 1 and 2, each with the difference found "the suite was
     cancelled". No `<scratch>/3` exists.
7. `only_runs_the_named_cycles`: `tree test --only b,d` on five `open`-only cycles.
   - Only `<scratch>/2/` and `<scratch>/4/` exist.
   - The first summary line starts `cycles: selected 2 of 5,`.
   - The report has `only: [b, d]` and `suite_cycles: 5`.
8. `only_repeated_equals_only_with_a_comma`: `--only b --only d` gives the same output and report
   as `--only b,d`.
9. `a_full_run_reports_no_only`: the report of the module's shared fixture run (`ran`, `:130`)
   has no `only` member and has `suite_cycles: 5`.
10. `only_naming_no_cycle_is_refused`: `--only nope` exits 2 (`TREE_TEST_REFUSED`) with
    `CycleUnknown` naming `nope`, and `--scratch` stays absent.
11. `workers_out_of_range_is_a_usage_error`: `--workers 0` and `--workers 65` each exit 2 before
    anything is read.
12. `workers_above_one_under_substrate_is_refused`, which needs no delegation.
    - `tree test --confinement substrate --workers 2` exits 2 with `SubstrateWorkersUnserved`,
      and the detail names `story:suite-runs-cycles-under-one-cgroup-root`. `--scratch` stays
      absent.
    - run_suite with `workers: 2` and `confinement: Some("substrate")` over a `local` runner is
      refused the same way.
    - `--confinement substrate --workers 1` keeps today's refusal of the missing root.
13. `the_selection_line_names_the_selected_cycles` (`suite_format/mod.rs`): `summary_lines`
    (`run.rs:212`) of a report with `only` set starts `cycles: selected 2 of 5,`. The line of a
    report without `only` is unchanged.

Docs: the guide and `TREE_TEST_HELP` show the two flags, the selection line and the two refusals.

## Findings left to later

These are measured by Runbook (`docs/cycles-suite.md` at `impl/runbook-test-suite`). They are
recorded here, not as stories.

- A binding's `timeout_seconds` over 3600 is refused `TimeoutOutOfRange`, and the whole file with
  it. Runbook's `bindings/sdc-bugfix.yaml` has 4200; a scratch copy with 3600 runs. The limit is
  documented (lines 212, 295-298).
- A selection on an absent file gives no value. "No finding" is therefore written `check: none`
  with a `where` (line 142, `cycles/tree-suite.yaml` cycle `offline-sync`).
- Once the allowance is spent, the fourth `change.implement` finishes with no exit. Counting the
  runs needs `where: {exit: 0}` (line 143, cycle `story-spent`).
- The format has no cycle member for a second tool that reads the finished run (lines 177,
  191-192), and none for cycles that share an opened tree (line 133).

## Out of scope

- More than one worker under `substrate`, `--cgroup-root`, and a working `--confinement substrate`
  for `tree test`: `story:suite-runs-cycles-under-one-cgroup-root`.
- The cycle kill, the ten slowest, `--list`, `decide` and `requests`: `story:tree-test-command`.
- Picking Runbook's worker count. Runbook picks it for its gate: with `W` workers a run takes about
  the larger of its slowest cycle and its total over `W`.
- A cycle that starts from another cycle's end state, and Runbook's `needs` pulling in cycles.

## Scope

Derived 2026-11-22 from the plan tree at `5bcc26d9` (`wave/2026-11-21-w7`). **Cited** means read
at the `file:line` named. **Inferred** means a reading that could be wrong.

- **ESS and generated:** `ess/domains/tree.yaml` (`:1011-1032`, `:1034-1067`),
  `generated/rust/loom`, `generated/ess/loom` — cited.
- **Suite runner:**
  - `crates/loom-tree/src/suite/run.rs`:
    - `SuiteObserver` (`:326-336`);
    - `SuiteSettings` and `new` (`:338-375`), where `new` sets `workers: 1` and an empty `only`,
      so the callers of `new` (`crates/loom-cli/src/tree.rs:464`,
      `crates/loom-cli/tests/tree_test/mod.rs:337`, Runbook's `test_suite.rs:197`) compile
      unchanged;
    - `run_suite` (`:480-556`), `SuiteRun` (`:559-566`), `cycle` (`:738-765`);
    - `summarize` (`:129`) and `summary_lines` (`:212`), which add the selection line;

    — cited.
  - `crates/loom-tree/src/suite/mod.rs`: the module doc (`:15-17`) — cited.
  - `crates/loom-intake-slice/src/confinement.rs` (`:78`, `:450-525`) is read, not changed —
    cited.
- **CLI:**
  - `crates/loom-cli/src/lib.rs`: `TREE_TEST_HELP` (`:296-307`) and `TreeTestArgs`
    (`:315-338`) — cited.
  - `crates/loom-cli/src/tree.rs`: `test` (`:444-509`) — cited.
- **Tests:**
  - `crates/loom-cli/tests/tree_test/mod.rs` and `crates/loom-tree/tests/suite_format/mod.rs` —
    cited.
  - New suites and stubs below `crates/loom-cli/tests/fixtures/tree-suite` — cited.
- **Docs:** `website/docs/guides/run-a-case-tree.md` (`:361-420`), `website/docs/reference/cli.md`
  (generated), `CHANGELOG.md` `[Unreleased]` — cited.

## Edges

- `depends_on story:tree-suite-format`, merged at `2e74232a`. This story changes its runner,
  `SuiteSettings`, the report and `tree test`.
- `depends_on story:cancel-a-running-bound-program` (active). Shared: `stop_exit`
  (`suite/run.rs:623-630`), which must map that story's new `TreeStop::Cancelled` because the
  match is exhaustive, and `ess/domains/tree.yaml`. This story's cancel rule reads that stop.
  That story's recorded scope does not list `suite/run.rs`, so its unit adds the arm.
- **Would collide with**, no edge:
  - `story:suite-cycle-writes-a-workspace-file` (draft). Shared: `suite/run.rs`,
    `suite/mod.rs`, `tree.yaml` (both edit `loom.tree.SuiteRefusalReason`, which is expected to
    merge as text), both test modules, `crates/loom-cli/tests/fixtures/tree-suite` and the guide.
    This story merges first, and that story merges its head.
  - `story:person-action-runs-a-program` (active): `suite/run.rs` (its `ApproveRequest`
    literals), `crates/loom-cli/src/tree.rs`, `lib.rs` and `tree.yaml`. Whichever merges second
    merges the other's head.
  - `story:tree-library-exports-what-an-engine-needs` (active): `crates/loom-cli/src/tree.rs`.
    This story edits `test` (`:444-509`). That story edits `host_catalog`, `tree_runner` and
    `place`. The hunks look disjoint, and a textual merge is expected. Whichever merges second
    merges the other's head.
- After it: `story:suite-runs-cycles-under-one-cgroup-root` and `story:tree-test-command` both
  depend on this story.
