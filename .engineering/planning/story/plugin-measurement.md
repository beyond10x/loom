---
format: aep.planning-md/3
id: story:plugin-measurement
kind: story
status: implemented
title: A page measures the plugin layer and names what can become YAML
relations:
- decomposes: epic:plugin-layer
- serves: vision:O1
- depends_on: story:slack-plugin
scope:
- confidence: cited
  path: docs/qualification/plugin-layer-measurement.md
- confidence: cited
  path: docs/qualification/plugin-layer-measurement.sh
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-09T18:02:33Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":5}}}
- {from: "proposed", to: "active", at: "2026-10-09T18:02:33Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":5}}}
- {from: "active", to: "implemented", at: "2026-10-09T18:09:16Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"review_outcome":5}}}
---
## Why

The operator: "we see how much code this will produce... eventually extract the generlizable parts
and express in YAML protocol+canon". The deferred YAML work (epic § Deferred) starts from this
page.

## ESS first

No behaviour change; exempt.

## Acceptance

- `docs/qualification/plugin-layer-measurement.md` gives non-blank, non-comment lines per crate
  (`crates/loom-plugin/src`, `crates/loom-plugin-slack/src`, `crates/loom-connectors/src/cli.rs`),
  counted by a script `docs/qualification/plugin-layer-measurement.sh` the page names; per hook,
  the lines of the module that implements it. Check: `measurement_counts_match`, the script rerun
  on the merge commit prints the page's numbers (the coordinator runs it at the wave close and
  pastes the output).
- The page has one row per `.rs` file under `crates/loom-plugin/src`, `crates/loom-plugin-slack/src`
  and `crates/loom-connectors/src/cli.rs`, each marked mechanism (stays Rust) or policy (could be
  YAML), with the YAML shape a policy row would take. Check: `measurement_table_is_complete`, the
  script's `--files` mode lists the same files as the table (the coordinator runs it at the wave
  close).

## Scope

`docs/qualification/plugin-layer-measurement.md` (new), `docs/qualification/plugin-layer-measurement.sh`
(new).
