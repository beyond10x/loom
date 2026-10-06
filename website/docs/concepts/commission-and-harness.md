---
title: Commission, Harness and Metaharness
sidebar_label: Commission and Harness
sidebar_position: 4
description: Commission's contracts and runtime inside Loom, the Harness Loom succeeds, and Metaharness, which compares harnesses.
status: shipped
lede: Commission's contracts and runtime live in Loom as their own crate, which never depends on the executor; Loom carries the Harness it succeeds over by porting it; Metaharness compares Loom with other harnesses.
source: Atlas ADRs 0075 and 0090; Taskfile.commission.yml (deps-guard); docs/design/harness-map.md
---

## Commission

Commission answers why an agent is working, on whose behalf, against which case, and under which
governed frontier. Its contracts and runtime are `b10x-loom-commission`, in this repository since
Atlas ADR 0090; its former repository is archived.

Loom implements Commission's `AgentExecutor`. Commission's contracts never depend on Loom's
executor, on Canon or on a model-provider crate: `task commission:deps-guard` fails the gate if they
do. So other executors, such as a human tool, a workflow or a test fake, sit in the same place, and
Commission's specification (`ess/commission/`) has its own reference: the
[domain model](/docs/reference/commission/domain-model) and its
[value types](/docs/reference/commission/types).

## Harness, the predecessor

Loom succeeds [Harness](https://beyond10x.github.io/ecosystem/harness/)
([GitHub](https://github.com/beyond10x/harness)). Harness's model wires, turn loop, tool round trips,
approvals, budgets, streaming, compaction and session transcripts are ported into Loom step by step,
not rewritten. Code carried over is relicensed Apache-2.0. Harness stays in service, unchanged,
until its consumers have moved.

Every Harness crate has one disposition. Six are ported into Loom: the wire, its HTTP transport, two
model-API projections, the loop, and session transcripts. Eight are not carried and stay with
Harness. The repository's
[Harness to Loom module map](https://github.com/beyond10x/loom/blob/main/docs/design/harness-map.md)
lists them.

:::caution[Planned]
The provider wires, messages, responses, HTTP, the turn loop, sessions, transcripts and streaming
are ported as modules of `b10x-loom-executor`, and `Loom::run_loop` runs a governed run through
them: each turn's tools are the catalogue projected from the case's current frontier, and a model's
tool call is selected, given its arguments and revalidated before Loom proposes it. Each compaction
of the run's session is recorded with the usage the endpoint reported for it. Budgets, interruption
and recovery are planned, and `b10x-loom run` does not use the loop yet:
today the slice's model calls go through
[llm](https://beyond10x.github.io/llm/) ([GitHub](https://github.com/beyond10x/llm)).
:::

## Metaharness

[Metaharness](https://beyond10x.github.io/metaharness/)
([GitHub](https://github.com/beyond10x/metaharness)) drives harnesses from outside and makes their
runs observable and comparable. Loom is the native executor it compares against other harnesses on
the same case and evidence criteria, for example Loom with a fast selector against Loom with a
reasoning selector.
