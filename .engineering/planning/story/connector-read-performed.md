---
format: aep.planning-md/3
id: story:connector-read-performed
kind: story
status: active
title: A read through Connectors answers Performed
relations:
- serves: vision:O3
- decomposes: epic:effect-bindings
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T17:26:31Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-08T17:26:31Z", actor: "human:timo", revision: 4}
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

## Upstream

No Connectors change is needed. Connectors `v0.35.0` anchors every admitted invocation on
`POST /v1alpha2/invoke`, read or write, in the host's execution audit before dispatch, and its
Response carries `audit_ref` and `audit_status` (release notes of `v0.35.0`); only an admitted
`external_write` also records a `mutation` with an attempt. A read's `Performed` can therefore name
the audit record (`audit_ref`, with `audit_status` `complete`) where a write's names the attempt.
Which of the two a binding must produce is Loom's to declare, from the operation's effect class in
`GET /v1/describe`.
