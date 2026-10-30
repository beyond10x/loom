---
format: aep.planning-md/3
id: story:engineering-protocols-0-15-pin
kind: story
status: active
title: Loom builds on engineering-protocols 0.15.2
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T06:45:42Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-30T06:45:42Z", actor: "human:timo", revision: 3}
---
## Outcome

Every Loom crate on `b10x-canon-engineering` pins engineering-protocols `0.15.2`. The bundled and
engineering protocol catalogs list `protocol-adopt@1` beside the protocols 0.14.0 had, and the
tests that list the catalog name this story.

## Why

D264 ride-along. engineering-protocols 0.15.0 added `protocol.adopt/1` and moved to ESS 0.77.0;
0.15.1 and 0.15.2 reword `pr.open` in `story.delivery/1`, `/2` and the vocabulary to "through the
run's forge". Canon stays at 0.8.0 (the dependency lines of 0.15.2 are 0.14.0's).

## Acceptance

- `cargo test -p b10x-loom-protocols`, `-p b10x-loom-governor`, `-p b10x-loom-intake-router` and
  `-p b10x-loom-intake-slice` pass on 0.15.2; no Loom test or fixture holds the reworded bytes.
- `the_bundled_catalog_lists_eval_gated_change` and
  `the_catalog_lists_what_the_changelog_says_callers_now_see` list `protocol-adopt@1`.

## ESS first

None: a dependency pin with no behaviour change of Loom's own.

## Scope

- `crates/loom-{governor,intake-router,intake-slice,protocols}/Cargo.toml`, `Cargo.lock`
- `crates/loom-protocols/tests/catalog.rs`, `crates/loom-protocols/tests/adversary_w8_pin.rs`
- README.md, AGENTS.md, website pages naming the pin, CHANGELOG.md
