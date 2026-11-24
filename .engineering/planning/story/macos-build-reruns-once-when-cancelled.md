---
format: aep.planning-md/3
id: story:macos-build-reruns-once-when-cancelled
kind: story
status: draft
title: loom / macos build is built once more when GitHub cancels it with no runner, not after a timeout
summary: 'Run 45188176292: macOS job cancelled with 0 steps and no runner; a timeout also ends cancelled, so a re-run job''s first step reads the jobs API and writes rerun=true|false; later steps run only on true; limit stays 60'
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
scope:
- confidence: inferred
  path: .github/scripts
- confidence: cited
  path: .github/workflows/check.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: inferred
  path: Taskfile.yml
revision: 8
---
## Outcome

When GitHub cancels `loom / macos build` because no runner took the job, the run builds it once
more on its own, and the workflow says so in the job's name. A macOS build that ran and hit its
limit is not re-run. Split out of `story:ci-job-time-budget` (its requirement 5 from the
coordinator's brief: "the macOS build gets one re-run on a no-runner cancel"), on the plan
critics' finding that it is its own change. No consumer asked for it; it saves a manual re-run of a
job that is not a required check. Cites are at `49499053` (tag 0.60.0); run figures from
`gh run view` and `gh api`, read 2026-11-23.

**Before.**

- `loom / macos build` (`.github/workflows/check.yml:192-216`) runs on `macos-15` with
  `timeout-minutes: 60` (`:195`; set on purpose by `69b26c55`: "on macos-15 (was macos-latest),
  with a 60 minute limit") and has no re-run.
- Run `45188176292`: the macOS job ended `cancelled` after 15.2 min with 0 steps and an empty
  runner name; its annotation reads "The job was not acquired by Runner of type hosted even after
  multiple attempts". I found one such cancel among the `check` runs of 2026-11-23, not two.
- **A job that hits `timeout-minutes` also ends `cancelled`.** In the same run, `loom / task check`
  reached its 30-minute limit and its conclusion is `cancelled` (30.3 min, 13 steps, runner
  `GitHub Actions 1000075754`). So `needs.macos.result == 'cancelled'` alone cannot tell a
  no-runner cancel from a timeout; the steps and the runner name can.
- The macOS build is not a required check (required: `loom / task check`, `Build documentation`,
  `common / Security and privacy`, `b10x-gates / local evidence`, read from
  `gh api repos/beyond10x/loom/rules/branches/main`).

**After.**

- `check.yml` gains a job `macos-rerun`, named `loom / macos build (re-run after a no-runner
  cancel)`, with `needs: macos`, `if: ${{ !cancelled() && needs.macos.result == 'cancelled' }}`,
  `runs-on: macos-15`, `timeout-minutes: 60` (as `macos`) and
  `permissions: {contents: read, actions: read}`.
- Its first step, `id: acquired`, runs a POSIX sh script (inferred path
  `.github/scripts/macos-acquired.sh`) on the JSON of this run's jobs, read through the REST API
  (`gh api repos/${{ github.repository }}/actions/runs/${{ github.run_id }}/jobs`, the token's
  `actions: read`; a read, no GitHub write). The script finds the job named `loom / macos build`
  and always exits 0:
  - no steps and an empty runner name: it prints
    `loom / macos build was not acquired by a runner: building it once more` and writes
    `rerun=true` to `$GITHUB_OUTPUT`;
  - otherwise: it prints `loom / macos build ran (<n> steps on <runner>) and is not re-run` and
    writes `rerun=false`;
  - no such job in the JSON, or JSON it cannot read: it prints why and exits 1 (the re-run job
    fails, so a broken read is seen).
- Every later step of `macos-rerun` (checkout, toolchain, fetch, build, test: the steps of `macos`)
  carries `if: steps.acquired.outputs.rerun == 'true'`, so after a timeout they are skipped and the
  job ends `success` with the second line printed.
- No comment in the workflow file: the job's name says what it is.
- The re-run job is not re-run again.

## Decisions

- **D1. Which cancels are re-run.** Default **a**.
  - a. Only a cancel with no steps and no runner name, read in the re-run job's first step with
    `actions: read`. A macOS build that ran and hit its limit is not re-run (re-running it would
    be waiting longer, which the folder rule does not count as a fix).
  - b. Every `cancelled` of the first job while the run itself is not cancelled: no API read, but a
    timed-out build runs a second time.
- **D2. How the re-run is started.** Default **a**: a second job in the same run, no GitHub write.
  **b**: a step that calls the re-run API with `actions: write`: a GitHub write outside
  `b10x-gates` (Loom's `AGENTS.md:3107-3111`), so not taken.

## Acceptance

No test binary, no Python, no network in a test. The workflow is read, not run locally.

1. `check.yml` holds the job named `loom / macos build (re-run after a no-runner cancel)` with
   `needs: macos`, the `if:` above, `permissions` `contents: read` and `actions: read` only, the
   first step `id: acquired`, and `if: steps.acquired.outputs.rerun == 'true'` on every later
   step; `macos` keeps `timeout-minutes: 60`. Read in the diff.
2. On the pull request run of this story (its first macOS job succeeds), the re-run job's
   conclusion is `skipped` (`gh run view <id> --json jobs`).
3. The script, run by `task check` with `sh` and no network (inferred: one `cmds` line in `check`
   beside `chart-check`, running a small POSIX sh test under `.github/scripts/`):
   - fed the jobs JSON of run `45188176292` saved as a fixture (`.github/scripts/fixtures/`), with
     `GITHUB_OUTPUT` set to a scratch file: exits 0, prints
     `loom / macos build was not acquired by a runner: building it once more`, and the file holds
     the line `rerun=true`;
   - fed the same JSON with the macOS job given 6 steps and the runner `GitHub Actions 1`: exits 0,
     prints `loom / macos build ran (6 steps on GitHub Actions 1) and is not re-run`, and the file
     holds `rerun=false`;
   - fed JSON with no job of that name: exits 1.

**Gate.** `task check` exits 0; the forge's checks of the pull request end green.

## Does not do

- D1 b: re-run a macOS build that ran and hit its limit.
- D2 b: re-run through the API with `actions: write`.
- Change the macOS job's limit (60 minutes, set on purpose in `69b26c55`; no ask to change it).
- Make the macOS build a required check, or change the ruleset.
- Change `loom / task check` (that is `story:ci-job-time-budget`).

## Scope

Derived 2026-11-23 from the source at `49499053` and run `45188176292`. **Cited**: read at the line
named. **Inferred**: a reading that could be wrong.

- `.github/workflows/check.yml` (`:192-216`) — cited.
- `.github/scripts/` (the script, its test and the fixture) — inferred, new.
- `Taskfile.yml` (`check`, `:20-45`) — inferred, one line.
- `AGENTS.md` (§ Gate, `:284-320`), `CHANGELOG.md` — cited files; one sentence and one bullet.

## Edges

- No `depends_on`.
- **Shared files.** `.github/workflows/check.yml` with `story:ci-job-time-budget` (its `check`
  job, `:26`) and `story:local-gate-equals-forge` (active; its toolchain step in `macos`):
  textual merge, the re-run job copies `macos`'s steps as they are when it merges. `Taskfile.yml`,
  `AGENTS.md` and `CHANGELOG.md` `[Unreleased]` with the CI budget story and the other three of the
  wave: one bullet each, each rebased on the one merged before.
