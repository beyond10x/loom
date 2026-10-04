---
slug: /
title: Overview
sidebar_label: Overview
sidebar_position: 1
description: What Loom is, what it owns, and where it stops.
---

# Loom

Loom is the native agent harness. It makes a model behave like an agent for one bounded run, inside
a governed frontier. It implements the executor contract of
[Commission](https://beyond10x.github.io/commission/), and it ends at proposing an action.

```text
frontier          admissible actions, from Commission
   ↓
catalogue         what the model may see
   ↓
selector          picks one action id
   ↓
arguments         for that action only
   ↓
ProposedAction    Loom stops here
   ↓
Commission        rechecks, then invokes
```

A selector can be wrong about which admissible action is best. It cannot produce an action the
frontier does not contain.

:::caution[Bootstrap]
Loom is at the start of its life. No model is called yet. Each page says whether what it describes
is **shipped**, **decided design** or **planned**; the [status page](./status.mdx) lists what exists.
:::

## How a model does useful work right now

Commission answers why an agent is working, on whose behalf, against which case, and under which
governed frontier. Loom answers a narrower question: how this model performs useful work in this
run. Commission hands Loom a commission and a frontier; Loom hands back an outcome.

## What Loom owns

Prompt and context construction, model invocation and the turn loop, the model-visible action
catalogue, action selection, argument generation, tool round trips, streaming, compaction, sessions
and transcripts, turn, token, time and cost budgets, interruption and recovery, and the model-facing
side of approval and suspension.

## What Loom does not own

Protocol semantics, engineering-domain semantics, case truth, organizational authority, connector
credentials, final completion, and system conformance semantics. Those belong to the systems around
it.

## Two rules about trust

- **The model is not trusted context.** It never supplies identity, authority, case revision,
  trusted time, approval results or tenant context. Those come from the governed side of the run.
- **A trace is not evidence.** What Loom records about a run helps you understand and compare runs.
  It does not stand in for the evidence a case needs.

## Where to go next

- [One run, five steps](./concepts/one-run.mdx)
- [Action selection and argument generation](./concepts/action-selection.md), with the safety rules
- [The fast selector](./concepts/fast-selector.md)
- [Commission, Harness and Metaharness](./concepts/commission-and-harness.md)
- [Where Loom ends](./concepts/where-loom-ends.md)
- [The ESS specification](./reference/ess/index.mdx), generated from the repository

Loom has no command line of its own: `b10x-loom` is a Rust library that Commission drives.
