---
format: aep.planning-md/3
id: decision-blocker:action-family-membership
kind: decision-blocker
status: open
title: Nobody has decided where an action's tool family comes from or how many families an action has
refs:
- provider: taskboard
  reference: L-010
relations:
- blocks: epic:fast-selector
revision: 2
---
## Question

Where does an action's tool family come from, and what is the relation between a family and its
actions: does every action belong to exactly one family, may an action belong to none, and may one
action appear under more than one family? Is a family declared by the protocol, carried on the
frontier action, or derived by Loom (for example from the dotted prefix of the action id)?

## Why it is open

Atlas ADR 0073 allows "hierarchical selection (family → action)" and
`docs/integrations/laya-fast-selection.md` says the hierarchy "must only contain frontier-admissible
actions", but no ess/1 document declares a family, and the frontier action Loom receives carries only
an id and a status (inferred from Canon `crates/canon/src/lib.rs:41-44` at `cf29c4b`, the revision
Loom's `Cargo.lock` pins; no ess/1 document declares this). The size at which hierarchy is used
(">~20 actions", `docs/contracts/loom-action-selection.md` safety rule 6) is an approximation, not a
decision.

## What it stops

TASKBOARD L-010, hierarchical tool-family selection. No story was drafted for it: the family noun
needs an ESS home (planning guardrail 7) and every way of grouping actions answers this question.

## Source

Decomposition of `epic:fast-selector`; Atlas ADR 0073; `docs/contracts/loom-action-selection.md`.
