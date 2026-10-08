---
format: aep.planning-md/3
id: story:connector-read-performed
kind: story
status: draft
title: A read through Connectors answers Performed
relations:
- serves: vision:O3
- decomposes: epic:effect-bindings
revision: 1
---
## Outcome

A read action bound to a Connector operation is answered `Performed` through
`b10x-loom-connectors`. Today it answers `Err`: Connectors `v0.35.0` records an attempt
(`mutation`) only for an admitted `external_write`, and `ConnectorEffects` treats a `Performed`
without an attempt as a failure to answer (`crates/loom-commission/src/ports/connector.rs`). Reads
and consequential actions take one path (read-action-effect-path, reading A), so the two rules
together make every read fail. Found by adversary pass 1 of wave 2026-10-08-w5 (finding 4).

## Acceptance

An admitted read action bound to a Connector read operation, against an in-process fake Connectors
service, answers `Performed` with the operation's result, and a consequential action still answers
`Err` when the service records no attempt.

## ESS first

Decide in `ess/commission/domains/responsibility.yaml` how a read's `Performed` is told apart from
a write's (the binding's operation kind, or the outcome's `attempt` being required only for
consequential actions), validate with the newest `ess`, regenerate, then implement.
