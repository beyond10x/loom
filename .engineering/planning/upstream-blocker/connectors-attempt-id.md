---
format: aep.planning-md/3
id: upstream-blocker:connectors-attempt-id
kind: upstream-blocker
status: open
title: connectors-client returns no Connector attempt id for an invocation
relations:
- blocks: story:connectors-invoker
revision: 3
---
## Waiting on

Connectors: a released `connectors-client` whose invocation result names the Connector attempt
(`connectors.mutations.AttemptRecord` id) the invocation produced. At `v0.33.0`, the latest
Connectors tag on 2026-10-08, `Client::invoke` returns only the result `Value`, and the wire
`Response` (`crates/connectors-core/src/lib.rs`) carries `version`, `request_id` and the outcome.
Loom's `ConnectorInvoker` must answer `Performed` with that attempt
(`commission.responsibility.EffectOutcomePerformed.attempt`).

Also unconfirmed: a released Connectors provider with file-edit and test-run operations, which a
coding phase needs.

## Clears when

A Connectors release tag ships the attempt id on a successful invocation;
`story:connectors-invoker` then pins that tag.

## Upstream story

Connectors `story:invoke-returns-attempt-id` (draft in the Connectors store on 2026-10-08) carries the change. `story:connectors-invoker` stays blocked until a Connectors release tag ships it.
