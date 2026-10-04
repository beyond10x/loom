# Commission — Governed Agent SDK

**Proposed repo:** `beyond10x/commission`  
**Rust façade:** `b10x-commission`

Commission is the product name for the governed-agent SDK described earlier as the "Beyond10x Agent SDK."

Read the full historical design here:

[`../history/beyond10x-agent-sdk-design-pre-commission-name.md`](../history/beyond10x-agent-sdk-design-pre-commission-name.md)

## Naming update

The core abstraction is responsibility, not simply an agent.

A Commission binds:

```text
AgentRevision
Case
Protocol/Governor
Principal
Authority context
Responsibility
```

The most useful runtime nouns are:

```text
Agent
Case
Commission
Run
Session
Frontier
Outcome
```

`Assignment` may remain an implementation/API alias if useful, but product language should prefer `Commission`.

## Product line

> **Commission — Build agents you can give responsibility to.**

## Default executor

Commission's native/default executor is Loom.

Commission must still support alternative executors through a stable `AgentExecutor` contract.
