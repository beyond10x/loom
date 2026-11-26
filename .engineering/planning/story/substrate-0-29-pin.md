---
format: aep.planning-md/3
id: story:substrate-0-29-pin
kind: story
status: implemented
title: 'Loom pins Substrate 0.29.0: the exec-list members judged, the gnu floor at glibc 2.17, musl unblocked'
summary: =0.24.0 85da2e2b to =0.29.0 1a10a30d; exec_subpaths/exec_denied_paths sent empty and refused in the record; RunObservation non_exhaustive in a test fake
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:substrate-0-24-0-pin
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T21:11:26Z", actor: "human:operator", revision: 2, executor: "agent:loom", correlation: "substrate-0-29-musl"}
- {from: "proposed", to: "active", at: "2026-11-26T21:11:26Z", actor: "human:operator", revision: 3, executor: "agent:loom", correlation: "substrate-0-29-musl"}
- {from: "active", to: "implemented", at: "2026-11-26T21:44:52Z", actor: "human:operator", revision: 4, decided_on: {"recorded":{"test_result":1}}, executor: "agent:loom", correlation: "substrate-0-29-musl"}
---
## Outcome

Loom pins Substrate `0.29.0` and builds and passes its lanes on it.

- **The pin.** `b10x-substrate-host` and `b10x-substrate-wire` are at the tag `0.29.0`, commit
  `1a10a30d6856e6b4e20d55104638d26d37a1b992`, with `version = "=0.29.0"`, as before by `rev` and
  `version`, `default-features = false` on the host crate. `Cargo.lock` moves only the four
  Substrate packages (`-host`, `-wire`, `-guarded`, `-exec-gate`) from `0.24.0` to `0.29.0`; no
  other package moves.
- **Breaks.** Substrate's breaking bullets from `0.25.0` to `0.29.0` that reach Loom:
  - `ConfinementRequest` and `AppliedConfinement` gain `exec_subpaths` and `exec_denied_paths`
    (0.28.0, design 37). Loom sends neither (`None`, empty) and `check_record` refuses a request
    or a record that names either, beside `requires`; the record is still taken apart without
    `..`, so a member a later Substrate adds does not compile until it is judged.
  - `RunObservation` is `#[non_exhaustive]` (0.27.0, design 36): the test fake in
    `confinement/engine.rs` that loses a run's end now mutates the observation it got.
  - Not reached: the 64-entry `exec_subpaths` refusal (Loom sends no list), `App::new`'s
    `Result` (`b10x-substrate-daemon`, not a dependency), `RunLimit` and `AuditRecord`
    (Loom matches neither).
- **Guarantees.** No confinement guarantee Loom states changes meaning: the families and facts a
  driver proves (`a_driver_missing_a_family_is_refused_naming_it`), the roots, files, apertures,
  working directory and record checks all pass unchanged.
- **The glibc floor.** Substrate 0.29.0 makes `memfd_create` and `renameat2` as system calls,
  so the `x86_64-unknown-linux-gnu` release binary links at glibc 2.17
  (`glibc-floor --max 2.17` passes), and Substrate builds for `x86_64-unknown-linux-musl`,
  which unblocks `story:release-binaries-musl`.

## Why

`story:release-binaries-musl` waits for a Substrate that compiles for musl, and the gnu binary's
floor of 2.27 came only from `memfd_create`.

## For a consumer of Loom's SDK

`ChildResult::driver_record` and `TestExecution::driver_record` are Substrate's
`AppliedConfinement`, which is not `#[non_exhaustive]`: a struct expression or an exhaustive
pattern of it adds the two members. That is the one `**Breaking:**` bullet; the re-exported
`EgressAperture`, `ExecEnvironment`, `BaselineEnvironment`, `NetworkMode`, `SandboxProfile`,
`MAX_EGRESS_APERTURES` and `MAX_WORKDIR_BYTES` are unchanged.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no model call, no network.

1. `host_git_hardening.rs` `every_host_git_command_goes_through_a_hardened_helper` reads one
   each of `b10x-substrate-host`, `-wire` and `-guarded` at `0.29.0`, from `rev=1a10a30d…`.
2. `cargo nextest run -p b10x-loom-intake-slice` with `LOOM_REQUIRE_DELEGATION=1` passes.
3. `binaries / x86_64-unknown-linux-gnu` builds for `x86_64-unknown-linux-gnu.2.17` and
   `glibc-floor --max 2.17` passes in CI.

## Scope

- **Files:** `crates/loom-intake-slice/Cargo.toml`, `Cargo.lock`,
  `crates/loom-intake-slice/src/confinement.rs`, `crates/loom-intake-slice/src/confinement/`,
  `crates/loom-intake-slice/tests/host_git_hardening.rs`, `.github/workflows/binaries.yml`,
  `crates/loom-xtask/tests/release_check.rs`, `AGENTS.md`, `CHANGELOG.md`, `README.md`,
  `website/docs/getting-started.md`, `website/docs/reference/.generated.json`.
