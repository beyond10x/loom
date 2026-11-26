---
format: aep.planning-md/3
id: story:release-binaries-musl
kind: story
status: active
title: Every release carries a static x86_64-unknown-linux-musl b10x-loom beside the gnu one
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:release-binaries-glibc-floor
- depends_on: story:substrate-0-29-pin
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T21:11:26Z", actor: "human:operator", revision: 3, executor: "agent:loom", correlation: "substrate-0-29-musl"}
- {from: "proposed", to: "active", at: "2026-11-26T21:11:27Z", actor: "human:operator", revision: 4, executor: "agent:loom", correlation: "substrate-0-29-musl"}
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

Unblocked by `story:substrate-0-29-pin`: Substrate `0.29.0` builds `b10x-substrate-host` for
`x86_64-unknown-linux-musl` (`renameat2` as a system call, `ioctl` requests typed per C library).

Before, measured once in CI, 2026-11-26, on the throwaway branch `probe/release-binaries-musl`
(run 45828038948): at rev `85da2e2b` `b10x-substrate-host` did not compile for musl (nix 0.31.3
gates `renameat2` on `target_env = "gnu"`; `libc::ioctl`'s request is a `c_int` on musl).

## Design

1. A second job in `binaries.yml`, `binaries / x86_64-unknown-linux-musl`, with the gnu job's
   toolchain step (Zig and cargo-zigbuild by pinned version and digest) and
   `rustup target add x86_64-unknown-linux-musl`, runs `cargo zigbuild --release --locked -p
   b10x-loom-cli --bin b10x-loom --target x86_64-unknown-linux-musl`.
2. `loom-xtask static-binary "$BINARY"` (in place since `story:release-binaries-glibc-floor`)
   refuses a binary with `PT_INTERP` or `DT_NEEDED`; `linked-libraries` and `unsynced-store` run
   as on gnu.
3. It packs `b10x-loom`, `LICENSE` and the `COPYRIGHT` of the musl Zig linked in (as
   `musl-COPYRIGHT`) as `b10x-loom-<version>-x86_64-unknown-linux-musl.tar.gz` with its `.sha256`,
   kept as the workflow artifact `b10x-loom-<version>-x86_64-unknown-linux-musl` (two jobs cannot
   upload one artifact name); AGENTS.md § Releases step 6 fetches both artifacts and attaches and
   verifies four files.
4. The job runs wherever the gnu job runs: in `release.yml`'s `binaries` call and on a pull
   request that touches the release build. It landed with `story:substrate-0-29-pin`.
5. The documented download stays gnu unless the measurement (both binaries on one CI runner:
   build time, binary and archive size, `--help` and `protocols list` timed) says otherwise.

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
