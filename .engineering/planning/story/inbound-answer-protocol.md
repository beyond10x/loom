---
format: aep.planning-md/3
id: story:inbound-answer-protocol
kind: story
status: draft
title: A read-only protocol answers an inbound item from data sources
relations:
- decomposes: epic:plugin-layer
- serves: vision:O1
scope:
- confidence: cited
  path: crates/loom-protocols/src/lib.rs
- confidence: cited
  path: crates/loom-protocols/tests/inbound_answer.rs
- confidence: cited
  path: protocols/inbound-answer/1.yaml
revision: 3
---
## Why

The turn of a plugin is governed (ADR 0072): the model sees only frontier actions. Nothing in the
catalog answers an item from data sources; `system.query/1` reads the clock only.

## ESS first

The specification change is `protocols/inbound-answer/1.yaml` (Canon `protocol/1`, id
`inbound.answer`, revision 1, registered `inbound-answer@1`, the way `system.query` is
`system-query@1`). The red test is `plugin_catalog_lists_inbound_answer` in
`crates/loom-protocols/tests/`, failing until the catalog lists it.

## Acceptance

- The protocol declares artifact `item`, evidence kinds `source_read` and `reply_proposed`, action
  `source.read` (`effect: read`, may produce `source_read`), action `reply.propose` (may produce
  `reply_proposed`; records a proposal, never sends), outcome `proposed` (requires a
  `reply_proposed` and at least one `source_read`) and outcome `declined`. Test:
  `inbound_answer_compiles` (Canon compile).
- `ProtocolCatalog::plugins()` lists `inbound-answer@1`; `ProtocolCatalog::bundled()`, which the
  router and `protocols list` read, does not. Tests: `plugin_catalog_lists_inbound_answer`,
  `bundled_catalog_excludes_inbound_answer`.
- A fresh case's frontier offers `source.read` and `reply.propose` and no action whose effect is a
  write. Test: `frontier_offers_no_write`.
- With a `reply_proposed` evidence but no `source_read`, the case is not `proposed`. Test:
  `proposed_needs_a_source_read`.
- With both, the case is `proposed`. Test: `proposed_with_a_read_and_a_proposal`.

## Scope

`protocols/inbound-answer/1.yaml` (new), `crates/loom-protocols/src/lib.rs`,
`crates/loom-protocols/tests/inbound_answer.rs` (new).
