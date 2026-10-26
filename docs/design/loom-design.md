# Loom — Native Executor

**Repo:** `beyond10x/loom`  
**Rust crate:** `b10x-loom-executor`  
**Started from:** Beyond10x Harness, whose port Loom carried until 0.30.0 and removed in 0.31.0
(Atlas ADR 0098)  
**Relationship:** native/default executor for Commission

## Purpose

Loom makes a model act as an agent for one bounded run, one governed decision at a time: each
decision is a structured request to an llm `Model`, and agent work in a harness goes through
Metaharness.

It is deliberately narrower than Commission.

Commission answers:

> Why is this agent working, on whose behalf, against which case, and under which governed frontier?

Loom answers:

> How does this model perform useful work right now?

## Owns

- prompt/context construction;
- model API invocation, through llm;
- dynamic model-visible action catalogue;
- action selection strategy;
- action argument generation;
- budgets on its model requests;
- model-facing approval/suspension mechanics.

Until 0.30.0 Loom also owned a model turn loop, tool round trips, streaming, compaction,
session/transcript state and interruption/recovery, through its port of Harness. The port was
removed in 0.31.0 (Atlas ADR 0098): Loom runs no model loop of its own, and a session in a harness
is Metaharness's to run behind an `AgentExecutor`.

## Does not own

- generic protocol semantics;
- engineering-domain semantics;
- case truth;
- organizational authority;
- connector credentials;
- final completion;
- system conformance semantics.

## Commission integration

Loom implements Commission's executor contract.

```text
Commission
    loads governed Frontier
        ↓
Loom
    projects Frontier.actions to model-visible catalogue
        ↓
selector
    chooses action
        ↓
argument generator
    fills selected action's schema
        ↓
Commission/runtime
    revalidates current frontier + authority
        ↓
trusted adapter
    executes
```

## Action selection strategies

Loom should support:

```text
ReasoningModelSelector
FastTypedSelector
RuleSelector
HybridSelector
```

### Laya

Laya is a particularly good fit for `FastTypedSelector`.

Its role:

```text
candidate actions
    ↓
fast typed choice
    ↓
action + confidence
```

The reasoning model then generates action arguments.

This can reduce latency/cost where action selection is a narrow classification problem.

### Safety invariant

The selector only sees already-admissible candidates.

It cannot widen capability.

## More than tool selection

Loom may also use fast typed decisions for:

- model-tier routing;
- "does this require deep reasoning?";
- "does this need human review?";
- context-compaction strategy;
- routing between specialist subagents.

Again, these are optimizations inside an already-governed envelope.

## Metaharness

Metaharness drives and compares the harnesses (Claude Code, Codex, b10x-harness). Loom runs agent
work through it and is the governed runtime above it, not one of the harnesses it compares (Atlas
ADRs 0096 and 0098; ADR 0096 superseded ADR 0071, Loom as the native harness). Comparing Loom with
the Laya selector against Loom with a reasoning selector on the same cases is a trial's work, which
reads Loom's decision records beside the harness traces (Atlas ADR 0101).
