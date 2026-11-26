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
revision: 2
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

Loom cannot build for musl at its substrate pin (rev `85da2e2b`, `crates/loom-intake-slice/Cargo.toml`).
Measured once in CI, 2026-11-26, on the throwaway branch `probe/release-binaries-musl` (run
45828038948, job `binaries / x86_64-unknown-linux-musl`, `cargo zigbuild --release --locked -p
b10x-loom-cli --bin b10x-loom --target x86_64-unknown-linux-musl`):
`error: could not compile b10x-substrate-host (lib) due to 13 previous errors`:

- E0425 `cannot find function renameat2 in module nix::fcntl` at `src/lib.rs:1114` and
  `src/quota.rs:219`, and E0433 `cannot find RenameFlags in fcntl` at `src/lib.rs:1119` and
  `src/quota.rs:224`: nix 0.31.3 gates `renameat2` on `target_env = "gnu"`.
- E0308 `mismatched types: expected i32, found u64` at `src/egress.rs:644, 715, 906, 1616, 1623,
  1625, 1676, 1694`: `libc::ioctl`'s request is a `c_int` on musl (`SIOCGIFFLAGS`,
  `NS_GET_USERNS`, …), and E0277 `can't compare u32 with usize` at `src/egress.rs:1694`.

Substrate's pull request 309 addresses the rename; the `ioctl` errors in `egress.rs` need a fix of
their own unless that pull request covers them. This story starts when Loom's pin moves to a
substrate commit that compiles for musl; Loom neither patches around it nor vendors or forks
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
