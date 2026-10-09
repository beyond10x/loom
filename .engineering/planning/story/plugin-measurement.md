---
format: aep.planning-md/3
id: story:plugin-measurement
kind: story
status: draft
title: A page measures the plugin layer and names what can become YAML
relations:
- decomposes: epic:plugin-layer
- serves: vision:O1
- depends_on: story:slack-plugin
scope:
- confidence: cited
  path: docs/qualification/plugin-layer-measurement.md
revision: 3
---
## Why

The operator: "we see how much code this will produce... eventually extract the generlizable parts
and express in YAML protocol+canon". The deferred YAML work (epic § Deferred) starts from this
page.

## ESS first

No behaviour change; exempt.

## Acceptance

- `docs/qualification/plugin-layer-measurement.md` gives non-blank, non-comment lines per crate
  (`loom-plugin`, `loom-plugin-slack`, the `cli` module of `loom-connectors`) and per hook, with
  the command that counted them and its output.
- The page has a table with one row per Rust module of those crates, each marked mechanism (stays
  Rust) or policy (could be YAML), with the YAML shape a policy row would take. A check compares
  the table's module list with `find crates/loom-plugin crates/loom-plugin-slack -name '*.rs'`
  and the page states that both lists are equal.

## Scope

`docs/qualification/plugin-layer-measurement.md` (new).
