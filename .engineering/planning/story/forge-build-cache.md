---
format: aep.planning-md/3
id: story:forge-build-cache
kind: story
status: draft
title: loom / task check restores the locked dependencies' build from a forge cache that only a push to main writes, and ends within 25 minutes on a warm entry
summary: 'story:ci-job-time-budget D1 c as its own story; warm vs cold on EPYC 7763: test build 1182 to 585 s, clippy 82 to 35, consumer 344 to 60, restore 21 s for 1.36 GB; separate main-push job writes, check restores only; actions/cache pinned 55cc8345 (v6.1.0)'
tags:
- gate
relations:
- serves: vision:O1
- informed_by: story:ci-job-time-budget
- informed_by: story:one-test-binary-per-crate
scope:
- confidence: cited
  path: .github/workflows/check.yml
- confidence: inferred
  path: .github/workflows/deps-cache.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-xtask/src
revision: 3
---
## Outcome

The forge job `loom / task check` restores the build of Loom's locked dependencies from a GitHub
Actions cache and ends within 25 minutes, under its `timeout-minutes: 30`
(`.github/workflows/check.yml:26`), on every runner CPU model the forge gives it while the cache
entry is warm. One job of its own, run on a push to `main` only, builds the dependencies and
writes the entry; `loom / task check` only reads it, on pull requests, `main` pushes and the
release's check run alike. This is the forge cache `story:ci-job-time-budget` excluded as D1 c
("Does not do"); that story removed the repeated builds, this one removes the cold dependency
build from the job's every run. Figures are from the forge runs named below, read 2026-11-24, and
the workflow `.github/workflows/diag-check-time.yml` of the unmerged branch `diag/check-time`
(head `57e0d7c68`), which holds the exact steps.

### Why (measured on the forge)

- `loom / task check` on `main` after `story:ci-job-time-budget`: 1680 s (run `45365476832`),
  1673 s, 1751 s; PR 170 (run `45366185420`) cancelled at 1812 s, about 2207 s had it finished.
  The limit is 1800 s.
- The cold test build is 776 to 1019 s of it (46 %). The runners have five CPU models (the diag
  logs print AMD EPYC 7763, AMD EPYC 9V45, Intel Xeon 6975P-C, Xeon Platinum 8573C and 8370C);
  the same dependency build costs 1565 to 2379 CPU-seconds on them, so a run's time depends on
  the runner it draws.
- Cache of the locked dependencies, warm against cold on an AMD EPYC 7763: test build 1182 to
  585 s, clippy 82 to 35 s, consumer build (`consumers/pinned`, own target) 344 to 60 s; restore
  21 s for 1.36 GB. Estimated job: about 18 min. The warm figures are run `45392527898`, job
  `138439691144` (log: `Cache Size: ~1358 MB (1424469415 B)`, `DIAG test-build seconds 585`,
  `DIAG clippy … seconds 35`, `DIAG consumer-own … seconds 60`); its cold run `45386884832` drew
  an EPYC 9V45 (job `138420007408`: test build 842 s, clippy 69 s, consumer 244 s). Cold on an
  EPYC 7763 elsewhere: consumer 344 s (job `138420007577`). The 1182 s cold test build is the
  diagnosis's figure; its run is not named here.
- `story:one-test-binary-per-crate` alone (the fold): 24.0 min on a fast runner, 31.5 min on a
  slow one, over the limit. Fold and cache together: about 12.7 min. The cache is what gives the
  margin on a slow runner.

**Before.** `check.yml` has no cache step; every run of `loom / task check` fetches the registry
and builds every dependency cold (`check.yml:23-121`). No workflow of the repository uses
`actions/cache`. `release.yml` runs `check.yml` on the tagged commit by `workflow_call`
(`release.yml` job `check`). The guard `the_task_check_job_is_bounded_at_30_minutes`
(`crates/loom-xtask/src/test_step.rs:1491-1501`) holds the limit; release_check's
`every_action_of_the_release_and_images_workflows_is_pinned_by_commit`
(`crates/loom-xtask/tests/release_check.rs:998`) holds digests for two workflows only.

**After.**

1. A workflow of its own (D2 a; inferred name `.github/workflows/deps-cache.yml`, job
   `loom / dependency cache`), `on: push: branches: [main]` and nothing else, `permissions:
   contents: read`, `timeout-minutes: 45`, the same toolchain, `env:` and installs as the
   `loom / task check` job. It looks the exact key up first (`lookup-only: true`) and ends on a
   hit. On a miss it restores by the key's prefix (`restore-keys`), runs the builds `task check`
   runs without running tests (the test build `cargo test --workspace --locked --no-run`, clippy
   `--workspace --all-targets`, the `durable-check` selections, the `consumer-check` build),
   removes every workspace artifact, then saves (D1 a).
2. The entry holds `~/.cargo/registry`, `~/.cargo/git`, `target/` and `consumers/pinned/target/`,
   each `target` with every workspace artifact removed before the save (the diag workflow's step
   `Keep only the dependencies' build`: `.fingerprint`, `build` and `deps` entries of
   `b10x-loom-*`, `loom-*`, `b10x_loom*`, `loom_*`, every executable of `deps`, `incremental`,
   `target/nextest`, `target/tmp`, `target/cargo-timings`, `target/release`).
   `consumers/pinned/target/` is in because it was measured useful (344 to 60 s).
3. The key: a fixed prefix, the runner's OS, the toolchain the job installs (1.99.1), and
   `hashFiles('Cargo.lock', '.cargo/config.toml', 'consumers/pinned/Cargo.toml')`; the
   `restore-keys` prefix is everything before the hash (D4 a).
4. `loom / task check` gains one step `Restore the dependency build`
   (`actions/cache/restore@55cc8345863c7cc4c66a329aec7e433d2d1c52a9`, same paths and key, the
   same `restore-keys`) before `cargo fetch --locked`, and one step that prints the runner's CPU
   model (`lscpu`, `Model name`) so a run's time can be read against its runner. It has no save
   step: no `actions/cache/save` and no combined `actions/cache` anywhere in `check.yml`.
5. A guard of `loom-xtask` holds 1-4 (Acceptance 1).
6. Documents (Acceptance 6).

## Decisions

- **D1. What the entry holds.** Default **a**.
  - a. Registry, git checkouts, `target/` and `consumers/pinned/target/` with every workspace
    artifact removed before the save. Reason: the workspace's crates change on every commit, so
    their artifacts in a shared entry are rebuilt anyway and only add size and trusted bytes;
    1.36 GB measured against the repository's 10 GB.
  - b. `target/` whole. Larger, stale on the next commit, and the workspace's own build becomes
    cached content a later run trusts.
  - c. Registry and git only: saves the fetch, not the 1182 s build.
- **D2. Who writes the shared entry.** Default **a**.
  - a. A separate job on push to `main`, with its own timeout, builds and saves; `loom / task
    check` restores only. Reason: a cold `loom / task check` runs 28 to 37 min and can be
    cancelled at 30, and a cancelled job saves nothing, so the cache must not depend on it.
  - b. `loom / task check` saves on a `main` push: no entry exactly when the job ran cold and was
    cancelled.
  - c. Pull requests save too: GitHub scopes a pull request's entry to `refs/pull/<n>/merge`,
    which only re-runs of that pull request read. Storage and save time for nothing.
- **D3. The action.** Default **a**.
  - a. The first-party `actions/cache/restore` and `actions/cache/save`, restore and save as
    separate steps, pinned by the commit digest the diag workflow uses,
    `55cc8345863c7cc4c66a329aec7e433d2d1c52a9`. Verified 2026-11-24 with `gh api
    repos/actions/cache/commits/55cc834…` and `git/matching-refs/tags`: it is the commit of the
    tags `v6` and `v6.1.0` of `actions/cache` (the merge of its PR 1768, "Bump @actions/cache to
    v6.1.0 - handle read-only cache access", 2026-06-23).
  - b. A third-party Rust cache action: another owner's code in the job, and it saves in a post
    step of the job that restores, which D2 a rules out.
  - c. sccache on the forge with the Actions cache as its store: a compile cache of the
    repository's own, which `~/beyond10x/AGENTS.md` § One compile cache forbids.
- **D4. The key.** Default **a**: lock file, `.cargo/config.toml`, `consumers/pinned/Cargo.toml`,
  toolchain and OS in the key, and a `restore-keys` prefix, so a lock change restores the last
  entry and rebuilds only the dependencies that changed. **b**: exact key only; every lock change
  runs cold. The `env:` of the jobs (`CARGO_INCREMENTAL`, `CARGO_PROFILE_*_DEBUG`) is not in the
  key: both jobs set the same values, and a difference costs a rebuild, not a wrong artifact
  (cargo's fingerprint holds the profile).
- **D5. A second lock on the read side.** Default **a**: no `cache-mode` key; the absence of a
  save step in `check.yml` is the mechanism and the guard holds it. **b**: also `cache-mode:
  read` on the job (documented on GitHub's dependency-caching page, untried in this repository;
  an invalid workflow turns the required check red). Left for a later story.

## Trust

- A pull request restores the entry and never writes the shared one: `check.yml` has no save
  step (D2 a), and GitHub writes a pull request's entries only to its merge ref's scope, which
  nothing else reads. Only a push to `main` saves, which is code that already merged through the
  ruleset.
- Cited, GitHub Docs, "Dependency caching reference"
  (<https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching>,
  § Restrictions for accessing a cache, read 2026-11-24): "Workflow runs can restore caches
  created in either the current branch or the default branch (usually `main`)"; a pull request
  run "can also restore caches created in the base branch"; a pull request's cache is "created
  for the merge ref (`refs/pull/.../merge`)" and "can only be restored by re-runs of the pull
  request"; "Workflow runs also cannot restore caches created for different tag names"; § Usage
  limits and eviction policy: "GitHub will remove any cache entries that have not been accessed
  in over 7 days", 10 GB per repository by default, evicted oldest access first. § Cache access
  for low-trust workflow triggers lists `push` among the triggers that may write the default
  branch's scope.
- A tag push (`release.yml` runs `check.yml` on the tagged commit) runs at the tag's ref and so
  restores from the default branch's scope. Inferred from the first quote, which names no
  exception for tags; Acceptance 4 is where the forge confirms it.
- What stays trusted unverified: cargo checks a cached artifact's rustc, profile, features and
  source fingerprint, not its content. A poisoned entry needs a writer at `main`'s scope, that
  is a merged commit or the repository's own workflow; that is the trust the code itself already
  has, so it is acceptable here. A release's binary and images are never built from the cache
  (`binaries.yml`, `images.yml` unchanged); the cache only speeds up the check that gates them.
- No secret reaches the cache steps: both jobs hold `contents: read` only, check out with
  `persist-credentials: false`, and name no `secrets.*`.

## Cold case

The first run after an eviction (7 days without a read), a lock change, a `.cargo/config.toml`
change or a toolchain change runs cold: about 28 to 37 min, as today. This story does not claim
to fix it. The `restore-keys` prefix limits a lock change to the dependencies that changed; the
warm-entry job runs on the same `main` push, so the next run (or a re-run of a cancelled one) is
warm.

## One compile cache

`~/beyond10x/AGENTS.md` § One compile cache stays intact: no sccache on the forge, no
`SCCACHE_*`, `RUSTC_WRAPPER` and `CARGO_BUILD_RUSTC_WRAPPER` untouched in every workflow and never
emptied, and the local gate unchanged. "The local gate equals the forge" does not cover a cache
that only the forge has: it changes how long a step takes, not what it builds or checks.

## ESS first

No declaration under `ess/` changes; a forge cache is not part of what Loom runs. Exempt as a
change with no behaviour change (AGENTS.md § ESS).

## Acceptance

Bounds for the implementor: every build or test on this host through the pool,
`slot --timeout 30m <command>` (a `cargo` call through the shim: `SLOT_TIMEOUT=30m cargo …`); a
job in the pool ends within 10 minutes unless it names its time, at most 30. Tests stay fast: no
test over 60 s, the test step within 120 s, no network, no model call in a test; scripts that
measure or wait run without the `slot` prefix. No new Python. No test binary is added: the guard
is unit tests in `loom-xtask`'s existing target. The forge is the verifier of 2 to 5.

1. **The guard.** A guard of `loom-xtask` (inferred: `crates/loom-xtask/src/forge_cache.rs`, after
   the form of `the_task_check_job_is_bounded_at_30_minutes`, reading the workflows with
   `serde_yaml_ng`) reads every file under `.github/workflows/` and fails, naming the file and
   the step:
   - when a workflow saves the cache outside a push to `main`: an `actions/cache/save` step in a
     workflow whose `on:` is anything but `push` with `branches: [main]`, or any combined
     `actions/cache@` (it saves in its post step) anywhere, `check.yml` included;
   - when an `actions/cache` use (`actions/cache`, `/restore`, `/save`) is not pinned by a 40-hex
     digest;
   - when `RUSTC_WRAPPER`, `CARGO_BUILD_RUSTC_WRAPPER` or `SCCACHE_` appears in a workflow;
   - when `timeout-minutes` of `loom / task check` is above 30 (the existing test keeps holding
     30 exactly).
   Each path has a passing and a failing case on inline YAML, each under 5 s, plus one case over
   the tree's own workflows that passes. The implementor plants each defect in the tree, shows
   the guard red, removes it, shows it green, and names the four runs in the report.
2. **The job fits.** On forge runs with a warm entry, `loom / task check` ends `success` within
   25 minutes on every runner CPU model seen. The implementor reads at least 2 warm runs (the pull
   request of this story after the entry exists, and the `main` run after its merge, or more) with
   `gh run view <id> --json jobs`, and names each run's job time and the CPU model its log prints.
3. **The test step.** Those runs' logs hold `wall time <n> s of 120 s` with n ≤ 120 and the
   `slowest 10 test(s):` block.
4. **The release restores.** The `release / check` run of the first tag after the merge logs
   `Cache restored from key: <key>` in `Restore the dependency build`. The story is not closed
   before that run is read.
5. **A pull request does not save.** The pull request run of this story logs a restore line in
   `Restore the dependency build` and holds no save step and no `Cache saved` line.
6. **Documents.** `AGENTS.md` § Gate, at the sentence "CI runs exactly this (`check.yml`, job
   `loom / task check`, …)" (`:306-307`) and at the paragraph on CI's runner (`:482-486`), says
   what the cache holds, that the separate job writes it on a push to `main` only, that
   `loom / task check` restores only, the cold case, and the one line on "The local gate equals
   the forge". `CHANGELOG.md` under `## [Unreleased]`, `### Changed` (0.61.0 is released, tag
   `0.61.0`): `- CI's loom / task check restores the locked dependencies' build from a cache that
   only a push to main writes (story:forge-build-cache)`.

**Gate.** `task check` exits 0 locally; the forge's `loom / task check` ends `success` within 25
minutes on a warm entry (Acceptance 2), under its 30-minute limit.

## Does not do

- One test binary per crate: `story:one-test-binary-per-crate`, next, its own story.
- `durable-check` building only `--lib` (diag e1); `consumer-check` in the workspace's target
  (diag e2).
- Change `opt-level` or any profile setting.
- Raise any limit, add a retry or a re-run, or skip or move a test.
- Fix the cold case (above); keep the entry warm with a schedule.
- A cache in `loom / macos build`, `loom / docs slow lane`, `pages.yml`, `binaries.yml` or
  `images.yml`; a cache of the task, ess, helm or nextest downloads.
- The `cache-mode` key (D5 b).
- sccache, `SCCACHE_*` or a change of `RUSTC_WRAPPER` in any workflow or in the local gate.
- Change the ruleset, the required check names or the runner size.

## Scope

Derived 2026-11-24 from `origin/main` and `origin/diag/check-time` (`57e0d7c68`), runs
`45365476832`, `45366185420`, `45386884832`, `45392527898`. **Cited**: read at the line named.
**Inferred**: a reading that could be wrong.

- `.github/workflows/check.yml` (`:23-121`, job `check`; `:26` unchanged) — cited.
- The warm-entry workflow, new (`.github/workflows/deps-cache.yml`) — inferred name.
- `crates/loom-xtask/src/` (the guard, a new module and its `mod` line in `main.rs`; the existing
  `test_step.rs:1491-1501`) — inferred file, cited surface.
- `AGENTS.md` (`:306-307`, `:482-486`), `CHANGELOG.md` (`:13`) — cited.
- `.github/workflows/release.yml` (job `check`) — read, not changed, cited.

## Edges

- No `depends_on`. `informed_by` `story:ci-job-time-budget` (this is its D1 c) and
  `story:one-test-binary-per-crate` (the fold, measured with it; not a dependency: each story
  shortens the job alone, the cache is what gives the margin).
- **Shared files.** `.github/workflows/check.yml` with `story:planning-only-pull-requests-skip-build`
  (active: its classification step comes first, and a skipped run should skip the restore too,
  inferred), `story:local-gate-equals-forge` (active), `story:macos-build-reruns-once-when-cancelled`
  (draft, another job) and `story:rust-1-100-toolchain` (draft: its toolchain move changes the
  key, so its first run is cold): textual merge. `AGENTS.md` § Gate and `CHANGELOG.md`
  `[Unreleased]` with `story:one-test-binary-per-crate`: one bullet each, rebased on the one
  merged before. `crates/loom-xtask/src/main.rs` with `story:one-test-binary-per-crate`
  (`TestLayout`): one `mod` line each.
