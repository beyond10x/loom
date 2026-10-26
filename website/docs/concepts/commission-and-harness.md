---
title: Commission, Harness and Metaharness
sidebar_label: Commission and Harness
sidebar_position: 4
description: Commission's contracts and runtime inside Loom, the Harness Loom started from, and Metaharness, the layer below Loom that drives and compares harnesses.
status: shipped
lede: Commission's contracts and runtime live in Loom as their own crate, which never depends on the executor; Loom no longer carries a port of Harness and reaches models through llm; Loom is the governed runtime above Metaharness, which drives and compares the harnesses.
source: Atlas ADRs 0075, 0090, 0096 and 0098; Taskfile.commission.yml (deps-guard)
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

Loom started from [Harness](https://beyond10x.github.io/ecosystem/harness/)
([GitHub](https://github.com/beyond10x/harness)): releases up to 0.30.0 carried a port of its model
wires, turn loop and session transcripts. Loom deleted that port (Atlas ADR 0098). Harness stays in
service, unchanged, as the in-house harness behind Metaharness.

Loom's decisions (routing an intent, selecting an action, generating its arguments, a workflow's
judgement step) are each one structured request to an llm `Model`, through
[llm](https://beyond10x.github.io/llm/) ([GitHub](https://github.com/beyond10x/llm)). Agent work in
a harness goes through Metaharness behind an `AgentExecutor`.
[Move from Harness](../guides/move-to-loom.md#move-from-harness) says what a Harness program uses
instead.

## Metaharness

[Metaharness](https://beyond10x.github.io/metaharness/)
([GitHub](https://github.com/beyond10x/metaharness)) drives harnesses from outside and makes their
runs observable and comparable. It is the layer below Loom: Loom is the governed runtime, not a
harness Metaharness compares, and when a step needs agent work Loom runs a session through
Metaharness in Harness, Claude Code or Codex (Atlas ADRs 0096 and 0098; ADR 0096 superseded ADR 0071,
which had made Loom the native harness).
