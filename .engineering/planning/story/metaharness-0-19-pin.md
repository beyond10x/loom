---
format: aep.planning-md/3
id: story:metaharness-0-19-pin
kind: story
status: active
title: Loom builds against metaharness 0.19.0, which admits harness 0.20.0
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: inferred
  path: README.md
- confidence: cited
  path: crates/loom-session-executor/Cargo.toml
- confidence: inferred
  path: crates/loom-session-executor/src
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:55:12Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-26T13:55:12Z", actor: "human:timo", revision: 5}
---
## Outcome

Loom builds against metaharness 0.19.0 (tag `0.19.0`, `e1b0e36c`), which pins `b10x-harness`
0.20.0, so a consumer that gets metaharness through Loom (takt) admits harness 0.20.0. The harness
revision in `Cargo.lock` follows from metaharness; Loom names no harness pin of its own.

## Why

The orchestrator, 2026-10-26: "loom moves its metaharness pin from 0.15.0 to 0.19.0 (released
2026-10-26, tag e1b0e36c; it pins harness 0.20.0 and llm 0.15.0), and whatever H9/harness pin loom
carries follows. Reason: takt's wave (running now) needs harness 0.20.0, which metaharness 0.19.0
admits (PINNED_VERSIONS [\"0.20.0\"]); takt gets metaharness through loom-sdk, so the move is
loom's." Class C dependency upgrade under the grant; released with the wave.

## Acceptance

- `crates/loom-session-executor/Cargo.toml` names `metaharness` and `metaharness-b10x` at tag
  `0.19.0`; `Cargo.lock` resolves both to `e1b0e36c` and `b10x-harness` to the 0.20.0 revision
  (`684c99f5`).
- `cargo test -p b10x-loom-session-executor --locked` passes; the full `task check` passes.
- Any adaptation to metaharness's breaking changes between 0.15.0 and 0.19.0 is the smallest that
  compiles and keeps the suite's meaning; a larger break is filed as its own story instead.
- CHANGELOG.md `## [Unreleased]` bullet; README/AGENTS.md dependency lines updated where they name
  the metaharness version.

## Scope

Derived 2026-10-26 by the coordinator from `5642e87e`.

- `crates/loom-session-executor/Cargo.toml`, `Cargo.lock` — cited
- `crates/loom-session-executor/src/` — inferred (only if 0.19.0 changed an API it uses)
- CHANGELOG.md, README.md, AGENTS.md — inferred
