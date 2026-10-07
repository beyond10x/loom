---
format: aep.planning-md/3
id: decision-blocker:read-action-effect-path
kind: decision-blocker
status: cleared
title: Nobody has decided whether read actions take the same binding and Substrate path as consequential ones
refs:
- provider: commission
  reference: decision-blocker:read-action-effect-path
relations:
- blocks: story:effect-invocation
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-07T00:06:18Z", actor: "human:timo", revision: 3, executor: "agent:loom"}
---
> Re-filed from `beyond10x/commission` `decision-blocker:read-action-effect-path` at `e61e4f0` (status there: `open`) under Atlas ADR 0090
> (loom `story:import-commission`). Paths below are Commission's: `ess/` is now `ess/commission/`,
> `docs/` is now `docs/commission/`.

## Question

Do read actions (`metrics.inspect`, `logs.search`) take the same path as consequential ones
(recheck, then invocation through the action's binding, Connectors inside Substrate), or a lighter
one, and if lighter, which steps does it skip?

## Source

Atlas ADR 0082 § Open (operator decision of 2026-10-04): "Whether read actions take the same binding
and Substrate path or a lighter one. The Loom stories cover only consequential actions."

## Relation

Read action -> effect path. UNMAPPED: no ESS document in this repository distinguishes read from
consequential actions, and nothing in the store says whether a frontier action carries that
distinction.

## What it stops

`story:effect-invocation`: whether its command and outcomes apply to every admitted action or only
to consequential ones, and whether its acceptance needs a read-action case.

## Clears when

An accepted decision says whether read actions take the binding and Substrate path, and the
Commission ESS domain states it.

## Decision (2026-10-07)

Option A. Read actions (`metrics.inspect`, `logs.search`, `repository.inspect`) take the same path as
consequential ones: the recheck against frontier, case revision and authority, then the invocation
through the action's binding. No read/consequential distinction is declared, and none is added to a
frontier action. `story:effect-invocation`'s command and outcomes apply to every admitted action.

Options B (reads skip Substrate) and C (reads skip the Commission effect port) are not taken: each
adds a second rule or a second path beside the one recheck every effect passes.
