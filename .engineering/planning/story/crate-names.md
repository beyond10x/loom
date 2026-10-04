---
format: aep.planning-md/3
id: story:crate-names
kind: story
status: draft
title: Loom's crates carry loom- names; no commission, governor or intake packages remain
relations:
- decomposes: epic:runtime-consolidation
- depends_on: story:runtime-merge
- depends_on: story:loom-cli
- depends_on: story:loom-sdk
revision: 1
---
## Outcome

Loom's crates carry `loom-` names that say what each holds: contracts, executor, runtime,
governor, intake, sdk, cli.

## Acceptance

- `cargo metadata --no-deps` lists only `loom-*` packages (plus `*-docs` and `*-xtask` tooling);
  every package name is published as `b10x-loom-*`.
- No package named `b10x-commission`, `b10x-governor` or `b10x-intake-*` remains.
- Generated ESS code is regenerated, not renamed by hand; drift check passes.
- `task check` exits 0.

## Open

Whether the contracts crate keeps the name `commission` (ADR 0090 point 3 allows it). Default:
`loom-contracts`.

## Depends on

`story:runtime-merge`, `story:loom-cli`, `story:loom-sdk`.

## Scope (inferred)

loom: every `crates/*/Cargo.toml`, `use` paths, `ess/` component names, docs.
