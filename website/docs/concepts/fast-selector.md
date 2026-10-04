---
title: The fast selector
sidebar_position: 3
description: A fast typed-decision model as one implementation of ActionSelector, gated by confidence.
---

# The fast selector

:::caution[Planned]
No fast selector exists yet. This page describes the design.
:::

A fast typed-decision model fits the selector role well: given the candidate actions, it returns one
of them with a probability. The reasoning model then generates arguments for that action. Where
selection is a narrow classification, this can reduce latency and cost.

The fast selector is one implementation of `ActionSelector`. It is never the governor, the
authority, the argument generator or the executor.

```text
frontier candidates           derived upstream; admissible only
        ↓
compact selection context
        ↓
fast typed choice             action id + probability
        ↓
confidence gate
   ├── high → reasoning model generates arguments → schema validation
   └── low  → reasoning-model selector, or full planner
        ↓
ProposedAction → Commission rechecks revision, frontier, authority → effect
```

## Confidence policy

An example policy:

| selector probability | example policy |
|---|---|
| `p ≥ 0.90` | Accept the fast selection. |
| `0.60 ≤ p < 0.90` | Use the reasoning-model selector instead. |
| `p < 0.60` | Use the full planner, ask for clarification, or do nothing. |

:::note[These thresholds are an example]
Exact thresholds must be calibrated per domain and measured by Metaharness. A high probability
changes which path picks the action; it never changes what the action is allowed to do.
:::

## Large catalogues

A large action set can be split into a hierarchical choice: first a tool family, then a specific
action. Every level of that hierarchy contains only frontier-admissible actions.

## No dependency on one vendor

The selector abstraction is meant to support local inference, hosted inference, other typed
decision models and deterministic selectors. Loom must not depend semantically on one selector
vendor or model. The repository's
[integration notes](https://github.com/beyond10x/loom/blob/main/docs/integrations/laya-fast-selection.md)
describe the first candidate.
