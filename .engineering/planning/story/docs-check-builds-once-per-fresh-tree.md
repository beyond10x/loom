---
format: aep.planning-md/3
id: story:docs-check-builds-once-per-fresh-tree
kind: story
status: proposed
title: task docs-check compares a committed digest of the generator's inputs and builds nothing when they are unchanged
summary: docs-check 421 s cold (CA); stamp of inputs and page hashes, loom-xtask docs-stamp --check, docs-generate the one build, docs-slow byte compare in CI when the stamp changed
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:planning-only-pull-requests-skip-build
scope:
- confidence: cited
  path: .github/workflows/check.yml
- confidence: inferred
  path: Cargo.lock
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/loom-docs
- confidence: cited
  path: crates/loom-xtask/Cargo.toml
- confidence: inferred
  path: crates/loom-xtask/src/docs_stamp.rs
- confidence: inferred
  path: crates/loom-xtask/src/main.rs
- confidence: inferred
  path: website/docs/reference/.generated.json
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T15:08:37Z", actor: "agent:loom", revision: 13}
---
## Outcome

`task docs-check` does not build the documentation generator when the sources the generated
pages come from are unchanged. A fresh unit tree whose change leaves those sources alone passes
the check in seconds. One whose change touches them builds the generator once, in
`task docs-generate`. The full regeneration that compares bytes runs in a lane named `slow`, and
only when the sources changed. Cites are at `23f1f4b3`.

**Measured** (cited from the w7 batch review, not re-timed):

- cold `docs-check` and `docs-generate` builds took over 5 minutes in every fresh unit tree;
- correction unit CA ran `docs-check` at 421 s of wall time.

**Why it builds.**

- `docs-check` is `cargo run --locked -p loom-docs -- generate --check` (`Taskfile.yml:202-205`),
  a step of `task check` (`:45`). `docs-generate` is the same without `--check` (`:197-200`).
- `loom-docs` depends on `b10x-loom-cli` and `b10x-loom-operator`
  (`crates/loom-docs/Cargo.toml:12-13`). It walks their clap definitions (`src/cli.rs:132`,
  `:141`) and reads constants of `b10x_loom_cli` (`src/main.rs:470`, `:482-485`).
- So the check builds the whole CLI and its dependency tree as a binary.
- What it generates comes from (`src/main.rs:1-29`):
  - those clap definitions;
  - `cargo metadata --no-deps`;
  - `ess generate --kind docs` and `ess specify compile` over `ess/` and `ess/intake/`;
  - the `intake.events` and `loom.runlog` declarations;
  - the hand-written `website/docs/reference/run-events.md`.

**After, under D1 a** (the folder's first accepted form, `~/beyond10x/AGENTS.md` § Tests stay
fast: "a committed generated file with a digest of its sources; a fast check compares the
digest"):


- **A committed stamp.** `website/docs/reference/.generated.json` holds:
  - the SHA-256 of the generator's inputs;
  - the version line `ess --version` prints;
  - the SHA-256 of each page and contract that `loom-docs` writes.
- **The inputs**, one list in one place (`INPUTS`, inferred name, in the shared file below):
  - `crates/loom-docs/`, `crates/loom-cli/` and `crates/loom-operator/`;
  - every workspace `Cargo.toml`, and `Cargo.lock`;
  - `ess/`, which holds `ess/intake/`;
  - `website/docs/reference/run-events.md`.

  The input hash covers the sorted list of input paths and each file's bytes, so an added or a
  removed file changes it.
- **One hash function, under D3 a.**
  - One source file, `crates/loom-docs/src/stamp.rs`, holds `INPUTS`, the input hash, the
    stamp's reader and writer, the read functions of test 5 and the raw-admonition lint (below).
    It uses `std` and `sha2` only.
  - **Errors in the shared file are `std::io::Error`.** Its functions return
    `std::io::Result`, with the path in the error's message. It does not use `anyhow`, and no
    `anyhow` is added to `loom-xtask`, which has none (`crates/loom-xtask/Cargo.toml:10-17`).
    - In `loom-docs`, the call sites turn the error into `anyhow` with `?` or `.with_context`.
    - In `loom-xtask`, the call sites turn it into the `String` errors that crate uses
      (`main.rs:482`, `:488`).
  - `loom-docs` declares it `mod stamp;`.
  - `loom-xtask` compiles the same file as `#[path = "../../loom-docs/src/stamp.rs"] mod
    stamp;`. There is no crate edge between the two. `loom-xtask` must not depend on
    `loom-docs`, which depends on `loom-cli` (`crates/loom-docs/Cargo.toml:12-13`), the build
    this story removes.
  - The shared file holds no test and nothing that calls `generate`, `cargo` or `ess`, so both
    crates compile it as it is. Its tests are tests 1-5 and 7 in `loom-xtask` and test 6 in
    `loom-docs`' `main.rs`.
  - Both manifests gain `sha2 = "0.10"`. `Cargo.lock` already resolves `sha2` 0.10.9
    (`:3764-3765`), so it gains no package; the dependency lists of `loom-docs` (`:2575`)
    and `loom-xtask` (`:2588`) in the lock each gain `sha2`.
- **The raw-admonition lint moves.** Today `generate --check` also refuses a raw admonition
  title in any page under `website/docs` (`crates/loom-docs/src/main.rs:603-612`, with
  `is_raw_admonition` and `raw_admonitions` at `:71-108`). Those pages are hand-written and
  not inputs, so the stamp does not see them. The lint moves into the shared file, and
  `docs-stamp --check` runs it; `loom-docs` keeps calling it in `--check`.
  - Today `raw_admonitions` returns `anyhow::Result` and uses `.with_context`
    (`main.rs:87-90`). In the shared file it returns `std::io::Result<Vec<String>>`, as above,
    and `main.rs:604` maps the error into `anyhow`.
  - Its existing test (`raw_admonition_titles_are_found_and_bracketed_ones_pass`,
    `main.rs:725-758`) moves into `docs_stamp.rs`. It makes its own scratch directory there, as
    `loom-xtask` does elsewhere: under `default_scratch` (`crates/loom-xtask/src/main.rs:588-593`,
    `target/loom-xtask/`), with the process id and a timestamp in the name, as `main.rs:494`
    does. It does not use `loom-docs`' private `scratch` (`crates/loom-docs/src/main.rs:655-663`).
- **`task docs-check`** becomes `loom-xtask docs-stamp --check`. `loom-xtask` is already built by
  the gate's test step, and its dependencies do not include `loom-cli`
  (`crates/loom-xtask/Cargo.toml:10-17`). It:
  - hashes the inputs in the working tree;
  - runs `ess --version`;
  - hashes each output named in the stamp;
  - compares all three with the stamp.

  It fails naming the first input, the `ess` version or the page that differs, and says `run
  task docs-generate`. It never builds `loom-docs`.
- **`task docs-generate`** runs `loom-docs generate` as today and writes the stamp after the
  pages. It is the one build, and it happens only when an input changed.
- **`task docs-slow`**, the `slow` lane, is today's `loom-docs generate --check`. It regenerates
  and compares bytes with the committed pages and the stamp. A full `generate` (it runs
  `cargo metadata` and `ess`) runs only here and in `docs-generate`, never in the test step.
- **When CI runs it (D2 a).** `loom-xtask docs-stamp --slow-needed --base <file>
  --event <name> --ref <ref>` prints `run` or `skip`:
  - `run` for event `push` on `refs/heads/main`;
  - `run` when the stamp differs from `<file>`, the base branch's stamp, which the job takes with
    `git show <base>:website/docs/reference/.generated.json`;
  - `skip` otherwise.

  `.github/workflows/check.yml` gains a job `docs-slow` whose one step runs the decision and
  then `task docs-slow` only on `run`. The ruleset is not changed.
- **A hand edit** of a generated page changes its SHA-256 in the check, so `docs-check` still
  fails on it, as `--check` does today.

## Decisions

- **D1. How the check avoids the build.** Default **a**.
  - **a.** A committed stamp, as above. A tree whose change leaves the inputs alone never builds
    `loom-docs`. The cost is one input list to keep complete (test 5).
  - **b.** A cache under `target/` keyed by the inputs' digest (the folder's second form): the
    generator's output kept under `target/loom-docs/<digest>/`. A second check in the same tree
    is fast, but every fresh tree still builds once, the 421 s this story is about.
  - **c.** Both. More code, and it gains over (a) only in a tree that changed the inputs and runs
    the check twice.
- **D2. When CI runs the `slow` lane.** Default **a**.
  - **a.** On a pull request whose stamp differs from its base's, and on every push to `main`.
    The push catches an input the list missed.
  - **b.** On every pull request. That is today's cost in CI, but nothing is missed.
- **D3. Where the shared hash function lives.** Default **a**.
  - **a.** One source file in `loom-docs`, compiled into `loom-xtask` by `#[path]`, as above. No
    crate edge, and no new crate.
  - **b.** A new crate, `loom-docs-stamp` (std and `sha2`), that both depend on. A cleaner edge,
    but one more crate and one more manifest.
  - **c.** `loom-docs` gains a library target that exports it, and `loom-xtask` depends on
    `loom-docs`. Rejected: that brings `loom-cli` into `loom-xtask`'s build.

## ESS first

No noun changes. The stamp is a file of the gate, not a declaration of Loom.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. New tools are
Rust (`loom-xtask`) or POSIX sh, never Python (§ No Python for permanent tools). Stubs on `PATH`
are POSIX sh. A slot job ends within 10 minutes, or names its time with `slot --timeout` (at most
30 minutes). A script that only measures or times runs without the `slot` prefix (§ Scripts that
do not build stay off slot).

**`crates/loom-xtask/src/docs_stamp.rs`**, unit tests in its `#[cfg(test)] mod tests`, as
`test_step.rs:645` holds its own. They are not an integration target under `tests/`, so they do
not move with `story:one-test-binary-per-crate`.

1. `the_check_passes_on_an_unchanged_tree_and_builds_nothing`.
   - On a scratch copy of the inputs and pages with a matching stamp, the check exits 0.
   - `PATH` holds only a stub `ess` that prints the stamp's version line, and no `cargo`.
   - Failing first: there is no `docs-stamp`.
2. `a_changed_input_or_page_fails_naming_it`. Each of these, on its own copy, fails the check
   and names the file and `run task docs-generate`:
   - one byte changed in `crates/loom-cli/src/lib.rs`;
   - one byte changed in `ess/domains/runlog.yaml`;
   - one byte changed in `website/docs/reference/cli.md`;
   - a new file `crates/loom-cli/src/added.rs`;
   - `ess/domains/runlog.yaml` removed.

   Failing first: as test 1.
3. `another_ess_version_fails_the_check`.
   - The stub `ess` prints another version line, and the check fails naming the `ess` version.
   - Failing first: as test 1.
4. `the_slow_lane_runs_only_when_needed`.
   - The decision gives:
     - `skip` for a pull request whose stamp equals its base's;
     - `run` for one whose stamp differs;
     - `run` for a push to `refs/heads/main` with equal stamps;
     - `skip` for a push to another branch with equal stamps.
   - It also reads `.github/workflows/check.yml`: the `docs-slow` job runs
     `docs-stamp --slow-needed` and runs `task docs-slow` only when that printed `run`.
   - Failing first: there is no decision and no job.
5. `every_file_loom_docs_reads_is_an_input`.
   - **What it scans:** file reads only (`fs::read`, `fs::read_to_string`, `fs::read_dir`), in
     the files of `crates/loom-docs/src/` other than `stamp.rs`. It skips each
     `#[cfg(test)]` module, so the test reads at `crates.rs:215`, `run_events.rs:626` and
     `run_log.rs:103` are not counted.
   - **What it asserts:** it finds no such read. The generator reads through
     `stamp::read_input` and `stamp::read_input_dir` (inferred names), which refuse a path that
     is not under an `INPUTS` entry or an output the stamp names.
   - **Process calls are not scanned.** They are covered by `INPUTS`:
     - `cargo metadata --no-deps` (`crates.rs:136`) reads the workspace's `Cargo.toml` files
       and `Cargo.lock`, which are inputs;
     - `ess` (`main.rs:304`) reads `ess/`, which is an input, and its version line is in the
       stamp.

     A new process call that reads something else is caught by the `slow` lane on the push to
     `main` (D2 a), not by this test.
   - **Failing first:** red on today's code. The generator reads directly at `main.rs:397`,
     `:413`, `:468`, `:495`, `:542`, `:568`, `:571` and `:627`. It goes green when those reads
     are routed through the two functions. The lint's reads (`:89`, `:101`) move into the
     shared file. After that, a planted direct read
     beside them turns it red again.

**`crates/loom-docs`**, a unit test in the `#[cfg(test)] mod tests` of `src/main.rs` (`:648`),
not in the shared file (`cargo test -p loom-docs`, as `pages.yml:52` runs it):

6. `the_writer_puts_the_stamp_after_the_pages`.
   - The page-and-stamp writer that `generate` calls (inferred: `write_outputs(root, outputs)`)
     is given fixed in-memory pages and a scratch root that holds a few input files.
   - The stamp's page hashes equal the written pages, and its input hash equals
     `stamp::input_hash` for that root: the same function `loom-xtask` compiles.
   - It does not call `generate`, so there is no `cargo metadata` and no `ess`. Expected under
     1 s, well inside the 60 s bound. The real `generate` runs only in the `slow` lane and in
     `docs-generate`.
   - Failing first: no stamp is written.

**`crates/loom-xtask/src/docs_stamp.rs`**, one more unit test beside tests 1-5:

7. `the_check_refuses_a_raw_admonition_title`.
   - The scratch copy of test 1 (matching stamp, stub `ess`, no `cargo`) gains the page
     `website/docs/concepts/planned.md`, whose second line is `:::caution Planned`. That page
     is not an input and not an output, so the stamp still matches.
   - The `docs-stamp --check` entry point fails. Its message names
     `website/docs/concepts/planned.md:2` and gives the hint
     ``raw admonition titles (write `:::kind[Title]`)``, the message of
     `crates/loom-docs/src/main.rs:606-609` today.
   - This tests the wiring. The moved test calls `raw_admonitions` directly, and it would stay
     green if `--check` never called the lint.
   - Failing first: there is no `docs-stamp`. A planted defect that drops the lint call from
     `--check` turns it red, and leaves test 1 and the moved test green.

**The measurement**, in the unit's report, wall time on a fresh tree with sccache warm and the
slot queue's wait excluded:

- `task docs-check` before: 421 s, the figure cited above.
- `task docs-check` after, on a tree whose change leaves the inputs alone: expected under 10 s.
- `task docs-generate` after, once.

**Gate.** `task check` exits 0, and the planning-only rule of
`story:planning-only-pull-requests-skip-build` still holds for the new CI job.

## Out of scope

- `commission:docs-drift` and `loom-commission-docs` (`Taskfile.yml:46`). Same shape, but a
  story of its own if it is slow.
- The site build (`task website`).

## Scope

Derived 2026-11-22 from the plan tree at `23f1f4b3`. **Cited** means read at the line named;
**inferred** means a reading that could be wrong.

- `Taskfile.yml` (`:45`, `:197-212`) — cited.
- `crates/loom-docs/Cargo.toml` (`:12-13`) and `crates/loom-docs/src/` (`main.rs:1-29`,
  `:71-108`, `:112-131`, `:304`, `:397-413`, `:468-495`, `:542-571`, `:603-627`, `:648`,
  `:725-758`; `cli.rs:132`, `:141`; `crates.rs:136`, `:215`; `run_events.rs:626`;
  `run_log.rs:103`) — cited.
- `crates/loom-xtask/src/main.rs` (the subcommand), `crates/loom-xtask/src/docs_stamp.rs`
  (new) and `crates/loom-xtask/Cargo.toml` (`:10-17`, `sha2`) — inferred for the new files,
  cited for the manifest.
- `crates/loom-docs/src/stamp.rs` (new) — inferred.
- `Cargo.lock`: `sha2` is at `:3764-3765` (cited). The two packages' dependency lists gain
  `sha2` (inferred).
- `website/docs/reference/.generated.json` (new) — inferred.
- `.github/workflows/check.yml` (`:113-114`) and `.github/workflows/pages.yml` (`:52-54`) —
  cited.

## Edges

- `depends_on story:planning-only-pull-requests-skip-build` (active). It owns the CI path list
  that lets a prose-only pull request pass. The new `docs-slow` job goes into that list's
  "build and test" side, so this story builds on its list.
- `AGENTS.md`, `CHANGELOG.md` and generated pages are shared with most stories. They are
  textual merges and are not listed below except as said.
- **Would collide with**, no edge. Each entry names the shared file and the merge order:
  - `Taskfile.yml` (`docs-check`, `docs-generate`, the new `docs-slow`). Other tasks; textual
    merge, no order:
    - active: `bound-program-retry-records`, `cancel-a-running-bound-program`,
      `decision-request-entity`, `local-gate-equals-forge`, `one-test-binary-per-crate`,
      `protocol-argument-schema`, `release-binaries`, `test-step-within-budget`,
      `tree-library-exports-what-an-engine-needs`, `tree-run-command` and
      `tree-run-tests-fit-the-budget`;
    - proposed: `readme-logo-and-badges` and `website-ux-loop`;
    - draft: `chart-upgrade-applies-crd` and `tree-run-tests-fit-the-test-budget`.
  - `crates/loom-xtask/src/main.rs` (one subcommand each). Textual merge, no order:
    - active: `drop-the-substrate-git-feature`, `local-gate-equals-forge`,
      `one-test-binary-per-crate`, `test-step-within-budget` and
      `tree-run-tests-fit-the-budget`;
    - proposed: `readme-logo-and-badges` and `website-ux-loop`.
  - `.github/workflows/check.yml`:
    - active: `decision-request-entity`, `local-gate-equals-forge` and
      `test-step-within-budget`. Textual merge, no order;
    - proposed: `readme-logo-and-badges`, `release-binaries-macos` and `website-ux-loop`.
      Textual merge, no order.
  - `crates/loom-docs`: `tree-run-command` and `tree-suite-stop-resume-reopen-steps` (active),
    and `readme-logo-and-badges` (proposed). Textual merge, no order.
  - `Cargo.lock`, the entries of `loom-docs` and `loom-xtask` only. Textual merge, no
    order; a lock conflict is resolved by taking the other's lock and running
    `cargo update -w`, which re-adds `sha2` to the two lists:
    - active: `canon-0-10-0-pin`, `canon-0-11-0-pin`, `drop-the-substrate-git-feature`,
      `engineering-protocols-dependency-removed` and `protocol-argument-schema`;
    - proposed: `gate-program-identity-bound-to-spawn`, `observe-executor-reads-workspace-files`,
      `observe-git-changed` and `substrate-pin-at-the-deny-list`;
    - draft: `child-launches-through-confinement-adapters`, `secrets-subcommand` and
      `substrate-0-24-0-pin`.
  - **Every story that changes a generated page**, that is every story that changes the CLI.
    For example, the ones on `website/docs/reference/cli.md`:
    - active: `person-action-runs-a-program`, `suite-runs-cycles-in-parallel`,
      `suite-runs-cycles-under-one-cgroup-root`, `tree-run-command`, `tree-suite-format`,
      `tree-suite-stop-resume-reopen-steps` and `tree-test-command`;
    - and twelve proposed stories.

    After this story each of them also commits the stamp `docs-generate` writes. A conflict in
    the stamp is resolved by running `task docs-generate` on the merge.
