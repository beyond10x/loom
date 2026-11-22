---
format: aep.planning-md/3
id: story:drop-the-substrate-git-feature
kind: story
status: proposed
title: Loom links Substrate without its git feature, so the binary needs no libgit2
summary: remove features = [git] from the host dependency; Cargo.lock loses git2, gix, curl; a lock-file test and an ldd check; a Git source stays refused
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:release-binaries
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-intake-slice/Cargo.toml
- confidence: cited
  path: crates/loom-intake-slice/tests/host_git_hardening.rs
- confidence: cited
  path: crates/loom-xtask/
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T05:46:20Z", actor: "agent:loom", revision: 5}
---
## Outcome

`b10x-substrate-host` is built without its `git` feature:

- `crates/loom-intake-slice/Cargo.toml:30` reads `default-features = false` with no `features`
  list.
- `crates/loom-intake-slice/Cargo.toml:31` already names no feature (`b10x-substrate-wire` has no
  features table at `adfde402`), so it is unchanged.
- `Cargo.lock` is regenerated and committed. It holds no `git2`, `libgit2-sys`, `gix`, `gix-*`,
  `curl`, `curl-sys`, `openssl-sys` or `libz-sys`.
- The built `b10x-loom` links no `libgit2`.
- Loom's behaviour does not change. Loom names no Git source, and a Git source that reaches the
  host driver is refused as before, now by Substrate's `git_unserved.rs`:
  `workspace.git-source-unserved`, "This build of the host driver has no `git` feature, so it
  serves no Git source."
- `crates/loom-intake-slice/src/git.rs` runs the `git` binary (`:276`) and uses no library. It
  stays as it is.

## Why

Today's debug `b10x-loom` links these system libraries, so a Linux archive from
`story:release-binaries` does not start on a machine that lacks any of them:

- `libgit2.so.1.9`, `libssh2.so.1`, `libllhttp.so.9.3`, `libpcre2-8.so.0`;
- `libzstd`, brotli;
- `libssl.so.3`, `libcrypto.so.3`, `libz.so.1`.

`b10x-loom-operator` does not link libgit2. Runbook's probe build needs the same libraries at its
link.

The libgit2 check, run against an export of the tree at `adfde402`, found:

- Loom's tree-run path does not use the feature. `crates/` names no `GitSourceBinding`,
  `git_sources`, `WorkspaceSource::Git`, `GitSource`, `GitChangeSet`, `git_baseline` or
  `create_workspace`.
- `driver_config` (`crates/loom-intake-slice/src/confinement.rs:343-350`) starts from
  `HostConfig::minimum(parent)` and sets no `git_sources`.
- Without the feature, Substrate compiles `git_unserved.rs` in place of `git.rs`, with the same
  signatures (`crates/substrate-host/src/lib.rs:22-27`, `git_unserved.rs:1-35` at `adfde402`).
- `b10x-loom-cli` type-checks without the feature, after the `OpenEffects::new` fix that is
  already merged.
- `Cargo.lock` goes from 541 to 456 packages.

## ESS first

Exempt: no noun or rule of Loom's specification changes. The first commit is red: the updated
lock-file test of § Acceptance fails while the feature is on. The second commit removes the
feature and regenerates `Cargo.lock`.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no network, no build
inside a test. The lock-file tests read `Cargo.lock` and `cargo metadata --locked --offline`
only.

- `every_host_git_command_goes_through_a_hardened_helper`
  (`crates/loom-intake-slice/tests/host_git_hardening.rs:444`) no longer exempts the pinned
  Substrate host:
  - the dependency walk at `:548-594` descends into `b10x-substrate-host` as well;
  - it asserts that no reachable package is `git2`, `libgit2-sys`, `gix` or `gix-*`;
  - the version and source asserted at `:575-581` stay until `story:substrate-pin-at-the-deny-list`
    moves them;
  - the comment at `:546-548` says Loom builds the host without its `git` feature.
- `Cargo.lock` names no package `git2`, `libgit2-sys`, `gix`, `gix-*`, `curl`, `curl-sys`,
  `openssl-sys` or `libz-sys`. A planted `name = "git2"` line in a copy of the lock is found.
  Test: `the_lock_file_holds_no_git_library_and_no_curl`.
- `loom-xtask linked-libraries <binary>` runs `ldd` on the binary. It exits non-zero, naming the
  library, when `ldd` names `libgit2`, `libssh2`, `libssl`, `libcrypto` or `libz`. It is written in
  Rust: no Python, by § No Python for permanent tools.
  - Its tests read recorded `ldd` output, one text from today's binary (refused, naming
    `libgit2.so.1.9`) and one without those libraries (passes), and run no `ldd`. Test:
    `linked_libraries_refuses_a_binary_that_links_libgit2`.
  - Running it on a real binary needs a build, so it runs outside the test step. Once
    `story:release-binaries` adds `binaries.yml`, it runs in that workflow after the build, and
    only the forge proves it there. Until then the unit runs it once on
    `target/release/b10x-loom` and puts the output in its report.
- A Git source is still refused on Loom's path:
  - `driver_config` returns no `git_sources`. Test: `the_driver_config_names_no_git_source`, a
    unit test in `confinement.rs`.
  - A host driver opened with Loom's `driver_config` plus one `GitSourceBinding` is refused
    `workspace.git-source-unserved`. Test: `a_git_source_on_loom_s_driver_is_refused_unserved`,
    gated like the other `linux` tests, which skip and name what is missing. Whether the refusal
    comes at open or at the first use is inferred, and the unit names which.
  - No existing Loom test covers this; the check found no `git_sources` in `crates/`.
- `CHANGELOG.md` `[Unreleased]` says that Loom builds Substrate's host without `git`, that the
  binary no longer links libgit2, libssh2, OpenSSL or zlib, and that a Git workspace source would
  need the feature back.
- Open, for the unit to prove and report, because the check did not establish it:
  `cargo test -p b10x-loom-intake-slice` passes without the feature. The unit runs the crate's
  tests that touch confinement and Git (`--test host_git_hardening --test confinement --test
  confined_child`) and reports any test that needed the feature.

## Out of scope

- A Git workspace source for Loom runs. That is a future feature, and it would add the `git`
  feature back.
- Vendoring libgit2. The check found that Substrate's workspace does not forward `vendored`, and
  vendoring was not asked for.
- Moving the Substrate pin. That is `story:substrate-pin-at-the-deny-list`. Both stories edit
  `Cargo.lock`, and this one goes first because it is smaller. The pin story keeps the feature off
  and updates the version and source lines of the lock-file test.

## Scope

- `crates/loom-intake-slice/Cargo.toml:30`. Cited.
- `Cargo.lock`, regenerated. Cited.
- `crates/loom-intake-slice/tests/host_git_hardening.rs`: `:444`, `:546-594`, `git_library` at
  `:630-647`. Cited.
- `crates/loom-intake-slice/src/confinement.rs`: `driver_config` at `:343-350`, plus the new unit
  test. Cited.
- `crates/loom-xtask/src/main.rs`: `Command` at `:88`, plus the new `linked-libraries` command.
  Cited; the new module's name is inferred.
- `CHANGELOG.md`. Cited.

Edges:

- depends_on `story:release-binaries`. The edge is soft: this story makes the release archive
  portable, and the `ldd` step goes into that story's workflow.
- Goes before `story:substrate-pin-at-the-deny-list`. Both change `Cargo.lock` and
  `host_git_hardening.rs`.
