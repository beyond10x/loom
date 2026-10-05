---
format: aep.planning-md/3
id: story:import-governor
kind: story
status: implemented
title: The governor crate lives in Loom with history over one Commission
relations:
- decomposes: epic:runtime-consolidation
- depends_on: story:import-commission
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: Cargo.toml
- confidence: inferred
  path: crates/governor
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-04T23:48:13Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-04T23:48:13Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-05T00:06:01Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
---
## Outcome

The governor crate (645 lines, governor `81fcc1e`) lives in Loom with its history and depends on
the imported Commission crates by path.

## Acceptance

- `crates/governor` in Loom, with history from `beyond10x/governor`.
- `cargo tree -i b10x-commission` in Loom shows one copy.
- The governor's tests pass in Loom; `task check` exits 0.
- `epic:governor` and its story exist in Loom's store, citing their old ids.

## Depends on

`story:import-commission`.

## Scope (inferred)

loom: `crates/governor`, `Cargo.toml`, `Cargo.lock`, `.engineering/`.
