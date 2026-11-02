---
format: aep.planning-md/3
id: story:agent-run-without-loops
kind: story
status: draft
title: agent run runs a spec that declares no loops
relations:
- decomposes: epic:loom-agent-command-variant
revision: 1
---
## Outcome

`b10x-loom agent run` runs a spec that declares no `loops`, and a channel binding without `list`, `get` or `search`, since a run polls nothing.

## Why

Out-of-scope row of review-result:adv-w20261102w2-a-pass-1: `agent run` inherits the serve checks "nothing would poll" (`crates/loom-agent/src/serve.rs:837-839`), so a spec written only for scheduled runs is refused.

## Acceptance

- A spec with no `loops` given to `agent run --input` runs one turn and exits 0; `agent serve` still refuses it.
