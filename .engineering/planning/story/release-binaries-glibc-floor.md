---
format: aep.planning-md/3
id: story:release-binaries-glibc-floor
kind: story
status: active
title: The x86_64-unknown-linux-gnu release binary starts on glibc 2.27 or newer, held there by a check
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T13:12:15Z", actor: "human:operator", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-26T13:12:15Z", actor: "human:operator", revision: 3}
---
## Outcome

The `x86_64-unknown-linux-gnu` binary a release carries starts on any Linux with glibc 2.27 or
newer (Ubuntu 18.04, Debian 10, RHEL 8 and later), not only on glibc 2.34 or newer. `binaries.yml` builds it with `cargo zigbuild` for
`x86_64-unknown-linux-gnu.2.27`, and `loom-xtask glibc-floor <binary> --max 2.27` refuses the
built binary before it is packed when it needs a newer `GLIBC_` symbol version, naming the
highest. The archive keeps its name, `b10x-loom-<version>-x86_64-unknown-linux-gnu.tar.gz`.

## Why

The operator, 2026-11-25, option "A": static musl archives beside the gnu one, and the gnu one built
against a glibc 2.17 floor with cargo-zigbuild, checked with `objdump -T`; measure before naming
either one the default download. Another repository's draft names the same failure: binaries
linked on a newer glibc fail on Debian bookworm.

Measured on the release assets: `0.60.0` and `0.61.0` need `GLIBC_2.34` (`objdump -T`, highest
version: `__libc_start_main`, `pthread_create` and others at 2.34), because `binaries.yml` links
them in `rust:1.99-bookworm`. README and the getting-started page said bookworm's 2.36.

**The floor is 2.27, not the 2.17 the operator named.** Measured on this story's pull request (run
45827283404): at `x86_64-unknown-linux-gnu.2.17` the link fails with `ld.lld: error: undefined
symbol: memfd_create`, referenced by `b10x-substrate-host` (`secrets.rs:254`, `exec_gate.rs:292`,
`libc::memfd_create`), which glibc has from 2.27. Every other symbol above 2.17 in the `0.61.0`
binary is weak (Rust's std probes it) or an older symbol under a newer version. A floor of 2.17
needs substrate to call `memfd_create` through `libc::syscall`; Loom does not patch substrate.

## Design

1. **Toolchain.** One step installs Zig and cargo-zigbuild, each by a pinned version and a pinned
   SHA-256 (`ZIG_VERSION`, `ZIG_SHA256`, `ZIGBUILD_VERSION`, `ZIGBUILD_SHA256`): `curl` to a file,
   `sha256sum --check --strict`, then `tar`. Zig from `ziglang.org/download/<v>/`, cargo-zigbuild's
   musl build from its GitHub release. No installer script, no package index, no Python.
2. **Build.** `cargo zigbuild --release --locked -p b10x-loom-cli --bin b10x-loom --target
   x86_64-unknown-linux-gnu.2.27` in the same container as before (the `loom-agent` image's
   builder, so the Rust toolchain is the image's). The binary is
   `target/x86_64-unknown-linux-gnu/release/b10x-loom`, the job's `BINARY`.
3. **Check.** `loom-xtask glibc-floor "$BINARY" --max 2.27`, after `linked-libraries` and
   `unsynced-store` and before packing. It reads the ELF in Rust (`binary_linkage.rs`): program
   headers, the dynamic section, `DT_VERNEED`; no `objdump` at run time and no new dependency
   (`object` is not in the lock). It fails above the floor and when no `GLIBC_` version is there.
   `static-binary <binary>` (no `PT_INTERP`, no `DT_NEEDED`) sits beside it for the musl archive
   (`story:release-binaries-musl`).
4. **Pull-request lane.** `binaries.yml` also runs on a pull request that touches `binaries.yml`,
   `release.yml`, the `loom-agent` Dockerfile, `Cargo.lock` or `crates/loom-xtask/**`. It packs the
   workspace version (a step that runs only on a pull request writes `VERSION` from
   `Cargo.toml`) and keeps the archive as a workflow artifact; nothing goes to a release.

## ESS first

No behaviour change in Loom's declared domains: a workflow, a repository check, tests and
documentation. The red step is the changed
`binaries_workflow_builds_read_only_and_keeps_the_archive_and_its_checksum`, failing on the
`cargo build` workflow.

## Acceptance

- `crates/loom-xtask/tests/release_check.rs`:
  `binaries_workflow_builds_read_only_and_keeps_the_archive_and_its_checksum` (zigbuild target
  `x86_64-unknown-linux-gnu.2.27`, `glibc-floor "$BINARY" --max 2.27` between the build and the
  packing, the pull-request paths, the upload name `b10x-loom-${{ env.VERSION }}`),
  `binaries_workflow_installs_zig_and_cargo_zigbuild_by_pinned_version_and_digest`,
  `binaries_workflow_reads_no_secret_uploads_to_no_release_and_puts_no_expression_in_a_shell`
  (GitHub is reached only for cargo-zigbuild's release files),
  `glibc_floor_and_static_binary_read_a_binary_the_host_linked`.
- `crates/loom-xtask/src/binary_linkage.rs`: the floor passes at 2.17, refuses 2.34 naming the
  highest, compares `2.3 < 2.10`; `static-binary` refuses an interpreter or a needed library and
  passes static and static-pie layouts; every prefix of a fixture reads without a panic.
- The pull request's `binaries / x86_64-unknown-linux-gnu` run is green, and its log prints
  `needs GLIBC_2.27 at most, within the glibc floor 2.27`.

All tests read files or fixtures built in memory; none builds a release binary, downloads or
calls GitHub.

## Not in scope

The musl archive (`story:release-binaries-musl`, blocked on the substrate pin); naming either
archive the default download; aarch64, macOS or Windows binaries; signing.

## Scope

- **Files:** `.github/workflows/binaries.yml`, `crates/loom-xtask/src/binary_linkage.rs` (new),
  `crates/loom-xtask/src/main.rs`, `crates/loom-xtask/tests/release_check.rs`, `AGENTS.md`
  (§ Checks that hold the rules, § Releases), `CHANGELOG.md`, `README.md`,
  `website/docs/getting-started.md` — cited
