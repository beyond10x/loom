---
format: aep.planning-md/3
id: decision-blocker:mantle-placement-in-effect-bindings
kind: decision-blocker
status: cleared
title: Nobody has decided whether story:mantle-placement belongs to epic:effect-bindings
relations:
- blocks: story:mantle-placement
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-08T15:59:01Z", actor: "human:timo", revision: 3}
---
## Question

Connector path lands with this wave; Substrate binding (`story:substrate-execution-binding`) remains. `story:mantle-placement` is titled "(later)" and is held.
| opt | does | cost |
|---|---|---|
| A | keep the epic proposed; mantle-placement stays draft outside the epic's acceptance | none |
| B | archive mantle-placement | re-file later |
| C | put mantle-placement into the epic's next wave | Mantle work now |
Recommend A.

## Decided

Option A (2026-10-08): `story:mantle-placement` stays draft and no longer decomposes `epic:effect-bindings`; it is not scheduled.
