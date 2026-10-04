---
format: aep.planning-md/3
id: epic:effect-bindings
kind: epic
status: proposed
title: Connector and Substrate execution bindings
summary: Bind protocol actions to Connector operations and run effects through Substrate (L-014, L-015).
refs:
- provider: atlas
  reference: epic:ga-governed-effects
relations:
- depends_on: epic:fast-selector
- serves: vision:governed-autonomy
- serves: vision:O1
- serves: vision:O3
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-10-04T00:01:26Z", actor: "human:timo", revision: 2}
---
## Outcome

Consequential actions leave Loom only through a Connector operation run inside Substrate. Covers
TASKBOARD L-014 and L-015.

## Acceptance

One consequential action selected in Loom executes through a Connector operation inside Substrate
after a Mandate approval, and Loom holds no provider credential at any point (`rg -il -w 'token|secret'
crates/` shows no credential handling outside the adapter seams named in the story).

## Source

Atlas `epic:ga-governed-effects`.
