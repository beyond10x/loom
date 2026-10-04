---
format: aep.planning-md/3
id: story:import-intake
kind: story
status: draft
title: Intake's router, references, slice and CLI live in Loom; its model helper in llm
relations:
- decomposes: epic:runtime-consolidation
- depends_on: story:import-commission
- depends_on: story:import-governor
revision: 1
---
## Outcome

Intake's router, references, slice and CLI live in Loom with their history, built against Loom's
own executor, Commission and governor crates by path. Intake's model helper lives in llm.

## Acceptance

- `crates/intake-router`, `crates/intake-references`, `crates/intake-slice`, `crates/intake-cli`
  in Loom, with history from `beyond10x/intake`.
- `intake-model`'s `call_tool` and Codex preset are in an llm client crate, released by tag; Loom's
  intake crates use that tag.
- Intake's offline slice test passes in Loom and still ends `ApprovalRequired (repository.merge)`.
- Intake's `ess/` domain is part of Loom's ESS input; `ess specify validate` passes.
- `docs/qualification/2026-10-04-live-run.md` is in Loom's `docs/`.
- `task check` exits 0.

## Depends on

`story:import-commission`, `story:import-governor`.

## Scope (inferred)

loom: `crates/intake-*`, `ess/`, `docs/qualification/`, `Cargo.toml`, `Cargo.lock`. llm: one client
crate, a release.
