---
format: aep.planning-md/3
id: decision-blocker:governor-restart-durability
kind: decision-blocker
status: cleared
title: Nobody has decided how epic:governor's restart durability is planned
relations:
- blocks: epic:governor
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-08T15:59:01Z", actor: "human:timo", revision: 3}
---
## Question

Its acceptance asks for a case kept across a process restart; in-process governor work is implemented (import-governor, hosted-governor, governor-evaluate). No story covers restart durability (`story:control-plane-storage` excludes a persistence provider).
| opt | does | cost |
|---|---|---|
| A | file `story:governor-restart-durability` under the epic; epic stays draft until it lands | 1 story to plan; epic open longer |
| B | narrow the epic's acceptance to in-process, move it to implemented; durability left unplanned | gap not tracked anywhere |
| C | archive the epic | loses the acceptance record |
Recommend A: the gap stays tracked and the epic closes on evidence.

## Decided

Option A (2026-10-08): `story:governor-restart-durability` is filed as a draft decomposing `epic:governor`; the epic stays open until it lands.
