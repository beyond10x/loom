---
format: aep.planning-md/3
id: decision-blocker:g6-first-consumer
kind: decision-blocker
status: cleared
title: Nobody has decided which consumer Loom serves first
relations:
- blocks: epic:loom-foundation-for-consumers
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-14T02:50:21Z", actor: "human:timo", revision: 3}
---
## Question

Which consumer does the epic serve first, and which of its repositories is the integration point?

## Options

- A. The engineering factory (consumer C) first: it runs the software-change slice Loom ships.
- B. The agent platform (consumer A) first: it needs the plugin host and stable SDK ports first.
- C. Both at once: two integration points, two sets of breaks per release.

Recommendation: A.

## Decision (2026-10-14)

A. The engineering factory is served first; the agent platform waits. The integration point inside this organisation is the control-plane repository, the local proof whose results move to the factory; stories integrate against it.
