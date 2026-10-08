---
format: aep.planning-md/3
id: decision-blocker:control-plane-storage-remainder
kind: decision-blocker
status: cleared
title: Nobody has decided what story:control-plane-storage adds beyond story:hosted-governor
relations:
- blocks: story:control-plane-storage
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-08T16:00:51Z", actor: "human:timo", revision: 3}
---
## Question

`story:hosted-governor` already delivered `FallibleCaseStore` and trusted evaluation time; what remains (fallible run-lifecycle port, distinct execution identities, 4 named tests) is unassessed.
| opt | does | cost |
|---|---|---|
| A | run `aep:story-scoper` against main, rewrite the body to the remainder, propose it | 1 scoper run |
| B | archive as superseded by hosted-governor | the 4 named tests may be lost if not delivered |
| C | leave draft | stays unplaceable |
Recommend A.

## Decided

Option A (2026-10-08): the remainder beyond `story:hosted-governor` is scoped. `story:control-plane-storage` now covers only a fallible run start/suspend interface and its `LoopFailure` variant (`crates/loom-commission/src/runtime.rs`); case-store failure reporting and per-Loom identities are on main and not reclaimed. The story stays draft.
