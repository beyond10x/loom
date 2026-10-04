---
title: Commission, Harness and Metaharness
sidebar_label: Commission and Harness
sidebar_position: 4
description: Where Loom sits between Commission, the Harness it succeeds, and Metaharness.
---

# Commission, Harness and Metaharness

## Commission

Loom depends on [Commission](https://beyond10x.github.io/commission/)'s core contracts and implements its
`AgentExecutor`. Commission loads the governed frontier, runs an executor, and decides what happens
with the outcome. Commission's core never depends on Loom, so other executors, such as a human tool
or a workflow executor, can sit in the same place.

## Harness, the predecessor

Loom succeeds the Beyond10x Harness. Harness's model wires, turn loop, tool round trips, approvals,
budgets, streaming, compaction and session transcripts are ported into Loom step by step, not
rewritten. Code carried over is relicensed Apache-2.0. Harness stays in service, unchanged, until
its consumers have moved.

Every Harness crate has one disposition. Six are ported into Loom: the wire, its HTTP transport, two
model-API projections, the loop, and session transcripts. Eight are not carried and stay with
Harness. The repository's
[Harness to Loom module map](https://github.com/beyond10x/loom/blob/main/docs/design/harness-map.md)
lists them.

:::caution[Planned]
The module map is done. The port itself has not started.
:::

## Metaharness

Metaharness evaluates and drives harnesses across cases. Loom is the native executor it compares
against other harnesses on the same case and evidence criteria — for example, Loom with a fast
selector against Loom with a reasoning selector.
