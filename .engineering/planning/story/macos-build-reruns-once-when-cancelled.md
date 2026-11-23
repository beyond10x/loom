---
format: aep.planning-md/3
id: story:macos-build-reruns-once-when-cancelled
kind: story
status: draft
title: loom / macos build is built once more when GitHub cancels it with no runner, not after a timeout, and its limit is 30 minutes
summary: 'Split from story:ci-job-time-budget: a second job needs: macos, runs on a cancelled first job and builds only when that job has no steps and no runner name (read with actions: read; a timeout also ends cancelled, run 45188176292); macOS timeout-minutes 60 to 30; no GitHub write, no workflow comment'
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
revision: 6
---
## Outcome

When GitHub cancels `loom / macos build` because no runner took the job, the run builds it once
more on its own, and the workflow says so in the job's name. A macOS build that ran and hit its
limit is not re-run. The macOS job's limit goes from 60 to 30 minutes. Split out of
`story:ci-job-time-budget` (its requirement 5 from the coordinator's brief: "the macOS build gets
one re-run on a no-runner cancel"), on the plan critics' finding that it is its own change. No
consumer asked for it; it saves a manual re-run of a job that is not a required check. Cites are at
`49499053` (tag 0.60.0); run figures from `gh run view` and `gh api`, read 2026-11-23.

**Before.**

- `loom / macos build` (`.github/workflows/check.yml:192-216`) runs on `macos-15` with
  `timeout-minutes: 60` (`:195`) and has no re-run.
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
- The macOS build's own time: 4.1-6.1 min on 0.59.0, 9.1-11.8 min on 0.60.0 (job durations of
  runs `45045080562`, `44912018605`, `45240437881`, `45248142889`).

**After.**

- `check.yml` gains a job `macos-rerun`, named `loom / macos build (re-run after a no-runner
  cancel)`, with `needs: macos`, `if: ${{ !cancelled() && needs.macos.result == 'cancelled' }}`,
  `runs-on: macos-15`, `timeout-minutes: 30` and `permissions: {contents: read, actions: read}`.
  Its first step reads the first job of this run through the REST API (`gh api
  repos/${{ github.repository }}/actions/runs/${{ github.run_id }}/jobs`, the token's
  `actions: read`; a read, no GitHub write) and goes on only when that job has no steps and no
  runner name (D1 a); otherwise it prints that the first job ran and was not re-run, and the
  remaining steps are skipped. Then the same steps as `macos`.
- `macos`'s `timeout-minutes` is 30 (D2 a).
- No comment in the workflow file: the job's name says what it is.
- A second cancel of the re-run fails nothing further and is not re-run again.

## Decisions

- **D1. Which cancels are re-run.** Default **a**.
  - a. Only a cancel with no steps and no runner name, read in the re-run job's first step with
    `actions: read`. A macOS build that ran and hit its limit is not re-run (re-running it would
    be waiting longer, which the folder rule does not count as a fix).
  - b. Every `cancelled` of the first job while the run itself is not cancelled: no API read, but a
    timed-out build runs a second 30 minutes.
- **D2. The macOS job's limit.** Default **a**: 30 minutes, like `loom / task check`; its build is
  9-12 min on 0.60.0. **b**: 60 as today.
- **D3. How the re-run is started.** Default **a**: a second job in the same run, no GitHub write.
  **b**: a step that calls the re-run API with `actions: write`: a GitHub write outside
  `b10x-gates` (Loom's `AGENTS.md:3107-3111`), so not taken.

## Acceptance

No test binary, no Python, no network in a test. The workflow is read, not run locally.

1. `check.yml` holds the job named `loom / macos build (re-run after a no-runner cancel)` with
   `needs: macos`, the `if:` above, `permissions` `contents: read` and `actions: read` only, and the
   first step that reads the jobs of the run; `macos` and `macos-rerun` both have
   `timeout-minutes: 30`. Read in the diff.
2. On the pull request run of this story (its first macOS job succeeds), the re-run job's
   conclusion is `skipped` (`gh run view <id> --json jobs`).
3. The first-step logic, as a POSIX sh script under `.github/scripts/` (inferred path) fed the
   JSON of run `45188176292`'s jobs saved as a fixture, prints that the job was not acquired and
   exits 0; fed the same JSON with the macOS job given steps and a runner name, it prints that the
   job ran and exits with the code that skips the build. Run with `sh` by `task check` (inferred:
   one `cmds` line beside `chart-check`), no network.

**Gate.** `task check` exits 0; the forge's checks of the pull request end green.

## Does not do

- D1 b: re-run a macOS build that ran and hit its limit.
- D2 b: keep 60 minutes.
- D3 b: re-run through the API with `actions: write`.
- Make the macOS build a required check, or change the ruleset.
- Change `loom / task check` (that is `story:ci-job-time-budget`).

## Scope

Derived 2026-11-23 from the source at `49499053` and runs `45188176292`, `45045080562`,
`44912018605`, `45240437881`, `45248142889`. **Cited**: read at the line named. **Inferred**: a
reading that could be wrong.

- `.github/workflows/check.yml` (`:192-216`) — cited.
- `.github/scripts/` (the first-step script and its fixture) — inferred, new.
- `Taskfile.yml` (`check`, `:20-45`) — inferred, one line.
- `AGENTS.md` (§ Gate, `:284-320`), `CHANGELOG.md` — cited files; one sentence and one bullet.

## Edges

- No `depends_on`.
- **Shared files.** `.github/workflows/check.yml` with `story:ci-job-time-budget` (its `check`
  job, `:26`) and `story:local-gate-equals-forge` (active; its toolchain step in `macos`):
  textual merge, the re-run job copies `macos`'s steps as they are when it merges. `Taskfile.yml`,
  `AGENTS.md` and `CHANGELOG.md` `[Unreleased]` with the CI budget story and the other three of the
  wave: one bullet each, each rebased on the one merged before.
