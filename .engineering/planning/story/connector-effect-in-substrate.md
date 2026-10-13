---
format: aep.planning-md/3
id: story:connector-effect-in-substrate
kind: story
status: draft
title: A bound Connector invocation runs inside Substrate
relations:
- decomposes: epic:effect-bindings
- serves: vision:O1
- depends_on: story:mandate-approval-on-resume
revision: 1
---
## Outcome

A Connector invocation Commission's runtime makes through an action's binding runs inside
Substrate confinement, observed from Substrate's operation ledger, with a named refusal when
confinement is unavailable, and Loom holds no provider credential at any point.

## Why

`epic:effect-bindings`' acceptance asks for a consequential action executed through a Connector
operation inside Substrate. `ConnectorsInvoker` calls the Connectors service directly today; the
effect does not run inside Substrate.

## Depends on

Connectors and Substrate: Connectors does not run its invocations inside Substrate, and Loom has
no Substrate path for a Connector call. Upstream stories in both are to be named before this is
scoped.

## Acceptance (draft)

- One consequential action selected in Loom executes through a Connector operation inside
  Substrate after a Mandate approval (with story:mandate-authority and
  story:mandate-approval-on-resume).
