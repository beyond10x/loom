---
title: Where Loom ends
sidebar_position: 5
description: Loom proposes an action; the Commission runtime rechecks it and invokes it through an effect port.
status: shipped
lede: Loom proposes an action; the Commission runtime rechecks it and invokes it through an effect port the embedder supplies.
source: Atlas ADR 0082, decided 2026-10-04, which amends ADRs 0070 and 0071; crates/loom-commission/src/ports/effect.rs, crates/loom-commission/src/runtime.rs, crates/loom-commission/src/ports/connector.rs, crates/loom-connectors/src/lib.rs
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
    CaseMoved(ExecutorOutcomeCaseMoved),
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
- The slice's `LocalEffects` performs `software.change/1` actions in a git work tree. It never
  merges, pushes or deploys.
- Commission's `ConnectorEffects` performs the actions a commission binds to Connector operations,
  through a `ConnectorInvoker`. `b10x-loom-connectors` (`loom_sdk::connectors`) fills that port
  over a Connectors service, pinned at Connectors `v0.35.0`. The host declares one
  `ConnectorEndpoint` per Connector instance (`ess/commission/domains/responsibility.yaml`): its URL,
  a credential reference its own resolver turns into the service credential, and whether plain
  `http` is admitted. `ConnectorsInvoker` describes the endpoint, invokes the bound operation once
  on `POST /v1alpha2/invoke`, never falls back and never resends.

| Connectors answers | The invoker answers |
|---|---|
| success, with the attempt it recorded | `Performed`, naming that attempt |
| an error whose recorded attempt is `refused` or `not_attempted` | `Refused` |
| anything else: a success naming no attempt, an error without a recorded attempt, an unknown outcome, a protocol or transport failure | `Err` |

An instance with no declared endpoint, a credential that does not resolve, and a service that
describes another instance are `Err` before the operation is invoked.

:::caution[Planned]
An effect port for Substrate is not decided. Whether a released Connectors provider offers the
file-edit and test-run operations a coding phase needs is not known yet.
:::
