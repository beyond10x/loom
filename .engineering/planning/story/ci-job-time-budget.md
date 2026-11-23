---
format: aep.planning-md/3
id: story:ci-job-time-budget
kind: story
status: draft
title: The forge job loom / task check ends within timeout-minutes 30 again, the workspace compiled once, the test step within 120 s, the local gate reading what the runner reads
summary: 'D146 condition: 14.7 min on 0.59.0, 33.7-42.6 on 0.60.0; measured cause compile at opt-level 1/2 repeated per cargo selection (conform tasks 3.75 to 15.4 min of compile, suites under 2 s, clippy unchanged); D1 a compile once; check.yml:26 back to 30; rust-toolchain.toml; one macOS re-run after a no-runner cancel'
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
scope:
- confidence: cited
  path: .config/nextest.toml
- confidence: cited
  path: .github/workflows/check.yml
- confidence: cited
  path: .github/workflows/pages.yml
- confidence: cited
  path: .github/workflows/release.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Taskfile.commission.yml
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/loom-xtask/src/test_step.rs
- confidence: inferred
  path: rust-toolchain.toml
revision: 11
---
## Outcome

The forge job `loom / task check` ends inside `timeout-minutes: 30` again, with margin, on every
pull request, `main` push and release, and the test step inside it stays within its 120 s and
prints its ten slowest tests. The story first names why the job went from 14.7 min on 0.59.0 to
33.7-42.6 min on 0.60.0, then removes the cause, puts `.github/workflows/check.yml:26` back from
60 to 30, makes the local gate read what the runner reads, and gives the macOS build one re-run
after a cancel that never got a runner. It is the condition the operator set on D146 under the
folder rule "Tests stay fast" ("a step over the budget is a defect [...] raising a timeout [...] is
not a fix"). Cites are at `49499053` (tag 0.60.0); run figures are from
`gh run view <id> --repo beyond10x/loom --json jobs` and the job logs, read 2026-11-23.

### Why the job went from 14.7 to 33.7+ min (measured)

The job's duration, `task check` step's duration, and the time from one `task: [x]` log line to
the next, in minutes:

| | 0.59.0 main `45047758440` | 0.60.0 main `45248142889` | PR `45240437881` |
|---|---|---|---|
| job / `task check` step | 14.7 / 13.3 | 36.3 / 33.7 | 42.6 / 39.7 |
| `conform` (Loom) | 1.95 | 7.8 | 9.25 |
| `commission:conform` | 0.75 | 3.2 | 3.6 |
| `intake-conform` | 1.05 | 4.4 | 5.2 |
| clippy | 1.1 | 1.1 | 1.2 |
| tests (`cargo test --workspace` / `test-step`) | 6.3 | 8.8 | 10.3 |
| `durable-check` (new in 0.60.0) | none | 3.05 | 3.7 |
| `consumer-check` | 1.35 | 4.2 | 5.0 |

The other 0.60.0 runs: `45188176292` cancelled at 30.3 min (the 30-minute limit); `45194449550`
failed after 38.4 min (task check step 36.9 min, the limit already 60); `45237318275` cancelled
after 17.7 min (superseded); release `45256549760` 37.0 min (step 34.2). 0.59.0 PR `45045080562`:
18.5 min.

Reading:

- **Not the conformance suites.** Each suite's own run is under 2 s in both releases (`test result:
  ok. 3 passed [...] finished in 0.58s`, `1 passed [...] 0.08s`, `2 passed [...] 0.08s` on 0.60.0
  main). The 11.6 extra minutes of the three conform tasks are compile time.
- **Not runner load.** Clippy, which compiles without code generation, took the same 65-66 s on
  both releases; a slower runner would slow it too.
- **Compile with optimisation.** 0.60.0 added `.cargo/config.toml`: `[profile.dev] opt-level = 1`,
  every dependency and `b10x-loom-governor` at `opt-level = 2` (commits `ca9b6449` and `8b353185`,
  2026-11-22, `AGENTS.md:459-469`), to bring the test step's run under 120 s on 4 threads. Every
  step that generates code got 3 to 4 times slower: the three conform tasks 3.75 to 15.4 min,
  `consumer-check` 1.35 to 4.2, the macOS build step 1.9-2.7 to 7.1-9.3 min. The test step's
  build of its binaries alone took 466.7 s (main) and 547.2 s (PR), "not counted in the budget".
- **Built several times.** Each `cargo test -p <crate> --test conform` and the test step build
  their own graph: 0.60.0 main compiles in `conform`, `commission:conform`, `intake-conform`,
  `test-step`, `durable-check` (feature `test-unsynced-store`) and `consumer-check` (its own
  manifest and target). Inferred: each package selection unifies features differently, so
  dependencies are rebuilt per selection; at `opt-level = 2` that repetition is what costs.
  **The first task of this story measures it**: on a clean `target/`, the number of `Compiling`
  lines and the wall time of `cargo nextest run --workspace --no-run` followed by each of the
  three `cargo test -p ... --test conform` and `durable-check`, on the runner (one PR run with
  `CARGO_LOG` off and the timings printed) and in the build pool. If the second and later
  builds compile under 10 % of the crates, the inference is wrong and D1 b is the default.

The test step itself meets its budget on the runner: "4073 test(s) in 502 binaries by nextest
(59.5 s), 96 skipped, doc tests (1.6 s); wall time 61.1 s of 120 s" (main) and 68.9 s (PR), and it
prints `slowest 10 test(s):` (the slowest 5.17 s). The budget is `STEP_BUDGET`, 120 s, and
`TEST_BUDGET`, 60 s (`crates/loom-xtask/src/test_step.rs:38`, `:40`).

**Before.**

- `check.yml:26` `timeout-minutes: 60` (raised from 30 by PR 166 as a diagnostic, merge `49499053`);
  `check.yml:195` macOS `timeout-minutes: 60`.
- `Taskfile.yml:20-45`: `check` runs `conform`, `commission:conform`, `intake-conform` as their own
  cargo builds (`:170-178`, `Taskfile.commission.yml:43-46`), then clippy, `test-step` (`:80-85`,
  `SLOT_TOKENS: '4'`), `durable-check` (`:87-90`), `consumer-check` (`:180-185`).
- `.config/nextest.toml:7` leaves the two `ess_gate` and the three `conform` binaries out of the
  test step because `task check` runs them as their own steps.
- No `rust-toolchain.toml`: every workflow names `toolchain: 1.99.1` (`check.yml:37`, `:139`, `:205`,
  `release.yml:31`, `pages.yml:32`); `AGENTS.md:292` says the local gate runs on that version, and
  `AGENTS.md:475-477` still says "There is no `rust-toolchain.toml`: CI builds on `stable`", which
  is stale. The runner's environment (`check.yml:16-20`: `CARGO_INCREMENTAL=0`,
  `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`) is not in the local gate.
- `loom / macos build` (`check.yml:192-216`) has no re-run. Run `45188176292`: the job was
  cancelled after 15.2 min with 0 steps and no runner name; its annotation reads "The job was not
  acquired by Runner of type hosted even after multiple attempts". The job is not a required
  check (required: `loom / task check`, `Build documentation`, `common / Security and privacy`,
  `b10x-gates / local evidence`).

**After.**

1. The first task's measurement is in the story's evidence and in the commit body, with the
   numbers above as the baseline.
2. The workspace is compiled once per job for the test profile (D1 a): the two `ess_gate` and three
   `conform` binaries run inside the test step's nextest run (removed from the `default-filter`
   exclusion, `.config/nextest.toml:7`), and `check` no longer runs them as separate cargo calls
   (the tasks stay for a person to run alone). `durable-check` and `consumer-check` reuse the same
   target directory where the profile and features allow (inferred: `CARGO_TARGET_DIR` for
   `consumers/pinned`).
3. The test step stays as it is: within 120 s on the runner, each test killed at 60 s, the ten
   slowest printed on every run (`test_step.rs`); conformance binaries count inside it.
4. `check.yml:26` is `timeout-minutes: 30`, and the job ends inside it with margin (Acceptance 2).
5. `rust-toolchain.toml` pins 1.99.1; every workflow and the local gate read it; `AGENTS.md:475-477`
   is corrected. The runner's three cargo variables are set for the local gate too
   (`Taskfile.yml` `env`, or `.cargo/config.toml` where they have a key) so a local `task check`
   compiles what the forge compiles.
6. A second macOS job, `loom / macos build (re-run after a no-runner cancel)`, `needs: macos`, runs
   only when the first ended `cancelled` and the run itself was not cancelled
   (`if: ${{ !cancelled() && needs.macos.result == 'cancelled' }}`), with the same steps. Its name
   says what it is (no comments in workflow files). A second cancel fails the run.

## Decisions

- **D1. How the job gets back under 30 minutes.** Default **a**. Costs are estimates from the
  0.60.0 main run.
  - a. Compile once: the five gate binaries run in the test step's build; `durable-check` and
    `consumer-check` share its target where they can. Estimate: setup 2.5 + spec and drift 1.5 +
    one build 8-9 + clippy 1.1 + tests 1.1 + `durable-check` up to 3 + `consumer-check` up to 4.2
    ≈ 19-22 min. Keeps `opt-level` and the test budget. Cost: a Taskfile, nextest and `test_step.rs`
    change, one PR run to measure; the conformance runs then share the 120 s (they take under 3 s).
  - b. Workspace crates back to `opt-level = 0`, dependencies and the governor at 2: fewer
    optimised crates to build per selection. Saves an estimated 5-8 min; the test step loses what
    `8b353185` bought (3 crates' tests 66 s to 92 s wall), so it may exceed 120 s on the runner.
    Cost: one line, a measured test-step run.
  - c. Cache `target/` between runs (`actions/cache`, keyed by `Cargo.lock`, the toolchain and
    `.cargo/config.toml`): warm PR runs compile only changed crates. Main and release runs start
    cold (36-37 min today, still over 30), the cache is many GB against the repository's limit, and
    a new third-party action is pinned by digest. Cost: high upkeep, cold path unsolved.
  - d. Split `task check` into parallel jobs behind one aggregator named `loom / task check`
    (the required name unchanged, the ruleset untouched): wall time about the longest job, runner
    minutes about three times today's, and the local gate no longer the forge's shape.
  - e. A larger runner: money, so Timo's decision, not a default.
- **D2. The macOS re-run.** Default **a**: a second job on `needs.macos.result == 'cancelled'`,
  no GitHub write from the workflow. **b**: a step that calls the re-run API with `actions: write`:
  a GitHub write outside `b10x-gates` (Loom's `AGENTS.md:3107-3111`), so not taken.
- **D3. The macOS job's own limit.** Default **a**: 30 minutes, like `check` (its build is 7-9 min).
  **b**: 60 as today.
- **D4. The toolchain pin.** Default **a**: `rust-toolchain.toml` with `channel = "1.99.1"` and
  the components the gate uses, read by every workflow and the local gate. **b**: keep the
  version in each workflow and check it against `rustc --version` locally.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call in a test. A slot
job ends within 10 minutes, or names its time with `slot --timeout` (at most 30 minutes); a
whole-workspace build in the pool names `SLOT_TIMEOUT=30m`. Scripts that measure or wait run
without the `slot` prefix; every `cargo` inside them goes through the shim. No new Python. No test
binary is added.

1. **The cause is named.** The story's evidence holds the first task's measurement (Compiling
   lines and wall time per build, before and after), and it says which of compile repetition,
   optimisation level or runner load it was. Observable before any change to `check.yml`.
2. **The job fits.** `check.yml:26` reads `timeout-minutes: 30`, and on the pull request run of
   this story and the `main` run after its merge, `loom / task check` ends `success` in at most
   25 minutes (5 minutes of margin), read from `gh run view <id> --json jobs`.
3. **The test step.** Those runs' logs hold `wall time <n> s of 120 s` with n ≤ 120 and the
   `slowest 10 test(s):` block, and the step's test count includes the `conform` and `ess_gate`
   tests (D1 a). A test over 60 s still fails the step
   (`a_test_over_60_s_fails_the_step_and_the_slowest_are_printed`, `test_step.rs:880`, unchanged).
4. **Local equals forge.** `rust-toolchain.toml` exists and pins 1.99.1; no workflow names a
   toolchain version of its own; `rustc --version` in a local `task check` prints 1.99.1; the
   three cargo variables of `check.yml:16-20` are in the local gate's environment.
5. **macOS.** `check.yml` holds the job named `loom / macos build (re-run after a no-runner
   cancel)` with `needs: macos` and the `if` above; on a run whose first macOS job succeeds it is
   `skipped`.
6. `AGENTS.md` (§ Gate, `:284-320`, and `:459-477`) and `CHANGELOG.md` say what changed. No
   ruleset, required check name or branch protection is changed.

**Gate.** `task check` exits 0 locally; the forge's `loom / task check` ends `success` within 30
minutes.

## Does not do

- Change the ruleset, the required check names or the runner size (D1 e).
- Raise any timeout, add shards, or move a test to the slow lane to fit the budget.
- Lower the test step's bounds (120 s, 60 s) or its thread count.

## Scope

Derived 2026-11-23 from the source at `49499053` and runs `45045080562`, `45047758440`,
`45188176292`, `45194449550`, `45237318275`, `45240437881`, `45248142889`, `45256549760`.
**Cited**: read at the line named. **Inferred**: a reading that could be wrong.

- `.github/workflows/check.yml` (`:16-20`, `:26`, `:37`, `:139`, `:192-216`) — cited.
- `.github/workflows/release.yml` (`:31`), `.github/workflows/pages.yml` (`:32`) — cited.
- `Taskfile.yml` (`:20-45`, `:80-90`, `:170-185`), `Taskfile.commission.yml` (`:43-46`) — cited.
- `.config/nextest.toml` (`:7`) — cited. `.cargo/config.toml` — cited (read, D1 b only).
- `crates/loom-xtask/src/test_step.rs` (`:38`, `:40`, `:880`) — cited.
- `rust-toolchain.toml` — new file, inferred.
- `AGENTS.md` (`:284-320`, `:459-477`), `CHANGELOG.md` — cited.

## Edges

- No `depends_on`.
- **Would collide with**, no edge: every story that adds a test binary or changes `Taskfile.yml`
  or `.config/nextest.toml`; textual merge, this story's change first in a wave.
