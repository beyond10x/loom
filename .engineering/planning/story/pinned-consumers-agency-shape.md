---
format: aep.planning-md/3
id: story:pinned-consumers-agency-shape
kind: story
status: draft
title: A pinned consumer defines an agent as data and runs it through loom_sdk alone
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:agent-domain
- depends_on: story:runtime-wrapper
revision: 1
---
## Outcome

`consumers/pinned` gains a second consumer shaped like Agency: it defines an `AgentSpec`, builds a
`Runtime` with `NoEvents` and a memory case store, and runs it through `loom_sdk` alone.

## Why

The epic's first acceptance needs proof from outside the workspace that an agent defined as data
runs on the SDK; `task consumer-check` is where that proof already lives.

## Acceptance

- `consumers/pinned/tests/agent_spec.rs` (`an_agent_defined_as_data_runs`): the run ends as the
  spec's budget and stop policy say, on recorded responses, with no network.
- `task consumer-check` runs it offline and resolves one copy of each Loom crate.

## ESS first

No new noun: it uses `loom.agent`. The red step is `an_agent_defined_as_data_runs`.

## Not in scope

Agency's own repository.

## Scope

`consumers/pinned/`, `crates/loom-xtask/tests/pinned_consumer.rs`, CHANGELOG.md.

## Ready

Waits on `story:agent-domain` and `story:runtime-wrapper`.
