---
format: aep.planning-md/3
id: story:rust-toolchain-pinned
kind: story
status: implemented
title: rust-toolchain.toml pins the one rustc Loom's local gate and every workflow build with
summary: 'rust-toolchain.toml channel 1.99.1 (clippy, rustfmt, minimal); 6 jobs read it through a step id: toolchain into dtolnay/rust-toolchain; rust: images held to its release by a test in release_check.rs; AGENTS.md § Gate names the file'
tags:
- gate
relations:
- derived_from: story:local-gate-equals-forge
- serves: vision:O1
scope:
- confidence: cited
  path: .github/workflows/check.yml
- confidence: cited
  path: .github/workflows/deps-cache.yml
- confidence: cited
  path: .github/workflows/pages.yml
- confidence: cited
  path: .github/workflows/release.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: crates/loom-xtask/tests/release_check.rs
- confidence: cited
  path: rust-toolchain.toml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T14:09:20Z", actor: "agent:loom", revision: 3, correlation: "chore/rust-toolchain-pin"}
- {from: "proposed", to: "active", at: "2026-11-26T14:09:21Z", actor: "agent:loom", revision: 4, correlation: "chore/rust-toolchain-pin"}
- {from: "active", to: "implemented", at: "2026-11-26T15:03:00Z", actor: "agent:loom", revision: 5, decided_on: {"recorded":{"test_result":1}}, correlation: "chore/rust-toolchain-pin"}
---
## Outcome

The local gate and every forge job build with the one rustc `rust-toolchain.toml` names. A session
that runs `task check` in a tree gets that release without setting anything, and a workflow that
names another release, or builds in a `rust:` image of another release, fails a test.

- `rust-toolchain.toml` (new, at the root): `[toolchain]`, `channel = "1.99.1"`,
  `components = ["clippy", "rustfmt"]`, `profile = "minimal"`. No `targets`: every Linux job's
  target is the host, and the macOS job builds its own host target.
- Every job that installs Rust (`check.yml` jobs `check`, `docs-slow`, `macos`; `deps-cache.yml`
  job `deps`; `pages.yml` job `build`; `release.yml` job `verify`) first runs a step
  `id: toolchain` that reads `channel` from the file, fails unless it is `X.Y.Z`, and writes
  `channel=X.Y.Z` to `$GITHUB_OUTPUT`; `dtolnay/rust-toolchain` gets
  `toolchain: ${{ steps.toolchain.outputs.channel }}`. No workflow holds a Rust version literal.
- `binaries.yml`'s container and the builders of `deploy/images/*/Dockerfile` stay
  `rust:1.99-bookworm` by digest; rustup in them reads the file of the checkout or the build
  context, and a test holds their tag to the pinned release.
- AGENTS.md § Gate names the file as the pin and drops the sentences that said CI builds on
  `stable` and that a gate is run with `RUSTUP_TOOLCHAIN=<version>`.

## Why

`~/beyond10x/AGENTS.md` § The local gate equals the forge: `rust-toolchain.toml` pins one rustc,
and the local gate and CI both read it.

2026-11-26: on a host whose default toolchain is 1.100.0, `task check` in a fresh tree exited 201
in its test step: `no dep-info file under …/target/debug names a source of …: build the tests of
b10x-loom-cli first` (`crates/loom-xtask/src/shared_trees.rs`). CI installs 1.99.1 in every
workflow, as a literal in each job; nothing made a local tree build with it, and AGENTS.md § Gate
said both "Every workflow installs Rust 1.99.1" and "CI builds on `stable`".

This story is the toolchain part of `story:local-gate-equals-forge`, taken first because a local
gate does not run without it. `.cargo/config.toml`, the runner image names, `forge-parity` and
`forge-watch` stay there. The move to 1.100.0 stays in `story:rust-1-100-toolchain`.

## Acceptance

In `crates/loom-xtask/tests/release_check.rs`; each case reads the repository's files or runs
`bash` on a fixture under `CARGO_TARGET_TMPDIR`, under 1 s, with no network.

- `every_workflow_builds_with_the_rust_release_rust_toolchain_toml_pins`: `rust-toolchain.toml`
  pins an `X.Y.Z` channel; each of the six jobs above has a `dtolnay/rust-toolchain` step with
  `toolchain: ${{ steps.toolchain.outputs.channel }}` after a step `id: toolchain`; no step of any
  workflow passes another `toolchain`; no workflow holds `RUSTUP_TOOLCHAIN`, `rustup default`,
  `rustup override` or `cargo +`; every `rust:` image of a job's `container` and of a
  `deploy/images/*/Dockerfile` `FROM` line carries the pinned release or its `X.Y` in its tag.
- `the_toolchain_step_writes_the_pinned_release_or_fails`: every job's `toolchain` step is the same
  script; run by `bash` on a fixture pinning `1.99.1` it writes `channel=1.99.1` to
  `GITHUB_OUTPUT`; on `stable`, `1.99` and a file with no `channel` it exits non-zero and writes
  nothing.
- In a tree of the change, with no `RUSTUP_TOOLCHAIN` set, `rustc --version` prints 1.99.1 at the
  root and in `consumers/pinned/`, and `task test-step` exits 0.
- The pull request's checks are green.

## Scope

`rust-toolchain.toml` (new), `.github/workflows/check.yml`, `.github/workflows/deps-cache.yml`,
`.github/workflows/pages.yml`, `.github/workflows/release.yml`,
`crates/loom-xtask/tests/release_check.rs`, `AGENTS.md`. Not `binaries.yml` or the Dockerfiles:
their image already carries 1.99.
