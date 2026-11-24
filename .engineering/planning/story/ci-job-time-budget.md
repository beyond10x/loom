---
format: aep.planning-md/3
id: story:ci-job-time-budget
kind: story
status: active
title: 'The forge job loom / task check ends within timeout-minutes 30 again: the test targets compiled once, the test step within 120 s'
summary: 'D146 condition: 14.7 min on 0.59.0, 33.7-42.6 on 0.60.0; measured cause compile at opt-level 1/2 repeated per cargo selection (conform tasks 3.75 to 15.4 min of compile, suites under 2 s, clippy unchanged); D1 a compile once; check.yml:26 back to 30'
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
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Taskfile.commission.yml
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/loom-xtask/src/test_step.rs
revision: 20
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T21:48:48Z", actor: "agent:sdc", revision: 18, executor: "agent:loom", correlation: "operator-d146-ci-time-budget-2026-11-23"}
- {from: "proposed", to: "active", at: "2026-11-23T21:48:48Z", actor: "agent:sdc", revision: 19, executor: "agent:loom", correlation: "operator-d146-ci-time-budget-2026-11-23"}
---
## Outcome

The forge job `loom / task check` ends inside `timeout-minutes: 30` again, with margin, on every
pull request, `main` push and release, and the test step inside it stays within its 120 s and
prints its ten slowest tests. The story first names why the job went from 14.7 min on 0.59.0 to
33.7-42.6 min on 0.60.0, then removes the cause and puts `.github/workflows/check.yml:26` back from
60 to 30. It is the condition the operator set on D146 under the folder rule "Tests stay fast" ("a
step over the budget is a defect [...] raising a timeout [...] is not a fix"). Cites are at
`49499053` (tag 0.60.0); run figures are from `gh run view <id> --repo beyond10x/loom --json jobs`
and the job logs, read 2026-11-23.

Owned elsewhere and not repeated here: the `rust-toolchain.toml` pin and the runner's cargo
variables in the local gate are the active `story:local-gate-equals-forge` (its lines 53-55 and
61-63; its environment check forbids `CARGO_PROFILE_*` in the gate), and a toolchain file is a
separate decision in `story:rust-1-100-toolchain` (its line 57). The macOS build's re-run is
`story:macos-build-reruns-once-when-cancelled`.

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

- **Not the conformance suites.** Each binary's own run is under 2 s in both releases. 0.60.0 main
  (PR): Loom's `conform` 0.58 s (0.67 s), `commission:conform` 0.08 s (0.10 s), `intake-conform`
  0.08 s; executor `ess_gate` 1.45 s (1.56 s), commission `ess_gate` 0.11 s (0.15 s),
  `executor_port` (the deps guard) 0.16 s. The 11.6 extra minutes of the three conform tasks are
  compile time.
- **Not runner load.** Clippy, which compiles without code generation, took the same 65-66 s on
  both releases; a slower runner would slow it too.
- **Compile with optimisation.** 0.60.0 added `.cargo/config.toml`: `[profile.dev] opt-level = 1`,
  every dependency and `b10x-loom-governor` at `opt-level = 2` (commits `ca9b6449` and `8b353185`,
  2026-11-22, `AGENTS.md:459-469`), to bring the test step's run under 120 s on 4 threads. Every
  step that generates code got 3 to 4 times slower: the three conform tasks 3.75 to 15.4 min,
  `consumer-check` 1.35 to 4.2, the macOS build step 1.9-2.7 to 7.1-9.3 min. The test step's
  build of its binaries alone took 466.7 s (main) and 547.2 s (PR), "not counted in the budget".
- **Built several times.** `check` (`Taskfile.yml:20-45`) starts these cargo builds of test
  targets, one after another: `ess-gate` and `commission:ess-gate` (`:26-27`), `conform`,
  `commission:conform`, `intake-conform` (`:35-37`), `commission:deps-guard` (`:38`,
  `Taskfile.commission.yml:48-51`), `test-step` (`:41`), `durable-check` (`:42`, feature
  `test-unsynced-store`) and `consumer-check` (`:43`, its own manifest and target). Inferred: each
  package selection unifies features differently, so dependencies are rebuilt per selection; at
  `opt-level = 2` that repetition is what costs. **The first task of this story measures it**: on
  a clean `target/`, the `Compiling` lines and wall time of `cargo nextest run --workspace
  --no-run` followed by each of the other builds, on the runner (one PR run printing the timings)
  and in the build pool. If the later builds compile under 10 % of the crates, the inference is
  wrong and D1 b is the default.

The test step meets its budget on the runner: "4073 test(s) in 502 binaries by nextest (59.5 s),
96 skipped, doc tests (1.6 s); wall time 61.1 s of 120 s" (main) and 68.9 s (PR), and it prints
`slowest 10 test(s):` (the slowest 5.17 s). The budget is `STEP_BUDGET`, 120 s, and `TEST_BUDGET`,
60 s (`crates/loom-xtask/src/test_step.rs:38`, `:40`).

**Before.**

- `check.yml:26` `timeout-minutes: 60` (raised from 30 by PR 166 as a diagnostic, merge `49499053`).
- `Taskfile.yml:20-45`, the builds listed above; `:68-71` `ess-gate`, `:80-85` `test-step`
  (`SLOT_TOKENS: '4'`), `:87-90` `durable-check`, `:170-178` the two Loom conform tasks,
  `:180-185` `consumer-check`; `Taskfile.commission.yml:38-46` `ess-gate` and `conform`, `:48-51`
  `deps-guard`.
- `.config/nextest.toml:7` leaves the two `ess_gate` and the three `conform` binaries out of the
  test step because `task check` runs them as their own steps. `executor_port` is not left out, so
  it already runs twice: as `commission:deps-guard` and in the test step.
- The commission `ess_gate` binary synthesizes a suite per test with no cache
  (`crates/loom-commission/tests/ess_gate.rs:138-150`, called from `gate` at `:246-251`, the test at
  `:276`): the per-test expensive step the folder rule forbids, fast today only because the
  domain is small.

**After.**

1. The first task's measurement is in the story's evidence and in the commit body, with the
   numbers above as the baseline.
2. The workspace's test targets are compiled once per job (D1 a): the three `conform` binaries run
   inside the test step's nextest run (taken out of the `default-filter` exclusion,
   `.config/nextest.toml:7`), and `check` no longer runs `conform`, `commission:conform`,
   `intake-conform` or `commission:deps-guard` as separate cargo calls (the tasks stay for a person
   to run alone; `executor_port` already runs in the step). Both `ess_gate` binaries stay out of
   the step and keep their own tasks (D2 a). `durable-check` and `consumer-check` reuse the step's
   target directory where profile and features allow (inferred: `CARGO_TARGET_DIR` for
   `consumers/pinned`).
3. The test step stays as it is: within 120 s on the runner, each test killed at 60 s, the ten
   slowest printed on every run (`test_step.rs`); the three conform binaries count inside it.
4. `check.yml:26` is `timeout-minutes: 30`, and the job ends inside it with 5 minutes of margin.
5. `AGENTS.md` § Gate (`:284-320`): the sentence "Loom's, Commission's and intake's conformance
   suites, the dependency guard," leaves the list of separate steps, and the test step's sentence
   names the three `conform` binaries and `executor_port` as part of its run; `:459-469` gains one
   sentence with the compile times measured by the first task. `CHANGELOG.md` under
   `[Unreleased]`, `### Changed`: `- The gate compiles the workspace's test targets once: the
   three conformance suites run inside the test step, and CI's loom / task check is bounded at
   30 minutes again (story:ci-job-time-budget)`.

## Decisions

- **D1. How the job gets back under 30 minutes.** Default **a**. Costs are estimates from the
  0.60.0 main run.
  - a. Compile once: the three `conform` binaries and the deps guard in the test step's build;
    `durable-check` and `consumer-check` share its target where they can. Estimate: setup 2.5 +
    spec, drift and both `ess_gate` 2 + one build 8-9 + clippy 1.1 + tests 1.1 +
    `durable-check` up to 3 + `consumer-check` up to 4.2 ≈ 20-23 min. Keeps `opt-level` and the
    test budget. Cost: a Taskfile, nextest config and `test_step.rs` change, one PR run to measure.
  - b. Workspace crates back to `opt-level = 0`, dependencies and the governor at 2: fewer
    optimised crates per selection. Saves an estimated 5-8 min; the test step loses what
    `8b353185` bought (three crates' tests 66 s to 92 s wall), so it may exceed 120 s on the runner.
    Cost: one line, a measured test-step run.
  - c. Cache `target/` between runs (`actions/cache`, keyed by `Cargo.lock`, the toolchain and
    `.cargo/config.toml`): warm PR runs compile only changed crates. Main and release runs start
    cold (36-37 min today, still over 30), the cache is many GB against the repository's limit,
    and a new third-party action is pinned by digest.
  - d. Split `task check` into parallel jobs behind one aggregator named `loom / task check` (the
    required name unchanged, the ruleset untouched): wall time about the longest job, runner
    minutes about three times today's, and the local gate no longer the forge's shape.
  - e. A larger runner: money, so the owner's decision.
- **D2. The two `ess_gate` binaries.** Default **a**: they stay out of the test step and keep their
  tasks, because the commission one synthesizes per test with no cache; they move in only after a
  story caches synthesis (queue entry below). **b**: move them in now (about 1.6 s today), against
  the folder rule's "an expensive product is made once, never per test".

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call in a test. A slot
job ends within 10 minutes, or names its time with `slot --timeout` (at most 30 minutes); a
whole-workspace build in the pool names `SLOT_TIMEOUT=30m`. Scripts that measure or wait run
without the `slot` prefix; every `cargo` inside them goes through the shim. No new Python. No test
binary is added.

1. **The cause is named.** The story's evidence holds the first task's measurement (`Compiling`
   lines and wall time per build, before and after), and it says which of compile repetition,
   optimisation level or runner load it was. Observable before any change to `check.yml`.
2. **The job fits.** `check.yml:26` reads `timeout-minutes: 30`, and on the pull request run of
   this story and the `main` run after its merge, `loom / task check` ends `success` in at most
   25 minutes, read from `gh run view <id> --json jobs`.
3. **The test step.** Those runs' logs hold `wall time <n> s of 120 s` with n ≤ 120 and the
   `slowest 10 test(s):` block. The job log holds nextest's per-test `PASS [ <seconds>s]` lines
   (the form the 0.60.0 log shows with colour codes removed, for example `PASS [   0.005s] ( 222/4073)
   b10x-loom-bindings::bindings_document a_budget_member_below_one_is_refused`) for the tests of
   `b10x-loom-conformance::conform`, `b10x-loom-commission-conformance::conform` and
   `b10x-loom-intake-conformance::conform`, each binary's lines summing to at most 5 s (measured
   today as each binary's `finished in`: 0.58-0.67 s, 0.08-0.10 s, 0.08 s), and no line of an
   `ess_gate` binary. The JUnit report (`target/nextest/default/junit.xml`, `test_step.rs:335`) is
   not uploaded by CI and is not read here. A test over 60 s still fails
   the step (`a_test_over_60_s_fails_the_step_and_the_slowest_are_printed`, `test_step.rs:880`,
   unchanged).
4. **One build of the test targets.** The `task check` log of those runs holds no `task:` line
   for `conform`, `commission:conform`, `intake-conform` or `commission:deps-guard`.
5. **Documents.** `AGENTS.md` and `CHANGELOG.md` hold the sentences and the bullet of After 5.

**Gate.** `task check` exits 0 locally; the forge's `loom / task check` ends `success` within 25
minutes (Acceptance 2), under its 30-minute limit.

## Does not do

- D1 b: workspace crates back to `opt-level = 0`. D1 c: a `target/` cache. D1 d: parallel jobs.
  D1 e: a larger runner.
- D2 b: the `ess_gate` binaries in the test step before synthesis is cached.
- The `rust-toolchain.toml` pin and the runner's cargo variables in the local gate
  (`story:local-gate-equals-forge`); a toolchain move (`story:rust-1-100-toolchain`).
- The macOS build's re-run (`story:macos-build-reruns-once-when-cancelled`) and its limit.
- Change the ruleset, the required check names or the runner size.
- Raise any timeout, add shards, or move a test to the slow lane to fit the budget.
- Lower the test step's bounds (120 s, 60 s) or its thread count.

## Scope

Derived 2026-11-23 from the source at `49499053` and runs `45045080562`, `45047758440`,
`45188176292`, `45194449550`, `45237318275`, `45240437881`, `45248142889`, `45256549760`.
**Cited**: read at the line named. **Inferred**: a reading that could be wrong.

- `.github/workflows/check.yml` (`:26`) — cited.
- `Taskfile.yml` (`:20-45`, `:68-71`, `:80-90`, `:170-185`), `Taskfile.commission.yml` (`:38-51`)
  — cited.
- `.config/nextest.toml` (`:7`) — cited. `.cargo/config.toml` — read (D1 b only), cited.
- `crates/loom-xtask/src/test_step.rs` (`:38`, `:40`, `:335`, `:880`) — cited.
- `crates/loom-commission/tests/ess_gate.rs` (`:138-150`, `:246-251`, `:276`) — read, not changed,
  cited.
- `AGENTS.md` (`:284-320`, `:459-469`), `CHANGELOG.md` — cited.

## Edges

- No `depends_on`.
- **Queue.** A story that caches the conformance synthesis the commission `ess_gate` runs per test,
  after which both `ess_gate` binaries can move into the test step (D2 a).
- **Shared files.** `AGENTS.md` and `CHANGELOG.md` `[Unreleased]` with
  `story:bound-program-timeout-above-an-hour`, `story:wildcard-result-row-does-not-match-timeout`,
  `story:program-error-carries-its-reason` and `story:spent-program-has-its-spent-record`: one
  bullet each, each rebased on the one merged before. `Taskfile.yml`, `.config/nextest.toml` and
  `.github/workflows/check.yml` with `story:local-gate-equals-forge` (active) and
  `story:macos-build-reruns-once-when-cancelled` (`check.yml`, another job): textual merge.
- Every story that adds a test binary or changes `Taskfile.yml` merges after this one or rebases
  on it.
