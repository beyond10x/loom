---
format: aep.planning-md/3
id: story:mantle-placement
kind: story
status: draft
title: A Loom run can be placed on a remote Mantle node (later)
relations:
- decomposes: epic:effect-bindings
- informed_by: architecture-design:effect-isolation
revision: 1
---
## Outcome

A whole Loom run can be placed on a remote Mantle node through a Mantle agent kind for Loom.

## Why

architecture-design:effect-isolation, decision 8 (operator, 2026-10-05): wanted later, not part of
the confinement work.

## Open

Everything: the agent kind, the session contract and what Loom keeps local. Not to be scoped until
the operator asks for it.
