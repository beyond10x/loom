---
title: Where Loom ends
sidebar_position: 5
description: Loom proposes an action; the Commission runtime rechecks it and invokes it through an effect port.
status: shipped
lede: Loom proposes an action; the Commission runtime rechecks it and invokes it through an effect port the embedder supplies.
source: Atlas ADR 0082, decided 2026-10-04, which amends ADRs 0070 and 0071; crates/loom-commission/src/ports/effect.rs, crates/loom-commission/src/runtime.rs
---

**Loom proposes. Commission invokes.** Every executor, Loom included, ends a step by returning a
`ProposedAction`: an action id and its arguments. Loom's side ends there. It makes no connector
call, holds no handle to connectors, and receives no provider credential just because a connection
exists.

```rust
pub enum ExecutorOutcome {
    ProposedAction(ExecutorOutcomeProposedAction),
    NeedsHumanJudgment(ExecutorOutcomeNeedsHumanJudgment),
    Suspended(ExecutorOutcomeSuspended),
    NoUsefulAction(Unit),
    CompletedLocalReasoning(Unit),
}
```

Immediately before the effect, the Commission runtime rechecks the frontier, the case revision and
authority. What it admits becomes an `AdmittedRequest`, which only the runtime can build, and goes
to the embedder's effect port:

```rust
pub trait EffectPort {
    fn performs(&self, action: &str) -> bool;
    fn invoke(
        &self,
        commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError>;
}
```

The alternative, Loom making the call itself, was rejected: it saves a hop, but every other
executor, such as a human tool or a workflow, would need its own effect path.

## What holds today

- The executor returns a `ProposedAction` and invokes nothing.
- An action that needs authority is proposed too. Loom never suspends for authority: the runtime
  rechecks the proposal and asks its authority provider.
- `run_until_blocked` invokes each admitted request once, and moves the case to a new revision only
  after an effect the port reports as `Performed`.
- The one effect port that exists is the slice's `LocalEffects`, which performs `software.change/1`
  actions in a git work tree. It never merges, pushes or deploys.

:::caution[Planned]
Effect ports for connectors and for Substrate are not decided.
:::
