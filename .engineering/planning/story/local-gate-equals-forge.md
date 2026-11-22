---
format: aep.planning-md/3
id: story:local-gate-equals-forge
kind: story
status: active
title: Loom's local gate and CI build with the one rustc rust-toolchain.toml pins, the gate fails where the tree differs from the runner, and the forge watch exits non-zero on red
summary: rust-toolchain.toml read by 4 workflow jobs; .cargo/config.toml; ubuntu-24.04; loom-xtask forge-parity (first step of check) and forge-watch (check runs, exit 0/1/2/3)
tags:
- gate
relations:
- serves: vision:O1
scope:
- confidence: inferred
  path: .cargo/config.toml
- confidence: cited
  path: .github/workflows/check.yml
- confidence: cited
  path: .github/workflows/pages.yml
- confidence: cited
  path: .github/workflows/release.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/loom-xtask/Cargo.toml
- confidence: inferred
  path: crates/loom-xtask/src/forge_parity.rs
- confidence: inferred
  path: crates/loom-xtask/src/forge_watch.rs
- confidence: cited
  path: crates/loom-xtask/src/main.rs
- confidence: inferred
  path: crates/loom-xtask/tests/forge_parity.rs
- confidence: inferred
  path: crates/loom-xtask/tests/forge_watch.rs
- confidence: inferred
  path: rust-toolchain.toml
- confidence: inferred
  path: website/docs/guides/run-the-checks.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T22:00:15Z", actor: "agent:host-operator", revision: 6, executor: "agent:loom", correlation: "loom-gate-rules-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T22:00:15Z", actor: "agent:host-operator", revision: 7, executor: "agent:loom", correlation: "loom-gate-rules-2026-11-21"}
---
## Outcome

The local gate and the forge build with one rustc, named once, in `rust-toolchain.toml`. What the
runner does differently from a session's tree is written in the repository, and `task check`
fails where the session's tree or environment differs from it in a way the gate can see. The
command that watches the forge after a push exits non-zero when a check on that commit ends red.

- `rust-toolchain.toml` (new): `[toolchain]`, `channel = "1.99.1"` (the release every workflow
  installs when the story is taken, named by number), `components = ["clippy", "rustfmt"]`,
  `profile = "minimal"`.
- Every job that installs Rust (`check.yml` jobs `check` and `macos`, `pages.yml` job `build`,
  `release.yml` job `verify`) first runs a step `id: toolchain` that reads `channel` from
  `rust-toolchain.toml`, fails unless it is `X.Y.Z`, and writes `channel=X.Y.Z` to
  `$GITHUB_OUTPUT`; `dtolnay/rust-toolchain` gets `toolchain: ${{ steps.toolchain.outputs.channel
  }}`. No workflow holds a Rust version literal.
- `.cargo/config.toml` (new) holds the build settings `check.yml:16-20` sets for CI today:
  `[build] incremental = false`, `[profile.dev] debug = 0`, `[profile.test] debug = 0`.
  `check.yml`'s `env:` keeps `CARGO_TERM_COLOR` only. A local build in the tree and CI's build use
  the same settings, `consumers/pinned/` and test fixtures under the checkout included.
- The runner OS image is named by version: `ubuntu-24.04` in place of `ubuntu-latest`
  (`check.yml:25`, `pages.yml:19`, `release.yml:17`), as `images.yml` names it already.
- `loom-xtask forge-parity` (new, `crates/loom-xtask/src/forge_parity.rs`), the first step of
  `task check`, so CI runs it too. It runs these checks, or those named by `--check <name>`, and
  exits 1 naming every failure, 0 printing one line per check:
  - `workflows`: every `dtolnay/rust-toolchain` step reads the `toolchain` step's output and every
    job with one has that step; no `runs-on` ends in `-latest`;
  - `rustc`: `$RUSTC --version`, else `rustc --version`, run at the root, names the channel, and
    `RUSTUP_TOOLCHAIN`, when set, starts with it; it prints both versions;
  - `environment`: none of `RUSTFLAGS`, `RUSTDOCFLAGS`, `CARGO_BUILD_RUSTFLAGS`,
    `CARGO_ENCODED_RUSTFLAGS`, `CARGO_TARGET_DIR`, `RUST_MIN_STACK`, `RUST_TEST_THREADS` and no
    `CARGO_PROFILE_*` is set to a non-empty value, and `CARGO_INCREMENTAL` is unset or `0`; the
    list is `RUNNER_UNSET` in `forge_parity.rs`, the variables CI does not set;
  - `stack`: the soft stack limit of `/proc/self/limits` is at least 8 MiB, the runner's
    (skipped with a line saying so where `/proc` is absent);
  - `tree`: `git status --porcelain=v1 -z --untracked-files=all` lists no untracked file that is
    not ignored, naming each (CI's checkout holds only committed files);
  - `ess-pin`: the `ESS_VERSION` of every workflow that installs `ess` equals `requires:` of
    `ess/ess-inputs.yaml`.
- `loom-xtask forge-watch --commit <sha> [--wait-seconds <n>] [--poll-seconds <n>]` (new,
  `crates/loom-xtask/src/forge_watch.rs`) and `task forge-watch COMMIT=<sha>`: it reads
  `gh api repos/beyond10x/loom/commits/<sha>/check-runs?per_page=100` (the program is `$LOOM_GH`,
  else `gh`, read-only as AGENTS.md § Commits allows), polls until at least one check run exists
  and every one is `completed`, prints `<name>  <conclusion>  <html_url>` for each, and exits 0
  when every conclusion is `success` or `skipped`, 1 when any other conclusion is there, 2 when no
  check run appeared or the wait (default 3600 s) ended first, 3 when `gh` failed or printed
  what is not that JSON.
- AGENTS.md § Gate replaces its two toolchain paragraphs (`AGENTS.md:270-274`, `:297-299`) with:
  the file is the pin; a table "What the runner does differently" (OS image, environment, stack,
  build settings, a clean checkout, the `target/` inside it, the `ess` binary) naming for each
  row where it is written and which `forge-parity` check holds it, or that none can; and the rule
  that when the forge goes red after a green local gate, the fix includes the change that makes
  the local gate catch the same failure. § Commits names the sequence: the bot push, then
  `task forge-watch COMMIT=<sha>`, and no merge while it exits non-zero.

## Why

`~/beyond10x/AGENTS.md` § The local gate equals the forge: on 2026-11-21 about 385 minutes of
merges were blocked by a forge run that went red after a green local gate; for Loom, 48 minutes,
rustc 1.100.0 on CI and 1.99.1 locally.

Loom today: `check.yml:35-38` and `:104-106`, `pages.yml:30-32` and `release.yml:29-31` each hold
the literal `toolchain: 1.99.1` (commit `25c96246`, "pin every workflow to 1.99.1", after
`toolchain: stable` moved to 1.100.0 on 2026-11-12 and turned the release check of pull request
163 red, `story:rust-1-100-toolchain` § Found). There is no `rust-toolchain.toml` and no
`.cargo/`. AGENTS.md says both "Every workflow installs Rust 1.99.1" (`:271`) and "CI builds on
`stable`" (`:297`). Nothing checks that a local gate ran on the pinned release; the gate record
names `rustc --version` by hand (`story:rust-1-100-toolchain` Acceptance). CI's build settings
live only in `check.yml:16-20`. `ubuntu-latest` (`check.yml:25`) moves with GitHub's image. There
is no script that watches the forge: AGENTS.md § Commits (`:2465-2476`) names the bot push and
the `b10x-gates` receipt, and `gh` is read-only.

`crates/loom-intake-slice/src/confinement.rs:3263-3275` already resolves a workspace's own
toolchain file with `RUSTUP_AUTO_INSTALL=0` and without `RUSTUP_TOOLCHAIN`; the file this story
adds is one such file for Loom's own tree.

Relation to `story:rust-1-100-toolchain`: this story replaces part of it. Its first Acceptance
line (four workflows install the version and AGENTS.md names it) becomes one line of
`rust-toolchain.toml`, and its Out of scope line excluding the file is withdrawn. The move to
1.100.0 and the fix of `adversary2_executor_guard.rs` stay there. This story does not depend on
it: it pins the release the workflows install today.

Forms chosen:

- A step that reads the file into an output. Rejected: dropping `toolchain:` and letting rustup
  read the file on the runner, which depends on the runner's rustup installing a missing
  toolchain on first use, and leaves `dtolnay/rust-toolchain` installing a second one.
- `.cargo/config.toml` for the build settings. Rejected: an env file read by Task's `dotenv`,
  which does not override a variable the session's shell already set, so the session's value
  would win without a word.
- A check of untracked files. Rejected: running the gate in a fresh checkout of `HEAD`, which is a
  cold build per gate (a full gate's `target/` reached 39G, approval record of wave
  2026-11-21-w1) and cannot share a build directory (AGENTS.md § Gate).
- Check runs of the commit. Rejected: workflow runs (`actions/runs?head_sha=`), which do not list
  the check `b10x-gates / local evidence` the bot publishes.
- A `loom-xtask` subcommand. Rejected: a shell script under `scripts/`, which Loom does not keep;
  its gate steps live in `loom-xtask` with tests beside them.

## ESS first

No declaration under `ess/` changes: the toolchain pin, the runner's settings and the forge watch
are the repository's gate, not part of what Loom runs. Their typed homes are
`rust-toolchain.toml`, `.cargo/config.toml` and the two `loom-xtask` modules.

## Acceptance

The cases are in `crates/loom-xtask/tests/forge_parity.rs` and
`crates/loom-xtask/tests/forge_watch.rs` (new). Each runs `loom-xtask` (`CARGO_BIN_EXE_loom-xtask`)
with `--root` on a fixture tree of a few files written under `CARGO_TARGET_TMPDIR`, or on the
repository, and runs under 2 s. `forge_watch.rs` uses a stub `gh` (a shell script that prints the
next of its canned answers) through `LOOM_GH`; no case reaches the network.

- `the_repository_passes_the_workflow_and_pin_checks`: `forge-parity --check workflows --check
  ess-pin` on the repository exits 0.
- `a_workflow_naming_another_toolchain_fails`: a fixture `check.yml` whose step says
  `toolchain: 1.100.0` beside a file pinning `1.99.1` exits 1 naming `check.yml`, the job, and
  both versions.
- `a_job_without_the_toolchain_step_fails`: a job that uses `dtolnay/rust-toolchain` with no step
  `id: toolchain` before it exits 1 naming the job.
- `a_pin_that_is_not_a_version_fails`: `channel = "stable"` exits 1; so does a file with no
  `channel`.
- `a_latest_runner_image_fails`: `runs-on: ubuntu-latest` exits 1 naming the job.
- `the_toolchain_step_reads_the_file`: the `toolchain` step's `run` text, run by `bash` in a
  fixture with `channel = "1.99.1"`, writes `channel=1.99.1` to `GITHUB_OUTPUT`; with
  `channel = "stable"` it exits non-zero.
- `a_rustc_of_another_release_fails`: `RUSTC` set to a stub printing `rustc 1.100.0 (0 2026-11-12)`
  exits 1 naming `1.100.0` and `1.99.1`; a stub printing `rustc 1.99.1 (0 2026-10-01)` exits 0.
- `a_rustup_override_of_another_release_fails`: `RUSTUP_TOOLCHAIN=1.100.0-x86_64-unknown-linux-gnu`
  exits 1; `1.99.1-x86_64-unknown-linux-gnu` exits 0.
- `an_environment_the_runner_does_not_have_fails`: `RUSTFLAGS=-Ctarget-cpu=native`,
  `CARGO_PROFILE_TEST_DEBUG=2`, `CARGO_INCREMENTAL=1` and `RUST_MIN_STACK=67108864` each exit 1
  naming the variable; `CARGO_INCREMENTAL=0` exits 0.
- `the_stack_limit_is_read_from_proc` (`forge_parity.rs` unit test): a `limits` text with
  `Max stack size 8388608` passes, `4194304` fails, `unlimited` passes.
- `an_untracked_file_fails_the_tree_check`: a fixture git repository with one committed file and
  one untracked file exits 1 naming the untracked one; the same file staged, or ignored by
  `.gitignore`, exits 0; a path with a newline is named as one path.
- `an_ess_version_off_the_pin_fails`: a fixture workflow with `ESS_VERSION: "0.97.0"` beside
  `requires: ess 0.96.0` exits 1 naming both.
- `taskfile_check_runs_forge_parity_first`: the first command of `Taskfile.yml` task `check` is
  `task: forge-parity`.
- `cargo_config_holds_the_runner_build_settings`: `.cargo/config.toml` sets `build.incremental =
  false`, `profile.dev.debug = 0` and `profile.test.debug = 0`, and `check.yml`'s `env:` sets none
  of `CARGO_INCREMENTAL`, `CARGO_PROFILE_DEV_DEBUG`, `CARGO_PROFILE_TEST_DEBUG`.
- `every_check_passing_exits_0`: two check runs `success` and one `skipped`, all `completed`,
  exit 0 and print three lines with name, conclusion and URL.
- `one_red_check_exits_1`: one check run `failure` beside a `success` exits 1 and prints the red
  one's name and URL; `cancelled`, `timed_out`, `action_required`, `neutral` and `stale` each
  exit 1.
- `a_check_still_running_is_waited_for`: an answer `in_progress`, then `completed`/`success`, with
  `--poll-seconds 0`, exits 0 after the stub was called twice.
- `no_check_run_exits_2`: an empty `check_runs` list with `--wait-seconds 0` exits 2.
- `the_wait_ending_exits_2`: a check run `queued` on every answer with `--wait-seconds 0` exits 2.
- `gh_failing_exits_3`: a stub exiting 1, and a stub printing `not json`, each exit 3.
- Not steered through the check run's text: a check run named `success` whose `conclusion` is
  `failure` exits 1; the name and URL are printed and never parsed for a verdict.
- `forge-watch` writes no file, so a stop of the command leaves nothing; it is run again.
- `agents_md_states_the_forge_rules`: AGENTS.md § Gate names `rust-toolchain.toml` as the pin,
  holds the table "What the runner does differently" and the sentence on fixing the local gate
  after a red forge run; § Commits names `task forge-watch`.
- `slot task check` exits 0 under the pinned rustc, its first lines naming `rustc 1.99.1` and
  `rust-toolchain.toml 1.99.1`, and `RUSTUP_TOOLCHAIN=1.100.0 slot task check` exits 1 at
  `forge-parity` (where 1.100.0 is installed).
- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings`
  exit 0; `cargo test -p loom-xtask --test forge_parity --test forge_watch --locked` ends within
  60 s on a warm build.

What the local gate cannot catch, stated in the AGENTS.md table:

- A file that `.gitignore` ignores and that exists only in a session's tree (the planning drafts,
  `website/node_modules/`, `generated/rust/*/.ess-output/`) and that a test reads.
- The OS image itself: the local machine is not Ubuntu 24.04; a difference of a system library or
  of bubblewrap's version shows only on the runner.
- A check that starts after every other run of the commit completed (`Documentation site` starts
  from `Documentation validation` on `main`): `forge-watch` may end before it appears.
- Whether `Shared source gates` (`pull_request_target`) reports its checks on the pull request's
  head commit: not established; the first watched pull request shows it.

## Out of scope

- The move to Rust 1.100.0 and the guard tests' binary lookup: `story:rust-1-100-toolchain`.
- `rust-version` in `Cargo.toml` and the toolchain of consumers' builds: a git dependency's
  `rust-toolchain.toml` is not read by the consumer's cargo.
- The test budget and nextest: `story:test-step-within-budget`. The planning-only skip:
  `story:planning-only-pull-requests-skip-build`.
- The ruleset, the merge queue and the required check names: Timo's, unchanged.
- Pushing: the bot push stays `b10x-gates bot … -- push`; `forge-watch` only watches.

## Scope

Read at `8a8fad3b` (`loom-sdc-plan`) and in `loom-w1121g-u4`.

- **Primary surface:** the gate's toolchain and environment, `Taskfile.yml` task `check` (15-39)
  and the workflows that install Rust — cited
- **Files (pin):** `rust-toolchain.toml` (new), `.cargo/config.toml` (new) — inferred
- **Files (CI):** `.github/workflows/check.yml` (16-20, 25, 35-38, 95, 104-106),
  `.github/workflows/pages.yml` (19, 30-32), `.github/workflows/release.yml` (17, 29-31) — cited
- **Files (xtask):** `crates/loom-xtask/src/main.rs` (subcommands `ForgeParity`, `ForgeWatch`;
  `--root` at 278-285) — cited; `crates/loom-xtask/src/forge_parity.rs`,
  `crates/loom-xtask/src/forge_watch.rs` (new) — inferred; `crates/loom-xtask/Cargo.toml`
  (`toml`, `serde_json` present; YAML through `serde_yaml_ng`, today a dev-dependency only, moves
  to `[dependencies]`) — cited
- **Files (tests):** `crates/loom-xtask/tests/forge_parity.rs`,
  `crates/loom-xtask/tests/forge_watch.rs` (new) — inferred
- **Files (gate):** `Taskfile.yml` (`forge-parity`, `forge-watch`, first command of `check`) —
  cited
- **Files (docs):** `AGENTS.md` § Gate (270-274, 297-299), § Commits (2465-2476) — cited;
  `website/docs/guides/run-the-checks.md` (names the toolchain and the gate's steps) — inferred
- **Symbols:** `forge_parity::{run, RUNNER_UNSET}`, `forge_watch::run` — inferred
- **Would collide with:** `story:test-step-within-budget` (`check.yml`, `Taskfile.yml` task
  `check`, `crates/loom-xtask/src/main.rs`, `AGENTS.md` § Gate),
  `story:planning-only-pull-requests-skip-build` (`check.yml` jobs and `env:`, `AGENTS.md`
  § Gate), `story:rust-1-100-toolchain` (the four workflows' toolchain steps) — cited
