---
format: aep.planning-md/3
id: story:agent-a2a
kind: story
status: draft
title: 'A served agent answers A2A: its card, one turn per message, and a refusal without the bearer'
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:agent-serve
- depends_on: story:runtime-events
revision: 1
---
## Outcome

A served agent whose spec declares `expose.a2a` answers A2A: an agent card listing the spec's
skills, `message/send` running one turn of the same plugin and returning its record, task status
from `RuntimeEvent`, and `/ready` and `/live` for the pod's probes. A request without the bearer
from the agent's Secret is refused. Milestone M5 of the Kubernetes design, in a new adapter crate
`loom-a2a`.

## Why

Q7 A (orchestrator): Loom owns the adapter, over a2a's generated server pinned by tag
(`a2a/generated/rust/a2a/src/bin/a2a-agent-server.rs` in the a2a checkout), until a2a ships a
hand-written server crate Loom links.

## Acceptance

- `agent_card_lists_the_spec_skills`.
- `a_message_runs_one_turn_and_returns_its_record`.
- `an_unauthenticated_request_is_refused`, and a wrong bearer the same way.
- Task status follows the turn's `RuntimeEvent`s.
- Tests on a loopback port with recorded model answers; no network.
- (manual) a local Claude Code session reaches it through a port-forward and gets a sourced answer.

## ESS first

The first commit adds the A2A scenarios to `ess/domains/agent.yaml` only; they are red until
`loom-a2a` lands.

## Not in scope

Mandate authentication (later); the MCP read proxy (M6, waits on Connectors' inbound MCP server).

## Scope

`crates/loom-a2a` (new), `crates/loom-agent` (serve wires the listener), `Cargo.toml` (a2a pin),
`ess/domains/agent.yaml`, README.md, AGENTS.md, CHANGELOG.md.

## Ready

After `story:agent-serve` and `story:runtime-events`.
