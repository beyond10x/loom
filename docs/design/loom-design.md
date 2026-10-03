# Loom — Native Agent Harness

**Proposed repo:** `beyond10x/loom`  
**Rust crate:** `b10x-loom`  
**Predecessor:** current Beyond10x Harness  
**Relationship:** native/default executor for Commission

## Purpose

Loom is the batteries-included harness that makes a model behave like an agent for one bounded run.

It is deliberately narrower than Commission.

Commission answers:

> Why is this agent working, on whose behalf, against which case, and under which governed frontier?

Loom answers:

> How does this model perform useful work right now?

## Owns

- prompt/context construction;
- model API invocation;
- model turn loop;
- dynamic model-visible action catalogue;
- action selection strategy;
- action argument generation;
- tool round trips;
- streaming;
- compaction;
- session/transcript state;
- turn/token/time/cost budgets;
- interruption/recovery;
- model-facing approval/suspension mechanics.

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

Metaharness remains the cross-harness evaluator/driver.

It should be able to compare:

```text
Loom + Laya selector
Loom + reasoning selector
external coding harness
other model/runtime
```

against the same cases and evidence criteria.
