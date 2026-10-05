---
format: aep.planning-md/3
id: story:released-dependencies
kind: story
status: active
title: Loom builds on llm 0.1.7, canon-engineering 0.1.0 and the newest ESS; intake-model goes to llm
relations:
- decomposes: epic:runtime-consolidation
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: Cargo.toml
- confidence: inferred
  path: b10x.toml
- confidence: inferred
  path: crates/loom-cli
- confidence: inferred
  path: crates/loom-commission-conformance
- confidence: inferred
  path: crates/loom-governor
- confidence: inferred
  path: crates/loom-intake-model
- confidence: inferred
  path: crates/loom-intake-router
- confidence: inferred
  path: crates/loom-intake-slice
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-10-05T09:36:02Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-05T09:36:02Z", actor: "human:timo", revision: 3}
---
## Outcome

Loom builds on released dependencies only: llm `0.1.7`, `b10x-canon-engineering` `0.1.0` from
`beyond10x/engineering-protocols`, and the newest ESS release. `loom-intake-model` is gone; the
intake router and the CLI make their forced tool calls through llm's `b10x-llm-tool-call`, as
`epic:runtime-consolidation` says ("`intake-model` goes to llm").

## Why

- llm `0.1.7` ships `b10x-llm-tool-call` (the call-tool helper), live streaming, Codex renewal and
  HTTP timeouts; Loom pins `0.1.6`.
- Loom pins `b10x-els` at `ac7dd03` from `beyond10x/els`; the repository is now
  `beyond10x/engineering-protocols` and the crate `b10x-canon-engineering`, released as `0.1.0`
  (engineering-protocols `story:consumer-repin` names these consumers).
- Operator rule 2026-10-05: always the newest ESS.

## Acceptance

- Every `b10x-llm-*` dependency in the workspace names tag `0.1.7`; `cargo tree --locked -i
  b10x-llm-core` shows one copy.
- `cargo tree --locked -i b10x-els` fails with "did not match"; `cargo tree --locked -i
  b10x-canon-engineering` shows one copy, from tag `0.1.0` of engineering-protocols.
- `crates/loom-intake-model` no longer exists; no workspace crate depends on
  `b10x-loom-intake-model`; the intake router's classification and the CLI's model calls go through
  `b10x-llm-tool-call`, and every behaviour the removed crate's tests asserted is asserted against
  the new path (or its absence is named and justified in the unit report).
- ESS crates and the `ess` pin name the newest ESS release.
- `task check` exits 0, and the intake slice's offline test still stops at
  `ApprovalRequired (repository.merge)`.

## ESS first

None — no behaviour change; the dependency and module moves are covered by the existing tests.

## Not in scope

New model behaviour; the live qualification run; retiring the old repositories
(`story:retire-repositories`).
