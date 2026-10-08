---
format: aep.planning-md/3
id: upstream-blocker:llm-catalog-model-port
kind: upstream-blocker
status: cleared
title: llm has not released its catalog-to-Model function
relations:
- blocks: story:cli-catalog-model-route
revision: 4
transitions:
- {from: "open", to: "cleared", at: "2026-10-08T08:50:28Z", actor: "human:timo", revision: 4}
---
## Waiting on

llm `story:catalog-model-port`: a function that turns an llm catalog file and a route alias into
an llm `Model`. Not in llm 0.3.1 (2026-10-07), the latest llm release on 2026-10-08.

## Clears when

An llm release tag ships the function; `story:cli-catalog-model-route` then pins that tag.

## Cleared

llm 0.4.0 (released 2026-10-08T08:35:19Z) ships it: `b10x-llm-models` `port` builds the `Model` a catalog serving model declares, and `CatalogModels` implements `llm_routing::Models` (release notes of https://github.com/beyond10x/llm/releases/tag/0.4.0). `story:cli-catalog-model-route` pins llm 0.4.0.
