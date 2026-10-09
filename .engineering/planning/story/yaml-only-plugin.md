---
format: aep.planning-md/3
id: story:yaml-only-plugin
kind: story
status: draft
title: A Loom plugin defined only by YAML
tags:
- idea
relations:
- serves: vision:O1
- informed_by: story:plugin-measurement
- informed_by: architecture-decision-record:plugin-hooks
revision: 1
---
## Idea

Not scheduled. The operator, 2026-10-09: "file: idea / loom plugin \"just\" from yaml".

A Loom plugin whose hooks are all declared in one YAML document, with no Rust crate:

- **source**: a Connectors read operation, its cursor field and the rule for what is unanswered;
- **classifier**: intent labels and their descriptions;
- **projection**: intent to tools and data sources;
- **turn**: the turn's instructions and limits;
- **result**: the shape of the proposed effect.

`loom-plugin` interprets the document. A Rust plugin stays possible for what YAML cannot say.

## Order

It follows the plugin-layer wave (a plugin layer of hooks and a read-only Slack handler, approved by
the operator 2026-10-09) and that wave's measurement. It is the step after extracting the
generalizable parts of a plugin into a YAML protocol that Canon checks.

## Open

- Which hooks the plugin layer defines, and so which keys the document has: settled by the
  plugin-layer wave.
- Whether the document is an ESS-declared noun or a Canon protocol, and which one checks it.

## Related

`story:reference-enrichment` (automatic extraction of referenced data into the turn's context),
declared per reference kind in this document.
