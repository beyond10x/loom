---
format: aep.planning-md/3
id: upstream-blocker:connectors-attempt-id
kind: upstream-blocker
status: cleared
title: connectors-client returns no Connector attempt id for an invocation
relations:
- blocks: story:connectors-invoker
revision: 6
transitions:
- {from: "open", to: "cleared", at: "2026-10-08T15:55:36Z", actor: "human:timo", revision: 6}
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

Connectors `story:invoke-returns-attempt-id` (draft in the Connectors store on 2026-10-08) carries the change. Connectors will return the attempt id on the HTTP path through `connectors-client`, in a new v1alpha2 invoke response carrying `MutationObservation`; v1alpha1 stays unchanged. It is its own Connectors wave after connectors 0.34.0, 3-4 units. `story:connectors-invoker` stays blocked until a Connectors release tag ships it.

## Cleared

Connectors `v0.35.0` (tag commit `aedd89aa45`, published 2026-10-08T15:53:56Z) ships it:
`connectors_client::Client::invoke_v1alpha2` invokes on `POST /v1alpha2/invoke` and returns
`Invoked` with the optional `MutationObservation`, whose `attempt: {instance, id}` names the
Connector attempt an admitted `external_write` recorded; a failure keeps the host's `mutation` when
one was recorded. Read from the GitHub Release notes of `v0.35.0`. `story:connectors-invoker` pins
that tag.

The second question (a released provider with file-edit and test-run operations) is not answered
by this release and does not gate the story's acceptance, which runs against an in-process fake
Connectors service.
