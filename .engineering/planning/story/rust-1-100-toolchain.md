---
format: aep.planning-md/3
id: story:rust-1-100-toolchain
kind: story
status: draft
title: Loom's workflows and gate run on Rust 1.100.0; no test assumes Cargo's test-binary directory
summary: CI pinned to 1.99.1 for 0.58.0 after stable 1.100.0 broke three guard cases; found on pull request 163
tags:
- from-finding
relations:
- serves: vision:O1
revision: 1
---
## Outcome

Every workflow and the local gate run on Rust 1.100.0, and no test depends on where Cargo puts
another test's binary.

## Found

Found 2026-11-21 on pull request 163 (Loom 0.58.0). The workflows installed `toolchain: stable`;
stable became 1.100.0 on 2026-11-12, after the last run on `main` (2026-11-05). The required
check `loom / task check` failed on the release head `94f22ddd` with no change of Loom behind it
(run 44900979058, job 136836609802):

- Cargo 1.100.0 ran the test binary at
  `target/debug/build/b10x-loom-commission-testkit/<hash>/out/adversary2_executor_guard-<hash>`,
  a directory of its own. Under 1.99.1 every test binary of a build sits in `target/debug/deps/`.
- Three cases of `crates/loom-commission-testkit/tests/adversary2_executor_guard.rs` look for the
  newest `executor_port-<hash>` binary beside their own (`guard_binary`, `:23-47`) and panic at
  `:44`: "no executor_port test binary in …".
- `crates/loom-commission-testkit/tests/adversary_executor_guard.rs:33-49` has the same helper.
- `cargo test --workspace --locked` stopped at that binary, so what else differs under 1.100.0
  is not known: the tests after it in the run, and every other test that reads a path under
  `target/` or runs `current_exe()` (`crates/loom-commission/tests/adversary2_gate.rs:78`,
  `crates/loom-conformance/tests/adversary2_w2_conformance_status.rs:58` and others).
- `cargo fmt --all --check` and clippy with `-D warnings` passed under 1.100.0 in that run, and
  so did `loom / macos build`.

The release pinned every workflow to 1.99.1 (commit "ci: pin the Rust toolchain of every
workflow to 1.99.1" on the release branch), the toolchain its local gate ran on.

## Acceptance

- `check.yml` (both jobs), `release.yml` and `pages.yml` install 1.100.0 (or the stable release
  current when the story is taken, named by number), and AGENTS.md names that version.
- `cargo test --workspace --locked --no-fail-fast` under that toolchain: 0 failed, and the count
  of executed cases is not lower than under 1.99.1 on the same commit (both counts recorded).
- The two guard files find the `executor_port` test binary by a route that does not assume a
  shared directory (for example by asking Cargo: `cargo test --no-run --message-format=json`),
  and each has a case that fails when the binary is missing or older than its source, as today.
- Every other test the 1.100.0 run turns red is listed in the story's "As built" with its cause.
- `task check` passes locally under the same toolchain; `rustc --version` is in the gate record.

## Out of scope

- A `rust-toolchain.toml` in the repository (it would bind consumers' checkouts and every
  session's builds): a separate decision.
- `rust-version` in `Cargo.toml`.
