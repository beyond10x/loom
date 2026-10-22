---
format: aep.planning-md/3
id: story:agent-domain
kind: story
status: draft
title: 'An agent is data: ESS domain loom.agent declares AgentSpec and ToolPolicy'
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

An agent is data: ESS domain `loom.agent` (`ess/domains/agent.yaml`) declares `AgentSpec` (id,
name, brief, session kind, model route, tool policy, run budget, stop policy, confinement) and
`ToolPolicy` (allowed and denied tools, egress apertures, decision mode), generated into Rust and
exported as `loom_sdk::agent`.

## Why

Agency, loom-coder and swarm each need a typed agent below the product (ADR 0105); without one each
invents its own. Decided by the orchestrator, Q1 A
(`approval-record:loom-sdk-for-products-filing`).

`commission.responsibility.Agent` already exists
(`ess/commission/domains/responsibility.yaml:864-870`), so the design's "no noun for an agent" is
wrong: `AgentSpec` relates to it (an `AgentSpec` defines how an assigned `Agent` runs) and does not
shadow it. Run budget and stop policy reuse Commission's declared types.

## Acceptance

- `ess/domains/agent.yaml` validates, compiles and synthesizes with 0 refusals under `task ess-gate`.
- `task generate` writes `AgentSpec`, `ToolPolicy` and `DecisionMode` under `generated/rust/loom/`;
  `task drift` and `task no-hand-model` pass.
- `loom_sdk::agent` re-exports them; `crates/loom-sdk/tests/host_ports.rs` resolves the paths and
  `sdk-stability.md` lists them.
- A JSON `AgentSpec` with an unknown member is refused by the generated decoder (one unit test).

## ESS first

The first commit adds `ess/domains/agent.yaml` only; `task drift` is red until the generated code is
committed.

## Not in scope

Building an executor from a spec (`story:session-executor-from-spec`); product fields (owner, team,
workforce), which are Agency's.

## Scope

`ess/domains/agent.yaml`, `ess/system.yaml`, `generated/rust/loom/`, `crates/loom-sdk/src/lib.rs`,
`crates/loom-sdk/tests/host_ports.rs`, `website/docs/reference/sdk-stability.md`,
`website/docs/reference/ess/` (generated), AGENTS.md, CHANGELOG.md.

## Ready

Ready now.
