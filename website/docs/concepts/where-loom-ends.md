---
title: Where Loom ends
sidebar_position: 5
description: Loom proposes an action; the Commission runtime rechecks and invokes it.
status: decided
lede: Loom proposes an action; the Commission runtime rechecks and invokes it.
source: Atlas ADR 0082, decided 2026-10-04, which amends ADRs 0070 and 0071
---

**Loom proposes. Commission invokes.** Every executor, Loom included, ends a step by returning a
`ProposedAction`: an action id and its arguments. Loom's side ends there. It makes no connector
call, holds no handle to connectors, and receives no provider credential just because a connection
exists.

Immediately before the effect, the Commission runtime rechecks the frontier, the case revision and
authority, and then invokes the action through its binding. The alternative, Loom making the call
itself, was rejected: it saves a hop, but every other executor would need its own effect path.

```rust
pub enum ExecutorOutcome {
    ProposedAction(ExecutorOutcomeProposedAction),
    NeedsHumanJudgment(ExecutorOutcomeNeedsHumanJudgment),
    Suspended(ExecutorOutcomeSuspended),
    NoUsefulAction(Unit),
    CompletedLocalReasoning(Unit),
}
```

The Loom side holds today: the executor returns a `ProposedAction` and invokes nothing. An action
that needs authority is proposed too; Loom never suspends for authority, because Commission
rechecks the proposal and asks its authority provider. The Commission side of the invocation is
planned work in Commission, not shipped.
