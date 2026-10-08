---
format: aep.planning-md/3
id: story:connectors-invoker
kind: story
status: active
title: A Connectors client fills ConnectorInvoker
relations:
- decomposes: epic:downstream-adoption
- serves: vision:O3
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: crates/loom-commission/src/ports/connector.rs
- confidence: inferred
  path: crates/loom-connectors/
- confidence: inferred
  path: crates/loom-sdk/Cargo.toml
- confidence: inferred
  path: crates/loom-sdk/src/lib.rs
- confidence: inferred
  path: ess/commission/domains/responsibility.yaml
- confidence: inferred
  path: generated/rust/commission/
revision: 16
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T15:55:36Z", actor: "human:timo", revision: 14}
- {from: "proposed", to: "active", at: "2026-10-08T15:55:37Z", actor: "human:timo", revision: 15}
---
## Outcome

`ConnectorEffects` performs an admitted request through the `ConnectorInvoker` port
(`crates/loom-commission/src/ports/connector.rs`), but nothing outside the testkit
(`crates/loom-commission-testkit/src/fake_invoker.rs`) implements it, so a coding phase cannot edit
files or run tests through Connectors. A new crate, `crates/loom-connectors`
(`b10x-loom-connectors`), implements `ConnectorInvoker` over Connectors' released
`connectors-client`: it resolves the binding's `instance_id` to one Connectors endpoint, invokes
`operation_id` once with the request's arguments, and answers `Performed` with the Connector
attempt, `Refused` when Connectors says nothing was performed, and `Err` otherwise. The SDK
re-exports it. Gap 3 of `epic:downstream-adoption`.

## Domain relations

- Commission -> ActionBinding, one-to-many, commission owns its bindings - inferable from
  `ess/commission/domains/responsibility.yaml` (`commission.responsibility.Commission`, relation
  `bindings`).
- ActionBinding -> Connector instance, many-to-one, by `instance_id` - inferable from
  `ess/commission/domains/responsibility.yaml` (`commission.responsibility.ActionBinding`, field
  `instance_id`); one instance is served by one endpoint - inferred from connectors
  `crates/connectors-core/src/lib.rs` at `v0.33.0`, `Descriptor.instance`, one per described
  endpoint; no ess/1 document in Loom declares the endpoint.
- EffectOutcomePerformed -> Connector attempt, zero-or-one, Connectors owns the attempt - inferable
  from `ess/commission/domains/responsibility.yaml` (`commission.responsibility.EffectOutcomePerformed`,
  field `attempt`, `commission.responsibility.ConnectorAttemptId`).

## Acceptance

`ConnectorEffects` built over the new invoker and a commission binding one action to a Connector
operation performs an admitted request of that action exactly once against an in-process fake
Connectors service and returns `Performed` naming the attempt the service reported.

## Checks

- An invocation Connectors reports as not performed, with nothing changed, answers `Refused`.
- Every other failure (transport, protocol, an unmapped error) answers `Err`, never `Refused`.
- Each `instance_id` resolves to exactly one Connectors endpoint; an unknown `instance_id` answers
  `Err` before any call.

## ESS first

Declare in `ess/commission/domains/responsibility.yaml` the host's Connector endpoint: one record
per `ConnectorInstanceId` naming its endpoint and a non-secret credential reference, with the
relation from `ActionBinding.instance_id`. Validate with the newest `ess`, run
`task commission:generate`, and implement the invoker against `generated/rust/commission/`. The red
test of the first commit is the new invoker test in `crates/loom-connectors/tests/`.

## Upstream

Pins Connectors `v0.35.0` (`aedd89aa45`). `connectors_client::Client::invoke_v1alpha2` returns
`Invoked` (the result value, the optional `MutationObservation` with `attempt: {instance, id}`,
`audit_ref`, `audit_status`) or a boxed `Failure` that keeps the host's `mutation` when one was
recorded; it never falls back to `/v1/invoke` and never resends. `Performed.attempt` is filled from
`MutationObservation.attempt`; a successful invocation with no attempt is a failure to answer
(`Err`), as the port already treats it. `Client::invoke` (v1alpha1) carries no attempt and is not
used. Still not known here: whether a released Connectors provider offers file-edit and test-run
operations; the acceptance does not depend on it.

## Notes

- `ConnectorInvoker::invoke` is synchronous and `connectors-client` is async; the crate owns the
  bridge.
- Which Connectors `ErrorCode` values mean "not performed, nothing changed" (`Refused`) versus a failure
  to answer (`Err`) is Connectors' contract to state; the story maps only what it states.
- The fake service binds a loopback port inside the test process; no external network.
