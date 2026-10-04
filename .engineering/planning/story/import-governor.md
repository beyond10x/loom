---
format: aep.planning-md/3
id: story:import-governor
kind: story
status: draft
title: The governor crate lives in Loom with history over one Commission
relations:
- decomposes: epic:runtime-consolidation
- depends_on: story:import-commission
revision: 1
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
