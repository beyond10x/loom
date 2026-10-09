---
format: aep.planning-md/3
id: story:inbound-answer-protocol
kind: story
status: implemented
title: A read-only protocol answers an inbound item from data sources
relations:
- decomposes: epic:plugin-layer
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: crates/loom-protocols/src/lib.rs
- confidence: cited
  path: crates/loom-protocols/tests/inbound_answer.rs
- confidence: cited
  path: protocols/inbound-answer/1.yaml
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-09T07:13:32Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-10-09T07:13:32Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "active", to: "implemented", at: "2026-10-09T18:09:15Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1,"review_outcome":4,"verification":1}}}
---
## Why

The turn of a plugin is governed (ADR 0072): the model sees only frontier actions. Nothing in the
catalog answers an item from data sources; `system.query/1` reads the clock only.

## ESS first

The specification change is `protocols/inbound-answer/1.yaml` (Canon `protocol/1`, id
`inbound.answer`, revision 1, registered `inbound-answer@1`, as `system.query` is `system-query@1`;
`system-query` landed the same way, a protocol file rather than `ess/`). The red test is
`plugin_catalog_lists_inbound_answer`, failing until the catalog lists it.

## Acceptance

- Declarations: artifact `item`; evidence kinds `source_read`, `reply_proposed`, `reply_declined`;
  action `source.read` (`effect: read`, `requires: capability datasource.read`, may produce
  `source_read`); action `reply.propose` (`requires: capability reply.propose`, may produce
  `reply_proposed`; records a proposal, never sends); action `reply.decline` (may produce
  `reply_declined`); outcome `proposed` (a `reply_proposed` and at least one `source_read`);
  outcome `declined` (a `reply_declined`). Test: `protocol_declares_its_actions_and_outcomes`
  (reads the compiled IR and checks each name).
- It compiles with Canon. Test: `inbound_answer_compiles`.
- `ProtocolCatalog::plugins()` lists `inbound-answer@1`. Test: `plugin_catalog_lists_inbound_answer`.
- `ProtocolCatalog::bundled()`, which the router and `protocols list` read, does not. Test:
  `bundled_catalog_excludes_inbound_answer`.
- A fresh case's frontier offers no action whose effect is a write. Test: `frontier_offers_no_write`.
- A `reply_proposed` without a `source_read` is not `proposed`. Test: `proposed_needs_a_source_read`.
- With both it is `proposed`. Test: `proposed_with_a_read_and_a_proposal`.
- A `reply_declined` gives `declined`. Test: `declined_with_a_decline`.
- `reply.propose` and `reply.decline` exclude each other: after one, the other is blocked (a precondition on the opposite claim), so a case never has two legitimate outcomes. Test: `propose_and_decline_exclude_each_other`.
- `AGENTS.md` § Protocol composition records the exception: a plugin host admits on
  `ProtocolCatalog::plugins()`, while routing (and a plugin's task path) reads `bundled()`, so a
  plugin protocol is never routed to and a routed protocol is never run by a plugin turn.

## Scope

`protocols/inbound-answer/1.yaml` (new), `crates/loom-protocols/src/lib.rs`,
`crates/loom-protocols/tests/inbound_answer.rs` (new), `AGENTS.md`.

