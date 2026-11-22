---
format: aep.planning-md/3
id: story:planning-only-pull-requests-skip-build
kind: story
status: active
title: A pull request that changes only planning or prose files passes Loom's required checks with no build and no test, and one under website/ runs only the docs checks
summary: GATE_PATHS in check.yml (skip/docs/full, first match, default full); .github/changed-paths.sh over git diff -z --no-renames HEAD^1; both jobs skip steps by class
tags:
- gate
relations:
- serves: vision:O1
scope:
- confidence: inferred
  path: .github/changed-paths.sh
- confidence: cited
  path: .github/workflows/check.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/loom-xtask/Cargo.toml
- confidence: inferred
  path: crates/loom-xtask/tests/changed_paths.rs
- confidence: inferred
  path: website/docs/guides/run-the-checks.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T22:00:14Z", actor: "agent:host-operator", revision: 6, executor: "agent:loom", correlation: "loom-gate-rules-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T22:00:15Z", actor: "agent:host-operator", revision: 7, executor: "agent:loom", correlation: "loom-gate-rules-2026-11-21"}
---
## Outcome

A pull request whose changed files are all planning or prose files passes `loom / task check`
and `loom / macos build` without building or testing: both jobs run, classify the change, and
end green with every build and test step skipped. A change under `website/` runs the docs checks
and skips the code's build and test. Any other change runs the full gate, as today. The ruleset
and the names of the required checks do not change.

- One path list, in `.github/workflows/check.yml`: the workflow-level `env:` member `GATE_PATHS`,
  one line per rule, `<class> <pattern>`, first match wins. Classes: `skip` (planning or prose),
  `docs` (the built docs site), `full`. A pattern is a shell pattern over the whole path, `*`
  matching `/` too. A path no rule matches is `full`. The list, in order:
  - `docs website/*`
  - `full` for every directory the build or a test reads: `crates/*`, `consumers/*`, `ess/*`,
    `generated/*`, `protocols/*`, `examples/*`, `deploy/*`, `charts/*`, `.github/*`
  - `full` for each `.md` file outside those that a test or a gate step reads: `AGENTS.md`,
    `README.md`, `CHANGELOG.md`, `docs/commission/contracts/frontier.md`,
    `docs/commission/contracts/commission-executor.md`,
    `docs/contracts/loom-action-selection.md`, `docs/design/loom-agent-crd-deployment.md`
  - `skip .engineering/*`, then `skip *.md`
- `.github/changed-paths.sh <event>` holds the logic and no list. For an event other than
  `pull_request` it prints `class=full` and reads nothing. For `pull_request` it reads
  NUL-separated paths on standard input, prints each path and its class to standard error, and
  prints `class=<c>` on standard output: `full` when any path is `full` or no path was read,
  else `docs` when any path is `docs`, else `skip`.
- Each of the two jobs has, right after `actions/checkout`, a step `id: changes` that runs
  `git diff -z --name-only --no-renames HEAD^1 HEAD | .github/changed-paths.sh
  "$GITHUB_EVENT_NAME" >> "$GITHUB_OUTPUT"` under `set -euo pipefail`. On a pull request the
  checkout is the merge commit, so `HEAD^1` is the base and the diff is the pull request's whole
  effect, also after the branch was updated from `main`. `check` already fetches the history
  (`fetch-depth: 0`, `check.yml:31-32`); `macos` checks out one commit (`check.yml:100-102`) and
  gains `fetch-depth: 2`.
- In `loom / task check`, the steps that install Rust, Task and `ess` and fetch the dependencies
  run when the class is not `skip`; bubblewrap, Helm, cargo-nextest and `task check` run when it
  is `full`; a step `Docs checks` runs `task docs-check` and `task commission:docs-drift` when it
  is `docs`. In `loom / macos build` every step after `changes` runs when the class is `full`. A
  last step in each job prints the class and, for `skip`, `planning or prose only: no build, no
  test`.
- `push` to `main` and the call from `release.yml` (`workflow_call`, whose event is the tag's
  `push`) always run the full gate.
- `task plan` runs, after `aep plan artifact validate`, the one test that reads the planning
  store: `cargo test -p b10x-loom-executor --test adversary_governor_import --locked`.

## Why

`~/beyond10x/AGENTS.md` § Planning-only pull requests skip the tests: substrate, 2026-11-21, 4 of
7 merged pull requests changed only `.engineering/` or documents and each ran the full gate; ess
pull request 803, one planning file, ran three full rounds of 628 to 672 runner-minutes. Loom's
`check.yml:3-7` starts both jobs on every pull request and every step runs
(`check.yml:29-91`, `:99-117`); `task check` is the whole gate (`Taskfile.yml:15-39`).

What the code allows, against the folder rule's "`*.md` outside a built docs site": Loom's tests
and gate read `.md` files outside `website/`, so those run the full gate:
`crates/loom-docs/tests/adv_q13_agent_stack_layers.rs:48` (`README.md`, `AGENTS.md`),
`crates/loom-commission/tests/ess_gate.rs:595` (`AGENTS.md`), `crates/loom-xtask/src/release.rs:70`
and `crates/loom-executor/tests/adversary_w20261021w5_u2.rs:97` (`CHANGELOG.md`),
`crates/loom-conformance/tests/conform.rs:638` (`ess/SKIPPED.md`),
`crates/loom-commission/tests/adversary_admission_contract.rs:86`,
`crates/loom-commission-testkit/tests/adversary2_executor_contract_doc.rs:23`,
`crates/loom-executor/tests/adversary_w20261021w5_u2.rs:93`,
`crates/loom-operator/tests/adversary_w20261105w1_u4_render.rs:97` (the four `docs/` files). One
test reads `.engineering/planning/`:
`the_governor_epic_and_its_story_are_re_filed_citing_their_old_ids`
(`crates/loom-executor/tests/adversary_governor_import.rs:59-60`, `:102`); a planning-only change
could turn it red unseen, so `task plan`, which AGENTS.md § Gate item 2 orders when the store
changed, runs it.

The required checks: `~/beyond10x/gates-policy/policy.json` holds for Loom only `id` and
`baseline`, no check names. `story:rust-1-100-toolchain` names `loom / task check` the required
check; AGENTS.md § Commits names `b10x-gates / local evidence`, a receipt this story does not
touch. Both job names stay.

Forms chosen:

- The required job runs and skips its steps. Rejected: `paths-ignore` on the trigger
  (`images.yml:4-11` uses `paths:`), under which a required check never starts and the pull
  request waits for it forever.
- The classification inside each job. Rejected: a `changes` job the two jobs `needs`, because a
  job skipped after a failed need counts as passed for a required check, so a red classifier
  would let the pull request merge.
- A script of ours over `git diff -z`. Rejected: a third-party paths-filter action, one more
  pinned action with its own pattern syntax and a second place the list could drift from.
- `--no-renames`. Rejected: rename detection, under which a code file renamed to `.md` is listed
  only under its new name.

## ESS first

No declaration under `ess/` changes. The list format is a CI rule, not part of what Loom runs; its
typed home is `.github/changed-paths.sh` and its test.

## Acceptance

The cases live in `crates/loom-xtask/tests/changed_paths.rs` (new). It reads
`.github/workflows/check.yml` with `serde_yaml_ng` (already a dev-dependency of `loom-xtask`,
`crates/loom-xtask/Cargo.toml`), takes `env.GATE_PATHS`, and runs
`bash .github/changed-paths.sh <event>` with `GATE_PATHS` set to that value and the case's paths
NUL-separated on standard input. The list is never copied into the test, so the two cannot
differ. Each case runs under 1 s.

- `planning_files_alone_skip`: `.engineering/planning/story/x.md` and
  `.engineering/planning/epic/y.md` give `class=skip`.
- `prose_outside_the_site_skips`: `docs/design/notes.md` gives `class=skip`.
- `a_site_page_runs_the_docs_checks`: `website/docs/guides/bindings.md` gives `class=docs`, and
  with `.engineering/planning/story/x.md` beside it still `docs`.
- `one_code_file_beside_planning_files_runs_the_full_gate`: `.engineering/planning/story/x.md`
  and `crates/loom-cli/src/lib.rs` give `class=full`.
- `a_site_page_beside_code_runs_the_full_gate`: `website/docs/index.md` and `Cargo.lock` give
  `full`.
- `markdown_the_tests_read_runs_the_full_gate`: `AGENTS.md`, `README.md`, `CHANGELOG.md`,
  `ess/SKIPPED.md`, `crates/loom-sdk/README.md` and each of the four `docs/` files above give
  `full`, one at a time.
- `a_code_file_renamed_to_markdown_runs_the_full_gate`: the `--no-renames` listing of a rename,
  `crates/loom-cli/src/old.rs` and `docs/old.md`, gives `full`.
- `the_workflow_and_the_script_run_the_full_gate`: `.github/workflows/check.yml` and
  `.github/changed-paths.sh` give `full`.
- `an_unmatched_path_runs_the_full_gate`: `Taskfile.yml`, `LICENSE` and `rust-toolchain.toml`
  give `full`.
- `no_path_runs_the_full_gate`: empty input gives `full`.
- `other_events_run_the_full_gate`: events `push` and `workflow_dispatch` with the input
  `.engineering/x.md` give `full`.
- `unusual_paths_are_one_path_each`: `.engineering/a<LF>b.md` is one path and gives `skip`;
  `docs/a<LF>crates/b.rs` is one path, ends in `.rs` and gives `full`; `docs/"q" *.md` and
  `docs/<0xFF>.md` give `skip`. Not steered: with `-z` git does not quote a path, a newline does
  not split one, and the path is matched as a subject, never as a pattern.
- `every_markdown_file_a_test_reads_runs_the_full_gate`: every string literal ending in `.md` in
  a `*.rs` file under `crates/` and `consumers/` that names a tracked file, from the root or from
  its crate directory, classifies `full`.
- `no_test_reads_the_planning_store`: no `*.rs` file under `crates/` and `consumers/` but
  `adversary_governor_import.rs` and this file holds the literal `.engineering/planning`.
- `both_jobs_classify_first_and_skip_by_class`: in jobs `check` and `macos` the step after
  `actions/checkout` has `id: changes` and a `run` holding `git diff -z --name-only --no-renames
  HEAD^1 HEAD` and `set -euo pipefail`; each checkout's `fetch-depth` is `0` or at least `2`;
  every later step has an `if:` naming
  `steps.changes.outputs.class`; the job names are `loom / task check` and `loom / macos build`.
- `the_diff_failing_fails_the_job`: the `changes` step's own `run` text, run by `bash` with
  `GITHUB_EVENT_NAME=pull_request` and a `git` stub first on `PATH` that exits 128 (a checkout
  without `HEAD^1`), exits non-zero, so the job is red and no later step runs.
- `taskfile_plan_runs_the_planning_store_test`: `Taskfile.yml` task `plan` runs
  `adversary_governor_import` after `aep plan artifact validate`.
- AGENTS.md § Gate names the list, its three classes, that a path it does not name runs the full
  gate, and that a new test reading a `.md` file outside `website/` adds that file to the list as
  `full`.
- `cargo test -p loom-xtask --test changed_paths --locked` exits 0 within 60 s on a warm build.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and
  `slot task check` exit 0.

What the list cannot steer, stated in AGENTS.md § Gate beside it:

- A pull request that changes one file outside the `skip` rules runs the full gate, whatever else
  it changes.
- A rename counts as the removal of the old path and the addition of the new one, so code renamed
  to `.md` runs the full gate.
- A path with a newline, a quote, a space, a glob character or a byte that is not UTF-8 is one
  path, matched whole.
- A test that builds a `.md` path from parts at run time (`root.join("docs").join(name)`) is not
  seen by `every_markdown_file_a_test_reads_runs_the_full_gate`; a planning-only change can then
  turn it red unseen until the next full run on `main`.

## Out of scope

- The ruleset ("branch must be up to date", a merge queue) and the required check names: Timo's,
  unchanged.
- `pages.yml` (`Documentation validation`): it keeps running for every pull request (ledger
  question 1).
- `Shared source gates` and `b10x-gates / local evidence`: scanning, not a build; unchanged.
- The test budget: `story:test-step-within-budget`. The toolchain pin, the runner's environment
  and the forge watch: `story:local-gate-equals-forge`.

## Scope

Read at `8a8fad3b` (`loom-sdc-plan`) and in `loom-w1121g-u4`.

- **Primary surface:** `.github/workflows/check.yml`, jobs `check` (24-91) and `macos` (93-117)
  — cited
- **Files (CI):** `.github/workflows/check.yml` (`env:` 16-20 gains `GATE_PATHS`; a step after
  each checkout, 30-33 and 100-102; `if:` on every later step) — cited;
  `.github/changed-paths.sh` (new) — inferred
- **Files (tests):** `crates/loom-xtask/tests/changed_paths.rs` (new) — inferred;
  `crates/loom-xtask/Cargo.toml` (`serde_yaml_ng` dev-dependency, present) — cited
- **Files (gate):** `Taskfile.yml` task `plan` (152-155) — cited
- **Files (docs):** `AGENTS.md` § Gate (267-332) — cited; `website/docs/guides/run-the-checks.md`
  — inferred
- **Symbols:** none in Rust outside the new test — cited
- **Would collide with:** `story:test-step-within-budget` (its `Install cargo-nextest` step in
  `check.yml`, `AGENTS.md` § Gate) and `story:local-gate-equals-forge` (`check.yml` toolchain
  steps 35-38 and 104-106, `env:` 16-20, `AGENTS.md` § Gate) — cited
