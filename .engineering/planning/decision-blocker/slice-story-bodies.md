---
format: aep.planning-md/3
id: decision-blocker:slice-story-bodies
kind: decision-blocker
status: cleared
title: Nobody has decided how the three slice stories with stale Commission paths are rewritten
relations:
- blocks: story:software-change-slice
- blocks: story:incident-response-slice
- blocks: story:approval-suspend-resume-slice
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-08T16:00:51Z", actor: "human:timo", revision: 3}
---
## Question

`story:software-change-slice`, `story:incident-response-slice`, `story:approval-suspend-resume-slice` (epic:vertical-slices) still cite Commission-repository paths from before the move into Loom.
| opt | does | cost |
|---|---|---|
| A | re-scope each with `aep:story-scoper` and rewrite bodies against current crates; stay draft | 3 scoper runs |
| B | archive all 3 and re-file when epic:vertical-slices is next | history split |
| C | leave | waves cannot place them |
Recommend A.

## Decided

Option A (2026-10-08): the three slice stories are re-scoped with `story-scoper` to Loom paths (`ess/commission/`, `docs/commission/`, `crates/loom-*`); bodies and scopes rewritten, meanings unchanged. They stay draft.
