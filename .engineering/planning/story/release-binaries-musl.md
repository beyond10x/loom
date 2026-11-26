---
format: aep.planning-md/3
id: story:release-binaries-musl
kind: story
status: draft
title: Every release carries a static x86_64-unknown-linux-musl b10x-loom beside the gnu one
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:release-binaries-glibc-floor
revision: 1
---
## Outcome

Every release carries a static `b10x-loom` for `x86_64-unknown-linux-musl` beside the gnu one:
`b10x-loom-<version>-x86_64-unknown-linux-musl.tar.gz` with its `.sha256`, a binary with no
program interpreter and no needed library, so it starts on any x86-64 Linux kernel Loom supports,
whatever its libc.

## Why

The operator, 2026-11-25, option "A": static musl archives beside gnu; measure before naming
either one the default download.

## Blocked on

Loom cannot build for musl at its substrate pin. `b10x-substrate-host` (rev `85da2e2b`,
`crates/loom-intake-slice/Cargo.toml`) calls `SYS_renameat2` in `rename.rs`, which the `libc` crate
does not define for musl. The fix is substrate's pull request 309. This story starts when Loom's
pin moves to a substrate commit that holds it; Loom neither patches around it nor vendors or forks
substrate.

## Design

1. A second job in `binaries.yml`, `binaries / x86_64-unknown-linux-musl`, with the gnu job's
   toolchain step (Zig and cargo-zigbuild by pinned version and digest) and
   `rustup target add x86_64-unknown-linux-musl`, runs `cargo zigbuild --release --locked -p
   b10x-loom-cli --bin b10x-loom --target x86_64-unknown-linux-musl`.
2. `loom-xtask static-binary "$BINARY"` (in place since `story:release-binaries-glibc-floor`)
   refuses a binary with `PT_INTERP` or `DT_NEEDED`; `linked-libraries` and `unsynced-store` run
   as on gnu.
3. It packs `b10x-loom-<version>-x86_64-unknown-linux-musl.tar.gz` and its `.sha256` into the same
   artifact `b10x-loom-<version>`; AGENTS.md § Releases step 6 attaches and verifies four files.
4. The job is written on the branch `feat/release-binaries-musl` and merged once the pin moves.
   It is not in `release.yml` or on a pull request before: a lane that always fails would be
   marked green by `continue-on-error` and say nothing.

## ESS first

No behaviour change in Loom's declared domains. The red step is a `release_check.rs` test naming
the musl job and its archive, failing before the job exists.

## Acceptance

- `release_check.rs` holds both jobs of `binaries.yml`, the musl archive name in the artifact,
  `static-binary "$BINARY"` before packing, and AGENTS.md § Releases attaching both archives.
- The pull request's `binaries / x86_64-unknown-linux-musl` run is green and prints
  `is static: no program interpreter and no shared library`.
- The binary starts in an image without glibc (`alpine`), measured once and recorded.

## Not in scope

Changing substrate; naming the default download (after both are measured); aarch64.

## Scope

- **Files:** `.github/workflows/binaries.yml`, `crates/loom-xtask/tests/release_check.rs`,
  `AGENTS.md` (§ Releases), `CHANGELOG.md`, `README.md`, `website/docs/getting-started.md`,
  `crates/loom-intake-slice/Cargo.toml` and `Cargo.lock` (the pin move, if it lands here) —
  inferred
