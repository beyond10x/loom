---
format: aep.planning-md/3
id: upstream-blocker:llm-catalog-model-port
kind: upstream-blocker
status: open
title: llm has not released its catalog-to-Model function
relations:
- blocks: story:cli-catalog-model-route
revision: 2
---
## Waiting on

llm `story:catalog-model-port`: a function that turns an llm catalog file and a route alias into
an llm `Model`. Not in llm 0.3.1 (2026-10-07), the latest llm release on 2026-10-08.

## Clears when

An llm release tag ships the function; `story:cli-catalog-model-route` then pins that tag.
