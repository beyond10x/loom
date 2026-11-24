---
format: aep.planning-md/3
id: story:forge-build-cache
kind: story
status: draft
title: loom / task check restores the locked dependencies' build from a forge cache that only a push to main writes, and ends within 25 minutes on a warm entry
summary: story:ci-job-time-budget D1 c, carrying its Acceptance 2 and Gate on warm runs; separate main-push job (40 min, own concurrency group) writes, check restores only; key ubuntu-24.04 + rustc cachekey + hashFiles(rust-toolchain.toml, Cargo.lock, .cargo/config.toml, **/Cargo.toml); actions/cache pinned 55cc8345 (v6.1.0); loom-xtask guard over the workflows
tags:
- gate
relations:
- serves: vision:O1
- informed_by: story:ci-job-time-budget
- informed_by: story:one-test-binary-per-crate
- blocks: story:local-gate-equals-forge
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
  path: crates/loom-xtask/src/forge_cache.rs
- confidence: cited
  path: crates/loom-xtask/src/main.rs
revision: 9
---
## Outcome

The forge job `loom / task check` restores the build of Loom's locked dependencies from a GitHub
Actions cache and ends within 25 minutes, under its `timeout-minutes: 30`
(`.github/workflows/check.yml:26`), on the warm runs read (Acceptance 2). One job of its own, run
on a push to `main` only, builds the dependencies and writes the entry; `loom / task check` only
reads it, on pull requests, `main` pushes and the release's check run alike. This is the forge
cache `story:ci-job-time-budget` excluded as D1 c ("Does not do"); that story removed the
repeated builds, this one removes the cold dependency build from the job's warm runs. Figures are
from the forge runs named below, read 2026-11-24, and the workflow
`.github/workflows/diag-check-time.yml` of the unmerged branch `diag/check-time` (head
`57e0d7c68`), which holds the exact steps.

**Carried forward.** This story carries forward `story:ci-job-time-budget`'s Acceptance 2 ("on the
pull request run of this story and the `main` run after its merge, `loom / task check` ends
`success` in at most 25 minutes") and its Gate ("the forge's `loom / task check` ends `success`
within 25 minutes"). Its cold runs end in 28 to 37 min, so that outcome is verified here, on warm
runs: this story's Acceptance 2 is the record that closes that story's Acceptance 2 and Gate.

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

**Before** (`origin/main` `0c454abed`). `check.yml` has no cache step; every run of
`loom / task check` fetches the registry and builds every dependency cold (`check.yml:23-121`).
No workflow of the repository uses `actions/cache`. `check.yml:25` runs on `ubuntu-latest`. There
is no `rust-toolchain.toml`; `check.yml` installs the literal `toolchain: 1.99.1` and sets
`CARGO_INCREMENTAL`, `CARGO_PROFILE_DEV_DEBUG` and `CARGO_PROFILE_TEST_DEBUG` in its `env:`
(`:16-20`); `.cargo/config.toml` holds `[profile.dev] debug = false` and the opt-levels.
`release.yml` runs `check.yml` on the tagged commit by `workflow_call` (job `check`). The guard
`the_task_check_job_is_bounded_at_30_minutes` (`crates/loom-xtask/src/test_step.rs:1491-1501`)
holds the limit; release_check's `every_action_of_the_release_and_images_workflows_is_pinned_by_commit`
(`crates/loom-xtask/tests/release_check.rs:998`) holds digests for two workflows only.

**After.**

1. A workflow of its own (D2 a; `.github/workflows/deps-cache.yml`, job
   `loom / dependency cache`), `on: push: branches: [main]` and nothing else, `permissions:
   contents: read`, `timeout-minutes: 40` (D8), `runs-on: ubuntu-24.04`, and its own
   `concurrency:` group, one per workflow and not per ref (`group: deps-cache`), with
   `cancel-in-progress: false` (D6 a). It installs the toolchain from the same source `check.yml`
   uses at the time, in a step with `id: rust` (D7). It looks the exact key up first
   (`lookup-only: true`) and ends on a hit. On a miss it restores by the `restore-keys` prefix,
   runs the builds `task check` runs without running tests (the test build `cargo test
   --workspace --locked --no-run`, clippy `--workspace --all-targets`, the `durable-check`
   selections, the `consumer-check` build), removes every workspace artifact, then saves under the
   key the restore step resolved (`steps.restore.outputs.cache-primary-key`, an output of
   `actions/cache/restore` at the pinned commit), not a key computed again after the builds
   (D1 a). Its 40 minutes are the limit of a new job; `loom / task check` keeps its 30.
2. The entry holds `~/.cargo/registry`, `~/.cargo/git`, `target/` and `consumers/pinned/target/`,
   each `target` with every workspace artifact removed before the save (the diag workflow's step
   `Keep only the dependencies' build`: `.fingerprint`, `build` and `deps` entries of
   `b10x-loom-*`, `loom-*`, `b10x_loom*`, `loom_*`, every executable of `deps`, `incremental`,
   `target/nextest`, `target/tmp`, `target/cargo-timings`, `target/release`).
   `consumers/pinned/target/` is in because it was measured useful (344 to 60 s).
3. The key (D4 a): `loom-deps-ubuntu-24.04-${{ steps.rust.outputs.cachekey }}-${{
   hashFiles('rust-toolchain.toml', 'Cargo.lock', '.cargo/config.toml', '**/Cargo.toml') }}`.
   The key holds the image name, the rustc hash and the hashed files; `restore-keys` holds the
   image name and the rustc hash (`loom-deps-ubuntu-24.04-${{ steps.rust.outputs.cachekey }}-`).
   `steps.rust.outputs.cachekey` is the output of the `dtolnay/rust-toolchain` step (`id: rust`),
   "A short hash of the rustc version, appropriate for use as a cache key" (its `action.yml` at
   the pinned commit `4360b52568e2003a75bf9bc1d59f33a8e3fc893c`, read 2026-11-24). On
   `origin/main` `rust-toolchain.toml` does not exist and `hashFiles` skips it; `Cargo.lock`,
   `.cargo/config.toml` and the manifests exist (`**/Cargo.toml` holds `consumers/pinned/Cargo.toml`).
   No version literal and no `env:` value is in the key.
4. `loom / task check` runs on `ubuntu-24.04` (`check.yml:25` moves from `ubuntu-latest` in this
   story unless `story:local-gate-equals-forge` moved it first; same value). Its
   `dtolnay/rust-toolchain` step gets `id: rust`; without it the key loses the rustc hash and never
   equals `deps-cache.yml`'s. It gains one step `Restore the dependency build`
   (`actions/cache/restore@55cc8345863c7cc4c66a329aec7e433d2d1c52a9`, same paths, key and
   `restore-keys`), after the `id: rust` step and before `cargo fetch --locked`, and one step that
   prints the runner's CPU model (`lscpu`, `Model name`), which Acceptance 2 reads. Both steps carry
   `if: steps.changes.outputs.class != 'skip'`, the condition `story:planning-only-pull-requests-skip-build`
   gives the install and fetch steps, so a planning-only run skips both and that story's guard
   `both_jobs_classify_first_and_skip_by_class` (every step after `changes` has an `if:` naming
   `steps.changes.outputs.class`) holds. Before that story lands there is no `changes` step: the
   output is empty, the condition is true and both steps run, so the `if:` is written in either
   landing order. It has no save step: no `actions/cache/save` and no combined `actions/cache`
   anywhere in `check.yml`. Step order: `changes` (where it has landed) first, then the toolchain
   step `id: rust`, then the restore step, then `cargo fetch --locked`.
5. A guard of `loom-xtask` holds 1 to 4 as Acceptance 1 lists its cases; the 30-minute limit stays
   held by `the_task_check_job_is_bounded_at_30_minutes`, unchanged.
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
- **D4. The key.** Default **a**: the pinned image name, the rustc hash of the install step and
  the hashed files (`rust-toolchain.toml` where it exists, `Cargo.lock`, `.cargo/config.toml`,
  `**/Cargo.toml`), and a `restore-keys` prefix of the image name and the rustc hash, so a lock or
  manifest change restores the last entry of the same rustc and image and rebuilds only the
  dependencies that changed. Every `Cargo.toml` is hashed, not only the lock: a feature change
  that leaves `Cargo.lock` unchanged changes which dependency artifacts the build needs, and with
  the lock alone the exact key would hit a stale entry that `deps-cache.yml` never refreshes. A
  version bump (every crate's `Cargo.toml` and `Cargo.lock`) changes the key once; the
  push-to-`main` run of `deps-cache.yml` on that commit writes the new entry. Reason: files and the installed rustc hold in either landing order with
  `story:local-gate-equals-forge` (D7), where a version literal or an `env:` value would not.
  The build settings `check.yml`'s `env:` sets today (`CARGO_INCREMENTAL=0`,
  `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`) are not copied into `deps-cache.yml`
  and not in the key: inferred, they change no dependency artifact, since incremental compilation
  applies to workspace crates only, which the entry does not hold, and `.cargo/config.toml`
  already sets `debug = false`, which the test profile inherits. Once
  `story:local-gate-equals-forge` moves them into `.cargo/config.toml`, they are in the key through
  that file. Acceptance 2's exact-key runs show it holds (a mismatch shows as a test build near the
  cold time). **b**: exact key only; every lock change runs cold.
- **D5. A second lock on the read side.** Default **a**: no `cache-mode` key; the absence of a
  save step in `check.yml` is the mechanism and the guard holds it. **b**: also `cache-mode:
  read` on the job (documented on GitHub's dependency-caching page, untried in this repository;
  an invalid workflow turns the required check red). Left for a later story.
- **D6. Concurrency of the warm-entry job.** Default **a**: one group per workflow
  (`group: deps-cache`), `cancel-in-progress: false`. Reason: the next push to `main` must not
  cancel a cold build before it saves, the failure D2 a exists to prevent; a run that waits behind
  it finds the exact key and ends on the lookup. **b**: per ref with cancellation, as `check.yml`
  does: a quick second merge cancels the only run that would have saved.
- **D7. The toolchain of `deps-cache.yml`.** Default **a**: the same source as `check.yml` at the
  time it is written. If `story:local-gate-equals-forge` has landed: its step `id: toolchain`
  reading `rust-toolchain.toml`, and `dtolnay/rust-toolchain` with `toolchain: ${{
  steps.toolchain.outputs.channel }}`. If not: the same `dtolnay/rust-toolchain` step `check.yml`
  uses, the one place this story repeats the version. Whichever of the two stories lands second
  rewrites `deps-cache.yml`'s toolchain step to the `id: toolchain` form. That duty is recorded
  here only: `story:local-gate-equals-forge`'s scope and body do not list `deps-cache.yml`
  (inferred: its forge-parity `workflows` check, which requires the `id: toolchain` step in every
  job that installs Rust, would go red on the file and show it). Either way the dtolnay step
  carries `id: rust` for the key. **b**: wait for `story:local-gate-equals-forge`: the release
  would wait on a story that is not merged.
- **D8. The limit of `loom / dependency cache`.** **40 minutes**, derived from the measured cold
  build of the writer's steps on the slowest runner model seen (Xeon Platinum 8370C, diag job
  `138385715022`, step times from `gh api …/actions/jobs/<id>`): setup, checkout and installs 19 s,
  `cargo fetch` 76 s, test build 1172 s, clippy 82 s, the two durable-check steps 123 + 144 s (an
  upper bound for the writer's durable-check build), consumer build in its own target 344 s; save
  13 s for 1.36 GB (job `138420007408`). Sum 1973 s (32.9 min); times 1.2, rounded up to a whole
  5 minutes: 40. A limit of at most 30 is not supported: the measured cold sum on that model is
  over 30 minutes, so the writer would be killed cold there and write no entry, the failure D2 a
  exists to prevent. It is the limit of a new job and raises no existing one; `loom / task check`
  keeps `timeout-minutes: 30`.

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
  has, so it is acceptable here. The image name in the key keeps build-script output made against
  one image's system libraries from being restored on another: a moved `ubuntu-latest` would do
  that unseen. A release's binary and images are never built from the cache (`binaries.yml`,
  `images.yml` unchanged); the cache only speeds up the check that gates them.
- No secret reaches the cache steps: both jobs hold `contents: read` only, check out with
  `persist-credentials: false`, and name no `secrets.*`.

## Cold case

The first run after an eviction (7 days without a read), a lock change, a `.cargo/config.toml`
change, a change to any `Cargo.toml`, a `rust-toolchain.toml` or rustc change, or an image
change runs cold or from the prefix: about 28 to 37 min,
as today. This story does not claim to fix it. The `restore-keys` prefix limits a lock change to
the dependencies that changed; the warm-entry job runs on the same `main` push, so a run that
starts after it has saved (a re-run of a cancelled one included) is warm.

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

1. **The guard.** A guard of `loom-xtask` (`crates/loom-xtask/src/forge_cache.rs`, after the form
   of `the_task_check_job_is_bounded_at_30_minutes`, reading the workflows with `serde_yaml_ng`)
   reads every file under `.github/workflows/` and fails, naming the file and the job or step:
   - when a workflow saves the cache outside a push to `main`: an `actions/cache/save` step in a
     workflow whose `on:` is anything but `push` with `branches: [main]`, or any combined
     `actions/cache@` (it saves in its post step) anywhere, `check.yml` included;
   - when an `actions/cache` use (`actions/cache`, `/restore`, `/save`) is not pinned by a 40-hex
     digest;
   - when `RUSTC_WRAPPER`, `CARGO_BUILD_RUSTC_WRAPPER` or `SCCACHE_` appears in a workflow
     (source: `~/beyond10x/AGENTS.md` § One compile cache; the forge has no wrapper, and the
     folder rule binds every repository);
   - when job `check` of `check.yml` has no step `Restore the dependency build` with the exact key
     of After 3, or that step does not sit after the step `id: rust` and before `cargo fetch
     --locked`;
   - when the restore step or the `lscpu` step of job `check` has no `if:` naming
     `steps.changes.outputs.class`;
   - when a job reads `steps.rust.outputs` and its `dtolnay/rust-toolchain` step has no `id: rust`;
   - when `deps-cache.yml` lacks `permissions: contents: read`, `timeout-minutes: 40`, or a
     workflow-level `concurrency:` group with `cancel-in-progress: false`;
   - when the key of either workflow does not hold the job's pinned `runs-on` image name
     (`ubuntu-24.04`), the rustc hash (`steps.rust.outputs.cachekey`) and the hashed files of
     After 3 (`rust-toolchain.toml`, `Cargo.lock`, `.cargo/config.toml`, `**/Cargo.toml`); when
     its `restore-keys` does not hold the image name and the rustc hash; or when the two
     workflows' keys differ.
   The 30-minute limit of `loom / task check` is not a case here: the existing
   `the_task_check_job_is_bounded_at_30_minutes` (`test_step.rs:1491-1501`) holds it, unchanged.
   Each case has a passing and a failing input on inline YAML, each under 5 s, plus one case over
   the tree's own workflows that passes. For each planted defect in the tree the implementor's
   report names the command, the failing test, and the green run after restoring the file.
2. **The job fits, warm.** A run counts as warm when its step `Restore the dependency build` logs
   `Cache restored from key: <key>` with `<key>` equal to the step's primary key (not a
   `restore-keys` prefix hit, not `Cache not found`). Warm runs are those of `loom / task check`
   that start after a `loom / dependency cache` run of the same key ended `success`: a re-run, a
   later `main` run, or a later pull request. This story's own pull request run and the `main` run
   started by its merge are cold (only a push to `main` writes the entry, and that run starts
   beside `deps-cache.yml`) and do not count. On at least 2 warm runs, read with `gh run view <id>
   --json jobs` and the job log, `loom / task check` ends `success` in at most 25 minutes. The
   report names each run's id, job time and the CPU model its `lscpu` step prints. The claim holds
   for the runs read: the report lists the runner CPU models seen so far (AMD EPYC 7763, AMD EPYC
   9V45, Intel Xeon 6975P-C, Xeon Platinum 8573C, Xeon Platinum 8370C), and names each one no warm
   run read covers as not covered. This item closes `story:ci-job-time-budget`'s Acceptance 2 and
   Gate (Outcome, Carried forward).
3. **The test step.** Those warm runs' logs hold `wall time <n> s of 120 s` with n ≤ 120 and the
   `slowest 10 test(s):` block.
4. **The release restores.** The `release / check` run of the first tag after the merge logs
   `Cache restored from key: <key>` in `Restore the dependency build`. The story is not closed
   before that run is read.
5. **A pull request does not save.** A pull request run's `Restore the dependency build` logs
   either `Cache not found for input keys: <key>, <prefix>` (a miss) or `Cache restored from key:
   <key>` (a hit), and in either case the job holds no save step and its log no `Cache saved`
   line. The report names one pull request run of each kind it saw, at least one.
6. **Documents.** `AGENTS.md` § Gate gains a sub-heading `### The forge's dependency cache` of its
   own: what the cache holds, that `loom / dependency cache` writes it on a push to `main` only,
   that `loom / task check` restores only, the key, the cold case, and the one line on "The local
   gate equals the forge"; the sentence of § Gate item 1 "CI runs exactly this (`check.yml`, job
   `loom / task check`, …)" points to it. `CHANGELOG.md` gets one bullet under `### Changed`:
   `- CI's loom / task check restores the locked dependencies' build from a cache that only a push
   to main writes (story:forge-build-cache)`. Where it goes: under `## [0.61.0]` (`CHANGELOG.md`,
   the dated section; `Cargo.toml` `version = "0.61.0"`) if the story merges before the tag
   `0.61.0` exists on the forge, else under `## [Unreleased]`. The deciding check is `git
   ls-remote --tags origin 0.61.0` (empty: not tagged), never a local tag. The story creates the
   `### Changed` header where the section lacks it.

**Gate.** `task check` exits 0 locally; the forge's `loom / task check` ends `success` within 25
minutes on warm runs (Acceptance 2), under its 30-minute limit.

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
- `rust-toolchain.toml`, moving build settings into `.cargo/config.toml`, or a forge-parity check:
  `story:local-gate-equals-forge`.
- Change the ruleset, the required check names or the runner size.

## Scope

Derived 2026-11-24 from `origin/main` (`0c454abed`) and `origin/diag/check-time` (`57e0d7c68`),
runs `45365476832`, `45366185420`, `45386884832`, `45392527898`. **Cited**: read at the line or
heading named. **Inferred**: a reading that could be wrong.

- `.github/workflows/check.yml` (job `check`: `runs-on`, `id: rust`, the restore and `lscpu`
  steps; the `timeout-minutes` line unchanged) — cited.
- `.github/workflows/deps-cache.yml` (new) — inferred name.
- `crates/loom-xtask/src/forge_cache.rs` (new) — inferred file.
- `crates/loom-xtask/src/main.rs` (one `mod forge_cache;` line in the `mod` block, `:60-72`) —
  cited. `test_step.rs`'s 30-minute test is read, not changed.
- `AGENTS.md` (§ Gate, new sub-heading `### The forge's dependency cache`), `CHANGELOG.md` (the
  section Acceptance 6 names) — cited.
- `.github/workflows/release.yml` (job `check`) — read, not changed, cited.

## Edges

- No `depends_on`. `informed_by` `story:ci-job-time-budget` (this is its D1 c, and this story
  carries forward its Acceptance 2 and Gate) and `story:one-test-binary-per-crate` (the fold,
  measured with it; not a dependency: each story shortens the job alone, the cache is what gives
  the margin).
- **Ordering with `story:local-gate-equals-forge`** (active): this story lands first, because it
  is the release blocker. The edge is `blocks: story:local-gate-equals-forge` on this story (the
  relation vocabulary lists `blocks`, "Prevents progress on something"). Its reason is the new
  file `.github/workflows/deps-cache.yml`: `story:local-gate-equals-forge`, landing second, has
  its `workflows` check read it and rewrites its toolchain step to the `id: toolchain` form (D7).
  That duty is recorded here only; that story's scope and body are its own and are not edited
  here, and they do not list `deps-cache.yml`. Whether its `forge-parity` check `workflows`
  iterates `.github/workflows/` or a fixed list is not written there: its Outcome names the four
  jobs that install Rust today in three files (`check.yml` `check` and `macos`, `pages.yml`
  `build`, `release.yml` `verify`) and states the check as "every `dtolnay/rust-toolchain` step
  reads the `toolchain` step's output and every job with one has that step", naming no file set
  (cited, `local-gate-equals-forge.md` § Outcome); that it would reach `deps-cache.yml` only if
  it iterates the directory is inferred. When it lands, `check.yml`'s `env:` build settings move
  into `.cargo/config.toml` (its change) and the key changes through `rust-toolchain.toml` and
  `.cargo/config.toml` (one cold run).
- **Shared files, textual merge.**
  - `.github/workflows/check.yml`: `story:local-gate-equals-forge` (active; `runs-on`, the
    toolchain steps, `env:`), `story:planning-only-pull-requests-skip-build` (active; its
    `changes` step comes first, and the restore and `lscpu` steps carry its `if:` on
    `steps.changes.outputs.class`, After 4), `story:macos-build-reruns-once-when-cancelled`
    (draft; another job), `story:release-binaries-macos` (proposed; a job beside `macos`) and
    `story:rust-1-100-toolchain` (draft; its toolchain move changes the key's rustc hash, so its
    first run is cold).
  - `crates/loom-xtask/src/main.rs`: `story:local-gate-equals-forge` adds `forge_parity` and
    `forge_watch` `mod` and subcommand lines in the same alphabetical `mod` block,
    `story:one-test-binary-per-crate` edits the file too (`TestLayout`), and so does
    `story:readme-logo-and-badges` (proposed; `main.rs` in its typed scope,
    `readme-logo-and-badges.md:29`). One `mod` line each; the guard module stays
    `forge_cache.rs`.
  - `AGENTS.md` § Gate: `story:local-gate-equals-forge` (§ Gate's two toolchain paragraphs and its
    table), `story:planning-only-pull-requests-skip-build`, `story:macos-build-reruns-once-when-cancelled`,
    `story:one-test-binary-per-crate` and `story:readme-logo-and-badges` (each lists `AGENTS.md`
    in its typed scope). The cache paragraph sits under its own sub-heading
    `### The forge's dependency cache`, so the hunks do not overlap.
  - `CHANGELOG.md`: the stories whose typed scope lists it, `story:macos-build-reruns-once-when-cancelled`
    and `story:readme-logo-and-badges`; one bullet each, rebased on the one merged before, and
    each story creates the `### Changed` header where its section lacks it.
- **Stale-scope neighbours.** `aep plan artifact waves` reports a collision with each of these
  five active stories on the file named. In each case their change to that file is already on
  `origin/main` (the commit named, an ancestor of `origin/main` `0c454abed`, cited), so the
  collision comes from a scope never closed, and this story holds no hunk of theirs to merge
  with; that no further change of theirs to the file is pending is inferred from the commits.
  - `story:test-step-within-budget`: `check.yml` and `main.rs`, both in `8c3607883` ("gate: the
    test step runs under nextest …", 2026-11-22) — cited.
  - `story:docs-check-builds-once-per-fresh-tree`: `check.yml` (cited) and `main.rs` (inferred in
    `waves`), both in `d395ddc61`, merged by `7483cdb29` — cited.
  - `story:tree-run-binary-fits-its-start-bound`: `main.rs`, merged by `7d50e45a1` — cited.
  - `story:drop-the-substrate-git-feature`: `main.rs` (`48b68f99e`, cited) and `forge_cache.rs`
    (inferred: its typed scope is the directory `crates/loom-xtask/`, which holds the new file;
    it never wrote it).
  - `story:ci-job-time-budget`: `check.yml` in `127d8fbca` ("ci: compile the gate's test targets
    once; loom / task check back to 30 min"), merged by `efe8839b3` — cited. It is a `check.yml`
    neighbour as well as `informed_by`; its open Acceptance 2 and Gate are carried here, not a
    pending change to the file.
