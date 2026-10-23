---
format: aep.planning-md/3
id: approval-record:metaharness-0-15-0
kind: approval-record
status: approved
title: metaharness 0.15.0, released as Loom 0.39.0
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-23T15:23:18Z", actor: "human:timo", revision: 2}
---
# Approval: metaharness 0.15.0, released as Loom 0.39.0

## Decision

The orchestrator, under the owner's delegation (2026-10-23): "loom 0.39.0 is the metaharness 0.15.0
pin with a **Breaking:** bullet naming the re-exported types that change (RunSpec, FloorInputs,
Credential, AuditReport; lib.rs:101), same as loom 0.37.0 (CHANGELOG.md:96). The ESS 0.69.0 move
becomes loom 0.40.0 ... Go ahead with the loom release through its own process".

## Why the minor version

`crates/loom-session-executor/src/lib.rs:101` re-exports the crate (`pub use metaharness;`), so
`loom_sdk::session::metaharness` is metaharness itself. metaharness 0.14.0 and 0.15.0 break library
callers: `RunSpec` gains `api_key_secret`, `FloorInputs` gains `secret_key_source`,
`metaharness_b10x::Credential` gains `Secret`, `AuditReport` gains `child_exit`, and the pinned
version arrays change length. A `**Breaking:**` bullet moves the minor version (§ Releases).

## Scope

- `crates/loom-session-executor/Cargo.toml`: `metaharness` and `metaharness-b10x` at tag `0.15.0`
  together; `Cargo.lock` follows.
- AGENTS.md and README name `0.15.0`.
- The release commit: workspace version 0.39.0, CHANGELOG `[0.39.0]`, README and site pages.
- ESS stays at 0.68.0, the newest ESS release.
