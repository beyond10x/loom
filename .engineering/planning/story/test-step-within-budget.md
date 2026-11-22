---
format: aep.planning-md/3
id: story:test-step-within-budget
kind: story
status: active
title: Loom's test step ends within 120 s with no test over 60 s, and reads a committed synthesized suite where its tests synthesized ess/ about 46 times
summary: generated/ess/loom/ suite + input digest; ess-gate and conform read it; task slow byte-compares; loom-xtask test-step on nextest 60 s/120 s, ten slowest; ESS_TOOLCHAIN from pin
tags:
- gate
relations:
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
  path: crates/loom-conformance/tests/adversary2_w2_conformance_status.rs
- confidence: cited
  path: crates/loom-conformance/tests/conform.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary2_ess_gate.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_ess_gate.rs
- confidence: cited
  path: crates/loom-executor/tests/ess_gate.rs
- confidence: cited
  path: crates/loom-executor/tests/support/ess_suite.rs
- confidence: cited
  path: crates/loom-xtask/src/main.rs
- confidence: cited
  path: crates/loom-xtask/src/test_step.rs
- confidence: cited
  path: generated/ess/loom/suite.json
- confidence: cited
  path: generated/ess/loom/synthesis.txt
- confidence: cited
  path: website/docs/guides/run-the-checks.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T22:00:14Z", actor: "agent:host-operator", revision: 5, executor: "agent:loom", correlation: "loom-gate-rules-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T22:00:14Z", actor: "agent:host-operator", revision: 6, executor: "agent:loom", correlation: "loom-gate-rules-2026-11-21"}
---
## Outcome

The test step of `task check` ends within 120 s of wall time on a warm build, no test in it runs
longer than 60 s, and the gate enforces both, locally and in CI. No test of that step synthesizes
`ess/` with the decision domain in it, and no test binary synthesizes one input twice.

- The conformance suite synthesized from `ess/` is a committed generated file:
  `generated/ess/loom/suite.json`, and beside it `generated/ess/loom/synthesis.txt` (format
  `loom-ess-synthesis/1`: the command, `regenerate: task suite-generate`, the `ess` version line,
  `inputs: sha256:<digest>` over every file under `ess/` by sorted path plus that version line,
  synthesize's `exit:` and `stdout:`, and `suite: sha256:<digest>` of `suite.json`).
  `task suite-generate` (`loom-xtask suite-generate`) writes both, running `ess` in `ess/`.
- Step 3 of `task ess-gate` and `task conform` read the committed files after
  `ess_suite::committed` checks that the recorded input digest is this tree's and the recorded
  suite digest is `suite.json`'s. A different input digest fails naming `task suite-generate`.
  Neither step synthesizes. The cover check (`synthesize_holds`, `unexecuted_covers`,
  `COVER_TABLE`) reads the committed `stdout:` and suite as it read a fresh run before.
- A probe test that changes a copy of `ess/` synthesizes the copy without the decision domain
  (`ess_suite::drop_decision`) unless the probe is about that domain, through
  `ess_suite::synthesize_once`, which keeps one result per input digest for every test process
  that shares the cache directory (a lock file holding the PID; a lock whose process is gone is
  taken over).
- The slow lane is `task slow`, never a dependency of `task check`: `loom-xtask suite-check`
  synthesizes `ess/` again and fails unless both committed files are equal byte for byte, naming
  each that differs; then `cargo nextest run --workspace --locked --profile slow` runs every test
  named `slow_`. A test that cannot meet 60 s is renamed `slow_…` and runs there; it is not
  deleted.
- The test step is `loom-xtask test-step` (`crates/loom-xtask/src/test_step.rs`): it builds the
  test binaries (`cargo test --workspace --locked --no-run`, outside the budget), then runs
  `cargo nextest run --workspace --locked --no-fail-fast` under the default profile of
  `.config/nextest.toml` (`slow-timeout = { period = "60s", terminate-after = 1 }`, `slow_` tests
  filtered out, a JUnit report), then `cargo test --doc --workspace --locked`. It prints the ten
  slowest tests with their times on every run, green or red, and exits non-zero when either run
  failed, a test took over `TEST_BUDGET` (60 s) or the step over `STEP_BUDGET` (120 s).
- The `ess` release every gate step and test runs is the one `ess/ess-inputs.yaml` pins (its line
  `requires: ess X.Y.Z`): `Taskfile.yml` and `Taskfile.commission.yml` set `ESS_TOOLCHAIN` from that
  line unless the caller set it, and `ess_suite::Ess::new` does the same for tests. Since
  2026-11-21 the `ess` on `PATH` can be a newer dispatcher that switches only when a pin lies at
  or above its working directory; tests run in crate directories, where it does not.
- CI installs cargo-nextest 0.9.143 from its release tarball, checked against a SHA-256 in
  `check.yml`, as it installs Task, `ess` and Helm.

## Why

At the base `8a8fad3b` every gate case calls `run_gate`
(`crates/loom-executor/tests/ess_gate.rs:69-107`), which synthesizes the tree it is given
(`:92`); `crates/loom-conformance/tests/conform.rs:203` and
`crates/loom-conformance/tests/adversary2_w2_conformance_status.rs:125` synthesize again, and
`adversary_ess_gate.rs` and `adversary2_ess_gate.rs` run the gate in replicas: about 46 synthesize
calls per test step. With `ess/domains/decision.yaml` (`story:decision-request-entity`) one call
takes 218 to 277 s; at `c1a1f443` it took 0.9 s; the decision domain alone takes 198 s. The step
`cargo test --workspace --locked` (`Taskfile.yml:36` at the base) ran 54 minutes, was stopped, and
held the machine's shared build pool. Nothing at the base kills a test: libtest has no timeout,
and no step measures its own time.

Runbook's code is not involved; this story holds Loom to `~/beyond10x/AGENTS.md` § Tests stay
fast, which names this defect as its cause and orders its fix before the next feature.

Forms chosen:

- A committed generated file with a digest of its inputs. Rejected: one `OnceLock` per test
  binary, which still synthesizes once in each of five binaries, about 20 minutes a step.
- cargo-nextest for the per-test kill. Rejected: a timer inside each test, which a new test can
  omit and which cannot stop a test blocked in a system call.
- The release from the pin line. Rejected: a private `ess` 0.96.0 first on `PATH`, which each
  machine and each session would set up by hand and CI would not.

## ESS first

No declaration under `ess/` changes. The unit's first commit is the generated suite with its
digest check, the gate still green.

`loom-ess-synthesis/1` is a new document format and gets its typed home in
`crates/loom-executor/tests/support/ess_suite.rs` (`Record`, `Record::parse`, `Record::render`),
shared with `loom-xtask` by `#[path]` (`crates/loom-xtask/src/main.rs:53-54`). Rejected: a
declaration under `ess/`, which would make a gate file part of the product's specification and
change the input digest it records.

## Acceptance

Times are wall time on a warm build of this machine, a wait for a build slot excluded.

- `slot task suite-generate` writes `generated/ess/loom/suite.json` and
  `generated/ess/loom/synthesis.txt`; the record's `ess:` line is `ess 0.96.0` and its `exit:` is
  `0`; `grep -c /home/ generated/ess/loom/*` prints `0` for each file. (One synthesis: minutes,
  not a test-step line.)
- `slot task slow` on the committed tree exits 0, and after a one-byte edit of `suite.json` it
  exits non-zero naming `generated/ess/loom/suite.json` and `task suite-generate`. (Slow lane.)
- `gate_holds_on_the_specification` (`crates/loom-executor/tests/ess_gate.rs`) passes reading the
  committed record, with no synthesize call.
- `gate_over_a_tree_whose_committed_suite_is_stale_fails_at_synthesize` (same file): a copy whose
  `ess/` differs from the recorded input digest fails at step 3 with `task suite-generate` in the
  message.
- `ess_conformance_report` (`crates/loom-conformance/tests/conform.rs`) reads the suite through
  `committed_suite` and keeps `COVER_TABLE` and `unexecuted_covers` as they are.
- `git grep -n '"synthesize"' -- crates/loom-executor crates/loom-conformance` prints one line,
  `crates/loom-executor/tests/support/ess_suite.rs` (`ess_suite::synthesize`), and every test
  that reaches it, `slow_…` cases aside, passes a copy without `domains/decision.yaml`. The
  Commission and intake tests synthesize `ess/commission/` and `ess/intake/`, which hold no
  decision domain.
- `gate_over_a_copy_synthesize_refuses_fails_at_synthesize` and
  `gate_over_a_copy_with_an_uncovered_refusal_fails_at_synthesize` run on copies without the
  decision domain, each under 5 s.
- A second `synthesize_once` call on the same input in one test process returns the cached
  result without starting `ess`: a case in `crates/loom-executor/tests/ess_gate.rs` with a stub
  `ess` program that counts its starts.
- `slow_gate_over_a_copy_missing_one_cover_fails_at_synthesize` and
  `slow_adversary2_w2_named_unsupported_scenarios_fail_task_conform_on_conformance_status` exist,
  `cargo nextest run --workspace --locked` (default profile) does not list them, and
  `cargo nextest list --profile slow --workspace --locked` lists both.
- `the_report_gives_each_test_its_binary_time_and_failure`
  (`crates/loom-xtask/src/test_step.rs`): the JUnit report gives each case its binary, time and
  failure.
- `a_test_over_60_s_fails_the_step_and_the_slowest_are_printed` (same file): a case of 61 s fails
  the verdict and the text lists the ten slowest with their times.
- `a_step_over_120_s_fails_and_a_red_run_fails` (same file): a step of 121 s fails, and so does a
  run with one failed case and a step under budget.
- A test that sleeps 70 s, added to a scratch copy of a crate, is killed by nextest at 60 s and
  counted failed: `cargo nextest run -p <that crate>` exits non-zero naming it `TIMEOUT`.
- `slot task check` runs `cargo run -q --locked -p loom-xtask -- test-step` in place of
  `cargo test --workspace --locked`, and `taskfile_check_runs_the_ess_gate` and
  `taskfile_check_runs_conform` still pass.
- `slot task ess-gate` and `slot task conform` each end within 60 s, measured twice.
- The test step ends within 120 s, measured twice; its output names the number of tests and
  binaries and the ten slowest. Every test over 5 s is named with its time in the unit's report.
- With an `ess` 0.97.1 dispatcher on `PATH` and no `ESS_TOOLCHAIN` in the caller's environment,
  `slot task spec` and `slot task ess-gate` exit 0, and the dispatcher's note names 0.96.0.
- Not steered by a copy: a copy whose `requires:` line names another release still runs the pin
  of the repository root, and `gate_over_a_copy_requiring_an_older_ess_fails_at_validate` fails
  it at step 1.
- Not steered by the caller: an `ESS_TOOLCHAIN` naming a release newer than the pin is refused by
  step 1 (`--strict-requires`), so the gate does not pass on a release the specification did not
  pin.
- Not seen by the fast gate: a hand edit of the `stdout:` or `exit:` lines of `synthesis.txt` is
  not covered by either digest; `task slow` finds it (byte compare), and AGENTS.md § Generated
  files says the file is never edited by hand.
- A stop between the two writes of `task suite-generate` (suite first, record second) leaves a
  suite whose digest differs from the record's, and `task ess-gate` fails naming
  `task suite-generate`.
- `check.yml` installs cargo-nextest 0.9.143 after `sha256sum --check --strict` of its tarball,
  and `cargo nextest --version` in the job prints `0.9.143`.
- AGENTS.md § Gate names the slow lane, when it runs (after every change under `ess/`, with
  `task suite-generate` before it) and the two budgets; § Generated files has the row for
  `generated/ess/loom/`.
- `slot task commission:conform` and `slot task commission:ess-gate` are measured once each and
  the times reported; either is changed only when it exceeds 60 s.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and
  `slot task check` exit 0.

## Out of scope

- The decision domain's own synthesis time and its guard `kind-not-principal`: they stay in
  `ess/domains/decision.yaml` as they are.
- Skipping the step for a planning-only pull request:
  `story:planning-only-pull-requests-skip-build`.
- `rust-toolchain.toml`, the runner's environment and the forge watch:
  `story:local-gate-equals-forge`. The move to Rust 1.100.0: `story:rust-1-100-toolchain`.
- `task drift` stays as it is.

## Scope

Read in the tree `loom-w1121g-u4` (branch `impl/w7-test-budget`, base `8a8fad3b`; head
`92342fc2` "gate: commit the synthesized conformance suite with the digest of its inputs", the
rest uncommitted) and at the base.

- **Primary surface:** the test step of `task check` and step 3 of `task ess-gate` — cited,
  `Taskfile.yml:36` at the base
- **Files (gate):** `Taskfile.yml` (root `env: ESS_TOOLCHAIN` 3-6, `check` running
  `loom-xtask -- test-step` at 41, `ess-gate`, `suite-generate`, `slow`) — cited;
  `Taskfile.commission.yml` (`env: ESS_TOOLCHAIN` 3-6) — cited; `.config/nextest.toml` — cited
- **Files (generated):** `generated/ess/loom/suite.json`, `generated/ess/loom/synthesis.txt` —
  cited
- **Files (tests):** `crates/loom-executor/tests/support/ess_suite.rs` (new),
  `crates/loom-executor/tests/ess_gate.rs`, `adversary_ess_gate.rs`, `adversary2_ess_gate.rs`,
  `crates/loom-conformance/tests/conform.rs`, `adversary2_w2_conformance_status.rs` — cited
- **Files (xtask):** `crates/loom-xtask/src/main.rs` (`SuiteGenerate`, `SuiteCheck` 115-124,
  227-256; `#[path]` module 53-54; `TestStep` 129, 261) — cited;
  `crates/loom-xtask/src/test_step.rs` (new, `mod test_step` at `main.rs:57`) — cited
- **Files (CI):** `.github/workflows/check.yml` (step `Install cargo-nextest`, after `Install
  helm`, 87-99) — cited
- **Files (docs):** `AGENTS.md` § ESS, § Gate, § Generated files; `CHANGELOG.md` (one `Changed`
  bullet); `website/docs/guides/run-the-checks.md` — cited
- **Symbols:** `ess_suite::{Ess, Record, pinned_release, inputs_digest, synthesize_once,
  committed, compare_fresh, generate, drop_decision}`; `test_step::{run, verdict, parse_junit,
  STEP_BUDGET, TEST_BUDGET}` — cited
- **Would collide with:** `story:planning-only-pull-requests-skip-build` and
  `story:local-gate-equals-forge` (`check.yml`, `Taskfile.yml`, `AGENTS.md` § Gate); any unit
  that changes `ess/` (it must run `task suite-generate` and commit the record) — cited
