---
format: aep.planning-md/3
id: decision-blocker:membership-guard-ess-056
kind: decision-blocker
status: cleared
title: Nobody has decided how frontier membership reaches conformance on ESS 0.56.0
relations:
- blocks: story:revalidation-membership-conformance
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-08T15:59:01Z", actor: "human:timo", revision: 3}
---
## Question

Needs an ESS membership guard over a list input (refused in 0.55.0 as ESS-SYNTH-003/004). Unchecked on 0.56.0.
| opt | does | cost |
|---|---|---|
| A | check `ess` 0.56.0 with one synthesis; supported → scope into the next wave; refused → `[NEED loom] ess` | minutes |
| B | take the story's second acceptance: fix the spec comment, executor test holds the rule; implement and close | rule stays outside conformance |
| C | leave draft | the `is_empty()` mutant stays green in `task conform` |
Recommend A.

## Decided

Option A (2026-10-08): ESS 0.56.0 was checked. With the outcome as `when_subject`, `action not in input.frontier_actions` and `contains(input.frontier_actions, action)` are both refused as ESS-SPEC-012 ("a predicate is either a comparison (`a.b == 0`) or a bare fact path"). The story stays draft behind `upstream-blocker:ess-list-membership-predicate`.
