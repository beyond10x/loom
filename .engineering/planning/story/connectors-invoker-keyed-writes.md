---
format: aep.planning-md/3
id: story:connectors-invoker-keyed-writes
kind: story
status: active
title: ConnectorsInvoker sends writes keyed with the admitted request's idempotency key (invoke_v1alpha2_keyed)
relations:
- decomposes: epic:effect-bindings
- serves: vision:O1
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-connectors/Cargo.toml
- confidence: cited
  path: crates/loom-connectors/src/lib.rs
- confidence: inferred
  path: crates/loom-connectors/tests/adversary.rs
- confidence: cited
  path: crates/loom-connectors/tests/connectors_invoker.rs
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T16:39:59Z", actor: "human:timo", revision: 8}
- {from: "proposed", to: "active", at: "2026-10-30T16:39:59Z", actor: "human:timo", revision: 9}
---
## Outcome

`ConnectorsInvoker` sends every write it invokes on `POST /v1alpha2/invoke` with an idempotency
key, through `connectors_client::Client::invoke_v1alpha2_keyed`: the key is
`cli::idempotency_key` of the admitted request, the one the local `ConnectorsCli::invoke_write`
already sends, so one admitted request has one key on either transport. Reads stay unkeyed
(`invoke_v1alpha2`), since a key on a read is `unsupported`. A replayed write
(`mutation.replayed: true`) is `Performed` naming the original attempt; `approval_required` and
`idempotency_conflict` are `Err` naming the code, never `Refused` and never retried. The invoker
still invokes once and never resends. `connectors-client` and `connectors-core` move together from
`v0.35.0` to `v0.62.0`, the first release with the keyed client.

A host whose configuration sets no `standing_grants` refuses the key `unsupported` before dispatch
(connectors `contracts/service/compatibility.md` § 2.1, "Refused §4 members"); the invoker reports
that as `Err` that reaches the run, naming the missing standing grant and how to add one: set the
host's `service` configuration `standing_grants` to a grant file (`connectors.standing-grants/1`)
that covers the instance and operation, with a `state` directory. It never resends unkeyed
(orchestrator's amendment, D285). The kind cluster's `connectors-github` instance has standing
grants (D275), so its runs are unaffected.

**Breaking:** a Loom host that invokes writes through `ConnectorsInvoker` against a Connectors
service without `standing_grants` gets that `Err` where the write was dispatched unkeyed.

## Why

Decision D269 (orchestrator, 2026-10-30 09:40Z): Connectors 0.62.0 keeps keyed writes under a
standing grant opt-in, and "default flips only after loom moves to invoke_v1alpha2_keyed (loom
story, next loom wave), so no consumer breaks". D283 puts it in this wave. A key also makes a
host-side retry of one admitted request a replay of its attempt rather than a second write.
Connectors v0.62.0 is released (tag `v0.62.0`, `crates/connectors-client/src/lib.rs:181`).

## Acceptance

`crates/loom-connectors/tests/connectors_invoker.rs` (fake service on a loopback port, extended):

- `a_write_is_sent_with_the_admitted_requests_idempotency_key`: the fake records the request body;
  it carries `idempotency_key` = `cli::idempotency_key(request)`; the outcome is `Performed` naming
  the attempt.
- `a_read_is_sent_without_a_key`.
- `a_replayed_write_is_performed_naming_the_original_attempt`.
- `approval_required_and_idempotency_conflict_are_errors_and_not_retried`: one request each.
- `a_host_without_standing_grants_refusing_the_key_is_an_error_naming_it`: one request, no resend;
  the error text names `standing_grants` and the grant file a host needs.
- Every existing invoker, adversary and read fan-out test passes on `v0.62.0`.

## ESS first

None expected: `ConnectorEndpoint` (`ess/commission/domains/responsibility.yaml:1161`) gains no
member. If the implementor finds the keyed outcome needs a new `ConnectorEffects` variant in the
generated model, that is an ESS change first and the story returns to the coordinator.

## Scope

- cited: `crates/loom-connectors/Cargo.toml` (the two connectors pins), `Cargo.lock`
- cited: `crates/loom-connectors/src/lib.rs` (`invoke_v1alpha2` call, about line 284, and the
  module doc, lines 1-60)
- inferred: `crates/loom-connectors/src/cli/write.rs` (`idempotency_key`, reused, not changed)
- cited: `crates/loom-connectors/tests/connectors_invoker.rs`; inferred: `tests/adversary.rs`,
  `tests/read_fan_out.rs`, `tests/adversary_read_performed.rs`,
  `crates/loom-commission-testkit/tests/read_fan_out.rs` if the v0.62.0 client changes types
- inferred: `consumers/pinned/` (it copies `Cargo.lock`), the dependency guard's lists
- AGENTS.md § Connectors (the `v0.35.0` pin line and the invoke paragraph); CHANGELOG.md is the
  coordinator's
